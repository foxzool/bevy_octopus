use std::fmt::Display;

use bevy::ecs::reflect::ReflectComponent;
use bevy::prelude::{Component, Reflect};

/// Channel marker
#[derive(Clone, PartialEq, Eq, Hash, Default, Component, Reflect, Copy, Debug)]
#[reflect(Component)]
pub struct ChannelId(pub u32);

impl Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChannelId({})", self.0)
    }
}

impl From<u32> for ChannelId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl From<ChannelId> for u32 {
    fn from(id: ChannelId) -> Self {
        id.0
    }
}
