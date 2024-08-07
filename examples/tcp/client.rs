use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_octopus::{prelude::*, transports::tcp::TcpAddress};

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
                client_send_raw_message_to_channel::<TcpAddress>,
                send_json_message,
                send_bincode_message,
            )
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, (handle_raw_packet, handle_message_events))
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        NetworkBundle::new(RAW_CHANNEL),
        ClientNode(TcpAddress::new("127.0.0.1:5003")),
    ));
    commands.spawn((
        NetworkBundle::new(JSON_CHANNEL),
        ClientNode(TcpAddress::new("127.0.0.1:5004")),
    ));
    commands.spawn((
        NetworkBundle::new(BINCODE_CHANNEL),
        ClientNode(TcpAddress::new("127.0.0.1:5005")),
    ));
}
