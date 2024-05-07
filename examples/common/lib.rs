#![allow(dead_code)]

use std::ops::Deref;

use bevy::{log::LogPlugin, prelude::*};
use serde::{Deserialize, Serialize};

use bevy_octopus::{
    network::NetworkData,
    network_node::NetworkNode,
    shared::NetworkNodeEvent,
};
use bevy_octopus::connections::NetworkPeer;
use bevy_octopus::prelude::*;

/// shared app setup
#[cfg(not(feature = "inspect"))]
pub fn shared_setup(app: &mut App) {
    app.add_plugins((
        MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        )),
        LogPlugin {
            filter: "bevy_octopus=debug".to_string(),
            ..default()
        },
    ))
        .add_plugins(OctopusPlugin);
}

#[cfg(feature = "inspect")]
pub fn shared_setup(app: &mut App) {
    app.add_plugins(DefaultPlugins.set(LogPlugin {
        filter: "bevy_octopus=debug".to_string(),
        ..default()
    }))
        .add_plugins(bevy_inspector_egui::quick::WorldInspectorPlugin::new())
        .add_plugins(OctopusPlugin);
}

/// this channel is sending and receiving raw packet
pub const RAW_CHANNEL: ChannelId = ChannelId(1);

/// this channel is sending and receiving json packet
pub const JSON_CHANNEL: ChannelId = ChannelId(2);

/// this channel is sending and receiving bincode packet
pub const BINCODE_CHANNEL: ChannelId = ChannelId(3);

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerInformation {
    pub health: usize,
    pub position: (u32, u32, u32),
}


pub fn handle_node_events(
    mut new_network_events: EventReader<NetworkNodeEvent>,
    q_net_node: Query<(&ChannelId, &NetworkNode)>,
) {
    for event in new_network_events.read() {
        if let Ok((channel_id, net)) = q_net_node.get(event.node) {
            info!(
                "{} {:?} {} got event: {:?}",
                channel_id, event.node, net, event.event
            );
        } else {
            info!("{:?} got event: {:?}", event.node, event.event);
        }
    }
}

pub fn handle_message_events(
    mut new_network_events: EventReader<NetworkData<PlayerInformation>>,
    q_net_node: Query<(&ChannelId, &NetworkNode)>,
) {
    for event in new_network_events.read() {
        let (channel_id, net) = q_net_node.get(event.source).unwrap();
        let player_info = event.deref();
        info!("{} {} Received: {:?}", channel_id, net, &player_info);
    }
}

pub fn handle_raw_packet(q_server: Query<(&ChannelId, &NetworkNode)>) {
    for (channel_id, net_node) in q_server.iter() {
        while let Ok(Some(packet)) = net_node.recv_message_channel.receiver.try_recv() {
            info!("{} {} Received: {:?}", channel_id, net_node, packet.bytes);
        }
    }
}

#[cfg(feature = "serde_json")]
pub fn send_json_message(q_nodes: Query<(&NetworkNode, &ChannelId), With<NetworkPeer>>) {
    for (node, channel_id) in q_nodes.iter() {
        if channel_id == &JSON_CHANNEL {
            let player_info = PlayerInformation {
                health: 100,
                position: (1, 2, 3),
            };
            node.send(serde_json::to_string(&player_info).unwrap().as_bytes());
        }
    }
}

#[cfg(feature = "bincode")]
/// send bincode message
pub fn send_bincode_message(q_nodes: Query<(&NetworkNode, &ChannelId), With<NetworkPeer>>) {
    for (node, channel_id) in q_nodes.iter() {
        if channel_id == &BINCODE_CHANNEL {
            let player_info = PlayerInformation {
                health: 200,
                position: (4, 5, 6),
            };
            node.send(&bincode::serialize(&player_info).unwrap());
        }
    }
}

pub fn send_channel_message(mut channel_messages: EventWriter<ChannelMessage<PlayerInformation>>) {
    channel_messages.send(ChannelMessage {
        channel_id: JSON_CHANNEL,
        message: PlayerInformation {
            health: 300,
            position: (4, 5, 6),
        },
    });
}

/// send raw message to server
pub fn send_raw_message_to_channel(q_client: Query<(&NetworkNode, &ChannelId), With<NetworkPeer>>) {
    for (node, channel_id) in q_client.iter() {
        if channel_id == &RAW_CHANNEL {
            node.send(format!("raw packet from {} to {}", node, channel_id).as_bytes());
        }
    }
}
