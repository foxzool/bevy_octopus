#![allow(dead_code)]

use bevy::{
    log::{Level, LogPlugin},
    prelude::*,
};
use serde::{Deserialize, Serialize};

use bevy_octopus::prelude::*;
use bevy_octopus_websocket::{WebsocketAddress, WebsocketPlugin};

/// shared app setup
pub fn shared_setup(app: &mut App) {
    app.add_plugins((
        MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
            std::time::Duration::from_secs_f64(1.0 / 60.0),
        )),
        LogPlugin {
            level: Level::INFO,
            filter: "bevy_octopus=debug".to_string(),
            ..default()
        },
    ))
    .add_plugins(OctopusPlugin)
    .add_plugins(WebsocketPlugin)
    .add_observer(on_node_event);
}

/// this channel is sending and receiving raw packet
pub const RAW_CHANNEL: ChannelId = ChannelId("raw channel");

/// this channel is sending and receiving json packet
pub const JSON_CHANNEL: ChannelId = ChannelId("json channel");

/// this channel is sending and receiving bincode packet
pub const BINCODE_CHANNEL: ChannelId = ChannelId("bincode channel");

#[derive(Serialize, Deserialize, Debug)]
pub struct PlayerInformation {
    pub health: usize,
    pub position: (u32, u32, u32),
}

pub fn on_node_event(trigger: Trigger<NetworkEvent>) {
    info!("{:?} trigger {:?}", trigger.target(), trigger.event());
}

pub fn handle_message_events(
    mut ev_channels: EventReader<ReceiveChannelMessage<PlayerInformation>>,
) {
    for event in ev_channels.read() {
        info!("{} Received: {:?}", event.channel_id, &event.message);
    }
}

pub fn handle_raw_packet(q_server: Query<(&ChannelId, &NetworkNode)>) {
    for (channel_id, net_node) in q_server.iter() {
        while let Ok(Some(packet)) = net_node.recv_message_channel.receiver.try_recv() {
            info!("{} Received: {:?}", channel_id, packet.bytes);
        }
    }
}

pub fn send_json_message(mut channel_messages: EventWriter<SendChannelMessage<PlayerInformation>>) {
    channel_messages.write(SendChannelMessage {
        channel_id: JSON_CHANNEL,
        message: PlayerInformation {
            health: 100,
            position: (1, 2, 3),
        },
    });
}

/// send bincode message
pub fn send_bincode_message(
    mut channel_messages: EventWriter<SendChannelMessage<PlayerInformation>>,
) {
    channel_messages.write(SendChannelMessage {
        channel_id: BINCODE_CHANNEL,
        message: PlayerInformation {
            health: 300,
            position: (4, 5, 6),
        },
    });
}

/// send raw message to server
pub fn send_raw_message_to_channel(
    q_client: Query<(&NetworkNode, &ChannelId), With<ClientNode<WebsocketAddress>>>,
) {
    for (node, channel_id) in q_client.iter() {
        if channel_id == &RAW_CHANNEL {
            node.send_bytes(format!("raw packet to {}", channel_id).as_bytes());
        }
    }
}
