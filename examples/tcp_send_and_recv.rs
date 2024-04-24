use std::time::Duration;
use crate::shared::{ClientMarker, handle_error_events, handle_message_events, RawPacketMarker, receive_raw_messages, ServerMarker, shared_setup};
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use bevy_ecs_net::network::NetworkNode;
use bevy_ecs_net::tcp::{TcpServerNode, TcpClientNode};

mod shared;

fn main() {
    let mut app = App::new();

    shared_setup(&mut app);

    app.add_systems(Startup, (setup_server, setup_clients))
        .add_systems(
            Update,
            (send_raw_messages).run_if(on_timer(Duration::from_secs_f64(1.0))),
        )
        .add_systems(
            Update,
            (
                receive_raw_messages,
                // handle_message_events,
                // handle_error_events,
            ),
        )
        .run()
}

fn setup_server(mut commands: Commands) {

    let tcp_node = TcpServerNode::new("0.0.0.0:6003");

    commands.spawn((tcp_node, ServerMarker, RawPacketMarker));
}

fn setup_clients(mut commands: Commands) {
    commands.spawn((TcpClientNode::new("127.0.0.1:6003"), ClientMarker, RawPacketMarker));
}

fn send_raw_messages(q_client: Query<&NetworkNode, (With<ClientMarker>, With<RawPacketMarker>)>) {
    for client in q_client.iter() {
        client.send(
            "I can send message to specify socket".as_bytes()
        );
    }
}