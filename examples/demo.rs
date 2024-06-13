use bevy::{app::App, DefaultPlugins};
use bevy_solari_directx::BevySolariPlugin;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, BevySolariPlugin))
        .run();
}
