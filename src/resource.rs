use bevy::prelude::{Entity, Resource};
use bevy::utils::HashMap;

#[derive(Resource, Default)]
pub struct NetworkResource {
    pub nodes: HashMap<&'static str, Entity>,
}
