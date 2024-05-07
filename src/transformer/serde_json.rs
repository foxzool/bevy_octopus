use bevy::prelude::{Reflect, Resource};
use serde::{Deserialize, Serialize};

use crate::{error::NetworkError, transformer::Transformer};

#[derive(Resource, Default, Reflect)]
pub struct JsonTransformer;


impl Transformer for JsonTransformer {
    const NAME: &'static str = "Json";

    fn encode<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, NetworkError> {
        match serde_json::to_vec(data) {
            Ok(value) => Ok(value),
            Err(e) => Err(NetworkError::SerializeError(e.to_string())),
        }
    }

    fn decode<T: for<'a> Deserialize<'a>>(&self, bytes: &[u8]) -> Result<T, NetworkError> {
        match serde_json::from_slice(bytes) {
            Ok(value) => Ok(value),
            Err(e) => Err(NetworkError::DeserializeError(e.to_string())),
        }
    }
}
