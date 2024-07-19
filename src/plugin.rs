use bevy::{
    app::{App, Plugin, PostUpdate, PreUpdate},
    prelude::{IntoSystemConfigs, IntoSystemSetConfigs},
};

use crate::{
    channels::{send_channel_message_system, ChannelId, ChannelPacket},
    client,
    network_node::network_node_event,
    scheduler::NetworkSet,
    server::StartServer,
    transformer::{DecoderChannels, EncoderChannels},
};

pub struct OctopusPlugin;

impl Plugin for OctopusPlugin {
    fn build(&self, app: &mut App) {
        let app = register_reflect_types(app);
        app.init_resource::<EncoderChannels>()
            .init_resource::<DecoderChannels>()
            .add_event::<ChannelPacket>()
            .add_event::<StartServer>()
            .configure_sets(
                PreUpdate,
                (NetworkSet::Receive, NetworkSet::Decoding).chain(),
            )
            .configure_sets(PostUpdate, (NetworkSet::Encoding, NetworkSet::Send).chain())
            .add_systems(PreUpdate, network_node_event.in_set(NetworkSet::Decoding))
            .add_systems(
                PostUpdate,
                send_channel_message_system.in_set(NetworkSet::Send),
            )
            .add_plugins(client::plugin);

        #[cfg(feature = "udp")]
        app.add_plugins(crate::transports::udp::UdpPlugin);

        #[cfg(feature = "tcp")]
        app.add_plugins(crate::transports::tcp::TcpPlugin);
    }
}

fn register_reflect_types(app: &mut App) -> &mut App {
    app.register_type::<ChannelId>()
}
