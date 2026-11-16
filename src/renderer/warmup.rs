use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

use bevy::asset::{AssetId, UntypedAssetId};
use bevy::camera::visibility::RenderLayers;
use bevy::material::key::{ErasedMaterialPipelineKey, ErasedMeshPipelineKey};
use bevy::pbr::{
    Material, MeshMaterial3d, MeshPipelineKey, PreparedMaterial, ViewKeyCache,
    alpha_mode_pipeline_key,
};
use bevy::prelude::*;
use bevy::render::erased_render_asset::ErasedRenderAssets;
use bevy::render::mesh::RenderMesh;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    CachedRenderPipelineId, PipelineCache, SpecializedRenderPipelines,
};
use bevy::render::view::{ExtractedView, ExtractedWindows, Msaa};
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};

use crate::gui::draw::{GuiPipeline, GuiPipelineKey, GuiPipelineKind};
use crate::platform::time::Instant;
use crate::renderer::terrain_pool::draw::{KeyInputs, TerrainPipeline};
use crate::renderer::terrain_pool::{
    STREAM_CUTOUT, STREAM_SOLID, STREAM_WATER, TerrainTier, TerrainView,
};

const PIPELINES_PER_FRAME: usize = 1;

const START_AFTER_FRAMES: u32 = 3;

const GIVE_UP_AFTER_FRAMES: u32 = 60;

const HOLD_AT_MOST: Duration = Duration::from_secs(30);

const WORLD_LAYERS: [usize; 2] = [0, crate::renderer::hand::HAND_LAYER];

const WORLD_CAMERA_ORDER: isize = 0;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static KEY_KNOWN: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicUsize = AtomicUsize::new(0);
static MENU_FRAMES: AtomicU32 = AtomicU32::new(0);
static PANORAMA_UP: AtomicBool = AtomicBool::new(false);

pub(crate) fn steer_world_camera(
    in_menu: bool,
    panorama_exists: bool,
    camera: &mut Mut<Camera>,
    layers: &mut Mut<RenderLayers>,
) -> bool {
    let up = in_menu && panorama_exists;
    PANORAMA_UP.store(up, Ordering::Relaxed);
    let frames = if up {
        MENU_FRAMES.load(Ordering::Relaxed).saturating_add(1)
    } else {
        0
    };
    MENU_FRAMES.store(frames, Ordering::Relaxed);
    let active = frames > START_AFTER_FRAMES;
    ACTIVE.store(active, Ordering::Relaxed);

    let borrow = active && !KEY_KNOWN.load(Ordering::Relaxed);
    let order = if borrow {
        crate::renderer::panorama::PANORAMA_CAMERA_ORDER - 1
    } else {
        WORLD_CAMERA_ORDER
    };
    if camera.order != order {
        camera.order = order;
    }
    if !borrow {
        return false;
    }
    if !camera.is_active {
        camera.is_active = true;
    }
    if **layers != RenderLayers::none() {
        **layers = RenderLayers::none();
    }
    true
}

pub(crate) fn holds_level_screen(opened_at: Option<Instant>) -> bool {
    PENDING.load(Ordering::Relaxed) > 0
        && (KEY_KNOWN.load(Ordering::Relaxed) || PANORAMA_UP.load(Ordering::Relaxed))
        && opened_at.is_some_and(|at| at.elapsed() < HOLD_AT_MOST)
}

pub struct WarmupPlugin;

impl Plugin for WarmupPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<WarmupState>()
            .add_systems(
                ExtractSchedule,
                (
                    collect::<crate::renderer::entity_material::EntityMaterial>,
                    collect::<StandardMaterial>,
                    collect::<crate::renderer::sky::BackdropMaterial>,
                    collect::<crate::renderer::hand::HandMaterial>,
                    collect::<crate::renderer::weather::WeatherMaterial>,
                    collect::<crate::renderer::world_border::WorldBorderMaterial>,
                    collect::<crate::renderer::esp_material::EspMaterial>,
                    collect::<crate::blockentities::render::end_portal::EndPortalMaterial>,
                ),
            )
            .add_systems(
                Render,
                (
                    read_view_key.in_set(RenderSystems::Specialize),
                    warm.in_set(RenderSystems::Queue),
                    measure.in_set(RenderSystems::Cleanup),
                ),
            );
        #[cfg(feature = "builtin_shaders")]
        render_app.add_systems(
            ExtractSchedule,
            collect::<crate::renderer::sky::SkyMaterial>,
        );
        #[cfg(all(not(feature = "webgpu"), not(feature = "shader_support")))]
        render_app.add_systems(
            ExtractSchedule,
            collect::<crate::renderer::clouds::CloudMaterial>,
        );
    }
}

