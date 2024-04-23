use std::time::Duration;

use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, time::common_conditions::on_timer,
};

use bevy_com::prelude::*;

use crate::shared::*;

mod shared;


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
            send_broadcast_messages.run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(Update, (receive_raw_messages, handle_error_events))
        .run();
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((
        ClientMarker,
        UdpNodeBuilder::new()
            .with_addrs("0.0.0.0:50002")
            .with_broadcast(true)

            .build(),
    ));
}

fn setup_server(mut commands: Commands) {
    let broadcast_receiver = UdpNodeBuilder::new()
        .with_addrs("0.0.0.0:60002")
        .with_broadcast(true)
        .build();
    commands.spawn((broadcast_receiver, ServerMarker, RawPacketMarker));
}

fn send_broadcast_messages(q_client: Query<&NetworkNode, With<ClientMarker>>) {
    for net_code in q_client.iter() {

        net_code.send_to(b"I can send broadcast message", "255.255.255.255:60002");

    }
}
