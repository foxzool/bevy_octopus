use std::{
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use async_std::{future::timeout, net::UdpSocket, task};
use bevy::prelude::*;
use bytes::Bytes;
use futures::future;
use kanal::{AsyncReceiver, AsyncSender};

use crate::{
    error::NetworkError,
    network::{ConnectTo, ListenTo, NetworkRawPacket},
    network_node::NetworkNode,
    peer::NetworkPeer,
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
    max_packet_size: usize,
) -> Result<(), NetworkError> {
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
                let _ = recv_tx.send(NetworkRawPacket::new(from_addr, bytes)).await;
            }
            #[cfg(target_os = "windows")]
            Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                // ignore for windows 10054 error
            }
            Err(e) => return Err(NetworkError::Listen(e)),
        }
    }
}

async fn send_loop(
    socket: Arc<UdpSocket>,
    message_receiver: AsyncReceiver<NetworkRawPacket>,
) -> Result<(), NetworkError> {
    while let Ok(packet) = message_receiver.recv().await {
        trace!(
            "{} Sending {} bytes to {:?}",
            socket.local_addr().unwrap(),
            packet.bytes.len(),
            packet.addr,
        );
        let arr: Vec<&str> = packet.addr.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];

        let max_retries = 5;
        let timeout_duration = Duration::from_secs(1);
        send_data(&socket, s, &packet.bytes, max_retries, timeout_duration).await?;
    }

    Ok(())
}

async fn send_data(
    socket: &UdpSocket,
    addr: &str,
    data: &[u8],
    max_retries: usize,
    timeout_duration: Duration,
) -> io::Result<()> {
    let mut attempts = 0;

    while attempts < max_retries {
        match timeout(timeout_duration, socket.send_to(data, addr)).await {
            Ok(Ok(_)) => {
                trace!("Data sent to {} successfully", addr);
                return Ok(());
            }
            Ok(Err(e)) if e.kind() == io::ErrorKind::ConnectionRefused => {
                trace!("Connection refused: {}", e);
                // Optionally, wait a bit before retrying
                task::sleep(Duration::from_secs(1)).await;
            }
            Ok(Err(e)) => {
                error!("Failed to send data: {}", e);
                return Err(e);
            }
            Err(_) => {
                error!("Send attempt timed out");
            }
        }

        attempts += 1;
        trace!("Retrying... attempt {}/{}", attempts, max_retries);
    }

    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "Failed to send data within retry limit",
    ))
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
    mut commands: Commands,
    q_udp: Query<
        (
            Entity,
            Option<&ListenTo>,
            Option<&ConnectTo>,
            Option<&UdpBroadcast>,
            Option<&MulticastV4Setting>,
            Option<&MulticastV6Setting>,
        ),
        Or<(Added<ListenTo>, Added<ConnectTo>)>,
    >,
) {
    for (entity, opt_listen_to, opt_connect_to, opt_broadcast, opt_v4, opt_v6) in q_udp.iter() {
        let mut local_addr = "0.0.0.0:0".parse::<SocketAddr>().unwrap();
        if let Some(listen_to) = opt_listen_to {
            if listen_to.scheme() == "udp" {
                local_addr = listen_to.local_addr();
            } else {
                continue;
            }
        };

        let mut remote_addr = None;

        if let Some(connect_to) = opt_connect_to {
            if connect_to.scheme() == "udp" {
                remote_addr = Some(connect_to.peer_addr())
            } else {
                continue;
            }
        };

        let net_node = NetworkNode::default();

        let has_broadcast = opt_broadcast.is_some();
        let opt_v4 = opt_v4.cloned();
        let opt_v6 = opt_v6.cloned();
        let listener_socket = local_addr;
        let recv_tx = net_node.recv_message_channel.sender.clone_async();
        let send_rx = net_node.send_message_channel.receiver.clone_async();
        let event_tx = net_node.event_channel.sender.clone_async();
        let shutdown_rx = net_node.shutdown_channel.receiver.clone_async();

        task::spawn(async move {
            let tasks = vec![
                task::spawn(listen(
                    listener_socket,
                    remote_addr,
                    has_broadcast,
                    opt_v4,
                    opt_v6,
                    recv_tx,
                    send_rx,
                    event_tx.clone(),
                )),
                task::spawn(async move {
                    match shutdown_rx.recv().await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(NetworkError::ReceiveError(e)),
                    }
                }),
            ];

            match future::try_join_all(tasks).await {
                Ok(_) => {}
                Err(err) => {
                    let _ = event_tx.send(NetworkEvent::Error(err)).await;
                }
            }
        });

        if remote_addr.is_some() {
            let peer = NetworkPeer {};
            commands.entity(entity).insert((net_node, peer));
        } else {
            commands.entity(entity).insert((net_node,));
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
) -> Result<(), NetworkError> {
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

    info!(
        "UDP listening on {} peer: {:?}",
        socket.local_addr().unwrap(),
        socket.peer_addr().ok()
    );

    let tasks = vec![
        task::spawn(send_loop(socket.clone(), send_rx)),
        task::spawn(recv_loop(socket, recv_tx, 65_507)),
    ];

    match future::try_join_all(tasks).await {
        Ok(_) => {}
        Err(err) => {
            let _ = event_tx.send(NetworkEvent::Error(err)).await;
        }
    }

    Ok(())
}
