use bevy::app::{App, Plugin};

pub struct BevyComPlugin;

impl Plugin for BevyComPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "udp")]
        app.add_plugins(bevy_udp_com::BevyUdpComPlugin);
    }
}