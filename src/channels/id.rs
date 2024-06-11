use std::fmt::Display;

use bevy::{
    ecs::reflect::ReflectComponent,
    prelude::{Component, Reflect},
};

/// Channel marker
#[derive(Clone, PartialEq, Eq, Hash, Default, Component, Reflect, Copy, Debug)]
#[reflect(Component)]
pub struct ChannelId(pub &'static str);

impl Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChannelId({})", self.0)
    }
}
