use bevy::prelude::*;
use bevy::render::render_resource::{
    BindGroupLayout, BindGroupLayoutEntry, BlendState, ColorTargetState, ColorWrites,
    CompareFunction, DepthStencilState, Face, FrontFace, PrimitiveState, PrimitiveTopology,
    RenderPipeline, ShaderStages, TextureFormat,
};
use bevy::render::renderer::RenderDevice;

use super::super::compile::{
    EntityKind, PACK_QUADS_SET, PACK_SET, Segment, ShadowCasters, SkyParts, TERRAIN_SET,
};
use super::super::entities::{
    EntityFrame, EntityPrograms, entity_instance_layout, entity_vertex_layout, sky_part_id,
};
use super::super::pipelines::{entity_pass_state, entity_topology};
use super::super::targets::DEPTH_FORMAT;
use super::super::{GPU_LABEL, pipeline_label};
use super::bind::binding_type;
use super::install::{NativeInstalled, bindable};
use super::overlay::{self, Overlay};
use crate::renderer::terrain_pool::draw::packed_vertex_layout;
use crate::renderer::terrain_pool::{
    ORIGIN_ROW, STREAM_CUTOUT, STREAM_SOLID, STREAM_WATER, SlotMeta,
};
use crate::shaderpack::native::api::{
    self, Binding, Draws, Group, OutputSize, Resource as Global, Stage, TerrainStream,
};
use crate::shaderpack::native::build::Built;

const _: () = assert!(api::GLOBALS_GROUP == PACK_SET);
const _: () = assert!(api::TERRAIN_GROUP == TERRAIN_SET);
const _: () =
    assert!(api::SLOT_TABLE_BINDING == crate::renderer::terrain_pool::draw::SLOT_TABLE_BINDING);
const _: () = assert!(api::PACK_QUADS_GROUP == PACK_QUADS_SET);
const _: () = assert!(api::ORIGIN_ROW == ORIGIN_ROW);
const _: () = assert!(api::QUAD_ROW == crate::renderer::packvertex::QUAD_ROW);
const _: () = assert!(api::SLOT_WORDS as usize == size_of::<SlotMeta>() / 16);
const _: () = assert!(std::mem::offset_of!(SlotMeta, origin) == 32);
const _: () = {
    use crate::renderer::sky::SkyPart as Mesh;
    use api::SkyPart as Part;
    assert!(sky_part_id(Mesh::SkyDisc) == Part::Disc.id());
    assert!(sky_part_id(Mesh::DarkDisc) == Part::DarkDisc.id());
    assert!(sky_part_id(Mesh::SunriseFan) == Part::Sunrise.id());
    assert!(sky_part_id(Mesh::Sun) == Part::Sun.id());
    assert!(sky_part_id(Mesh::Moon) == Part::Moon.id());
    assert!(sky_part_id(Mesh::Stars) == Part::Stars.id());
    assert!(sky_part_id(Mesh::EndSky) == Part::End.id());
    assert!(sky_part_id(Mesh::EndFlash) == Part::EndFlash.id());
    assert!(super::super::entities::SKY_PART_SHIFT == 8);
};

#[derive(Clone, Copy, Debug)]
pub(super) struct KindDraw {
    pub(super) group: usize,
    pub(super) stage: usize,
}

pub(super) struct Geometry {
    pub(super) layouts: Vec<BindGroupLayout>,
    pub(super) kinds: [Option<KindDraw>; EntityKind::COUNT],
    pub(super) entities: EntityPrograms,
    pub(super) entity_frame: EntityFrame,
    pub(super) camera: Vec3,
    pub(super) reads_depth: Vec<bool>,
    pub(super) overlay: Overlay,
}

impl Geometry {
    pub(super) fn new(device: &RenderDevice, built: &Built) -> Result<Geometry, String> {
        let max = device.limits().max_color_attachments as usize;
        let mut layouts = Vec::with_capacity(built.geometry.len());
        for group in &built.geometry {
            let what = format!("[geometry.{}] ({})", group.group.name(), group.file);
            if group.writes.len() > max {
                return Err(format!(
                    "{what} writes {} targets; this device draws into at most {max} at once",
                    group.writes.len()
                ));
            }
            for &t in &group.writes {
                let target = &built.targets[t];
                if target.size != OutputSize::Scale(1.0) {
                    return Err(format!(
                        "{what} writes `{}`, which is not the view's size and cannot share its depth",
                        target.id
                    ));
                }
            }
            layouts.push(globals_layout(device, built, &group.bindings, &what)?);
        }
        let kinds = kind_draws(built)?;
        let mut segments = EntityKind::ALL.map(EntityKind::segment);
        for (at, draw) in kinds.iter().enumerate() {
            if let Some(draw) = draw {
                segments[at] = segment(built.geometry[draw.group].stages[draw.stage].stage);
            }
        }
        let shadow = built.shadow.as_ref();
        let mut drawn = kinds.map(|draw| draw.is_some());
        drawn[EntityKind::ShadowCaster.index()] = shadow.is_some();
        for kind in overlay::KINDS {
            drawn[kind.index()] = true;
            segments[kind.index()] = Segment::Display;
        }
        let entities = EntityPrograms {
            kinds: drawn,
            casters: ShadowCasters {
                entities: true,
                player: true,
                block_entities: true,
            },
            sky_parts: sky_parts(&built.sky_parts),
            entity_ids: Default::default(),
            caster_reach: shadow.map_or(0.0, |s| super::cascade::depth_reach(s.distance as f32)),
            shadow_usable: shadow.is_some(),
            segments,
            fallbacks: false,
            display_copies: false,
        };
        Ok(Geometry {
            layouts,
            kinds,
            entities,
            entity_frame: EntityFrame::default(),
            camera: Vec3::ZERO,
            reads_depth: built
                .geometry
                .iter()
                .map(|g| g.bindings.iter().any(|b| b.resource == Global::SceneDepth))
                .collect(),
            overlay: Overlay::new(device, built)?,
        })
    }
}

