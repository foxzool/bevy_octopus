use bevy::prelude::*;
use kanal::{unbounded, Receiver, Sender};

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
