use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_octopus::{
    network::{LocalSocket, RemoteSocket},
    network_node::NetworkNode,
    shared::NetworkProtocol,
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
        .add_systems(Startup, setup_clients)
        .add_systems(
            Update,
            (
                send_raw_message_to_channel,
                send_json_message,
                send_bincode_message,
                send_socket_packet,
                send_channel_message
            )
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(
            Update,
            (
                handle_raw_packet,
                handle_message_events,
                handle_node_events,
            ),
        )
        .run();
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
