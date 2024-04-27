use bevy::prelude::Resource;
use serde::Deserialize;

use crate::{decoder::DecoderProvider, error::NetworkError};

#[derive(Resource, Default)]
pub struct SerdeJsonProvider;

impl DecoderProvider for SerdeJsonProvider {
    const NAME: &'static str = "SerdeJson";

    fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NetworkError> {
        match serde_json::from_slice(bytes) {
            Ok(value) => Ok(value),
            Err(e) => Err(NetworkError::DeserializeError(e.to_string())),
        }
    }
}
