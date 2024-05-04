use bevy::prelude::{Component, Reflect, Resource};
use serde::{Deserialize, Serialize};

use crate::{error::NetworkError, transformer::Transformer};

#[derive(Resource, Default, Reflect)]
pub struct BincodeTransformer;

#[derive(Component)]
pub struct BincodeMarker;

impl Transformer for BincodeTransformer {
    const NAME: &'static str = "Bincode";

    fn encode<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, NetworkError> {
        bincode::serialize(data).map_err(|e| NetworkError::SerializeError(e.to_string()))
    }

    fn decode<T: for<'a> Deserialize<'a>>(&self, bytes: &[u8]) -> Result<T, NetworkError> {
        match bincode::deserialize(bytes) {
            Ok(value) => Ok(value),
            Err(e) => Err(NetworkError::DeserializeError(e.to_string())),
        }
    }
}
