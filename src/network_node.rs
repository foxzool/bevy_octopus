use std::{
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs},
};

use bevy::prelude::*;
use bytes::Bytes;
use kanal::{unbounded, Receiver, Sender};

use crate::{error::NetworkError, prelude::ChannelId};

pub trait NetworkAddress: Debug + Clone + Send + Sync {
    fn to_string(&self) -> String;
    fn from_string(s: &str) -> Result<Self, String>
    where
        Self: Sized;
}

/// [`NetworkRawPacket`]s are raw packets that are sent over the network.
#[derive(Clone)]
pub struct NetworkRawPacket {
    pub addr: Option<SocketAddr>,
    pub bytes: Bytes,
    pub text: Option<String>,
}

impl Debug for NetworkRawPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkRawPacket")
            .field("addr", &self.addr)
            .field("len", &self.bytes.len())
            .finish()
    }
}

#[derive(Bundle)]
pub struct NetworkBundle {
    pub channel_id: ChannelId,
    pub network_node: NetworkNode,
    pub reconnect: ReconnectSetting,
}

impl NetworkBundle {
    pub fn new(channel_id: ChannelId) -> Self {
        Self {
            channel_id,
            network_node: NetworkNode::default(),
            reconnect: ReconnectSetting::default(),
        }
    }
}

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
}

impl NetworkNode {
    pub fn start(&mut self) {
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Send text message
    pub fn send_text_to(&self, text: String, remote_addr: impl ToSocketAddrs) {
        let addr = remote_addr.to_socket_addrs().unwrap().next().unwrap();
        let _ = self.send_message_channel.sender.try_send(NetworkRawPacket {
            addr: Some(addr),
            bytes: Bytes::new(),
            text: Some(text),
        });
    }

    pub fn send_bytes_to(&self, bytes: &[u8], addr: impl ToSocketAddrs) {
        let _ = self.send_message_channel.sender.try_send(NetworkRawPacket {
            addr: Some(addr.to_socket_addrs().unwrap().next().unwrap()),
            bytes: Bytes::copy_from_slice(bytes),
            text: None,
        });
    }

    pub fn send_bytes(&self, bytes: &[u8]) {
        let _ = self.send_message_channel.sender.try_send(NetworkRawPacket {
            addr: None,
            bytes: Bytes::copy_from_slice(bytes),
            text: None,
        });
    }
}

/// A network peer on server
#[derive(Component)]
pub struct NetworkPeer;

#[derive(Reflect, Debug, Clone)]
pub struct AsyncChannel<T> {
    pub sender: Sender<T>,
    pub receiver: Receiver<T>,
}

impl<T> Default for AsyncChannel<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AsyncChannel<T> {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();

        Self { sender, receiver }
    }
}

#[derive(Debug, Event)]
/// A network event originating from a network node
pub enum NetworkEvent {
    Listen,
    Connected,
    Disconnected,
    Error(NetworkError),
}

#[derive(Debug, Component)]
pub struct ReconnectSetting {
    /// Delay in seconds
    pub delay: f32,
    pub max_retries: usize,
    pub retries: usize,
}

impl Default for ReconnectSetting {
    fn default() -> Self {
        Self {
            delay: 2.0,
            max_retries: usize::MAX,
            retries: 0,
        }
    }
}

/// send network node error channel to events
#[allow(clippy::type_complexity)]
pub(crate) fn network_node_event(
    mut commands: Commands,
    mut q_net: Query<(Entity, &mut NetworkNode)>,
) {
    for (entity, mut net_node) in q_net.iter_mut() {
        while let Ok(Some(event)) = net_node.event_channel.receiver.try_recv() {
            match event {
                NetworkEvent::Listen | NetworkEvent::Connected => {
                    net_node.start();
                }
                NetworkEvent::Disconnected | NetworkEvent::Error(_) => {
                    net_node.stop();
                }
            }
            commands.trigger_targets(event, vec![entity]);
        }
    }
}
