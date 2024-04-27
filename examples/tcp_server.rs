use bevy::prelude::*;

use bevy_ecs_net::decoder::{DecodeWorker, NetworkMessageDecoder};
use bevy_ecs_net::network::LocalSocket;
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
        .add_systems(Startup, setup_server)
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

fn setup_server(mut commands: Commands) {
    commands.spawn((
        TCPProtocol,
        LocalSocket::new("0.0.0.0:6003"),
        ServerMarker,
        RawPacketMarker,
    ));
    commands.spawn((
        TCPProtocol,
        LocalSocket::new("0.0.0.0:6004"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, SerdeJsonProvider>::new(),
    ));
    commands.spawn((
        TCPProtocol,
        LocalSocket::new("0.0.0.0:6005"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, BincodeProvider>::new(),
    ));
}
