use std::fmt::Debug;

use serde::{de::DeserializeOwned, Serialize};

/// Any type that should be sent over the wire has to implement [`NetworkMessage`].
///
/// ## Example
/// ```rust
/// use bevy_com::prelude::NetworkMessage;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, Debug)]
/// struct PlayerInformation {
///     health: usize,
///     position: (u32, u32, u32)
/// }
///
/// impl NetworkMessage for PlayerInformation {
///     const NAME: &'static str = "PlayerInfo";
/// }
/// ```

/// Marks a type as an eventwork message
pub trait NetworkMessage: Serialize + DeserializeOwned + Send + Sync + Debug + 'static {
    /// A unique name to identify your message, this needs to be unique __across all included
    /// crates__
    ///
    /// A good combination is crate name + struct name.
    const NAME: &'static str;
}
