use std::{any::TypeId, collections::HashMap, fmt::Debug, ops::Deref};

use bevy::{prelude::*, reflect::GetTypeRegistration};
use bytes::Bytes;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[cfg(feature = "bincode")]
pub use bincode::BincodeTransformer;
#[cfg(feature = "serde_json")]
pub use serde_json::JsonTransformer;

use crate::{
    channels::{ChannelId, ChannelMessage},
    connections::NetworkPeer,
    error::NetworkError,
    network::{ConnectTo, NetworkData, NetworkRawPacket},
    network_node::NetworkNode,
    shared::{NetworkEvent, NetworkNodeEvent},
};

#[cfg(feature = "bincode")]
mod bincode;

#[cfg(feature = "serde_json")]
mod serde_json;

pub trait Transformer:
    'static + Send + Sync + Reflect + Resource + Default + GetTypeRegistration
{
    const NAME: &'static str;
    fn encode<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, NetworkError>;
    fn decode<T: for<'a> Deserialize<'a>>(&self, bytes: &[u8]) -> Result<T, NetworkError>;
}

pub trait NetworkMessageTransformer {
    fn add_transformer<
        M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
        T: Transformer,
    >(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self;
}

impl NetworkMessageTransformer for App {
    fn add_transformer<
        M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
        T: Transformer,
    >(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self {
        debug!(
            "Registering {} transformer for {} in {}",
            T::NAME,
            std::any::type_name::<M>(),
            channel_id
        );
        if self.world().get_resource::<T>().is_none() {
            self.world_mut().init_resource::<T>();
        }

        self.register_type::<T>();

        let transform_type_id = TypeId::of::<T>();
        let message_type_id = TypeId::of::<M>();

        let mut trans_for_channels = self.world_mut().resource_mut::<TransformerForChannels>();
        match trans_for_channels.0.get_mut(&transform_type_id) {
            None => {
                trans_for_channels.insert(transform_type_id, vec![channel_id]);
                self.add_systems(PostUpdate, spawn_marker::<T>);
            }
            Some(channels) => {
                if !channels.contains(&channel_id) {
                    channels.push(channel_id);
                }
            }
        }

        let mut msg_for_channels = self.world_mut().resource_mut::<MessageForChannels>();
        match msg_for_channels.get_mut(&message_type_id) {
            None => {
                msg_for_channels.0.insert(message_type_id, vec![channel_id]);
                self.add_systems(PreUpdate, decode_system::<M, T>);
                self.add_systems(PostUpdate, encode_system::<M, T>);

            }
            Some(message_types) => {
                if !message_types.contains(&channel_id) {
                    message_types.push(channel_id);
                }
            }
        }

        self.add_event::<NetworkData<M>>();
        self.add_event::<ChannelMessage<M>>();

        self
    }
}

pub(crate) type TransformerTypeId = TypeId;
pub(crate) type MessageTypeId = TypeId;

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub(crate) struct TransformerForChannels(pub HashMap<TransformerTypeId, Vec<ChannelId>>);

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub(crate) struct MessageForChannels(pub HashMap<MessageTypeId, Vec<ChannelId>>);

#[derive(Component, Debug)]
pub struct TransformerSenderMarker {
    pub channel_id: ChannelId,
    pub transformer_id: TypeId,
}

#[derive(Component, Debug)]
pub struct TransformerRecvMarker {
    pub channel_id: ChannelId,
    pub transformer_id: TypeId,
}

fn encode_system<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer + bevy::prelude::Resource,
>(
    mut message_ev: EventReader<ChannelMessage<M>>,
    transformer: Res<T>,
    query: Query<
        (
            &ChannelId,
            &NetworkNode,
            &ConnectTo,
            &TransformerSenderMarker,
        ),
        With<NetworkPeer>,
    >,
) {
    for message in message_ev.read() {
        for (channel_id, net_node, connect_to, channel_marker) in query.iter() {
            if *channel_id == message.channel_id
                && channel_marker.transformer_id == TypeId::of::<T>()
            {
                trace!(
                    "{} {} Encoding message for {}",
                    channel_id,
                    T::NAME,
                    std::any::type_name::<M>(),
                );
                match transformer.encode(&message.message) {
                    Ok(bytes) => {
                        let _ = net_node
                            .send_message_channel
                            .sender
                            .send(NetworkRawPacket::new(
                                connect_to.to_string(),
                                Bytes::from_iter(bytes),
                            ));
                    }

                    Err(e) => {
                        let _ = net_node.event_channel.sender.send(NetworkEvent::Error(
                            NetworkError::SerializeError(e.to_string()),
                        ));
                    }
                }
            }
        }
    }
}

fn decode_system<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer + bevy::prelude::Resource,
>(
    mut data_events: EventWriter<NetworkData<M>>,
    mut node_events: EventWriter<NetworkNodeEvent>,
    transformer: Res<T>,
    query: Query<(Entity, &ChannelId, &NetworkNode, &TransformerRecvMarker)>,
) {
    for (entity, channel_id, network_node, channel_marker) in query.iter() {
        if channel_marker.channel_id != *channel_id
            || channel_marker.transformer_id != TypeId::of::<T>()
        {
            continue;
        }

        decode_packets::<M, T>(
            entity,
            *channel_id,
            network_node,
            transformer.deref(),
            &mut data_events,
            &mut node_events,
        );
    }
}

fn decode_packets<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer,
>(
    entity: Entity,
    channel_id: ChannelId,
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
        println!(
            "{} decode_system {:?} {:?} {:?} ",
            entity,
            channel_id,
            std::any::type_name::<M>(),
            TypeId::of::<T>()
        );
        let (messages, errors): (Vec<_>, Vec<_>) = packets
            .into_iter()
            .map(|msg| transformer.decode::<M>(&msg))
            .partition(Result::is_ok);
        debug!(
            "{} decoding {} packets error {} for {}",
            T::NAME,
            messages.len(),
            errors.len(),
            std::any::type_name::<M>(),
        );
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
                    channel_id,
                    event: NetworkEvent::Error(error),
                })
                .collect::<Vec<_>>(),
        );
    }
}

fn spawn_marker<T: Transformer>(
    mut commands: Commands,
    transformer_index: Res<TransformerForChannels>,
    q_channel: Query<(Entity, &ChannelId, Option<&ConnectTo>), Added<ChannelId>>,
) {
    for (entity, channel_id, option_remote) in q_channel.iter() {
        if let Some(channels) = transformer_index.0.get(&TypeId::of::<T>()) {
            if channels.contains(channel_id) {
                if option_remote.is_some() {
                    trace!(
                        "{:?} Spawning sender marker for {} in {}",
                        entity,
                        T::NAME,
                        channel_id
                    );
                    commands.entity(entity).insert(TransformerSenderMarker {
                        channel_id: *channel_id,
                        transformer_id: TypeId::of::<T>(),
                    });
                } else {
                    trace!(
                        "{:?} Spawning recv   marker for {} in {}",
                        entity,
                        T::NAME,
                        channel_id
                    );
                    commands.entity(entity).insert(TransformerRecvMarker {
                        channel_id: *channel_id,
                        transformer_id: TypeId::of::<T>(),
                    });
                }
            }
        }
    }
}
