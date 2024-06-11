use std::{
    fmt::Debug,
    net::{SocketAddr, ToSocketAddrs},
    ops::Deref,
};

use bevy::prelude::{Component, Deref, Entity, Event};
use bytes::Bytes;
use url::Url;

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
    pub addr: String,
    pub bytes: Bytes,
    #[cfg(feature = "websocket")]
    pub text: Option<String>,
}

impl NetworkRawPacket {
    pub fn new(addr: impl ToString, bytes: Bytes) -> NetworkRawPacket {
        NetworkRawPacket {
            addr: addr.to_string(),
            bytes,
            #[cfg(feature = "websocket")]
            text: None,
        }
    }
}

impl Debug for NetworkRawPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkRawPacket")
            .field("addr", &self.addr)
            .field("len", &self.bytes.len())
            .finish()
    }
}

#[derive(Component, Deref, Clone, Debug)]
pub struct ListenTo(pub Url);

impl ListenTo {
    pub fn new(url_str: &str) -> Self {
        let url = Url::parse(url_str).expect("url format error");
        check_support_scheme(&url);
        Self(url)
    }

    pub fn local_addr(&self) -> SocketAddr {
        let url_str = self.0.to_string();
        let arr: Vec<&str> = url_str.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];
        s.to_socket_addrs().unwrap().next().unwrap()
    }
}

#[derive(Component, Deref, Clone, Debug)]
pub struct ConnectTo(pub Url);

impl ConnectTo {
    pub fn new(url_str: &str) -> Self {
        let url = Url::parse(url_str).expect("url format error");
        check_support_scheme(&url);
        Self(url)
    }

    pub fn peer_addr(&self) -> SocketAddr {
        let url_str = self.0.to_string();
        let arr: Vec<&str> = url_str.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];
        s.to_socket_addrs().unwrap().next().unwrap()
    }
}

/// check if the scheme is supported
fn check_support_scheme(url: &Url) {
    match url.scheme() {
        "tcp" => {
            if cfg!(not(feature = "tcp")) {
                panic!("tcp feature not enabled");
            }
        }
        "udp" => {
            if cfg!(not(feature = "udp")) {
                panic!("udp feature not enabled");
            }
        }
        "ws" | "wss" => {
            if cfg!(not(feature = "websocket")) {
                panic!("websocket feature not enabled");
            }
        }
        _ => panic!("scheme {} not supported", url.scheme()),
    }
}
