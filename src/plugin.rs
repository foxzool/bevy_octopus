use bevy::app::{App, Plugin};
use bevy::tasks::TaskPoolBuilder;

use crate::resource::NetworkResource;
use crate::runtime::EventworkRuntime;

pub struct BevyComPlugin;

impl Plugin for BevyComPlugin {
    fn build(&self, app: &mut App) {


        app.init_resource::<NetworkResource>();
        #[cfg(feature = "udp")]
        app.add_plugins(crate::udp::UdpPlugin);
    }
}
