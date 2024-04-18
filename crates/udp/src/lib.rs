pub struct BevyUdpComPlugin;

impl Plugin for BevyUdpComPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(startup.system());
        app.add_system(update.system());
    }
}