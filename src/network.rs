use std::{fmt::Debug, ops::Deref};
use std::net::{SocketAddr, ToSocketAddrs};

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
    pub fn new(input: &str) -> Self {
        let url = Url::parse(input).expect("url format error");

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
    pub fn new(input: &str) -> Self {
        let url = Url::parse(input).expect("url format error");

        Self(url)
    }

    pub fn peer_addr(&self) -> SocketAddr {
        let url_str = self.0.to_string();
        let arr: Vec<&str> = url_str.split("//").collect();
        let s = arr[1].split('/').collect::<Vec<&str>>()[0];
        s.to_socket_addrs().unwrap().next().unwrap()
    }
}

#[test]
fn test_url() {
    let str = "ws://127.0.0.1:44012/ws/";
    let url = Url::parse(str).unwrap();
    println!("{:#?}", url);
    println!("{:?}", url.host());
    println!("{:?}", url.port_or_default());
    println!("{:?}", url.serialize_path());
    println!("{:?}", url.serialize_host());
    println!("{:?}", url.serialize_no_fragment());
    println!("{:?}", url.serialize());

    let str = "tcp://127.0.0.1:44012";
    let url = Url::parse(str).unwrap();
    println!("{:#?}", url);
    println!("{:?}", url.host());
    println!("{:?}", url.port_or_default());
    println!("{:?}", url.serialize_path());
    println!("{:?}", url.serialize_host());
    println!("{:?}", url.serialize_no_fragment());
    println!("{:?}", url.serialize());
}
