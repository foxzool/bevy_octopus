use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use bytes::Bytes;

use bevy_ecs_net::{
    decoder::{BincodeProvider, DecodeWorker, NetworkMessageDecoder, SerdeJsonProvider},
    network::{LocalSocket, NetworkRawPacket, RemoteSocket},
    network_manager::NetworkNode,
    shared::NetworkProtocol,
};
use bevy_ecs_net::network_manager::ChannelMessage;
use bevy_ecs_net::prelude::*;

use crate::common::*;

const CHANNEL_1: ChannelId = ChannelId(1);
const CHANNEL_2: ChannelId = ChannelId(2);
const CHANNEL_3: ChannelId = ChannelId(3);

#[path = "common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();

    shared_setup(&mut app);

    app.register_decoder::<PlayerInformation, SerdeJsonProvider>()
        .register_decoder::<PlayerInformation, BincodeProvider>()
        .add_systems(Startup, setup_server)
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
            (broadcast_message, channel_message).run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .run()
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        CHANNEL_1,
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6003"),
        ServerMarker,
        RawPacketMarker,
    ));
    commands.spawn((
        CHANNEL_2,
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6004"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, SerdeJsonProvider>::new(),
    ));
    commands.spawn((
        CHANNEL_3,
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6005"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, BincodeProvider>::new(),
    ));
}

/// broadcast message to all connected clients in channel
fn channel_message(mut channel_events: EventWriter<ChannelMessage>) {
    channel_events.send(ChannelMessage::new(CHANNEL_1, b"channel 1 message"));
}

/// handle send message to connected clients
fn broadcast_message(
    q_net_node: Query<(&NetworkNode, &Children), (With<ServerMarker>, With<RawPacketMarker>)>,
    q_child: Query<(&NetworkNode, &RemoteSocket)>,
) {
    for (_net, children) in q_net_node.iter() {
        for &child in children.iter() {
            let message = b"broadcast message!";

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
