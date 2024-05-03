use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::{error::NetworkError, transformer::Transformer};

#[derive(Resource, Default)]
pub struct SerdeJsonProvider;

impl Transformer for SerdeJsonProvider {
    const NAME: &'static str = "SerdeJson";

    fn encode<T: Serialize>(data: &T) -> Result<Vec<u8>, NetworkError> {
        match serde_json::to_vec(data) {
            Ok(value) => Ok(value),
            Err(e) => Err(NetworkError::SerializeError(e.to_string())),
        }
    }

    fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NetworkError> {
        match serde_json::from_slice(bytes) {
            Ok(value) => Ok(value),
            Err(e) => Err(NetworkError::DeserializeError(e.to_string())),
        }
    }
}
