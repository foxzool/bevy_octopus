use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use std::time::Duration;

use bevy_com::prelude::*;

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
        .add_systems(Startup, setup_clients)
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn(UdpClientNode::new("udp client 1", "0.0.0.0:6001"));
}
