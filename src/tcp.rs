use crate::error::NetworkError;
use crate::network::{NetworkNode, NetworkProtocol, NetworkRawPacket};
use crate::prelude::UdpNode;
use crate::AsyncChannel;
use async_net::{TcpListener, TcpStream};
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, IoTaskPool};
use futures_lite::stream::block_on;
use futures_lite::{AsyncReadExt, AsyncWriteExt, StreamExt};
use kanal::{AsyncReceiver, AsyncSender};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use bytes::Bytes;

pub struct TcpPlugin;

impl Plugin for TcpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                control_tcp_client,
                control_tcp_server,
                handle_new_connection,
            ),
        );
    }
}

#[derive(Component)]
pub struct TcpServerNode {
    listener: Option<TcpListener>,
    new_connections: AsyncChannel<TcpStream>,
}

impl TcpServerNode {
    pub fn new(addrs: impl ToSocketAddrs) -> Self {
        let sockets: Vec<_> = addrs.to_socket_addrs().unwrap().collect();
        let listener = futures_lite::future::block_on(
            ComputeTaskPool::get()
                .spawn(async move { TcpListener::bind(&*sockets).await.unwrap() }),
        );
        debug!(
            "Starting TCP server on {:?}",
            listener.local_addr().unwrap()
        );

        Self {
            listener: Some(listener),
            new_connections: AsyncChannel::new(),
        }
    }

    pub fn start(&self, network_node: &mut NetworkNode) {
        match self.listener.clone() {
            None => network_node
                .error_channel()
                .sender
                .send(NetworkError::Error("server not exist".to_string()))
                .expect("Error channel has closed"),
            Some(listener) => {
                let new_connections_sender = self.new_connections.sender.clone_async();
                IoTaskPool::get()
                    .spawn(async move {
                        let mut incoming = listener.incoming();
                        loop {
                            while let Some(Ok(income)) = incoming.next().await {
                                new_connections_sender.send(income).await.unwrap();
                            }
                        }
                    })
                    .detach();
            }
        }
    }

    pub async fn recv_loop(
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

            let n = stream.read(&mut buffer).await.unwrap();
            if n == 0 {
                break; // EOF
            }
            debug!(
                "{} Received {} bytes from {}",
                "?",
                n,
                stream.local_addr().unwrap(),
            );
            let bytes = Bytes::copy_from_slice(&buffer[..n]);
            message_sender
                .send(NetworkRawPacket {
                    socket: Some(stream.local_addr().unwrap()),
                    bytes,
                })
                .await
                .expect("Message channel has closed.");
        }
    }
}

#[derive(Component)]
pub struct TcpClientNode {
    client: TcpStream,
}

impl TcpClientNode {
    pub fn new(addrs: impl ToSocketAddrs) -> Self {
        let sockets: Vec<_> = addrs.to_socket_addrs().unwrap().collect();
        let client = futures_lite::future::block_on(
            ComputeTaskPool::get()
                .spawn(async move { TcpStream::connect(&*sockets).await.unwrap() }),
        );

        Self { client }
    }

    pub fn start(&self, net: &mut NetworkNode) {
        let client = self.client.clone();
        let cancel_flag = net.cancel_flag.clone();
        let message_receiver = net.send_channel().receiver.clone_async();
        let error_sender = net.error_channel().sender.clone_async();
        IoTaskPool::get()
            .spawn(async move {
                Self::send_loop(
                    client,
                    message_receiver,
                    error_sender.clone(),
                    cancel_flag.clone(),
                )
                .await;
            })
            .detach()
    }

    async fn send_loop(
        mut client: TcpStream,
        message_receiver: AsyncReceiver<NetworkRawPacket>,
        error_sender: AsyncSender<NetworkError>,
        cancel_flag: Arc<AtomicBool>,
    ) {
        loop {
            if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

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
}

fn control_tcp_client(
    mut commands: Commands,
    mut q_tcp_client: Query<(Entity, &TcpClientNode), Added<TcpClientNode>>,
) {
    for (e, tcp_client) in q_tcp_client.iter_mut() {
        let mut net_node = NetworkNode::new(
            NetworkProtocol::TCP,
            tcp_client.client.clone().local_addr().unwrap(),
            None,
        );
        tcp_client.start(&mut net_node);
        commands.entity(e).insert(net_node);
    }
}

fn control_tcp_server(
    mut commands: Commands,
    mut q_tcp_server: Query<(Entity, &TcpServerNode), Added<TcpServerNode>>,
) {
    for (e, tcp_server) in q_tcp_server.iter() {
        let mut net_node = NetworkNode::new(
            NetworkProtocol::TCP,
            tcp_server.listener.clone().unwrap().local_addr().unwrap(),
            None,
        );
        tcp_server.start(&mut net_node);
        commands.entity(e).insert(net_node);
    }
}

fn handle_new_connection(mut q_tcp_server: Query<(Entity, &mut TcpServerNode, &mut NetworkNode)>) {
    for (e, tcp_server, net_node) in q_tcp_server.iter_mut() {
        while let Ok(Some(tcp_stream)) = tcp_server.new_connections.receiver.try_recv() {
            debug!(
                "new Tcp client {:?} connected",
                tcp_stream.local_addr().unwrap()
            );
            let cancel_flag = net_node.cancel_flag.clone();
            let recv_sender = net_node.recv_channel().sender.clone_async();
            let error_sender = net_node.error_channel().sender.clone_async();

            IoTaskPool::get()
                .spawn(async move {
                    TcpServerNode::recv_loop(tcp_stream,
                                             recv_sender,
                                             error_sender.clone(),
                                             cancel_flag.clone(),
                                             65_507,
                    ).await;
                })
                .detach();
        }
    }
}
