use bevy::app::{App, Plugin, PostUpdate, PreUpdate};
use bevy::prelude::{IntoSystemConfigs, IntoSystemSetConfigs};

use crate::{
    channels::systems::send_channel_message_system,
    channels::{ChannelId, ChannelPacket},
    network::NetworkProtocol,
    network_node::update_network_node,
    scheduler::NetworkSet,
    shared::{AsyncRuntime, NetworkNodeEvent},
    tcp, udp,
};

pub struct BevyNetPlugin;

impl Plugin for BevyNetPlugin {
    fn build(&self, app: &mut App) {
        let async_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        app.register_type::<NetworkProtocol>()
            .register_type::<ChannelId>()
            .insert_resource(AsyncRuntime(async_runtime))
            .add_event::<NetworkNodeEvent>()
            .add_event::<ChannelPacket>()
            .configure_sets(
                PreUpdate,
                (NetworkSet::Receive, NetworkSet::Decoding).chain(),
            )
            .configure_sets(PostUpdate, (NetworkSet::Encoding, NetworkSet::Send).chain())
            .add_systems(
                PreUpdate,
                crate::shared::network_node_event.in_set(NetworkSet::Decoding),
            )
            .add_systems(
                PostUpdate,
                send_channel_message_system.in_set(NetworkSet::Send),
            )
            .add_systems(PostUpdate, update_network_node);

        #[cfg(feature = "udp")]
        app.add_plugins(udp::UdpPlugin);

        #[cfg(feature = "tcp")]
        app.add_plugins(tcp::TcpPlugin);
    }
}
