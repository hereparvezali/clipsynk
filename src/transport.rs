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
    pub peers: Arc<Mutex<HashMap<SocketAddr, mpsc::UnboundedReceiver<Frame>>>>,
    pub tcp_listener: Option<TcpListener>,
}
impl Transport {
    pub async fn new_start(
        local_rx: mpsc::UnboundedReceiver<Frame>,
        cm: ClipboardManager,
    ) -> Result<(), Box<dyn Error>> {
        let tcp_listener = TcpListener::bind("0.0.0.0:0").await?;
        let mut transport = Self {
            peers: Arc::new(Mutex::new(HashMap::<
                SocketAddr,
                mpsc::UnboundedReceiver<Frame>,
            >::new())),
            tcp_listener: Some(tcp_listener),
        };

        transport.discover().await;
        transport.broadcast_local(local_rx).await;
        transport.listen(cm.clone()).await;
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
                dbg!("Udp broadcast sent");
                sleep(Duration::from_secs(20)).await;
            }
        });

        let peers = self.peers.clone();
        tokio::spawn(async move {
            let recv_socket = UdpSocket::bind("0.0.0.0:4000").await.unwrap();
            let mut buf = [0u8; 256];
            let my_addresses = local_ip_address::list_afinet_netifas()
                .unwrap()
                .iter()
                .map(|&(_, i)| i)
                .collect::<Vec<IpAddr>>();
            while let Ok((n, sender_addr)) = recv_socket.recv_from(&mut buf).await {
                let Ok(announce) = serde_json::from_slice::<Announce>(&buf[..n]) else {
                    continue;
                };
                if my_addresses.contains(&sender_addr.ip()) {
                    continue;
                }
                peers.lock().await.insert();
                dbg!("{:?}:{} added to peers", sender_addr.ip(), announce.port);
            }
        });
    }

    pub async fn broadcast_local(&mut self, mut local_rx: mpsc::UnboundedReceiver<Frame>) {
        // let peers = self.peers.clone();

        // tokio::spawn(async move {
        //     while let Some(frame) = local_rx.recv().await {
        //         let peers_vec: Vec<(IpAddr, u16)> = peers
        //             .lock()
        //             .await
        //             .iter()
        //             .map(|(&ip, &port)| (ip, port))
        //             .collect();
        //         let frame_encoded = frame.encode().unwrap();

        //         for peer in peers_vec.into_iter() {
        //             let peers = peers.clone();
        //             let frame_encoded = frame_encoded.clone();

        //             tokio::spawn(async move {
        //                 let target_addr = SocketAddr::new(peer.0, peer.1);

        //                 let connect_result = tokio::time::timeout(
        //                     std::time::Duration::from_secs(3),
        //                     TcpStream::connect(target_addr),
        //                 )
        //                 .await;

        //                 match connect_result {
        //                     Ok(Ok(mut stream)) => {
        //                         if let Err(e) = stream.write_all(&frame_encoded).await {
        //                             dbg!("[TCP Error] Failed to write to {}: {:?}", target_addr, e);
        //                         }
        //                         let _ = stream.shutdown().await;
        //                     }
        //                     _ => {
        //                         dbg!("[TCP] Peer {} unreachable, removing from peer list", peer.0);
        //                         peers.lock().await.remove(&peer.0);
        //                     }
        //                 }
        //             });
        //         }
        //     }
        // });
    }
    async fn listen(&mut self, cm: ClipboardManager) {
        let tcp_listener = self.tcp_listener.take().expect("TcpListener not exist!");
        let cm = cm.clone();

        while let Ok((mut stream, addr)) = tcp_listener.accept().await {
            let cm = cm.clone();

            dbg!("{:?} connected to Tcp", addr.clone().ip());
            tokio::spawn(async move {
                let mut buff = Vec::with_capacity(1000);
                let n = stream.read_to_end(&mut buff).await.unwrap();
                let frame = Frame::decode(&buff[..n]).unwrap();

                if cm.clipboard.lock().await.hash != frame.hash
                    && cm.clipboard.lock().await.timestamp < frame.timestamp
                {
                    let mut cb = cm.clipboard.lock().await;
                    cb.hash = frame.hash;
                    cb.timestamp = frame.timestamp;
                    cm.set_content(&frame.bytes).await;
                }
                dbg!("{:?} Got content successfully", addr.ip());
            });
        }
    }
}
