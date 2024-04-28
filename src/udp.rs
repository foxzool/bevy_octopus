use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{atomic::AtomicBool, Arc},
};

use async_net::UdpSocket;
use bevy::{prelude::*, tasks::IoTaskPool};
use bytes::Bytes;
use kanal::{AsyncReceiver, AsyncSender};

use crate::network::{LocalSocket, NetworkEvent, RemoteSocket};
use crate::network_manager::NetworkNode;
use crate::{
    error::NetworkError,
    network::{NetworkProtocol, NetworkRawPacket},
    AsyncChannel,
};

pub struct UdpPlugin;

impl Plugin for UdpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, (spawn_udp_socket, control_udp_node));
    }
}

#[derive(Component)]
pub struct UdpNode {
    new_socket: AsyncChannel<UdpSocket>,
}

impl Default for UdpNode {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Component)]
pub struct UdpBroadcast;

impl UdpNode {
    pub fn new() -> Self {
        Self {
            new_socket: AsyncChannel::new(),
        }
    }

    /// Start the UDP node
    pub fn start(&mut self, socket: UdpSocket, net_node: &mut NetworkNode) {
        let socket1 = socket.clone();
        let cancel_flag = net_node.cancel_flag.clone();
        let recv_sender = net_node.recv_channel().sender.clone_async();
        let error_sender = net_node.error_channel().sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                recv_loop(
                    socket1.clone(),
                    recv_sender,
                    error_sender.clone(),
                    cancel_flag.clone(),
                    65_507,
                )
                .await;
            })
            .detach();

        let socket = socket.clone();
        let cancel_flag = net_node.cancel_flag.clone();
        let message_receiver = net_node.send_channel().receiver.clone_async();
        let error_sender = net_node.error_channel().sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                send_loop(socket, message_receiver, error_sender, cancel_flag).await;
            })
            .detach();

        net_node.start();
    }

    pub fn stop(&mut self, local_addr: SocketAddr, net_node: &mut NetworkNode) {
        // this is a hack to send a message to the server to shut down
        net_node
            .send_channel()
            .sender
            .try_send(NetworkRawPacket {
                socket: local_addr,
                bytes: Bytes::from_static(b"shutdown"),
            })
            .expect("Message channel has closed.");

        // net_node.stop();
    }

    //
    // pub fn join_multicast_v4(
    //     &self,
    //     net_node: &NetworkNode,
    //     multi_addr: Ipv4Addr,
    //     interface: Ipv4Addr,
    // ) {
    //     let socket = self.socket.clone();
    //     let error_sender = net_node.error_channel().sender.clone();
    //     match socket.join_multicast_v4(multi_addr, interface) {
    //         Ok(_) => {
    //             debug!(
    //                 "{} Joined multicast group {} on interface {}",
    //                 socket.local_addr().unwrap(),
    //                 multi_addr,
    //                 interface
    //             );
    //         }
    //         Err(e) => {
    //             error_sender
    //                 .send(NetworkError::Error(format!(
    //                     "Failed to join multicast group: {}",
    //                     e
    //                 )))
    //                 .expect("Error channel has closed.");
    //         }
    //     }
    // }
    //
    // pub fn leave_multicast_v4(
    //     &self,
    //     net_node: &NetworkNode,
    //     multi_addr: Ipv4Addr,
    //     interface: Ipv4Addr,
    // ) {
    //     let socket = self.socket.clone();
    //     let error_sender = net_node.error_channel().sender.clone();
    //     match socket.leave_multicast_v4(multi_addr, interface) {
    //         Ok(_) => {
    //             debug!(
    //                 "{} Left multicast group {} on interface {}",
    //                 socket.local_addr().unwrap(),
    //                 multi_addr,
    //                 interface
    //             );
    //         }
    //         Err(e) => {
    //             error_sender
    //                 .send(NetworkError::Error(format!(
    //                     "Failed to leave multicast group: {}",
    //                     e
    //                 )))
    //                 .expect("Error channel has closed.");
    //         }
    //     }
    // }
    //
    // pub fn join_multicast_v6(&self, net_node: &NetworkNode, multi_addr: Ipv6Addr, interface: u32) {
    //     let socket = self.socket.clone();
    //     let error_sender = net_node.error_channel().sender.clone();
    //     match socket.join_multicast_v6(&multi_addr, interface) {
    //         Ok(_) => {
    //             debug!(
    //                 "{} Joined multicast group {} on interface {}",
    //                 socket.local_addr().unwrap(),
    //                 multi_addr,
    //                 interface
    //             );
    //         }
    //         Err(e) => {
    //             error_sender
    //                 .send(NetworkError::Error(format!(
    //                     "Failed to join multicast group: {}",
    //                     e
    //                 )))
    //                 .expect("Error channel has closed.");
    //         }
    //     }
    // }
    //
    // pub fn leave_multicast_v6(&self, net_node: &NetworkNode, multi_addr: Ipv6Addr, interface: u32) {
    //     let socket = self.socket.clone();
    //     let error_sender = net_node.error_channel().sender.clone();
    //     match socket.leave_multicast_v6(&multi_addr, interface) {
    //         Ok(_) => {
    //             debug!(
    //                 "{} Left multicast group {} on interface {}",
    //                 socket.local_addr().unwrap(),
    //                 multi_addr,
    //                 interface
    //             );
    //         }
    //         Err(e) => {
    //             error_sender
    //                 .send(NetworkError::Error(format!(
    //                     "Failed to leave multicast group: {}",
    //                     e
    //                 )))
    //                 .expect("Error channel has closed.");
    //         }
    //     }
    // }
}

