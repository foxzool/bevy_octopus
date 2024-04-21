use serde::{Deserialize, Serialize};

use bevy_com::prelude::NetworkMessage;

#[derive(Serialize, Deserialize)]
pub struct PlayerInformation {
    pub health: usize,
    pub position: (u32, u32, u32),
}

impl NetworkMessage for PlayerInformation {
    const NAME: &'static str = "PlayerInfo";
}
