use std::{
    collections::HashMap,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Mutex, mpsc},
    time::sleep,
};
use uuid::Uuid;

use crate::{
    clipboard::ClipboardManager,
    errors::AppErr,
    frame::{Frame, HandShake},
};

#[derive(Debug)]
pub struct Details {
    #[allow(unused)]
    address: SocketAddr,
    out_tx: mpsc::UnboundedSender<Frame>,
}
impl Details {
    pub fn new(address: SocketAddr, out_tx: mpsc::UnboundedSender<Frame>) -> Self {
        Self { address, out_tx }
    }
}

pub type Map = HashMap<Uuid, Details>;

#[derive(Debug, Clone)]
pub struct Transport {
    pub device_id: Uuid,
    pub tcp_port: u16,
    pub peers: Arc<Mutex<Map>>,
    pub cm: ClipboardManager,
}

impl Transport {
    pub async fn new_start(
        device_id: Uuid,
        broadcast_port: u16,
        local_rx: mpsc::UnboundedReceiver<Frame>,
        cm: ClipboardManager,
    ) -> Result<(), Box<dyn Error>> {
        let tcp_listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let tcp_port = tcp_listener.local_addr().unwrap().port();
        let transport = Self {
            device_id,
            tcp_port,
            peers: Arc::new(Mutex::new(Map::new())),
            cm,
        };

        transport.discover(broadcast_port).await;
        transport.listen(tcp_listener).await;
        transport.broadcast_local(local_rx).await;
        Ok(())
    }
    fn create_udp_socket(port: u16, broadcast: bool) -> UdpSocket {
        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
        sock.set_nonblocking(true).unwrap();
        sock.set_broadcast(broadcast).unwrap();
        sock.set_reuse_address(true).unwrap();

        #[cfg(unix)]
        sock.set_reuse_port(true).unwrap();

        sock.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())
            .expect("Broadcast ports binding failed");
        UdpSocket::from_std(sock.into())
            .expect("Coversion of socket2 to tokio::net::UdpSocket failed")
    }
    async fn discover(&self, broadcast_port: u16) {
        let handshake = HandShake::new(self.device_id, self.tcp_port);
        let send_socket = Self::create_udp_socket(0, true);

        tokio::spawn(async move {
            let payload = handshake.encode().unwrap();
            println!("[BROADCASTING] in port: {}", broadcast_port);
            while send_socket
                .send_to(&payload, format!("255.255.255.255:{}", broadcast_port))
                .await
                .is_ok()
            {
                sleep(Duration::from_secs(300)).await;
            }
        });

        let this = self.clone();
        tokio::spawn(async move {
            let recv_socket = Self::create_udp_socket(broadcast_port, false);
            let mut buf = [0u8; 256];
            let my_addresses = local_ip_address::list_afinet_netifas()
                .unwrap()
                .iter()
                .map(|&(_, i)| i)
                .collect::<Vec<IpAddr>>();

            while let Ok((n, sender_addr)) = recv_socket.recv_from(&mut buf).await {
                let Ok(announce) = HandShake::decode(&buf[..n]) else {
                    continue;
                };

                let sender_sock = SocketAddr::new(sender_addr.ip(), announce.tcp_port);

                let this = this.clone();
                if my_addresses.contains(&sender_addr.ip())
                    || this.peers.lock().await.contains_key(&announce.device_id)
                {
                    continue;
                }

                tokio::spawn(async move {
                    let Ok(stream) = TcpStream::connect(sender_sock).await else {
                        return;
                    };
                    let _ = this.handle_connection(stream).await;
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
                    let _ = this.handle_connection(stream).await;
                    println!("[LISTENING] {:?} port:{}", this.device_id, this.tcp_port);
                });
            }
        });
    }
    pub async fn handle_connection(&self, stream: TcpStream) -> Result<(), AppErr> {
        let peer_addr = stream.peer_addr().map_err(|_| AppErr::AddressErr)?;

        let (mut rh, mut wh) = stream.into_split();
        let payload = HandShake::new(self.device_id, self.tcp_port);
        payload.write(&mut wh).await?;

        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Frame>();

        let handshake = HandShake::read(&mut rh).await?;

        {
            let mut peers = self.peers.lock().await;
            if !peers.contains_key(&handshake.device_id) {
                peers.insert(handshake.device_id, Details::new(peer_addr, out_tx));
                println!("[ADDED] {:?}", handshake.device_id);
            }
        }

        let writer = async move {
            while let Some(frame) = out_rx.recv().await {
                if frame.write(&mut wh).await.is_err() {
                    break;
                }
            }
        };

        let cm = self.cm.clone();
        let reader = async move {
            while let Ok(frame) = Frame::read(&mut rh).await {
                cm.resolve(frame).await;
            }
        };

        tokio::select! {
            _ = writer => {
                println!("[handle_connection] writing discarded!")
            }
            _ = reader => {
                println!("[handle_connection] reading discarded!")
            }
        }
        self.peers.lock().await.remove(&handshake.device_id);
        println!("[REMOVED] {:?}", handshake.device_id);

        Ok(())
    }

    pub async fn broadcast_local(&self, mut local_rx: mpsc::UnboundedReceiver<Frame>) {
        let peers = self.peers.clone();
        while let Some(frame) = local_rx.recv().await {
            peers.lock().await.retain(|id, details| {
                println!("[SENDING] {:?}", id);
                details.out_tx.send(frame.clone()).is_ok()
            });
        }
    }
}