pub(super) fn globals_layout(
    device: &RenderDevice,
    built: &Built,
    bindings: &[Binding],
    what: &str,
) -> Result<BindGroupLayout, String> {
    let mut entries = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let ty = binding_type(built, binding)
            .filter(|_| bindable(binding.resource))
            .ok_or_else(|| {
                format!(
                    "{what} uses `{}`, which this build does not bind yet",
                    binding.name
                )
            })?;
        entries.push(BindGroupLayoutEntry {
            binding: binding.binding,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty,
            count: None,
        });
    }
    Ok(device.create_bind_group_layout(gpu_label!("native geometry"), &entries))
}

fn kind_named(name: &str) -> Option<EntityKind> {
    EntityKind::ALL
        .into_iter()
        .find(|kind| format!("{kind:?}") == name)
}

fn sky_parts(parts: &[api::SkyPart]) -> SkyParts {
    let has = |part: api::SkyPart| parts.contains(&part);
    SkyParts {
        sky: has(api::SkyPart::Disc),
        sunrise: has(api::SkyPart::Sunrise),
        void: has(api::SkyPart::DarkDisc),
        end: has(api::SkyPart::End),
        end_flash: has(api::SkyPart::EndFlash),
        sun: has(api::SkyPart::Sun),
        moon: has(api::SkyPart::Moon),
        stars: has(api::SkyPart::Stars),
    }
}

fn kind_draws(built: &Built) -> Result<[Option<KindDraw>; EntityKind::COUNT], String> {
    let mut kinds = [None; EntityKind::COUNT];
    for (group, built_group) in built.geometry.iter().enumerate() {
        for (stage, built_stage) in built_group.stages.iter().enumerate() {
            let Draws::Entities(names) = built_stage.draws else {
                continue;
            };
            for name in names {
                let kind = kind_named(name).ok_or_else(|| {
                    format!(
                        "stage `{:?}` draws `{name}`, which this client has no meshes of",
                        built_stage.stage
                    )
                })?;
                kinds[kind.index()] = Some(KindDraw { group, stage });
            }
        }
    }
    Ok(kinds)
}

fn segment(stage: Stage) -> Segment {
    match (stage, stage.info().group) {
        (Stage::Sky, _) => Segment::Sky,
        (Stage::Hand, _) => Segment::HandSolid,
        (Stage::Clouds, _) => Segment::AfterTranslucent,
        (_, Group::Opaque) => Segment::World,
        (_, Group::Translucent) => Segment::WorldTranslucent,
    }
}

pub(super) struct GroupPipelines {
    pub(super) terrain: Vec<(u32, RenderPipeline)>,
    pub(super) entities: [Option<(RenderPipeline, RenderPipeline)>; EntityKind::COUNT],
}

pub(super) struct GroupLayouts<'a> {
    pub(super) globals: &'a BindGroupLayout,
    pub(super) terrain: &'a BindGroupLayout,
    pub(super) quads: &'a BindGroupLayout,
}

pub(super) fn reversed(compare: CompareFunction) -> CompareFunction {
    match compare {
        CompareFunction::Less => CompareFunction::Greater,
        CompareFunction::LessEqual => CompareFunction::GreaterEqual,
        CompareFunction::Greater => CompareFunction::Less,
        CompareFunction::GreaterEqual => CompareFunction::LessEqual,
        other => other,
    }
}

struct Draw<'a> {
    label: String,
    layout: &'a wgpu::PipelineLayout,
    vertex: &'a str,
    fragment: &'a str,
    buffers: Vec<bevy::mesh::VertexBufferLayout>,
    primitive: PrimitiveState,
    depth: Option<(bool, CompareFunction)>,
    blend: Option<BlendState>,
}

