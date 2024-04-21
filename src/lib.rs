use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use async_channel::{Receiver, Sender, unbounded};
use async_trait::async_trait;
use bevy::prelude::{Event, Resource};
use bytes::Bytes;
use futures_lite::Stream;
use serde::{Deserialize, Serialize};

use crate::error::NetworkError;
use crate::runtime::JoinHandle;

pub mod event;
pub mod plugin;
pub mod prelude;
pub mod resource;

pub mod error;

pub mod runtime;

pub mod component;

pub type ChannelName = String;

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
            .finish()
    }
}

/// A trait used to drive the network. This is responsible
/// for generating the futures that carryout the underlying app network logic.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait NetworkProvider: 'static + Send + Sync {
    /// This is to configure particular protocols
    type NetworkSettings: Resource + Clone;

    /// The type that acts as a combined sender and reciever for the network.
    /// This type needs to be able to be split.
    type Socket: Send;

    /// The read half of the given socket type.
    type ReadHalf: Send;

    /// The write half of the given socket type.
    type WriteHalf: Send;

    /// Info necessary to start a connection, an [`std::net::SocketAddr`] for instance
    type ConnectInfo: Send;

    /// Info necessary to start a connection, an [`std::net::SocketAddr`] for instance
    type AcceptInfo: Send;

    /// The output type of [`Self::accept_loop`]
    type AcceptStream: Stream<Item = Self::Socket> + Unpin + Send;

    /// This will be spawned as a background operation to continuously add new connections.
    async fn accept_loop(
        accept_info: Self::AcceptInfo,
        network_settings: Self::NetworkSettings,
    ) -> Result<Self::AcceptStream, NetworkError>;

    /// Attempts to connect to a remote
    async fn connect_task(
        connect_info: Self::ConnectInfo,
        network_settings: Self::NetworkSettings,
    ) -> Result<Self::Socket, NetworkError>;

    /// Recieves messages over the network, forwards them to Eventwork via a sender.
    async fn recv_loop(
        read_half: Self::ReadHalf,
        messages: Sender<NetworkRawPacket>,
        settings: Self::NetworkSettings,
    );

    /// Sends messages over the network, receives packages from Eventwork via receiver.
    async fn send_loop(
        write_half: Self::WriteHalf,
        messages: Receiver<NetworkRawPacket>,
        settings: Self::NetworkSettings,
    );

    /// Split the socket into a read and write half, so that the two actions
    /// can be handled concurrently.
    fn split(combined: Self::Socket) -> (Self::ReadHalf, Self::WriteHalf);
}

#[cfg(feature = "udp")]
pub mod udp;
