use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
};

use bevy::prelude::{Event, Resource};
use bytes::Bytes;
use futures_lite::Stream;
use kanal::{Receiver, Sender, unbounded};
use serde::{Deserialize, Serialize};

use crate::runtime::JoinHandle;

pub mod event;
pub mod plugin;
pub mod prelude;
pub mod resource;

pub mod error;

pub mod runtime;

pub mod component;

pub type ChannelName = String;

#[derive()]
struct AsyncChannel<T> {
    pub(crate) sender: Sender<T>,
    pub(crate) receiver: Receiver<T>,
}

impl<T> AsyncChannel<T> {
    fn new() -> Self {
        let (sender, receiver) = unbounded();

        Self { sender, receiver }
    }
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
    pub socket: SocketAddr,
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
