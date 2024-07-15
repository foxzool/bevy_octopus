use bevy::prelude::*;
use kanal::{unbounded, Receiver, Sender};

use crate::{
    error::NetworkError,
    network_node::NetworkNode,
    prelude::{ChannelId, ConnectTo},
};

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
