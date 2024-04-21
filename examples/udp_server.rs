#![allow(missing_docs)]

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;

use bevy_com::prelude::*;
use bevy_com::udp::UdpNode;

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
        .add_systems(Startup, setup_servers)
        .add_systems(Update, (close_and_restart, receive_raw_messages))
        .run();
}

fn setup_servers(mut commands: Commands) {
    commands.spawn(UdpNode::new("0.0.0.0:6001"));
    commands.spawn(UdpNode::new("0.0.0.0:6002"));
    // commands.spawn(UdpServerNode::new("udp_server_1", "0.0.0.0:6001"));
    // commands.spawn(UdpServerNode::new("udp_server_2", "0.0.0.0:6002"));
}

fn close_and_restart(time: Res<Time>, mut q_server: Query<(Entity, &mut UdpNode)>) {
    if time.elapsed_seconds() > 2.0 && time.elapsed_seconds() < 3.0 {
        for (_e, mut server) in q_server.iter_mut() {
            if server.is_running() {
                server.stop();
            }
        }
    }

    if time.elapsed_seconds() > 4.0 {
        for (_e, mut server) in q_server.iter_mut() {
            if !server.is_running() {
                server.start();
            }
        }
    }
}

fn receive_raw_messages(q_server: Query<&UdpNode>) {
    for server in q_server.iter() {
        while let Ok(Some(packet)) = server.receiver().try_recv() {
            println!("Received: {:?}", packet);
        }
    }
}
