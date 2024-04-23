use std::time::Duration;

use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, time::common_conditions::on_timer,
};

use bevy_com::{
    component::ConnectTo,
    prelude::*,
    udp::UdpNodeBuilder,
};

use crate::shared::*;

mod shared;

#[derive(Component)]
struct ClientMarker;

#[derive(Component)]
struct ServerMarker;

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
            .with_addrs("0.0.0.0:5002")
            .with_broadcast(true)
            .build(),
        ConnectTo::new("255.255.255.255:6002"),
    ));

    commands.spawn((
        ClientMarker,
        UdpNodeBuilder::new()
            .with_addrs("0.0.0.0:5012")
            .with_broadcast(true)
            .build(),
    ));
}

fn setup_server(mut commands: Commands) {
    let broadcast_receiver = UdpNodeBuilder::new()
        .with_addrs("0.0.0.0:6002")
        .with_broadcast(true)
        .build();
    commands.spawn((broadcast_receiver, ServerMarker));
}

fn send_broadcast_messages(
    q_client: Query<(&NetworkNode, Option<&ConnectTo>), With<ClientMarker>>,
) {
    for (client, opt_connect) in q_client.iter() {
        if opt_connect.is_some() {
            client.send(
                &bincode::serialize(&PlayerInformation {
                    health: 100,
                    position: (0, 0, 1),
                })
                .unwrap(),
            );
        } else {
            client.send_to(
                "I can send message to specify socket ".as_bytes(),
                "127.0.0.1:6002",
            );
        }
    }
}
