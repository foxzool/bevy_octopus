use crate::ChannelName;

pub enum ServerEvent {
    ServerStarted(ChannelName),
    ServerStopped(ChannelName),
}
