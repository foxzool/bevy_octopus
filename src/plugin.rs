use bevy::{
    app::{App, Plugin, PostUpdate, PreUpdate},
    prelude::{IntoSystemConfigs, IntoSystemSetConfigs, Update},
};

use crate::{
    channels::{ChannelId, ChannelPacket, send_channel_message_system},
    network_node::{ConnectTo, handle_reconnect_timer, ListenTo, network_node_event},
    prelude::client_reconnect,
    scheduler::NetworkSet,
    transformer::{DecoderChannels, EncoderChannels},
};
use crate::network_node::cleanup_client_session;

pub struct OctopusPlugin;

impl Plugin for OctopusPlugin {
    fn build(&self, app: &mut App) {
        let app = register_reflect_types(app);
        app.init_resource::<EncoderChannels>()
            .init_resource::<DecoderChannels>()
            .add_event::<ChannelPacket>()
            .add_event::<ConnectTo>()
            .add_event::<ListenTo>()
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
            .add_systems(Update, handle_reconnect_timer)
            .observe(cleanup_client_session)
            .observe(client_reconnect);

        #[cfg(feature = "udp")]
        app.add_plugins(crate::transports::udp::UdpPlugin);

        #[cfg(feature = "tcp")]
        app.add_plugins(crate::transports::tcp::TcpPlugin);
    }
}

fn register_reflect_types(app: &mut App) -> &mut App {
    app.register_type::<ChannelId>()
}
