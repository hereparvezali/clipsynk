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
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct Transport {
    pub peers: Arc<Mutex<HashMap<IpAddr, (u16, mpsc::UnboundedSender<Frame>)>>>,
    pub cm: ClipboardManager,
}

impl Transport {
    pub async fn new_start(
        local_rx: mpsc::UnboundedReceiver<Frame>,
        cm: ClipboardManager,
    ) -> Result<(), Box<dyn Error>> {
        let tcp_listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = tcp_listener.local_addr().unwrap().port();
        let transport = Self {
            peers: Arc::new(Mutex::new(HashMap::<
                IpAddr,
                (u16, mpsc::UnboundedSender<Frame>),
            >::new())),
            cm,
        };

        transport.discover(port).await;
        transport.listen(tcp_listener).await;
        transport.broadcast_local(local_rx).await;
        Ok(())
    }
    async fn discover(&self, port: u16) {
        let announce = Announce { port };

        let send_socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        send_socket.set_broadcast(true).unwrap();

        tokio::spawn(async move {
            let payload = serde_json::to_string(&announce).unwrap();
            loop {
                send_socket
                    .send_to(payload.as_bytes(), "255.255.255.255:4000")
                    .await
                    .unwrap();
                sleep(Duration::from_secs(20)).await;
            }
        });

        let this = self.clone();
        tokio::spawn(async move {
            let recv_socket = UdpSocket::bind("0.0.0.0:4000").await.unwrap();
            let mut buf = [0u8; 256];
            let my_addresses = local_ip_address::list_afinet_netifas()
                .unwrap()
                .iter()
                .map(|&(_, i)| i)
                .collect::<Vec<IpAddr>>();
            let this = this.clone();
            while let Ok((n, sender_addr)) = recv_socket.recv_from(&mut buf).await {
                let Ok(announce) = serde_json::from_slice::<Announce>(&buf[..n]) else {
                    continue;
                };
                let sender_sock = SocketAddr::new(sender_addr.ip(), announce.port);
                let this = this.clone();
                if my_addresses.contains(&sender_addr.ip())
                    || this.peers.lock().await.contains_key(&sender_sock.ip())
                {
                    continue;
                }
                tokio::spawn(async move {
                    let Ok(stream) = TcpStream::connect(sender_sock).await else {
                        return;
                    };
                    this.handle_connection(stream).await;
                });
            }
        });
    }
    async fn listen(&self, tcp_listener: TcpListener) {
        let this = self.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = tcp_listener.accept().await {
                let this = this.clone();
                tokio::spawn(async move {
                    this.handle_connection(stream).await;
                });
            }
        });
    }
    pub async fn handle_connection(&self, stream: TcpStream) {
        let peers = self.peers.clone();
        let peer_addr = stream.peer_addr().unwrap().clone();
        let (mut rh, mut wh) = stream.into_split();

        let (wh_tx, mut wh_rx) = mpsc::unbounded_channel::<Frame>();
        tokio::spawn(async move {
            while let Some(frame) = wh_rx.recv().await {
                let peers = peers.clone();
                let payload = frame.encode().unwrap();
                let len = (payload.len() as u32).to_be_bytes();
                if wh.write_all(&len).await.is_err() || wh.write_all(&payload).await.is_err() {
                    let _ = peers.lock().await.remove(&peer_addr.ip());
                    break;
                }
            }
        });
        self.peers
            .lock()
            .await
            .entry(peer_addr.ip())
            .or_insert((peer_addr.port(), wh_tx));

        let cm = self.cm.clone();
        tokio::spawn(async move {
            let mut len_buf = [0u8; 4];
            while rh.read_exact(&mut len_buf).await.is_ok() {
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut data = vec![0u8; len];
                if rh.read_exact(&mut data).await.is_err() {
                    break;
                }
                if let Ok(frame) = Frame::decode(&data) {
                    cm.resolve(frame).await;
                }
            }
        });
    }

    pub async fn broadcast_local(&self, mut local_rx: mpsc::UnboundedReceiver<Frame>) {
        let peers = self.peers.clone();
        while let Some(frame) = local_rx.recv().await {
            let mut peers = peers.lock().await;
            peers.retain(|_, (_, tx)| tx.send(frame.clone()).is_ok());
            dbg!("{:?}", peers.clone());
        }
    }
}
