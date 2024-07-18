use bevy::prelude::*;

use crate::common::*;
use bevy_octopus::{prelude::*, transports::udp::UdpAddress};

#[path = "../common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();
    shared_setup(&mut app);

    app.add_transformer::<PlayerInformation, JsonTransformer>(JSON_CHANNEL)
        .add_transformer::<PlayerInformation, BincodeTransformer>(BINCODE_CHANNEL)
        .add_systems(Startup, setup_server)
        .add_systems(Update, (handle_raw_packet, handle_message_events))
        .run();
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        NetworkBundle::new(RAW_CHANNEL),
        Server(UdpAddress::new("127.0.0.1:6001")),
    ));
    commands.spawn((
        NetworkBundle::new(JSON_CHANNEL),
        Server(UdpAddress::new("127.0.0.1:6002")),
    ));
    commands.spawn((
        NetworkBundle::new(BINCODE_CHANNEL),
        Server(UdpAddress::new("127.0.0.1:6003")),
    ));
}
