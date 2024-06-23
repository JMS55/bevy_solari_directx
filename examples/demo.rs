use bevy::{
    app::{App, Startup},
    prelude::{Commands, IntoSystemConfigs, Query, Res, ResMut, Resource},
    DefaultPlugins,
};
use bevy_directx::{
    update_render_target,
    windows::Win32::Graphics::{
        Direct3D::*,
        Direct3D12::*,
        Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
    },
    BevyDirectXPlugin, Gpu, Render, WindowRenderTarget,
};
use std::mem::{transmute_copy, ManuallyDrop};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, BevyDirectXPlugin))
        .add_systems(Startup, setup_pipeline)
        .add_systems(Render, render_frame.after(update_render_target))
        .run();
}

#[derive(Resource)]
struct Pipeline {
    root_signature: ID3D12RootSignature,
    pipeline: ID3D12PipelineState,
}

fn setup_pipeline(gpu: Res<Gpu>, mut commands: Commands) {
    let shader_vs = include_bytes!("../assets/triangle_vs.dxil");
    let shader_ps = include_bytes!("../assets/triangle_ps.dxil");

    let root_signature = gpu
        .create_root_signature(&[], &[], D3D12_ROOT_SIGNATURE_FLAG_NONE)
        .unwrap();
    let pipeline_desc = pipeline_desc(&root_signature, shader_vs, shader_ps);
    let pipeline = unsafe { gpu.device.CreateGraphicsPipelineState(&pipeline_desc) }.unwrap();

    commands.insert_resource(Pipeline {
        root_signature,
        pipeline,
    });
}

fn render_frame(
    mut gpu: ResMut<Gpu>,
    pipeline: Res<Pipeline>,
    render_target: Query<&WindowRenderTarget>,
) {
    let Ok(render_target) = render_target.get_single() else {
        return;
    };
    let (render_target_texture, render_target_rtv) = render_target.get_rtv();

    let command_list = gpu.reset_commands(Some(&pipeline.pipeline)).unwrap();
    unsafe {
        // TODO: Enhanced barriers
        command_list.SetGraphicsRootSignature(&pipeline.root_signature);
        command_list.ResourceBarrier(&[D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: transmute_copy(render_target_texture),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_PRESENT,
                    StateAfter: D3D12_RESOURCE_STATE_RENDER_TARGET,
                }),
            },
        }]);
        command_list.OMSetRenderTargets(1, Some(&render_target_rtv), false, None);
        command_list.ClearRenderTargetView(render_target_rtv, &[0.0, 0.0, 0.0, 1.0], None);
        command_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        command_list.DrawInstanced(3, 1, 0, 0);
        command_list.ResourceBarrier(&[D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: transmute_copy(render_target_texture),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_RENDER_TARGET,
                    StateAfter: D3D12_RESOURCE_STATE_PRESENT,
                }),
            },
        }]);
    }

    gpu.execute_command_list().unwrap();
    render_target.present();
    gpu.signal_fence().unwrap();
}

fn pipeline_desc(
    root_signature: &ID3D12RootSignature,
    shader_vs: &[u8],
    shader_ps: &[u8],
) -> D3D12_GRAPHICS_PIPELINE_STATE_DESC {
    D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: unsafe { transmute_copy(root_signature) },

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
