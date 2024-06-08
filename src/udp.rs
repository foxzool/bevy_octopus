use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use bevy::prelude::*;
use bytes::Bytes;
use kanal::{AsyncReceiver, AsyncSender};
use tokio::net::UdpSocket;

use crate::{
    connections::NetworkPeer,
    error::NetworkError,
    network::{LocalSocket, NetworkProtocol, NetworkRawPacket, RemoteSocket},
    network_node::NetworkNode,
    shared::AsyncRuntime,
    shared::NetworkEvent,
};

pub struct UdpPlugin;

impl Plugin for UdpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, (spawn_udp_socket,));
    }
}

#[derive(Component, Deref, DerefMut)]
pub struct UdpNode(pub UdpSocket);

#[derive(Component)]
pub struct UdpBroadcast;

async fn recv_loop(
    socket: Arc<UdpSocket>,
    recv_tx: AsyncSender<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
    max_packet_size: usize,
) {
    let mut buf: Vec<u8> = vec![0; max_packet_size];

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, from_addr)) => {
                let bytes = Bytes::copy_from_slice(&buf[..len]);
                trace!(
                    "{} Received {} bytes from {}",
                    socket.local_addr().unwrap(),
                    len,
                    from_addr
                );
                recv_tx
                    .send(NetworkRawPacket {
                        addr: from_addr,
                        bytes,
                    })
                    .await
                    .expect("Message channel has closed.");
            }
            #[cfg(target_os = "windows")]
            Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                // ignore for windows 10054 error
            }
            Err(e) => {
                event_tx
                    .send(NetworkEvent::Error(NetworkError::Listen(e)))
                    .await
                    .expect("Error channel has closed.");
            }
        }
    }
}

async fn send_loop(
    socket: Arc<UdpSocket>,
    message_receiver: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
) {
    loop {
        while let Ok(packet) = message_receiver.recv().await {
            trace!(
                "{} Sending {} bytes to {:?}",
                socket.local_addr().unwrap(),
                packet.bytes.len(),
                packet.addr,
            );

            if let Err(_e) = socket.send_to(packet.bytes.as_ref(), packet.addr).await {
                event_tx
                    .send(NetworkEvent::Error(NetworkError::SendError))
                    .await
                    .expect("Error channel has closed.");
            }
        }
    }
}

#[derive(Component, Clone)]
pub struct MulticastV4Setting {
    pub multi_addr: Ipv4Addr,
    pub interface: Ipv4Addr,
}

impl MulticastV4Setting {
    pub fn new(multi_addr: Ipv4Addr, interface: Ipv4Addr) -> Self {
        Self {
            multi_addr,
            interface,
        }
    }
}

#[derive(Component, Clone)]
pub struct MulticastV6Setting {
    pub multi_addr: Ipv6Addr,
    pub interface: u32,
}

impl MulticastV6Setting {
    pub fn new(multi_addr: Ipv6Addr, interface: u32) -> Self {
        Self {
            multi_addr,
            interface,
        }
    }
}

#[allow(clippy::type_complexity)]
fn spawn_udp_socket(
    rt: Res<AsyncRuntime>,
    mut commands: Commands,
    q_udp: Query<
        (
            Entity,
            &NetworkProtocol,
            Option<&LocalSocket>,
            Option<&RemoteSocket>,
            Option<&UdpBroadcast>,
            Option<&MulticastV4Setting>,
            Option<&MulticastV6Setting>,
        ),
        Added<NetworkProtocol>,
    >,
) {
    for (entity, protocol, opt_local_addr, opt_remote_addr, opt_broadcast, opt_v4, opt_v6) in
        q_udp.iter()
    {
        if *protocol != NetworkProtocol::UDP {
            continue;
        }

        let local_addr = opt_local_addr.cloned().unwrap_or_else(LocalSocket::default);
        let remote_addr = opt_remote_addr.cloned().map(|addr| addr.0);
        let net_node = NetworkNode::default();

        let has_broadcast = opt_broadcast.is_some();
        let opt_v4 = opt_v4.cloned();
        let opt_v6 = opt_v6.cloned();
        let listener_socket = local_addr.0;
        let recv_tx = net_node.recv_message_channel.sender.clone_async();
        let send_rx = net_node.send_message_channel.receiver.clone_async();
        let event_tx = net_node.event_channel.sender.clone_async();
        let shutdown_rx = net_node.shutdown_channel.receiver.clone_async();

        rt.spawn(async move {
            listen(
                listener_socket,
                remote_addr,
                has_broadcast,
                opt_v4,
                opt_v6,
                recv_tx,
                send_rx,
                event_tx,
                shutdown_rx,
            )
            .await
        });

        if remote_addr.is_some() {
            let peer = NetworkPeer {};
            commands
                .entity(entity)
                .insert((net_node, LocalSocket(local_addr.0), peer));
        } else {
            commands
                .entity(entity)
                .insert((net_node, LocalSocket(*local_addr)));
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn listen(
    listener_socket: SocketAddr,
    bind: Option<SocketAddr>,
    has_broadcast: bool,
    opt_v4: Option<MulticastV4Setting>,
    opt_v6: Option<MulticastV6Setting>,
    recv_tx: AsyncSender<NetworkRawPacket>,
    send_rx: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
    shutdown_rx: AsyncReceiver<()>,
) -> Result<(), std::io::Error> {
    let socket = Arc::new(UdpSocket::bind(listener_socket).await?);

    if has_broadcast {
        socket.set_broadcast(true)?;
    }

    if let Some(remote_addr) = bind {
        socket.connect(remote_addr).await?;
    }
    if let Some(multi_v4) = opt_v4 {
        debug!(
            "Joining multicast group {:?} on interface {:?}",
            multi_v4.multi_addr, multi_v4.interface
        );
        socket.join_multicast_v4(multi_v4.multi_addr, multi_v4.interface)?;
    } else if let Some(multi_v6) = opt_v6 {
        debug!(
            "Joining multicast group {:?} on interface {:?}",
            multi_v6.multi_addr, multi_v6.interface
        );
        socket.join_multicast_v6(&multi_v6.multi_addr, multi_v6.interface)?;
    }
    let shutdown_rx_clone = shutdown_rx.clone();
    let server = async move {
        info!(
            "UDP listening on {} peer: {:?}",
            socket.local_addr().unwrap(),
            socket.peer_addr().ok()
        );

        let event_tx_clone = event_tx.clone();

        tokio::select! {
            // handle shutdown signal
            _ = shutdown_rx_clone.recv() => {

            }
            // process new connection
            _ = send_loop(socket.clone(), send_rx, event_tx_clone) => {

            }

            _ = recv_loop(socket, recv_tx, event_tx, 65_507) => {

            }
        }

        println!("over");

        Ok::<(), NetworkError>(())
    };

    tokio::spawn(server);

    if let Ok(()) = shutdown_rx.recv().await {
        println!("Shutting down TCP server...");
    }

    Ok(())
}
