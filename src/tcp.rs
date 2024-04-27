use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use async_net::{TcpListener, TcpStream};
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use bytes::Bytes;
use futures_lite::{AsyncReadExt, AsyncWriteExt, StreamExt};
use kanal::{AsyncReceiver, AsyncSender};

use crate::error::NetworkError;
use crate::network::{LocalSocket, NetworkEvent, NetworkProtocol, NetworkRawPacket};
use crate::network_manager::NetworkNode;
use crate::prelude::RemoteSocket;
use crate::{AsyncChannel, ConnectionId};

pub struct TcpPlugin;

impl Plugin for TcpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (spawn_tcp_client, spawn_tcp_server, handle_new_connection),
        );
    }
}

#[derive(Component)]
pub struct TCPProtocol;

#[derive(Component)]
pub struct TcpServerNode {
    new_connections: AsyncChannel<TcpStream>,
}

impl TcpServerNode {
    pub fn new() -> Self {
        Self {
            new_connections: AsyncChannel::new(),
        }
    }
    pub fn start(
        &self,
        addr: SocketAddr,
        error_sender: AsyncSender<NetworkError>,
        cancel_flag: Arc<AtomicBool>,
    ) {
        let new_connection_sender = self.new_connections.sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                match TcpListener::bind(addr).await {
                    Ok(listener) => {
                        debug!(
                            "Starting TCP server on {:?}",
                            listener.local_addr().unwrap()
                        );
                        let mut incoming = listener.incoming();
                        loop {
                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }

                            while let Some(Ok(income)) = incoming.next().await {
                                new_connection_sender.send(income).await.unwrap();
                            }
                        }
                    }
                    Err(e) => {
                        error_sender
                            .send(NetworkError::Listen(e))
                            .await
                            .expect("Error channel has been closed");
                    }
                }
            })
            .detach();
    }

    pub async fn recv_loop(
        mut stream: TcpStream,
        message_sender: AsyncSender<NetworkRawPacket>,
        error_sender: AsyncSender<NetworkError>,
        cancel_flag: Arc<AtomicBool>,
        max_packet_size: usize,
    ) {
        let mut buffer = vec![0; max_packet_size];

        loop {
            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            match stream.read(&mut buffer).await {
                Ok(0) => {
                    error_sender
                        .send(NetworkError::ChannelClosed(ConnectionId { id: 0 }))
                        .await
                        .expect("Error channel has closed");

                    break;
                }
                Ok(n) => {
                    debug!(
                        "{} Received {} bytes from {}",
                        "?",
                        n,
                        stream.local_addr().unwrap(),
                    );
                    let bytes = Bytes::copy_from_slice(&buffer[..n]);
                    message_sender
                        .send(NetworkRawPacket {
                            socket: stream.local_addr().unwrap(),
                            bytes,
                        })
                        .await
                        .expect("Message channel has closed.");
                }
                Err(e) => {
                    error_sender
                        .send(NetworkError::Error(e.to_string()))
                        .await
                        .expect("Error channel has closed");
                    break;
                }
            }
        }
    }
}

#[derive(Component)]
pub struct TcpClientNode {
    socket: SocketAddr,
}

impl TcpClientNode {
    pub fn new(addrs: impl ToSocketAddrs) -> Self {
        Self {
            socket: addrs.to_socket_addrs().unwrap().next().unwrap(),
        }
    }

    pub fn start(&self, net: &NetworkNode) {
        let socket = self.socket.clone();
        let cancel_flag = net.cancel_flag.clone();
        let message_receiver = net.send_channel().receiver.clone_async();
        let error_sender = net.error_channel().sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                match TcpStream::connect(&socket).await {
                    Ok(stream) => {
                        Self::send_loop(
                            stream,
                            message_receiver,
                            error_sender.clone(),
                            cancel_flag.clone(),
                        )
                        .await;
                    }
                    Err(e) => error_sender
                        .send(NetworkError::Connection(e))
                        .await
                        .expect("Error channel has closed"),
                }
            })
            .detach()
    }

    async fn send_loop(
        mut client: TcpStream,
        message_receiver: AsyncReceiver<NetworkRawPacket>,
        error_sender: AsyncSender<NetworkError>,
        cancel_flag: Arc<AtomicBool>,
    ) {
        loop {
            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            while let Ok(message) = message_receiver.recv().await {
                debug!("send packet {:?}", message);
                if let Err(e) = client.write_all(&message.bytes).await {
                    error!("{:?}", e);
                    error_sender
                        .send(NetworkError::SendError)
                        .await
                        .expect("Error channel has closed")
                }
            }
        }
    }
}

fn spawn_tcp_client(
    mut commands: Commands,

    q_tcp_client: Query<(Entity, &RemoteSocket), (Added<RemoteSocket>, With<TCPProtocol>)>,
) {
    for (e, remote_socket) in q_tcp_client.iter() {
        let net_node = NetworkNode::new(NetworkProtocol::TCP, None, Some(**remote_socket));
        let tcp_client = TcpClientNode::new(**remote_socket);
        tcp_client.start(&net_node);
        commands.entity(e).insert((net_node, tcp_client));
    }
}

fn spawn_tcp_server(
    mut commands: Commands,
    q_tcp_server: Query<(Entity, &LocalSocket), (Added<LocalSocket>, With<TCPProtocol>)>,
) {
    for (e, local_addr) in q_tcp_server.iter() {
        let net_node = NetworkNode::new(NetworkProtocol::TCP, Some(**local_addr), None);
        let tcp_server = TcpServerNode::new();
        tcp_server.start(
            **local_addr,
            net_node.error_channel().sender.clone_async(),
            net_node.cancel_flag.clone(),
        );

        commands.entity(e).insert((net_node, tcp_server));
    }
}

fn handle_new_connection(
    mut commands: Commands,
    mut q_tcp_server: Query<(Entity, &mut TcpServerNode, &mut NetworkNode)>,
    mut node_events: EventWriter<NetworkEvent>,
) {
    for (entity, tcp_server, net_node) in q_tcp_server.iter_mut() {
        while let Ok(Some(tcp_stream)) = tcp_server.new_connections.receiver.try_recv() {
            debug!(
                "new Tcp client {:?} connected",
                tcp_stream.local_addr().unwrap()
            );
            let cancel_flag = net_node.cancel_flag.clone();
            let recv_sender = net_node.recv_channel().sender.clone_async();
            let error_sender = net_node.error_channel().sender.clone_async();
            let tcp_client = commands
                .spawn(NetworkNode::new(
                    NetworkProtocol::TCP,
                    None,
                    tcp_stream.clone().peer_addr().ok(),
                ))
                .id();
            commands.entity(entity).push_children(&[tcp_client]);

            IoTaskPool::get()
                .spawn(async move {
                    TcpServerNode::recv_loop(
                        tcp_stream,
                        recv_sender,
                        error_sender.clone(),
                        cancel_flag.clone(),
                        65_507,
                    )
                    .await;
                })
                .detach();

            node_events.send(NetworkEvent::Connected(tcp_client));
        }
    }
}
