use bevy::{
    app::{App, Startup},
    prelude::{Commands, IntoSystemConfigs, Query, Res, ResMut, Resource},
    DefaultPlugins,
};
use bevy_directx::{
    update_swapchains,
    windows::Win32::{
        Foundation::HANDLE,
        Graphics::{
            Direct3D::*,
            Direct3D12::*,
            Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
        },
        System::Threading::{CreateEventW, WaitForSingleObjectEx},
    },
    BevyDirectXPlugin, Gpu, Render, WindowRenderTarget,
};
use std::mem::ManuallyDrop;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, BevyDirectXPlugin))
        .add_systems(Startup, setup_resources)
        .add_systems(Render, render_frame.after(update_swapchains))
        .run();
}

#[derive(Resource)]
struct DemoResources {
    root_signature: ID3D12RootSignature,
    pipeline: ID3D12PipelineState,
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12GraphicsCommandList7,
    fence: ID3D12Fence,
    fence_event: HANDLE,
    fence_counter: u64,
}

fn setup_resources(gpu: Res<Gpu>, mut commands: Commands) {
    let shader_vs = include_bytes!("../assets/triangle_vs.dxil");
    let shader_ps = include_bytes!("../assets/triangle_ps.dxil");

    let root_signature = gpu
        .create_root_signature(&[], &[], D3D12_ROOT_SIGNATURE_FLAG_NONE)
        .unwrap();
    let pipeline_desc = pipeline_desc(root_signature.clone(), shader_vs, shader_ps);
    let pipeline = unsafe { gpu.device.CreateGraphicsPipelineState(&pipeline_desc) }.unwrap();

    let command_allocator = unsafe {
        gpu.device
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
    }
    .unwrap();
    let command_list = unsafe {
        gpu.device.CreateCommandList(
            0,
            D3D12_COMMAND_LIST_TYPE_DIRECT,
            &command_allocator,
            &pipeline,
        )
    }
    .unwrap();

    let fence = unsafe { gpu.device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }.unwrap();
    let fence_event = unsafe { CreateEventW(None, false, false, None) }.unwrap();

    commands.insert_resource(DemoResources {
        root_signature,
        pipeline,
        command_allocator,
        command_list,
        fence,
        fence_event,
        fence_counter: 0,
    });
}

fn render_frame(
    gpu: Res<Gpu>,
    mut resources: ResMut<DemoResources>,
    render_target: Query<&WindowRenderTarget>,
) {
    let render_target = render_target.single();
    let (rtv, rtv_handle) = render_target.get_rtv();

    unsafe {
        WaitForSingleObjectEx(resources.fence_event, 1000, true);

        resources.command_allocator.Reset().unwrap();
        let command_list = &resources.command_list;
        command_list
            .Reset(&resources.command_allocator, &resources.pipeline)
            .unwrap();

        // TODO: Enhanced barriers
        command_list.SetGraphicsRootSignature(&resources.root_signature);
        command_list.ResourceBarrier(&[D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: ManuallyDrop::new(Some(rtv.clone())),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_PRESENT,
                    StateAfter: D3D12_RESOURCE_STATE_RENDER_TARGET,
                }),
            },
        }]);
        command_list.OMSetRenderTargets(1, Some(&rtv_handle), false, None);
        command_list.ClearRenderTargetView(rtv_handle, &[0.0, 0.0, 0.0, 1.0], None);
        command_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        command_list.DrawInstanced(3, 1, 0, 0);
        command_list.ResourceBarrier(&[D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: ManuallyDrop::new(Some(rtv)),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_RENDER_TARGET,
                    StateAfter: D3D12_RESOURCE_STATE_PRESENT,
                }),
            },
        }]);
        command_list.Close().unwrap();

        gpu.queue
            .ExecuteCommandLists(&[Some(command_list.clone().into())]);
        render_target.present();

        gpu.queue
            .Signal(&resources.fence, resources.fence_counter)
            .unwrap();
        resources.fence_counter += 1;
        resources
            .fence
            .SetEventOnCompletion(resources.fence_counter, resources.fence_event)
            .unwrap();
    }
}

fn pipeline_desc(
    root_signature: ID3D12RootSignature,
    shader_vs: &[u8],
    shader_ps: &[u8],
) -> D3D12_GRAPHICS_PIPELINE_STATE_DESC {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: ManuallyDrop::new(Some(root_signature)),
        VS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: shader_vs.as_ptr() as _,
            BytecodeLength: shader_vs.len(),
        },
        PS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: shader_ps.as_ptr() as _,
            BytecodeLength: shader_ps.len(),
        },
        BlendState: D3D12_BLEND_DESC {
            RenderTarget: [D3D12_RENDER_TARGET_BLEND_DESC {
                RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                ..Default::default()
            }; 8],
            ..Default::default()
        },
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: D3D12_CULL_MODE_BACK,
            FrontCounterClockwise: true.into(),
            DepthClipEnable: true.into(),
            ..Default::default()
        },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        RTVFormats: [
            DXGI_FORMAT_R8G8B8A8_UNORM,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
            DXGI_FORMAT_UNKNOWN,
        ],
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            ..Default::default()
        },
        ..Default::default()
    }
}
