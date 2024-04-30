use std::marker::PhantomData;

use bevy::{
    app::App,
    log::debug,
    prelude::{Component, Entity, EventWriter, Query},
};
use bevy::app::PreUpdate;
use serde::Deserialize;

#[cfg(feature = "bincode")]
pub use bincode::BincodeProvider;
#[cfg(feature = "serde_json")]
pub use serde_json::SerdeJsonProvider;

use crate::{
    error::NetworkError,
    network::{NetworkData, NetworkMessage},
};
use crate::network_manager::NetworkNode;
use crate::shared::{NetworkEvent, NetworkNodeEvent};

#[cfg(feature = "bincode")]
mod bincode;

#[cfg(feature = "serde_json")]
mod serde_json;

///
pub trait DecoderProvider: 'static + Send + Sync + Default {
    const NAME: &'static str;
    fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NetworkError>;
}

#[derive(Debug, Component, Default)]
pub struct DecodeWorker<T, P>
    where
        T: for<'a> Deserialize<'a>,
        P: DecoderProvider,
{
    inner: PhantomData<T>,
    provider_inner: PhantomData<P>,
}

impl<T: for<'a> serde::Deserialize<'a>, DP: DecoderProvider> DecodeWorker<T, DP> {
    pub fn new() -> Self {
        Self {
            inner: PhantomData,
            provider_inner: PhantomData,
        }
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<T, NetworkError> {
        DP::decode::<T>(bytes)
    }
}

pub trait NetworkMessageDecoder {
    fn register_decoder<T: NetworkMessage, D: DecoderProvider>(&mut self) -> &mut Self;
}

impl NetworkMessageDecoder for App {
    fn register_decoder<T: NetworkMessage, D: DecoderProvider>(&mut self) -> &mut Self {
        debug!("Registering {} decoder for {}", D::NAME, T::NAME);

        self.add_event::<NetworkData<T>>();
        self.add_systems(PreUpdate, decode_system::<T, D>);
        self
    }
}

fn decode_system<T: NetworkMessage, D: DecoderProvider>(
    mut data_events: EventWriter<NetworkData<T>>,
    mut node_events: EventWriter<NetworkNodeEvent>,
    query: Query<(Entity, &NetworkNode, &DecodeWorker<T, D>)>,
) {
    for (entity, network_node, decoder) in query.iter() {
        let mut packets = vec![];
        while let Ok(Some(packet)) = network_node.recv_message_channel.receiver.try_recv() {
            packets.push(packet.bytes);
        }

        let (messages, errors): (Vec<_>, Vec<_>) = packets
            .into_iter()
            .map(|msg| decoder.decode(&msg))
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
