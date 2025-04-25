use crate::{
    channels::{ChannelId, ChannelPacket, send_channel_message_system},
    client,
    network_node::{NetworkNode, network_node_event},
    server::StartServer,
    transformer::{DecoderChannels, EncoderChannels},
    transports::{tcp::TcpPlugin, udp::UdpPlugin},
};
use bevy::{
    app::{App, Plugin, PostUpdate, PreUpdate},
    prelude::{IntoScheduleConfigs, SystemSet},
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

        app.add_plugins(UdpPlugin).add_plugins(TcpPlugin);
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum NetworkSet {
    Receive,
    Decoding,
    Encoding,
    Send,
}

fn register_reflect_types(app: &mut App) -> &mut App {
    app.register_type::<ChannelId>()
        .register_type::<NetworkNode>()
        .register_type::<&'static str>()
}
