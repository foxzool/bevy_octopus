use crate::client::Client;
use std::{
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs},
};

use bevy::{
    ecs::component::{ComponentHooks, StorageType},
    prelude::*,
};
use bytes::Bytes;
use kanal::{unbounded, Receiver, Sender};
use url::Url;

use crate::{error::NetworkError, prelude::ChannelId};

pub trait NetworkAddress: Debug + Clone + Send + Sync {
    fn to_string(&self) -> String;
    fn from_string(s: &str) -> Result<Self, String>
    where
        Self: Sized;
}

pub trait NetworkAddressRegister {
    fn register_network_address<T: NetworkAddress + 'static>(&mut self) -> &mut Self;
}

impl NetworkAddressRegister for App {
    fn register_network_address<T: NetworkAddress + 'static>(&mut self) -> &mut Self {
        self.add_systems(Update, handle_reconnect_timer::<T>);

        self
    }
}

#[derive(Deref)]
pub struct Server<T: NetworkAddress>(pub T);

impl<T: NetworkAddress + 'static> Component for Server<T> {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, targeted_entity, _component_id| {
            let server_addr = world.get::<Server<T>>(targeted_entity).unwrap();
            world.trigger_targets(ListenTo::new(&server_addr.0.to_string()), targeted_entity);
        });
    }
}

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
pub struct ListenTo(pub String);

impl ListenTo {
    pub fn new(url_str: &str) -> Self {
        Self(url_str.to_string())
    }

    pub fn local_addr(&self) -> SocketAddr {
        let url_str = self.0.to_string();
        let arr: Vec<&str> = url_str.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];
        s.to_socket_addrs().unwrap().next().unwrap()
    }
}

#[derive(Event, Deref, Clone, Debug)]
pub struct ConnectTo(pub String);

impl ConnectTo {
    pub fn new(url_str: &str) -> Self {
        Self(url_str.to_string())
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
            world.trigger_targets(ListenTo(server_addr.0.to_string()), targeted_entity);
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
            world.trigger_targets(ConnectTo(remote_addr.0.to_string()), targeted_entity);
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

pub(crate) fn client_reconnect(
    trigger: Trigger<NetworkEvent>,
    mut commands: Commands,
    mut q_net: Query<&mut ReconnectSetting, Without<NetworkPeer>>,
) {
    if let Ok(mut reconnect) = q_net.get_mut(trigger.entity()) {
        let event = trigger.event();
        if reconnect.retries < reconnect.max_retries {
            reconnect.retries += 1;
        } else {
            return;
        }
        match event {
            NetworkEvent::Listen | NetworkEvent::Connected => reconnect.retries = 0,
            NetworkEvent::Disconnected | NetworkEvent::Error(NetworkError::Connection(_)) => {
                commands
                    .entity(trigger.entity())
                    .insert(ReconnectTimer(Timer::from_seconds(
                        reconnect.delay,
                        TimerMode::Once,
                    )));
            }
            _ => {}
        }
    }
}

#[derive(Component, Deref, DerefMut)]
pub struct ReconnectTimer(pub Timer);

pub(crate) fn handle_reconnect_timer<T: NetworkAddress + 'static>(
    mut commands: Commands,
    time: Res<Time>,
    mut q_reconnect: Query<(Entity, &Client<T>, &mut ReconnectTimer)>,
) {
    for (entity, remote_addr, mut timer) in q_reconnect.iter_mut() {
        if timer.tick(time.delta()).just_finished() {
            commands.entity(entity).remove::<ReconnectTimer>();
            commands.trigger_targets(ConnectTo(remote_addr.0.to_string()), entity);
        }
    }
}

pub(crate) fn cleanup_client_session(
    trigger: Trigger<NetworkEvent>,
    mut commands: Commands,
    q_net: Query<Entity, With<NetworkPeer>>,
) {
    if let Ok(entity) = q_net.get(trigger.entity()) {
        let event = trigger.event();

        match event {
            NetworkEvent::Disconnected | NetworkEvent::Error(NetworkError::Connection(_)) => {
                commands.entity(entity).despawn_recursive();
            }
            _ => {}
        }
    }
}
