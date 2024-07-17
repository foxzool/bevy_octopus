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
    network_node::{
        ListenTo, NetworkEvent, NetworkNode, NetworkPeer, NetworkRawPacket, RemoteAddr, ServerAddr,
    },
};

pub struct UdpPlugin;

impl Plugin for UdpPlugin {
    fn build(&self, app: &mut App) {
        app.observe(on_listen_to);
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
            Err(ref e) if e.kind() == io::ErrorKind::ConnectionReset => {
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
                // trace!("Data sent to {} successfully", addr);
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

    debug!(
        "UDP listening on {} peer: {:?}",
        socket.local_addr().unwrap(),
        socket.peer_addr().ok()
    );

    event_tx.send(NetworkEvent::Listen).await?;

    let tasks = vec![
        task::spawn(send_loop(socket.clone(), send_rx)),
        task::spawn(recv_loop(socket, recv_tx, 65_507)),
    ];

    if let Err(err) = future::try_join_all(tasks).await {
        let _ = event_tx.send(NetworkEvent::Error(err)).await;
    }

    Ok(())
}

#[allow(clippy::type_complexity)]
fn on_listen_to(
    trigger: Trigger<ListenTo>,
    q_udp: Query<
        (
            &NetworkNode,
            &ServerAddr,
            Option<&RemoteAddr>,
            Option<&UdpBroadcast>,
            Option<&MulticastV4Setting>,
            Option<&MulticastV6Setting>,
        ),
        Without<NetworkPeer>,
    >,
) {
    if let Ok((net_node, server_addr, opt_remote_addr, opt_broadcast, opt_v4, opt_v6)) =
        q_udp.get(trigger.entity())
    {
        if "udp" != server_addr.scheme() {
            return;
        }

        let local_addr = server_addr.local_addr();

        let remote_addr = opt_remote_addr.map(|remote_addr| remote_addr.peer_addr());

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
                        Err(e) => Err(NetworkError::RxReceiveError(e)),
                    }
                }),
            ];

            if let Err(err) = future::try_join_all(tasks).await {
                let _ = event_tx.send(NetworkEvent::Error(err)).await;
            }
        });
    }
}
