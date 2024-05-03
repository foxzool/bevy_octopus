use bevy::prelude::Event;
use bytes::Bytes;

use crate::channels::ChannelId;

#[derive(Event)]
pub struct ChannelPacket {
    pub channel_id: ChannelId,
    pub bytes: Bytes,
}

impl ChannelPacket {
    pub fn new(channel_id: ChannelId, bytes: &[u8]) -> Self {
        Self {
            channel_id,
            bytes: Bytes::copy_from_slice(bytes),
        }
    }
}


#[derive(Event)]
pub struct ChannelMessage<T> {
    pub channel_id: ChannelId,
    pub message: T,
}