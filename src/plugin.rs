use bevy::app::{App, Plugin};

use crate::NetworkErrorEvent;
use crate::resource::NetworkResource;

pub struct BevyComPlugin;

impl Plugin for BevyComPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkResource>()
            .add_event::<NetworkErrorEvent>();


        #[cfg(feature = "udp")]
        app.add_plugins(crate::udp::UdpPlugin);
    }
}