async fn recv_loop(
    socket: UdpSocket,
    message_sender: AsyncSender<NetworkRawPacket>,
    error_sender: AsyncSender<NetworkError>,
    cancel_flag: Arc<AtomicBool>,
    max_packet_size: usize,
) {
    let mut buf: Vec<u8> = vec![0; max_packet_size];

    loop {
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        match socket.recv_from(&mut buf).await {
            Ok((len, from_addr)) => {
                let bytes = Bytes::copy_from_slice(&buf[..len]);
                trace!(
                    "{} Received {} bytes from {}",
                    socket.local_addr().unwrap(),
                    len,
                    from_addr
                );
                message_sender
                    .send(NetworkRawPacket {
                        socket: from_addr,
                        bytes,
                    })
                    .await
                    .expect("Message channel has closed.");
            }
            Err(e) => {
                error_sender
                    .send(NetworkError::Listen(e))
                    .await
                    .expect("Error channel has closed.");
            }
        }
    }
}

async fn send_loop(
    socket: UdpSocket,
    message_receiver: AsyncReceiver<NetworkRawPacket>,
    error_sender: AsyncSender<NetworkError>,
    cancel_flag: Arc<AtomicBool>,
) {
    loop {
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        while let Ok(packet) = message_receiver.recv().await {
            trace!(
                "{} Sending {} bytes to {:?}",
                socket.local_addr().unwrap(),
                packet.bytes.len(),
                packet.socket,
            );

            if let Err(_e) = socket.send_to(packet.bytes.as_ref(), packet.socket).await {
                error_sender
                    .send(NetworkError::SendError)
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
    mut commands: Commands,
    q_udp: Query<
        (
            Entity,
            &UdpNode,
            Option<&LocalSocket>,
            Option<&RemoteSocket>,
            Option<&UdpBroadcast>,
            Option<&MulticastV4Setting>,
            Option<&MulticastV6Setting>,
        ),
        Added<UdpNode>,
    >,
) {
    for (entity, udp_node, opt_local_addr, opt_remote_addr, opt_broadcast, opt_v4, opt_v6) in
        q_udp.iter()
    {
        let local_addr = opt_local_addr.cloned().unwrap_or_else(LocalSocket::default);
        let remote_addr = opt_remote_addr.cloned();
        let net_node = NetworkNode::new(
            NetworkProtocol::UDP,
            Some(local_addr.0),
            opt_remote_addr.map(|addr| addr.0),
        );

        let has_broadcast = opt_broadcast.is_some();
        let opt_v4 = opt_v4.cloned();
        let opt_v6 = opt_v6.cloned();

        let listener_socket = local_addr.0;
        let error_sender = net_node.error_channel().sender.clone_async();

        let new_socket = udp_node.new_socket.sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                match listen(remote_addr, has_broadcast, opt_v4, opt_v6, listener_socket).await {
                    Ok(socket) => {
                        new_socket
                            .send(socket)
                            .await
                            .expect("Socket channel has closed.");
                    }

                    Err(e) => {
                        error_sender
                            .send(NetworkError::Listen(e))
                            .await
                            .expect("Error channel has closed.");
                    }
                }
            })
            .detach();
        commands.entity(entity).insert(net_node);
    }
}

async fn listen(
    remote_addr: Option<RemoteSocket>,
    has_broadcast: bool,
    opt_v4: Option<MulticastV4Setting>,
    opt_v6: Option<MulticastV6Setting>,
    listener_socket: SocketAddr,
) -> Result<UdpSocket, std::io::Error> {
    debug!("Listening on {:?}", listener_socket);
    let socket = UdpSocket::bind(listener_socket).await?;

    if has_broadcast {
        socket.set_broadcast(true)?;
    }

    if let Some(remote_addr) = remote_addr {
        socket.connect(remote_addr.0).await?;
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

    Ok(socket)
}

fn control_udp_node(
    mut commands: Commands,
    mut q_udp_node: Query<
        (
            Entity,
            &mut UdpNode,
            &mut NetworkNode,
            Option<&mut LocalSocket>,
        ),
        Added<NetworkNode>,
    >,
    mut network_event: EventWriter<NetworkEvent>,
) {
    for (entity, mut udp_node, mut net_node, opt_local_socket) in q_udp_node.iter_mut() {
        while let Ok(Some(socket)) = udp_node.new_socket.receiver.try_recv() {
            if opt_local_socket.is_none() {
                commands
                    .entity(entity)
                    .insert(LocalSocket(socket.local_addr().unwrap()));
            }
            if let Ok(peer) = socket.peer_addr() {
                net_node.peer_addr = Some(peer);

                debug!(
                    "Starting udp {:?} peer {:?} ",
                    socket.local_addr().unwrap(),
                    peer
                );
                network_event.send(NetworkEvent::Connected(entity));
            } else {
                debug!(
                    "Starting udp {:?} with no peer",
                    socket.local_addr().unwrap(),
                );

                network_event.send(NetworkEvent::Listen(entity));
            }

            udp_node.start(socket, &mut net_node);
        }
    }
}
