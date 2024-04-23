use std::marker::PhantomData;

use bevy::{
    app::{App, PostUpdate},
    log::debug,
    prelude::{Component, Entity, EventWriter, Query},
};
use serde::Deserialize;

use crate::{
    component::NetworkNode, error::NetworkError, network::NetworkMessage, NetworkData,
    NetworkErrorEvent,
};

pub trait DecoderProvider: 'static + Send + Sync + Default {
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

pub trait AppMessageDecoder {
    fn register_decoder<T: NetworkMessage, D: DecoderProvider>(&mut self) -> &mut Self;
}

impl AppMessageDecoder for App {
    fn register_decoder<T: NetworkMessage, D: DecoderProvider>(&mut self) -> &mut Self {
        debug!("Registering decoder for {}", T::NAME);

        self.add_event::<NetworkData<T>>();
        self.add_systems(PostUpdate, decode_system::<T, D>);
        self
    }
}

fn decode_system<T: NetworkMessage, D: DecoderProvider>(
    mut msg_events: EventWriter<NetworkData<T>>,
    mut error_events: EventWriter<NetworkErrorEvent>,
    query: Query<(Entity, &NetworkNode, &DecodeWorker<T, D>)>,
) {
    for (source, network_node, decoder) in query.iter() {
        let mut packets = vec![];
        while let Ok(Some(packet)) = network_node.recv_channel().receiver.try_recv() {
            packets.push(packet.bytes);
        }

        let (messages, errors): (Vec<_>, Vec<_>) = packets
            .into_iter()
            .map(|msg| decoder.decode(&msg))
            .partition(Result::is_ok);

        msg_events.send_batch(
            messages
                .into_iter()
                .map(Result::unwrap)
                .map(|m| NetworkData { source, inner: m })
                .collect::<Vec<_>>(),
        );
        error_events.send_batch(
            errors
                .into_iter()
                .map(Result::unwrap_err)
                .map(|error| NetworkErrorEvent { source, error })
                .collect::<Vec<_>>(),
        );
    }
}

#[cfg(feature = "bincode")]
pub mod bincode;

#[cfg(feature = "serde_json")]
pub mod serde_json;
