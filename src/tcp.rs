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
            (
                spawn_tcp_client,
                spawn_tcp_server,
                handle_new_connection,
                broadcast_message,
            ),
        );
    }
}

#[derive(Component)]
pub struct TCPProtocol;

#[derive(Component, Deref, DerefMut)]
pub struct TcpStreamNode(pub TcpStream);

impl TcpStreamNode {
    pub fn new(stream: TcpStream, net_node: &NetworkNode) -> Self {
        let sender_rx = net_node.send_channel().receiver.clone_async();
        let error_tx = net_node.error_channel().sender.clone_async();
        let cancel_flag = net_node.cancel_flag.clone();
        let send_stream = stream.clone();
        IoTaskPool::get()
            .spawn(async move {
                send_loop(
                    send_stream,
                    sender_rx.clone(),
                    error_tx.clone(),
                    cancel_flag.clone(),
                )
                .await;
            })
            .detach();

        let receiver = net_node.recv_channel().sender.clone_async();
        let error_tx = net_node.error_channel().sender.clone_async();
        let cancel_flag = net_node.cancel_flag.clone();
        let recv_stream = stream.clone();
        let max_packet_size = net_node.max_packet_size;
        IoTaskPool::get()
            .spawn(async move {
                recv_loop(
                    recv_stream,
                    receiver,
                    error_tx,
                    cancel_flag,
                    max_packet_size,
                )
                .await;
            })
            .detach();
        Self(stream)
    }
}

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
}

async fn send_loop(
    mut client: TcpStream,
    message_receiver: AsyncReceiver<NetworkRawPacket>,
    error_sender: AsyncSender<NetworkError>,
    cancel_flag: Arc<AtomicBool>,
) {
    loop {
        println!("send loop");
        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        println!("send loop 2");

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
async fn recv_loop(
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
                        send_loop(
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

/// TCPProtocol with local socket meas TCP server
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
    q_tcp_server: Query<(Entity, &TcpServerNode, &NetworkNode)>,
    mut node_events: EventWriter<NetworkEvent>,
) {
    for (entity, tcp_server, _net_node) in q_tcp_server.iter() {
        while let Ok(Some(tcp_stream)) = tcp_server.new_connections.receiver.try_recv() {
            debug!(
                "new Tcp client {:?} connected",
                tcp_stream.peer_addr().unwrap()
            );

            let new_net_node = NetworkNode::new(
                NetworkProtocol::TCP,
                None,
                Some(tcp_stream.peer_addr().unwrap()),
            );
            // Create a new entity for the client
            let tcp_client = commands
                .spawn((
                    TcpStreamNode::new(tcp_stream.clone(), &new_net_node),
                    new_net_node,
                    RemoteSocket(tcp_stream.peer_addr().unwrap()),

                ))
                .id();

            debug!("Tcp client entity: {:?} => {:?}", entity, tcp_client);
            // Add the client to the server's children
            commands.entity(entity).add_child(tcp_client);

            node_events.send(NetworkEvent::Connected(tcp_client));
        }
    }
}

fn broadcast_message(
    q_server: Query<(&NetworkNode, &Children)>,
    q_child: Query<(&NetworkNode, &RemoteSocket)>,
) {
    for (net_node, children) in q_server.iter() {
        while let Ok(Some(message)) = net_node.broadcast_channel().receiver.try_recv() {
            debug!("Broadcasting message: {:?} to {}", message, children.len());
            for &child in children.iter() {
                debug!("Broadcasting to child: {:?}", child);
                let (child_net_node, child_remote_addr) = q_child.get(child).unwrap();
                child_net_node
                    .send_channel()
                    .sender
                    .try_send(NetworkRawPacket {
                        socket: **child_remote_addr,
                        bytes: message.bytes.clone(),
                    })
                    .expect("Message channel has closed.");
            }
        }
    }
}
