use std::marker::PhantomData;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "bincode")]
pub use bincode::BincodeProvider;
#[cfg(feature = "serde_json")]
pub use serde_json::SerdeJsonProvider;

use crate::{
    error::NetworkError,
    network::{NetworkData, NetworkMessage},
};
use crate::channels::ChannelMessage;
use crate::connections::NetworkPeer;
use crate::network::{NetworkRawPacket, RemoteSocket};
use crate::network_manager::NetworkNode;
use crate::shared::{NetworkEvent, NetworkNodeEvent};

#[cfg(feature = "bincode")]
mod bincode;

#[cfg(feature = "serde_json")]
mod serde_json;

///
pub trait Transformer: 'static + Send + Sync + Default {
    const NAME: &'static str;
    fn encode<T: Serialize>(data: &T) -> Result<Vec<u8>, NetworkError>;
    fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NetworkError>;
}

#[derive(Debug, Component, Default, Clone)]
pub struct CodingWorker<T, P>
    where
        T: for<'a> Deserialize<'a> + Serialize,
        P: Transformer,
{
    inner: PhantomData<T>,
    provider_inner: PhantomData<P>,
}

impl<T: for<'a> serde::Deserialize<'a> + serde::Serialize, DP: Transformer> CodingWorker<T, DP> {
    pub fn new() -> Self {
        Self {
            inner: PhantomData,
            provider_inner: PhantomData,
        }
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<T, NetworkError> {
        DP::decode::<T>(bytes)
    }

    pub fn encode(&self, message: &T) -> Result<Vec<u8>, NetworkError> {
        DP::encode(message)
    }
}

pub trait NetworkMessageDecoder {
    fn register_transformer<T: NetworkMessage, D: Transformer>(&mut self) -> &mut Self;
}

impl NetworkMessageDecoder for App {
    fn register_transformer<T: NetworkMessage, D: Transformer>(&mut self) -> &mut Self {
        debug!("Registering {} decoder for {}", D::NAME, T::NAME);

        self.add_event::<NetworkData<T>>();
        self.add_event::<ChannelMessage<T>>();
        self.add_systems(PreUpdate, decode_system::<T, D>);
        self.add_systems(PostUpdate, encode_system::<T, D>);
        self.add_systems(Last, spawn_child_worker::<T, D>);
        self
    }
}

fn encode_system<T: NetworkMessage, D: Transformer>(
    mut message_ev: EventReader<ChannelMessage<T>>,
    query: Query<(&NetworkNode, &CodingWorker<T, D>, &RemoteSocket), With<NetworkPeer>>,
) {
    for message in message_ev.read() {
        for (net_node, coding_worker, remote_socket) in query.iter() {
            match coding_worker.encode(&message.message) {
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

fn decode_system<T: NetworkMessage, D: Transformer>(
    mut data_events: EventWriter<NetworkData<T>>,
    mut node_events: EventWriter<NetworkNodeEvent>,
    query: Query<(Entity, &NetworkNode, &CodingWorker<T, D>)>,
) {
    for (entity, network_node, decoder) in query.iter() {
        let mut packets = vec![];
        while let Ok(Some(packet)) = network_node.recv_message_channel.receiver.try_recv() {
            packets.push(packet.bytes);
        }

        if !packets.is_empty() {
            debug!("Decoding {} packets for {}", packets.len(), T::NAME);
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
}

fn spawn_child_worker<T: NetworkMessage, D: Transformer>(
    mut commands: Commands,
    q_parent: Query<&CodingWorker<T, D>, With<NetworkNode>>,
    q_child: Query<(Entity, &Parent), Added<NetworkNode>>,
) {
    for (entity, parent) in q_child.iter() {
        if let Ok(worker) = q_parent.get(parent.get()) {
            // commands.entity(entity).insert(*worker.clone());
        }
    }
}
