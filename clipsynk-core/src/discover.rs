use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::{net::UdpSocket, time::sleep};
use uuid::Uuid;

use crate::{frame::HandShake, transport::Transport};

pub struct Discovery;

impl Discovery {
    pub fn create_udp_socket(port: u16, broadcast: bool) -> Result<UdpSocket, std::io::Error> {
        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_nonblocking(true)?;
        sock.set_broadcast(broadcast)?;
        sock.set_reuse_address(true)?;

        #[cfg(unix)]
        sock.set_reuse_port(true)?;

        sock.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())?;
        UdpSocket::from_std(sock.into())
    }

    pub async fn start(
        device_id: Uuid,
        tcp_port: u16,
        broadcast_port: u16,
        transport: Arc<Transport>,
    ) {
        let handshake = HandShake::new(device_id, tcp_port);

        // 1. Periodic UDP Broadcaster
        tokio::spawn(async move {
            let Ok(send_socket) = Self::create_udp_socket(0, true) else {
                eprintln!("[DISCOVERY] Failed to create UDP broadcast sender socket");
                return;
            };

            let payload = match handshake.encode() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[DISCOVERY] Failed to encode handshake: {:?}", e);
                    return;
                }
            };

            let target_addr = format!("255.255.255.255:{}", broadcast_port);
            println!("[BROADCASTING] in port: {}", broadcast_port);

            while send_socket.send_to(&payload, &target_addr).await.is_ok() {
                sleep(Duration::from_secs(300)).await;
            }
        });

        // 2. Incoming Discovery Listener
        tokio::spawn(async move {
            let Ok(recv_socket) = Self::create_udp_socket(broadcast_port, false) else {
                eprintln!("[DISCOVERY] Failed to bind UDP broadcast listener on port {}", broadcast_port);
                return;
            };

            let mut buf = [0u8; 256];
            let my_addresses = local_ip_address::list_afinet_netifas()
                .unwrap_or_default()
                .iter()
                .map(|&(_, ip)| ip)
                .collect::<Vec<IpAddr>>();

            while let Ok((n, sender_addr)) = recv_socket.recv_from(&mut buf).await {
                let Ok(announce) = HandShake::decode(&buf[..n]) else {
                    continue;
                };

                if my_addresses.contains(&sender_addr.ip())
                    || transport.has_peer(&announce.device_id).await
                {
                    continue;
                }

                let sender_sock = SocketAddr::new(sender_addr.ip(), announce.tcp_port);
                let transport_clone = transport.clone();

                tokio::spawn(async move {
                    transport_clone.connect_peer(sender_sock).await;
                });
            }
        });
    }
}
