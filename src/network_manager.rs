use std::fmt::Display;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use bevy::prelude::Component;
use bytes::Bytes;

use crate::error::NetworkError;
use crate::network::NetworkRawPacket;
use crate::shared::NetworkProtocol;
use crate::AsyncChannel;

#[derive(Component)]
pub struct NetworkNode {
    /// Channel for receiving messages
    pub recv_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for sending messages for peer
    pub send_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for broadcasting messages
    pub broadcast_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for errors
    pub error_channel: AsyncChannel<NetworkError>,
    pub shutdown_channel: AsyncChannel<()>,
    /// A flag to cancel the node
    pub cancel_flag: Arc<AtomicBool>,
    /// Whether the node is running or not
    pub running: bool,
    /// Local address
    pub local_addr: Option<SocketAddr>,
    pub peer_addr: Option<SocketAddr>,
    pub max_packet_size: usize,
    pub auto_start: bool,
    protocol: NetworkProtocol,
}

impl NetworkNode {
    pub fn new(
        protocol: NetworkProtocol,
        local_addr: Option<SocketAddr>,
        peer_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            recv_message_channel: AsyncChannel::new(),
            send_message_channel: AsyncChannel::new(),
            broadcast_message_channel: AsyncChannel::new(),
            error_channel: AsyncChannel::new(),
            shutdown_channel: AsyncChannel::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            running: false,
            local_addr,
            peer_addr,
            max_packet_size: 65535,
            auto_start: true,
            protocol,
        }
    }
    pub fn start(&mut self) {
        self.cancel_flag
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.running = false;
    }

    pub fn send(&self, bytes: &[u8]) {
        match self.peer_addr {
            None => {
                // self.error_channel
                //     .sender
                //     .try_send(NetworkError::NoPeer)
                //     .expect("Error channel has closed");
            }
            Some(remote_addr) => {
                self.send_message_channel
                    .sender
                    .try_send(NetworkRawPacket {
                        socket: remote_addr,
                        bytes: Bytes::copy_from_slice(bytes),
                    })
                    .expect("Message channel has closed.");
            }
        }
    }

    pub fn send_to(&self, bytes: &[u8], addr: impl ToSocketAddrs) {
        let remote_addr = addr.to_socket_addrs().unwrap().next().unwrap();
        self.send_message_channel
            .sender
            .try_send(NetworkRawPacket {
                socket: remote_addr,
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
    }

    pub fn broadcast(&self, bytes: &[u8]) {
        self.broadcast_message_channel
            .sender
            .try_send(NetworkRawPacket {
                socket: self.local_addr.unwrap(),
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
    }

    pub fn recv_channel(&self) -> &AsyncChannel<NetworkRawPacket> {
        &self.recv_message_channel
    }

    pub fn send_channel(&self) -> &AsyncChannel<NetworkRawPacket> {
        &self.send_message_channel
    }

    pub fn error_channel(&self) -> &AsyncChannel<NetworkError> {
        &self.error_channel
    }

    pub fn broadcast_channel(&self) -> &AsyncChannel<NetworkRawPacket> {
        &self.broadcast_message_channel
    }

    pub fn schema(&self) -> String {
        if let Some(local_addr) = self.local_addr {
            format!(
                "{}://{}:{}",
                self.protocol,
                local_addr.ip(),
                local_addr.port()
            )
        } else if let Some(peer_addr) = self.peer_addr {
            format!(
                "{}://{}:{}",
                self.protocol,
                peer_addr.ip(),
                peer_addr.port()
            )
        } else {
            format!("{}://", self.protocol)
        }
    }
}

impl Display for NetworkNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.schema())
    }
}
