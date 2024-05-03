use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use bytes::Bytes;

use bevy_ecs_net::{
    network::{LocalSocket, NetworkRawPacket, RemoteSocket},
    network_manager::NetworkNode,
    shared::NetworkProtocol,
    transformer::{BincodeProvider, NetworkMessageTransformer, SerdeJsonProvider},
};
use bevy_ecs_net::connections::NetworkPeer;
use bevy_ecs_net::prelude::{ChannelId, ChannelPacket};

use crate::common::*;

#[path = "common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();

    shared_setup(&mut app);

    app.register_channel_transformer::<PlayerInformation, SerdeJsonProvider>(JSON_CHANNEL)
        .register_channel_transformer::<PlayerInformation, BincodeProvider>(BINCODE_CHANNEL).add_systems(Startup, setup_server)
        .add_systems(
            Update,
            (
                receive_raw_messages,
                handle_message_events,
                handle_node_events,
            ),
        )
        .add_systems(
            Update,
            (broadcast_message, channel_packet).run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .run()
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        RAW_CHANNEL,
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6003"),
    ));
    commands.spawn((
        JSON_CHANNEL,
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6004"),
    ));
    commands.spawn((
        BINCODE_CHANNEL,
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6005"),
    ));
}

/// broadcast message to all connected clients in channel
fn channel_packet(mut channel_events: EventWriter<ChannelPacket>) {
    channel_events.send(ChannelPacket::new(RAW_CHANNEL, b"channel 1 message\r\n"));
}

/// handle send message to connected clients
fn broadcast_message(
    q_net_node: Query<(&ChannelId, &NetworkNode, &Children), Without<NetworkPeer>>,
    q_child: Query<(&NetworkNode, &RemoteSocket)>,
) {
    for (channel_id, _net, children) in q_net_node.iter() {
        if channel_id != &RAW_CHANNEL {
            continue;
        }
        for &child in children.iter() {
            let message = b"broadcast message!\r\n";

            let (child_net_node, child_remote_addr) =
                q_child.get(child).expect("Child node not found.");

            child_net_node
                .send_message_channel
                .sender
                .try_send(NetworkRawPacket {
                    addr: **child_remote_addr,
                    bytes: Bytes::from_static(message),
                })
                .expect("Message channel has closed.");
        }
    }
}
