use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use bytes::Bytes;
use kanal::{bounded_async, AsyncReceiver, AsyncSender};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::runtime::Runtime;
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use tokio::task;

use crate::error::NetworkError;
use crate::network::{LocalSocket, NetworkEvent, NetworkRawPacket};
use crate::network_manager::NetworkNode;
use crate::prelude::RemoteSocket;
use crate::shared::{AsyncRuntime, NetworkProtocol};
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
    new_connection_channel: AsyncChannel<(TcpStream, SocketAddr)>,
}

// impl Default for TcpNode {
//     fn default() -> Self {
//         Self::new()
//     }
// }

// impl TcpNode {
//     pub fn new() -> Self {
//         Self {
//             new_connections: AsyncChannel::new(),
//         }
//     }
//
//     pub fn connect(
//         &self,
//         addr: SocketAddr,
//         error_sender: AsyncSender<NetworkError>,
//         cancel_flag: Arc<AtomicBool>,
//     ) {
//         let new_connection_sender = self.new_connections.sender.clone_async();
//         IoTaskPool::get()
//             .spawn(async move {
//                 match TcpStream::connect(addr).await {
//                     Ok(stream) => {
//                         debug!(
//                             "Starting TCP Client on {:?} => {:?}",
//                             stream.local_addr().unwrap(),
//                             stream.peer_addr().unwrap(),
//                         );
//                         loop {
//                             if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
//                                 stream.shutdown(std::net::Shutdown::Both).unwrap();
//                                 break;
//                             }
//                         }
//                         new_connection_sender.send(stream).await.unwrap();
//                     }
//                     Err(e) => {
//                         // error_sender
//                         //     .send(NetworkError::Connection(e))
//                         //     .await
//                         //     .expect("Error channel has been closed");
//                     }
//                 }
//             })
//             .detach();
//     }
//
//     pub fn stream(entity: Entity, stream: TcpStream, net_node: &NetworkNode) {
//         let sender_rx = net_node.send_channel().receiver.clone_async();
//         let error_tx = net_node.error_channel().sender.clone_async();
//         let cancel_flag = net_node.cancel_flag.clone();
//         let send_stream = stream.clone();
//         IoTaskPool::get()
//             .spawn(async move {
//                 send_loop(
//                     entity,
//                     send_stream,
//                     sender_rx.clone(),
//                     error_tx.clone(),
//                     cancel_flag.clone(),
//                 )
//                 .await;
//             })
//             .detach();
//
//         let receiver = net_node.recv_channel().sender.clone_async();
//         let error_tx = net_node.error_channel().sender.clone_async();
//         let cancel_flag = net_node.cancel_flag.clone();
//         let recv_stream = stream.clone();
//         let max_packet_size = net_node.max_packet_size;
//         // IoTaskPool::get()
//         //     .spawn(async move {
//         //         recv_loop(
//         //             entity,
//         //             recv_stream,
//         //             receiver,
//         //             error_tx,
//         //             cancel_flag,
//         //             max_packet_size,
//         //         )
//         //         .await;
//         //     })
//         //     .detach();
//     }
// }

impl TcpNode {
    pub fn new() -> Self {
        Self {
            new_connection_channel: AsyncChannel::new(),
        }
    }
    pub async fn start(
        addr: SocketAddr,
        message_rx: AsyncReceiver<NetworkRawPacket>,
        recv_tx: AsyncSender<NetworkRawPacket>,
        new_connection_tx: AsyncSender<(TcpStream, SocketAddr)>,
        shutdown_rx: AsyncReceiver<()>,
    ) -> Result<(), NetworkError> {
        let stop = Arc::new(AtomicBool::new(false));

        let shutdown_rx_clone = shutdown_rx.clone();

        let server = async move {
            let listener = TcpListener::bind(addr).await.unwrap();
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
                                new_connection_tx.send((tcp_stream, socket)).await.unwrap();

                                // tokio::spawn(handle_connection(tcp_stream, recv_tx.clone(), message_rx.clone()));
                            }
                            Err(e) => {
                                eprintln!("Failed to accept client connection: {}", e);
                            }
                        }
                    }
                }
            }
        };

        tokio::spawn(server);

        if let Ok(()) = shutdown_rx.recv().await {
            println!("Shutting down server...");
        }

        Ok(())
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    recv_tx: AsyncSender<NetworkRawPacket>,
    message_rx: AsyncReceiver<NetworkRawPacket>,
    error_tx: AsyncSender<NetworkError>,
    shutdown_rx: AsyncReceiver<()>,
) {
    let addr = socket.peer_addr().unwrap();
    let (mut reader, mut writer) = socket.split();

    let read_task = async {
        let mut buffer = vec![0; 1024];

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("connection closed by peer");
                    // 连接关闭
                    break;
                }
                Ok(n) => {
                    let data = buffer[..n].to_vec();
                    recv_tx
                        .send(NetworkRawPacket {
                            socket: addr,
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
            println!("write data {:?}", data);
            if let Err(e) = writer.write_all(&data.bytes).await {
                eprintln!("Failed to write data to socket: {}", e);
                break;
            }
        }
    };

    // 启动读取和写入任务
    tokio::select! {
        Ok(_) = shutdown_rx.recv() => {
            debug!("shutdown connection");
            return;
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

        let local_addr = local_addr.0.clone();
        let shutdown_clone = net_node.shutdown_channel.receiver.clone_async();
        let message_rx_clone = net_node.send_message_channel.receiver.clone_async();
        let recv_tx = net_node.recv_message_channel.sender.clone_async();
        let tcp_node = TcpNode::new();
        let new_connection_tx = tcp_node.new_connection_channel.sender.clone_async();
        rt.spawn(async move {
            TcpNode::start(
                local_addr,
                message_rx_clone,
                recv_tx,
                new_connection_tx,
                shutdown_clone,
            )
            .await
            .unwrap()
        });

        commands.entity(e).insert((net_node, tcp_node));
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

        // tcp_node.connect(
        //     **remote_socket,
        //     net_node.error_channel().sender.clone_async(),
        //     net_node.cancel_flag.clone(),
        // );

        commands.entity(e).insert(net_node);
    }
}

fn handle_new_connection(
    rt: Res<AsyncRuntime>,
    mut commands: Commands,
    q_tcp_server: Query<(Entity, &TcpNode, &NetworkNode)>,
    mut node_events: EventWriter<NetworkEvent>,
) {
    for (entity, tcp_node, _net_node) in q_tcp_server.iter() {
        while let Ok(Some((tcp_stream, socket))) =
            tcp_node.new_connection_channel.receiver.try_recv()
        {
            let new_net_node = NetworkNode::new(NetworkProtocol::TCP, None, Some(socket));
            // Create a new entity for the client
            let child_tcp_client = commands.spawn_empty().id();
            let recv_tx = new_net_node.recv_message_channel.sender.clone_async();
            let message_rx = new_net_node.send_message_channel.receiver.clone_async();
            let error_tx = new_net_node.error_channel.sender.clone_async();
            let shutdown_rx = new_net_node.shutdown_channel.receiver.clone_async();
            rt.spawn(async move {
                handle_connection(tcp_stream, recv_tx, message_rx, error_tx, shutdown_rx).await;
                println!("handle connection end");
            });

            debug!(
                "new TCP client {:?} connected {:?}",
                socket, child_tcp_client
            );
            commands.entity(child_tcp_client).insert((
                RemoteSocket(socket),
                NetworkProtocol::TCP,
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
                        .send_message_channel
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
