use std::marker::PhantomData;

use bevy::log::debug;
use bevy::prelude::{Component, Deref, Entity, Query, Resource, With};
use serde::Deserialize;

use crate::component::NetworkNode;
use crate::decoder::DecoderProvider;
use crate::error::NetworkError;

#[derive(Resource, Default)]
pub struct SerdeJsonProvider;

impl DecoderProvider for SerdeJsonProvider {
    fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NetworkError> {
        match serde_json::from_slice(bytes) {
            Ok(value) => Ok(value),
            Err(e) => {
                debug!("Error decoding message: {:?}", e);
                Err(NetworkError::DeserializeError)
            }
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

