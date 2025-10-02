use crate::network_node::NetworkAddress;
use bevy::{
    ecs::component::{Immutable, StorageType},
    prelude::*,
};

#[derive(Deref)]
pub struct ServerNode<T: NetworkAddress>(pub T);

impl<T: NetworkAddress + 'static> Component for ServerNode<T> {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = Immutable;

    fn on_insert() -> Option<bevy::ecs::lifecycle::ComponentHook> {
        Some(|mut world, ctx| {
            world.trigger(StartServer { entity: ctx.entity });
        })
    }
}

#[derive(EntityEvent, Clone)]
pub struct StartServer {
    pub entity: Entity,
}
