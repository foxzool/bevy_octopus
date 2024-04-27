use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

use bevy_ecs_net::decoder::NetworkMessageDecoder;
use bevy_ecs_net::network::RemoteSocket;
use bevy_ecs_net::prelude::{BincodeProvider, SerdeJsonProvider};
use bevy_ecs_net::tcp::TCPProtocol;

use crate::common::*;

#[path = "common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();

    shared_setup(&mut app);

    app.register_decoder::<PlayerInformation, SerdeJsonProvider>()
        .register_decoder::<PlayerInformation, BincodeProvider>()
        .add_systems(Startup, setup_clients)
        .add_systems(
            Update,
            (send_raw_message, send_json_message, send_bincode_message)
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(
            Update,
            (
                receive_raw_messages,
                handle_message_events,
                handle_node_events,
            ),
        )
        .run()
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        TCPProtocol,
        RemoteSocket::new("127.0.0.1:6003"),
        ClientMarker,
        RawPacketMarker,
    ));
    commands.spawn((
        TCPProtocol,
        RemoteSocket::new("127.0.0.1:6003"),
        ClientMarker,
        RawPacketMarker,
    ));
    commands.spawn((
        TCPProtocol,
        RemoteSocket::new("127.0.0.1:6004"),
        ClientMarker,
        JsonMarker,
    ));
    commands.spawn((
        TCPProtocol,
        RemoteSocket::new("127.0.0.1:6005"),
        ClientMarker,
        BincodeMarker,
    ));
}
