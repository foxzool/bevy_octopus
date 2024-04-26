use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
    ops::Deref,
};

use bevy::prelude::{Entity, Event};
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
