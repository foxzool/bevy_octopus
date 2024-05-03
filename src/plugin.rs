use bevy::app::{App, Plugin, PostUpdate, PreUpdate};
use bevy::prelude::{IntoSystemConfigs, IntoSystemSetConfigs};

use crate::{tcp, udp};
use crate::channels::systems::send_channel_message_system;
use crate::network_manager::ChannelMessage;
use crate::scheduler::NetworkSet;
use crate::shared::{AsyncRuntime, NetworkNodeEvent, NetworkProtocol};

pub struct BevyNetPlugin;

impl Plugin for BevyNetPlugin {
    fn build(&self, app: &mut App) {
        let async_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        app.register_type::<NetworkProtocol>()
            .insert_resource(AsyncRuntime(async_runtime))
            .add_event::<NetworkNodeEvent>()
            .add_event::<ChannelMessage>()
            .configure_sets(
                PreUpdate,
                (NetworkSet::Receive, NetworkSet::Process).chain(),
            )
            .configure_sets(PostUpdate, (NetworkSet::Process, NetworkSet::Send).chain())
            .add_systems(
                PreUpdate,
                crate::shared::network_node_event.in_set(NetworkSet::Process),
            )
            .add_systems(
                PostUpdate,
                send_channel_message_system.in_set(NetworkSet::Send),
            );

        #[cfg(feature = "udp")]
        app.add_plugins(udp::UdpPlugin);

        #[cfg(feature = "tcp")]
        app.add_plugins(tcp::TcpPlugin);
    }
}
