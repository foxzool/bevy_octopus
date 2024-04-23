use std::{net::Ipv4Addr, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, time::common_conditions::on_timer,
};

use bevy_com::{
    prelude::*,
    udp::{MulticastV4Setting, UdpNode},
};

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
        .add_systems(Startup, (setup_clients, setup_server))
        .add_systems(
            Update,
            send_multicast_messages.run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, (receive_raw_messages, handle_error_events))
        .run();
}

fn setup_clients(mut commands: Commands) {
    let multicast_setting = MulticastV4Setting {
        multi_addr: Ipv4Addr::new(224, 0, 0, 1),
        interface: Ipv4Addr::UNSPECIFIED,
    };
    commands.spawn((
        ClientMarker,
        UdpNode::new("0.0.0.0:5003"),
        multicast_setting,
    ));
}

fn setup_server(mut commands: Commands) {
    let multicast_setting = MulticastV4Setting {
        multi_addr: Ipv4Addr::new(224, 0, 0, 2),
        interface: Ipv4Addr::UNSPECIFIED,
    };
    commands.spawn((
        UdpNode::new("0.0.0.0:6004"),
        multicast_setting,
        ServerMarker,
    ));
}

fn send_multicast_messages(q_client: Query<&NetworkNode, With<ClientMarker>>) {
    for client in q_client.iter() {
        client.send_to(
            &bincode::serialize(&PlayerInformation {
                health: 100,
                position: (0, 0, 1),
            })
            .unwrap(),
            "224.0.0.2:6004",
        );
    }
}

fn receive_raw_messages(q_server: Query<(&UdpNode, &NetworkNode), With<ServerMarker>>) {
    for (udp_node, network_node) in q_server.iter() {
        while let Ok(Some(packet)) = network_node.message_receiver().try_recv() {
            println!("{} Received: {:?}", udp_node, packet);
        }
    }
}
