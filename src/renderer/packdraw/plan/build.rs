use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindingResource, Buffer, BufferDescriptor,
    BufferUsages, Sampler, TextureView,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::view::ExtractedView;

use super::super::GPU_LABEL;
use super::super::compile::{ScreenStage, Segment, Step};
use super::super::formats::wgpu_format;
use super::super::frame::{Install, PackRender, Sized, Stage, Usage};
use super::super::fullscreen::Blit;
use super::super::install::Installed;
use super::super::pipelines::{Building, Pipelines};
use super::super::sources::{Depth, Stub, TextureSource};
use super::super::targets::{
    ShadowTargets, Statics, Target, Targets, display_view, image_target, storage_buffer,
    zero_layout,
};
use super::super::uniforms::PackUniforms;
use super::super::usage::depth_index;
use super::*;
use crate::renderer::terrain_pool::{TerrainTextures, TerrainView, stream};
use crate::shaderpack::directives::{MAX_COLOR_TARGETS, SampleKind};
use crate::shaderpack::pipeline::Binding;
use crate::shaderpack::programs::Geometry;

const MAX_ATTACHMENTS: usize = 8;

fn pass_limit() -> Option<usize> {
    static LIMIT: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
    *LIMIT.get_or_init(|| std::env::var("MC_SHADERPACK_PASSES").ok()?.parse().ok())
}

