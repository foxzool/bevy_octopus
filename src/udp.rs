use std::fmt::Display;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::spawn;

use async_net::{AsyncToSocketAddrs, UdpSocket};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool, TaskPool, TaskPoolBuilder};
use bytes::Bytes;
use dashmap::DashMap;
use futures_lite::future::block_on;
use kanal::{AsyncReceiver, AsyncSender, Receiver, Sender};

use crate::{AsyncChannel, ChannelName, Connection, ConnectionId, NetworkRawPacket};
use crate::component::{ConnectTo, NetworkSetting};
use crate::error::NetworkError;
use crate::runtime::{JoinHandle, run_async};

pub struct UdpPlugin;

impl Plugin for UdpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, control_udp_node);
    }
}

#[derive(Component)]
pub struct UdpProtocol;

#[derive(Component)]
pub struct UdpNode {
    /// The UDP socket
    socket: UdpSocket,
    /// Channel for receiving messages
    recv_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for sending messages
    send_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for errors
    error_channel: AsyncChannel<NetworkError>,
    /// A flag to cancel the node
    cancel_flag: Arc<AtomicBool>,
    /// Whether the node is running or not
    running: bool,
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
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let send_channel = AsyncChannel::<NetworkRawPacket>::new();
        let recv_channel = AsyncChannel::<NetworkRawPacket>::new();
        let error_channel = AsyncChannel::<NetworkError>::new();

        let addrs = self.addrs;

        let socket = block_on(
            ComputeTaskPool::get()
                .spawn(async move { UdpSocket::bind(&*addrs).await.expect("Failed to bind") }),
        );

        UdpNode {
            recv_message_channel: recv_channel,
            send_message_channel: send_channel,
            error_channel,
            cancel_flag,
            running: false,
            socket,
            broadcast: self.broadcast,
        }
    }
}

impl UdpNode {
    pub fn new(addrs: impl ToSocketAddrs) -> Self {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let send_channel = AsyncChannel::<NetworkRawPacket>::new();
        let recv_channel = AsyncChannel::<NetworkRawPacket>::new();
        let error_channel = AsyncChannel::<NetworkError>::new();

        let addrs = addrs.to_socket_addrs().unwrap().collect::<Vec<_>>();

        let socket = block_on(
            ComputeTaskPool::get()
                .spawn(async move { UdpSocket::bind(&*addrs).await.expect("Failed to bind") }),
        );

        Self {
            recv_message_channel: recv_channel,
            send_message_channel: send_channel,
            error_channel,
            cancel_flag,
            running: false,
            socket,
            broadcast: false,
        }
    }

    fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr().unwrap()
    }

    fn peer_addr(&self) -> SocketAddr {
        self.socket.peer_addr().unwrap()
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
                    trace!("Received {} bytes from {}", len, from_addr);
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
                trace!("Sending {} bytes to {}", packet.bytes.len(), packet.socket,);
                let buf = packet.bytes.as_ref();
                if let Err(_e) = socket.send_to(&buf, packet.socket).await {
                    error_sender
                        .send(NetworkError::SendError)
                        .await
                        .expect("Error channel has closed.");
                }
            }
        }
    }

    /// Check if the UDP node is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Start the UDP node
    pub fn start(&mut self) {
        debug!("Starting {}", self);
        self.socket
            .set_broadcast(self.broadcast)
            .expect("Failed to set broadcast");
        self.cancel_flag
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let socket = self.socket.clone();
        let cancel_flag = self.cancel_flag.clone();
        let message_sender = self.recv_message_channel.sender.clone_async();
        let error_sender = self.error_channel.sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                Self::recv_loop(
                    socket.clone(),
                    message_sender,
                    error_sender.clone(),
                    cancel_flag.clone(),
                    65_507,
                )
                    .await;
            })
            .detach();

        let socket = self.socket.clone();
        let cancel_flag = self.cancel_flag.clone();
        let message_receiver = self.send_message_channel.receiver.clone_async();
        let error_sender = self.error_channel.sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                Self::send_loop(socket, message_receiver, error_sender, cancel_flag).await;
            })
            .detach();

        self.running = true;
    }

    pub fn stop(&mut self) {
        debug!("Stopping {}", self);
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let self_addr = self.local_addr();

        // this is a hack to send a message to the server to shut down
        self.send_message_channel
            .sender
            .try_send(NetworkRawPacket {
                socket: self_addr,
                bytes: Bytes::from_static(b"shutdown"),
            })
            .expect("Message channel has closed.");
        self.running = false;
    }

    pub fn connect_to(&mut self, connect_to: &ConnectTo) {
        let socket = self.socket.clone();
        let connect_to = connect_to.addrs.clone();
        let error_sender = self.error_channel.sender.clone_async();

        IoTaskPool::get()
            .spawn(async move {
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
            })
            .detach();
    }

    pub fn send(&self, bytes: &[u8]) {
        self.send_message_channel
            .sender
            .try_send(NetworkRawPacket {
                socket: self.peer_addr(),
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
    }

    pub fn send_to(&self, bytes: &[u8], addr: impl ToSocketAddrs) {
        let peer_addr = addr.to_socket_addrs().unwrap().next().unwrap();
        self.send_message_channel
            .sender
            .try_send(NetworkRawPacket {
                socket: peer_addr,
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
    }

    pub fn receiver(&self) -> Receiver<NetworkRawPacket> {
        self.recv_message_channel.receiver.clone()
    }

    pub fn sender(&self) -> Sender<NetworkRawPacket> {
        self.send_message_channel.sender.clone()
    }
}

fn control_udp_node(
    mut q_udp_node: Query<
        (&mut UdpNode, Option<&NetworkSetting>, Option<&ConnectTo>),
        Added<UdpNode>,
    >,
) {
    for (mut udp_node, opt_setting, opt_connect_to) in q_udp_node.iter_mut() {
        let setting = match opt_setting {
            Some(setting) => setting.clone(),
            None => NetworkSetting::default(),
        };

        if setting.auto_start {
            udp_node.start();

            if let Some(connect_to) = opt_connect_to {
                udp_node.connect_to(connect_to);
            }
        }
    }
}
