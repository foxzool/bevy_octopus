use bevy::prelude::SystemSet;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum NetworkSet {
    Receive,
    Decoding,
    Encoding,
    Send,
}
