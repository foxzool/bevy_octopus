use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
};

use bevy::{
    ecs::world::CommandQueue,
    hierarchy::DespawnRecursiveExt,
    prelude::{
        Added, Bundle, Commands, Component, Deref, Entity, Event, EventWriter, Or, Query, Reflect,
        ResMut, Resource,
    },
    tasks::block_on,
};
use bytes::Bytes;
use kanal::{unbounded, Receiver, Sender};
use url::Url;

use crate::{error::NetworkError, prelude::ChannelId};

/// [`NetworkRawPacket`]s are raw packets that are sent over the network.
#[derive(Clone)]
pub struct NetworkRawPacket {
    pub addr: String,
    pub bytes: Bytes,
    pub text: Option<String>,
}

impl NetworkRawPacket {
    pub fn new(addr: impl ToString, bytes: Bytes) -> NetworkRawPacket {
        NetworkRawPacket {
            addr: addr.to_string(),
            bytes,
            text: None,
        }
    }
}

impl Debug for NetworkRawPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkRawPacket")
            .field("addr", &self.addr)
            .field("len", &self.bytes.len())
            .finish()
    }
}

#[derive(Component, Deref, Clone, Debug)]
pub struct ListenTo(pub Url);

impl ListenTo {
    pub fn new(url_str: &str) -> Self {
        let url = Url::parse(url_str).expect("url format error");
        Self(url)
    }

    pub fn local_addr(&self) -> SocketAddr {
        let url_str = self.0.to_string();
        let arr: Vec<&str> = url_str.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];
        s.to_socket_addrs().unwrap().next().unwrap()
    }
}

#[derive(Component, Deref, Clone, Debug)]
pub struct ConnectTo(pub Url);

impl ConnectTo {
    pub fn new(url_str: &str) -> Self {
        let url = Url::parse(url_str).expect("url format error");
        Self(url)
    }

    pub fn peer_addr(&self) -> SocketAddr {
        let url_str = self.0.to_string();
        let arr: Vec<&str> = url_str.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];
        s.to_socket_addrs().unwrap().next().unwrap()
    }
}

#[derive(Bundle)]
pub struct NetworkBundle {
    pub channel_id: ChannelId,
    pub network_node: NetworkNode,
}

impl NetworkBundle {
    pub fn new(channel_id: ChannelId) -> Self {
        Self {
            channel_id,
            network_node: NetworkNode::default(),
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
                let _ =
                    self.event_channel
                        .sender
                        .try_send(NetworkEvent::Error(NetworkError::Custom(
                            "No connection".to_string(),
                        )));
            }
            Some(connect_to) => {
                let addr = connect_to.to_string();
                let _ = self
                    .send_message_channel
                    .sender
                    .try_send(NetworkRawPacket::new(addr, Bytes::copy_from_slice(bytes)));
            }
        }
    }

    /// Send text message
    pub fn send_text(&self, text: String) {
        match self.connect_to.as_ref() {
            None => {
                let _ =
                    self.event_channel
                        .sender
                        .try_send(NetworkEvent::Error(NetworkError::Custom(
                            "No connection".to_string(),
                        )));
            }
            Some(connect_to) => {
                let addr = connect_to.to_string();
                let _ = self.send_message_channel.sender.try_send(NetworkRawPacket {
                    addr,
                    bytes: Bytes::new(),
                    text: Some(text),
                });
            }
        }
    }

    pub fn send_to(&self, bytes: &[u8], addr: impl ToString) {
        let _ = self
            .send_message_channel
            .sender
            .try_send(NetworkRawPacket::new(addr, Bytes::copy_from_slice(bytes)));
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

/// A network peer on server
#[derive(Component)]
pub struct NetworkPeer;

#[derive(Resource, Default)]
pub(crate) struct CommandQueueTasks {
    pub tasks: AsyncChannel<CommandQueue>,
}

pub(crate) fn handle_command_queue_tasks(task: ResMut<CommandQueueTasks>, mut commands: Commands) {
    while let Ok(Some(mut commands_queue)) = task.tasks.receiver.try_recv() {
        block_on(async {
            commands.append(&mut commands_queue);
        });
    }
}

#[derive(Reflect, Clone)]
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
pub struct NetworkNodeEvent {
    pub node: Entity,
    pub channel_id: ChannelId,
    pub event: NetworkEvent,
}

#[derive(Debug, Event)]
/// A network event originating from a network node
pub enum NetworkEvent {
    Listen,
    Connected,
    Disconnected,
    Error(NetworkError),
}

/// send network node error channel to events
pub(crate) fn network_node_event(
    mut commands: Commands,
    mut q_net: Query<(Entity, &ChannelId, &mut NetworkNode, Option<&ConnectTo>)>,
    mut node_events: EventWriter<NetworkNodeEvent>,
) {
    for (entity, channel_id, net_node, opt_connect_to) in q_net.iter_mut() {
        while let Ok(Some(event)) = net_node.event_channel.receiver.try_recv() {
            match event {
                NetworkEvent::Listen => {}
                NetworkEvent::Connected => {}
                NetworkEvent::Disconnected => {
                    if let Some(connect_to) = opt_connect_to {
                        commands
                            .entity(entity)
                            .remove::<ConnectTo>()
                            .insert(connect_to.clone());
                    } else {
                        commands.entity(entity).despawn_recursive();
                    }
                }
                NetworkEvent::Error(ref network_error) => {
                    if let NetworkError::Connection(_) = network_error {
                        if let Some(connect_to) = opt_connect_to {
                            commands
                                .entity(entity)
                                .remove::<ConnectTo>()
                                .insert(connect_to.clone());
                        } else {
                            commands.entity(entity).despawn_recursive();
                        }
                    }
                }
            }
            node_events.send(NetworkNodeEvent {
                node: entity,
                channel_id: *channel_id,
                event,
            });
        }
    }
}
