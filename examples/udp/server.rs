use bevy::prelude::*;

use bevy_octopus::prelude::*;

use crate::common::*;

#[path = "../common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();
    shared_setup(&mut app);

    app.add_transformer::<PlayerInformation, JsonTransformer>(JSON_CHANNEL)
        .add_transformer::<PlayerInformation, BincodeTransformer>(BINCODE_CHANNEL)
        .add_systems(Startup, setup_server)
        .add_systems(
            Update,
            (handle_raw_packet, handle_message_events, handle_node_events),
        )
        .run();
}

fn setup_server(mut commands: Commands) {
    commands.spawn((RAW_CHANNEL, ListenTo::new("udp://127.0.0.1:6001")));
    commands.spawn((JSON_CHANNEL, ListenTo::new("udp://127.0.0.1:6002")));
    commands.spawn((BINCODE_CHANNEL, ListenTo::new("udp://127.0.0.1:6003")));
}
