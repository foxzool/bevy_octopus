use std::io;

/// Internal errors used by Spicy
#[derive(thiserror::Error, Debug)]
pub enum NetworkError {
    // /// A default networking error returned when no other more specific type can be determined
    // Error(String),
    //
    // /// Error occurred when accepting a new connection.
    // Accept(io::Error),
    //
    // /// Connection couldn't be found.
    // ConnectionNotFound(ConnectionId),
    //
    // /// Failed to send across channel because it was closed.
    // ChannelClosed(Entity),
    //
    #[error("An error occurred while trying to listen: {0}")]
    Listen(io::Error),
    //
    #[error("An error occurred when trying to connect.")]
    Connection(io::Error),
    #[error("Failed to send data over a closed internal channel")]
    SendError,
    //
    #[error("Failed to serialize data: {0}")]
    Serialization(String),
    #[error("Failed to deserialize data: {0}")]
    DeserializeError(String),
    #[error("Failed to read/write file(s)")]
    IoError(#[from] io::Error),
}
