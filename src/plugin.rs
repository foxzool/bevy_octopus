use bevy::app::{App, Plugin, Update};

use crate::{tcp, udp};
use crate::shared::{AsyncRuntime, NetworkNodeEvent, NetworkProtocol};

pub struct BevyNetPlugin;

impl Plugin for BevyNetPlugin {
    fn build(&self, app: &mut App) {
        let async_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        app.register_type::<NetworkProtocol>()
            .insert_resource(AsyncRuntime(async_runtime))
            .add_event::<NetworkNodeEvent>()
            .add_systems(Update, crate::shared::network_node_event);

        #[cfg(feature = "udp")]
        app.add_plugins(udp::UdpPlugin);

        #[cfg(feature = "tcp")]
        app.add_plugins(tcp::TcpPlugin);
    }
}