use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
    ops::Deref,
    sync::{atomic::AtomicBool, Arc},
};

use bevy::prelude::{Component, Entity, Event};
use bytes::Bytes;
use serde::{de::DeserializeOwned, Serialize};

use crate::{error::NetworkError, AsyncChannel};

/// Any type that should be sent over the wire has to implement [`NetworkMessage`].
///
/// ## Example
/// ```rust
/// use bevy_ecs_net::prelude::NetworkMessage;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, Debug)]
/// struct PlayerInformation {
///     health: usize,
///     position: (u32, u32, u32)
/// }
///
/// impl NetworkMessage for PlayerInformation {
///     const NAME: &'static str = "PlayerInfo";
/// }
/// ```

/// Marks a type as an network message
pub trait NetworkMessage: Serialize + DeserializeOwned + Send + Sync + Debug + 'static {
    /// A unique name to identify your message, this needs to be unique __across all included
    /// crates__
    ///
    /// A good combination is crate name + struct name.
    const NAME: &'static str;
}

#[derive(Debug, Event)]
/// A network event originating from a network node
pub enum NetworkEvent {
    Connected(Entity),
    Disconnected(Entity),
    Error(Entity, NetworkError),
}

impl NetworkEvent {
    pub fn entity(&self) -> Entity {
        match self {
            NetworkEvent::Connected(entity) => *entity,
            NetworkEvent::Disconnected(entity) => *entity,
            NetworkEvent::Error(entity, _) => *entity,
        }
    }
}

#[derive(Debug, Event)]
/// [`NetworkData`] is what is sent over the bevy event system
///
/// Please check the root documentation how to up everything
pub struct NetworkData<T> {
    pub source: Entity,
    inner: T,
}

impl<T> Deref for NetworkData<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> NetworkData<T> {
    pub fn new(source: Entity, inner: T) -> Self {
        Self { source, inner }
    }
    /// The source entity of this network data
    pub fn source(&self) -> &Entity {
        &self.source
    }

    /// Get the inner data out of it
    pub fn into_inner(self) -> T {
        self.inner
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

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
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

#[derive(Component)]
pub struct NetworkNode {
    /// Channel for receiving messages
    recv_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for sending messages
    send_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for errors
    error_channel: AsyncChannel<NetworkError>,
    /// A flag to cancel the node
    pub cancel_flag: Arc<AtomicBool>,
    /// Whether the node is running or not
    pub running: bool,
    /// Local address
    pub local_addr: SocketAddr,
    pub peer_addr: Option<SocketAddr>,
    pub max_packet_size: usize,
    pub auto_start: bool,
    protocol: NetworkProtocol,
}

impl NetworkNode {
    pub fn new(
        protocol: NetworkProtocol,
        local_addr: SocketAddr,
        peer_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            recv_message_channel: AsyncChannel::new(),
            send_message_channel: AsyncChannel::new(),
            error_channel: AsyncChannel::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            running: false,
            local_addr,
            peer_addr,
            max_packet_size: 65535,
            auto_start: true,
            protocol,
        }
    }
    pub fn start(&mut self) {
        self.cancel_flag
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.running = true;
    }

    pub fn stop(&mut self) {
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.running = false;
    }

    pub fn send(&self, bytes: &[u8]) {
        self.send_message_channel
            .sender
            .try_send(NetworkRawPacket {
                socket: self.peer_addr,
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
    }

    pub fn send_to(&self, bytes: &[u8], addr: impl ToSocketAddrs) {
        let peer_addr = addr.to_socket_addrs().unwrap().next().unwrap();
        self.send_message_channel
            .sender
            .try_send(NetworkRawPacket {
                socket: Some(peer_addr),
                bytes: Bytes::copy_from_slice(bytes),
            })
            .expect("Message channel has closed.");
    }

    pub fn recv_channel(&self) -> &AsyncChannel<NetworkRawPacket> {
        &self.recv_message_channel
    }

    pub fn send_channel(&self) -> &AsyncChannel<NetworkRawPacket> {
        &self.send_message_channel
    }

    pub fn error_channel(&self) -> &AsyncChannel<NetworkError> {
        &self.error_channel
    }

    pub fn schema(&self) -> String {
        format!(
            "{}://{}:{}",
            self.protocol,
            self.local_addr.ip(),
            self.local_addr.port()
        )
    }
}

impl Display for NetworkNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.schema())
    }
}
