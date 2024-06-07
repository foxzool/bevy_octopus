use bevy::app::{App, Plugin, PostUpdate};

pub struct WebsocketPlugin;


impl Plugin for WebsocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, (spawn_websocket_server, spawn_websocket_client));
    }
}

fn spawn_websocket_server() {
    todo!()
}

fn spawn_websocket_client() {
    todo!()
}