use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroupLayout, ColorTargetState, ColorWrites, DepthStencilState, FrontFace, PrimitiveState,
    RenderPipeline, TextureFormat,
};
use bevy::render::renderer::RenderDevice;

use super::super::compile::EntityKind;
use super::super::entities::{entity_instance_layout, entity_vertex_layout};
use super::super::pipelines::{entity_pass_state, entity_topology};
use super::super::targets::DEPTH_FORMAT;
use super::super::{GPU_LABEL, pipeline_label};
use super::geometry::{globals_layout, reversed};
use super::install::NativeInstalled;
use crate::shaderpack::native::api::{self, Binding};
use crate::shaderpack::native::build::Built;

pub(super) const KINDS: [EntityKind; 2] = [EntityKind::DamagedBlock, EntityKind::Line];

pub(super) type OverlayPipelines = [(RenderPipeline, RenderPipeline); 2];

#[derive(Clone)]
pub(super) struct Overlay {
    wgsl: Arc<str>,
    pub(super) bindings: Vec<Binding>,
    pub(super) layout: BindGroupLayout,
}

impl Overlay {
    pub(super) fn new(device: &RenderDevice, built: &Built) -> Result<Overlay, String> {
        let (wgsl, bindings) = api::overlay_module();
        let layout = globals_layout(device, built, &bindings, "the engine's overlays")?;
        Ok(Overlay {
            wgsl: wgsl.into(),
            bindings,
            layout,
        })
    }
}

pub(super) fn overlay_pipelines(
    device: &RenderDevice,
    installed: &NativeInstalled,
    overlay: &Overlay,
    format: TextureFormat,
) -> OverlayPipelines {
    let generation = installed.generation;
    let module = device.create_and_validate_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&format!(
            "{GPU_LABEL}/{}/{generation}/overlay.wgsl",
            installed.name
        )),
        source: wgpu::ShaderSource::Wgsl(overlay.wgsl.as_ref().into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&pipeline_label(generation, format_args!("native overlays"))),
        bind_group_layouts: &[Some(&*overlay.layout)],
        immediate_size: 0,
    });
    let layouts = [entity_vertex_layout(), entity_instance_layout()];
    let buffers: Vec<wgpu::VertexBufferLayout> = layouts
        .iter()
        .map(|b| wgpu::VertexBufferLayout {
            array_stride: b.array_stride,
            step_mode: b.step_mode,
            attributes: &b.attributes,
        })
        .collect();
    let make = |kind: EntityKind, second: bool| -> RenderPipeline {
        let state = entity_pass_state(kind, second);
        let fragment = match kind {
            EntityKind::DamagedBlock => api::OVERLAY_CRACKS,
            _ => api::OVERLAY_LINE,
        };
        let targets = [Some(ColorTargetState {
            format,
            blend: state.blend,
            write_mask: ColorWrites::ALL,
        })];
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&pipeline_label(
                generation,
                format_args!("native overlay {kind:?} {}", u8::from(second)),
            )),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some(api::OVERLAY_VERTEX),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            primitive: PrimitiveState {
                topology: entity_topology(kind),
                front_face: FrontFace::Ccw,
                cull_mode: state.cull,
                ..default()
            },
            depth_stencil: state.depth.map(|(write, compare)| DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(write),
                depth_compare: Some(reversed(compare)),
                stencil: default(),
                bias: default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(fragment),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    KINDS.map(|kind| (make(kind, false), make(kind, true)))
}
