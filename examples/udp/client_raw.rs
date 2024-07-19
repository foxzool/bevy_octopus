use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_octopus::{prelude::*, transports::udp::UdpAddress};

use crate::common::*;

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
        ServerNode(UdpAddress::new("0.0.0.0:7006")),
        ClientNode(UdpAddress::new("127.0.0.1:6001")),
    ));
    commands.spawn((
        NetworkBundle::new(RAW_CHANNEL),
        ServerNode(UdpAddress::new("0.0.0.0:0")),
        ClientNode(UdpAddress::new("127.0.0.1:6001")),
    ));
    commands.spawn((
        NetworkBundle::new(RAW_CHANNEL),
        ServerNode(UdpAddress::new("0.0.0.0:0")),
        ClientNode(UdpAddress::new("127.0.0.1:6001")),
    ));
}

fn send_socket_packet(q_node: Query<&NetworkNode, With<ServerNode<UdpAddress>>>) {
    for node in q_node.iter() {
        node.send_bytes_to(
            "I can send message to specify socket".as_bytes(),
            "127.0.0.1:6002",
        );
    }
}
