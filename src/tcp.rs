use async_net::TcpListener;
use bevy::prelude::*;

pub struct TcpPlugin;

impl Plugin for TcpPlugin {
    fn build(&self, _app: &mut App) {}
}

#[derive(Component)]
pub struct TcpListenerNode {
    socket: TcpListener,
}

#[derive(Component)]
pub struct TcpStreamNode {
    socket: async_net::TcpStream,
}
