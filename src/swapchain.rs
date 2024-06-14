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
        Graphics::{
            Direct3D12::*,
            Dxgi::{
                Common::{DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
                *,
            },
        },
        System::Threading::WaitForMultipleObjectsEx,
    },
};

// TODO: Frame pacing, HDR/WCG support, VRR support?

pub const FRAMES_IN_FLIGHT: usize = 2;

#[derive(Component)]
pub struct WindowRenderTarget {
    swapchain: IDXGISwapChain4,
    wait_object: HANDLE,
    rtv_heap: ID3D12DescriptorHeap,
    rtvs: Option<[ID3D12Resource; FRAMES_IN_FLIGHT]>,
    rtv_handles: Option<[D3D12_CPU_DESCRIPTOR_HANDLE; FRAMES_IN_FLIGHT]>,
}

pub fn wait_for_ready_swapchains(windows: Query<&WindowRenderTarget>) {
    let wait_objects = windows
        .iter()
        .map(|window| window.wait_object)
        .collect::<SmallVec<[HANDLE; 2]>>();

    unsafe { WaitForMultipleObjectsEx(&wait_objects, true, 1000, true) };

    // TODO: Wait for fence?
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
            BufferCount: FRAMES_IN_FLIGHT as u32,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32, // TODO: VRR support
            ..Default::default()
        };

        if let Some(mut render_target) = render_target {
            // Skip resizing swapchain if unchanged
            let mut old_swapchain_desc = Default::default();
            unsafe { render_target.swapchain.GetDesc1(&mut old_swapchain_desc) }.unwrap();
            if swapchain_desc == old_swapchain_desc {
                continue;
            }

            // TODO: Wait for idle swapchain

            // Drop old RTVs
            render_target.rtvs = None;
            render_target.rtv_handles = None;

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

            // Recreate RTVs
            let (rtvs, rtv_handles) = create_rtvs(
                &gpu.device,
                &render_target.swapchain,
                &render_target.rtv_heap,
            );
            render_target.rtvs = Some(rtvs);
            render_target.rtv_handles = Some(rtv_handles);
        } else {
            // Create new swapchain
            let factory = gpu.factory.cast::<IDXGIFactory2>().unwrap();
            let swapchain = unsafe {
                factory.CreateSwapChainForHwnd(
                    &gpu.queue,
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
            let rtv_heap = unsafe {
                gpu.device
                    .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                        Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                        NumDescriptors: FRAMES_IN_FLIGHT as u32,
                        ..Default::default()
                    })
            }
            .unwrap();
            let (rtvs, rtv_handles) = create_rtvs(&gpu.device, &swapchain, &rtv_heap);

            // Add a WindowRenderTarget component to the window entity
            commands.entity(entity).insert(WindowRenderTarget {
                swapchain,
                wait_object,
                rtv_heap,
                rtvs: Some(rtvs),
                rtv_handles: Some(rtv_handles),
            });
        }
    }

    // Wait for any new swapchains to be ready
    if !new_wait_objects.is_empty() {
        unsafe { WaitForMultipleObjectsEx(&new_wait_objects, true, 1000, true) };
    }
}

fn create_rtvs(
    device: &ID3D12Device9,
    swapchain: &IDXGISwapChain4,
    rtv_heap: &ID3D12DescriptorHeap,
) -> (
    [ID3D12Resource; FRAMES_IN_FLIGHT],
    [D3D12_CPU_DESCRIPTOR_HANDLE; FRAMES_IN_FLIGHT],
) {
    let mut rtvs = SmallVec::with_capacity(FRAMES_IN_FLIGHT);
    let mut handles = [D3D12_CPU_DESCRIPTOR_HANDLE::default(); FRAMES_IN_FLIGHT];

    let heap_increment =
        unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) } as usize;
    let mut handle = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

    for i in 0..FRAMES_IN_FLIGHT {
        let rtv = unsafe { swapchain.GetBuffer::<ID3D12Resource>(i as u32) }.unwrap();
        unsafe { device.CreateRenderTargetView(&rtv, None, handle) };

        rtvs.push(rtv);
        handles[i] = handle;

        handle.ptr += heap_increment;
    }

    (rtvs.into_inner().unwrap(), handles)
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
