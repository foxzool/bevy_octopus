use std::io;

/// Internal errors used by Octopus
#[derive(thiserror::Error, Debug)]
pub enum NetworkError {
    #[error("{0}")]
    Common(String),
    #[error("An error occurred while trying to listen: {0}")]
    Listen(io::Error),
    #[error("An error occurred when trying to connect.")]
    Connection(String),
    #[error("Failed to serialize data: {0}")]
    SerializeError(String),
    #[error("Failed to deserialize data: {0}")]
    DeserializeError(String),
    #[error("Failed to read/write file(s)")]
    IoError(#[from] io::Error),
}
