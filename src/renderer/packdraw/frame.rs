use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases};
use bevy::render::render_resource::{
    BindGroupLayout, Buffer, RenderPipeline, TextureFormat, TextureView, TextureViewId,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::{ExtractedView, ViewTarget};

use super::GPU_LABEL;
use super::compile::EntityKind;
use super::entities::{EntityBatch, EntityDraw, EntityInstance};
use super::fullscreen::Blit;
use super::inputs::Inputs;
use super::install::{CompiledPack, Installed};
use super::packatlas::PackAtlas;
use super::pipelines::{Building, Pipelines, make_layout, program_layout};
use super::plan::{Plan, PlanKey};
use super::targets::{
    ShadowTargets, Statics, Target, Targets, custom_target, image_stub, image_target, noise_target,
    storage_buffer,
};
use super::uniforms::{BlockSpec, FrameValues, History, MatrixSet, PackUniforms};
pub(super) use super::usage::Usage;
use crate::renderer::terrain_pool::TerrainView;
use crate::renderer::terrain_pool::cull::{PackCullView, ShadowCullInput, shadow_cull_shape};
use crate::renderer::terrain_pool::draw::{PackTakeover, TerrainBindGroup};
use crate::shaderpack::expressions::CustomState;

#[derive(Resource)]
pub(crate) struct PackRender {
    pub(crate) failed: u32,
    pub(super) process: Option<ProcessGpu>,
    pub(super) install: Option<Box<Install>>,
}

pub(super) struct ProcessGpu {
    pub(super) statics: Statics,
    pub(super) blit: Blit,
    pub(super) mipgen: Option<super::mipmaps::MipGen>,
    pub(super) depth_copy: Option<super::depthcopy::DepthCopy>,
    pub(super) pack_atlas: Option<PackAtlas>,
}

impl ProcessGpu {
    fn new(device: &RenderDevice, queue: &RenderQueue) -> ProcessGpu {
        ProcessGpu {
            statics: Statics::new(device, queue),
            blit: Blit::new(device),
            pack_atlas: None,
            mipgen: None,
            depth_copy: None,
        }
    }
}

pub(super) struct Install {
    pub(super) installed: Arc<Installed>,
    pub(super) usage: Usage,
    pub(super) gpu: InstallGpu,
    pub(super) stage: Stage,
    pub(super) finals: Vec<(TextureFormat, Building<RenderPipeline>)>,
    history: History,
    customs: CustomState,
    pub(super) frame: FrameState,
    pub(super) entity_textures:
        std::collections::HashMap<AssetId<Image>, (TextureViewId, TextureView)>,
}

pub(super) struct InstallGpu {
    pub(super) layouts: Vec<BindGroupLayout>,
    pub(super) image_stubs: Vec<Target>,
    pub(super) zeros: Option<Buffer>,
    pub(super) uniforms: PackUniforms,
    pub(super) noise: Target,
    pub(super) images: Vec<Target>,
    pub(super) custom_images: Vec<Target>,
    pub(super) buffers: Vec<Buffer>,
    pub(super) image_snapshots: Vec<Option<Target>>,
    pub(super) shadow_targets: Option<ShadowTargets>,
}

pub(super) enum Stage {
    Queued,
    Started {
        pipelines: Building<Pipelines>,
        sized: Option<Sized>,
    },
}

impl Stage {
    pub(super) fn built(&self) -> Option<&Pipelines> {
        match self {
            Stage::Started { pipelines, .. } => pipelines.ready(),
            Stage::Queued => None,
        }
    }

    pub(super) fn sized(&self) -> Option<&Sized> {
        match self {
            Stage::Started { sized, .. } => sized.as_ref(),
            Stage::Queued => None,
        }
    }
}

pub(super) struct Sized {
    pub(super) key: PlanKey,
    pub(super) targets: Targets,
    pub(super) plans: Vec<Plan>,
    pub(super) setup_done: std::sync::atomic::AtomicBool,
    pub(super) fully_cleared: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
pub(super) struct FrameState {
    far: f32,
    pub(super) inputs: Inputs,
    pub(super) camera: Vec3,
    pub(super) entity_draws: Vec<EntityDraw>,
    pub(super) entity_roots: std::collections::HashMap<Entity, i32>,
    pub(super) entity_instances: Vec<EntityInstance>,
    pub(super) entity_batches: Vec<EntityBatch>,
    pub(super) entity_buffer: Option<Buffer>,
    pub(super) entity_frame: u32,
    pub(super) entity_texture_use: std::collections::HashMap<Option<AssetId<Image>>, u32>,
}

impl PackRender {
    pub(crate) fn new() -> PackRender {
        PackRender {
            failed: 0,
            process: None,
            install: None,
        }
    }

    pub(crate) fn drawable(&self, format: TextureFormat) -> Option<&RenderPipeline> {
        let (install, process) = (self.install.as_deref()?, self.process.as_ref()?);
        let plan = install.stage.sized()?.plans.first()?;
        match plan.final_pass {
            Some(_) => install
                .finals
                .iter()
                .find(|(f, _)| *f == format)
                .and_then(|(_, b)| b.ready()),
            None => process.blit.pipeline(format),
        }
    }

    pub(crate) fn draws_hand(&self) -> bool {
        self.install.as_ref().is_some_and(|i| {
            EntityKind::ALL
                .iter()
                .any(|k| k.is_hand() && i.installed.entities[k.index()].is_some())
        })
    }

    pub(crate) fn generation(&self) -> u32 {
        self.install.as_ref().map_or(0, |i| i.installed.generation)
    }

    pub(crate) fn reject_install(&mut self) {
        let generation = self.generation();
        self.failed = generation;
        super::install::reject(generation);
        self.install = None;
    }
}

pub(crate) fn extract_pack(
    main: Extract<Option<Res<CompiledPack>>>,
    environment: Extract<Option<Res<crate::renderer::environment::Environment>>>,
    inputs: Extract<Option<Res<Inputs>>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    render: Option<ResMut<PackRender>>,
) {
    let Some(mut render) = render else { return };
    let render = &mut *render;
    let failed = render.failed;
    let wanted = main
        .as_deref()
        .map(|c| &c.installed)
        .filter(|i| i.generation != failed);
    if render.generation() != wanted.map_or(0, |i| i.generation) {
        render.install = wanted.map(|installed| {
            render
                .process
                .get_or_insert_with(|| ProcessGpu::new(&device, &queue));
            Box::new(install(&device, &queue, Arc::clone(installed)))
        });
        super::set_current_generation(render.generation());
    }
    let Some(install) = render.install.as_deref_mut() else {
        return;
    };
    if let Some(environment) = environment.as_deref() {
        install.frame.far = (environment.render_distance * 16) as f32;
    }
    if let Some(inputs) = inputs.as_deref() {
        install.frame.inputs = *inputs;
    }
}

fn install(device: &RenderDevice, queue: &RenderQueue, installed: Arc<Installed>) -> Install {
    let usage = Usage::of(device, &installed);
    let layouts = (0..installed.programs.len())
        .map(|index| {
            make_layout(
                device,
                &program_layout(&installed, index, usage.shadow_filtered),
            )
        })
        .collect();
    let uniforms = PackUniforms::new(
        device,
        &format!("{GPU_LABEL} uniforms"),
        installed
            .programs
            .iter()
            .map(|program| BlockSpec {
                layout: &program.layout.uniforms,
                set: program.kind.matrices(),
                render_stage: program.kind.render_stage(),
            })
            .chain(
                EntityKind::ALL
                    .into_iter()
                    .filter(|kind| kind.is_hand())
                    .filter_map(|kind| {
                        let program = &installed.programs[installed.entities[kind.index()]?];
                        Some(BlockSpec {
                            layout: &program.layout.uniforms,
                            set: MatrixSet::Hand,
                            render_stage: kind.render_stage(),
                        })
                    }),
            ),
        &installed.customs,
    );
    let gpu = InstallGpu {
        image_stubs: installed
            .custom_images
            .iter()
            .map(|image| image_stub(device, image))
            .collect(),
        layouts,
        zeros: None,
        uniforms,
        noise: noise_target(device, queue, installed.constants.noise_resolution),
        images: installed
            .images
            .iter()
            .map(|image| custom_target(device, queue, image))
            .collect(),
        custom_images: installed
            .custom_images
            .iter()
            .map(|image| image_target(device, image, UVec2::ONE))
            .collect(),
        buffers: installed
            .buffers
            .iter()
            .map(|b| storage_buffer(device, b, [1, 1]))
            .collect(),
        image_snapshots: installed
            .custom_images
            .iter()
            .enumerate()
            .map(|(index, image)| {
                (usage.snapshots & (1 << index) != 0)
                    .then(|| image_target(device, image, UVec2::ONE))
            })
            .collect(),
        shadow_targets: usage
            .shadow
            .then(|| ShadowTargets::new(device, &installed, usage.shadow_filtered)),
    };
    Install {
        customs: installed.customs.state(),
        history: History::default(),
        usage,
        gpu,
        stage: Stage::Queued,
        finals: Vec::new(),
        frame: FrameState::default(),
        entity_textures: std::collections::HashMap::new(),
        installed,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn take_over_view(
    terrain_group: Res<TerrainBindGroup>,
    takeover: Option<ResMut<PackTakeover>>,
    render: Option<Res<PackRender>>,
    views: Query<(&ExtractedView, &ViewTarget), With<TerrainView>>,
    hand_views: Query<&ExtractedView, With<crate::renderer::hand::HandCamera>>,
    mut opaque: ResMut<ViewBinnedRenderPhases<bevy::core_pipeline::core_3d::Opaque3d>>,
    mut alpha_mask: ResMut<ViewBinnedRenderPhases<bevy::core_pipeline::core_3d::AlphaMask3d>>,
    mut transmissive: ResMut<ViewSortedRenderPhases<bevy::core_pipeline::core_3d::Transmissive3d>>,
    mut transparent: ResMut<ViewSortedRenderPhases<bevy::core_pipeline::core_3d::Transparent3d>>,
    mut prepass: (
        Option<ResMut<ViewBinnedRenderPhases<bevy::core_pipeline::prepass::Opaque3dPrepass>>>,
        Option<ResMut<ViewBinnedRenderPhases<bevy::core_pipeline::prepass::AlphaMask3dPrepass>>>,
    ),
) {
    let Some(mut takeover) = takeover else { return };
    let mut taken_over = false;
    if let Some(render) = render
        && terrain_group.group.is_some()
    {
        let mut hide = |key: &bevy::render::view::RetainedViewEntity| {
            opaque.remove(key);
            alpha_mask.remove(key);
            transmissive.remove(key);
            transparent.remove(key);
            if let Some(phases) = prepass.0.as_mut() {
                phases.remove(key);
            }
            if let Some(phases) = prepass.1.as_mut() {
                phases.remove(key);
            }
        };
        for (view, target) in &views {
            if render.drawable(target.main_texture_format()).is_some() {
                taken_over = true;
                hide(&view.retained_view_entity);
            }
        }
        if taken_over && render.draws_hand() {
            for view in &hand_views {
                hide(&view.retained_view_entity);
            }
        }
    }
    if takeover.0 != taken_over {
        takeover.0 = taken_over;
    }
}

#[cfg(feature = "builtin_shaders")]
pub(crate) fn hide_world_shadows(
    takeover: Option<Res<PackTakeover>>,
    cameras: Query<&bevy::pbr::ViewLightEntities, With<TerrainView>>,
    lights: Query<&ExtractedView, With<bevy::pbr::LightEntity>>,
    mut shadows: ResMut<ViewBinnedRenderPhases<bevy::pbr::Shadow>>,
) {
    if !takeover.is_some_and(|t| t.0) {
        return;
    }
    for view_lights in &cameras {
        for light in view_lights
            .lights
            .iter()
            .filter_map(|entity| lights.get(*entity).ok())
        {
            shadows.remove(&light.retained_view_entity);
        }
    }
}

pub(crate) fn prepare_uniforms(
    queue: Res<RenderQueue>,
    views: Query<&ExtractedView, With<TerrainView>>,
    time: Res<Time>,
    frames: Res<bevy::diagnostic::FrameCount>,
    render: Option<ResMut<PackRender>>,
    takeover: Option<Res<PackTakeover>>,
    mut pack_view: ResMut<PackCullView>,
) {
    pack_view.0 = None;
    let Some(mut render) = render else { return };
    let Some(install) = render
        .install
        .as_deref_mut()
        .filter(|i| i.stage.built().is_some())
    else {
        return;
    };
    let Some(view) = views.iter().next() else {
        return;
    };
    let installed = &install.installed;
    let values = FrameValues::of(
        view,
        time.elapsed_secs_wrapped(),
        time.delta_secs(),
        frames.0,
        install.frame.far,
        install.frame.inputs,
        &installed.constants,
        &mut install.history,
    );
    if install.usage.shadow && takeover.is_some_and(|t| t.0) {
        let constants = &installed.constants;
        pack_view.0 = Some(shadow_cull_shape(&ShadowCullInput {
            mode: installed
                .shadow_culling
                .mode(install.usage.shadow_voxelizes),
            camera_clip: values.camera_clip(),
            toward_light: values.toward_light(),
            camera: values.camera().as_dvec3(),
            shadow_distance: constants.shadow_distance,
            voxel_distance: constants.voxel_distance,
            render_mul: constants.shadow_distance_render_mul,
            render_distance: install.frame.far,
        }));
    }
    install.frame.camera = values.camera();
    installed
        .customs
        .evaluate(&mut install.customs, time.delta_secs(), &|id| {
            values.input(id)
        });
    install
        .gpu
        .uniforms
        .write(&values, &install.customs.values, &queue);
}
