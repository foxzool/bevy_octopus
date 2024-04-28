use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
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
use crate::AsyncChannel;

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
pub struct TcpNode {
    new_connections: AsyncChannel<TcpStream>,
}

impl Default for TcpNode {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpNode {
    pub fn new() -> Self {
        Self {
            new_connections: AsyncChannel::new(),
        }
    }
    pub fn listen(
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

    pub fn connect(
        &self,
        addr: SocketAddr,
        error_sender: AsyncSender<NetworkError>,
        cancel_flag: Arc<AtomicBool>,
    ) {
        let new_connection_sender = self.new_connections.sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                match TcpStream::connect(addr).await {
                    Ok(stream) => {
                        debug!(
                            "Starting TCP Client on {:?} => {:?}",
                            stream.local_addr().unwrap(),
                            stream.peer_addr().unwrap(),
                        );
                        loop {
                            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                                stream.shutdown(std::net::Shutdown::Both).unwrap();
                                break;
                            }
                        }
                        new_connection_sender.send(stream).await.unwrap();
                    }
                    Err(e) => {
                        error_sender
                            .send(NetworkError::Connection(e))
                            .await
                            .expect("Error channel has been closed");
                    }
                }
            })
            .detach();
    }

    pub fn stream(entity: Entity, stream: TcpStream, net_node: &NetworkNode) {
        let sender_rx = net_node.send_channel().receiver.clone_async();
        let error_tx = net_node.error_channel().sender.clone_async();
        let cancel_flag = net_node.cancel_flag.clone();
        let send_stream = stream.clone();
        IoTaskPool::get()
            .spawn(async move {
                send_loop(
                    entity,
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
                    entity,
                    recv_stream,
                    receiver,
                    error_tx,
                    cancel_flag,
                    max_packet_size,
                )
                .await;
            })
            .detach();
    }
}

async fn send_loop(
    _entity: Entity,
    mut client: TcpStream,
    message_receiver: AsyncReceiver<NetworkRawPacket>,
    error_sender: AsyncSender<NetworkError>,
    cancel_flag: Arc<AtomicBool>,
) {
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            // debug!("{:?} cancel flag is set", entity);
            break;
        }

        while let Ok(message) = message_receiver.recv().await {
            debug!("send packet {:?}", message);
            if let Err(_e) = client.write_all(&message.bytes).await {
                error_sender
                    .send(NetworkError::SendError)
                    .await
                    .expect("Error channel has closed")
            }
        }
    }
}
async fn recv_loop(
    entity: Entity,
    mut stream: TcpStream,
    message_sender: AsyncSender<NetworkRawPacket>,
    error_sender: AsyncSender<NetworkError>,
    cancel_flag: Arc<AtomicBool>,
    max_packet_size: usize,
) {
    let mut buffer = vec![0; max_packet_size];

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }
        match stream.read(&mut buffer).await {
            Ok(0) => {
                error_sender
                    .send(NetworkError::ChannelClosed(entity))
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

/// TcpNode with local socket meas TCP server need to listen socket
#[allow(clippy::type_complexity)]
fn spawn_tcp_server(
    mut commands: Commands,
    q_tcp_server: Query<
        (Entity, &TcpNode, &LocalSocket),
        (Added<LocalSocket>, Without<NetworkNode>),
    >,
) {
    for (e, tcp_node, local_addr) in q_tcp_server.iter() {
        let net_node = NetworkNode::new(NetworkProtocol::TCP, Some(**local_addr), None);
        tcp_node.listen(
            **local_addr,
            net_node.error_channel().sender.clone_async(),
            net_node.cancel_flag.clone(),
        );

        commands.entity(e).insert(net_node);
    }
}

#[allow(clippy::type_complexity)]
fn spawn_tcp_client(
    mut commands: Commands,
    q_tcp_client: Query<
        (Entity, &TcpNode, &RemoteSocket),
        (Added<RemoteSocket>, Without<NetworkNode>),
    >,
) {
    for (e, tcp_node, remote_socket) in q_tcp_client.iter() {
        let net_node = NetworkNode::new(NetworkProtocol::TCP, None, Some(**remote_socket));

        tcp_node.connect(
            **remote_socket,
            net_node.error_channel().sender.clone_async(),
            net_node.cancel_flag.clone(),
        );

        commands.entity(e).insert(net_node);
    }
}

fn handle_new_connection(
    mut commands: Commands,
    q_tcp_server: Query<(Entity, &TcpNode, &NetworkNode)>,
    mut node_events: EventWriter<NetworkEvent>,
) {
    for (entity, tcp_node, _net_node) in q_tcp_server.iter() {
        while let Ok(Some(tcp_stream)) = tcp_node.new_connections.receiver.try_recv() {
            debug!(
                "new TCP client {:?} connected",
                tcp_stream.peer_addr().unwrap()
            );

            let new_net_node = NetworkNode::new(
                NetworkProtocol::TCP,
                None,
                Some(tcp_stream.peer_addr().unwrap()),
            );
            // Create a new entity for the client
            let child_tcp_client = commands.spawn_empty().id();
            commands.entity(child_tcp_client).insert((
                RemoteSocket(tcp_stream.peer_addr().unwrap()),
                TcpNode::stream(child_tcp_client, tcp_stream, &new_net_node),
                new_net_node,
            ));

            // Add the client to the server's children
            commands.entity(entity).add_child(child_tcp_client);

            node_events.send(NetworkEvent::Connected(child_tcp_client));
        }
    }
}

fn broadcast_message(
    q_server: Query<(&NetworkNode, Option<&Children>)>,
    q_child: Query<(&NetworkNode, &RemoteSocket)>,
) {
    for (net_node, opt_children) in q_server.iter() {
        while let Ok(Some(message)) = net_node.broadcast_channel().receiver.try_recv() {
            if let Some(children) = opt_children {
                for &child in children.iter() {
                    debug!("Broadcasting message: {:?} to {:?}", message, child);
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
}
