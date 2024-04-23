#![allow(missing_docs)]

use std::{net::Ipv4Addr, time::Duration};

use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};

use bevy_com::{
    prelude::*,
    udp::{MulticastV4Setting, UdpNode, UdpNodeBuilder},
};

use crate::shared::PlayerInformation;

mod shared;

fn main() {
    App::new()
        .add_plugins(LogPlugin::default())
        .add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            ))),
        )
        .add_plugins(BevyComPlugin)
        .register_decoder::<PlayerInformation>()
        .add_systems(Startup, setup_servers)
        .add_systems(Update, (close_and_restart, receive_raw_messages))
        .run();
}

fn setup_servers(mut commands: Commands) {
    commands.spawn(UdpNode::new("0.0.0.0:6001"));
    commands.spawn((UdpNode::new("0.0.0.0:6002"), BitcodeDeserializer));

    // listen for broadcast messages
    commands.spawn(
        UdpNodeBuilder::new()
            .with_addrs("0.0.0.0:6003")
            .with_broadcast(true)
            .build(),
    );

    // listen for multicast messages
    let multicast_setting = MulticastV4Setting {
        multi_addr: Ipv4Addr::new(224, 0, 0, 2),
        interface: Ipv4Addr::UNSPECIFIED,
    };
    commands.spawn((UdpNode::new("0.0.0.0:6004"), multicast_setting));

    // handle typed messages
    commands.spawn((
        UdpNode::new("0.0.0.0:6005"),
        Decoder::<PlayerInformation>::new(),
    ));
}

fn close_and_restart(
    mut commands: Commands,
    time: Res<Time>,
    mut q_server: Query<(Entity, &mut UdpNode, &mut NetworkNode), With<UdpNode>>,
) {
    if time.elapsed_seconds() > 2.0 && time.elapsed_seconds() < 3.0 {
        for (e, mut udp, mut server) in q_server.iter_mut() {
            if server.running {
                // TODO
                udp.stop(&mut server);
            }
        }
    }

    if time.elapsed_seconds() > 4.0 {
        for (e, mut udp, mut server) in q_server.iter_mut() {
            if !server.running {
                udp.start(&mut server);
            }
        }
    }
}

#[derive(Component)]
struct BitcodeDeserializer;

// impl Decoder<String> for BitcodeDeserializer {
//     fn decode(bytes: Bytes) -> Result<String, NetworkError> where Self: Sized {
//         bincode::deserialize(&bytes[..]).map_err(|_| NetworkError::DeserializeError)
//     }
// }

fn receive_raw_messages(q_server: Query<(&UdpNode, &NetworkNode)>) {
    for (udp_node, network_node) in q_server.iter() {
        while let Ok(Some(packet)) = network_node.message_receiver().try_recv() {
            println!("{} Received: {:?}", udp_node, packet);
        }
    }
}
