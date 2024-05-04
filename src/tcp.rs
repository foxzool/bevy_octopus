use std::net::SocketAddr;

use bevy::prelude::*;
use bytes::Bytes;
use kanal::{AsyncReceiver, AsyncSender};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::channels::ChannelId;
use crate::connections::NetworkPeer;
use crate::error::NetworkError;
use crate::network::{LocalSocket, NetworkRawPacket};
use crate::network::RemoteSocket;
use crate::network_manager::NetworkNode;
use crate::shared::{AsyncChannel, AsyncRuntime, NetworkEvent, NetworkNodeEvent, NetworkProtocol};

pub struct TcpPlugin;

impl Plugin for TcpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (spawn_tcp_client, spawn_tcp_server, handle_endpoint),
        );
    }
}

#[derive(Component)]
pub struct TcpNode {
    new_connection_channel: AsyncChannel<(TcpStream, SocketAddr)>,
}

impl Default for TcpNode {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpNode {
    pub fn new() -> Self {
        Self {
            new_connection_channel: AsyncChannel::new(),
        }
    }
    pub async fn listen(
        addr: SocketAddr,
        new_connection_tx: AsyncSender<(TcpStream, SocketAddr)>,
        shutdown_rx: AsyncReceiver<()>,
    ) -> Result<(), NetworkError> {
        let shutdown_rx_clone = shutdown_rx.clone();

        let server = async move {
            let listener = TcpListener::bind(addr).await?;

            debug!("TCP Server listening on {}", addr);

            loop {
                tokio::select! {
                    // handle shutdown signal
                    _ = shutdown_rx_clone.recv() => {
                        break;
                    }
                    // process new connection
                    result = listener.accept() => {
                        match result {
                            Ok((tcp_stream, socket)) => {
                                 tcp_stream.set_nodelay(true).expect("set_nodelay call failed");
                                new_connection_tx.send((tcp_stream, socket)).await.unwrap();

                            }
                            Err(e) => {
                                eprintln!("Failed to accept client connection: {}", e);
                            }
                        }
                    }
                }
            }

            Ok::<(), NetworkError>(())
        };

        tokio::spawn(server);

        if let Ok(()) = shutdown_rx.recv().await {
            println!("Shutting down TCP server...");
        }

        Ok(())
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    recv_tx: AsyncSender<NetworkRawPacket>,
    message_rx: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
    shutdown_rx: AsyncReceiver<()>,
) {
    let local_addr = stream.local_addr().unwrap();
    let addr = stream.peer_addr().unwrap();
    let (mut reader, mut writer) = stream.split();

    let event_tx_clone = event_tx.clone();
    let read_task = async {
        let mut buffer = vec![0; 1024];

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    event_tx_clone
                        .send(NetworkEvent::Disconnected)
                        .await
                        .expect("event channel has closed");
                    break;
                }
                Ok(n) => {
                    let data = buffer[..n].to_vec();
                    trace!("{} read {} bytes from {}", local_addr, n, addr);
                    recv_tx
                        .send(NetworkRawPacket {
                            addr,
                            bytes: Bytes::copy_from_slice(&data),
                        })
                        .await
                        .unwrap();
                }
                Err(e) => {
                    eprintln!("Failed to read data from socket: {}", e);
                    break;
                }
            }
        }
    };

    let write_task = async move {
        while let Ok(data) = message_rx.recv().await {
            // trace!("write {} bytes to {} ", data.bytes.len(), addr);
            if let Err(e) = writer.write_all(&data.bytes).await {
                eprintln!("Failed to write data to socket: {}", e);
                event_tx
                    .send(NetworkEvent::Error(NetworkError::SendError))
                    .await
                    .unwrap();
                break;
            }
        }
    };

    tokio::select! {
        Ok(_) = shutdown_rx.recv() => {
            debug!("shutdown connection");
        }
        _ = read_task => (),
        _ = write_task => (),
    }
}

