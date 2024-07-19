use std::{net::Ipv4Addr, time::Duration};

use bevy::{prelude::*, time::common_conditions::on_timer};

use crate::common::*;
use bevy_octopus::{
    prelude::*,
    transports::udp::{MulticastV4Setting, UdpAddress, UdpBroadcast},
};

#[path = "../common/lib.rs"]
mod common;

#[derive(Component)]
struct BroadcastMarker;

#[derive(Component)]
struct MulticastMarker;

pub const BROADCAST_CHANNEL: ChannelId = ChannelId("broadcast");
pub const MULTICAST_CHANNEL: ChannelId = ChannelId("multicast");

fn main() {
    let mut app = App::new();
    shared_setup(&mut app);
    app.add_systems(Startup, (setup_clients, setup_server))
        .add_systems(
            Update,
            (send_broadcast_messages, send_multicast_messages)
                .run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, (handle_raw_packet, handle_raw_packet))
        .run();
}

fn setup_server(mut commands: Commands) {
    // broadcast udp receiver
    commands.spawn((
        NetworkBundle::new(BROADCAST_CHANNEL),
        Server(UdpAddress::new("0.0.0.0:60002")),
        UdpBroadcast,
    ));

    // multicast udp receiver
    commands.spawn((
        NetworkBundle::new(MULTICAST_CHANNEL),
        Server(UdpAddress::new("0.0.0.0:60003")),
        MulticastV4Setting::new(Ipv4Addr::new(239, 1, 2, 3), Ipv4Addr::UNSPECIFIED),
    ));
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        NetworkBundle::new(BROADCAST_CHANNEL),
        Server(UdpAddress::new("0.0.0.0:0")),
        Client(UdpAddress::new("255.255.255.255:60002")),
        UdpBroadcast,
        BroadcastMarker,
    ));

    commands.spawn((
        NetworkBundle::new(BROADCAST_CHANNEL),
        Server(UdpAddress::new("0.0.0.0:0")),
        UdpBroadcast,
        BroadcastMarker,
    ));

    commands.spawn((
        NetworkBundle::new(MULTICAST_CHANNEL),
        Server(UdpAddress::new("0.0.0.0:60005")),
        MulticastV4Setting::new(Ipv4Addr::new(239, 1, 2, 3), Ipv4Addr::UNSPECIFIED),
        MulticastMarker,
    ));
}

#[allow(clippy::type_complexity)]
fn send_broadcast_messages(
    q_client: Query<
        (
            &NetworkNode,
            &Server<UdpAddress>,
            Option<&Client<UdpAddress>>,
        ),
        With<BroadcastMarker>,
    >,
) {
    for (net_node, local_addr, opt_remote_addr) in q_client.iter() {
        if let Some(remote_addr) = opt_remote_addr {
            net_node.send_bytes_to(
                format!(
                    "broadcast message from {} with send_to {}",
                    local_addr.socket_addr, remote_addr.socket_addr
                )
                .as_bytes(),
                remote_addr.to_string(),
            );
        } else {
            net_node.send_bytes_to(
                format!(
                    "broadcast message from {} with send_to",
                    local_addr.socket_addr
                )
                .as_bytes(),
                "255.255.255.255:60002",
            );
        }
    }
}

#[allow(clippy::type_complexity)]
fn send_multicast_messages(
    q_client: Query<
        (
            &NetworkNode,
            &Server<UdpAddress>,
            Option<&Client<UdpAddress>>,
        ),
        With<MulticastMarker>,
    >,
) {
    for (net_node, local_addr, opt_remote_addr) in q_client.iter() {
        if let Some(remote_addr) = opt_remote_addr {
            net_node.send_bytes_to(
                format!("multicast message from {}", local_addr.socket_addr).as_bytes(),
                remote_addr.to_string(),
            );
        } else {
            net_node.send_bytes_to(
                format!("multicast message from {}", local_addr.socket_addr).as_bytes(),
                "239.1.2.3:60003",
            );
        }
    }
}
