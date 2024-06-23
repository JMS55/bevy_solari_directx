mod gpu;
mod swapchain;

use bevy::{
    app::{First, Last, MainScheduleOrder, Plugin},
    ecs::schedule::ScheduleLabel,
    prelude::App,
};

pub use crate::{
    gpu::Gpu,
    swapchain::{update_swapchain, wait_for_ready_frame, WindowRenderTarget},
};
pub use windows;

pub struct BevyDirectXPlugin;

impl Plugin for BevyDirectXPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(Render);
        app.world_mut()
            .resource_mut::<MainScheduleOrder>()
            .insert_after(Last, Render);

        let gpu = Gpu::new().expect("BevyDirectX: Failed to initialize renderer");

        app.insert_resource(gpu)
            .add_systems(First, wait_for_ready_frame) // TODO: Should probably be it's own schedule before First
            .add_systems(Render, update_swapchain);
    }
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Render;
