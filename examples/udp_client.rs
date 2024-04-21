use std::{net::Ipv4Addr, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, time::common_conditions::on_timer,
};

use bevy_com::{
    component::ConnectTo,
    prelude::*,
    udp::{MulticastV4Setting, UdpNode, UdpNodeBuilder},
};

use crate::shared::PlayerInformation;

mod shared;

#[derive(Component)]
struct UnicastUdpMarker;

#[derive(Component)]
struct BroadcastUdpMarker;

#[derive(Component)]
struct MulticastUdpMarker;

#[derive(Component)]
struct TypedUdpMarker;

fn main() {
    App::new()
        .add_plugins(LogPlugin::default())
        .add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            ))),
        )
        .add_plugins(BevyComPlugin)
        .add_systems(Startup, setup_clients)
        .add_systems(
            Update,
            (
                send_unicast_messages,
                send_broadcast_messages,
                send_multicast_messages,
                send_typed_messages,
            )
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .run();
}

fn setup_clients(mut commands: Commands) {
    // send unicast udp
    commands.spawn((
        UnicastUdpMarker,
        UdpNode::new("0.0.0.0:5001"),
        ConnectTo::new("0.0.0.0:6001"),
    ));
    commands.spawn((
        UnicastUdpMarker,
        UdpNode::default(),
        ConnectTo::new("0.0.0.0:6001"),
    ));
    commands.spawn((UnicastUdpMarker, UdpNode::default()));
    commands.spawn((
        TypedUdpMarker,
        UdpNode::default(),
        ConnectTo::new("0.0.0.0:6005"),
    ));

    // this will spawn an udp node can  broadcast message
    commands.spawn((
        BroadcastUdpMarker,
        UdpNodeBuilder::new()
            .with_addrs("0.0.0.0:5002")
            .with_broadcast(true)
            .build(),
        ConnectTo::new("255.255.255.255:6003"),
    ));

    // this will spawn an udp node can  multicast message
    let multicast_setting = MulticastV4Setting {
        multi_addr: Ipv4Addr::new(224, 0, 0, 1),
        interface: Ipv4Addr::UNSPECIFIED,
    };
    commands.spawn((
        MulticastUdpMarker,
        UdpNode::new("0.0.0.0:5003"),
        multicast_setting,
    ));
}

fn send_unicast_messages(q_client: Query<(&NetworkNode, Option<&ConnectTo>), With<UnicastUdpMarker>>) {
    for (client, opt_connect) in q_client.iter() {
        if opt_connect.is_some() {
            client.send("I can send unicast message to connect".as_bytes());
        } else {
            client.send_to(
                "I can send message to specify socket ".as_bytes(),
                "0.0.0.0:6002",
            );
        }
    }
}

fn send_broadcast_messages(q_client: Query<&NetworkNode, With<BroadcastUdpMarker>>) {
    for client in q_client.iter() {
        client.send("I can broadcast message".as_bytes());
    }
}

fn send_multicast_messages(q_client: Query<&NetworkNode, With<MulticastUdpMarker>>) {
    for client in q_client.iter() {
        client.send_to(
            "I can send multicast message to".as_bytes(),
            "224.0.0.2:6004",
        );
    }
}

fn send_typed_messages(q_client: Query<&NetworkNode, With<TypedUdpMarker>>) {
    for client in q_client.iter() {
        client.send(
            &bincode::serialize(&PlayerInformation {
                health: 100,
                position: (0, 0, 1),
            })
                .unwrap(),
        );
    }
}
