use std::fmt::Display;

use bevy::prelude::*;
use tokio::runtime::Runtime;

#[derive(Resource, Deref, DerefMut)]
pub struct AsyncRuntime(pub(crate) Runtime);

#[derive(Resource, Deref, DerefMut)]
pub struct RuntimeHandle(pub(crate) tokio::runtime::Handle);

#[derive(Debug, Component, Ord, PartialOrd, Eq, PartialEq, Reflect)]
pub enum NetworkProtocol {
    UDP,
    TCP,
    SSL,
    WS,
    WSS,
}

impl Display for NetworkProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                NetworkProtocol::UDP => "udp",
                NetworkProtocol::TCP => "tcp",
                NetworkProtocol::SSL => "ssl",
                NetworkProtocol::WS => "ws",
                NetworkProtocol::WSS => "wss",
            }
        )
    }
}
