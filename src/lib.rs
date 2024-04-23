use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
};

use bevy::{
    app::{App, Plugin},
    prelude::{Entity, Event},
};
use bytes::Bytes;
use kanal::{Receiver, Sender, unbounded};

use crate::{error::NetworkError, prelude::NetworkResource, runtime::JoinHandle};

pub mod event;
pub mod prelude;
pub mod resource;

pub mod error;

pub mod decoder;
mod network;
mod system;

pub mod runtime;

pub mod component;

pub type ChannelName = String;

pub struct BevyComPlugin;

impl Plugin for BevyComPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkResource>()
            .add_event::<NetworkErrorEvent>();

        #[cfg(feature = "udp")]
        app.add_plugins(crate::udp::UdpPlugin);
    }
}

#[derive()]
pub struct AsyncChannel<T> {
    pub sender: Sender<T>,
    pub receiver: Receiver<T>,
}

impl<T> AsyncChannel<T> {
    fn new() -> Self {
        let (sender, receiver) = unbounded();

        Self { sender, receiver }
    }
}

#[derive(Debug, Event)]
/// A network event originating from another eventwork app
pub struct NetworkErrorEvent {
    /// The entity that caused the error
    pub source: Entity,
    /// An error occurred while trying to do a network operation
    pub error: NetworkError,
}

#[derive(Debug, Event)]
/// [`NetworkData`] is what is sent over the bevy event system
///
/// Please check the root documentation how to up everything
pub struct NetworkData<T> {
    source: Entity,
    inner: T,
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
/// A [`ConnectionId`] denotes a single connection
pub struct ConnectionId {
    /// The key of the connection.
    pub id: u32,
}

impl Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Connection with ID={0}", self.id))
    }
}

struct Connection {
    receive_task: Box<dyn JoinHandle>,
    map_receive_task: Box<dyn JoinHandle>,
    send_task: Box<dyn JoinHandle>,
    send_message: Sender<Bytes>,
}

impl Connection {
    fn stop(mut self) {
        self.receive_task.abort();
        self.send_task.abort();
        self.map_receive_task.abort();
    }
}

/// [`NetworkRawPacket`]s are raw packets that are sent over the network.
pub struct NetworkRawPacket {
    pub socket: Option<SocketAddr>,
    pub bytes: Bytes,
}

impl Debug for NetworkRawPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkRawPacket")
            .field("socket", &self.socket)
            .field("len", &self.bytes.len())
            .finish()
    }
}

#[cfg(feature = "udp")]
pub mod udp;
