use std::{
    fmt::Display,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    sync::{atomic::AtomicBool, Arc},
};

use async_net::{AsyncToSocketAddrs, UdpSocket};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool, TaskPool, TaskPoolBuilder},
};
use bytes::Bytes;
use futures_lite::future::block_on;
use kanal::{AsyncReceiver, AsyncSender};

use crate::{
    component::{ConnectTo, NetworkNode},
    error::NetworkError,
    Connection, ConnectionId, NetworkRawPacket,
};

pub struct UdpPlugin;

impl Plugin for UdpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (create_network_node, control_udp_node));
    }
}

#[derive(Component)]
pub struct UdpProtocol;

#[derive(Component)]
pub struct UdpNode {
    /// The UDP socket
    socket: UdpSocket,
    /// Whether the node is broadcasting or not
    broadcast: bool,
}

impl Default for UdpNode {
    fn default() -> Self {
        Self::new("0.0.0.0:0")
    }
}

impl Display for UdpNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("UDP Node on {:?}", self.local_addr()))
    }
}

#[derive(Default)]
pub struct UdpNodeBuilder {
    addrs: Vec<SocketAddr>,
    max_packet_size: usize,
    auto_start: bool,
    broadcast: bool,
}

impl UdpNodeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_addrs(mut self, addrs: impl ToSocketAddrs) -> Self {
        self.addrs = addrs.to_socket_addrs().unwrap().collect();
        self
    }

    pub fn with_max_packet_size(mut self, max_packet_size: usize) -> Self {
        self.max_packet_size = max_packet_size;
        self
    }

    pub fn with_auto_start(mut self, auto_start: bool) -> Self {
        self.auto_start = auto_start;
        self
    }

    pub fn with_broadcast(mut self, broadcast: bool) -> Self {
        self.broadcast = broadcast;
        self
    }

    pub fn build(self) -> UdpNode {
        let addrs = self.addrs;

        let socket = block_on(
            ComputeTaskPool::get()
                .spawn(async move { UdpSocket::bind(&*addrs).await.expect("Failed to bind") }),
        );

        UdpNode {
            socket,
            broadcast: self.broadcast,
        }
    }
}

impl UdpNode {
    pub fn new(addrs: impl ToSocketAddrs) -> Self {
        let addrs = addrs.to_socket_addrs().unwrap().collect::<Vec<_>>();

        let socket = block_on(
            ComputeTaskPool::get()
                .spawn(async move { UdpSocket::bind(&*addrs).await.expect("Failed to bind") }),
        );

        Self {
            socket,
            broadcast: false,
        }
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.socket.local_addr().ok()
    }

    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.socket.peer_addr().ok()
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
                            socket: Some(from_addr),
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

                if packet.socket.is_none() {
                    error_sender
                        .send(NetworkError::SendError)
                        .await
                        .expect("Error channel has closed.");
                    continue;
                }

