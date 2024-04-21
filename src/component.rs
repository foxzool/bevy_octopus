use std::net::{SocketAddr, ToSocketAddrs};

use bevy::prelude::Component;

#[derive(Component)]
pub struct ConnectTo {
    pub addrs: Vec<SocketAddr>,
}

impl ConnectTo {
    pub fn new(addrs: impl ToSocketAddrs) -> Self {
        Self {
            addrs: addrs.to_socket_addrs().unwrap().collect()
        }
    }
}

#[derive(Component, Clone)]
pub struct NetworkSetting {
    pub max_packet_size: usize,
    pub auto_start: bool,
}

impl Default for NetworkSetting {
    fn default() -> Self {
        Self {
            max_packet_size: 65535,
            auto_start: true,
        }
    }
}