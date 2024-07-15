use std::io;

/// Internal errors used by Octopus
#[derive(thiserror::Error, Debug)]
pub enum NetworkError {
    /// A default networking error returned when no other more specific type can be determined
    #[error(transparent)]
    Common(#[from] anyhow::Error),
    #[error("{0}")]
    Custom(String),
    #[error("An error occurred while trying to listen: {0}")]
    Listen(io::Error),
    //
    #[error("An error occurred when trying to connect.")]
    Connection(String),
    #[error("Failed to send to channel: {0}")]
    SendError(#[from] kanal::SendError),
    #[error("Failed to receive from channel: {0}")]
    ReceiveError(#[from] kanal::ReceiveError),
    #[error("Failed to serialize data: {0}")]
    SerializeError(String),
    #[error("Failed to deserialize data: {0}")]
    DeserializeError(String),
    #[error("Failed to read/write file(s)")]
    IoError(#[from] io::Error),
}
