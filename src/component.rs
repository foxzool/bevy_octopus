use std::{
    marker::PhantomData,
    net::{SocketAddr, ToSocketAddrs},
    sync::{Arc, atomic::AtomicBool},
};

use bevy::prelude::{Component, Deref};
use bytes::Bytes;
use kanal::Receiver;
use serde::Deserialize;

use crate::{AsyncChannel, error::NetworkError, NetworkRawPacket};

#[derive(Component)]
pub struct ConnectTo {
    pub addrs: Vec<SocketAddr>,
}

impl ConnectTo {
    pub fn new(addrs: impl ToSocketAddrs) -> Self {
        Self {
            addrs: addrs.to_socket_addrs().unwrap().collect(),
        }
    }
}



#[derive(Component)]
pub struct NetworkNode {
    /// Channel for receiving messages
    pub recv_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for sending messages
    pub send_message_channel: AsyncChannel<NetworkRawPacket>,
    /// Channel for errors
    pub error_channel: AsyncChannel<NetworkError>,
    /// A flag to cancel the node
    pub cancel_flag: Arc<AtomicBool>,
    /// Whether the node is running or not
    pub running: bool,
    pub local_addr: Option<SocketAddr>,
    pub peer_addr: Option<SocketAddr>,
    pub max_packet_size: usize,
    pub auto_start: bool,
}

impl Default for NetworkNode {
    fn default() -> Self {
        Self {
            recv_message_channel: AsyncChannel::new(),
            send_message_channel: AsyncChannel::new(),
            error_channel: AsyncChannel::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            running: false,
            local_addr: None,
            peer_addr: None,
            max_packet_size: 65535,
            auto_start: true,
        }
    }
}

impl NetworkNode {
    pub fn new(local_addr: Option<SocketAddr>, peer_addr: Option<SocketAddr>) -> Self {
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

    pub fn message_receiver(&self) -> &Receiver<NetworkRawPacket> {
        &self.recv_message_channel.receiver
    }

    pub fn error_receiver(&self) -> &Receiver<NetworkError> {
        &self.error_channel.receiver
    }
}

#[derive(Component)]
pub struct StopMarker;
