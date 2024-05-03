use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;

use bevy::ecs::component::ComponentId;
use bevy::ecs::component::SparseStorage;
use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use serde::{Deserialize, Serialize};

#[cfg(feature = "bincode")]
pub use bincode::BincodeProvider;
#[cfg(feature = "serde_json")]
pub use serde_json::SerdeJsonProvider;

use crate::{
    error::NetworkError,
    network::{NetworkData, NetworkMessage},
};
use crate::channels::{ChannelId, ChannelMessage};
use crate::connections::NetworkPeer;
use crate::network::{NetworkRawPacket, RemoteSocket};
use crate::network_manager::NetworkNode;
use crate::shared::{NetworkEvent, NetworkNodeEvent};

#[cfg(feature = "bincode")]
mod bincode;

#[cfg(feature = "serde_json")]
mod serde_json;

///
pub trait Transformer:
'static + Send + Sync + Reflect + Resource + Default + GetTypeRegistration
{
    const NAME: &'static str;
    fn encode<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, NetworkError>;
    fn decode<T: for<'a> Deserialize<'a>>(&self, bytes: &[u8]) -> Result<T, NetworkError>;

    fn marker(&self) -> impl Component;
}

pub trait NetworkMessageTransformer {
    fn register_channel_transformer<M: NetworkMessage, T: Transformer>(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self;
}

impl NetworkMessageTransformer for App {
    fn register_channel_transformer<M: NetworkMessage, T: Transformer>(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self {
        debug!(
            "Registering {} transformer for  {}  in {}",
            // T::NAME,
            "T",
            M::NAME,
            channel_id
        );
        if self.world.get_resource::<T>().is_none() {
            self.world.init_resource::<T>();
        }

        self.register_type::<T>();
        {
            if self
                .world
                .get_resource_ref::<ChannelTransformers>()
                .is_none()
            {
                self.world.insert_resource(ChannelTransformers::default());
            }
            let type_registry = self
                .world
                .get_resource::<AppTypeRegistry>()
                .unwrap()
                .clone();
            let type_registry = type_registry.read();
            let registration = type_registry.get(TypeId::of::<T>()).unwrap();
            self.world
                .resource_mut::<ChannelTransformers>()
                .0
                .insert(channel_id, registration.type_id());
        }

        self.add_event::<NetworkData<M>>();
        self.add_event::<ChannelMessage<M>>();

        self.add_systems(PreUpdate, decode_system::<M, T>);
        self.add_systems(PostUpdate, encode_system::<M, T>);
        self
    }
}

#[derive(Resource, Deref, DerefMut, Debug)]
pub struct ChannelTransformers(pub HashMap<ChannelId, TypeId>);

impl Default for ChannelTransformers {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

fn encode_system<M: NetworkMessage, T: Transformer + bevy::prelude::Resource>(
    type_registry: Res<AppTypeRegistry>,
    mut message_ev: EventReader<ChannelMessage<M>>,
    transformer: Res<T>,
    transformer_index: Res<ChannelTransformers>,
    query: Query<(&ChannelId, &NetworkNode, &RemoteSocket), With<NetworkPeer>>,
) {
    for message in message_ev.read() {
        for (channel_id, net_node, remote_socket) in query.iter() {
            let type_registry = type_registry.read();
            let registration = type_registry.get(TypeId::of::<T>()).unwrap();
            if registration.type_id() == *transformer_index.0.get(channel_id).unwrap() {
                match transformer.encode(&message.message) {
                    Ok(bytes) => net_node
                        .send_message_channel
                        .sender
                        .send(NetworkRawPacket {
                            addr: **remote_socket,
                            bytes: bytes.into(),
                        })
                        .expect("send channel has closed"),
                    Err(_) => {}
                }
            }
        }
    }
}

fn decode_system<M: NetworkMessage, T: Transformer + bevy::prelude::Resource>(
    type_registry: Res<AppTypeRegistry>,
    mut data_events: EventWriter<NetworkData<M>>,
    mut node_events: EventWriter<NetworkNodeEvent>,
    transformer: Res<T>,
    transformer_index: Res<ChannelTransformers>,
    query: Query<(Entity, &ChannelId, &NetworkNode)>,
) {
    for (entity, channel_id, network_node) in query.iter() {
        match transformer_index.0.get(channel_id) {
            Some(type_id) => {
                let type_registry = type_registry.read();
                let registration = type_registry.get(TypeId::of::<T>()).unwrap();

                if registration.type_id() == *type_id {
                    decode_packets::<M, T>(
                        entity,
                        network_node,
                        transformer.deref(),
                        &mut data_events,
                        &mut node_events,
                    );
                }
            }
            None => {
                continue;
            }
        }
    }
}

fn decode_packets<M: NetworkMessage, T: Transformer>(
    entity: Entity,
    network_node: &NetworkNode,
    transformer: &T,
    data_events: &mut EventWriter<NetworkData<M>>,
    node_events: &mut EventWriter<NetworkNodeEvent>,
) {
    let mut packets = vec![];
    while let Ok(Some(packet)) = network_node.recv_message_channel.receiver.try_recv() {
        packets.push(packet.bytes);
    }

    if !packets.is_empty() {
        debug!(
            "{} Decoding {} packets for {}",
            T::NAME,
            packets.len(),
            M::NAME
        );

        let (messages, errors): (Vec<_>, Vec<_>) = packets
            .into_iter()
            .map(|msg| transformer.decode::<M>(&msg))
            .partition(Result::is_ok);

        data_events.send_batch(
            messages
                .into_iter()
                .map(Result::unwrap)
                .map(|m| NetworkData::new(entity, m))
                .collect::<Vec<_>>(),
        );
        node_events.send_batch(
            errors
                .into_iter()
                .map(Result::unwrap_err)
                .map(|error| NetworkNodeEvent {
                    node: entity,
                    event: NetworkEvent::Error(error),
                })
                .collect::<Vec<_>>(),
        );
    }
}

fn spawn_child_worker<D: Transformer>(
    mut commands: Commands,
    // q_parent: Query<&CodingWorker<T, D>, With<NetworkNode>>,
    q_child: Query<(Entity, &Parent), Added<NetworkNode>>,
) {
    for (entity, parent) in q_child.iter() {
        // if let Ok(worker) = q_parent.get(parent.get()) {
        //     commands.entity(entity).insert(*worker.clone());
        // }
    }
}
