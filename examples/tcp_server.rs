use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

use bevy_ecs_net::decoder::{DecodeWorker, NetworkMessageDecoder};
use bevy_ecs_net::network::LocalSocket;
use bevy_ecs_net::prelude::{BincodeProvider, NetworkNode, NetworkProtocol, SerdeJsonProvider};

use crate::common::*;

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
            broadcast_message.run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .run()
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6003"),
        ServerMarker,
        RawPacketMarker,
    ));
    commands.spawn((
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6004"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, SerdeJsonProvider>::new(),
    ));
    commands.spawn((
        NetworkProtocol::TCP,
        LocalSocket::new("0.0.0.0:6005"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, BincodeProvider>::new(),
    ));
}

fn broadcast_message(q_net_node: Query<&NetworkNode, (With<ServerMarker>, With<RawPacketMarker>)>) {
    for net in q_net_node.iter() {
        net.broadcast(b"broadcast message\r");
    }
}
