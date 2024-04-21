use std::fmt::Display;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::spawn;

use async_channel::{Receiver, Sender};
use async_net::{AsyncToSocketAddrs, UdpSocket};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool, TaskPool, TaskPoolBuilder};
use bytes::Bytes;
use dashmap::DashMap;
use futures_lite::future::block_on;

use crate::{AsyncChannel, ChannelName, Connection, ConnectionId, NetworkRawPacket};
use crate::component::{ConnectTo, NetworkSetting};
use crate::error::NetworkError;
use crate::runtime::{JoinHandle, run_async, Runtime};

pub struct UdpPlugin;

impl Plugin for UdpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, manage_udp_server);
    }
}

/// The setting for a UDP server
#[derive(Component, Clone)]
pub struct UdpServerSetting {
    /// The name of the server
    pub name: ChannelName,
    /// The address to bind to. This can be a single address or multiple addresses
    pub address: Vec<SocketAddr>,
    /// The maximum packet size to accept
    /// Default is 65,507 bytes
    pub max_packet_size: usize,
    /// Whether to start the server automatically
    pub auto_start: bool,
}

impl UdpServerSetting {
    pub fn new(channel_name: impl ToString, addrs: impl ToSocketAddrs) -> Self {
        Self {
            name: channel_name.to_string(),
            address: addrs.to_socket_addrs().unwrap().collect(),
            max_packet_size: 65_507,
            auto_start: true,
        }
    }
}

#[derive(Component)]
pub struct UdpProtocol;

#[derive(Component)]
pub struct UdpNode {
    socket: UdpSocket,
    message_channel: AsyncChannel<NetworkRawPacket>,
    error_channel: AsyncChannel<NetworkError>,
    cancel_flag: Arc<AtomicBool>,
    running: bool,
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

impl UdpNode {
    pub fn new(addrs: impl ToSocketAddrs) -> Self {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let message_channel = AsyncChannel::<NetworkRawPacket>::new();
        let error_channel = AsyncChannel::<NetworkError>::new();

        let addrs = addrs.to_socket_addrs().unwrap().collect::<Vec<_>>();

        let socket = block_on(
            ComputeTaskPool::get()
                .spawn(async move { UdpSocket::bind(&*addrs).await.expect("Failed to bind") }),
        );

        Self {
            message_channel,
            error_channel,
            cancel_flag,
            running: false,
            socket,
        }
    }

    fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr().unwrap()
    }

    async fn recv_loop(
        socket: UdpSocket,
        message_sender: Sender<NetworkRawPacket>,
        error_sender: Sender<NetworkError>,
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
                    println!("Received {} bytes from {}", len, from_addr);
                    message_sender
                        .send(NetworkRawPacket { from_addr, bytes })
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
        message_receiver: Receiver<NetworkRawPacket>,
        error_sender: Sender<NetworkError>,
        cancel_flag: Arc<AtomicBool>,
    ) {
        loop {
            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            while let Ok(packet) = message_receiver.recv().await {
                trace!(
                    "Sending {} bytes from {} to {}",
                    packet.bytes.len(),
                    packet.from_addr,
                    socket.peer_addr().unwrap()
                );
                let buf = packet.bytes.as_ref();
                if let Err(e) = socket.send(&buf).await {
                    error_sender
                        .send(NetworkError::SendError)
                        .await
                        .expect("Error channel has closed.");
                }
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn start(&mut self) {
        debug!("Starting {}", self);
        self.cancel_flag
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let socket = self.socket.clone();
        let cancel_flag = self.cancel_flag.clone();
        let message_sender = self.message_channel.sender.clone();
        let error_sender = self.error_channel.sender.clone();

        IoTaskPool::get()
            .spawn(async move {
                Self::recv_loop(socket, message_sender, error_sender, cancel_flag, 65_507).await;
            })
            .detach();

        self.running = true;
    }

    pub fn stop(&mut self) {
        debug!("Stopping {}", self);
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let self_addr = self.local_addr();

        self.running = false;
        // this is a hack to send a message to the server to shut down
        IoTaskPool::get()
            .spawn(async move {
                let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
                socket.send_to(b"shutdown", &self_addr).await.unwrap();
            })
            .detach();
    }

    pub fn connect_to(&mut self, connect_to: &ConnectTo) {
        let socket = self.socket.clone();
        let connect_to = connect_to.addrs.clone();
        let cancel_flag = self.cancel_flag.clone();
        let message_receiver = self.message_channel.receiver.clone();
        let error_sender = self.error_channel.sender.clone();

        IoTaskPool::get()
            .spawn(async move {
                match socket.connect(&*connect_to).await {
                    Ok(_) => {
                        debug!("Connected to {}", socket.peer_addr().unwrap());
                        Self::send_loop(socket, message_receiver, error_sender, cancel_flag).await;
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
        self.message_channel
            .sender
            .try_send(NetworkRawPacket {
                from_addr: self.local_addr(),
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
    }
}

fn manage_udp_server(
    mut commands: Commands,
    // mut q_servers: Query<(Entity, &mut UdpServerNode), Added<UdpServerNode>>,
    // mut q_settings: Query<&mut UdpServerSetting, (Added<UdpServerSetting>, Without<BindSocket>)>,
    mut q_udp_node: Query<
        (
            Entity,
            &mut UdpNode,
            Option<&NetworkSetting>,
            Option<&ConnectTo>,
        ),
        (Added<UdpNode>),
    >,
) {
    for (entity, mut udp_node, opt_setting, opt_connect_to) in q_udp_node.iter_mut() {
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