                if let Err(_e) = socket
                    .send_to(
                        packet.bytes.as_ref(),
                        packet.socket.expect("send packet must have dest socket"),
                    )
                    .await
                {
                    error_sender
                        .send(NetworkError::SendError)
                        .await
                        .expect("Error channel has closed.");
                }
            }
        }
    }

    /// Start the UDP node
    pub fn start(&mut self, network_node: &mut NetworkNode) {
        debug!("Starting {}", self);
        self.socket
            .set_broadcast(self.broadcast)
            .expect("Failed to set broadcast");

        let socket = self.socket.clone();
        let cancel_flag = network_node.cancel_flag.clone();
        let recv_sender = network_node.recv_channel().sender.clone_async();
        let error_sender = network_node.error_channel().sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                Self::recv_loop(
                    socket.clone(),
                    recv_sender,
                    error_sender.clone(),
                    cancel_flag.clone(),
                    65_507,
                )
                .await;
            })
            .detach();

        let socket = self.socket.clone();
        let cancel_flag = network_node.cancel_flag.clone();
        let message_receiver = network_node.send_channel().receiver.clone_async();
        let error_sender = network_node.error_channel().sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                Self::send_loop(socket, message_receiver, error_sender, cancel_flag).await;
            })
            .detach();

        network_node.start();
    }

    pub fn stop(&mut self, network_node: &mut NetworkNode) {
        debug!("Stopping {}", self);

        let self_addr = self.local_addr();

        // this is a hack to send a message to the server to shut down
        network_node
            .send_channel()
            .sender
            .try_send(NetworkRawPacket {
                socket: self_addr,
                bytes: Bytes::from_static(b"shutdown"),
            })
            .expect("Message channel has closed.");

        network_node.stop();
    }

    pub fn connect_to(&mut self, network_node: &mut NetworkNode, connect_to: &ConnectTo) {
        let socket = self.socket.clone();
        let connect_to = connect_to.addrs.clone();
        let error_sender = network_node.error_channel().sender.clone_async();

        block_on(ComputeTaskPool::get().spawn(async move {
            match socket.connect(&*connect_to).await {
                Ok(_) => {
                    debug!(
                        "{} Connected to {}",
                        socket.local_addr().unwrap(),
                        socket.peer_addr().unwrap()
                    );
                }
                Err(e) => {
                    error_sender
                        .send(NetworkError::Connection(e))
                        .await
                        .expect("Error channel has closed.");
                }
            }
        }));

        network_node.peer_addr = self.socket.peer_addr().ok();
    }

    pub fn join_multicast_v4(
        &self,
        network_node: &NetworkNode,
        multi_addr: Ipv4Addr,
        interface: Ipv4Addr,
    ) {
        let socket = self.socket.clone();
        let error_sender = network_node.error_channel().sender.clone();
        match socket.join_multicast_v4(multi_addr, interface) {
            Ok(_) => {
                debug!(
                    "{} Joined multicast group {} on interface {}",
                    socket.local_addr().unwrap(),
                    multi_addr,
                    interface
                );
            }
            Err(e) => {
                error_sender
                    .send(NetworkError::Error(format!(
                        "Failed to join multicast group: {}",
                        e
                    )))
                    .expect("Error channel has closed.");
            }
        }
    }

    pub fn leave_multicast_v4(
        &self,
        network_node: &NetworkNode,
        multi_addr: Ipv4Addr,
        interface: Ipv4Addr,
    ) {
        let socket = self.socket.clone();
        let error_sender = network_node.error_channel().sender.clone();
        match socket.leave_multicast_v4(multi_addr, interface) {
            Ok(_) => {
                debug!(
                    "{} Left multicast group {} on interface {}",
                    socket.local_addr().unwrap(),
                    multi_addr,
                    interface
                );
            }
            Err(e) => {
                error_sender
                    .send(NetworkError::Error(format!(
                        "Failed to leave multicast group: {}",
                        e
                    )))
                    .expect("Error channel has closed.");
            }
        }
    }

    pub fn join_multicast_v6(
        &self,
        network_node: &NetworkNode,
        multi_addr: Ipv6Addr,
        interface: u32,
    ) {
        let socket = self.socket.clone();
        let error_sender = network_node.error_channel().sender.clone();
        match socket.join_multicast_v6(&multi_addr, interface) {
            Ok(_) => {
                debug!(
                    "{} Joined multicast group {} on interface {}",
                    socket.local_addr().unwrap(),
                    multi_addr,
                    interface
                );
            }
            Err(e) => {
                error_sender
                    .send(NetworkError::Error(format!(
                        "Failed to join multicast group: {}",
                        e
                    )))
                    .expect("Error channel has closed.");
            }
        }
    }

    pub fn leave_multicast_v6(
        &self,
        network_node: &NetworkNode,
        multi_addr: Ipv6Addr,
        interface: u32,
    ) {
        let socket = self.socket.clone();
        let error_sender = network_node.error_channel().sender.clone();
        match socket.leave_multicast_v6(&multi_addr, interface) {
            Ok(_) => {
                debug!(
                    "{} Left multicast group {} on interface {}",
                    socket.local_addr().unwrap(),
                    multi_addr,
                    interface
                );
            }
            Err(e) => {
                error_sender
                    .send(NetworkError::Error(format!(
                        "Failed to leave multicast group: {}",
                        e
                    )))
                    .expect("Error channel has closed.");
            }
        }
    }
}

#[derive(Component)]
pub struct MulticastV4Setting {
    pub multi_addr: Ipv4Addr,
    pub interface: Ipv4Addr,
}

#[derive(Component)]
pub struct MulticastV6Setting {
    pub multi_addr: Ipv6Addr,
    pub interface: u32,
}

fn create_network_node(mut commands: Commands, q_udp: Query<(Entity, &UdpNode), Added<UdpNode>>) {
    for (entity, udp_node) in q_udp.iter() {
        commands.entity(entity).insert(NetworkNode::new(
            udp_node.local_addr(),
            udp_node.peer_addr(),
        ));
    }
}

fn control_udp_node(
    mut q_udp_node: Query<
        (
            &mut UdpNode,
            &mut NetworkNode,
            Option<&ConnectTo>,
            Option<&MulticastV4Setting>,
            Option<&MulticastV6Setting>,
        ),
        Added<NetworkNode>,
    >,
) {
    for (mut udp_node, mut network_node, opt_connect_to, opt_multi_v4, opt_multi_v6) in
        q_udp_node.iter_mut()
    {
        if network_node.auto_start {
            udp_node.start(&mut network_node);

            if let Some(connect_to) = opt_connect_to {
                udp_node.connect_to(&mut network_node, connect_to);
            }

            if let Some(multi_v4) = opt_multi_v4 {
                udp_node.join_multicast_v4(&network_node, multi_v4.multi_addr, multi_v4.interface);
            }

            if let Some(multi_v6) = opt_multi_v6 {
                udp_node.join_multicast_v6(&network_node, multi_v6.multi_addr, multi_v6.interface);
            }
        }
    }
}
