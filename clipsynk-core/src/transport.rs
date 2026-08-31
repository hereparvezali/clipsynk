use std::{collections::HashMap, error::Error, net::SocketAddr, sync::Arc};

use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, mpsc},
};
use uuid::Uuid;

use crate::{
    DEFAULT_CHANNEL_CAPACITY,
    discover::Discovery,
    errors::AppErr,
    frame::{Frame, HandShake},
};

#[derive(Debug)]
pub struct Details {
    #[allow(unused)]
    address: SocketAddr,
    out_tx: mpsc::Sender<Frame>,
}

impl Details {
    pub fn new(address: SocketAddr, out_tx: mpsc::Sender<Frame>) -> Self {
        Self { address, out_tx }
    }
}

pub type Map = HashMap<Uuid, Details>;

#[derive(Debug, Clone)]
pub struct Transport {
    pub device_id: Uuid,
    pub tcp_port: u16,
    pub peers: Arc<Mutex<Map>>,
    pub remote_tx: mpsc::Sender<Frame>,
}

impl Transport {
    pub async fn new_start(
        device_id: Uuid,
        broadcast_port: u16,
        local_rx: mpsc::Receiver<Frame>,
        remote_tx: mpsc::Sender<Frame>,
    ) -> Result<(), Box<dyn Error>> {
        let tcp_listener = TcpListener::bind("0.0.0.0:0").await?;
        let tcp_port = tcp_listener.local_addr()?.port();

        let transport = Arc::new(Self {
            device_id,
            tcp_port,
            peers: Arc::new(Mutex::new(Map::new())),
            remote_tx,
        });

        Discovery::start(device_id, tcp_port, broadcast_port, transport.clone()).await;
        transport.clone().listen(tcp_listener).await;

        let t_clone = transport.clone();
        t_clone.broadcast_local(local_rx).await;

        Ok(())
    }

    pub async fn has_peer(&self, device_id: &Uuid) -> bool {
        self.peers.lock().await.contains_key(device_id)
    }

    pub async fn connect_peer(&self, sender_sock: SocketAddr) {
        if let Ok(stream) = TcpStream::connect(sender_sock).await {
            let _ = self.handle_connection(stream).await;
        }
    }

    pub async fn listen(self: Arc<Self>, tcp_listener: TcpListener) {
        let this = self.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = tcp_listener.accept().await {
                let this = this.clone();
                tokio::spawn(async move {
                    let _ = this.handle_connection(stream).await;
                });
            }
        });
        println!("[LISTENING] {:?} port:{}", self.device_id, self.tcp_port);
    }

    pub async fn handle_connection(&self, stream: TcpStream) -> Result<(), AppErr> {
        let peer_addr = stream.peer_addr().map_err(|_| AppErr::AddressErr)?;

        let (mut rh, mut wh) = stream.into_split();
        let payload = HandShake::new(self.device_id, self.tcp_port);
        payload.write(&mut wh).await?;

        let (out_tx, mut out_rx) = mpsc::channel::<Frame>(DEFAULT_CHANNEL_CAPACITY);

        let handshake = HandShake::read(&mut rh).await?;

        {
            let mut peers = self.peers.lock().await;
            if let std::collections::hash_map::Entry::Vacant(e) = peers.entry(handshake.device_id) {
                e.insert(Details::new(peer_addr, out_tx));
                println!("[ADDED] {:?}", handshake.device_id);
            }
        }

        let writer = async move {
            while let Some(frame) = out_rx.recv().await {
                if frame.write(&mut wh).await.is_err() {
                    break;
                }
                println!("[SENT] {:?}", handshake.device_id);
            }
        };

        let remote_tx = self.remote_tx.clone();
        let reader = async move {
            while let Ok(frame) = Frame::read(&mut rh).await {
                if remote_tx.send(frame).await.is_err() {
                    break;
                }
                println!("[RECEIVED] {:?}", handshake.device_id);
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

    pub async fn broadcast_local(&self, mut local_rx: mpsc::Receiver<Frame>) {
        let peers = self.peers.clone();
        tokio::spawn(async move {
            while let Some(frame) = local_rx.recv().await {
                peers.lock().await.retain(|_id, details| {
                    match details.out_tx.try_send(frame.clone()) {
                        Ok(_) => true,
                        Err(mpsc::error::TrySendError::Full(_)) => true,
                        Err(mpsc::error::TrySendError::Closed(_)) => false,
                    }
                });
            }
        });
    }
}
