use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, UdpSocket},
    sync::mpsc,
};

#[derive(Debug)]
pub struct Peer {
    pub device_id: String,
    pub addr: SocketAddr,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Announce {
    pub device_id: String,
    pub port: u16,
}

pub async fn discover(device_id: impl Into<String>) -> (TcpListener, mpsc::Receiver<Peer>) {
    let tcp_listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let announce = Announce {
        device_id: device_id.into(),
        port: tcp_listener.local_addr().unwrap().port(),
    };

    let (tx, rx) = mpsc::channel::<Peer>(100);

    let send_socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    send_socket.set_broadcast(true).unwrap();

    send_socket
        .send_to(
            serde_json::to_string(&announce).unwrap().as_bytes(),
            "255.255.255.255:4000",
        )
        .await
        .unwrap();

    tokio::spawn(async move {
        let recv_socket = UdpSocket::bind("255.255.255.255:4000").await.unwrap();
        let mut buf = [0u8; 100];
        loop {
            let (_, sender_addr) = recv_socket.recv_from(&mut buf).await.unwrap();
            let Ok(announce) = serde_json::from_slice::<Announce>(&buf) else {
                continue;
            };
            let peer = Peer {
                device_id: announce.device_id,
                addr: sender_addr,
            };
            tx.send(peer).await.unwrap();
        }
    });
    (tcp_listener, rx)
}
