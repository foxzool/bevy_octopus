use async_std::net::{TcpListener, TcpStream};
use async_std::task;
use async_tungstenite::async_std::connect_async;
use async_tungstenite::tungstenite::Message;
use bevy::prelude::*;
use bytes::Bytes;
use futures::pin_mut;
use futures::prelude::*;
use kanal::{AsyncReceiver, AsyncSender};

use {async_tungstenite::accept_async, std::net::SocketAddr};

use crate::channels::ChannelId;
use crate::connections::NetworkPeer;
use crate::error::NetworkError;
use crate::network::{ConnectTo, NetworkRawPacket};
use crate::network_node::NetworkNode;
use crate::prelude::ListenTo;
use crate::shared::{AsyncChannel, NetworkEvent, NetworkNodeEvent};

pub struct WebsocketPlugin;

impl Plugin for WebsocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                spawn_websocket_server,
                spawn_websocket_client,
                handle_endpoint,
            ),
        );
    }
}

#[derive(Component)]
pub struct WebsocketNode {
    new_connection_channel: AsyncChannel<(TcpStream, SocketAddr)>,
}

impl Default for WebsocketNode {
    fn default() -> Self {
        Self::new()
    }
}

impl WebsocketNode {
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

            debug!("Websocket Server listening on {}", addr);
            while let Ok((tcp_stream, peer_addr)) = listener.accept().await {
                tcp_stream
                    .set_nodelay(true)
                    .expect("set_nodelay call failed");
                new_connection_tx
                    .send((tcp_stream, peer_addr))
                    .await
                    .unwrap();
            }
            loop {
                tokio::select! {
                    // handle shutdown signal
                    _ = shutdown_rx_clone.recv() => {
                        break;
                    }
                    // process new connection
                    result = listener.accept() => {
                        match result {
                            Ok((tcp_stream, peer_addr)) => {
                                 tcp_stream.set_nodelay(true).expect("set_nodelay call failed");
                                new_connection_tx.send((tcp_stream, peer_addr)).await.unwrap();

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

        async_std::task::spawn(server);

        if let Ok(()) = shutdown_rx.recv().await {
            println!("Shutting down TCP server...");
        }

        Ok(())
    }
}

fn spawn_websocket_server(
    mut commands: Commands,
    q_ws_server: Query<(Entity, &ListenTo), (Added<ListenTo>, Without<NetworkNode>)>,
) {
    for (e, listen_to) in q_ws_server.iter() {
        if !["ws", "wss"].contains(&listen_to.scheme.as_str()) {
            continue;
        }

        let net_node = NetworkNode::default();

        let local_addr = listen_to.local_addr();
        let event_tx = net_node.event_channel.sender.clone_async();
        let shutdown_clone = net_node.shutdown_channel.receiver.clone_async();
        let tcp_node = WebsocketNode::new();
        let new_connection_tx = tcp_node.new_connection_channel.sender.clone_async();
        async_std::task::spawn(async move {
            match WebsocketNode::listen(local_addr, new_connection_tx, shutdown_clone).await {
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

fn spawn_websocket_client(
    mut commands: Commands,
    q_ws_client: Query<(Entity, &ConnectTo), (Added<ConnectTo>, Without<NetworkNode>)>,
) {
    for (e, connect_to) in q_ws_client.iter() {
        if !["ws", "wss"].contains(&connect_to.scheme.as_str()) {
            continue;
        }

        let new_net_node = NetworkNode::default();
        let remote_addr = connect_to.to_string();

        let recv_tx = new_net_node.recv_message_channel.sender.clone_async();
        let message_rx = new_net_node.send_message_channel.receiver.clone_async();
        let event_tx = new_net_node.event_channel.sender.clone_async();
        let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();

        let url_str = connect_to.0.to_string();
        async_std::task::spawn(async move {
            let tasks = vec![
                task::spawn(handle_client_conn(
                    url_str,
                    remote_addr,
                    recv_tx,
                    message_rx,
                    event_tx.clone(),
                )),
                task::spawn(async move {
                    while shutdown_rx.recv().await.is_ok() {
                        break;
                    }
                    Ok(())
                }),
            ];
            match future::try_join_all(tasks).await {
                Ok(_) => {}
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

async fn handle_client_conn(
    url: String,
    addr: String,
    recv_tx: AsyncSender<NetworkRawPacket>,
    message_rx: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
) -> Result<(), NetworkError> {
    let ws_stream = connect_async(url.clone()).await?;

    let (mut writer, read) = ws_stream.0.split();

    let ws_to_output = {
        read.for_each(|message| async {
            let data = message.unwrap().into_data();
            recv_tx
                .send(NetworkRawPacket {
                    addr: addr.clone(),
                    bytes: Bytes::copy_from_slice(&data),
                })
                .await
                .unwrap();
        })
    };

    let write_task = async move {
        while let Ok(data) = message_rx.recv().await {
            // trace!("write {} bytes to {} ", data.bytes.len(), addr);
            if let Err(e) = writer.send(Message::binary(data.bytes)).await {
                eprintln!("Failed to write data to  ws socket: {}", e);
                event_tx
                    .send(NetworkEvent::Error(NetworkError::SendError))
                    .await
                    .unwrap();
                break;
            }
        }
    };

    pin_mut!(write_task, ws_to_output);
    future::select(write_task, ws_to_output).await;

    Ok(())
}
async fn server_handle_conn(
    tcp_stream: TcpStream,
    addr: String,
    recv_tx: AsyncSender<NetworkRawPacket>,
    message_rx: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
) {
    let ws_stream = accept_async(tcp_stream)
        .await
        .expect("Failed TCP incoming connection");
    let (mut writer, read) = ws_stream.split();

    let ws_to_output = {
        read.for_each(|message| async {
            let data = message.unwrap().into_data();
            recv_tx
                .send(NetworkRawPacket {
                    addr: addr.clone(),
                    bytes: Bytes::copy_from_slice(&data),
                })
                .await
                .unwrap();
        })
    };

    let write_task = async move {
        while let Ok(data) = message_rx.recv().await {
            // trace!("write {} bytes to {} ", data.bytes.len(), addr);
            if let Err(e) = writer.send(Message::binary(data.bytes)).await {
                eprintln!("Failed to write data to  ws socket: {}", e);
                event_tx
                    .send(NetworkEvent::Error(NetworkError::SendError))
                    .await
                    .unwrap();
                break;
            }
        }
    };

    pin_mut!(write_task, ws_to_output);
    future::select(write_task, ws_to_output).await;
}

fn handle_endpoint(
    mut commands: Commands,
    q_ws_server: Query<(Entity, &WebsocketNode, &NetworkNode, &ChannelId)>,
    mut node_events: EventWriter<NetworkNodeEvent>,
) {
    for (entity, ws_node, net_node, channel_id) in q_ws_server.iter() {
        while let Ok(Some((tcp_stream, socket))) =
            ws_node.new_connection_channel.receiver.try_recv()
        {
            let new_net_node = NetworkNode::default();
            // Create a new entity for the client
            let child_tcp_client = commands.spawn_empty().id();
            let recv_tx = net_node.recv_message_channel.sender.clone_async();
            let message_rx = new_net_node.send_message_channel.receiver.clone_async();
            let event_tx = new_net_node.event_channel.sender.clone_async();
            let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();

            task::spawn(async move {
                let tasks = vec![
                    task::spawn(server_handle_conn(
                        tcp_stream,
                        socket.to_string(),
                        recv_tx,
                        message_rx,
                        event_tx,
                    )),
                    task::spawn(async move {
                        while shutdown_rx.recv().await.is_ok() {
                            break;
                        }
                    }),
                ];

                future::join_all(tasks).await
            });

            let peer = NetworkPeer {};

            debug!(
                "new TCP client {:?} connected {:?}",
                socket, child_tcp_client
            );
            let url_str = format!("tcp://{}", socket);
            commands.entity(child_tcp_client).insert((
                ConnectTo::new(&url_str),
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
