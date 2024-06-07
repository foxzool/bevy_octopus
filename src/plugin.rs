use bevy::app::{App, Last, Plugin, PostUpdate, PreUpdate};
use bevy::prelude::{IntoSystemConfigs, IntoSystemSetConfigs};

use crate::{
    channels::{ChannelId, ChannelPacket},
    channels::systems::send_channel_message_system,
    network::NetworkProtocol,
    network_node::update_network_node,
    scheduler::NetworkSet,
    shared::{AsyncRuntime, NetworkNodeEvent},
    tcp, udp,
};
use crate::transformer::{TransformerForChannels, TransformerForMessages};

pub struct OctopusPlugin;

impl Plugin for OctopusPlugin {
    fn build(&self, app: &mut App) {
        let async_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        app.register_type::<NetworkProtocol>()
            .register_type::<ChannelId>()
            .insert_resource(AsyncRuntime(async_runtime))
            .init_resource::<TransformerForChannels>()
            .init_resource::<TransformerForMessages>()
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
            .add_systems(Last, update_network_node);

        #[cfg(feature = "udp")]
        app.add_plugins(udp::UdpPlugin);

        #[cfg(feature = "tcp")]
        app.add_plugins(tcp::TcpPlugin);

        #[cfg(feature = "websocket")]
        app.add_plugins(crate::websocket::WebsocketPlugin);
    }
}
