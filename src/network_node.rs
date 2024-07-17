use std::{
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs},
};

use bevy::{
    ecs::{
        component::{ComponentHooks, StorageType},
        world::CommandQueue,
    },
    hierarchy::DespawnRecursiveExt,
    prelude::{
        Bundle, Commands, Component, Deref, Entity, Event, EventWriter, Query, Reflect, ResMut,
        Resource,
    },
    tasks::block_on,
};
use bytes::Bytes;
use kanal::{Receiver, Sender, unbounded};
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

#[derive(Event, Deref, Clone, Debug)]
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

#[derive(Event, Deref, Clone, Debug)]
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
}

impl NetworkNode {
    pub fn start(&mut self) {
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Send text message
    pub fn send_text_to(&self, text: String, remote_addr: impl ToString) {
        let addr = remote_addr.to_string();
        let _ = self.send_message_channel.sender.try_send(NetworkRawPacket {
            addr,
            bytes: Bytes::new(),
            text: Some(text),
        });
    }

    pub fn send_bytes_to(&self, bytes: &[u8], addr: impl ToString) {
        let _ = self
            .send_message_channel
            .sender
            .try_send(NetworkRawPacket::new(addr, Bytes::copy_from_slice(bytes)));
    }
}

#[derive(Debug, Deref)]
pub struct ServerAddr(pub Url);

impl Component for ServerAddr {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, targeted_entity, _component_id| {
            let server_addr = world.get::<ServerAddr>(targeted_entity).unwrap();
            world.trigger_targets(ListenTo(server_addr.0.clone()), targeted_entity);
        });
    }
}

impl ServerAddr {
    pub fn new(addr: impl ToString) -> Self {
        Self(addr.to_string().parse().unwrap())
    }

    pub fn local_addr(&self) -> SocketAddr {
        let url_str = self.0.to_string();
        let arr: Vec<&str> = url_str.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];
        s.to_socket_addrs().unwrap().next().unwrap()
    }
}

#[derive(Debug, Deref)]
pub struct RemoteAddr(pub Url);

impl Component for RemoteAddr {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, targeted_entity, _component_id| {
            let remote_addr = world.get::<RemoteAddr>(targeted_entity).unwrap();
            world.trigger_targets(ConnectTo(remote_addr.0.clone()), targeted_entity);
        });
    }
}

impl RemoteAddr {
    pub fn new(addr: impl ToString) -> Self {
        Self(addr.to_string().parse().unwrap())
    }

    pub fn peer_addr(&self) -> SocketAddr {
        let url_str = self.0.to_string();
        let arr: Vec<&str> = url_str.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];
        s.to_socket_addrs().unwrap().next().unwrap()
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

#[derive(Debug, Component)]
pub struct Reconnect {
    /// Delay in seconds
    pub delay: f64,
    pub max_retries: usize,
    pub retries: usize,
}

impl Default for Reconnect {
    fn default() -> Self {
        Self {
            delay: 1.0,
            max_retries: usize::MAX,
            retries: 0,
        }
    }
}

/// send network node error channel to events
pub(crate) fn network_node_event(
    mut commands: Commands,
    mut q_net: Query<(
        Entity,
        &ChannelId,
        &mut NetworkNode,
        Option<&RemoteAddr>,
        Option<&NetworkPeer>,
    )>,
    mut node_events: EventWriter<NetworkNodeEvent>,
) {
    for (entity, channel_id, net_node, opt_remote_addr, opt_network_peer) in q_net.iter_mut() {
        while let Ok(Some(event)) = net_node.event_channel.receiver.try_recv() {
            match event {
                NetworkEvent::Listen => {}
                NetworkEvent::Connected => {}
                NetworkEvent::Disconnected => {
                    if opt_network_peer.is_some() {
                        commands.entity(entity).despawn_recursive();
                    } else if let Some(remote_addr) = opt_remote_addr {
                        commands.trigger_targets(ConnectTo(remote_addr.0.clone()), entity);
                    } else {
                        commands.entity(entity).despawn_recursive();
                    }
                }
                NetworkEvent::Error(ref network_error) => {
                    if let NetworkError::Connection(_) = network_error {
                        if opt_network_peer.is_some() {
                            commands.entity(entity).despawn_recursive();
                        } else if let Some(remote_addr) = opt_remote_addr {
                            commands.trigger_targets(ConnectTo(remote_addr.0.clone()), entity);
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
