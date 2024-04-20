use std::fmt::Display;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use std::sync::atomic::AtomicBool;
use std::thread::spawn;

use async_channel::{Receiver, Sender};
use async_net::UdpSocket;
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, TaskPool, TaskPoolBuilder};
use bytes::Bytes;
use dashmap::DashMap;

use crate::{AsyncChannel, ChannelName, Connection, ConnectionId, NetworkRawPacket};

use crate::error::NetworkError;

use crate::runtime::{run_async, JoinHandle, Runtime};

pub struct UdpPlugin;

impl Plugin for UdpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, manage_udp_server);
    }
}

/// The setting for a UDP server
#[derive(Clone)]
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

/// A UDP server node
#[derive(Component)]
pub struct UdpServerNode {
    setting: UdpServerSetting,
    message_channel: AsyncChannel<NetworkRawPacket>,
    error_channel: AsyncChannel<NetworkError>,
    cancel_flag: Arc<AtomicBool>,
    running: bool,
}

impl Display for UdpServerNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "UDP Server '{}' on {:?}",
            self.setting.name, self.setting.address
        ))
    }
}

impl UdpServerNode {
    pub fn new(channel_name: impl ToString, addrs: impl ToSocketAddrs) -> Self {
        let setting = UdpServerSetting::new(channel_name, addrs);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let message_channel = AsyncChannel::<NetworkRawPacket>::new();
        let error_channel = AsyncChannel::<NetworkError>::new();

        Self {
            message_channel,
            error_channel,
            setting,
            running: false,
            cancel_flag,
        }
    }

    pub fn new_with_setting(setting: UdpServerSetting) -> Self {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let message_channel = AsyncChannel::<NetworkRawPacket>::new();
        let error_channel = AsyncChannel::<NetworkError>::new();
        Self {
            message_channel,
            error_channel,
            setting,
            running: false,
            cancel_flag,
        }
    }

    async fn recv_loop(
        setting: UdpServerSetting,
        cancel_flag: Arc<AtomicBool>,
        message_sender: Sender<NetworkRawPacket>,
        error_sender: Sender<NetworkError>,
    ) {
        match UdpSocket::bind(&*setting.address).await {
            Ok(socket) => {
                let mut buf: Vec<u8> = vec![0; setting.max_packet_size];

                loop {
                    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        debug!("Stop UDP Server '{}' on {:?}", setting.name, setting.address);
                        break;
                    }

                    if let Ok((len, from_addr)) = socket.recv_from(&mut buf).await {
                        // Checks to see if the addr is in the connected_clients dashmap, and if
                        // it isn't, it adds it
                        // let mut conn_id = None;
                        // let not_connected =
                        //     !udp_connected_clients_clone.iter().any(|key_val| {
                        //         let local_conn_id = key_val.key();
                        //
                        //         if local_conn_id.addr == recv_addr {
                        //             conn_id = Some(*local_conn_id);
                        //             true
                        //         } else {
                        //             false
                        //         }
                        //     });

                        let bytes = Bytes::copy_from_slice(&buf[..len]);
                        println!("Received {} bytes from {}", len, from_addr);
                        message_sender
                            .send(NetworkRawPacket { from_addr, bytes })
                            .await
                            .expect("Message channel has closed.");
                        // message_sender.send(bytes).await.expect("Message channel has closed.")
                    }
                }
            }
            Err(e) => {
                error_sender
                    .send(NetworkError::Listen(e))
                    .await
                    .expect("Error channel has closed.");
            }
        }
    }

    pub fn start(&mut self) {
        debug!("Start {}", self);
        self.cancel_flag
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let setting_clone = self.setting.clone();
        let cancel_flag_clone = self.cancel_flag.clone();
        let message_sender = self.message_channel.sender.clone();
        let error_sender = self.error_channel.sender.clone();
        IoTaskPool::get()
            .spawn(async move {
                Self::recv_loop(
                    setting_clone,
                    cancel_flag_clone,
                    message_sender,
                    error_sender,
                )
                .await;
            })
            .detach();

        self.running = true;
    }

    /// Shuts down the server
    pub fn stop(&mut self) {
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let self_addr = self.setting.address.clone();

        // this is a hack to send a message to the server to shut down
        IoTaskPool::get()
            .spawn(async move {
                let socket = UdpSocket::bind("0.0.0.0:0").await.unwrap();
                socket.send_to(b"shutdown", &*self_addr).await.unwrap();
            })
            .detach();

        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}

fn manage_udp_server(
    mut q_servers: Query<(Entity, &mut UdpServerNode), Added<UdpServerNode>>,
) {
    for (_entity, mut server) in q_servers.iter_mut() {
        if server.setting.auto_start {
            server.start();
        }
    }
}
