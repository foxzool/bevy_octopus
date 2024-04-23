use std::marker::PhantomData;

use bevy::prelude::{Component, Deref, Resource};
use serde::Deserialize;

use crate::{decoder::DecoderProvider, error::NetworkError};

#[derive(Resource, Default)]
pub struct SerdeJsonProvider;

impl DecoderProvider for SerdeJsonProvider {
    fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NetworkError> {
        match serde_json::from_slice(bytes) {
            Ok(value) => Ok(value),
            Err(e) => Err(NetworkError::DeserializeError(e.to_string())),
        }
    }
}

#[derive(Debug, Deref, Component, Default)]
pub struct SerdeJsonMarker<T>
where
    T: for<'a> Deserialize<'a>,
{
    inner: PhantomData<T>,
}
