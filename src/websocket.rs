use async_tungstenite::tokio::{connect_async, ConnectStream};
use bevy::prelude::*;
use bytes::Bytes;
use futures::AsyncWriteExt;
use kanal::{AsyncReceiver, AsyncSender};

use {
    async_tungstenite::{accept_async, tokio::TokioAdapter},
    futures::AsyncReadExt,
    std::net::SocketAddr,
    tokio::net::{TcpListener, TcpStream},
    ws_stream_tungstenite::*,
};

use crate::channels::ChannelId;
use crate::connections::NetworkPeer;
use crate::error::NetworkError;
use crate::network::{ConnectTo, NetworkRawPacket};
use crate::network_node::NetworkNode;
use crate::prelude::ListenTo;
use crate::shared::{AsyncChannel, AsyncRuntime, NetworkEvent, NetworkNodeEvent};

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

        tokio::spawn(server);

        if let Ok(()) = shutdown_rx.recv().await {
            println!("Shutting down TCP server...");
        }

        Ok(())
    }
}

fn spawn_websocket_server(
    mut commands: Commands,
    rt: Res<AsyncRuntime>,
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
        rt.spawn(async move {
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
    rt: Res<AsyncRuntime>,
    mut commands: Commands,
    q_ws_client: Query<(Entity, &ConnectTo), (Added<ConnectTo>, Without<NetworkNode>)>,
) {
    for (e, connect_to) in q_ws_client.iter() {
        if !["ws", "wss"].contains(&connect_to.scheme.as_str()) {
            continue;
        }

        let new_net_node = NetworkNode::default();
        let remote_addr = connect_to.peer_addr();

        let recv_tx = new_net_node.recv_message_channel.sender.clone_async();
        let message_rx = new_net_node.send_message_channel.receiver.clone_async();
        let event_tx = new_net_node.event_channel.sender.clone_async();
        let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();

        let url_str = connect_to.0.to_string();
        rt.spawn(async move {
            match connect_async(&url_str).await {
                Ok(stream) => {
                    let ws_stream = WsStream::new(stream.0);
                    handle_conn(
                        ws_stream,
                        remote_addr,
                        recv_tx,
                        message_rx,
                        event_tx,
                        shutdown_rx,
                    )
                    .await;
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

async fn handle_conn(
    ws_stream: WsStream<ConnectStream>,
    addr: SocketAddr,
    recv_tx: AsyncSender<NetworkRawPacket>,
    message_rx: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
    shutdown_rx: AsyncReceiver<()>,
) {
    let (mut reader, mut writer) = ws_stream.split();

    let event_tx_clone = event_tx.clone();
    let read_task = async {
        let mut buffer = vec![0; 1024];

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    event_tx_clone
                        .send(NetworkEvent::Disconnected)
                        .await
                        .expect("ws event channel has closed");
                    break;
                }
                Ok(n) => {
                    let data = buffer[..n].to_vec();
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
                eprintln!("Failed to write data to  ws socket: {}", e);
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

fn handle_endpoint(
    rt: Res<AsyncRuntime>,
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

            rt.spawn(async move {
                let s = accept_async(TokioAdapter::new(tcp_stream))
                    .await
                    .expect("Failed TCP incoming connection");
                let ws_stream = WsStream::new(s);
                handle_conn(
                    ws_stream,
                    socket,
                    recv_tx,
                    message_rx,
                    event_tx,
                    shutdown_rx,
                )
                .await;
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
