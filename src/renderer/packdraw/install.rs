use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::Task;

use super::GPU_LABEL;
use super::compile::{self, Frame, ProgramKind};
use super::sources::TextureSource;
use super::uniforms;
use crate::renderer::dimension::Dimension;
use crate::renderer::terrain_pool::TerrainTier;
use crate::shaderpack::directives::{Constants, Targets};
use crate::shaderpack::expressions::Customs;
use crate::shaderpack::pipeline::Layout;

impl InstalledProgram {
    pub(crate) fn fragment_for(&self, default: Option<compile::AlphaTest>) -> &Module {
        compile::alpha_for(self.alpha_override, default)
            .and_then(|test| self.fragment_tests.iter().find(|(t, _)| *t == test))
            .map_or(&self.fragment, |(_, module)| module)
    }

    pub(crate) fn texture_source(&self, binding: u32) -> TextureSource {
        self.textures
            .iter()
            .find(|(b, _)| *b == binding)
            .map(|(_, source)| *source)
            .expect("compilation resolved every texture binding")
    }
}

pub(crate) struct Module {
    pub(crate) label: String,
    pub(crate) wgsl: String,
}

pub(crate) struct InstalledProgram {
    pub(crate) name: String,
    pub(crate) kind: ProgramKind,
    pub(crate) vertex: Module,
    pub(crate) fragment: Module,
    pub(crate) fragment_tests: Vec<(compile::AlphaTest, Module)>,
    pub(crate) alpha_override: Option<Option<compile::AlphaTest>>,
    pub(crate) fragment_linear: Option<Module>,
    pub(crate) compute: Option<Module>,
    pub(crate) compute_used: Option<Vec<u32>>,
    pub(crate) compute_parity: Option<[(Module, Vec<u32>); 2]>,
    pub(crate) dispatch: compile::Dispatch,
    pub(crate) layout: Layout,
    pub(crate) draw_buffers: Vec<u8>,
    pub(crate) textures: Vec<(u32, TextureSource)>,
    pub(crate) mipmapped: u16,
}

pub(crate) struct Installed {
    pub(crate) generation: u32,
    pub(crate) direct: bool,
    pub(crate) programs: Vec<InstalledProgram>,
    pub(crate) geometry: [Option<usize>; 3],
    pub(crate) stages: [Vec<super::compile::Step>; 4],
    pub(crate) final_computes: Vec<usize>,
    pub(crate) final_pass: Option<usize>,
    pub(crate) setup: Vec<usize>,
    pub(crate) shadow: Option<usize>,
    pub(crate) shadowcomp: Vec<usize>,
    pub(crate) entities: [Option<usize>; super::compile::EntityKind::COUNT],
    pub(crate) sky_parts: super::compile::SkyParts,
    pub(crate) shadow_casters: super::compile::ShadowCasters,
    pub(crate) shadow_culling: super::compile::ShadowCulling,
    pub(crate) targets: Targets,
    pub(crate) constants: Constants,
    pub(crate) customs: Customs,
    pub(crate) used_targets: u16,
    pub(crate) used_shadow: u8,
    pub(crate) images: Vec<super::images::CustomTexture>,
    pub(crate) block_ids: Option<Arc<crate::renderer::packvertex::BlockIds>>,
    pub(crate) custom_images: Vec<crate::shaderpack::customimages::CustomImage>,
    pub(crate) buffers: Vec<crate::shaderpack::customimages::BufferObject>,
    pub(crate) entity_ids: std::collections::HashMap<String, i32>,
    pub(crate) item_ids: std::collections::HashMap<String, i32>,
    pub(crate) old_hand_light: bool,
}

#[derive(Resource)]
pub(crate) struct CompiledPack {
    name: String,
    revision: u64,
    dimension: Dimension,
    pub(crate) installed: Arc<Installed>,
}

pub(crate) struct Pending {
    name: String,
    revision: u64,
    dimension: Dimension,
    task: Task<Result<Frame, String>>,
}

fn install(pack: &str, generation: u32, direct: bool, frame: Frame) -> Installed {
    let add = |program: &str, stage: &str, wgsl: String| Module {
        label: format!("{GPU_LABEL}/{pack}/{generation}/{program}.{stage}.wgsl"),
        wgsl,
    };
    let programs = frame
        .programs
        .into_iter()
        .map(|p| InstalledProgram {
            vertex: add(&p.name, "vsh", p.vertex),
            fragment: add(&p.name, "fsh", p.fragment),
            compute: p.compute.map(|c| add(&p.name, "csh", c)),
            compute_used: p.compute_used,
            compute_parity: p
                .compute_parity
                .map(|[(even, even_used), (odd, odd_used)]| {
                    [
                        (add(&p.name, "even.csh", even), even_used),
                        (add(&p.name, "odd.csh", odd), odd_used),
                    ]
                }),
            dispatch: p.dispatch,
            fragment_tests: p
                .fragment_tests
                .into_iter()
                .enumerate()
                .map(|(n, (test, f))| (test, add(&p.name, &format!("alpha{n}.fsh"), f)))
                .collect(),
            alpha_override: p.alpha_override,
            fragment_linear: p.fragment_linear.map(|f| add(&p.name, "linear.fsh", f)),
            name: p.name,
            kind: p.kind,
            layout: p.layout,
            draw_buffers: p.draw_buffers,
            textures: p.textures,
            mipmapped: p.mipmapped,
        })
        .collect();
    Installed {
        generation,
        direct,
        programs,
        geometry: frame.geometry,
        stages: frame.stages,
        final_computes: frame.final_computes,
        final_pass: frame.final_pass,
        setup: frame.setup,
        shadow: frame.shadow,
        shadowcomp: frame.shadowcomp,
        entities: frame.entities,
        sky_parts: frame.sky_parts,
        shadow_casters: frame.shadow_casters,
        shadow_culling: frame.shadow_culling,
        targets: frame.targets,
        constants: frame.constants,
        customs: frame.customs,
        used_targets: frame.used_targets,
        used_shadow: frame.used_shadow,
        images: frame.images,
        block_ids: frame.block_ids,
        item_ids: frame.item_ids,
        old_hand_light: frame.old_hand_light,
        entity_ids: frame.entity_ids,
        custom_images: frame.custom_images,
        buffers: frame.buffers,
    }
}

