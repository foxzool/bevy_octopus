use bevy::log::debug;
use bevy::prelude::Resource;
use serde::Deserialize;

use crate::decoder::DecoderProvider;
use crate::error::NetworkError;


#[derive(Resource, Default)]
pub struct BincodeProvider;

impl DecoderProvider for BincodeProvider {
    fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NetworkError> {
        match bincode::deserialize(bytes) {
            Ok(value) => Ok(value),
            Err(e) => {
                debug!("Error decoding message: {:?}", e);
                Err(NetworkError::DeserializeError)
            }
        }
    }
}