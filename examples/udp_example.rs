use std::time::Duration;

use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, time::common_conditions::on_timer,
};

use bevy_com::{
    component::ConnectTo,
    decoder::{AppMessageDecoder, DecodeWorker, serde_json::SerdeJsonProvider},
    prelude::*,
    udp::UdpNode,
};

use crate::shared::{handle_error_events, handle_message_events, PlayerInformation};

mod shared;

#[derive(Component)]
struct ClientMarker;

#[derive(Component)]
struct ServerMarker;

#[derive(Component)]
struct RawPacketMarker;

#[derive(Component)]
struct JsonMarker;

#[derive(Component)]
struct BincodeMarker;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            ))),
            LogPlugin {
                filter: "bevy_com=debug".to_string(),
                ..default()
            },
        ))
        .add_plugins(BevyComPlugin)
        .register_decoder::<PlayerInformation, SerdeJsonProvider>()
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
        UdpNode::new("0.0.0.0:5001"),
        ConnectTo::new("127.0.0.1:6001"),
        ClientMarker,
        // marker to send json
        JsonMarker,
    ));
    // or listen to rand port
    commands.spawn((
        UdpNode::default(),
        ConnectTo::new("127.0.0.1:6002"),
        ClientMarker,
        // marker to send bincode
        BincodeMarker,
    ));
    // ConnectTo is not necessary component
    commands.spawn((UdpNode::default(), ClientMarker));

    // this is an udp node to send raw bytes;
    commands.spawn((UdpNode::default(), ClientMarker, RawPacketMarker));
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

/// if you recv message directly from receiver,  typed message to wait handled may be missed
fn receive_raw_messages(
    q_server: Query<(&UdpNode, &NetworkNode), (With<ServerMarker>, With<RawPacketMarker>)>,
) {
    for (udp_node, network_node) in q_server.iter() {
        while let Ok(Some(packet)) = network_node.recv_channel().receiver.try_recv() {
            info!("{} Received: {:?}", udp_node, String::from_utf8(packet.bytes.to_vec()));
        }
    }
}
