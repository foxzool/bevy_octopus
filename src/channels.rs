use std::fmt::Display;

use bevy::{
    ecs::reflect::ReflectComponent,
    prelude::{Component, Event, EventReader, Query, Reflect},
};
use bytes::Bytes;

use crate::network_node::{NetworkNode, NetworkRawPacket};
use crate::prelude::ClientAddr;

/// Channel marker
#[derive(Clone, PartialEq, Eq, Hash, Default, Component, Reflect, Copy, Debug)]
#[reflect(Component)]
pub struct ChannelId(pub &'static str);

impl Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Channel({})", self.0)
    }
}

#[derive(Event)]
pub struct ChannelPacket {
    pub channel_id: ChannelId,
    pub bytes: Bytes,
    pub text: Option<String>,
}

impl ChannelPacket {
    pub fn new(channel_id: ChannelId, bytes: &[u8]) -> Self {
        Self {
            channel_id,
            bytes: Bytes::copy_from_slice(bytes),
            text: None,
        }
    }
}

#[derive(Event, Debug)]
pub struct ChannelSendMessage<M> {
    pub channel_id: ChannelId,
    pub message: M,
}

impl<M> ChannelSendMessage<M> {
    pub fn new(channel_id: ChannelId, message: M) -> Self {
        Self {
            channel_id,
            message,
        }
    }
}

#[derive(Event, Debug)]
pub struct ChannelReceivedMessage<M> {
    pub channel_id: ChannelId,
    pub message: M,
}

impl<M> ChannelReceivedMessage<M> {
    pub fn new(channel_id: ChannelId, message: M) -> Self {
        Self {
            channel_id,
            message,
        }
    }
}

pub(crate) fn send_channel_message_system(
    q_net: Query<(&ChannelId, &NetworkNode, &ClientAddr)>,
    mut channel_events: EventReader<ChannelPacket>,
) {
    for channel_ev in channel_events.read() {
        q_net.par_iter().for_each(|(channel_id, net_node, client_addr)| {
            if channel_id == &channel_ev.channel_id {
                let _ = net_node.send_message_channel.sender.send(NetworkRawPacket {
                    bytes: channel_ev.bytes.clone(),
                    addr: client_addr.to_string(),
                    text: channel_ev.text.clone(),
                });
            }
        });
    }
}
