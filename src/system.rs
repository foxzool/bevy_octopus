use bevy::{ecs::query::QueryData, prelude::*};
use serde::Deserialize;

use crate::{
    component::{JsonDecoder, NetworkNode},
    prelude::{NetworkMessage, StopMarker},
};

pub trait AppNetworkMessage {
    fn register_json_decoder<T: NetworkMessage>(&mut self) -> &mut Self;
}

impl AppNetworkMessage for App {
    fn register_json_decoder<T: NetworkMessage>(&mut self) -> &mut Self {
        debug!("Registering decoder for {}", T::NAME);
        self.add_systems(PostUpdate, decode_system::<T>);
        self
    }
}

fn decode_system<T: for<'a> Deserialize<'a> + Send + Sync + 'static>(
    query: Query<(Entity, &NetworkNode, &JsonDecoder<T>), With<JsonDecoder<T>>>,
) {
    for (_entity, network_node, decoder) in query.iter() {
        while let Ok(Some(packet)) = network_node.message_receiver().try_recv() {
            debug!("Decoding packet {:?}", packet);
            // let decoded: T = bincode::deserialize(&packet.bytes).unwrap();
            let decoded: Option<T> = decoder.decode(&packet.bytes);
            // debug!("Decoded packet {:?}", decoded);
        }
    }
}
