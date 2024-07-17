use std::{any::TypeId, collections::HashMap, fmt::Debug, marker::PhantomData};

use bevy::{prelude::*, reflect::GetTypeRegistration};
use bytes::Bytes;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[cfg(feature = "bincode")]
pub use bincode::BincodeTransformer;
#[cfg(feature = "serde_json")]
pub use serde_json::JsonTransformer;

use crate::{
    channels::{ChannelId, ChannelReceivedMessage, ChannelSendMessage},
    error::NetworkError,
    network_node::{NetworkEvent, NetworkNode, NetworkRawPacket, RemoteAddr},
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

    fn add_encoder<
        M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
        T: Transformer,
    >(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self;

    fn add_decoder<
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
        self.add_encoder::<M, T>(channel_id)
            .add_decoder::<M, T>(channel_id)
    }

    fn add_encoder<
        M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
        T: Transformer,
    >(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self {
        debug!(
            "Registering {} encoder for {} {}",
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

        let mut encoder_channels = self.world_mut().resource_mut::<EncoderChannels>();
        if let Some(ids) = encoder_channels.get_mut(&(message_type_id, transform_type_id)) {
            ids.push(channel_id);
        } else {
            encoder_channels.insert((message_type_id, transform_type_id), vec![channel_id]);
            self.add_systems(PostUpdate, spawn_encoder_marker::<M, T>);
            self.add_systems(PostUpdate, encode_system::<M, T>);
        }

        self.add_event::<ChannelReceivedMessage<M>>();

        self
    }

    fn add_decoder<
        M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
        T: Transformer,
    >(
        &mut self,
        channel_id: ChannelId,
    ) -> &mut Self {
        debug!(
            "Registering {} decoder for {} {}",
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

        let mut decoder_channels = self.world_mut().resource_mut::<DecoderChannels>();
        if let Some(ids) = decoder_channels.get_mut(&(message_type_id, transform_type_id)) {
            ids.push(channel_id);
        } else {
            decoder_channels.insert((message_type_id, transform_type_id), vec![channel_id]);
            self.add_systems(PreUpdate, decode_system::<M, T>);
            self.add_systems(PostUpdate, spawn_decoder_marker::<M, T>);
        }

        self.add_event::<ChannelSendMessage<M>>();

        self
    }
}

pub(crate) type TransformerTypeId = TypeId;
pub(crate) type MessageTypeId = TypeId;

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub(crate) struct DecoderChannels(
    pub(crate) HashMap<(MessageTypeId, TransformerTypeId), Vec<ChannelId>>,
);

#[derive(Resource, Deref, DerefMut, Debug, Default)]
pub(crate) struct EncoderChannels(
    pub(crate) HashMap<(MessageTypeId, TransformerTypeId), Vec<ChannelId>>,
);

#[derive(Component, Debug)]
pub struct TransformerSenderMarker {
    pub channel_id: ChannelId,
    pub transformer_id: TypeId,
    // pub message_id: TypeId
}

#[derive(Component, Debug)]
pub struct EncoderMarker<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer,
> {
    _message: PhantomData<M>,
    _transformer: PhantomData<T>,
}

impl<M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static, T: Transformer> Default
    for EncoderMarker<M, T>
{
    fn default() -> Self {
        Self {
            _message: PhantomData,
            _transformer: PhantomData,
        }
    }
}

#[derive(Component, Debug)]
pub struct DecoderMarker<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer,
> {
    _message: PhantomData<M>,
    _transformer: PhantomData<T>,
}

impl<M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static, T: Transformer> Default
    for DecoderMarker<M, T>
{
    fn default() -> Self {
        Self {
            _message: PhantomData,
            _transformer: PhantomData,
        }
    }
}

/// encode system fro encoder marker
#[allow(clippy::type_complexity)]
fn encode_system<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer + bevy::prelude::Resource,
>(
    mut message_ev: EventReader<ChannelSendMessage<M>>,
    transformer: Res<T>,
    query: Query<(&ChannelId, &NetworkNode, &RemoteAddr), With<EncoderMarker<M, T>>>,
) {
    for message in message_ev.read() {
        for (channel_id, net_node, remote_addr) in query.iter() {
            if channel_id != &message.channel_id || !net_node.running {
                continue;
            }

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
                            remote_addr.to_string(),
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

/// decode system with decoder marker
#[allow(clippy::type_complexity)]
fn decode_system<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer + bevy::prelude::Resource,
>(
    mut channel_message: EventWriter<ChannelReceivedMessage<M>>,
    mut commands: Commands,
    transformer: Res<T>,
    query: Query<(Entity, &ChannelId, &NetworkNode), With<DecoderMarker<M, T>>>,
) {
    for (entity, channel_id, network_node) in query.iter() {
        let mut packets = vec![];
        while let Ok(Some(packet)) = network_node.recv_message_channel.receiver.try_recv() {
            packets.push(packet.bytes);
        }

        if !packets.is_empty() {
            let (messages, errors): (Vec<_>, Vec<_>) = packets
                .into_iter()
                .map(|msg| transformer.decode::<M>(&msg))
                .partition(Result::is_ok);
            trace!(
                "{} decoding {} {} packets error {} for {}",
                channel_id,
                T::NAME,
                messages.len(),
                errors.len(),
                std::any::type_name::<M>(),
            );
            channel_message.send_batch(
                messages
                    .into_iter()
                    .map(Result::unwrap)
                    .map(|m| ChannelReceivedMessage::new(*channel_id, m))
                    .collect::<Vec<_>>(),
            );
            for error in errors.into_iter().map(Result::unwrap_err) {
                commands.trigger_targets(NetworkEvent::Error(error), entity);
            }
        }
    }
}

fn spawn_encoder_marker<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer,
>(
    mut commands: Commands,
    mt_ids: Res<EncoderChannels>,
    q_channel: Query<(Entity, &ChannelId), Added<ChannelId>>,
) {
    for (entity, channel_id) in q_channel.iter() {
        if let Some(channels) = mt_ids.0.get(&(TypeId::of::<M>(), TypeId::of::<T>())) {
            if channels.contains(channel_id) {
                trace!(
                    "{:?} Spawning encoder marker for {} in {}",
                    entity,
                    T::NAME,
                    channel_id
                );
                commands
                    .entity(entity)
                    .insert(EncoderMarker::<M, T>::default());
            }
        }
    }
}

fn spawn_decoder_marker<
    M: Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    T: Transformer,
>(
    mut commands: Commands,
    mt_ids: Res<DecoderChannels>,
    q_channel: Query<(Entity, &ChannelId), Added<ChannelId>>,
) {
    for (entity, channel_id) in q_channel.iter() {
        if let Some(channels) = mt_ids.0.get(&(TypeId::of::<M>(), TypeId::of::<T>())) {
            if channels.contains(channel_id) {
                trace!(
                    "{:?} Spawning decoder marker for {} in {}",
                    entity,
                    T::NAME,
                    channel_id
                );
                commands
                    .entity(entity)
                    .insert(DecoderMarker::<M, T>::default());
            }
        }
    }
}
