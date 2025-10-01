use crate::{
    error::NetworkError,
    network_node::{NetworkAddress, NetworkEvent, NetworkPeer, NodeEvent},
};
use bevy::{ecs::component::{Immutable, StorageType}, prelude::*};

pub(super) fn plugin(app: &mut App) {
    app.register_type::<ReconnectSetting>()
        .add_systems(Update, handle_reconnect_timer)
        .add_observer(cleanup_client_session)
        .add_observer(client_reconnect);
}

#[derive(Component)]
pub struct ClientTag;

#[derive(Deref)]
pub struct ClientNode<T: NetworkAddress>(pub T);

impl<T: NetworkAddress + 'static> Component for ClientNode<T> {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    type Mutability = Immutable;

    fn on_insert() -> Option<bevy::ecs::lifecycle::ComponentHook> {
        Some(|mut world, ctx| {
            let entity = ctx.entity;
            world.commands().entity(entity).insert(ClientTag);
            world.trigger(StartClient { entity });
        })
    }
}
#[derive(EntityEvent, Clone)]
pub struct StartClient {
    pub entity: Entity,
}

#[derive(Debug, Component, Reflect)]
#[reflect(Component)]
pub struct ReconnectSetting {
    /// Delay in seconds
    pub delay: f32,
    pub max_retries: usize,
    pub retries: usize,
}

impl Default for ReconnectSetting {
    fn default() -> Self {
        Self {
            delay: 2.0,
            max_retries: usize::MAX,
            retries: 0,
        }
    }
}

pub(crate) fn client_reconnect(
    on: On<NodeEvent>,
    mut commands: Commands,
    mut q_net: Query<&mut ReconnectSetting, Without<NetworkPeer>>,
) {
    let ev = on.event();
    if let Ok(mut reconnect) = q_net.get_mut(ev.entity) {
        let event = &ev.event;
        if reconnect.retries < reconnect.max_retries {
            reconnect.retries += 1;
        } else {
            return;
        }
        match event {
            NetworkEvent::Listen | NetworkEvent::Connected => reconnect.retries = 0,
            NetworkEvent::Disconnected | NetworkEvent::Error(NetworkError::Connection(_)) => {
                commands
                    .entity(ev.entity)
                    .insert(ReconnectTimer(Timer::from_seconds(
                        reconnect.delay,
                        TimerMode::Once,
                    )));
            }
            _ => {}
        }
    }
}

#[derive(Component, Deref, DerefMut)]
pub struct ReconnectTimer(pub Timer);

pub(crate) fn handle_reconnect_timer(
    mut commands: Commands,
    time: Res<Time>,
    mut q_reconnect: Query<(Entity, &mut ReconnectTimer)>,
) {
    for (entity, mut timer) in q_reconnect.iter_mut() {
        if timer.tick(time.delta()).just_finished() {
            commands.entity(entity).remove::<ReconnectTimer>();
            commands.trigger(StartClient { entity });
        }
    }
}

pub(crate) fn cleanup_client_session(
    on: On<NodeEvent>,
    mut commands: Commands,
    q_net: Query<Entity, With<NetworkPeer>>,
) {
    let ev = on.event();
    if let Ok(entity) = q_net.get(ev.entity) {
        let event = &ev.event;

        match event {
            NetworkEvent::Disconnected | NetworkEvent::Error(NetworkError::Connection(_)) => {
                commands.entity(entity).despawn();
            }
            _ => {}
        }
    }
}
