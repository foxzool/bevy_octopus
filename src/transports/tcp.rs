use std::net::{SocketAddr, ToSocketAddrs};

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
    client::{ClientNode, StartClient},
    error::NetworkError,
    network_node::{
        AsyncChannel, NetworkAddress, NetworkEvent, NetworkNode, NetworkPeer, NetworkRawPacket,
    },
    server::{ServerNode, StartServer},
};

pub struct TcpPlugin;

impl Plugin for TcpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, handle_endpoint)
            .add_observer(on_start_server)
            .add_observer(on_start_client);
    }
}

#[derive(Debug, Clone)]
pub struct TcpAddress {
    pub socket_addr: SocketAddr,
    new_connection_channel: AsyncChannel<TcpStream>,
}

impl TcpAddress {
    pub fn new(address: impl ToSocketAddrs) -> Self {
        let socket_addr = address.to_socket_addrs().unwrap().next().unwrap();
        Self {
            socket_addr,
            new_connection_channel: Default::default(),
        }
    }
}

impl NetworkAddress for TcpAddress {
    fn to_string(&self) -> String {
        self.socket_addr.to_string()
    }

    fn from_string(s: &str) -> Result<Self, String> {
        match s.parse() {
            Ok(socket_addr) => Ok(Self {
                socket_addr,
                new_connection_channel: Default::default(),
            }),
            Err(e) => Err(e.to_string()),
        }
    }
}

async fn listen(
    addr: SocketAddr,
    event_tx: AsyncSender<NetworkEvent>,
    new_connection_tx: AsyncSender<TcpStream>,
) -> Result<(), NetworkError> {
    let listener = TcpListener::bind(addr).await?;
    info!("TCP Server listening on {}", addr);
    let _ = event_tx.send(NetworkEvent::Listen).await;
    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        stream.set_nodelay(true).expect("set_nodelay call failed");
        new_connection_tx.send(stream).await.unwrap();
    }

    Ok(())
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
    info!("TCP local {} connected to remote {}", local_addr, addr);

    let (mut reader, mut writer) = stream.split();
    let _ = event_tx.send(NetworkEvent::Connected).await;
    let event_tx_clone = event_tx.clone();
    let read_task = async move {
        let mut buffer = vec![0; 1024];

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    let _ = event_tx_clone.send(NetworkEvent::Disconnected).await;
                    break;
                }
                Ok(n) => {
                    let data = buffer[..n].to_vec();
                    trace!("{} read {} bytes from {}", local_addr, n, addr);
                    let _ = recv_tx
                        .send(NetworkRawPacket {
                            addr: Some(addr),
                            bytes: Bytes::from_iter(data),
                            text: None,
                        })
                        .await;
                }
                Err(e) => {
                    trace!("Failed to read data from socket: {}", e);
                    let _ = event_tx_clone
                        .send(NetworkEvent::Error(NetworkError::Common(e.to_string())))
                        .await;
                    let _ = event_tx_clone.send(NetworkEvent::Disconnected).await;
                    break;
                }
            }
        }
    };

    let write_task = async move {
        while let Ok(data) = message_rx.recv().await {
            trace!("write {} bytes to {} ", data.bytes.len(), addr);
            if let Err(e) = writer.write_all(&data.bytes).await {
                trace!("Failed to write data to socket: {}", e);
                let _ = event_tx
                    .send(NetworkEvent::Error(NetworkError::Common(e.to_string())))
                    .await;
                let _ = event_tx.send(NetworkEvent::Disconnected).await;
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
fn on_start_server(
    on: On<StartServer>,
    q_tcp_server: Query<(&NetworkNode, &ServerNode<TcpAddress>)>,
) {
    let ev = on.event();
    if let Ok((net_node, server)) = q_tcp_server.get(ev.entity) {
        let local_addr = server.socket_addr;
        let event_tx = net_node.event_channel.sender.clone_async();
        let event_tx_clone = net_node.event_channel.sender.clone_async();
        let shutdown_clone = net_node.shutdown_channel.receiver.clone_async();
        let new_connection_tx = server.new_connection_channel.sender.clone_async();
        task::spawn(async move {
            let tasks = vec![
                task::spawn(listen(local_addr, event_tx_clone, new_connection_tx)),
                task::spawn(async move {
                    match shutdown_clone.recv().await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(NetworkError::Common(e.to_string())),
                    }
                }),
            ];

            if let Err(err) = future::try_join_all(tasks).await {
                let _ = event_tx.send(NetworkEvent::Error(err)).await;
            }
        });
    }
}

fn on_start_client(
    on: On<StartClient>,
    q_tcp_client: Query<(&NetworkNode, &ClientNode<TcpAddress>), Without<NetworkPeer>>,
) {
    let ev = on.event();
    if let Ok((net_node, remote_addr)) = q_tcp_client.get(ev.entity) {
        info!("try connect to {}", remote_addr.to_string());

        let addr = remote_addr.socket_addr;
        let recv_tx = net_node.recv_message_channel.sender.clone_async();
        let message_rx = net_node.send_message_channel.receiver.clone_async();
        let event_tx = net_node.event_channel.sender.clone_async();
        let shutdown_rx = net_node.shutdown_channel.receiver.clone_async();

        task::spawn(async move {
            match TcpStream::connect(addr).await {
                Ok(tcp_stream) => {
                    tcp_stream
                        .set_nodelay(true)
                        .expect("set_nodelay call failed");
                    handle_connection(tcp_stream, recv_tx, message_rx, event_tx, shutdown_rx).await;
                }
                Err(err) => {
                    let _ = event_tx
                        .send(NetworkEvent::Error(NetworkError::Connection(
                            err.to_string(),
                        )))
                        .await;
                }
            }
        });
    }
}

fn handle_endpoint(
    mut commands: Commands,
    q_tcp_server: Query<(Entity, &ServerNode<TcpAddress>, &NetworkNode, &ChannelId)>,
) {
    for (entity, tcp_node, net_node, channel_id) in q_tcp_server.iter() {
        while let Ok(Some(tcp_stream)) = tcp_node.new_connection_channel.receiver.try_recv() {
            let new_net_node = NetworkNode::default();
            // Create a new entity for the client
            let peer_entity = commands.spawn_empty().id();
            let recv_tx = net_node.recv_message_channel.sender.clone_async();
            let message_rx = new_net_node.send_message_channel.receiver.clone_async();
            let event_tx = new_net_node.event_channel.sender.clone_async();
            let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();
            let peer_socket = tcp_stream.peer_addr().unwrap();
            task::spawn(async move {
                handle_connection(tcp_stream, recv_tx, message_rx, event_tx, shutdown_rx).await;
            });
            let peer = NetworkPeer;

            commands.entity(peer_entity).insert((
                new_net_node,
                *channel_id,
                ClientNode(TcpAddress::new(peer_socket)),
                peer,
            ));

            info!("new client connected {:?}", peer_entity);

            // Add the client to the server's children
            commands.entity(entity).add_child(peer_entity);
            // commands.trigger_targets(NetworkEvent::Connected, vec![peer_entity]);
        }
    }
}
