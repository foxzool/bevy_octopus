use bevy::prelude::Resource;
use serde::Deserialize;

use crate::{decoder::DecoderProvider, error::NetworkError};

#[derive(Resource, Default)]
pub struct BincodeProvider;

impl DecoderProvider for BincodeProvider {
    fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NetworkError> {
        match bincode::deserialize(bytes) {
            Ok(value) => Ok(value),
            Err(e) => Err(NetworkError::DeserializeError(e.to_string())),
        }
    }
}
