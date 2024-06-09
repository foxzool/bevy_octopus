use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_octopus::prelude::*;

use crate::common::*;

#[path = "../common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();
    shared_setup(&mut app);

    app.add_systems(Startup, setup_clients)
        .add_systems(
            Update,
            (send_raw_message_to_channel, send_socket_packet)
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, (handle_raw_packet, handle_node_events))
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        RAW_CHANNEL,
        ListenTo::new("udp://0.0.0.0:7006"),
        ConnectTo::new("udp://127.0.0.1:6001"),
    ));
    commands.spawn((
        RAW_CHANNEL,
        ListenTo::new("udp://0.0.0.0:0"),
        ConnectTo::new("udp://127.0.0.1:6001"),
    ));
    commands.spawn((RAW_CHANNEL, ConnectTo::new("udp://127.0.0.1:6001")));
}

fn send_socket_packet(q_client: Query<&NetworkNode, With<ConnectTo>>) {
    for client in q_client.iter() {
        client.send_to(
            "I can send message to specify socket".as_bytes(),
            "udp://127.0.0.1:6001",
        );
    }
}
