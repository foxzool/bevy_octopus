use bevy::{
    ecs::component::{ComponentHooks, StorageType},
    prelude::*,
};

use crate::network_node::{ConnectTo, NetworkAddress};

pub(super) fn plugin(_app: &mut App) {}

#[derive(Component)]
pub struct ClientTag;

#[derive(Deref)]
pub struct Client<T: NetworkAddress>(pub T);

impl<T: NetworkAddress + 'static> Component for Client<T> {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, targeted_entity, _component_id| {
            world.commands().entity(targeted_entity).insert(ClientTag);
            let client_addr = world.get::<Client<T>>(targeted_entity).unwrap();
            world.trigger_targets(ConnectTo::new(&client_addr.0.to_string()), targeted_entity);
        });
    }
}
