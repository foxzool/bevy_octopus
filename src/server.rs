use crate::network_node::NetworkAddress;
use bevy::{
    ecs::component::{ComponentHooks, HookContext, Immutable, StorageType},
    prelude::*,
};

#[derive(Deref)]
pub struct ServerNode<T: NetworkAddress>(pub T);

impl<T: NetworkAddress + 'static> Component for ServerNode<T> {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = Immutable;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, HookContext { entity, .. }| {
            world.trigger_targets(StartServer, entity);
        });
    }
}

#[derive(Event, Clone)]
pub struct StartServer;