pub(crate) fn prepare_plan(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    views: Query<&ExtractedView, With<TerrainView>>,
    textures: Option<Res<TerrainTextures>>,
    images: Res<RenderAssets<GpuImage>>,
    render: Option<ResMut<PackRender>>,
) {
    let Some(mut render) = render else { return };
    let PackRender {
        process, install, ..
    } = &mut *render;
    let (Some(process), Some(install)) = (process.as_mut(), install.as_deref_mut()) else {
        return;
    };
    let Some(view) = views.iter().next() else {
        return;
    };
    let Some(textures) = textures else { return };
    let (Some(atlas), Some(lightmap)) =
        (images.get(&textures.atlas), images.get(&textures.lightmap))
    else {
        return;
    };
    let Install {
        installed,
        usage,
        gpu,
        stage,
        ..
    } = install;
    let Stage::Started {
        pipelines: Building::Ready(pipelines),
        sized,
    } = stage
    else {
        return;
    };
    let size = UVec2::new(view.viewport.z, view.viewport.w).max(UVec2::ONE);
    let key = PlanKey {
        generation: installed.generation,
        size,
        atlas: atlas.texture_view.id(),
        lightmap: lightmap.texture_view.id(),
    };
    if sized.as_ref().is_some_and(|s| s.key == key) {
        return;
    }
    let (targets, setup_done, fully_cleared) = match sized.take() {
        Some(old) if old.targets.size == size => (
            old.targets,
            old.setup_done.into_inner(),
            old.fully_cleared.into_inner(),
        ),
        _ => {
            for (at, buffer) in installed.buffers.iter().enumerate() {
                if buffer.relative.is_some() {
                    gpu.buffers[at] = storage_buffer(&device, buffer, [size.x, size.y]);
                }
            }
            for (index, image) in installed.custom_images.iter().enumerate() {
                if matches!(
                    image.size,
                    crate::shaderpack::customimages::ImageSize::Relative { .. }
                ) {
                    gpu.custom_images[index] = image_target(&device, image, size);
                    if gpu.image_snapshots[index].is_some() {
                        gpu.image_snapshots[index] = Some(image_target(&device, image, size));
                    }
                }
            }
            (
                Targets::new(
                    &device,
                    installed,
                    usage.mip_targets,
                    usage.copy_no_hand,
                    size,
                ),
                false,
                false,
            )
        }
    };
    if !device.features().contains(wgpu::Features::CLEAR_TEXTURE) {
        let needed = installed
            .custom_images
            .iter()
            .zip(&gpu.custom_images)
            .filter(|(image, _)| image.clear)
            .map(|(_, target)| zero_layout(&target.texture).1)
            .max()
            .unwrap_or(0);
        if needed > 0 && gpu.zeros.as_ref().is_none_or(|z| z.size() < needed) {
            gpu.zeros = Some(device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{GPU_LABEL} image zeros")),
                size: needed,
                usage: BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
        }
    }
    if usage.mip_targets != 0 {
        let mipgen = process
            .mipgen
            .get_or_insert_with(|| super::super::mipmaps::MipGen::new(&device));
        for index in (0..MAX_COLOR_TARGETS).filter(|n| usage.mip_targets & (1 << n) != 0) {
            mipgen.ensure(
                &device,
                wgpu_format(installed.targets.settings[index].format),
            );
        }
    }
    if usage.shadow_filtered.iter().any(|f| *f) {
        process
            .depth_copy
            .get_or_insert_with(|| super::super::depthcopy::DepthCopy::new(&device));
    }
    let pack_atlas = process.pack_atlas(&device, &queue, atlas);
    let resources = PlanResources {
        installed,
        usage,
        targets: &targets,
        statics: &process.statics,
        noise: &gpu.noise,
        images: &gpu.images,
        mipgen: process.mipgen.as_ref(),
        depth_copy: process.depth_copy.as_ref(),
        shadow: gpu.shadow_targets.as_ref(),
        custom_images: &gpu.custom_images,
        image_stubs: &gpu.image_stubs,
        buffers: &gpu.buffers,
        image_snapshots: &gpu.image_snapshots,
        atlas: pack_atlas,
        lightmap: display_view(lightmap),
        atlas_sampler: atlas.sampler.clone(),
        lightmap_sampler: lightmap.sampler.clone(),
        layouts: &gpu.layouts,
        uniforms: &gpu.uniforms,
        device: &device,
    };
    let first = resources.plan(pipelines, &process.blit, Flips::default());
    let end = first.end;
    let plans = if end.0 == 0 {
        vec![first]
    } else {
        vec![first, resources.plan(pipelines, &process.blit, end)]
    };
    *sized = Some(Sized {
        key,
        targets,
        plans,
        setup_done: std::sync::atomic::AtomicBool::new(setup_done),
        fully_cleared: std::sync::atomic::AtomicBool::new(fully_cleared),
    });
}

struct PlanResources<'a> {
    installed: &'a Installed,
    usage: &'a Usage,
    targets: &'a Targets,
    statics: &'a Statics,
    noise: &'a Target,
    images: &'a [Target],
    mipgen: Option<&'a super::super::mipmaps::MipGen>,
    depth_copy: Option<&'a super::super::depthcopy::DepthCopy>,
    shadow: Option<&'a ShadowTargets>,
    custom_images: &'a [Target],
    image_stubs: &'a [Target],
    buffers: &'a [Buffer],
    image_snapshots: &'a [Option<Target>],
    atlas: TextureView,
    lightmap: TextureView,
    atlas_sampler: Sampler,
    lightmap_sampler: Sampler,
    layouts: &'a [BindGroupLayout],
    uniforms: &'a PackUniforms,
    device: &'a RenderDevice,
}

