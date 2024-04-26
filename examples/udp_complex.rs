use std::{net::Ipv4Addr, time::Duration};

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_ecs_net::prelude::*;

use crate::common::*;

#[path = "common/lib.rs"]
mod common;

#[derive(Component)]
struct BroadcastMarker;

#[derive(Component)]
struct MulticastMarker;

fn main() {
    let mut app = App::new();
    shared_setup(&mut app);
    app.add_systems(Startup, (setup_clients, setup_server))
        .add_systems(
            Update,
            (send_broadcast_messages, send_multicast_messages)
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, (receive_raw_messages, handle_node_events))
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        ClientMarker,
        BroadcastMarker,
        UdpNodeBuilder::new()
            .with_addrs("0.0.0.0:50002")
            .with_broadcast(true)
            .build(),
    ));

    commands.spawn((
        ClientMarker,
        MulticastMarker,
        UdpNodeBuilder::new()
            .with_addrs("0.0.0.0:0")
            .with_multicast_v4(Ipv4Addr::new(224, 0, 0, 1), Ipv4Addr::UNSPECIFIED)
            .build(),
    ));
}

fn setup_server(mut commands: Commands) {
    let broadcast_receiver = UdpNodeBuilder::new()
        .with_addrs("0.0.0.0:60002")
        .with_broadcast(true)
        .build();
    commands.spawn((broadcast_receiver, ServerMarker, RawPacketMarker));

    let multicast_receiver = UdpNodeBuilder::new()
        .with_addrs("0.0.0.0:60003")
        .with_multicast_v4(Ipv4Addr::new(224, 0, 0, 1), Ipv4Addr::UNSPECIFIED)
        .build();
    commands.spawn((multicast_receiver, ServerMarker, RawPacketMarker));
}

fn send_broadcast_messages(
    q_client: Query<&NetworkNode, (With<ClientMarker>, With<BroadcastMarker>)>,
) {
    for net_node in q_client.iter() {
        net_node.send_to(
            format!("broadcast message from {}", net_node),
            "255.255.255.255:60002",
        );
    }
}

fn send_multicast_messages(
    q_client: Query<&NetworkNode, (With<ClientMarker>, With<MulticastMarker>)>,
) {
    for net_node in q_client.iter() {
        net_node.send_to(format!("multicast message from {}", net_node), "224.0.0.1:60003");
    }
}
