use bevy::{
    prelude::{Entity, Resource},
    utils::HashMap,
};

#[derive(Resource, Default)]
pub struct NetworkResource {
    pub nodes: HashMap<&'static str, Entity>,
}
