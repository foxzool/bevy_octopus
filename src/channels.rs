use std::fmt::Display;

use bevy::{
    ecs::reflect::ReflectComponent,
    prelude::{Component, Event, EventReader, Query, Reflect},
};
use bytes::Bytes;

use crate::network_node::{NetworkNode, NetworkRawPacket};

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
pub struct SendChannelMessage<M> {
    pub channel_id: ChannelId,
    pub message: M,
}

impl<M> SendChannelMessage<M> {
    pub fn new(channel_id: ChannelId, message: M) -> Self {
        Self {
            channel_id,
            message,
        }
    }
}

#[derive(Event, Debug)]
pub struct ReceiveChannelMessage<M> {
    pub channel_id: ChannelId,
    pub message: M,
}

impl<M> ReceiveChannelMessage<M> {
    pub fn new(channel_id: ChannelId, message: M) -> Self {
        Self {
            channel_id,
            message,
        }
    }
}

pub(crate) fn send_channel_message_system(
    q_net: Query<(&ChannelId, &NetworkNode)>,
    mut channel_events: EventReader<ChannelPacket>,
) {
    for channel_ev in channel_events.read() {
        q_net.par_iter().for_each(|(channel_id, net_node)| {
            if channel_id == &channel_ev.channel_id {
                let _ = net_node.send_message_channel.sender.send(NetworkRawPacket {
                    bytes: channel_ev.bytes.clone(),
                    addr: "".to_string(),
                    text: channel_ev.text.clone(),
                });
            }
        });
    }
}
