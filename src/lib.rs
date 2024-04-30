#![doc = include_str!("../README.md")]
// #![warn(missing_docs)]

use std::fmt::{Debug, Display};

use bevy::app::{App, Plugin, Update};
use bevy::hierarchy::DespawnRecursiveExt;
use bevy::prelude::{Commands, Entity, EventWriter, Query};
use bevy::reflect::Reflect;
use kanal::{unbounded, Receiver, Sender};

use crate::network_manager::NetworkNode;
use crate::prelude::NetworkEvent;
use crate::shared::NetworkNodeEvent;
use crate::shared::{AsyncRuntime, NetworkProtocol};

pub mod decoder;
pub mod error;
pub mod network;
pub mod network_manager;
pub mod prelude;

pub mod shared;

#[cfg(feature = "udp")]
pub mod udp;

#[cfg(feature = "tcp")]
pub mod tcp;

pub type ChannelName = String;

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

impl<T> AsyncChannel<T> {
    fn new() -> Self {
        let (sender, receiver) = unbounded();

        Self { sender, receiver }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
/// A [`ConnectionId`] denotes a single connection
pub struct ConnectionId {
    /// The key of the connection.
    pub id: u32,
}

impl Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Connection with ID={0}", self.id))
    }
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
