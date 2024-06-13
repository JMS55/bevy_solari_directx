use bevy::prelude::Resource;
use windows::{
    core::Error,
    Win32::Graphics::{
        Direct3D::D3D_FEATURE_LEVEL_12_2,
        Direct3D12::{
            D3D12CreateDevice, D3D12GetDebugInterface, ID3D12CommandAllocator, ID3D12CommandQueue,
            ID3D12Debug3, ID3D12Device9, D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC,
        },
        Dxgi::{
            CreateDXGIFactory2, IDXGIAdapter4, IDXGIFactory7, DXGI_CREATE_FACTORY_DEBUG,
            DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
        },
    },
};

#[derive(Resource)]
pub struct Gpu {
    device: ID3D12Device9,
    queue: ID3D12CommandQueue,
    command_allocator: ID3D12CommandAllocator,
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

        // Queue
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;

        // Command allocator
        let command_allocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;

        Ok(Self {
            device,
            queue,
            command_allocator,
        })
    }
}
