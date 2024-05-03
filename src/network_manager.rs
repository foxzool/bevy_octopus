use std::fmt::Display;
use std::net::{SocketAddr, ToSocketAddrs};

use bevy::prelude::Component;
use bytes::Bytes;

use crate::error::NetworkError;
use crate::network::NetworkRawPacket;
use crate::shared::{AsyncChannel, NetworkEvent, NetworkProtocol};

#[derive(Component)]
pub struct NetworkNode {
    /// Channel for receiving messages
    pub recv_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for sending messages for peer
    pub send_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for events
    pub event_channel: AsyncChannel<NetworkEvent>,
    /// Channel for shutdown
    pub shutdown_channel: AsyncChannel<()>,
    /// Whether the node is running or not
    pub running: bool,
    /// Local address
    pub local_addr: Option<SocketAddr>,
    pub peer_addr: Option<SocketAddr>,
    pub max_packet_size: usize,
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
            event_channel: AsyncChannel::new(),
            shutdown_channel: AsyncChannel::new(),
            running: false,
            local_addr,
            peer_addr,
            max_packet_size: 65535,
            protocol,
        }
    }
    pub fn start(&mut self) {
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn send(&self, bytes: &[u8]) {
        match self.peer_addr {
            None => {
                self.event_channel
                    .sender
                    .try_send(NetworkEvent::Error(NetworkError::SendError))
                    .expect("Error channel has closed");
            }
            Some(remote_addr) => {
                self.send_message_channel
                    .sender
                    .try_send(NetworkRawPacket {
                        addr: remote_addr,
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
                addr: remote_addr,
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
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
