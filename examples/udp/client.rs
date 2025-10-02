use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_octopus::{prelude::*, transports::udp::UdpAddress};

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
            (send_json_message, send_bincode_message)
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, (handle_raw_packet, handle_message_events))
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        NetworkBundle::new(JSON_CHANNEL),
        ServerNode(UdpAddress::new("0.0.0.0:0")),
        ClientNode(UdpAddress::new("127.0.0.1:6002")),
    ));
    commands.spawn((
        NetworkBundle::new(BINCODE_CHANNEL),
        ServerNode(UdpAddress::new("0.0.0.0:0")),
        ClientNode(UdpAddress::new("127.0.0.1:6003")),
    ));
}
