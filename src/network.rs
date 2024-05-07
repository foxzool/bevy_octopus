use std::{
    fmt::{Debug, Display},
    net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs},
    ops::Deref,
};

use bevy::prelude::{Component, Deref, Entity, Event, Reflect};
use bytes::Bytes;

#[derive(Debug, Event)]
/// [`NetworkData`] is what is sent over the bevy event system
///
/// Please check the root documentation how to up everything
pub struct NetworkData<T> {
    pub source: Entity,
    pub inner: T,
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
}

/// [`NetworkRawPacket`]s are raw packets that are sent over the network.
pub struct NetworkRawPacket {
    pub addr: SocketAddr,
    pub bytes: Bytes,
}

impl Debug for NetworkRawPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkRawPacket")
            .field("addr", &self.addr)
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

#[derive(Debug, Clone, Copy, Component, Ord, PartialOrd, Eq, PartialEq, Reflect, Default)]
pub enum NetworkProtocol {
    UDP,
    #[default]
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
