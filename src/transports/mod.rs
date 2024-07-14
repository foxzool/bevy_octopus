use bevy::prelude::{Added, Without};

use crate::{network::ConnectTo, network_node::NetworkNode};

#[cfg(feature = "udp")]
pub mod udp;

#[cfg(feature = "tcp")]
pub mod tcp;

pub type ServerNodeAddedFilter = (Added<ConnectTo>, Without<NetworkNode>);
