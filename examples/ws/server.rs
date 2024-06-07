

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use bytes::Bytes;

use bevy_octopus::connections::NetworkPeer;
use bevy_octopus::prelude::{ChannelId, ChannelPacket};
use bevy_octopus::{
    network::NetworkProtocol,
    network::{LocalSocket, NetworkRawPacket, RemoteSocket},
    network_node::NetworkNode,
    transformer::{BincodeTransformer, JsonTransformer, NetworkMessageTransformer},
};

use crate::common::*;

#[path = "../common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();

    shared_setup(&mut app);

    app.add_transformer::<PlayerInformation, JsonTransformer>(JSON_CHANNEL)
        .add_transformer::<PlayerInformation, BincodeTransformer>(BINCODE_CHANNEL)
        .add_systems(Startup, setup_server)
        .add_systems(
            Update,
            (handle_raw_packet, handle_message_events, handle_node_events),
        )
        .add_systems(
            Update,
            (broadcast_message, send_channel_message, send_channel_packet)
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .run()
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        RAW_CHANNEL,
        NetworkProtocol::WS,
        LocalSocket::new("0.0.0.0:7003"),
    ));
    commands.spawn((
        JSON_CHANNEL,
        NetworkProtocol::WS,
        LocalSocket::new("0.0.0.0:7004"),
    ));
    commands.spawn((
        BINCODE_CHANNEL,
        NetworkProtocol::WS,
        LocalSocket::new("0.0.0.0:7005"),
    ));
}

/// broadcast message to all connected clients in channel
fn send_channel_packet(mut channel_events: EventWriter<ChannelPacket>) {
    channel_events.send(ChannelPacket::new(RAW_CHANNEL, b"channel 1 message\r\n"));
}

/// handle send message to connected websocket clients
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
