use crate::gpu::Gpu;
use bevy::{
    prelude::{Commands, Component, Entity, Query, Res},
    window::{RawHandleWrapperHolder, Window, WindowMode},
};
use raw_window_handle::RawWindowHandle;
use smallvec::SmallVec;
use windows::{
    core::Interface,
    Win32::{
        Foundation::{HANDLE, HWND},
        Graphics::Dxgi::{
            Common::{DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
            IDXGIFactory2, IDXGISwapChain4, DXGI_SWAP_CHAIN_DESC1,
            DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, DXGI_SWAP_EFFECT_FLIP_DISCARD,
            DXGI_USAGE_RENDER_TARGET_OUTPUT,
        },
        System::Threading::WaitForMultipleObjectsEx,
    },
};

// TODO: Frame pacing, HDR/WCG support, VRR support?

pub const FRAMES_IN_FLIGHT: u32 = 1;

#[derive(Component)]
pub struct WindowRenderTarget {
    swapchain: IDXGISwapChain4,
    wait_object: HANDLE,
}

pub fn wait_for_ready_swapchains(windows: Query<&WindowRenderTarget>) {
    let wait_objects = windows
        .iter()
        .map(|window| window.wait_object)
        .collect::<SmallVec<[HANDLE; 2]>>();

    unsafe { WaitForMultipleObjectsEx(&wait_objects, true, 1000, true) };
}

pub fn update_swapchains(
    mut windows: Query<(
        Entity,
        &Window,
        &RawHandleWrapperHolder,
        Option<&mut WindowRenderTarget>,
    )>,
    mut commands: Commands,
    gpu: Res<Gpu>,
) {
    let mut new_wait_objects = SmallVec::<[HANDLE; 2]>::new();

    for (entity, window, window_handle, render_target) in &mut windows {
        // Check for unsupported window modes
        if !matches!(
            window.mode,
            WindowMode::Windowed | WindowMode::BorderlessFullscreen
        ) {
            panic!(
                "BevySolari: WindowMode must be Windowed or BorderlessFullscreen, was {:?}",
                window.mode
            );
        }

        // Setup swapchain descriptor
        let swapchain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: window.physical_width(),
            Height: window.physical_height(),
            Format: DXGI_FORMAT_R8G8B8A8_UNORM, // TODO
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                ..Default::default()
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT, // TODO
            BufferCount: FRAMES_IN_FLIGHT,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32, // TODO: VRR support
            ..Default::default()
        };

        if let Some(render_target) = render_target {
            // Skip resizing swapchain if unchanged
            let mut old_swapchain_desc = Default::default();
            unsafe { render_target.swapchain.GetDesc1(&mut old_swapchain_desc) }.unwrap();
            if swapchain_desc == old_swapchain_desc {
                continue;
            }

            // TODO: Wait for idle swapchain

            // Resize swapchain
            unsafe {
                render_target.swapchain.ResizeBuffers(
                    swapchain_desc.BufferCount,
                    swapchain_desc.Width,
                    swapchain_desc.Height,
                    swapchain_desc.Format,
                    swapchain_desc.Flags,
                )
            }
            .unwrap();
        } else {
            // Create new swapchain
            let factory = gpu.factory.cast::<IDXGIFactory2>().unwrap();
            let swapchain = unsafe {
                factory.CreateSwapChainForHwnd(
                    &gpu.device,
                    get_hwnd(window_handle),
                    &swapchain_desc,
                    None,
                    None,
                )
            }
            .unwrap()
            .cast::<IDXGISwapChain4>()
            .unwrap();

            // Setup frame latency
            unsafe { swapchain.SetMaximumFrameLatency(1).unwrap() };
            let wait_object = unsafe { swapchain.GetFrameLatencyWaitableObject() };
            new_wait_objects.push(wait_object);

            // Setup RTVs
            // TODO

            // Add a WindowRenderTarget component to the window entity
            commands.entity(entity).insert(WindowRenderTarget {
                swapchain,
                wait_object,
            });
        }
    }

    // Wait for new swapchains to be ready
    if !new_wait_objects.is_empty() {
        unsafe { WaitForMultipleObjectsEx(&new_wait_objects, true, 1000, true) };
    }
}

fn get_hwnd(window_handle: &RawHandleWrapperHolder) -> HWND {
    match window_handle
        .0
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .window_handle
    {
        RawWindowHandle::Win32(window_handle) => HWND(window_handle.hwnd.into()),
        _ => unreachable!(),
    }
}
