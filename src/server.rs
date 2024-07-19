use bevy::{
    ecs::component::{ComponentHooks, StorageType},
    prelude::*,
};

use crate::network_node::NetworkAddress;

#[derive(Deref)]
pub struct ServerNode<T: NetworkAddress>(pub T);

impl<T: NetworkAddress + 'static> Component for ServerNode<T> {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, targeted_entity, _component_id| {
            world.trigger_targets(StartServer, targeted_entity);
        });
    }
}

#[derive(Event, Clone)]
pub struct StartServer;