fn report(name: &str, installed: &Installed, notes: &[String]) {
    crate::log_info!(
        "shaders",
        "compiled {name}: {} programs (begin {}, prepare {}, deferred {}, composite {} steps, final {}, setup {}), generation {}",
        installed.programs.len(),
        installed.stages[0].len(),
        installed.stages[1].len(),
        installed.stages[2].len(),
        installed.stages[3].len(),
        installed.final_pass.is_some(),
        installed.setup.len(),
        installed.generation
    );
    for program in &installed.programs {
        let zero = uniforms::unsupplied(&program.layout.uniforms, &installed.customs);
        if !zero.is_empty() {
            crate::log_info!(
                "shaders",
                "{name}/{}: uniforms reading zero: {}",
                program.name,
                zero.join(", ")
            );
        }
    }
    for note in notes {
        crate::log_info!("shaders", "{name}: {note}");
    }
}

pub(crate) fn sync_compiled_pack(
    mut commands: Commands,
    gui: Res<crate::gui::GuiState>,
    tier: Option<Res<TerrainTier>>,
    compiled: Option<Res<CompiledPack>>,
    mut failed: Local<Option<(String, u64, Dimension)>>,
    mut generation: Local<u32>,
    mut pending: Local<Option<Pending>>,
) {
    let Some(tier) = tier else { return };
    let loaded = gui.shaderpacks.loaded.as_ref();
    let wanted = loaded.map(|l| l.name.clone());
    let revision = gui.shaderpacks.revision();
    let editing = gui.screen == crate::gui::Screen::ShaderOptions;
    let direct = !tier.indirect;
    let dimension = crate::renderer::dimension::current();

    if let Some(running) = pending.as_mut() {
        let current = Some(&running.name) == wanted.as_ref()
            && (editing || running.revision == revision)
            && running.dimension == dimension;
        if !current {
            *pending = None;
        } else {
            let Some(result) = bevy::tasks::futures::check_ready(&mut running.task) else {
                return;
            };
            let Pending {
                name,
                revision,
                dimension,
                ..
            } = pending.take().expect("matched above");
            match result {
                Ok(mut frame) => {
                    let notes = std::mem::take(&mut frame.notes);
                    let installed = install(&name, *generation, direct, frame);
                    report(&name, &installed, &notes);
                    *failed = None;
                    commands.insert_resource(CompiledPack {
                        name,
                        revision,
                        dimension,
                        installed: Arc::new(installed),
                    });
                }
                Err(e) => {
                    crate::log_warn!("shaders", "{name} did not compile: {e}");
                    *failed = Some((name, revision, dimension));
                    commands.remove_resource::<CompiledPack>();
                }
            }
            return;
        }
    }

    let stale = match compiled.as_deref() {
        None => wanted.is_some(),
        Some(current) => {
            Some(&current.name) != wanted.as_ref()
                || (!editing && current.revision != revision)
                || current.dimension != dimension
        }
    };
    if !stale {
        return;
    }
    let (Some(name), Some(loaded)) = (wanted, loaded) else {
        commands.remove_resource::<CompiledPack>();
        *failed = None;
        return;
    };
    if *failed == Some((name.clone(), revision, dimension)) {
        return;
    }

    *generation = generation.wrapping_add(1).max(1);
    let values = loaded.values().clone();
    let task_name = name.clone();
    let task = bevy::tasks::AsyncComputeTaskPool::get()
        .spawn(async move { compile::compile_frame(&task_name, direct, Some(values), dimension) });
    *pending = Some(Pending {
        name,
        revision,
        dimension,
        task,
    });
}

static REJECTED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn reject(generation: u32) {
    REJECTED.store(generation, std::sync::atomic::Ordering::Release);
}

pub(crate) fn sync_pack_vertices(
    compiled: Option<Res<CompiledPack>>,
    shared: Res<crate::renderer::systems::Shared>,
    mut published: Local<Option<Arc<crate::renderer::packvertex::BlockIds>>>,
) {
    let rejected = REJECTED.load(std::sync::atomic::Ordering::Acquire);
    let live = compiled
        .as_deref()
        .filter(|c| c.installed.generation != rejected);
    let player_shadow = live.is_some_and(|c| {
        let casters = c.installed.shadow_casters;
        c.installed.entities[super::compile::EntityKind::ShadowCaster.index()].is_some()
            && (casters.player || casters.entities)
    });
    crate::renderer::packvertex::set_player_shadow(player_shadow);
    let wanted = live.and_then(|c| c.installed.block_ids.clone());
    let remesh = match (published.as_ref(), wanted.as_ref()) {
        (None, None) => return,
        (Some(old), Some(new)) if Arc::ptr_eq(old, new) => return,
        (Some(old), Some(new)) => old != new,
        _ => true,
    };
    crate::renderer::packvertex::set_current(wanted.clone());
    *published = wanted;
    if remesh {
        let Ok(mut state) = shared.0.lock();
        state.reload_chunks_requested = true;
    }
}