/// TcpNode with local socket meas TCP server need to listen socket
#[allow(clippy::type_complexity)]
fn spawn_tcp_server(
    mut commands: Commands,
    rt: Res<AsyncRuntime>,
    q_tcp_server: Query<
        (Entity, &NetworkProtocol, &LocalSocket),
        (Added<LocalSocket>, Without<NetworkNode>),
    >,
) {
    for (e, protocol, local_addr) in q_tcp_server.iter() {
        if *protocol != NetworkProtocol::TCP {
            continue;
        }

        let net_node = NetworkNode::new(NetworkProtocol::TCP, Some(**local_addr), None);

        let local_addr = local_addr.0;
        let event_tx = net_node.event_channel.sender.clone_async();
        let shutdown_clone = net_node.shutdown_channel.receiver.clone_async();
        let tcp_node = TcpNode::new();
        let new_connection_tx = tcp_node.new_connection_channel.sender.clone_async();
        rt.spawn(async move {
            match TcpNode::listen(local_addr, new_connection_tx, shutdown_clone).await {
                Ok(_) => {}
                Err(err) => {
                    event_tx
                        .send(NetworkEvent::Error(err))
                        .await
                        .expect("event channel has closed");
                }
            }
        });

        commands.entity(e).insert((net_node, tcp_node));
    }
}

#[allow(clippy::type_complexity)]
fn spawn_tcp_client(
    rt: Res<AsyncRuntime>,
    mut commands: Commands,
    q_tcp_client: Query<
        (Entity, &NetworkProtocol, &RemoteSocket),
        (Added<RemoteSocket>, Without<NetworkNode>),
    >,
) {
    for (e, protocol, remote_socket) in q_tcp_client.iter() {
        if *protocol != NetworkProtocol::TCP {
            continue;
        }

        let new_net_node = NetworkNode::new(NetworkProtocol::TCP, None, Some(**remote_socket));

        let addr = remote_socket.0;
        let recv_tx = new_net_node.recv_message_channel.sender.clone_async();
        let message_rx = new_net_node.send_message_channel.receiver.clone_async();
        let event_tx = new_net_node.event_channel.sender.clone_async();
        let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();

        rt.spawn(async move {
            match TcpStream::connect(addr).await {
                Ok(tcp_stream) => {
                    tcp_stream
                        .set_nodelay(true)
                        .expect("set_nodelay call failed");
                    handle_connection(tcp_stream, recv_tx, message_rx, event_tx, shutdown_rx).await;
                }
                Err(err) => event_tx
                    .send(NetworkEvent::Error(NetworkError::Connection(err)))
                    .await
                    .expect("event channel has closed"),
            }
        });

        let peer = NetworkPeer {};

        commands.entity(e).insert((new_net_node, peer));
    }
}

fn handle_endpoint(
    rt: Res<AsyncRuntime>,
    mut commands: Commands,
    q_tcp_server: Query<(Entity, &TcpNode, &NetworkNode, &ChannelId)>,
    mut node_events: EventWriter<NetworkNodeEvent>,
) {
    for (entity, tcp_node, net_node, channel_id) in q_tcp_server.iter() {
        while let Ok(Some((tcp_stream, socket))) =
            tcp_node.new_connection_channel.receiver.try_recv()
        {
            let new_net_node = NetworkNode::new(NetworkProtocol::TCP, None, Some(socket));
            // Create a new entity for the client
            let child_tcp_client = commands.spawn_empty().id();
            let recv_tx = net_node.recv_message_channel.sender.clone_async();
            let message_rx = new_net_node.send_message_channel.receiver.clone_async();
            let event_tx = new_net_node.event_channel.sender.clone_async();
            let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();
            rt.spawn(async move {
                handle_connection(tcp_stream, recv_tx, message_rx, event_tx, shutdown_rx).await;
            });
            let peer = NetworkPeer {};

            debug!(
                "new TCP client {:?} connected {:?}",
                socket, child_tcp_client
            );
            commands.entity(child_tcp_client).insert((
                RemoteSocket(socket),
                NetworkProtocol::TCP,
                new_net_node,
                *channel_id,
                peer,
            ));

            // Add the client to the server's children
            commands.entity(entity).add_child(child_tcp_client);

            node_events.send(NetworkNodeEvent {
                node: entity,
                event: NetworkEvent::Connected,
            });
        }
    }
}
