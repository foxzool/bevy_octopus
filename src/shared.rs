use std::fmt::Display;

use bevy::prelude::*;
use kanal::{Receiver, Sender, unbounded};
use tokio::runtime::Runtime;

use crate::error::NetworkError;
use crate::network_node::NetworkNode;

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

#[derive(Debug, Clone, Copy, Component, Ord, PartialOrd, Eq, PartialEq, Reflect, Default)]
pub enum NetworkProtocol {
    UDP,
    #[default]
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
pub(crate) fn network_node_event(
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