enum Item {
    Terrain(u32),
    Vignette,
    Material(AssetId<Mesh>, UntypedAssetId),
}

struct Entry {
    item: Item,
    waited: u32,
}

#[derive(Default)]
struct Round {
    started: Option<Instant>,
    queued: u32,
    mark: Option<Instant>,
    slowest: Duration,
}

#[derive(Resource)]
struct WarmupState {
    view_key: Option<MeshPipelineKey>,
    seen: HashSet<(AssetId<Mesh>, UntypedAssetId)>,
    entries: VecDeque<Entry>,
    round: Round,
}

impl Default for WarmupState {
    fn default() -> Self {
        let mut state = WarmupState {
            view_key: None,
            seen: HashSet::new(),
            entries: VecDeque::new(),
            round: Round::default(),
        };
        state.seed();
        state
    }
}

impl WarmupState {
    fn seed(&mut self) {
        let fixed = [
            Item::Terrain(STREAM_SOLID),
            Item::Terrain(STREAM_CUTOUT),
            Item::Terrain(STREAM_WATER),
            Item::Vignette,
        ];
        self.entries
            .extend(fixed.into_iter().map(|item| Entry { item, waited: 0 }));
    }
}

fn collect<M: Material>(
    query: Extract<Query<(&Mesh3d, &MeshMaterial3d<M>, Option<&RenderLayers>)>>,
    mut state: ResMut<WarmupState>,
) {
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    let world_layers = RenderLayers::from_layers(&WORLD_LAYERS);
    for (mesh, material, layers) in query.iter() {
        if layers.is_some_and(|layers| !layers.intersects(&world_layers)) {
            continue;
        }
        let pair = (mesh.0.id(), material.0.id().untyped());
        if state.seen.contains(&pair) {
            continue;
        }
        state.seen.insert(pair);
        state.entries.push_back(Entry {
            item: Item::Material(pair.0, pair.1),
            waited: 0,
        });
    }
}

fn read_view_key(
    views: Query<&ExtractedView, With<TerrainView>>,
    keys: Res<ViewKeyCache>,
    mut state: ResMut<WarmupState>,
) {
    let Some(key) = views
        .iter()
        .find_map(|view| keys.get(&view.retained_view_entity).copied())
    else {
        return;
    };
    if state.view_key == Some(key) {
        return;
    }
    if state.view_key.is_some() {
        state.seen.clear();
        state.entries.clear();
        state.seed();
    }
    state.view_key = Some(key);
    KEY_KNOWN.store(true, Ordering::Relaxed);
}

enum Outcome {
    Specialized(CachedRenderPipelineId),
    Skipped,
    NotReady,
}

fn warm(world: &mut World) {
    if !ACTIVE.load(Ordering::Relaxed) {
        PENDING.store(0, Ordering::Relaxed);
        return;
    }
    let Some(view_key) = world.resource::<WarmupState>().view_key else {
        PENDING.store(
            world.resource::<WarmupState>().entries.len(),
            Ordering::Relaxed,
        );
        return;
    };
    let created = world.resource::<PipelineCache>().pipelines().count();
    let mut unexamined = world.resource::<WarmupState>().entries.len();
    let mut queued = 0;
    while queued < PIPELINES_PER_FRAME && unexamined > 0 {
        unexamined -= 1;
        let Some(mut entry) = world.resource_mut::<WarmupState>().entries.pop_front() else {
            break;
        };
        let outcome = match &entry.item {
            Item::Terrain(stream) => terrain(world, view_key, *stream),
            Item::Vignette => vignette(world),
            Item::Material(mesh, material) => material_pipeline(world, view_key, *mesh, *material),
        };
        match outcome {
            Outcome::Specialized(id) => {
                if id.id() >= created {
                    queued += 1;
                }
            }
            Outcome::Skipped => {}
            Outcome::NotReady => {
                entry.waited += 1;
                if entry.waited < GIVE_UP_AFTER_FRAMES {
                    world.resource_mut::<WarmupState>().entries.push_back(entry);
                }
            }
        }
    }
    PENDING.store(unexamined, Ordering::Relaxed);
    if queued > 0 {
        let now = Instant::now();
        let mut state = world.resource_mut::<WarmupState>();
        state.round.started.get_or_insert(now);
        state.round.queued += queued as u32;
        state.round.mark = Some(now);
    }
}

