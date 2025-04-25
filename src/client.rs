use crate::{
    error::NetworkError,
    network_node::{NetworkAddress, NetworkEvent, NetworkPeer},
};
use bevy::{
    ecs::component::{ComponentHooks, HookContext, Immutable, StorageType},
    prelude::*,
};

pub(super) fn plugin(app: &mut App) {
    app.register_type::<ReconnectSetting>()
        .add_event::<StartClient>()
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

    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_insert(|mut world, HookContext { entity, .. }| {
            world.commands().entity(entity).insert(ClientTag);
            world.trigger_targets(StartClient, entity);
        });
    }
}
#[derive(Event, Clone)]
pub struct StartClient;

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
    trigger: Trigger<NetworkEvent>,
    mut commands: Commands,
    mut q_net: Query<&mut ReconnectSetting, Without<NetworkPeer>>,
) {
    if let Ok(mut reconnect) = q_net.get_mut(trigger.target()) {
        let event = trigger.event();
        if reconnect.retries < reconnect.max_retries {
            reconnect.retries += 1;
        } else {
            return;
        }
        match event {
            NetworkEvent::Listen | NetworkEvent::Connected => reconnect.retries = 0,
            NetworkEvent::Disconnected | NetworkEvent::Error(NetworkError::Connection(_)) => {
                commands
                    .entity(trigger.target())
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
            commands.trigger_targets(StartClient, entity);
        }
    }
}

pub(crate) fn cleanup_client_session(
    trigger: Trigger<NetworkEvent>,
    mut commands: Commands,
    q_net: Query<Entity, With<NetworkPeer>>,
) {
    if let Ok(entity) = q_net.get(trigger.target()) {
        let event = trigger.event();

        match event {
            NetworkEvent::Disconnected | NetworkEvent::Error(NetworkError::Connection(_)) => {
                commands.entity(entity).despawn();
            }
            _ => {}
        }
    }
}
