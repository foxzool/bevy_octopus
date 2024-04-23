use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use bevy_com::{
    prelude::{NetworkMessage, NetworkNode},
    NetworkErrorEvent,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerInformation {
    pub health: usize,
    pub position: (u32, u32, u32),
}

impl NetworkMessage for PlayerInformation {
    const NAME: &'static str = "PlayerInfo";
}

pub fn handle_error_events(
    mut new_network_events: EventReader<NetworkErrorEvent>,
    q_net_node: Query<&NetworkNode>,
) {
    for event in new_network_events.read() {
        let net = q_net_node.get(event.source).unwrap();
        error!("{:?} got Error: {:?}", net.local_addr, event.error);
    }
}
