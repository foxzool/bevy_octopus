#![allow(missing_docs)]

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;

use bevy_com::prelude::*;
use bevy_com::udp::{UdpServerNode, UdpServerSetting};

fn main() {
    App::new()
        .add_plugins(LogPlugin::default())
        .add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            ))),
        )
        .add_plugins(BevyComPlugin)
        .add_systems(Startup, setup_udp)
        .add_systems(Update, close_udp_server)
        .run();
}

fn setup_udp(mut commands: Commands) {
    commands.spawn(UdpServerSetting::new("udp_server_1", "0.0.0.0:6001"));
}

fn close_udp_server(
    time: Res<Time>,
    mut q_server: Query<(Entity, &mut UdpServerNode)>,
    mut commands: Commands,
) {
    if time.elapsed_seconds() > 5.0 {
        for (e, mut server) in q_server.iter_mut() {
            if server.is_running() {
                println!("Closing UDP server: {:?}", e);
                server.shutdown();
            }
        }
    }
}
