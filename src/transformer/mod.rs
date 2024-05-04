use std::any::TypeId;
use std::collections::HashMap;
use std::ops::Deref;

use bevy::prelude::*;
use bevy::reflect::GetTypeRegistration;
use serde::{Deserialize, Serialize};

#[cfg(feature = "bincode")]
pub use bincode::BincodeTransformer;
#[cfg(feature = "serde_json")]
pub use serde_json::JsonTransformer;

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
}

pub trait NetworkMessageTransformer {
    fn add_transformer<M: NetworkMessage, T: Transformer>(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self;
}

impl NetworkMessageTransformer for App {
    fn add_transformer<M: NetworkMessage, T: Transformer>(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self {
        debug!(
            "Registering {} transformer for  {}  in {}",
            T::NAME,
            M::NAME,
            channel_id
        );
        if self.world.get_resource::<T>().is_none() {
            self.world.init_resource::<T>();
        }

        self.register_type::<T>();

        if self
            .world
            .get_resource_ref::<ChannelTransformers>()
            .is_none()
        {
            self.world.insert_resource(ChannelTransformers::default());
        }

        debug!(
            "Inserting {} into channel transformers {} {:?}",
            channel_id,
            T::NAME,
            TypeId::of::<T>()
        );
        self.world
            .resource_mut::<ChannelTransformers>()
            .0
            .insert(channel_id, TypeId::of::<T>());

        self.add_event::<NetworkData<M>>();
        self.add_event::<ChannelMessage<M>>();

        self.add_systems(PreUpdate, decode_system::<M, T>);
        self.add_systems(PostUpdate, encode_system::<M, T>);
        self.add_systems(PostUpdate, spawn_marker::<T>);
        self
    }
}

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub struct ChannelTransformers(pub HashMap<ChannelId, TypeId>);

#[derive(Component)]
pub struct ChannelTransformerMarker {
    pub channel_id: ChannelId,
    pub transformer_id: TypeId,
}

fn encode_system<M: NetworkMessage, T: Transformer + bevy::prelude::Resource>(
    mut message_ev: EventReader<ChannelMessage<M>>,
    transformer: Res<T>,
    query: Query<
        (
            &ChannelId,
            &NetworkNode,
            &RemoteSocket,
            &ChannelTransformerMarker,
        ),
        With<NetworkPeer>,
    >,
) {
    for message in message_ev.read() {
        for (channel_id, net_node, remote_socket, channel_marker) in query.iter() {
            if channel_marker.channel_id != *channel_id
                || channel_marker.transformer_id != TypeId::of::<T>()
            {
                continue;
            }
            trace!(
                "{} {} Encoding message for {}",
                channel_id,
                T::NAME,
                M::NAME
            );
            match transformer.encode(&message.message) {
                Ok(bytes) => net_node
                    .send_message_channel
                    .sender
                    .send(NetworkRawPacket {
                        addr: **remote_socket,
                        bytes: bytes.into(),
                    })
                    .expect("send channel has closed"),
                Err(e) => {
                    net_node
                        .event_channel
                        .sender
                        .send(NetworkEvent::Error(NetworkError::SerializeError(
                            e.to_string(),
                        )))
                        .expect("event channel has closed");
                }
            }
        }
    }
}

fn decode_system<M: NetworkMessage, T: Transformer + bevy::prelude::Resource>(
    mut data_events: EventWriter<NetworkData<M>>,
    mut node_events: EventWriter<NetworkNodeEvent>,
    transformer: Res<T>,
    query: Query<(Entity, &ChannelId, &NetworkNode, &ChannelTransformerMarker)>,
) {
    for (entity, channel_id, network_node, channel_marker) in query.iter() {
        if channel_marker.channel_id != *channel_id
            || channel_marker.transformer_id != TypeId::of::<T>()
        {
            continue;
        }

        decode_packets::<M, T>(
            entity,
            network_node,
            transformer.deref(),
            &mut data_events,
            &mut node_events,
        );
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

fn spawn_marker<T: Transformer>(
    mut commands: Commands,
    transformer_index: Res<ChannelTransformers>,
    q_channel: Query<(Entity, &ChannelId), Added<ChannelId>>,
) {
    for (entity, channel_id) in q_channel.iter() {
        if let Some(transformer_id) = transformer_index.0.get(channel_id) {
            if *transformer_id == TypeId::of::<T>() {
                commands.entity(entity).insert(ChannelTransformerMarker {
                    channel_id: *channel_id,
                    transformer_id: TypeId::of::<T>(),
                });
            }
        }
    }
}
