use bevy::{
    app::{App, Last, Plugin, PostUpdate, PreUpdate},
    prelude::{IntoSystemConfigs, IntoSystemSetConfigs},
};

use crate::{
    channels::{systems::send_channel_message_system, ChannelId, ChannelPacket},
    network_node::update_network_node,
    scheduler::NetworkSet,
    shared::NetworkNodeEvent,
    transformer::{TransformerForChannels, MessageForChannels},
};

pub struct OctopusPlugin;

impl Plugin for OctopusPlugin {
    fn build(&self, app: &mut App) {
        let app = register_reflect_types(app);
        app.init_resource::<TransformerForChannels>()
            .init_resource::<MessageForChannels>()
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
        app.add_plugins(crate::transports::udp::UdpPlugin);

        #[cfg(feature = "tcp")]
        app.add_plugins(crate::transports::tcp::TcpPlugin);

        #[cfg(feature = "websocket")]
        app.add_plugins(crate::transports::websocket::WebsocketPlugin);
    }
}

fn register_reflect_types(app: &mut App) -> &mut App {
    app.register_type::<ChannelId>()
}
