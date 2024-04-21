use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

use bevy_com::component::ConnectTo;
use bevy_com::prelude::*;
use bevy_com::udp::{UdpNode, UdpNodeBuilder};

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
        .add_systems(
            Update,
            send_messages.run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .run();
}

#[derive(Component)]
struct Broadcast;

fn setup_clients(mut commands: Commands) {
    // bind and connect to remote addr
    commands.spawn((UdpNode::new("0.0.0.0:5001"), ConnectTo::new("0.0.0.0:6001")));
    commands.spawn((UdpNode::default(), ConnectTo::new("0.0.0.0:6001")));
    commands.spawn(UdpNode::default());

    // this will spawn an udp node can  broadcast message
    commands.spawn((
        UdpNodeBuilder::new()
            .with_addrs("0.0.0.0:5002")
            .with_broadcast(true)
            .build(),
        ConnectTo::new("255.255.255.255:6003"),
    ));
}

fn send_messages(q_client: Query<(&UdpNode, Option<&ConnectTo>)>) {
    for (client, opt_connect_to) in q_client.iter() {
        if opt_connect_to.is_some() {
            client.send("Hello World".as_bytes());
        } else {
            client.send_to("Hello World2".as_bytes(), "0.0.0.0:6002");
        }
    }
}
