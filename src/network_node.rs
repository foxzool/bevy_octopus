use std::fmt::Display;
use std::net::{SocketAddr, ToSocketAddrs};

use bevy::prelude::{Added, Component, Query};
use bytes::Bytes;

use crate::error::NetworkError;
use crate::network::{LocalSocket, NetworkRawPacket, RemoteSocket};
use crate::shared::{AsyncChannel, NetworkEvent, NetworkProtocol};

#[derive(Component, Default)]
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

pub(crate) fn update_network_node(
    mut q_net: Query<
        (
            &mut NetworkNode,
            &NetworkProtocol,
            Option<&LocalSocket>,
            Option<&RemoteSocket>,
        ),
        Added<NetworkNode>,
    >,
) {
    for (mut net_node, protocol, opt_local_socket, opt_remote_socket) in q_net.iter_mut() {
        net_node.protocol = *protocol;
        if let Some(local_socket) = opt_local_socket {
            if net_node.local_addr.is_none() {
                net_node.local_addr = Some(local_socket.0);
            }
        }
        if let Some(remote_socket) = opt_remote_socket {
            if net_node.peer_addr.is_none() {
                net_node.peer_addr = Some(remote_socket.0);
            }
        }
    }
}
