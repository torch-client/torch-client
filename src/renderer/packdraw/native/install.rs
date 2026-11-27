use std::path::Path;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::render_resource::TextureUsages;
use bevy::tasks::Task;

use crate::renderer::dimension::Dimension;
use crate::renderer::packvertex::{BlockIds, PackLighting, VertexColors};
use crate::shaderpack::native::api::{Resource as Global, TerrainTier as SeamTier};
use crate::shaderpack::native::build::{self, Built};
use crate::shaderpack::native::options::Values;
use crate::shaderpack::native::{caps, check, problem};

const GENERATION_BIT: u32 = 1 << 31;

pub(crate) struct NativeInstalled {
    pub(crate) generation: u32,
    pub(crate) name: String,
    pub(crate) built: Built,
    pub(crate) uses_depth: bool,
    pub(crate) block_ids: Option<Arc<BlockIds>>,
}

impl NativeInstalled {
    pub(crate) fn geometry(&self) -> bool {
        !self.built.geometry.is_empty()
    }
}

#[derive(Resource)]
pub(crate) struct Renderable(pub(super) Arc<[caps::Format]>);

#[derive(Resource)]
pub(crate) struct NativePack {
    name: String,
    revision: u64,
    dimension: Dimension,
    pub(crate) installed: Arc<NativeInstalled>,
}

pub(crate) struct Pending {
    name: String,
    revision: u64,
    dimension: Dimension,
    task: Task<Result<(Built, Option<Arc<BlockIds>>), String>>,
}

pub(crate) fn sync_native_pack(
    mut commands: Commands,
    gui: Res<crate::gui::GuiState>,
    current: Option<Res<NativePack>>,
    renderable: Option<Res<Renderable>>,
    tier: Option<Res<crate::renderer::terrain_pool::TerrainTier>>,
    mut failed: Local<Option<(String, u64, Dimension)>>,
    mut generation: Local<u32>,
    mut pending: Local<Option<Pending>>,
) {
    let wanted = gui
        .shaderpacks
        .loaded
        .as_ref()
        .filter(|l| l.native)
        .map(|l| l.name.clone());
    let revision = gui.shaderpacks.revision();
    let dimension = crate::renderer::dimension::current();

    if let Some(running) = pending.as_mut() {
        if Some(&running.name) != wanted.as_ref()
            || running.revision != revision
            || running.dimension != dimension
        {
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
                Ok((built, block_ids)) => {
                    *generation = generation.wrapping_add(1).max(1) & !GENERATION_BIT;
                    let installed = NativeInstalled {
                        generation: *generation | GENERATION_BIT,
                        uses_depth: built.geometry.is_empty()
                            && built.passes.iter().any(|p| p.binds(Global::SceneDepth)),
                        name: name.clone(),
                        built,
                        block_ids,
                    };
                    report(&installed);
                    *failed = None;
                    crate::shaderpack::FAILED.set(Vec::new());
                    commands.insert_resource(NativePack {
                        name,
                        revision,
                        dimension,
                        installed: Arc::new(installed),
                    });
                }
                Err(e) => {
                    crate::log_warn!("shaders", "{name} did not build: {e}");
                    crate::shaderpack::FAILED.set(vec![name.clone()]);
                    if current.as_deref().is_none_or(|c| c.name != name) {
                        commands.remove_resource::<NativePack>();
                    }
                    *failed = Some((name, revision, dimension));
                }
            }
            return;
        }
    }

    let stale = match current.as_deref() {
        None => wanted.is_some(),
        Some(current) => {
            Some(&current.name) != wanted.as_ref()
                || current.revision != revision
                || current.dimension != dimension
        }
    };
    if !stale {
        return;
    }
    let Some(name) = wanted else {
        commands.remove_resource::<NativePack>();
        *failed = None;
        return;
    };
    if *failed == Some((name.clone(), revision, dimension)) {
        return;
    }
    let Some(renderable) = renderable.map(|r| Arc::clone(&r.0)) else {
        return;
    };
    let Some(tier) = tier else { return };
    let terrain = if tier.indirect {
        SeamTier::Indirect
    } else {
        SeamTier::Direct
    };
    let path = crate::shaderpack::discover::dir().join(&name);
    let values = crate::gui::shaderpacks::values_path(&name);
    let sunlit = dimension.has_sun();
    let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
        build_pack(
            &path,
            &values,
            dimension.registry_id(),
            sunlit,
            &renderable,
            terrain,
        )
    });
    *pending = Some(Pending {
        name,
        revision,
        dimension,
        task,
    });
}

fn report(installed: &NativeInstalled) {
    let built = &installed.built;
    let passes: Vec<String> = built
        .passes
        .iter()
        .map(|p| format!("{} {}", p.anchor.name(), p.id))
        .collect();
    let targets: Vec<&str> = built.targets.iter().map(|t| t.id.as_str()).collect();
    let groups: Vec<&str> = built.geometry.iter().map(|g| g.group.name()).collect();
    let shadow = built.shadow.as_ref().map_or_else(
        || "none".to_owned(),
        |s| format!("{} to {} blocks", s.resolution, s.distance),
    );
    crate::log_info!(
        "shaders",
        "built {} ({}): geometry [{}], passes [{}], targets [{}], shadow {}, generation {:#x}",
        installed.name,
        built.id,
        groups.join(", "),
        passes.join(", "),
        targets.join(", "),
        shadow,
        installed.generation
    );
}

