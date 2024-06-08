use std::fmt::Display;
use std::net::ToSocketAddrs;

use bevy::prelude::{Added, Component, Or, Query};
use bytes::Bytes;

use crate::error::NetworkError;
use crate::network::{ConnectTo, ListenTo, NetworkRawPacket};
use crate::shared::{AsyncChannel, NetworkEvent};

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
    pub max_packet_size: usize,
    pub listen_to: Option<ListenTo>,
    pub connect_to: Option<ConnectTo>,
}

impl NetworkNode {
    pub fn start(&mut self) {
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn send(&self, bytes: &[u8]) {
        match self.connect_to.as_ref() {
            None => {
                println!("send error");
                self.event_channel
                    .sender
                    .try_send(NetworkEvent::Error(NetworkError::SendError))
                    .expect("Error channel has closed");
            }
            Some(connect_to) => {
                let addr = connect_to.to_string();
                self.send_message_channel
                    .sender
                    .try_send(NetworkRawPacket {
                        addr,
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
                addr: remote_addr.to_string(),
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
    }

    pub fn schema(&self) -> String {
        if let Some(local_addr) = self.listen_to.as_ref() {
            local_addr.to_string()
        } else if let Some(connect_to) = self.connect_to.as_ref() {
            connect_to.to_string()
        } else {
            "".to_string()
        }
    }
}

impl Display for NetworkNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.schema())
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn update_network_node(
    mut q_net: Query<
        (&mut NetworkNode, Option<&ListenTo>, Option<&ConnectTo>),
        Or<(Added<NetworkNode>, Added<NetworkNode>)>,
    >,
) {
    for (mut net_node, opt_listen_to, opt_connect_to) in q_net.iter_mut() {
        if let Some(listen_to) = opt_listen_to {
            if net_node.listen_to.is_none() {
                net_node.listen_to = Some(listen_to.clone());
            }
        }
        if let Some(connect_to) = opt_connect_to {
            if net_node.connect_to.is_none() {
                net_node.connect_to = Some(connect_to.clone());
            }
        }
    }
}
