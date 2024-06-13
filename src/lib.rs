mod gpu;

use bevy::{
    app::{Plugin, Startup},
    prelude::{App, Commands, Query, With},
    window::{PrimaryWindow, RawHandleWrapperHolder},
};
use gpu::Gpu;

pub struct BevySolariPlugin;

impl Plugin for BevySolariPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_renderer);
    }
}

fn setup_renderer(
    window: Query<&RawHandleWrapperHolder, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    let window = window.single();
    commands.insert_resource(unsafe { Gpu::new() }.expect("Failed to initialize renderer"));
}
