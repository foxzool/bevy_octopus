use std::net::SocketAddr;

use async_std::{
    net::{TcpListener, TcpStream},
    task,
};
use async_tungstenite::{accept_async, async_std::connect_async, tungstenite::Message};
use bevy::prelude::*;
use bytes::Bytes;
use futures::{pin_mut, prelude::*};
use kanal::{AsyncReceiver, AsyncSender};

use bevy_octopus::prelude::*;

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
    ) -> Result<(), NetworkError> {
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

            Ok::<(), NetworkError>(())
        };

        async_std::task::spawn(server);

        Ok(())
    }
}

fn spawn_websocket_server(
    mut commands: Commands,
    q_ws_server: Query<(Entity, &NetworkNode, &ListenTo), Added<ListenTo>>,
) {
    for (e, net_node, listen_to) in q_ws_server.iter() {
        if !["ws", "wss"].contains(&listen_to.scheme()) {
            continue;
        }

        let local_addr = listen_to.local_addr();
        let event_tx = net_node.event_channel.sender.clone_async();
        let shutdown_clone = net_node.shutdown_channel.receiver.clone_async();
        let ws_node = WebsocketNode::new();
        let new_connection_tx = ws_node.new_connection_channel.sender.clone_async();
        async_std::task::spawn(async move {
            let tasks = vec![
                async_std::task::spawn(WebsocketNode::listen(local_addr, new_connection_tx)),
                async_std::task::spawn(async move {
                    let _ = shutdown_clone.recv().await;
                    Ok(())
                }),
            ];

            if let Err(err) = future::try_join_all(tasks).await {
                let _ = event_tx.send(NetworkEvent::Error(err)).await;
            }
        });

        commands.entity(e).insert(ws_node);
    }
}

#[allow(clippy::type_complexity)]
fn spawn_websocket_client(
    q_ws_client: Query<(&NetworkNode, &ConnectTo), (Added<ConnectTo>, Without<NetworkPeer>)>,
) {
    for (net_node, connect_to) in q_ws_client.iter() {
        if !["ws", "wss"].contains(&connect_to.scheme()) {
            continue;
        }

        let remote_addr = connect_to.to_string();

        let recv_tx = net_node.recv_message_channel.sender.clone_async();
        let message_rx = net_node.send_message_channel.receiver.clone_async();
        let event_tx = net_node.event_channel.sender.clone_async();
        let shutdown_rx = net_node.shutdown_channel.receiver.clone_async();

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
                    let _ = shutdown_rx.recv().await;
                    Ok(())
                }),
            ];
            if let Err(err) = future::try_join_all(tasks).await {
                let _ = event_tx.send(NetworkEvent::Error(err)).await;
            }
        });
    }
}

async fn handle_client_conn(
    url: String,
    addr: String,
    recv_tx: AsyncSender<NetworkRawPacket>,
    message_rx: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
) -> Result<(), NetworkError> {
    let ws_stream = connect_async(url.clone())
        .await
        .map_err(|e| NetworkError::Connection(e.to_string()))?;

    let _ = event_tx.send(NetworkEvent::Connected).await;

    let (mut writer, read) = ws_stream.0.split();

    let ws_to_output = {
        read.for_each(|message| async {
            match message {
                Ok(message) => {
                    let data = message.into_data();
                    recv_tx
                        .send(NetworkRawPacket::new(addr.clone(), Bytes::from_iter(data)))
                        .await
                        .unwrap();
                }
                Err(err) => {
                    error!("{} websocket error {:?}", addr, err);
                }
            }
        })
    };

    let write_task = async move {
        while let Ok(data) = message_rx.recv().await {
            trace!("write {} bytes to {} ", data.bytes.len(), data.addr);
            let message = if let Some(text) = data.text {
                Message::Text(text)
            } else {
                Message::binary(data.bytes)
            };

            if let Err(e) = writer.send(message).await {
                let _ = event_tx
                    .send(NetworkEvent::Error(NetworkError::Custom(e.to_string())))
                    .await;

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
            match message {
                Ok(message) => {
                    let data = message.into_data();
                    let _ = recv_tx
                        .send(NetworkRawPacket::new(addr.clone(), Bytes::from_iter(data)))
                        .await;
                }
                Err(err) => {
                    error!("{} websocket error {:?}", addr, err);
                }
            }
        })
    };

    let write_task = async move {
        while let Ok(data) = message_rx.recv().await {
            let message = if let Some(text) = data.text {
                Message::Text(text)
            } else {
                Message::binary(data.bytes)
            };
            if let Err(e) = writer.send(message).await {
                let _ = event_tx
                    .send(NetworkEvent::Error(NetworkError::Custom(e.to_string())))
                    .await;

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
            let child_ws_client = commands.spawn_empty().id();
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
                        let _ = shutdown_rx.recv().await;
                    }),
                ];

                future::join_all(tasks).await
            });

            let peer = NetworkPeer {};

            debug!(
                "new websocket client {:?} connected {:?}",
                socket, child_ws_client
            );
            let url_str = format!("ws://{}", socket);
            commands.entity(child_ws_client).insert((
                ConnectTo::new(&url_str),
                new_net_node,
                *channel_id,
                peer,
            ));

            // Add the client to the server's children
            commands.entity(entity).add_child(child_ws_client);

            node_events.send(NetworkNodeEvent {
                node: child_ws_client,
                channel_id: *channel_id,
                event: NetworkEvent::Connected,
            });
        }
    }
}
