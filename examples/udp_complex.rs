use std::net::Ipv4Addr;
use std::time::Duration;

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_ecs_net::{
    network::{LocalSocket, RemoteSocket},
    network_manager::NetworkNode,
    shared::NetworkProtocol,
    udp::{MulticastV4Setting, UdpBroadcast},
};

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

fn setup_server(mut commands: Commands) {
    // broadcast udp receiver
    commands.spawn((
        NetworkProtocol::UDP,
        UdpBroadcast,
        LocalSocket::new("0.0.0.0:60002"),
    ));

    // multicast udp receiver
    commands.spawn((
        NetworkProtocol::UDP,
        MulticastV4Setting::new(Ipv4Addr::new(239, 1, 2, 3), Ipv4Addr::UNSPECIFIED),
        LocalSocket::new("0.0.0.0:60003"),
    ));
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        NetworkProtocol::UDP,
        UdpBroadcast,
        RemoteSocket::new("255.255.255.255:60002"),
        // example marker for query filter
        ClientMarker,
        BroadcastMarker,
    ));

    commands.spawn((
        NetworkProtocol::UDP,
        UdpBroadcast,
        // example marker for query filter
        ClientMarker,
        BroadcastMarker,
    ));

    commands.spawn((
        NetworkProtocol::UDP,
        MulticastV4Setting::new(Ipv4Addr::new(239, 1, 2, 3), Ipv4Addr::UNSPECIFIED),
        // example marker for query filter
        ClientMarker,
        MulticastMarker,
    ));
}

fn send_broadcast_messages(
    q_client: Query<
        (&NetworkNode, &LocalSocket, Option<&RemoteSocket>),
        (With<ClientMarker>, With<BroadcastMarker>),
    >,
) {
    for (net_node, local_addr, opt_remote_addr) in q_client.iter() {
        if opt_remote_addr.is_some() {
            net_node.send(format!("broadcast message from {}", local_addr.0).as_bytes());
        } else {
            net_node.send_to(
                format!("broadcast message from {} with send_to", local_addr.0).as_bytes(),
                "255.255.255.255:60002",
            );
        }
    }
}

fn send_multicast_messages(
    q_client: Query<
        (&NetworkNode, &LocalSocket, Option<&RemoteSocket>),
        (With<ClientMarker>, With<MulticastMarker>),
    >,
) {
    for (net_node, local_addr, opt_remote_addr) in q_client.iter() {
        if opt_remote_addr.is_some() {
            net_node.send(format!("multicast message from {}", local_addr.0).as_bytes());
        } else {
            net_node.send_to(
                format!("multicast message from {}", local_addr.0).as_bytes(),
                "239.1.2.3:60003",
            );
        }
    }
}
