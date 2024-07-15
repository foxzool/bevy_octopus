use std::fmt::Display;

use bevy::{
    ecs::world::CommandQueue,
    prelude::{Added, Bundle, Commands, Component, Or, Query, ResMut, Resource},
    tasks::block_on,
};
use bytes::Bytes;

use crate::{
    error::NetworkError,
    network::{ConnectTo, ListenTo, NetworkRawPacket},
    prelude::ChannelId,
    shared::{AsyncChannel, NetworkEvent},
};

#[derive(Bundle)]
pub struct NetworkBundle {
    pub channel_id: ChannelId,
    pub network_node: NetworkNode,
}

impl NetworkBundle {
    pub fn new(channel_id: ChannelId) -> Self {
        Self {
            channel_id,
            network_node: NetworkNode::default(),
        }
    }
}

#[derive(Component, Default)]
pub struct NetworkNode {
    /// Channel for receiving messages
    pub recv_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for sending messages for peer
    pub send_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for events
    pub event_channel: AsyncChannel<NetworkEvent>,
    /// Channel for shutdown
    pub shutdown_channel: AsyncChannel<()>,
    /// Whether the node is running or not
    pub running: bool,
    pub max_packet_size: usize,
    pub listen_to: Option<ListenTo>,
    pub connect_to: Option<ConnectTo>,
}

impl NetworkNode {
    pub fn start(&mut self) {
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn send(&self, bytes: &[u8]) {
        match self.connect_to.as_ref() {
            None => {
                let _ =
                    self.event_channel
                        .sender
                        .try_send(NetworkEvent::Error(NetworkError::Custom(
                            "No connection".to_string(),
                        )));
            }
            Some(connect_to) => {
                let addr = connect_to.to_string();
                let _ = self
                    .send_message_channel
                    .sender
                    .try_send(NetworkRawPacket::new(addr, Bytes::copy_from_slice(bytes)));
            }
        }
    }

    /// Send text message
    pub fn send_text(&self, text: String) {
        match self.connect_to.as_ref() {
            None => {
                let _ =
                    self.event_channel
                        .sender
                        .try_send(NetworkEvent::Error(NetworkError::Custom(
                            "No connection".to_string(),
                        )));
            }
            Some(connect_to) => {
                let addr = connect_to.to_string();
                let _ = self.send_message_channel.sender.try_send(NetworkRawPacket {
                    addr,
                    bytes: Bytes::new(),
                    text: Some(text),
                });
            }
        }
    }

    pub fn send_to(&self, bytes: &[u8], addr: impl ToString) {
        let _ = self
            .send_message_channel
            .sender
            .try_send(NetworkRawPacket::new(addr, Bytes::copy_from_slice(bytes)));
    }

    pub fn schema(&self) -> String {
        if let Some(local_addr) = self.listen_to.as_ref() {
            local_addr.to_string()
        } else if let Some(connect_to) = self.connect_to.as_ref() {
            connect_to.to_string()
        } else {
            "".to_string()
        }
    }
}

impl Display for NetworkNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.schema())
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn update_network_node(
    mut q_net: Query<
        (&mut NetworkNode, Option<&ListenTo>, Option<&ConnectTo>),
        Or<(Added<NetworkNode>, Added<NetworkNode>)>,
    >,
) {
    for (mut net_node, opt_listen_to, opt_connect_to) in q_net.iter_mut() {
        if let Some(listen_to) = opt_listen_to {
            if net_node.listen_to.is_none() {
                net_node.listen_to = Some(listen_to.clone());
            }
        }
        if let Some(connect_to) = opt_connect_to {
            if net_node.connect_to.is_none() {
                net_node.connect_to = Some(connect_to.clone());
            }
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct CommandQueueTasks {
    pub tasks: AsyncChannel<CommandQueue>,
}

pub(crate) fn handle_command_queue_tasks(task: ResMut<CommandQueueTasks>, mut commands: Commands) {
    while let Ok(Some(mut commands_queue)) = task.tasks.receiver.try_recv() {
        block_on(async {
            commands.append(&mut commands_queue);
        });
    }
}
