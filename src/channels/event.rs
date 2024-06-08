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

#[derive(Event)]
pub struct ChannelMessage<T> {
    pub channel_id: ChannelId,
    pub message: T,
}

impl<T> ChannelMessage<T> {
    pub fn new(channel_id: ChannelId, message: T) -> Self {
        Self {
            channel_id,
            message,
        }
    }
}
