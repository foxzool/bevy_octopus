use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

use bevy_ecs_net::{
    network::RemoteSocket,
    transformer::{BincodeProvider, NetworkMessageDecoder, SerdeJsonProvider},
};
use bevy_ecs_net::shared::NetworkProtocol;

use crate::common::*;

#[path = "common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();

    shared_setup(&mut app);

    app.register_transformer::<PlayerInformation, SerdeJsonProvider>()
        .register_transformer::<PlayerInformation, BincodeProvider>()
        .add_systems(Startup, setup_clients)
        .add_systems(
            Update,
            (
                send_raw_message_to_channel,
                send_json_message,
                send_bincode_message,
            )
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
        RAW_CHANNEL,
        NetworkProtocol::TCP,
        RemoteSocket::new("127.0.0.1:38551"),
    ));
    commands.spawn((
        RAW_CHANNEL,
        NetworkProtocol::TCP,
        RemoteSocket::new("127.0.0.1:6003"),
    ));
    commands.spawn((
        JSON_CHANNEL,
        NetworkProtocol::TCP,
        RemoteSocket::new("127.0.0.1:6004"),
    ));
    commands.spawn((
        BINCODE_CHANNEL,
        NetworkProtocol::TCP,
        RemoteSocket::new("127.0.0.1:6005"),
    ));
}
