#![doc = include_str!("../README.md")]
// #![warn(missing_docs)]


pub mod decoder;
pub mod error;
pub mod network;
pub mod network_manager;

pub mod prelude;
pub mod plugin;
pub mod shared;

#[cfg(feature = "udp")]
pub mod udp;

#[cfg(feature = "tcp")]
pub mod tcp;
