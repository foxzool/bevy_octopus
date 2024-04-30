use std::fmt::Display;

use bevy::prelude::*;
use kanal::{Receiver, Sender, unbounded};
use tokio::runtime::Runtime;

use crate::{tcp, udp};
use crate::error::NetworkError;
use crate::network_manager::NetworkNode;

pub struct BevyNetPlugin;

impl Plugin for BevyNetPlugin {
    fn build(&self, app: &mut App) {
        let async_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        app.register_type::<NetworkProtocol>()
            .insert_resource(AsyncRuntime(async_runtime))
            .add_event::<NetworkNodeEvent>()
            .add_systems(Update, network_node_event);

        #[cfg(feature = "udp")]
        app.add_plugins(udp::UdpPlugin);

        #[cfg(feature = "tcp")]
        app.add_plugins(tcp::TcpPlugin);
    }
}

#[derive(Reflect)]
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

#[derive(Resource, Deref, DerefMut)]
pub struct AsyncRuntime(pub(crate) Runtime);

#[derive(Resource, Deref, DerefMut)]
pub struct RuntimeHandle(pub(crate) tokio::runtime::Handle);

#[derive(Debug, Component, Ord, PartialOrd, Eq, PartialEq, Reflect)]
pub enum NetworkProtocol {
    UDP,
    TCP,
    SSL,
    WS,
    WSS,
}

impl Display for NetworkProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                NetworkProtocol::UDP => "udp",
                NetworkProtocol::TCP => "tcp",
                NetworkProtocol::SSL => "ssl",
                NetworkProtocol::WS => "ws",
                NetworkProtocol::WSS => "wss",
            }
        )
    }
}

#[derive(Debug, Event)]
pub struct NetworkNodeEvent {
    pub node: Entity,
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
fn network_node_event(
    mut commands: Commands,
    mut q_net: Query<(Entity, &mut NetworkNode)>,
    mut node_events: EventWriter<NetworkNodeEvent>,
) {
    for (entity, net_node) in q_net.iter_mut() {
        while let Ok(Some(event)) = net_node.event_channel.receiver.try_recv() {
            match event {
                NetworkEvent::Listen => {}
                NetworkEvent::Connected => {}
                NetworkEvent::Disconnected => {
                    commands.entity(entity).despawn_recursive();
                }
                NetworkEvent::Error(_) => {}
            }
            node_events.send(NetworkNodeEvent {
                node: entity,
                event,
            });
        }
    }
}
