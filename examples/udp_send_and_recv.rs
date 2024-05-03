use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_ecs_net::{
    decoder::{BincodeProvider, DecodeWorker, NetworkMessageDecoder, SerdeJsonProvider},
    network::{LocalSocket, RemoteSocket},
    network_manager::NetworkNode,
    shared::NetworkProtocol,
};

use crate::common::*;

#[path = "common/lib.rs"]
mod common;


fn main() {
    let mut app = App::new();
    shared_setup(&mut app);

    app.register_decoder::<PlayerInformation, SerdeJsonProvider>()
        .register_decoder::<PlayerInformation, BincodeProvider>()
        .add_systems(Startup, (setup_clients, setup_server))
        .add_systems(
            Update,
            (
                send_raw_message_to_channel,
                send_json_message,
                send_bincode_message,
                send_socket_packet,
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
        .run();
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        RAW_CHANNEL,
        NetworkProtocol::UDP,
        LocalSocket::new("0.0.0.0:6001"),
    ));
    commands.spawn((
        JSON_CHANNEL,
        NetworkProtocol::UDP,
        LocalSocket::new("0.0.0.0:6002"),
        DecodeWorker::<PlayerInformation, SerdeJsonProvider>::new(),
    ));

    commands.spawn((
        BINCODE_CHANNEL,
        NetworkProtocol::UDP,
        LocalSocket::new("0.0.0.0:6003"),
        DecodeWorker::<PlayerInformation, BincodeProvider>::new(),
    ));
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        RAW_CHANNEL,
        NetworkProtocol::UDP,
        LocalSocket::new("0.0.0.0:7006"),
        RemoteSocket::new("127.0.0.1:6001"),
    ));
    commands.spawn((
        RAW_CHANNEL,
        NetworkProtocol::UDP,
        LocalSocket::new("0.0.0.0:0"),
        RemoteSocket::new("127.0.0.1:6001"),
    ));
    commands.spawn((
        RAW_CHANNEL,
        NetworkProtocol::UDP,
        RemoteSocket::new("127.0.0.1:6001"),
    ));

    commands.spawn((
        JSON_CHANNEL,
        NetworkProtocol::UDP,
        RemoteSocket::new("127.0.0.1:6002"),
    ));

    commands.spawn((
        BINCODE_CHANNEL,
        NetworkProtocol::UDP,
        RemoteSocket::new("127.0.0.1:6003"),
    ));
}

fn send_socket_packet(q_client: Query<&NetworkNode, With<RemoteSocket>>) {
    for client in q_client.iter() {
        client.send_to(
            "I can send message to specify socket".as_bytes(),
            "127.0.0.1:6001",
        );
    }
}
