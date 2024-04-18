
use async_channel::{unbounded, Receiver, Sender};

use async_trait::async_trait;
use bevy::prelude::Resource;
use futures_lite::Stream;
use crate::error::NetworkError;

use crate::NetworkPacket;

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
        messages: Sender<NetworkPacket>,
        settings: Self::NetworkSettings,
    );

    /// Sends messages over the network, receives packages from Eventwork via receiver.
    async fn send_loop(
        write_half: Self::WriteHalf,
        messages: Receiver<NetworkPacket>,
        settings: Self::NetworkSettings,
    );

    /// Split the socket into a read and write half, so that the two actions
    /// can be handled concurrently.
    fn split(combined: Self::Socket) -> (Self::ReadHalf, Self::WriteHalf);
}