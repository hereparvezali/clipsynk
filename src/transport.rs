use std::{
    collections::HashMap,
    error::Error,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Mutex, mpsc},
    time::sleep,
};

use crate::{clipboard::ClipboardManager, frame::Frame};

#[derive(Debug, Serialize, Deserialize)]
pub struct Announce {
    pub ip: IpAddr,
    pub port: u16,
}

pub struct Transport {
    pub peers: Arc<Mutex<HashMap<IpAddr, u16>>>,
    pub tcp_listener: Option<TcpListener>,
}
impl Transport {
    pub async fn new_start(
        local_rx: mpsc::UnboundedReceiver<Frame>,
        clipboard: ClipboardManager,
    ) -> Result<(), Box<dyn Error>> {
        let tcp_listener = TcpListener::bind("0.0.0.0:0").await?;
        let mut transport = Self {
            peers: Arc::new(Mutex::new(HashMap::<IpAddr, u16>::new())),
            tcp_listener: Some(tcp_listener),
        };

        transport.discover().await;
        transport.broadcast_local(local_rx).await;
        transport.listen(Arc::new(Mutex::new(clipboard))).await;
        Ok(())
    }
    async fn discover(&mut self) {
        let tcp_listener = self
            .tcp_listener
            .as_ref()
            .expect("TcpListener didn't crate!");
        let announce = Announce {
            ip: tcp_listener.local_addr().unwrap().ip(),
            port: tcp_listener.local_addr().unwrap().port(),
        };

        let send_socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        send_socket.set_broadcast(true).unwrap();

        tokio::spawn(async move {
            let payload = serde_json::to_string(&announce).unwrap();
            loop {
                send_socket
                    .send_to(payload.as_bytes(), "255.255.255.255:4000")
                    .await
                    .unwrap();
                println!("Udp broadcast sent");
                sleep(Duration::from_secs(10)).await;
            }
        });

        let peers = self.peers.clone();
        tokio::spawn(async move {
            let recv_socket = UdpSocket::bind("255.255.255.255:4000").await.unwrap();
            let mut buf = [0u8; 256];
            loop {
                let (n, sender_addr) = recv_socket.recv_from(&mut buf).await.unwrap();
                println!("Udp broadcast received");
                let Ok(announce) = serde_json::from_slice::<Announce>(&buf[..n]) else {
                    continue;
                };
                // peers.lock().await.insert(sender_addr.ip(), announce.port);
                peers
                    .lock()
                    .await
                    .entry(sender_addr.ip())
                    .or_insert(announce.port);
                println!("{:?}:{} added to peers", sender_addr.ip(), announce.port);
            }
        });
    }

    pub async fn broadcast_local(&mut self, mut local_rx: mpsc::UnboundedReceiver<Frame>) {
        let peers = self.peers.clone();

        tokio::spawn(async move {
            while let Some(frame) = local_rx.recv().await {
                let peers_vec: Vec<(IpAddr, u16)> = peers
                    .lock()
                    .await
                    .iter()
                    .map(|(&ip, &port)| (ip, port))
                    .collect();
                let frame_encoded = frame.encode().unwrap();

                for peer in peers_vec.into_iter() {
                    let peers = peers.clone();
                    let frame_encoded = frame_encoded.clone();

                    tokio::spawn(async move {
                        if let Ok(mut stream) =
                            TcpStream::connect(SocketAddr::new(peer.0.clone(), peer.1.clone()))
                                .await
                        {
                            let _ = stream.write_all(&frame_encoded).await;
                            let _ = stream.shutdown().await;
                        } else {
                            peers.lock().await.remove(&peer.0);
                        }
                    });
                }
            }
        });
    }
    async fn listen(&mut self, clipboard: Arc<Mutex<ClipboardManager>>) {
        let tcp_listener = self.tcp_listener.take().expect("TcpListener not exist!");

        while let Ok((mut stream, addr)) = tcp_listener.accept().await {
            let cp = clipboard.clone();
            println!("{:?} connected to Tcp", addr.clone().ip());
            tokio::spawn(async move {
                let mut buff = Vec::with_capacity(1000);
                let n = stream.read_to_end(&mut buff).await.unwrap();
                let frame = Frame::new(&buff[..n]);
                let mut cp = cp.lock().await;
                if cp.hash != frame.hash && cp.timestamp < frame.timestamp {
                    cp.hash = frame.hash;
                    cp.timestamp = frame.timestamp;
                    cp.set_content(&frame.bytes).await;
                }
                println!("{:?} sent content successfully", addr.ip());
            });
        }
    }
}
