use bevy::prelude::{error, info, warn, Resource};
use core::ffi::c_void;
use std::{backtrace::Backtrace, ptr, sync::Mutex};
use windows::{
    core::{Error, Interface, PCSTR},
    Win32::Graphics::{
        Direct3D::D3D_FEATURE_LEVEL_12_2,
        Direct3D12::*,
        Dxgi::{
            CreateDXGIFactory2, IDXGIAdapter4, IDXGIFactory7, DXGI_CREATE_FACTORY_DEBUG,
            DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
        },
    },
};

#[derive(Resource)]
pub struct Gpu {
    pub factory: IDXGIFactory7,
    pub device: ID3D12Device9,
    pub queue: ID3D12CommandQueue,
    pub command_allocator: Mutex<ID3D12CommandAllocator>,
}

impl Gpu {
    pub unsafe fn new() -> Result<Self, Error> {
        // Debug layers
        let mut factory_flags = 0;
        if cfg!(debug_assertions) {
            let mut debug_interface: Option<ID3D12Debug3> = None;
            D3D12GetDebugInterface(&mut debug_interface)?;
            let debug_interface = debug_interface.unwrap();
            debug_interface.EnableDebugLayer();
            debug_interface.SetEnableGPUBasedValidation(true);

            factory_flags = DXGI_CREATE_FACTORY_DEBUG;
        }

        // Factory
        let factory: IDXGIFactory7 = CreateDXGIFactory2(factory_flags)?;

        // Adapter
        let adapter: IDXGIAdapter4 =
            factory.EnumAdapterByGpuPreference(0, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE)?;

        // Device
        let mut device: Option<ID3D12Device9> = None;
        D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_12_2, &mut device)?;
        let device = device.unwrap();

        // Debug layer callback
        let info_queue = device.cast::<ID3D12InfoQueue1>()?;
        let mut cookie = 0;
        info_queue.RegisterMessageCallback(
            Some(log_debug_layer_message),
            D3D12_MESSAGE_CALLBACK_FLAG_NONE,
            ptr::null_mut(),
            &mut cookie,
        )?;
        if cookie == 0 {
            panic!("BevySolari: Failed to register debug layer callback");
        }

        // Queue
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;

        // Command allocator
        let command_allocator =
            Mutex::new(device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?);

        Ok(Self {
            factory,
            device,
            queue,
            command_allocator,
        })
    }
}

pub unsafe extern "system" fn log_debug_layer_message(
    category: D3D12_MESSAGE_CATEGORY,
    severity: D3D12_MESSAGE_SEVERITY,
    id: D3D12_MESSAGE_ID,
    description: PCSTR,
    _: *mut c_void,
) {
    let id = id.0;
    let description = description.to_string().unwrap();
    let backtrace = Backtrace::force_capture();

    let category = match category {
        D3D12_MESSAGE_CATEGORY_APPLICATION_DEFINED => "Application Defined",
        D3D12_MESSAGE_CATEGORY_MISCELLANEOUS => "Miscellaneous",
        D3D12_MESSAGE_CATEGORY_INITIALIZATION => "Initialization",
        D3D12_MESSAGE_CATEGORY_CLEANUP => "Cleanup",
        D3D12_MESSAGE_CATEGORY_COMPILATION => "Compilation",
        D3D12_MESSAGE_CATEGORY_STATE_CREATION => "State Creation",
        D3D12_MESSAGE_CATEGORY_STATE_SETTING => "State Setting",
        D3D12_MESSAGE_CATEGORY_STATE_GETTING => "State Getting",
        D3D12_MESSAGE_CATEGORY_RESOURCE_MANIPULATION => "Resource Manipulation",
        D3D12_MESSAGE_CATEGORY_EXECUTION => "Execution",
        D3D12_MESSAGE_CATEGORY_SHADER => "Shader",
        _ => "Unknown",
    };

    match severity {
        D3D12_MESSAGE_SEVERITY_CORRUPTION => {
            error!("D3D12 Corruption {category} ({id}): {description}\n{backtrace}");
        }
        D3D12_MESSAGE_SEVERITY_ERROR => {
            error!("D3D12 {category} ({id}): {description}\n{backtrace}");
        }
        D3D12_MESSAGE_SEVERITY_WARNING => {
            warn!("D3D12 {category} ({id}): {description}\n{backtrace}");
        }
        _ => info!("D3D12 {category} ({id}): {description}\n{backtrace}"),
    }
}