fn build_pack(
    path: &Path,
    values_path: &Path,
    dimension: &str,
    sunlit: bool,
    renderable: &[caps::Format],
    terrain: SeamTier,
) -> Result<(Built, Option<Arc<BlockIds>>), String> {
    let (report, package) = check::check_and_open(path);
    if !report.problems.is_empty() {
        return Err(problem::summary(&report.problems));
    }
    let package = package.ok_or_else(|| "the pack could not be read".to_owned())?;
    let running = caps::running();
    if !report.tiers.contains(&running) {
        let tiers: Vec<&str> = report.tiers.iter().map(|t| t.name()).collect();
        return Err(format!(
            "it does not run on {} (it runs on {}): {}",
            running.name(),
            if tiers.is_empty() {
                "none".to_owned()
            } else {
                tiers.join(", ")
            },
            check::describe(&report.needs)
        ));
    }
    let manifest = &package.manifest;
    let values = match crate::platform::packfs::read_to_string(values_path) {
        Ok(text) => Values::load(&text, &manifest.options),
        Err(_) => {
            let values = manifest.default_values(running);
            if let Some(preset) = manifest
                .presets
                .iter()
                .find(|p| p.default_on.iter().any(|(t, _)| *t == running))
            {
                crate::log_info!(
                    "shaders",
                    "{}: no saved options, using preset `{}` for {}",
                    manifest.id,
                    preset.id,
                    running.name()
                );
            }
            values
        }
    };
    let choose = |list: &[caps::Format]| list.iter().position(|format| renderable.contains(format));
    let mut built = build::build(&package, &values, Some(dimension), &choose, terrain)
        .map_err(|p| problem::summary(&p))?;
    if !sunlit && built.shadow.take().is_some() {
        crate::log_info!(
            "shaders",
            "{}: {dimension} has no sun or moon, so the shadow pass is left out",
            built.id
        );
    }
    unbound(&built)?;
    let block_ids = (!built.geometry.is_empty()).then(|| Arc::new(block_ids(&built)));
    Ok((built, block_ids))
}

fn block_ids(built: &Built) -> BlockIds {
    let materials = &built.materials;
    if !materials.tags.is_empty() {
        let tags: Vec<String> = materials
            .tags
            .iter()
            .map(|(id, tag)| format!("#{tag} (material {id})"))
            .collect();
        crate::log_info!(
            "shaders",
            "{}: block tags are not resolved yet, left out: {}",
            built.id,
            tags.join(", ")
        );
    }
    BlockIds::build(&materials.entries, PackLighting::default()).with_colors(VertexColors::Vanilla)
}

fn unbound(built: &Built) -> Result<(), String> {
    let passes = built
        .passes
        .iter()
        .map(|pass| (format!("pass `{}`", pass.id), &pass.file, &pass.bindings));
    let groups = built.geometry.iter().map(|group| {
        (
            format!("[geometry.{}]", group.group.name()),
            &group.file,
            &group.bindings,
        )
    });
    let shadow = built
        .shadow
        .iter()
        .map(|shadow| ("[shadow]".to_owned(), &shadow.file, &shadow.bindings));
    for (what, file, bindings) in passes.chain(groups).chain(shadow) {
        for binding in bindings {
            if !bindable(binding.resource) {
                return Err(format!(
                    "{what} ({file}) uses `{}`, which this build does not bind yet",
                    binding.name
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn bindable(resource: Global) -> bool {
    match resource {
        Global::Frame
        | Global::LinearClamp
        | Global::LinearRepeat
        | Global::NearestClamp
        | Global::SceneColor
        | Global::SceneDepth
        | Global::BlockAtlas
        | Global::Lightmap
        | Global::Noise
        | Global::EntityTexture
        | Global::Target(_)
        | Global::TargetPrevious(_)
        | Global::ShadowCompare
        | Global::ShadowDepth
        | Global::ShadowColor => true,
        Global::Texture(_) | Global::TargetOut(_) => false,
    }
}

pub(crate) fn sync_depth_usage(
    native: Option<Res<NativePack>>,
    mut cameras: Query<&mut Camera3d, With<crate::renderer::systems::WorldCamera>>,
    mut added: Local<bool>,
) {
    let wanted = native.is_some_and(|n| n.installed.uses_depth);
    if wanted && !*added {
        for mut camera in &mut cameras {
            let usage = TextureUsages::from(camera.depth_texture_usages);
            if !usage.contains(TextureUsages::TEXTURE_BINDING) {
                camera.depth_texture_usages = (usage | TextureUsages::TEXTURE_BINDING).into();
                *added = true;
            }
        }
    } else if !wanted && *added {
        for mut camera in &mut cameras {
            let usage = TextureUsages::from(camera.depth_texture_usages);
            camera.depth_texture_usages = (usage - TextureUsages::TEXTURE_BINDING).into();
        }
        *added = false;
    }
}
