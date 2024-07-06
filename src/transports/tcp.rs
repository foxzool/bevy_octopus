use std::net::SocketAddr;

use async_std::{
    io::WriteExt,
    net::{TcpListener, TcpStream},
    prelude::StreamExt,
    task,
};
use bevy::prelude::*;
use bytes::Bytes;
use futures::{AsyncReadExt, future};
use kanal::{AsyncReceiver, AsyncSender};

use crate::{
    channels::ChannelId,
    connections::NetworkPeer,
    error::NetworkError,
    network::{ConnectTo, ListenTo, NetworkRawPacket},
    network_node::NetworkNode,
    shared::{AsyncChannel, NetworkEvent, NetworkNodeEvent},
};

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
    new_connection_channel: AsyncChannel<TcpStream>,
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
        new_connection_tx: AsyncSender<TcpStream>,
    ) -> Result<(), NetworkError> {
        let listener = TcpListener::bind(addr).await?;

        debug!("TCP Server listening on {}", addr);
        let mut incoming = listener.incoming();

        while let Some(stream) = incoming.next().await {
            let stream = stream?;
            stream.set_nodelay(true).expect("set_nodelay call failed");
            new_connection_tx.send(stream).await.unwrap();
        }

        Ok(())
    }
}

async fn handle_connection(
    stream: TcpStream,
    recv_tx: AsyncSender<NetworkRawPacket>,
    message_rx: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
    shutdown_rx: AsyncReceiver<()>,
) {
    let local_addr = stream.local_addr().unwrap();
    let addr = stream.peer_addr().unwrap();
    let (mut reader, mut writer) = stream.split();

    let event_tx_clone = event_tx.clone();
    let read_task = async move {
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
                        .send(NetworkRawPacket::new(addr, Bytes::from_iter(data)))
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

    let tasks = vec![
        task::spawn(read_task),
        task::spawn(write_task),
        task::spawn(async move {
            let _ = shutdown_rx.recv().await;
        }),
    ];

    future::join_all(tasks).await;
}

/// TcpNode with local socket meas TCP server need to listen socket
#[allow(clippy::type_complexity)]
fn spawn_tcp_server(
    mut commands: Commands,
    q_tcp_server: Query<(Entity, &ListenTo), (Added<ListenTo>, Without<NetworkNode>)>,
) {
    for (e, listen_to) in q_tcp_server.iter() {
        if !["tcp", "ssl"].contains(&listen_to.scheme()) {
            continue;
        }

        let net_node = NetworkNode::default();

        let local_addr = listen_to.local_addr();
        let event_tx = net_node.event_channel.sender.clone_async();
        let shutdown_clone = net_node.shutdown_channel.receiver.clone_async();
        let tcp_node = TcpNode::new();
        let new_connection_tx = tcp_node.new_connection_channel.sender.clone_async();
        task::spawn(async move {
            let tasks = vec![
                task::spawn(TcpNode::listen(local_addr, new_connection_tx)),
                task::spawn(async move {
                    match shutdown_clone.recv().await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(NetworkError::ReceiveError(e)),
                    }
                }),
            ];

            match future::try_join_all(tasks).await {
                Ok(_) => {}
                Err(err) => event_tx
                    .send(NetworkEvent::Error(err))
                    .await
                    .expect("event channel has closed"),
            }
        });

        commands.entity(e).insert((net_node, tcp_node));
    }
}

#[allow(clippy::type_complexity)]
fn spawn_tcp_client(
    mut commands: Commands,
    q_tcp_client: Query<(Entity, &ConnectTo), (Added<ConnectTo>, Without<NetworkNode>)>,
) {
    for (e, connect_to) in q_tcp_client.iter() {
        if !["tcp", "ssl"].contains(&connect_to.scheme()) {
            continue;
        }

        let new_net_node = NetworkNode::default();

        let addr = connect_to.peer_addr();
        let recv_tx = new_net_node.recv_message_channel.sender.clone_async();
        let message_rx = new_net_node.send_message_channel.receiver.clone_async();
        let event_tx = new_net_node.event_channel.sender.clone_async();
        let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();

        task::spawn(async move {
            match TcpStream::connect(addr).await {
                Ok(tcp_stream) => {
                    tcp_stream
                        .set_nodelay(true)
                        .expect("set_nodelay call failed");
                    handle_connection(tcp_stream, recv_tx, message_rx, event_tx, shutdown_rx).await;
                }
                Err(err) => event_tx
                    .send(NetworkEvent::Error(NetworkError::Connection(
                        err.to_string(),
                    )))
                    .await
                    .expect("event channel has closed"),
            }
        });

        let peer = NetworkPeer {};

        commands.entity(e).insert((new_net_node, peer));
    }
}

fn handle_endpoint(
    mut commands: Commands,
    q_tcp_server: Query<(Entity, &TcpNode, &NetworkNode, &ChannelId)>,
    mut node_events: EventWriter<NetworkNodeEvent>,
) {
    for (entity, tcp_node, net_node, channel_id) in q_tcp_server.iter() {
        while let Ok(Some(tcp_stream)) = tcp_node.new_connection_channel.receiver.try_recv() {
            let new_net_node = NetworkNode::default();
            // Create a new entity for the client
            let child_tcp_client = commands.spawn_empty().id();
            let recv_tx = net_node.recv_message_channel.sender.clone_async();
            let message_rx = new_net_node.send_message_channel.receiver.clone_async();
            let event_tx = new_net_node.event_channel.sender.clone_async();
            let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();
            let peer_str = format!("tcp://{}", tcp_stream.peer_addr().unwrap());
            task::spawn(async move {
                handle_connection(tcp_stream, recv_tx, message_rx, event_tx, shutdown_rx).await;
            });
            let peer = NetworkPeer {};

            debug!("new client {:?} connected {:?}", peer_str, child_tcp_client);

            commands.entity(child_tcp_client).insert((
                ConnectTo::new(&peer_str),
                new_net_node,
                *channel_id,
                peer,
            ));

            // Add the client to the server's children
            commands.entity(entity).add_child(child_tcp_client);

            node_events.send(NetworkNodeEvent {
                node: child_tcp_client,
                channel_id: *channel_id,
                event: NetworkEvent::Connected,
            });
        }
    }
}
