use std::{net::Ipv4Addr, time::Duration};

use bevy::{prelude::*, time::common_conditions::on_timer};

use bevy_octopus::{
    channels::ChannelId,
    network::{ConnectTo, ListenTo},
    network_node::NetworkNode,
    transports::udp::{MulticastV4Setting, UdpBroadcast},
};

use crate::common::*;

#[path = "../common/lib.rs"]
mod common;

#[derive(Component)]
struct BroadcastMarker;

#[derive(Component)]
struct MulticastMarker;

pub const BROADCAST_CHANNEL: ChannelId = ChannelId("broadcast channel");
pub const MULTICAST_CHANNEL: ChannelId = ChannelId("multicast channel");

fn main() {
    let mut app = App::new();
    shared_setup(&mut app);
    app.add_systems(Startup, (setup_clients, setup_server))
        .add_systems(
            Update,
            (send_broadcast_messages, send_multicast_messages)
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(
            Update,
            (handle_raw_packet, handle_raw_packet, handle_node_events),
        )
        .run();
}

fn setup_server(mut commands: Commands) {
    // broadcast udp receiver
    commands.spawn((
        BROADCAST_CHANNEL,
        UdpBroadcast,
        ListenTo::new("udp://127.0.0.1:60002"),
    ));

    // multicast udp receiver
    commands.spawn((
        MULTICAST_CHANNEL,
        MulticastV4Setting::new(Ipv4Addr::new(239, 1, 2, 3), Ipv4Addr::UNSPECIFIED),
        ListenTo::new("udp://0.0.0.0:60003"),
    ));
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        BROADCAST_CHANNEL,
        UdpBroadcast,
        ConnectTo::new("udp://255.255.255.255:60002"),
        BroadcastMarker,
    ));

    commands.spawn((
        BROADCAST_CHANNEL,
        UdpBroadcast,
        ListenTo::new("udp://0.0.0.0:0"),
        // example marker for query filter
        BroadcastMarker,
    ));

    commands.spawn((
        MULTICAST_CHANNEL,
        ListenTo::new("udp://0.0.0.0:0"),
        MulticastV4Setting::new(Ipv4Addr::new(239, 1, 2, 3), Ipv4Addr::UNSPECIFIED),
        // example marker foClientMarker,
        MulticastMarker,
    ));
}

fn send_broadcast_messages(
    q_client: Query<(&NetworkNode, &ListenTo, Option<&ConnectTo>), With<BroadcastMarker>>,
) {
    for (net_node, local_addr, opt_remote_addr) in q_client.iter() {
        if opt_remote_addr.is_some() {
            net_node.send(format!("broadcast message from {}", local_addr.0).as_bytes());
        } else {
            net_node.send_to(
                format!("broadcast message from {} with send_to", local_addr.0).as_bytes(),
                "udp://255.255.255.255:60002",
            );
        }
    }
}

fn send_multicast_messages(
    q_client: Query<(&NetworkNode, &ListenTo, Option<&ConnectTo>), With<MulticastMarker>>,
) {
    for (net_node, local_addr, opt_remote_addr) in q_client.iter() {
        if opt_remote_addr.is_some() {
            net_node.send(format!("multicast message from {}", local_addr.0).as_bytes());
        } else {
            net_node.send_to(
                format!("multicast message from {}", local_addr.0).as_bytes(),
                "udp://239.1.2.3:60003",
            );
        }
    }
}
