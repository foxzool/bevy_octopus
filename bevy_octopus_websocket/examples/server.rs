use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};
use bytes::Bytes;

use bevy_octopus::prelude::*;

use crate::common::*;

#[path = "./common/lib.rs"]
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
            (broadcast_message, send_channel_packet).run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .run();
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        NetworkBundle::new(RAW_CHANNEL),
        ServerAddr::new("ws://0.0.0.0:7003"),
    ));
    commands.spawn((
        NetworkBundle::new(JSON_CHANNEL),
        ServerAddr::new("ws://0.0.0.0:7004"),
    ));
    commands.spawn((
        NetworkBundle::new(BINCODE_CHANNEL),
        ServerAddr::new("ws://0.0.0.0:7005"),
    ));
}

/// broadcast message to all connected clients in channel
fn send_channel_packet(mut channel_events: EventWriter<ChannelPacket>) {
    channel_events.send(ChannelPacket::new(RAW_CHANNEL, b"channel 1 message\r\n"));
}

/// handle send message to connected websocket clients
fn broadcast_message(
    q_net_node: Query<(&ChannelId, &NetworkNode, &Children), Without<NetworkPeer>>,
    q_child: Query<(&NetworkNode, &ConnectTo)>,
) {
    for (channel_id, _net, children) in q_net_node.iter() {
        if channel_id != &RAW_CHANNEL {
            continue;
        }
        for &child in children.iter() {
            let message = b"broadcast message!\r\n";

            let (child_net_node, connect_to) = q_child.get(child).expect("Child node not found.");

            let _ = child_net_node
                .send_message_channel
                .sender
                .try_send(NetworkRawPacket::new(
                    connect_to.to_string(),
                    Bytes::from_static(message),
                ));
        }
    }
}
