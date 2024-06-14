mod gpu;
mod swapchain;

use crate::gpu::Gpu;
use crate::swapchain::update_swapchains;
use crate::swapchain::wait_for_ready_swapchains;
use bevy::{
    app::{First, Last, MainScheduleOrder, Plugin},
    ecs::schedule::ScheduleLabel,
    prelude::App,
};

pub struct BevySolariPlugin;

impl Plugin for BevySolariPlugin {
    fn build(&self, app: &mut App) {
        app.init_schedule(Render);
        app.world_mut()
            .resource_mut::<MainScheduleOrder>()
            .insert_after(Last, Render);

        let gpu = unsafe { Gpu::new() }.expect("BevySolari: Failed to initialize renderer");

        app.insert_resource(gpu)
            .add_systems(First, wait_for_ready_swapchains) // TODO: Should probably be it's own schedule before First
            .add_systems(Render, update_swapchains);
    }
}

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct Render;