impl PlanResources<'_> {
    fn attachments(&self, buffers: &[u8], half: impl Fn(u8) -> usize) -> Vec<Attachment> {
        buffers
            .iter()
            .map(|n| Attachment {
                target: *n,
                view: self.targets.color(*n, half(*n)).clone(),
            })
            .collect()
    }

    fn view(
        &self,
        source: TextureSource,
        flips: Flips,
        drawing: u16,
        geometry: bool,
        mipmapped: u16,
        shadow_pass: bool,
    ) -> &TextureView {
        let shadow = self.shadow.filter(|_| !shadow_pass);
        match source {
            TextureSource::ShadowDepth(depth) => match shadow {
                Some(shadow) => match (&shadow.filtered[depth_index(depth)], depth) {
                    (Some(filtered), _) => &filtered.view,
                    (None, Depth::All) => &shadow.depth.view,
                    (None, Depth::Opaque) => &shadow.depth_opaque.view,
                },
                None => &self.statics.white_2d.view,
            },
            TextureSource::ShadowColor(index) => shadow
                .and_then(|shadow| shadow.color[usize::from(index)].as_ref())
                .map_or(&self.statics.white_2d.view, |target| &target.view),
            TextureSource::Color(index) if drawing & (1u16 << index) != 0 => {
                self.targets.color(index, flips.other(index))
            }
            TextureSource::Color(index) if mipmapped & (1u16 << index) != 0 => {
                self.targets.color_mipmapped(index, flips.current(index))
            }
            TextureSource::Color(index) => self.targets.color(index, flips.current(index)),
            TextureSource::Depth(Depth::All) if geometry => &self.targets.depth_opaque.view,
            TextureSource::Depth(Depth::All) => &self.targets.depth.view,
            TextureSource::Depth(Depth::Opaque) => &self.targets.depth_opaque.view,
            TextureSource::DepthNoHand => self
                .targets
                .depth_no_hand
                .as_ref()
                .map_or(&self.targets.depth_opaque.view, |t| &t.view),
            TextureSource::Noise => &self.noise.view,
            TextureSource::EntityTexture => &self.statics.white_2d.view,
            TextureSource::Image { index, .. } => &self.custom_images[usize::from(index)].view,
            TextureSource::Custom(index) => &self.images[usize::from(index)].view,
            TextureSource::Atlas => &self.atlas,
            TextureSource::Lightmap => &self.lightmap,
            TextureSource::Stub(Stub::White2d) => &self.statics.white_2d.view,
            TextureSource::Stub(Stub::Zero3d) => &self.statics.zero_3d.view,
            TextureSource::Stub(Stub::Uint2d) => &self.statics.uint_2d.view,
            TextureSource::Stub(Stub::Uint3d) => &self.statics.uint_3d.view,
            TextureSource::Stub(Stub::FlatNormal) => &self.statics.flat_normal.view,
            TextureSource::Stub(Stub::Black2d) => &self.statics.black_2d.view,
        }
    }

    fn sampler(&self, source: TextureSource, mipmapped: u16) -> &Sampler {
        match source {
            TextureSource::Color(index) if mipmapped & (1u16 << index) != 0 => {
                &self.statics.mip_linear
            }
            TextureSource::ShadowDepth(depth)
                if self
                    .shadow
                    .is_some_and(|shadow| shadow.filtered[depth_index(depth)].is_some()) =>
            {
                &self.statics.linear
            }
            TextureSource::Atlas => &self.atlas_sampler,
            TextureSource::Lightmap => &self.lightmap_sampler,
            TextureSource::EntityTexture => &self.statics.nearest,
            TextureSource::Noise => &self.statics.repeat,
            TextureSource::Custom(index) => {
                let image = &self.installed.images[usize::from(index)];
                match (image.blur, image.clamp) {
                    (true, true) => &self.statics.linear,
                    (false, true) => &self.statics.nearest,
                    (true, false) => &self.statics.repeat,
                    (false, false) => &self.statics.repeat_nearest,
                }
            }
            _ => match source.sample_kind(&self.installed.targets) {
                SampleKind::Float => &self.statics.linear,
                _ => &self.statics.nearest,
            },
        }
    }

    fn bind_group(&self, index: usize, flips: Flips, drawing: u16, geometry: bool) -> BindGroup {
        self.bind_template(index, index, flips, drawing, geometry, 0)
            .create(self.device, None)
    }

    fn bind_template(
        &self,
        index: usize,
        block: usize,
        flips: Flips,
        drawing: u16,
        geometry: bool,
        parity: usize,
    ) -> BindTemplate {
        let program = &self.installed.programs[index];
        let mipmapped = if geometry {
            0
        } else {
            program.mipmapped & self.targets.mipmapped
        };
        let shadow_pass = program.kind.is_shadow();
        let conflicts = self.usage.image_conflicts[index][parity];
        let used = match &program.compute_parity {
            Some(variants) => Some(variants[parity].1.as_slice()),
            None => program.compute_used.as_deref(),
        };
        let unreached = |binding: u32| used.is_some_and(|used| !used.contains(&binding));
        let entries = program
            .layout
            .bindings
            .iter()
            .map(|binding| {
                let resource = match binding {
                    Binding::Uniform { .. } => {
                        let (offset, size) = self.uniforms.range(block);
                        Owned::Range(self.uniforms.buffer.clone(), offset, size)
                    }
                    Binding::Texture { binding, .. } => {
                        let source = program.texture_source(*binding);
                        let substitute = match source {
                            TextureSource::Image { index, .. } if unreached(*binding) => {
                                Some(&self.image_stubs[usize::from(index)])
                            }
                            TextureSource::Image { index, .. } if conflicts & (1 << index) != 0 => {
                                self.image_snapshots[usize::from(index)].as_ref()
                            }
                            _ => None,
                        };
                        Owned::View(match substitute {
                            Some(target) => target.view.clone(),
                            None => self
                                .view(source, flips, drawing, geometry, mipmapped, shadow_pass)
                                .clone(),
                        })
                    }
                    Binding::Sampler { binding, .. } => Owned::Sampler(
                        self.sampler(program.texture_source(binding - 1), mipmapped)
                            .clone(),
                    ),
                    Binding::Image { name, binding, .. } => {
                        let index = self
                            .installed
                            .custom_images
                            .iter()
                            .position(|i| i.name == *name)
                            .expect("compilation refused an undeclared image");
                        let target = if unreached(*binding) {
                            &self.image_stubs[index]
                        } else {
                            &self.custom_images[index]
                        };
                        Owned::View(target.view.clone())
                    }
                    Binding::Buffer { index, .. } => {
                        let at = self
                            .installed
                            .buffers
                            .iter()
                            .position(|b| b.index == *index)
                            .expect("compilation refused an undeclared buffer");
                        Owned::Buffer(self.buffers[at].clone())
                    }
                };
                (binding.binding(), resource)
            })
            .collect();
        let entity_textures = program
            .textures
            .iter()
            .filter(|(_, source)| *source == TextureSource::EntityTexture)
            .map(|(binding, _)| *binding)
            .collect();
        BindTemplate {
            label: format!("{GPU_LABEL} {}", program.name),
            layout: self.layouts[index].clone(),
            entries,
            entity_textures,
        }
    }

    fn plan(&self, pipelines: &Pipelines, blit: &Blit, start: Flips) -> Plan {
        let installed = self.installed;
        let mut flips = start;

        let compute = |index: usize, flips: Flips, size: [u32; 2]| -> Option<ComputePass> {
            let pipelines = pipelines.compute[index].as_ref()?;
            let variant = |parity: usize| ComputeVariant {
                pipeline: pipelines[parity].clone(),
                bind_group: self
                    .bind_template(index, index, flips, 0, false, parity)
                    .create(self.device, None),
                snapshots: self.usage.image_conflicts[index][parity],
            };
            Some(ComputePass {
                variants: if installed.programs[index].compute_parity.is_some() {
                    [variant(0), variant(1)]
                } else {
                    let only = variant(0);
                    [only.clone(), only]
                },
                groups: installed.programs[index].dispatch.groups(size),
            })
        };
        let screen_size = [self.targets.size.x, self.targets.size.y];

        let mipmapped = self.targets.mipmapped;
        let rebuilt = |asked: u16, flips: Flips| -> Vec<(u8, usize)> {
            (0..MAX_COLOR_TARGETS as u8)
                .filter(|n| asked & mipmapped & (1 << n) != 0)
                .map(|n| (n, flips.current(n)))
                .collect()
        };

        let screen_pass = |index: usize, flips: &mut Flips| -> Option<ScreenPass> {
            let program = &installed.programs[index];
            let pass = ScreenPass {
                pipeline: pipelines.screen[index].clone()?,
                bind_group: self.bind_group(index, *flips, 0, false),
                attachments: self.attachments(&program.draw_buffers, |n| flips.other(n)),
                mipmaps: rebuilt(program.mipmapped, *flips),
            };
            flips.flip(&program.draw_buffers);
            Some(pass)
        };

        let mut remaining = pass_limit().unwrap_or(usize::MAX);
        let mut stage_passes = |stage: ScreenStage, flips: &mut Flips| -> Vec<StepPass> {
            let mut out = Vec::new();
            for step in &installed.stages[stage.index()] {
                if remaining == 0 {
                    break;
                }
                match *step {
                    Step::Compute(index) => {
                        out.extend(compute(index, *flips, screen_size).map(StepPass::Compute))
                    }
                    Step::Screen(index) => {
                        remaining -= 1;
                        out.extend(screen_pass(index, flips).map(StepPass::Screen));
                    }
                }
            }
            out
        };

        let cleared = self.usage.cleared;
        let begin = stage_passes(ScreenStage::Begin, &mut flips);

        let shadow = match (installed.shadow, pipelines.shadow.clone(), self.shadow) {
            (Some(index), Some((pipeline, cutout)), Some(targets)) => {
                let program = &installed.programs[index];
                Some(ShadowPass {
                    pipeline,
                    cutout,
                    bind_group: self.bind_group(index, flips, 0, true),
                    attachments: shadow_attachments(targets, &program.draw_buffers),
                    copy_depth: self.usage.shadow_copy_opaque,
                    depth_source: self
                        .depth_copy
                        .filter(|_| targets.filtered.iter().any(Option::is_some))
                        .map(|copy| copy.source(self.device, &targets.depth.view)),
                })
            }
            _ => None,
        };
        let shadowcomp = if shadow.is_some() {
            installed
                .shadowcomp
                .iter()
                .filter_map(|&index| compute(index, flips, screen_size))
                .collect()
        } else {
            Vec::new()
        };
        let prepare = stage_passes(ScreenStage::Prepare, &mut flips);

        let geometry_pass = |slot: usize, flips: Flips| -> Option<GeometryPass> {
            let index = installed.geometry[slot]?;
            let program = &installed.programs[index];
            Some(GeometryPass {
                pipeline: pipelines.geometry[slot].clone()?,
                bind_group: self.bind_group(index, flips, mask(&program.draw_buffers), true),
                attachments: self.attachments(&program.draw_buffers, |n| flips.current(n)),
                stream: stream(Geometry::ALL[slot]),
                clears: 0,
            })
        };
        let mut opaque: Vec<GeometryPass> = Geometry::ALL
            .iter()
            .enumerate()
            .filter(|(_, g)| !g.translucent())
            .filter_map(|(slot, _)| geometry_pass(slot, flips))
            .collect();
        let sky = [EntityKind::SkyBasic, EntityKind::SkyTextured]
            .iter()
            .any(|kind| pipelines.entities[kind.index()].is_some());
        let drew_early = sky
            || begin
                .iter()
                .chain(&prepare)
                .any(|step| matches!(step, StepPass::Screen(_)));
        let merged = match opaque.first_mut() {
            Some(first) if !drew_early => {
                first.clears = mask(first.attachments.iter().map(|a| &a.target)) & cleared;
                first.clears
            }
            _ => 0,
        };
        let standalone: Vec<(u8, usize)> = (0..MAX_COLOR_TARGETS as u8)
            .filter(|n| cleared & (1 << n) != 0)
            .flat_map(|n| {
                let current = start.current(n);
                let merged_here = merged & (1 << n) != 0;
                [(n, current), (n, start.other(n))]
                    .into_iter()
                    .filter(move |(_, half)| !(merged_here && *half == current))
            })
            .collect();
        let clear_passes = |pairs: &[(u8, usize)]| -> Vec<Clear> {
            let size_of = |n: u8| self.targets.pair(n)[0].texture.size();
            let mut sizes: Vec<_> = pairs.iter().map(|(n, _)| size_of(*n)).collect();
            sizes.dedup();
            sizes.sort_unstable_by_key(|s| (s.width, s.height));
            sizes.dedup();
            sizes
                .into_iter()
                .flat_map(|size| {
                    let same: Vec<(u8, usize)> = pairs
                        .iter()
                        .copied()
                        .filter(|(n, _)| size_of(*n) == size)
                        .collect();
                    same.chunks(MAX_ATTACHMENTS)
                        .map(|group| Clear {
                            attachments: group
                                .iter()
                                .map(|(n, half)| Attachment {
                                    target: *n,
                                    view: self.targets.color(*n, *half).clone(),
                                })
                                .collect(),
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        let clears = clear_passes(&standalone);
        let kept: Vec<(u8, usize)> = (0..MAX_COLOR_TARGETS as u8)
            .filter(|n| installed.used_targets & (1 << n) != 0 && cleared & (1 << n) == 0)
            .flat_map(|n| [(n, 0), (n, 1)])
            .collect();
        let full_clears = clear_passes(&kept);

        let entity_pass = |at: usize, flips: Flips| -> Option<EntityPass> {
            let (index, (opaque, blend)) =
                (installed.entities[at]?, pipelines.entities[at].clone()?);
            let program = &installed.programs[index];
            let (attachments, drawing) = match EntityKind::ALL[at].segment() {
                Segment::Shadow => (
                    shadow_attachments(
                        self.shadow.filter(|_| shadow.is_some())?,
                        &program.draw_buffers,
                    ),
                    0,
                ),
                _ => (
                    self.attachments(&program.draw_buffers, |n| flips.current(n)),
                    mask(&program.draw_buffers),
                ),
            };
            Some(EntityPass {
                opaque,
                blend,
                attachments,
                template: self.bind_template(
                    index,
                    self.usage.kind_blocks[at]?,
                    flips,
                    drawing,
                    true,
                    0,
                ),
                groups: std::collections::HashMap::new(),
            })
        };
        let before_deferred = flips;

        let deferred = stage_passes(ScreenStage::Deferred, &mut flips);
        let entities = std::array::from_fn(|at| match EntityKind::ALL[at].segment() {
            Segment::Shadow | Segment::Sky | Segment::World | Segment::HandSolid => {
                entity_pass(at, before_deferred)
            }
            Segment::WorldTranslucent | Segment::AfterTranslucent | Segment::HandTranslucent => {
                entity_pass(at, flips)
            }
        });
        let translucent = Geometry::ALL
            .iter()
            .enumerate()
            .filter(|(_, g)| g.translucent())
            .filter_map(|(slot, _)| geometry_pass(slot, flips))
            .collect();
        let composite = stage_passes(ScreenStage::Composite, &mut flips);
        let final_allowed = remaining > 0;
        let final_computes = if final_allowed {
            installed
                .final_computes
                .iter()
                .filter_map(|&index| compute(index, flips, screen_size))
                .collect()
        } else {
            Vec::new()
        };
        let final_pass = installed
            .final_pass
            .filter(|_| final_allowed)
            .map(|index| FinalPass {
                bind_group: self.bind_group(index, flips, 0, false),
                mipmaps: rebuilt(installed.programs[index].mipmapped, flips),
            });

        let blit = final_pass.is_none().then(|| {
            self.device.create_bind_group(
                gpu_label!("blit"),
                &blit.layout,
                &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(self.targets.color(0, flips.current(0))),
                }],
            )
        });

        let chains = (0..MAX_COLOR_TARGETS as u8)
            .map(|n| match self.mipgen {
                Some(mipgen) if mipmapped & (1 << n) != 0 => [0, 1].map(|half| {
                    let texture = &self.targets.pair(n)[half].texture;
                    mipgen.chain(
                        self.device,
                        texture,
                        &format!("{GPU_LABEL} colortex{n}.{half} mipmap"),
                    )
                }),
                _ => [None, None],
            })
            .collect();

        Plan {
            setup: installed
                .setup
                .iter()
                .filter_map(|&index| compute(index, start, [1, 1]))
                .collect(),
            stages: [begin, prepare, deferred, composite],
            shadow,
            shadowcomp,
            entities,
            clears,
            full_clears,
            opaque,
            translucent,
            final_computes,
            final_pass,
            blit,
            end: flips,
            copy_depth: self.usage.copy_depth,
            chains,
        }
    }
}

fn mask<'a>(buffers: impl IntoIterator<Item = &'a u8>) -> u16 {
    buffers.into_iter().fold(0, |m, b| m | 1u16 << b)
}

fn shadow_attachments(targets: &ShadowTargets, draw_buffers: &[u8]) -> Vec<Attachment> {
    draw_buffers
        .iter()
        .filter_map(|b| {
            let view = targets.color[usize::from(*b)].as_ref()?.view.clone();
            Some(Attachment { target: *b, view })
        })
        .collect()
}
