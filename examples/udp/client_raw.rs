use std::time::Duration;

use crate::common::*;
use bevy::{prelude::*, time::common_conditions::on_timer};
use bevy_octopus::{prelude::*, transports::udp::UdpAddress};

#[path = "../common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();
    shared_setup(&mut app);

    app.add_systems(Startup, setup_clients)
        .add_systems(
            Update,
            (
                client_send_raw_message_to_channel::<UdpAddress>,
                send_socket_packet,
            )
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, handle_raw_packet)
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        NetworkBundle::new(RAW_CHANNEL),
        Server(UdpAddress::new("0.0.0.0:7006")),
        Client(UdpAddress::new("127.0.0.1:6001")),
    ));
    commands.spawn((
        NetworkBundle::new(RAW_CHANNEL),
        Server(UdpAddress::new("0.0.0.0:0")),
        Client(UdpAddress::new("127.0.0.1:6001")),
    ));
    commands.spawn((
        NetworkBundle::new(RAW_CHANNEL),
        Server(UdpAddress::new("0.0.0.0:0")),
        Client(UdpAddress::new("127.0.0.1:6001")),
    ));
}

fn send_socket_packet(q_node: Query<&NetworkNode, With<ServerAddr>>) {
    for node in q_node.iter() {
        node.send_bytes_to(
            "I can send message to specify socket".as_bytes(),
            "127.0.0.1:6002",
        );
    }
}