pub(super) fn group_pipelines(
    device: &RenderDevice,
    installed: &NativeInstalled,
    index: usize,
    layouts: GroupLayouts<'_>,
    formats: &[TextureFormat],
) -> GroupPipelines {
    let generation = installed.generation;
    let built = &installed.built;
    let group = &built.geometry[index];
    let module = device.create_and_validate_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&format!(
            "{GPU_LABEL}/{}/{generation}/{}",
            installed.name, group.file
        )),
        source: wgpu::ShaderSource::Wgsl(group.wgsl.as_str().into()),
    });
    let constants: Vec<(&str, f64)> = group
        .overrides
        .iter()
        .map(|(id, value)| (id.as_str(), *value))
        .collect();
    let pipeline_layout = |groups: &[&BindGroupLayout]| {
        let groups: Vec<Option<&wgpu::BindGroupLayout>> =
            groups.iter().map(|l| Some(&***l)).collect();
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&pipeline_label(
                generation,
                format_args!("native {}", group.group.name()),
            )),
            bind_group_layouts: &groups,
            immediate_size: 0,
        })
    };
    let terrain_layout = pipeline_layout(&[layouts.globals, layouts.terrain, layouts.quads]);
    let mesh_layout = pipeline_layout(&[layouts.globals]);
    let targets = |blend: Option<BlendState>| -> Vec<Option<ColorTargetState>> {
        group
            .writes
            .iter()
            .map(|&t| {
                Some(ColorTargetState {
                    format: formats[t],
                    blend: blend.filter(|_| built.targets[t].format.info().filterable),
                    write_mask: ColorWrites::ALL,
                })
            })
            .collect()
    };
    let make = |draw: Draw<'_>| -> RenderPipeline {
        let buffers: Vec<wgpu::VertexBufferLayout> = draw
            .buffers
            .iter()
            .map(|b| wgpu::VertexBufferLayout {
                array_stride: b.array_stride,
                step_mode: b.step_mode,
                attributes: &b.attributes,
            })
            .collect();
        let options = || wgpu::PipelineCompilationOptions {
            constants: constants.as_slice(),
            ..Default::default()
        };
        let color_targets = targets(draw.blend);
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&draw.label),
            layout: Some(draw.layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some(draw.vertex),
                compilation_options: options(),
                buffers: &buffers,
            },
            primitive: draw.primitive,
            depth_stencil: draw.depth.map(|(write, compare)| DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(write),
                depth_compare: Some(compare),
                stencil: default(),
                bias: default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(draw.fragment),
                compilation_options: options(),
                targets: &color_targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let primitive = |topology: PrimitiveTopology, cull: Option<Face>| PrimitiveState {
        topology,
        front_face: FrontFace::Ccw,
        cull_mode: cull,
        ..default()
    };
    let terrain_blend = (group.group == Group::Translucent).then_some(BlendState::ALPHA_BLENDING);

    let mut terrain = Vec::new();
    let mut entities: [Option<(RenderPipeline, RenderPipeline)>; EntityKind::COUNT] =
        std::array::from_fn(|_| None);
    for stage in &group.stages {
        let cutout = stage.cutout.as_deref().unwrap_or(&stage.fragment);
        match stage.draws {
            Draws::Terrain(streams) => {
                for stream in streams {
                    let (number, fragment) = match stream {
                        TerrainStream::Solid => (STREAM_SOLID, stage.fragment.as_str()),
                        TerrainStream::Cutout => (STREAM_CUTOUT, cutout),
                        TerrainStream::Water => (STREAM_WATER, stage.fragment.as_str()),
                    };
                    terrain.push((
                        number,
                        make(Draw {
                            label: pipeline_label(
                                generation,
                                format_args!("native {:?} stream {number}", stage.stage),
                            ),
                            layout: &terrain_layout,
                            vertex: &stage.vertex,
                            fragment,
                            buffers: vec![packed_vertex_layout()],
                            primitive: primitive(PrimitiveTopology::TriangleList, Some(Face::Back)),
                            depth: Some((true, CompareFunction::GreaterEqual)),
                            blend: terrain_blend,
                        }),
                    ));
                }
            }
            Draws::Entities(names) => {
                for kind in names.iter().filter_map(|name| kind_named(name)) {
                    let variant = |second: bool| {
                        let state = entity_pass_state(kind, second);
                        make(Draw {
                            label: pipeline_label(
                                generation,
                                format_args!(
                                    "native {:?} {kind:?} {}",
                                    stage.stage,
                                    u8::from(second)
                                ),
                            ),
                            layout: &mesh_layout,
                            vertex: &stage.vertex,
                            fragment: if state.blend.is_none() {
                                cutout
                            } else {
                                stage.fragment.as_str()
                            },
                            buffers: vec![entity_vertex_layout(), entity_instance_layout()],
                            primitive: primitive(entity_topology(kind), state.cull),
                            depth: state
                                .depth
                                .map(|(write, compare)| (write, reversed(compare))),
                            blend: state.blend,
                        })
                    };
                    entities[kind.index()] = Some((variant(false), variant(true)));
                }
            }
            Draws::Nothing => {}
        }
    }
    GroupPipelines { terrain, entities }
}
