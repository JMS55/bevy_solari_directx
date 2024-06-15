use crate::gpu::Gpu;
use bevy::{
    prelude::{Commands, Component, Entity, Query, Res, With},
    window::{PrimaryWindow, RawHandleWrapperHolder, Window, WindowMode},
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
        System::Threading::WaitForSingleObjectEx,
    },
};

// TODO: Reflex-like frame pacing, HDR/WCG support, VRR support

const SWAPCHAIN_BUFFER_COUNT: usize = 2;

/// Stores a swapchain and other objects necessary for rendering to a [`Window`].
#[derive(Component)]
pub struct WindowRenderTarget {
    swapchain: IDXGISwapChain4,
    wait_object: HANDLE,
    rtv_heap: ID3D12DescriptorHeap,
    rtvs: Option<[ID3D12Resource; SWAPCHAIN_BUFFER_COUNT]>,
    rtv_handles: Option<[D3D12_CPU_DESCRIPTOR_HANDLE; SWAPCHAIN_BUFFER_COUNT]>,
}

/// Delay starting the main schedule until the swapchain estimates there is 1 frame's worth of time left
/// before it is able to accept a new frame, reducing overall frame latency.
///
/// It's better to block here, before we read user inputs, update game state, and record rendering commands, rather
/// than blocking at the end of the frame waiting for the swapchain to become available. This minimizes the latency
/// between reading user inputs, and submitting the rendered frame to the swapchain.
pub fn wait_for_ready_swapchains(window: Query<&WindowRenderTarget, With<PrimaryWindow>>) {
    if let Ok(render_target) = window.get_single() {
        unsafe { WaitForSingleObjectEx(render_target.wait_object, 1000, true) };

        // TODO: Wait for fence
    }
}

/// Create or update the swapchain for a newly created or changed window.
pub fn update_swapchains(
    mut window: Query<
        (
            Entity,
            &Window,
            &RawHandleWrapperHolder,
            Option<&mut WindowRenderTarget>,
        ),
        With<PrimaryWindow>,
    >,
    mut commands: Commands,
    gpu: Res<Gpu>,
) {
    let Ok((entity, window, window_handle, render_target)) = window.get_single_mut() else {
        return;
    };

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
        BufferCount: SWAPCHAIN_BUFFER_COUNT as u32,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32, // TODO: VRR support
        ..Default::default()
    };

    // If there's an existing swapchain, resize if needed, else create a new swapchain
    if let Some(mut render_target) = render_target {
        resize_swapchain_if_needed(&mut render_target, swapchain_desc, &gpu);
    } else {
        commands
            .entity(entity)
            .insert(create_new_swapchain(&gpu, window_handle, swapchain_desc));
    }
}

fn create_new_swapchain(
    gpu: &Gpu,
    window_handle: &RawHandleWrapperHolder,
    swapchain_desc: DXGI_SWAP_CHAIN_DESC1,
) -> WindowRenderTarget {
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
    unsafe { WaitForSingleObjectEx(wait_object, 1000, true) };

    // Setup RTVs
    let rtv_heap = unsafe {
        gpu.device
            .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: SWAPCHAIN_BUFFER_COUNT as u32,
                ..Default::default()
            })
    }
    .unwrap();
    let (rtvs, rtv_handles) = create_rtvs(&gpu.device, &swapchain, &rtv_heap);

    // Wrap into a component
    WindowRenderTarget {
        swapchain,
        wait_object,
        rtv_heap,
        rtvs: Some(rtvs),
        rtv_handles: Some(rtv_handles),
    }
}

fn resize_swapchain_if_needed(
    render_target: &mut WindowRenderTarget,
    swapchain_desc: DXGI_SWAP_CHAIN_DESC1,
    gpu: &Gpu,
) {
    // Skip resizing swapchain if unchanged
    let mut old_swapchain_desc = Default::default();
    unsafe { render_target.swapchain.GetDesc1(&mut old_swapchain_desc) }.unwrap();
    if swapchain_desc == old_swapchain_desc {
        return;
    }

    // TODO: Wait for idle swapchain via fences

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
}

fn create_rtvs(
    device: &ID3D12Device9,
    swapchain: &IDXGISwapChain4,
    rtv_heap: &ID3D12DescriptorHeap,
) -> (
    [ID3D12Resource; SWAPCHAIN_BUFFER_COUNT],
    [D3D12_CPU_DESCRIPTOR_HANDLE; SWAPCHAIN_BUFFER_COUNT],
) {
    let mut rtvs = SmallVec::with_capacity(SWAPCHAIN_BUFFER_COUNT);
    let mut handles = [D3D12_CPU_DESCRIPTOR_HANDLE::default(); SWAPCHAIN_BUFFER_COUNT];

    let heap_increment =
        unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) } as usize;
    let mut handle = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

    for i in 0..SWAPCHAIN_BUFFER_COUNT {
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
