use std::time::Duration;

use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, time::common_conditions::on_timer,
};

use bevy_com::{component::ConnectTo, prelude::*, udp::UdpNode};
use bevy_com::decoder::{AppMessageDecoder, DecodeWorker};
use bevy_com::decoder::serde_json::{ SerdeJsonProvider};

use crate::shared::{handle_error_events, PlayerInformation};

mod shared;

#[derive(Component)]
struct ClientMarker;

#[derive(Component)]
struct ServerMarker;

fn main() {
    App::new()
        .add_plugins(LogPlugin {
            filter: "bevy_com=debug".to_string(),
            ..default()
        })
        .add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            ))),
        )
        .add_plugins(BevyComPlugin)
        .register_decoder::<PlayerInformation, SerdeJsonProvider>()
        .add_systems(Startup, (setup_clients, setup_server))
        .add_systems(
            Update,
            send_unicast_messages.run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, (receive_raw_messages, handle_error_events))
        .run();
}

fn setup_clients(mut commands: Commands) {
    // send unicast udp
    commands.spawn((
        UdpNode::new("0.0.0.0:5001"),
        ConnectTo::new("127.0.0.1:6001"),
        ClientMarker,
    ));
    commands.spawn((
        UdpNode::default(),
        ConnectTo::new("127.0.0.1:6001"),
        ClientMarker,
    ));
    commands.spawn((UdpNode::default(), ClientMarker));
}

fn setup_server(mut commands: Commands) {
    commands.spawn((
        UdpNode::new("0.0.0.0:6001"),
        ServerMarker,
        DecodeWorker::<PlayerInformation, SerdeJsonProvider>::new(),
    ));
}

fn send_unicast_messages(q_client: Query<(&NetworkNode, Option<&ConnectTo>), With<ClientMarker>>) {
    for (client, opt_connect) in q_client.iter() {
        if opt_connect.is_some() {
            client.send(
                serde_json::to_string(&PlayerInformation {
                    health: 100,
                    position: (0, 0, 1),
                }).unwrap().as_bytes()
            );
        } else {
            client.send_to(
                "I can send message to specify socket ".as_bytes(),
                "127.0.0.1:6001",
            );
        }
    }
}

fn receive_raw_messages(q_server: Query<(&UdpNode, &NetworkNode), With<ServerMarker>>) {
    // for (udp_node, network_node) in q_server.iter() {
    //     while let Ok(Some(packet)) = network_node.message_receiver().try_recv() {
    //         println!("{} Received: {:?}", udp_node, packet);
    //     }
    // }
}
