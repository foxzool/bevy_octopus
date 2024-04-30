use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::{fmt::Debug, net::SocketAddr, ops::Deref};

use bevy::prelude::{Component, Deref, Entity, Event};
use bytes::Bytes;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::NetworkError;

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
    Listen(Entity),
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
            NetworkEvent::Listen(entity) => *entity,
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

#[derive(Component, Clone, Debug, Deref)]
pub struct LocalSocket(pub SocketAddr);

impl LocalSocket {
    pub fn new(addr: impl ToSocketAddrs) -> Self {
        let socket = addr
            .to_socket_addrs()
            .expect("not valid socket format")
            .next()
            .expect("must have one socket addr");
        Self(socket)
    }
}

impl Default for LocalSocket {
    fn default() -> Self {
        Self(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
    }
}

#[derive(Component, Clone, Debug, Deref)]
pub struct RemoteSocket(pub SocketAddr);

impl Default for RemoteSocket {
    fn default() -> Self {
        Self(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
    }
}

impl RemoteSocket {
    pub fn new(addr: impl ToSocketAddrs) -> Self {
        let socket = addr
            .to_socket_addrs()
            .expect("not valid socket format")
            .next()
            .expect("must have one socket addr");
        Self(socket)
    }
}
