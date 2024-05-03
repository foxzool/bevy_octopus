use std::marker::PhantomData;
use std::net::SocketAddr;

use bevy::app::App;
use bevy::prelude::{Component, debug};

use crate::network::NetworkRawPacket;
use crate::shared::{AsyncChannel, NetworkEvent};

pub trait AppNetProvider {
    fn register_network_provider<NP: NetworkProvider>(&mut self) -> &mut Self;
}

impl AppNetProvider for App {
    fn register_network_provider<NP: NetworkProvider>(&mut self) -> &mut Self {
        debug!("Register network provider: {}", NP::NAME);
        self
    }
}

pub trait NetworkProvider {
    const NAME: &'static str;
}


#[derive(Component)]
pub struct NetworkNode<C> {
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
    /// Local address
    pub local_addr: Option<SocketAddr>,
    pub peer_addr: Option<SocketAddr>,
    pub max_packet_size: usize,
    phantom_data: PhantomData<C>,
}