fn terrain(world: &mut World, view_key: MeshPipelineKey, stream: u32) -> Outcome {
    let Some(tier) = world.get_resource::<TerrainTier>() else {
        return Outcome::NotReady;
    };
    let key = KeyInputs::current(tier).main(view_key, stream);
    if world.get_resource::<TerrainPipeline>().is_none() {
        return Outcome::NotReady;
    }
    world.resource_scope(
        |world, mut pipelines: Mut<SpecializedRenderPipelines<TerrainPipeline>>| {
            let id = pipelines.specialize(
                world.resource::<PipelineCache>(),
                world.resource::<TerrainPipeline>(),
                key,
            );
            Outcome::Specialized(id)
        },
    )
}

fn vignette(world: &mut World) -> Outcome {
    let windows = world.resource::<ExtractedWindows>();
    let Some(format) = windows
        .primary
        .and_then(|window| windows.get(&window))
        .and_then(|window| window.swap_chain_texture_view_format)
    else {
        return Outcome::NotReady;
    };
    if world.get_resource::<GuiPipeline>().is_none() {
        return Outcome::NotReady;
    }
    world.resource_scope(
        |world, mut pipelines: Mut<SpecializedRenderPipelines<GuiPipeline>>| {
            let id = pipelines.specialize(
                world.resource::<PipelineCache>(),
                world.resource::<GuiPipeline>(),
                GuiPipelineKey {
                    kind: GuiPipelineKind::Vignette,
                    format,
                },
            );
            Outcome::Specialized(id)
        },
    )
}

fn material_pipeline(
    world: &mut World,
    view_key: MeshPipelineKey,
    mesh: AssetId<Mesh>,
    material: UntypedAssetId,
) -> Outcome {
    let Some((mesh_bits, layout)) = world
        .resource::<RenderAssets<RenderMesh>>()
        .get(mesh)
        .map(|mesh| (mesh.key_bits.bits(), mesh.layout.clone()))
    else {
        return Outcome::NotReady;
    };
    let Some(properties) = world
        .resource::<ErasedRenderAssets<PreparedMaterial>>()
        .get(material)
        .map(|material| material.properties.clone())
    else {
        return Outcome::NotReady;
    };
    let Some(base_specialize) = properties.base_specialize else {
        return Outcome::Skipped;
    };

    let mut material_bits: MeshPipelineKey = properties.mesh_pipeline_key_bits.downcast();
    material_bits.insert(alpha_mode_pipeline_key(
        properties.alpha_mode,
        &Msaa::from_samples(view_key.msaa_samples()),
    ));
    let mesh_key = view_key | MeshPipelineKey::from_bits_retain(mesh_bits) | material_bits;
    let key = ErasedMaterialPipelineKey {
        type_id: material.type_id(),
        mesh_key: ErasedMeshPipelineKey::new(mesh_key),
        material_key: properties.material_key.clone(),
    };
    match base_specialize(world, key, &layout, &properties) {
        Ok(id) => Outcome::Specialized(id),
        Err(_) => Outcome::Skipped,
    }
}

fn measure(mut state: ResMut<WarmupState>) {
    let state = &mut *state;
    if let Some(mark) = state.round.mark.take() {
        state.round.slowest = state.round.slowest.max(mark.elapsed());
    }
    let done = state.entries.is_empty() || !ACTIVE.load(Ordering::Relaxed);
    if state.round.queued == 0 || !done {
        return;
    }
    let total = state
        .round
        .started
        .map_or(Duration::ZERO, |started| started.elapsed());
    crate::log_info!(
        "warmup",
        "linked {} pipelines ahead of the world in {} ms; slowest frame {} ms{}",
        state.round.queued,
        total.as_millis(),
        state.round.slowest.as_millis(),
        if state.entries.is_empty() {
            ""
        } else {
            " (menu closed with entries left)"
        }
    );
    state.round = Round::default();
}
