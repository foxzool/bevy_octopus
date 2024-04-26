use bevy::prelude::*;

use bevy_ecs_net::decoder::{AppMessageDecoder, DecodeWorker};
use bevy_ecs_net::prelude::{BincodeProvider, SerdeJsonProvider};
use bevy_ecs_net::tcp::TcpServerNode;

use crate::common::*;

#[path = "common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();

    shared_setup(&mut app);

    app.register_decoder::<PlayerInformation, SerdeJsonProvider>()
        .register_decoder::<PlayerInformation, BincodeProvider>()
        .add_systems(Startup, setup_server)
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

fn setup_server(mut commands: Commands) {
    let tcp_node = TcpServerNode::new("0.0.0.0:6003");
    commands.spawn((tcp_node, ServerMarker, RawPacketMarker));

    let tcp_node = TcpServerNode::new("0.0.0.0:6004");
    commands.spawn((
        tcp_node,
        ServerMarker,
        DecodeWorker::<PlayerInformation, SerdeJsonProvider>::new(),
    ));
    let tcp_node = TcpServerNode::new("0.0.0.0:6005");
    commands.spawn((
        tcp_node,
        ServerMarker,
        DecodeWorker::<PlayerInformation, BincodeProvider>::new(),
    ));
}
