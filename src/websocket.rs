use bevy::app::{App, Plugin, PostUpdate};
use bevy::log::{debug, trace};
use bevy::prelude::{Added, Commands, Component, Entity, Query, Res, Without};
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

use crate::connections::NetworkPeer;
use crate::error::NetworkError;
use crate::network::{LocalSocket, NetworkProtocol, NetworkRawPacket, RemoteSocket};
use crate::network_node::NetworkNode;
use crate::shared::{AsyncChannel, AsyncRuntime, NetworkEvent};

pub struct WebsocketPlugin;

impl Plugin for WebsocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, (spawn_websocket_server, spawn_websocket_client));
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
    q_ws_server: Query<
        (Entity, &NetworkProtocol, &LocalSocket),
        (Added<LocalSocket>, Without<NetworkNode>),
    >,
) {
    for (e, protocol, local_addr) in q_ws_server.iter() {
        if *protocol != NetworkProtocol::TCP {
            continue;
        }

        let net_node = NetworkNode::default();

        let local_addr = local_addr.0;
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
    q_ws_client: Query<
        (Entity, &NetworkProtocol, &RemoteSocket),
        (Added<RemoteSocket>, Without<NetworkNode>),
    >,
) {
    for (e, protocol, remote_socket) in q_ws_client.iter() {
        if *protocol != NetworkProtocol::TCP {
            continue;
        }

        let new_net_node = NetworkNode::default();

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
                    crate::websocket::handle_connection(
                        tcp_stream,
                        recv_tx,
                        message_rx,
                        event_tx,
                        shutdown_rx,
                    )
                    .await;
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

async fn handle_connection(
    tcp_stream: TcpStream,
    recv_tx: AsyncSender<NetworkRawPacket>,
    message_rx: AsyncReceiver<NetworkRawPacket>,
    event_tx: AsyncSender<NetworkEvent>,
    shutdown_rx: AsyncReceiver<()>,
) {
    let local_addr = tcp_stream.local_addr().unwrap();
    let addr = tcp_stream.peer_addr().unwrap();
    let socket = accept_async(TokioAdapter::new(tcp_stream))
        .await
        .expect("Failed TCP incoming connection");
    let ws_stream = WsStream::new(socket);
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
