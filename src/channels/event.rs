use bevy::prelude::Event;
use bytes::Bytes;

use crate::channels::ChannelId;

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
