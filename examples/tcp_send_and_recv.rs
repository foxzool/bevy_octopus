use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

use bevy_ecs_net::decoder::{AppMessageDecoder, DecodeWorker};
use bevy_ecs_net::prelude::{BincodeProvider, SerdeJsonProvider};
use bevy_ecs_net::tcp::{TcpClientNode, TcpServerNode};

use crate::common::*;

#[path = "common/lib.rs"]
mod common;

fn main() {
    let mut app = App::new();

    shared_setup(&mut app);

    app.register_decoder::<PlayerInformation, SerdeJsonProvider>()
        .register_decoder::<PlayerInformation, BincodeProvider>()
        .add_systems(Startup, (setup_server, setup_clients))
        .add_systems(
            Update,
            (send_raw_packet, send_json_packet, send_bincode_packet)
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(
            Update,
            (
                receive_raw_messages,
                handle_message_events,
                handle_error_events,
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

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        TcpClientNode::new("127.0.0.1:6003"),
        ClientMarker,
        RawPacketMarker,
    ));
    commands.spawn((
        TcpClientNode::new("127.0.0.1:6004"),
        ClientMarker,
        JsonMarker,
    ));
    commands.spawn((
        TcpClientNode::new("127.0.0.1:6005"),
        ClientMarker,
        BincodeMarker,
    ));
}
