use bevy::prelude::{error, info, warn, Resource};
use std::{
    backtrace::{Backtrace, BacktraceStatus},
    os::raw::c_void,
    ptr, slice, str,
};
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

/// Central interface for managing GPU resources and rendering work.
#[derive(Resource)]
pub struct Gpu {
    pub factory: IDXGIFactory7,
    pub device: ID3D12Device9,
    pub queue: ID3D12CommandQueue,
}

impl Gpu {
    pub fn new() -> Result<Self, Error> {
        unsafe {
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
            info_queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_ERROR, true)?;
            let mut cookie = 0;
            info_queue.RegisterMessageCallback(
                Some(log_debug_layer_message),
                D3D12_MESSAGE_CALLBACK_FLAG_NONE,
                ptr::null_mut(),
                &mut cookie,
            )?;
            if cookie == 0 {
                panic!("BevyDirectX: Failed to register debug layer callback");
            }

            // Queue
            let queue: ID3D12CommandQueue =
                device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                    Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                    ..Default::default()
                })?;

            Ok(Self {
                factory,
                device,
                queue,
            })
        }
    }

    pub fn create_root_signature(
        &self,
        parameters: &[D3D12_ROOT_PARAMETER1],
        static_samplers: &[D3D12_STATIC_SAMPLER_DESC],
        flags: D3D12_ROOT_SIGNATURE_FLAGS,
    ) -> Result<ID3D12RootSignature, Error> {
        let desc = D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
            Version: D3D_ROOT_SIGNATURE_VERSION_1_1,
            Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                Desc_1_1: D3D12_ROOT_SIGNATURE_DESC1 {
                    NumParameters: parameters.len() as u32,
                    pParameters: parameters.as_ptr(),
                    NumStaticSamplers: static_samplers.len() as u32,
                    pStaticSamplers: static_samplers.as_ptr(),
                    Flags: flags,
                },
            },
        };

        let mut root_signature = None;
        let mut error = None;
        unsafe {
            D3D12SerializeVersionedRootSignature(&desc, &mut root_signature, Some(&mut error))?;
        }

        if let Some(error) = error {
            let error = unsafe {
                slice::from_raw_parts(error.GetBufferPointer() as *const u8, error.GetBufferSize())
            };
            let error = str::from_utf8(error).unwrap();
            panic!("BevyDirectX: Failed to create root signature: {error}");
        }
        let root_signature = root_signature.unwrap();

        unsafe {
            let root_signature = slice::from_raw_parts(
                root_signature.GetBufferPointer() as *const u8,
                root_signature.GetBufferSize(),
            );
            self.device.CreateRootSignature(0, root_signature)
        }
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

    let backtrace = Backtrace::capture();
    let backtrace = if let BacktraceStatus::Disabled = backtrace.status() {
        "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace".to_owned()
    } else {
        format!("{backtrace}")
    };

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
