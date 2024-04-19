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

use crate::{AsyncChannel, Connection, ConnectionId, NetworkPacket};

use crate::error::NetworkError;

use crate::runtime::{JoinHandle, run_async, Runtime};

pub struct UdpPlugin;

impl Plugin for UdpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, manage_udp_server);
    }
}

#[derive(Component, Clone)]
pub struct UdpServerSetting {
    /// The name of the server
    pub name: String,
    /// The address to bind to. This can be a single address or multiple addresses
    pub address: Vec<SocketAddr>,
    /// The maximum packet size to accept
    /// Default is 65,507 bytes
    pub max_packet_size: usize,
}

impl UdpServerSetting {
    pub fn new(channel_name: impl ToString, addrs: impl ToSocketAddrs) -> Self {
        Self {
            name: channel_name.to_string(),
            address: addrs.to_socket_addrs().unwrap().collect(),
            max_packet_size: 65_507,
        }
    }
}

#[derive(Component)]
pub struct UdpServerNode {
    recv_message_map: Arc<DashMap<&'static str, Vec<(ConnectionId, Vec<u8>)>>>,
    established_connections: Arc<DashMap<ConnectionId, Connection>>,
    error_channel: AsyncChannel<NetworkError>,
    cancel_flag: Arc<AtomicBool>,
    server_handle: Option<bevy::tasks::Task<()>>,
    running: bool,
}

impl UdpServerNode {
    pub fn build_from_setting(setting: UdpServerSetting) -> Self {
        debug!("Starting UDP server on {:?}", setting.address);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_clone = cancel_flag.clone();
        // let message_channel = AsyncChannel::<Bytes>::new();
        // let message_sender = message_channel.sender.clone();
        let error_channel = AsyncChannel::<NetworkError>::new();
        let error_sender = error_channel.sender.clone();

        IoTaskPool::get()
            .spawn(async move {
                Self::recv_loop(setting, cancel_flag_clone, error_sender).await;
            })
            .detach();

        Self {
            recv_message_map: Arc::new(DashMap::new()),
            established_connections: Arc::new(Default::default()),
            error_channel,
            server_handle: None,
            running: true,
            cancel_flag
        }
    }

    async fn recv_loop(
        setting: UdpServerSetting,
        cancel_flag: Arc<AtomicBool>,
        error_sender: Sender<NetworkError>,
    ) {
        match async_net::UdpSocket::bind(&*setting.address).await {
            Ok(socket) => {
                let mut buf: Vec<u8> = vec![0; setting.max_packet_size];

                loop {
                    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        println!("Shutting down UDP server");
                        break;
                    }

                    if let Ok((len, addr)) = socket.recv_from(&mut buf).await {
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
                        println!("Received {} bytes from {}", len, addr);
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

    pub fn shutdown(&mut self) {
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);


        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}

fn manage_udp_server(
    q_server_setting: Query<(Entity, &UdpServerSetting), Added<UdpServerSetting>>,
    mut commands: Commands,
) {
    for (entity, setting) in q_server_setting.iter() {
        commands
            .entity(entity)
            .insert(UdpServerNode::build_from_setting(setting.clone()));
    }
}
