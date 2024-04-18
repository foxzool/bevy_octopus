
pub mod error;
mod runtime;
pub mod managers;

use std::fmt::{Debug, Display};
use bevy::reflect::erased_serde::__private::serde::{Deserialize, Serialize};
pub use runtime::Runtime;


#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
/// A [`ConnectionId`] denotes a single connection
pub struct ConnectionId {
    /// The key of the connection.
    pub id: u32,
}

impl Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Connection with ID={0}", self.id))
    }
}


#[derive(Serialize, Deserialize)]
/// [`NetworkPacket`]s are untyped packets to be sent over the wire
pub struct NetworkPacket {
    kind: String,
    data: Vec<u8>,
}

impl Debug for NetworkPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkPacket")
            .field("kind", &self.kind)
            .finish()
    }
}