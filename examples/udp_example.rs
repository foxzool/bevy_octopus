use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_com::{
    decoder::{AppMessageDecoder, DecodeWorker, serde_json::SerdeJsonProvider},
    prelude::*,
    udp::UdpNode,
};

use crate::shared::*;

mod shared;

fn main() {
    let mut app = App::new();
    shared_setup(&mut app);

    app.register_decoder::<PlayerInformation, SerdeJsonProvider>()
        .add_systems(Startup, (setup_clients, setup_server))
        .add_systems(
            Update,
            (send_typed_messages, send_raw_messages).run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(
            Update,
            (
                receive_raw_messages,
                handle_message_events,
                handle_error_events,
            ),
        )
        .run();
}

fn setup_clients(mut commands: Commands) {
    // udp listen to specify  port and connect to remote
    commands.spawn((
        UdpNode::new_with_peer("0.0.0.0:5001", "127.0.0.1:6001"),
        ClientMarker,
        // marker to send json
        JsonMarker,
    ));
    // or listen to rand port
    commands.spawn((
        UdpNode::new_with_peer("0.0.0.0:0", "127.0.0.1:6002"),
        ClientMarker,
        // marker to send bincode
        BincodeMarker,
    ));
    // ConnectTo is not necessary component
    commands.spawn((UdpNode::default(), ClientMarker));

    // this is an udp node to send raw bytes;
    commands.spawn((UdpNode::new("0.0.0.0:5002"), ClientMarker, RawPacketMarker));
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        UdpNode::new("0.0.0.0:6001"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, SerdeJsonProvider>::new(),
    ));

    commands.spawn((
        UdpNode::new("0.0.0.0:6002"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, BincodeProvider>::new(),
    ));

    commands.spawn((UdpNode::new("0.0.0.0:6003"), ServerMarker, RawPacketMarker));
}

fn send_typed_messages(
    q_client: Query<
        (&NetworkNode, Option<&JsonMarker>, Option<&BincodeMarker>),
        (With<ClientMarker>, Without<RawPacketMarker>),
    >,
) {
    for (client, opt_json, opt_bincode) in q_client.iter() {
        if let (false, true) = (opt_json.is_some(), opt_bincode.is_some()) {
            client.send(
                &bincode::serialize(&PlayerInformation {
                    health: 100,
                    position: (0, 0, 1),
                })
                .unwrap(),
            );
        } else {
            client.send(
                serde_json::to_string(&PlayerInformation {
                    health: 100,
                    position: (0, 0, 1),
                })
                .unwrap()
                .as_bytes(),
            );
        }
    }
}

fn send_raw_messages(q_client: Query<&NetworkNode, (With<ClientMarker>, With<RawPacketMarker>)>) {
    for client in q_client.iter() {
        client.send_to(
            "I can send message to specify socket".as_bytes(),
            "127.0.0.1:6003",
        );
    }
}
