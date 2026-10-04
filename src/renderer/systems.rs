use crate::platform::time::Instant;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::camera::ClearColorConfig;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::light::cluster::ClusterConfig;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::input::{
    CameraAngles, FreecamState, THIRD_PERSON_DISTANCE, ThirdPersonState, apply_gui_nav,
    bot_movement_input, flight_toggle_input, freecam_movement, handle_hotbar_keys,
    handle_hotbar_scroll, handle_keybinds, handle_mouse_input, handle_spectator_action_key,
    handle_spectator_fly_speed_scroll, mouse_look, reload_all_chunks, toggle_advanced_tooltips,
    toggle_chunk_borders, toggle_freecam, toggle_screen, toggle_third_person, web_fullscreen,
    web_pointer_lock, window_focus_cursor,
};
use super::overlays::{
    BlockOutline, BlockOutlineMarker, BreakOverlay, BreakOverlayMarker, ChunkBorderMarker,
    ChunkBorderOverlay, chunk_border_mesh, crumbling_mesh, outline_mesh, update_block_outline,
    update_break_overlay, update_chunk_borders,
};

const SKY_COLOR: Color = Color::srgb(0.753, 0.847, 1.0);

pub const VANILLA_FOV_DEGREES: f32 = 70.0;

use super::frame_view::{FrameView, FrameViewSystems};
use super::terrain_pool::{TerrainOp, TerrainOps};
use super::visgraph::VisibilitySet;
use super::{AppState, PendingSection};
use crate::session::SharedMutex;
use crate::session::SharedState;

#[derive(Resource)]
pub struct Shared(pub Arc<SharedMutex>);

#[derive(Component)]
pub struct WorldCamera;

#[derive(Resource)]
pub struct ChunkMaterial(pub Handle<StandardMaterial>);

#[derive(Resource)]
#[allow(
    dead_code,
    reason = "resource holds the handle that keeps the asset alive"
)]
pub struct WaterMaterial(pub Handle<StandardMaterial>);

#[derive(Resource, Default)]
pub struct LightmapState {
    flicker: f32,
    pub current: super::lightmap::State,
}

fn update_environment(
    view: Res<FrameView>,
    mut env: ResMut<super::environment::Environment>,
    time: Res<Time>,
    gui: Res<crate::gui::GuiState>,
) {
    crate::prof_span!("render:update_environment");
    let ticks = view.day_ticks;
    let eye = view.tick_eye();

    let dim = super::dimension::current();
    let bases = super::environment::Bases::for_dimension(dim);
    let layer = view
        .biome
        .unwrap_or_else(|| super::environment::BiomeLayer::from(&bases));
    let delta_ticks = time.delta_secs() * 20.0;
    env.resolve(
        dim,
        ticks,
        &layer,
        gui.options.render_distance as i32,
        delta_ticks,
        || super::lightmap::levels_at([eye[0] as f32, eye[1] as f32, eye[2] as f32]).1 as f32,
        crate::client::envprobe::camera_has_precipitation(),
    );
}

fn update_lightmap(
    env: Res<super::environment::Environment>,
    mut pixels: ResMut<super::terrain::LightmapPixels>,
    mut state: ResMut<LightmapState>,
    time: Res<Time>,
    gui: Res<crate::gui::GuiState>,
) {
    crate::prof_span!("render:update_lightmap");
    let t = time.elapsed_secs();
    let noise = [
        (t * 12.9898).sin().abs(),
        (t * 78.233).sin().abs(),
        (t * 37.719).sin().abs(),
    ];
    state.flicker = super::lightmap::tick_flicker(state.flicker, noise);

    let gamma = gui.options.gamma as f32 / 100.0;
    let lm =
        super::lightmap::State::sample(super::dimension::current(), &env.sky, state.flicker, gamma);
    state.current = lm;
    super::lightmap::write_image_data(&lm, &mut pixels.data);
}

#[derive(Resource)]
pub struct ChunkIndex {
    columns: HashMap<(i32, i32), Column>,
    free_slots: Vec<u32>,
    next_slot: u32,
    generation: u64,
    y_lo: i32,
    y_hi: i32,
}

impl Default for ChunkIndex {
    fn default() -> Self {
        Self {
            columns: HashMap::new(),
            free_slots: Vec::new(),
            next_slot: 0,
            generation: 0,
            y_lo: i32::MAX,
            y_hi: i32::MIN,
        }
    }
}

#[derive(Default)]
struct Column {
    sections: HashMap<i32, SectionSlot>,
}

#[derive(Clone, Copy)]
pub struct SectionSlot {
    pub slots: [Option<u32>; 2],
    pub vis: VisibilitySet,
}

pub const LAYER_OPAQUE: usize = 0;
pub const LAYER_WATER: usize = 1;

impl Default for SectionSlot {
    fn default() -> Self {
        Self {
            slots: [None, None],
            vis: VisibilitySet::all(),
        }
    }
}

impl ChunkIndex {
    pub fn next_slot(&self) -> u32 {
        self.next_slot
    }

    pub fn store_section(
        &mut self,
        cx: i32,
        cz: i32,
        sec_y: i32,
        vis: VisibilitySet,
        meshes: [super::mesh::MeshBuf; 2],
        ops: &TerrainOps,
    ) {
        let origin = [cx * 16, sec_y, cz * 16];
        let Self {
            columns,
            free_slots,
            next_slot,
            generation,
            y_lo,
            y_hi,
        } = self;
        *y_lo = (*y_lo).min(sec_y);
        *y_hi = (*y_hi).max(sec_y + 16);
        let entry = columns
            .entry((cx, cz))
            .or_default()
            .sections
            .entry(sec_y)
            .or_default();
        entry.vis = vis;

        for (layer, mesh) in meshes.into_iter().enumerate() {
            let held = entry.slots[layer];
            if mesh.is_empty() {
                if let Some(slot) = held {
                    ops.push(TerrainOp::Free { slot });
                    free_slots.push(slot);
                }
                entry.slots[layer] = None;
                continue;
            }
            let slot = match held.or_else(|| free_slots.pop()) {
                Some(slot) => slot,
                None => {
                    let slot = *next_slot;
                    *next_slot += 1;
                    slot
                }
            };
            ops.push(TerrainOp::Upload {
                slot,
                origin,
                water: layer == LAYER_WATER,
                mesh,
            });
            entry.slots[layer] = Some(slot);
        }
        *generation += 1;
    }

    pub fn slots_of(&self, cx: i32, cz: i32, sec_y: i32) -> (Option<u32>, Option<u32>) {
        self.columns
            .get(&(cx, cz))
            .and_then(|column| column.sections.get(&sec_y))
            .map(|slot| (slot.slots[LAYER_OPAQUE], slot.slots[LAYER_WATER]))
            .unwrap_or((None, None))
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn len(&self) -> usize {
        self.columns.values().map(|c| c.sections.len()).sum()
    }

    pub fn y_range(&self) -> (i32, i32) {
        if self.y_lo > self.y_hi {
            return (0, 0);
        }
        (self.y_lo - 16, self.y_hi + 16)
    }

    pub fn visibility(&self, pos: (i32, i32, i32)) -> VisibilitySet {
        self.columns
            .get(&(pos.0, pos.2))
            .and_then(|column| column.sections.get(&pos.1))
            .map(|slot| slot.vis)
            .unwrap_or_else(VisibilitySet::all)
    }

    pub fn sections(&self) -> impl Iterator<Item = ((i32, i32, i32), VisibilitySet)> + '_ {
        self.columns.iter().flat_map(|(&(cx, cz), column)| {
            column
                .sections
                .iter()
                .map(move |(&sec_y, slot)| ((cx, sec_y, cz), slot.vis))
        })
    }

    pub fn clear(&mut self, ops: &TerrainOps) {
        ops.push(TerrainOp::ClearAll);
        self.columns.clear();
        self.free_slots.clear();
        self.next_slot = 0;
        self.y_lo = i32::MAX;
        self.y_hi = i32::MIN;
        self.generation += 1;
    }

    pub fn drop_column(&mut self, key: (i32, i32), ops: &TerrainOps) {
        let Some(column) = self.columns.remove(&key) else {
            return;
        };
        for slot in column.sections.values().flat_map(|s| s.slots).flatten() {
            ops.push(TerrainOp::Free { slot });
            self.free_slots.push(slot);
        }
        self.generation += 1;
    }
}

#[derive(Resource)]
pub struct AssetsReady;

#[derive(Resource)]
pub struct BlockAtlasHandle(pub Handle<Image>);

#[derive(Resource)]
struct PendingAtlas(Image);

#[derive(Resource)]
pub struct BlockTileMap(pub HashMap<String, u32>);

#[derive(Resource, Default)]
struct ProfVisible {
    shown: bool,
    used_as_modifier: bool,
    page: crate::gui::hud::DebugPage,
}

const FPS_WINDOW_MS: f32 = 500.0;

#[derive(Default)]
struct FpsWindow {
    ms: f32,
    frames: u32,
    shown: f32,
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    #[cfg(feature = "click_gui")] mut esp_materials: ResMut<
        Assets<crate::renderer::esp_material::EspMaterial>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut cursor_opts: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut pending_atlas: ResMut<PendingAtlas>,
    start_screen: Res<crate::gui::render::StartScreen>,
) {
    let atlas = images.add(std::mem::take(&mut pending_atlas.0));
    commands.remove_resource::<PendingAtlas>();
    commands.insert_resource(BlockAtlasHandle(atlas.clone()));
    let chunk_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(atlas.clone()),
        perceptual_roughness: 0.95,
        unlit: true,
        alpha_mode: AlphaMode::Mask(0.1),
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(ChunkMaterial(chunk_material));

    let water_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(atlas.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(WaterMaterial(water_material));

    let lightmap = images.add(super::terrain::new_lightmap_image());
    commands.insert_resource(super::terrain_pool::TerrainParams {
        cutoff: if no_cutout() { 0.0 } else { 0.1 },
        ..default()
    });
    commands.insert_resource(super::terrain::LightmapPixels {
        image: lightmap.clone(),
        data: Vec::new(),
    });
    commands.insert_resource(super::terrain_pool::TerrainTextures {
        atlas: atlas.clone(),
        lightmap,
    });

    let break_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(2.0, 2.0, 2.0),
        base_color_texture: Some(atlas.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Multiply,
        ..default()
    });
    let (entity, mesh) = spawn_overlay(
        &mut commands,
        &mut meshes,
        crumbling_mesh(&[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]], 0, 1),
        break_mat.clone(),
        BreakOverlayMarker,
    );
    commands.insert_resource(BreakOverlay {
        entity,
        mesh,
        material: break_mat,
        shown: None,
    });

    let outline_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.4),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let (entity, mesh) = spawn_overlay(
        &mut commands,
        &mut meshes,
        outline_mesh(&[[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]]),
        outline_mat,
        BlockOutlineMarker,
    );
    commands.insert_resource(BlockOutline {
        entity,
        mesh,
        shown: None,
    });

    let chunk_border_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    let (entity, mesh) = spawn_overlay(
        &mut commands,
        &mut meshes,
        chunk_border_mesh(-64.0, 320.0, -64.0),
        chunk_border_mat,
        ChunkBorderMarker,
    );
    commands.insert_resource(ChunkBorderOverlay {
        entity,
        mesh,
        shown: None,
    });

    #[cfg(feature = "click_gui")]
    {
        let esp_mat = esp_materials.add(crate::renderer::esp_material::EspMaterial::default());
        let (entity, mesh) = spawn_overlay(
            &mut commands,
            &mut meshes,
            super::overlays::empty_esp_mesh(),
            esp_mat,
            super::overlays::EspMarker,
        );
        commands.insert_resource(super::overlays::EspOverlay {
            entity,
            mesh,
            shown: None,
        });
    }

    if let Ok(opts) = cursor_opts.single_mut() {
        let open = start_screen.0.is_open();
        let (grab, visible) = world_cursor();
        apply_cursor(
            opts,
            if open { CursorGrabMode::None } else { grab },
            open || visible,
        );
    }

    commands.spawn((
        WorldCamera,
        super::terrain_pool::TerrainView,
        Camera3d::default(),
        ClusterConfig::None,
        Camera {
            clear_color: ClearColorConfig::Custom(SKY_COLOR),
            ..default()
        },
        Msaa::Off,
        Tonemapping::None,
        Projection::Perspective(PerspectiveProjection {
            fov: VANILLA_FOV_DEGREES.to_radians(),
            ..default()
        }),
        DistanceFog {
            color: SKY_COLOR,
            falloff: FogFalloff::Linear {
                start: 96.0,
                end: 160.0,
            },
            ..default()
        },
        Transform::from_xyz(0.0, 64.0, 0.0),
        no_indirect_drawing(),
    ));
}

fn take_budgeted(
    queue: &mut Vec<PendingSection>,
    verts: &mut usize,
    sections: &mut usize,
) -> Vec<PendingSection> {
    if queue.is_empty() || *verts == 0 || *sections == 0 {
        return Vec::new();
    }
    let mut room = 0usize;
    for sec in queue.iter() {
        room += 1;
        let n = sec.opaque.verts.len() + sec.water.verts.len();
        let over = n >= *verts;
        *verts = verts.saturating_sub(n);
        if over || room >= *sections {
            break;
        }
    }
    *sections -= room.min(*sections);
    queue.drain(..room).collect()
}

fn mesh_on_render_thread(mut scratch: Local<crate::client::worker::Scratch>) {
    if crate::client::mesh_worker::active() {
        return;
    }
    {
        #[cfg(feature = "budget")]
        let _t = crate::diag::budget::timed(crate::diag::budget::Slot::Light);
        crate::lighting::level::pump();
    }
    let Some(queue) = crate::CHUNK_Q.get() else {
        return;
    };
    #[cfg(feature = "budget")]
    let _t = crate::diag::budget::timed(crate::diag::budget::Slot::Mesh);
    crate::client::worker::drain_on_caller(queue, &mut scratch);
}

fn poll_shared_state(
    shared: Res<Shared>,
    freecam: Res<FreecamState>,
    third_person: Res<ThirdPersonState>,
    angles: Res<CameraAngles>,
    ops: Res<TerrainOps>,
    mut index: ResMut<ChunkIndex>,
    mut camera: Query<&mut Transform, With<WorldCamera>>,
    mut last_poll_ms: Local<f32>,
    mut dedup_seen: Local<HashSet<(i32, i32, i32)>>,
    mut dedup_keep: Local<Vec<bool>>,
    gui: Res<crate::gui::GuiState>,
    mut view: ResMut<FrameView>,
) {
    crate::prof_span!("render:poll_shared_state");
    #[cfg(feature = "budget")]
    let _t = crate::diag::budget::timed(crate::diag::budget::Slot::Poll);
    let poll_start = Instant::now();

    let lock_t0 = Instant::now();
    let mut state = shared.0.lock().unwrap();
    let lock_wait_ms = lock_t0.elapsed().as_secs_f32() * 1000.0;

    sync_bot_options(&mut state, &gui, &third_person, &freecam);
    view.fill(&state, true);

    if !freecam.active {
        let [ix, iy, iz] = view.player_lerp;
        if let Ok(mut transform) = camera.single_mut() {
            let eye = Vec3::new(ix, iy + view.eye_height, iz);
            if third_person.active {
                let forward = angles.rotation() * Vec3::NEG_Z;
                transform.translation = eye - forward * THIRD_PERSON_DISTANCE;
            } else if view.sleeping {
                let mc_yaw = match view.bed_orientation {
                    Some(dir) => dir.to_y_rot() - 180.0,
                    None => 0.0,
                };
                let bevy_yaw = -mc_yaw - 180.0;
                transform.translation = eye + Vec3::Y * SLEEPING_CAMERA_NUDGE;
                transform.rotation = Quat::from_rotation_y(bevy_yaw.to_radians());
            } else {
                transform.translation = eye;
                transform.rotation = angles.rotation();
            }
        }
    }

    state.profiling.record_lock_wait(lock_wait_ms);
    if *last_poll_ms > 0.0 {
        state
            .profiling
            .record_poll(std::mem::take(&mut *last_poll_ms));
    }
    state.profiling.queue_depth =
        state.session.pending_chunks.len() + state.session.pending_edits.len();
    if let Some(q) = crate::CHUNK_Q.get() {
        q.set_backlog(state.session.pending_chunks.len() + state.session.pending_edits.len());
    }
    crate::client::mesh_worker::note_backlog(
        state.session.pending_chunks.len() + state.session.pending_edits.len(),
    );

    let unloaded: Vec<(i32, i32)> = std::mem::take(&mut state.session.unloaded_chunks);
    let clear_all = std::mem::take(&mut state.session.clear_chunks);

    if state.session.pending_chunks.is_empty()
        && state.session.pending_edits.is_empty()
        && unloaded.is_empty()
        && !clear_all
    {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    const DEFAULT_VERT_BUDGET: usize = 30_000;
    #[cfg(not(target_arch = "wasm32"))]
    const DEFAULT_VERT_BUDGET: usize = 120_000;
    #[cfg(target_arch = "wasm32")]
    const DEFAULT_SECTION_BUDGET: usize = 24;
    #[cfg(not(target_arch = "wasm32"))]
    const DEFAULT_SECTION_BUDGET: usize = 128;
    static VERT_BUDGET: OnceLock<usize> = OnceLock::new();
    static SECTION_BUDGET: OnceLock<usize> = OnceLock::new();
    let max_verts = env_budget(&VERT_BUDGET, "MC_UPLOAD_VERTS", DEFAULT_VERT_BUDGET);
    let max_sections = env_budget(
        &SECTION_BUDGET,
        "MC_UPLOAD_SECTIONS",
        DEFAULT_SECTION_BUDGET,
    );
    let mut verts = max_verts;
    let mut sections = max_sections;
    let mut pending = take_budgeted(&mut state.session.pending_edits, &mut verts, &mut sections);
    pending.extend(take_budgeted(
        &mut state.session.pending_chunks,
        &mut verts,
        &mut sections,
    ));
    drop(state);
    crate::log_debug!("render", "uploading {} sections", pending.len());

    crate::client::tracking::retain_loaded(&mut pending, |s| (s.chunk_x, s.chunk_z));

    if clear_all {
        index.clear(&ops);
    } else {
        for key in unloaded {
            index.drop_column(key, &ops);
        }
    }

    dedup_seen.clear();
    dedup_keep.clear();
    dedup_keep.extend(
        pending
            .iter()
            .rev()
            .map(|c| dedup_seen.insert((c.chunk_x, c.chunk_z, c.sec_y))),
    );
    dedup_keep.reverse();
    let mut next = 0;
    pending.retain(|_| {
        let keep = dedup_keep[next];
        next += 1;
        keep
    });

    {
        let _churn = crate::diag::alloc::scope(crate::diag::alloc::Site::Upload);
        for sec in pending {
            index.store_section(
                sec.chunk_x,
                sec.chunk_z,
                sec.sec_y,
                sec.vis,
                [sec.opaque, sec.water],
                &ops,
            );
            crate::diag::bump(crate::diag::Stat::SectionsUploaded);
        }
    }

    *last_poll_ms = poll_start.elapsed().as_secs_f32() * 1000.0;
}

fn refresh_frame_view(
    shared: Res<Shared>,
    gui: Res<crate::gui::GuiState>,
    third_person: Res<ThirdPersonState>,
    freecam: Res<FreecamState>,
    mut view: ResMut<FrameView>,
) {
    let mut state = shared.0.lock().unwrap();
    sync_bot_options(&mut state, &gui, &third_person, &freecam);
    view.fill(&state, false);
}

fn env_budget(lock: &'static OnceLock<usize>, name: &str, default: usize) -> usize {
    *lock.get_or_init(|| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(default)
    })
}

pub fn partial_ticks(state: &SharedState) -> f32 {
    state
        .session
        .last_tick_time
        .map(|t| (t.elapsed().as_secs_f32() / TICK_SECS).min(1.0))
        .unwrap_or(0.0)
}

const TICK_SECS: f32 = 0.05;

fn spawn_overlay<M: bevy::pbr::Material, K: Component>(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mesh: Mesh,
    material: Handle<M>,
    marker: K,
) -> (Entity, Handle<Mesh>) {
    let mesh = meshes.add(mesh);
    let entity = commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            marker,
        ))
        .id();

    (entity, mesh)
}

pub enum CameraStart {
    Active,
    Inactive,
}

pub fn scene_camera(
    order: isize,
    clear_color: ClearColorConfig,
    start: CameraStart,
    layer: usize,
) -> impl Bundle {
    (
        Camera3d::default(),
        Msaa::Off,
        ClusterConfig::None,
        Camera {
            order,
            clear_color,
            is_active: matches!(start, CameraStart::Active),
            ..default()
        },
        Tonemapping::None,
        DebandDither::Disabled,
        RenderLayers::layer(layer),
    )
}

fn skip_redundant_blits(
    primary_window: Query<Entity, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Camera, &bevy::camera::RenderTarget)>,
) {
    use bevy::camera::{CameraOutputMode, NormalizedRenderTarget};

    let primary = primary_window.single().ok();
    let mut last: HashMap<NormalizedRenderTarget, isize> = HashMap::new();
    for (camera, target) in cameras.iter() {
        if !camera.is_active {
            continue;
        }
        let Some(target) = target.normalize(primary) else {
            continue;
        };
        let order = camera.order;
        last.entry(target)
            .and_modify(|best| *best = (*best).max(order))
            .or_insert(order);
    }

    for (mut camera, target) in cameras.iter_mut() {
        let writes = camera.is_active
            && target
                .normalize(primary)
                .is_some_and(|t| last.get(&t) == Some(&camera.order));
        let skips = matches!(camera.output_mode, CameraOutputMode::Skip);
        if writes == skips {
            camera.output_mode = if writes {
                CameraOutputMode::default()
            } else {
                CameraOutputMode::Skip
            };
        }
    }
}

fn no_cutout() -> bool {
    static OFF: OnceLock<bool> = OnceLock::new();
    *OFF.get_or_init(|| {
        matches!(
            std::env::var("MC_NO_CUTOUT").as_deref(),
            Ok("1") | Ok("true")
        )
    })
}

pub fn eye_height(crouching: bool) -> f32 {
    if crouching {
        EYE_HEIGHT_CROUCHING
    } else {
        EYE_HEIGHT
    }
}

const EYE_HEIGHT: f32 = 1.62;
const EYE_HEIGHT_CROUCHING: f32 = 1.27;
pub const SLEEPING_EYE_HEIGHT: f32 = 0.2;
const SLEEPING_CAMERA_NUDGE: f32 = 0.3;

pub fn session_eye_height(session: &crate::session::SessionState) -> f32 {
    if session.sleeping {
        SLEEPING_EYE_HEIGHT
    } else {
        eye_height(session.crouching)
    }
}

const NAME_TAG_SCALE: f32 = 0.025;

const NAME_TAG_RANGE: f32 = 64.0;
const NAME_TAG_RANGE_DISCRETE: f32 = 32.0;

const BELOW_NAME_RANGE: f32 = 10.0;

fn nametag_scale(vanilla: f32, mul: f32, constant: bool) -> f32 {
    let raw = if constant { mul } else { vanilla * mul };
    raw.clamp(NAME_TAG_MIN_SCALE, NAME_TAG_MAX_SCALE)
}

const NAME_TAG_MIN_SCALE: f32 = 0.08;
const NAME_TAG_MAX_SCALE: f32 = 6.0;

const NDC_CULL: f32 = 1.25;

struct TagProjector {
    eye: Vec3,
    forward: Vec3,
    view_from_world: bevy::math::Affine3A,
    clip_from_view: Mat4,
    units_per_block: f32,
    view: Vec2,
}

impl TagProjector {
    fn new(camera: &Camera, cam_tf: &Transform, projection: &Projection, view: Vec2) -> Self {
        Self {
            eye: cam_tf.translation,
            forward: *cam_tf.forward(),
            view_from_world: cam_tf.compute_affine().inverse(),
            clip_from_view: camera.clip_from_view(),
            units_per_block: match projection {
                Projection::Perspective(p) => view.y / (2.0 * (p.fov * 0.5).tan()),
                _ => view.y,
            },
            view,
        }
    }

    fn project(
        &self,
        name: &[crate::text::Span],
        below: &[crate::text::Span],
        world: Vec3,
        range: f32,
    ) -> Option<crate::gui::hud::NameTag> {
        let Self {
            eye,
            forward,
            view_from_world,
            clip_from_view,
            units_per_block,
            view,
        } = *self;
        let depth = (world - eye).dot(forward);
        if depth <= 0.05 {
            return None;
        }
        let distance = world.distance(eye);
        if distance > range {
            return None;
        }
        let ndc = clip_from_view.project_point3(view_from_world.transform_point3(world));
        if ndc.is_nan() {
            return None;
        }
        if ndc.x.abs() > NDC_CULL || ndc.y.abs() > NDC_CULL {
            return None;
        }
        Some(crate::gui::hud::NameTag {
            name: name.to_vec(),
            below: if distance <= BELOW_NAME_RANGE {
                below.to_vec()
            } else {
                Vec::new()
            },
            x: (ndc.x * 0.5 + 0.5) * view.x,
            y: (0.5 - ndc.y * 0.5) * view.y,
            scale: NAME_TAG_SCALE * units_per_block / depth,
            distance,
            health: None,
            max_health: crate::session::VANILLA_MAX_HEALTH,
            gamemode: crate::gui::hud::TagGamemode::Hidden,
        })
    }
}

fn collect_player_tags(
    players: &[crate::session::OtherPlayerInfo],
    partial: f32,
    projector: &TagProjector,
    tags: &mut Vec<crate::gui::hud::NameTag>,
) {
    use crate::gui::hud::TagGamemode;

    let (on, scale_mul, constant, want_health, want_gamemode) =
        match crate::modules::nametags::config() {
            Some(c) => (
                true,
                c.scale,
                c.sizing == crate::modules::nametags::Sizing::Constant,
                c.health != crate::modules::nametags::Health::Off,
                c.gamemode,
            ),
            None => (false, 1.0, false, false, false),
        };
    for info in players {
        if info.hidden_by_team && !on {
            continue;
        }
        if info.name_tag.is_empty() {
            continue;
        }
        let feet = info.anim.position(partial);
        let height = if info.discrete { 1.5 } else { 1.8 };
        let range = match (on, info.discrete) {
            (true, _) => f32::INFINITY,
            (false, true) => NAME_TAG_RANGE_DISCRETE,
            (false, false) => NAME_TAG_RANGE,
        };
        let world = Vec3::new(feet[0], feet[1] + height + 0.5, feet[2]);
        let Some(mut tag) = projector.project(&info.name_tag, &info.below_name, world, range)
        else {
            continue;
        };
        if on {
            tag.scale = nametag_scale(tag.scale, scale_mul, constant);
            tag.health = want_health.then_some(info.health).flatten();
            tag.max_health = info.max_health;
            tag.gamemode = match (want_gamemode, info.gamemode) {
                (false, _) => TagGamemode::Hidden,
                (true, Some(mode)) => TagGamemode::Known(mode),
                (true, None) => TagGamemode::Unknown,
            };
        }
        tags.push(tag);
    }
}

fn collect_nametags(
    entities: &[crate::entities::feed::EntityAnim],
    crosshair_entity: Option<i32>,
    partial: f32,
    projector: &TagProjector,
    tags: &mut Vec<crate::gui::hud::NameTag>,
) {
    for anim in entities {
        let extras = anim.shared();
        if extras.name_spans.is_empty() {
            continue;
        }
        if !extras.name_visible && crosshair_entity != Some(anim.id) {
            continue;
        }
        if anim.is_invisible() && anim.kind != azalea_registry::builtin::EntityKind::ArmorStand {
            continue;
        }
        let pos = anim.position(partial);
        let world = Vec3::new(pos[0], pos[1] + anim.bounding_box_height() + 0.5, pos[2]);
        tags.extend(projector.project(&extras.name_spans, &[], world, NAME_TAG_RANGE));
    }

    tags.sort_by(|a, b| b.distance.total_cmp(&a.distance));
}

fn collect_hud_info(
    shared: Res<Shared>,
    view: Res<FrameView>,
    time: Res<Time>,
    freecam: Res<FreecamState>,
    prof: Res<ProfVisible>,
    angles: Res<CameraAngles>,
    input: Res<crate::gui::GuiInput>,
    camera_q: Query<(&Camera, &Transform, &Projection), With<WorldCamera>>,
    terrain_stats: Res<super::terrain_pool::TerrainStats>,
    rig_node_q: Query<(), With<crate::entities::EntityRigNode>>,
    rigs: Res<crate::entities::EntityRigs>,
    block_entity_rigs: Res<crate::blockentities::BeRigs>,
    images: Res<Assets<Image>>,
    mut state: ResMut<crate::gui::GuiState>,
    (mut memory_cache, frame_gate, mut gated_ms, mut fps): (
        Local<Option<(Instant, crate::gui::hud::MemoryUse)>>,
        Res<FrameGate>,
        Local<f32>,
        Local<FpsWindow>,
    ),
) {
    crate::prof_span!("render:collect_hud_info");
    let hud_visible = !state.in_menu() && !state.hide_gui;
    let camera = camera_q.single().ok();
    let mut nametags = std::mem::take(&mut state.hud.nametags);
    nametags.clear();
    let mut frame_history = std::mem::take(&mut state.hud.frame_history);

    let (pos, gamemode, flying, in_world) =
        (view.player_lerp, view.gamemode, view.flying, view.in_world);
    let projector = match (camera, hud_visible) {
        (Some((camera, cam_tf, projection)), true) => {
            Some(TagProjector::new(camera, cam_tf, projection, input.size))
        }
        _ => None,
    };
    let (status, profiling, active_effects) = if hud_visible || prof.shown {
        let s = {
            crate::prof_span!("hud:lock");
            shared.0.lock().unwrap()
        };
        if let Some(projector) = &projector {
            collect_player_tags(
                &s.session.other_players,
                view.partial,
                projector,
                &mut nametags,
            );
        }
        (
            if hud_visible {
                s.session.status.clone()
            } else {
                None
            },
            prof.shown.then(|| s.profiling.clone()),
            if hud_visible {
                s.session.active_effects.clone()
            } else {
                Vec::new()
            },
        )
    } else {
        (None, None, Vec::new())
    };
    if let Some(projector) = &projector {
        collect_nametags(
            &view.entities,
            view.crosshair_entity,
            view.partial,
            projector,
            &mut nametags,
        );
    }

    #[cfg(feature = "budget")]
    crate::diag::budget::note_drawn_entities(rig_node_q.iter().count());
    #[cfg(feature = "budget")]
    if let Some(sampled) = crate::diag::budget::sample() {
        crate::log_debug!("render", "frame budget: {}", sampled.line());
    }

    *gated_ms += time.delta_secs() * 1000.0;
    let frame_ms = if frame_gate.render {
        std::mem::take(&mut *gated_ms)
    } else {
        0.0
    };
    if frame_ms > 0.0 {
        if frame_history.len() >= crate::gui::hud::FRAME_HISTORY_LEN {
            frame_history.pop_front();
        }
        frame_history.push_back(frame_ms);
        fps.ms += frame_ms;
        fps.frames += 1;
        if fps.ms >= FPS_WINDOW_MS {
            fps.shown = 1000.0 * fps.frames as f32 / fps.ms;
            fps.ms = 0.0;
            fps.frames = 0;
        }
    }
    let fps_now = if fps.shown > 0.0 {
        fps.shown
    } else if frame_ms > 0.0 {
        1000.0 / frame_ms
    } else {
        0.0
    };

    let debug = prof.shown.then(|| {
        let now = Instant::now();
        let stale =
            memory_cache.is_none_or(|(at, _)| now.duration_since(at) >= MEMORY_SAMPLE_WINDOW);
        if stale {
            let fresh = measure_memory(
                &terrain_stats,
                &rig_node_q,
                &rigs,
                &block_entity_rigs,
                &images,
            );
            *memory_cache = Some((now, fresh));
        }
        crate::gui::hud::DebugInfo {
            page: prof.page,
            frame_ms,
            memory: memory_cache.map(|(_, m)| m).unwrap_or_default(),
            profiling: profiling.unwrap_or_default(),
            pipeline: {
                crate::prof_span!("hud:counts");
                crate::diag::counts()
            },
        }
    });

    let ram = {
        use crate::gui::hud_layout::{ElementId, Visibility};
        let visibility = state.hud_state.layout.get(ElementId::Ram).visibility;
        #[cfg(feature = "hud_editor")]
        let editing = state.screen == crate::gui::Screen::HudEditor;
        #[cfg(not(feature = "hud_editor"))]
        let editing = false;
        crate::diag::MEMORY_METRICS
            && (visibility == Visibility::Shown || (editing && visibility == Visibility::Hidden))
    };
    let runtime = if prof.shown || ram {
        crate::diag::runtime()
    } else {
        crate::diag::Runtime::default()
    };

    state.hud = crate::gui::hud::HudInfo {
        pos,
        gamemode,
        yaw: angles.yaw,
        flying,
        freecam: freecam.active,
        status,
        fps: fps_now,
        in_world,
        runtime,
        frame_history,
        nametags,
        active_effects,
        debug,
    };
}

const MEMORY_SAMPLE_WINDOW: Duration = Duration::from_millis(500);

fn measure_memory(
    terrain: &super::terrain_pool::TerrainStats,
    rig_nodes: &Query<(), With<crate::entities::EntityRigNode>>,
    rigs: &crate::entities::EntityRigs,
    block_entity_rigs: &crate::blockentities::BeRigs,
    images: &Assets<Image>,
) -> crate::gui::hud::MemoryUse {
    let terrain_bytes = terrain.0.used_bytes.load(AtomicOrdering::Relaxed);
    let sections = terrain.0.live_slots.load(AtomicOrdering::Relaxed);

    let (world_bytes, world_chunks) = measure_world_bytes();
    let (light_bytes, light_sections) = crate::client::worldsync::light_map().read().bytes();

    let mut texture_bytes = 0u64;
    let mut textures = 0usize;
    for (_, image) in images.iter() {
        let size = image.texture_descriptor.size;
        let texel = image
            .texture_descriptor
            .format
            .block_copy_size(None)
            .unwrap_or(0) as u64;
        texture_bytes +=
            size.width as u64 * size.height as u64 * size.depth_or_array_layers as u64 * texel;
        textures += 1;
    }

    let (light_engine_bytes, light_engine_sections) = crate::lighting::level::engine_bytes();
    let (baked_bytes, baked_states, baked_models) = crate::blocks::cache_bytes();

    crate::gui::hud::MemoryUse {
        terrain_bytes,
        sections,
        world_bytes,
        world_chunks,
        light_bytes,
        light_sections,
        light_engine_bytes,
        light_engine_sections,
        baked_bytes,
        baked_states,
        baked_models,
        texture_bytes,
        textures,
        entities: rigs.len(),
        entity_nodes: rig_nodes.iter().count(),
        block_entities: block_entity_rigs.len(),
    }
}

fn measure_world_bytes() -> (u64, usize) {
    let last = || {
        (
            LAST_WORLD_BYTES.load(AtomicOrdering::Relaxed),
            LAST_WORLD_CHUNKS.load(AtomicOrdering::Relaxed) as usize,
        )
    };
    let Some(world) = crate::client::tracking::current_world()
        .lock()
        .unwrap()
        .clone()
    else {
        return (0, 0);
    };
    let Some(world) = world.try_read() else {
        return last();
    };
    let mut bytes = 0u64;
    let mut chunk_count = 0usize;
    for pos in world.chunks.chunks().iter() {
        let Some(chunk) = world.chunks.get(pos) else {
            continue;
        };
        let Some(chunk) = chunk.try_read() else {
            return last();
        };
        for section in chunk.sections.iter() {
            bytes += (section.states.storage.data.len() * 8) as u64;
            bytes += (section.biomes.storage.data.len() * 8) as u64;
        }
        chunk_count += 1;
    }

    LAST_WORLD_BYTES.store(bytes, AtomicOrdering::Relaxed);
    LAST_WORLD_CHUNKS.store(chunk_count as u64, AtomicOrdering::Relaxed);
    (bytes, chunk_count)
}

static LAST_WORLD_BYTES: AtomicU64 = AtomicU64::new(0);
static LAST_WORLD_CHUNKS: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "android")]
fn track_native_window_size(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let Some(native) = bevy::android::ANDROID_APP
        .get()
        .and_then(|a| a.native_window())
    else {
        return;
    };
    let (w, h) = (native.width().max(0) as u32, native.height().max(0) as u32);
    if w == 0 || h == 0 {
        return;
    }
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if window.physical_width() != w || window.physical_height() != h {
        window.resolution.set_physical_resolution(w, h);
    }
}

#[cfg(target_os = "android")]
fn log_frame_rate(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    index: Res<ChunkIndex>,
    terrain_stats: Res<super::terrain_pool::TerrainStats>,
    gui: Res<crate::gui::GuiState>,
    device: Res<bevy::render::renderer::RenderDevice>,
    mut frames: Local<u32>,
    mut since: Local<f32>,
    mut limits_logged: Local<bool>,
) {
    const REPORT_EVERY: f32 = 5.0;
    *frames += 1;
    *since += time.delta_secs();
    if *since < REPORT_EVERY {
        return;
    }
    if !*limits_logged {
        *limits_logged = true;
        let limits = device.limits();
        crate::log_info!(
            "render",
            "gpu limits: texture 2d {}, texture layers {}, uniform binding {}, storage binding \
             {}, buffer {}, vertex buffers {}, vertex attributes {}",
            limits.max_texture_dimension_2d,
            limits.max_texture_array_layers,
            limits.max_uniform_buffer_binding_size,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
            limits.max_vertex_buffers,
            limits.max_vertex_attributes,
        );
    }
    let focused = windows.single().map(|w| w.focused).unwrap_or(false);
    let drawn = terrain_stats.0.drawn.load(AtomicOrdering::Relaxed);
    let live = terrain_stats.0.live_slots.load(AtomicOrdering::Relaxed);
    let draws = terrain_stats.0.draws.load(AtomicOrdering::Relaxed);
    crate::log_info!(
        "render",
        "{:.1} fps ({:.1} ms a frame) over {:.0}s, focused {focused}, {} sections indexed, \
         {drawn}/{live} slots drawn in {draws} draws",
        *frames as f32 / *since,
        *since * 1000.0 / *frames as f32,
        *since,
        index.len(),
    );
    let c = crate::diag::counts();
    crate::log_info!(
        "render",
        "render distance {} chunks, supersampling {}, frame cap {}",
        gui.options.render_distance,
        if gui.options.antialiasing {
            "on"
        } else {
            "off"
        },
        if gui.options.max_fps >= crate::gui::MAX_FPS_UNLIMITED {
            "unlimited".to_string()
        } else {
            format!("{} fps", gui.options.max_fps)
        },
    );
    crate::log_info!(
        "render",
        "pipeline: {} packets ({} outside the window), {} columns received ({} dropped), {} lit \
         ({} light jobs dropped), {} mesh jobs, {} sections meshed ({} panicked), {} uploaded",
        c[crate::diag::Stat::ChunkPacketsSeen],
        c[crate::diag::Stat::ChunkPacketsOutOfWindow],
        c[crate::diag::Stat::ChunksReceived],
        c[crate::diag::Stat::ChunksDropped],
        c[crate::diag::Stat::ColumnsLit],
        c[crate::diag::Stat::LightJobsDropped],
        c[crate::diag::Stat::MeshJobs],
        c[crate::diag::Stat::SectionsMeshed],
        c[crate::diag::Stat::MeshPanics],
        c[crate::diag::Stat::SectionsUploaded],
    );
    *frames = 0;
    *since = 0.0;
}

fn log_window_geometry(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut last: Local<Option<(u32, u32, f32)>>,
) {
    let Ok(window) = windows.single() else { return };
    let now = (
        window.physical_width(),
        window.physical_height(),
        window.scale_factor(),
    );
    if *last == Some(now) {
        return;
    }
    *last = Some(now);
    let (pw, ph, scale) = now;
    crate::log_info!(
        "render",
        "window {}x{} physical ({:.0}x{:.0} logical, scale {scale}), aspect {:.3}",
        pw,
        ph,
        window.width(),
        window.height(),
        pw as f32 / ph.max(1) as f32,
    );
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
pub(super) fn apply_cursor(_opts: Mut<CursorOptions>, _grab: CursorGrabMode, _visible: bool) {}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub(super) fn apply_cursor(mut opts: Mut<CursorOptions>, grab: CursorGrabMode, visible: bool) {
    opts.grab_mode = grab;
    opts.visible = visible;
}

fn world_cursor() -> (CursorGrabMode, bool) {
    #[cfg(feature = "mobile_ui")]
    {
        (CursorGrabMode::None, true)
    }
    #[cfg(not(feature = "mobile_ui"))]
    {
        (CursorGrabMode::Locked, false)
    }
}

pub(super) fn reset_menu_state(state: &mut crate::gui::GuiState) {
    state.slots.reset_for_screen_change();
    state.container.reset();
}

pub(super) fn enter_screen(
    next: crate::gui::Screen,
    state: &mut crate::gui::GuiState,
    shared: &Shared,
    windows: &mut Query<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let was_open = state.screen.is_open();
    if next != state.screen {
        crate::platform::keyboard::hide();
    }
    reset_menu_state(state);
    if next != state.screen {
        crate::gui::focus::clear();
    }
    #[cfg(feature = "click_gui")]
    if next == crate::gui::Screen::ClickGui && state.screen != next {
        state.clickgui.opened();
    }
    #[cfg(feature = "hud_editor")]
    if state.screen == crate::gui::Screen::HudEditor && next != state.screen {
        state.hud_state.close();
    }
    #[cfg(feature = "hud_editor")]
    if next == crate::gui::Screen::HudEditor && state.screen != next {
        state.hud_state.opened();
    }
    state.screen = next;
    let open = next.is_open();
    {
        let mut s = shared.0.lock().unwrap();
        s.screen_open = open;
        s.session.attack_held = false;
        s.session.use_held = false;
        s.session.attack_clicked = false;
        s.session.use_clicked = false;
    }
    if open || world_cursor().0 == CursorGrabMode::None {
        crate::platform::pointer::unlock();
    } else {
        crate::platform::pointer::lock();
    }
    if let Ok((window, opts)) = windows.single_mut() {
        let (grab, visible) = world_cursor();
        apply_cursor(
            opts,
            if open { CursorGrabMode::None } else { grab },
            open || visible,
        );
        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        if open && !was_open {
            let mut window = window;
            let center = Vec2::new(window.width() / 2.0, window.height() / 2.0);
            window.set_cursor_position(Some(center));
        }
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        let _ = (&window, was_open);
    }
    if was_open && !open {
        state.suppress_click = true;
    }
}

fn limit_framerate(gui: Res<crate::gui::GuiState>, mut next_frame: Local<Option<Instant>>) {
    if cfg!(target_arch = "wasm32") {
        return;
    }
    let target = gui.options.max_fps;
    let now = Instant::now();
    if target >= crate::gui::MAX_FPS_UNLIMITED {
        *next_frame = None;
        return;
    }
    let period = Duration::from_secs_f64(1.0 / target as f64);
    match *next_frame {
        Some(deadline) if deadline > now => {
            std::thread::sleep(deadline - now);
            *next_frame = Some(deadline + period);
        }
        Some(deadline) => {
            *next_frame = Some((deadline + period).max(now));
        }
        None => *next_frame = Some(now + period),
    }
}

#[derive(Resource)]
pub struct FrameGate {
    pub render: bool,
}

impl Default for FrameGate {
    fn default() -> Self {
        FrameGate { render: true }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn frame_gate_decision(
    now: Instant,
    deadline: Option<Instant>,
    period: Duration,
) -> (bool, Option<Instant>) {
    let tolerance = period / 4;
    match deadline {
        Some(deadline) if now + tolerance < deadline => (false, Some(deadline)),
        Some(deadline) => (true, Some((deadline + period).max(now))),
        None => (true, Some(now + period)),
    }
}

#[cfg(target_arch = "wasm32")]
fn web_frame_gate(
    gui: Res<crate::gui::GuiState>,
    mut gate: ResMut<FrameGate>,
    mut next_frame: Local<Option<Instant>>,
) {
    let target = gui.options.max_fps;
    if target >= crate::gui::MAX_FPS_UNLIMITED {
        *next_frame = None;
        gate.render = true;
        return;
    }
    let period = Duration::from_secs_f64(1.0 / target as f64);
    let (render, next) = frame_gate_decision(Instant::now(), *next_frame, period);
    *next_frame = next;
    gate.render = render;
}

fn unfocused_update_mode() -> bevy::winit::UpdateMode {
    bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs_f64(1. / 30.))
}

#[cfg(target_arch = "wasm32")]
fn web_text_focus_pacing(
    gui: Res<crate::gui::GuiState>,
    mut settings: ResMut<bevy::winit::WinitSettings>,
    mut overridden: Local<bool>,
) {
    let held = (gui.screen.is_open() || *overridden) && crate::platform::keyboard::has_focus();
    if held == *overridden {
        return;
    }
    *overridden = held;
    settings.unfocused_mode = if held {
        settings.focused_mode
    } else {
        unfocused_update_mode()
    };
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource, Default)]
struct GatedCameras(Vec<Entity>);

#[cfg(target_arch = "wasm32")]
fn ungate_cameras(mut gated: ResMut<GatedCameras>, mut cameras: Query<&mut Camera>) {
    for entity in gated.0.drain(..) {
        if let Ok(mut camera) = cameras.get_mut(entity)
            && !camera.is_active
        {
            camera.is_active = true;
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn gate_cameras(
    gate: Res<FrameGate>,
    mut gated: ResMut<GatedCameras>,
    mut cameras: Query<(Entity, &mut Camera)>,
) {
    if gate.render {
        return;
    }
    for (entity, mut camera) in &mut cameras {
        if camera.is_active {
            camera.is_active = false;
            gated.0.push(entity);
        }
    }
}

#[cfg(feature = "budget")]
fn budget_frame_start() {
    crate::diag::budget::frame_start();
}

#[cfg(feature = "budget")]
fn budget_frame_end(gate: Res<FrameGate>) {
    crate::diag::budget::frame_end(gate.render);
}

#[cfg(feature = "budget")]
fn report_render_spans(
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut last: Local<Option<Instant>>,
) {
    let now = Instant::now();
    if last.is_some_and(|at| now.duration_since(at) < Duration::from_secs(1)) {
        return;
    }
    *last = Some(now);

    let mut spans: Vec<(String, f64)> = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let path = diagnostic.path().as_str();
            let node = path.strip_prefix("render/")?.strip_suffix("/elapsed_cpu")?;
            Some((node.to_owned(), diagnostic.smoothed()?))
        })
        .collect();
    if spans.is_empty() {
        return;
    }
    let total: f64 = spans.iter().map(|(_, ms)| ms).sum();
    spans.sort_by(|a, b| b.1.total_cmp(&a.1));
    spans.truncate(8);
    let line = spans
        .iter()
        .map(|(node, ms)| format!("{node} {ms:.3}"))
        .collect::<Vec<_>>()
        .join("  ");
    crate::log_info!("render", "graph cpu {total:.2} ms total: {line}");
}

#[cfg(feature = "budget")]
fn budget_post_start() {
    crate::diag::budget::post_start();
}

#[cfg(feature = "budget")]
fn budget_post_end() {
    crate::diag::budget::post_end();
}

#[cfg(feature = "budget")]
fn budget_render_begin() {
    crate::diag::budget::render_begin();
}

#[cfg(feature = "budget")]
fn budget_mark_upload() {
    crate::diag::budget::render_mark(crate::diag::budget::Phase::Upload);
}

#[cfg(feature = "budget")]
fn budget_mark_acquire() {
    crate::diag::budget::render_mark(crate::diag::budget::Phase::Acquire);
}

#[cfg(feature = "budget")]
fn budget_mark_prepare() {
    crate::diag::budget::render_mark(crate::diag::budget::Phase::Prepare);
}

#[cfg(feature = "budget")]
fn budget_mark_draw() {
    crate::diag::budget::render_mark(crate::diag::budget::Phase::Draw);
}

#[cfg(feature = "budget")]
fn budget_count_render_work(
    opaque: Res<
        bevy::render::render_phase::ViewBinnedRenderPhases<bevy::core_pipeline::core_3d::Opaque3d>,
    >,
    alpha_mask: Res<
        bevy::render::render_phase::ViewBinnedRenderPhases<
            bevy::core_pipeline::core_3d::AlphaMask3d,
        >,
    >,
    transparent: Res<
        bevy::render::render_phase::ViewSortedRenderPhases<
            bevy::core_pipeline::core_3d::Transparent3d,
        >,
    >,
) {
    fn binned<I: bevy::render::render_phase::BinnedPhaseItem>(
        phases: &bevy::render::render_phase::ViewBinnedRenderPhases<I>,
    ) -> usize {
        phases
            .values()
            .map(|phase| {
                phase.batchable_meshes.len()
                    + phase
                        .unbatchable_meshes
                        .values()
                        .map(|bin| bin.entities.len())
                        .sum::<usize>()
                    + phase
                        .non_mesh_items
                        .values()
                        .map(|bin| bin.entities.len())
                        .sum::<usize>()
            })
            .sum()
    }

    let draws = binned(&opaque)
        + binned(&alpha_mask)
        + transparent
            .values()
            .map(|phase| phase.items.len())
            .sum::<usize>();
    crate::diag::budget::note_render_work(opaque.len(), draws);
}

fn note_frame() {
    crate::diag::note_frame();
}

fn sync_app_state(
    view: Res<FrameView>,
    current: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let wanted = if view.in_world {
        AppState::InGame
    } else {
        AppState::Menu
    };
    if *current.get() != wanted {
        next.set(wanted);
    }
}

#[derive(Default)]
struct ZoomEase(f32);

fn apply_fov(
    state: Res<crate::gui::GuiState>,
    view: Res<FrameView>,
    time: Res<Time>,
    mut ease: Local<ZoomEase>,
    mut camera: Query<&mut Projection, With<WorldCamera>>,
) {
    let Ok(mut projection) = camera.single_mut() else {
        return;
    };
    if let Projection::Perspective(perspective) = &mut *projection {
        let modifier = view.fov_modifier;

        let zoom = crate::modules::zoom::settings();
        let (target_fov, target) = (zoom.fov, if zoom.on { 1.0 } else { 0.0 });
        let step = time.delta_secs() / zoom.duration;
        ease.0 = if ease.0 < target {
            (ease.0 + step).min(target)
        } else {
            (ease.0 - step).max(target)
        };

        let base_fov = state.options.fov as f32 + (target_fov - state.options.fov as f32) * ease.0;
        let fov = (base_fov * modifier).to_radians();
        if perspective.fov != fov {
            perspective.fov = fov;
        }
    }
}

fn trace_reextracted_meshes(
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Res<Assets<Mesh>>,
    holders: Query<(Entity, &Mesh3d, Option<&Name>)>,
    mut reported: Local<HashSet<AssetId<Mesh>>>,
) {
    for event in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = event else {
            continue;
        };
        let Some(mesh) = meshes.get(*id) else {
            continue;
        };
        if mesh.asset_usage != RenderAssetUsages::RENDER_WORLD {
            continue;
        }
        if mesh.try_attribute_option(Mesh::ATTRIBUTE_POSITION).is_ok() {
            continue;
        }
        if !reported.insert(*id) {
            continue;
        }
        let held_by = holders
            .iter()
            .find(|(_, handle, _)| handle.0.id() == *id)
            .map(|(entity, _, name)| {
                format!("{entity:?} ({})", name.map_or("unnamed", |n| n.as_str()))
            })
            .unwrap_or_else(|| "no entity".to_string());
        crate::log_warn!(
            "render",
            "mesh {:?} on {held_by} ({:?}) is being asked to extract twice; its data \
             was already taken, so it will not be uploaded and has stopped drawing.",
            id,
            mesh.primitive_topology()
        );
    }
}

fn sync_bot_options(
    s: &mut SharedState,
    state: &crate::gui::GuiState,
    third_person: &ThirdPersonState,
    freecam: &FreecamState,
) {
    let wanted = state.options.render_distance;
    let signing = state.options.allow_signed_chat;
    let effects = state.options.fov_effects as f32 / 100.0;
    let first_person = !third_person.active && !freecam.active;
    if s.render_distance_sent != Some(wanted) {
        s.render_distance_sent = Some(wanted);
        s.session.render_distance_request = Some(wanted);
    }
    let skin_prefs = crate::session::SkinPrefs {
        main_hand_left: state.options.main_hand_left,
        #[cfg(feature = "skins")]
        skin_parts: state.options.skin_parts,
    };
    s.skin_prefs = skin_prefs;
    if s.skin_prefs_sent != Some(skin_prefs) {
        s.skin_prefs_sent = Some(skin_prefs);
        s.session.skin_prefs_request = Some(skin_prefs);
    }
    if s.chat_signing_allowed != signing {
        s.chat_signing_allowed = signing;
    }
    s.session.fov.effects = effects;
    s.session.fov.first_person = first_person;
}

#[cfg(feature = "audio")]
fn sync_audio_listener(camera_q: Query<&GlobalTransform, With<WorldCamera>>) {
    let Ok(transform) = camera_q.single() else {
        return;
    };
    crate::audio::set_listener(
        transform.translation().into(),
        (*transform.forward()).into(),
        (*transform.right()).into(),
    );
}

fn sync_smooth_lighting(state: Res<crate::gui::GuiState>, mut last: Local<Option<bool>>) {
    let wanted = state.options.smooth_lighting;
    if *last == Some(wanted) {
        return;
    }
    *last = Some(wanted);
    crate::renderer::set_smooth_lighting(wanted);
    forward_lighting_options();
}

fn forward_lighting_options() {
    crate::client::mesh_worker::sync_options(
        crate::renderer::lighting_enabled(),
        crate::renderer::smooth_lighting_enabled(),
    );
}

fn sync_lighting_enabled(
    state: Res<crate::gui::GuiState>,
    shared: Res<Shared>,
    mut last: Local<Option<bool>>,
) {
    let wanted = state.options.lighting_enabled;
    if *last == Some(wanted) {
        return;
    }
    let had_previous_value = last.is_some();
    *last = Some(wanted);
    crate::renderer::set_lighting_enabled(wanted);
    forward_lighting_options();
    if had_previous_value {
        shared.0.lock().unwrap().reload_chunks_requested = true;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_vsync(
    state: Res<crate::gui::GuiState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut last: Local<Option<bool>>,
) {
    let wanted = state.options.vsync;
    if *last == Some(wanted) {
        return;
    }
    *last = Some(wanted);
    if let Ok(mut window) = windows.single_mut() {
        window.present_mode = if wanted {
            bevy::window::PresentMode::AutoVsync
        } else {
            bevy::window::PresentMode::AutoNoVsync
        };
    }
}

fn auto_harness(
    time: Res<Time<bevy::time::Real>>,
    mut angles: ResMut<CameraAngles>,
    shared: Res<Shared>,
    mut camera: Query<&mut Transform, With<WorldCamera>>,
    mut cfg: Local<Option<(f32, Option<u64>)>>,
    mut exit: bevy::ecs::message::MessageWriter<bevy::app::AppExit>,
) {
    let (look, quit_at) = *cfg.get_or_insert_with(|| {
        (
            std::env::var("MC_AUTO_LOOK")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            std::env::var("MC_EXIT_AFTER_MS")
                .ok()
                .and_then(|v| v.parse().ok()),
        )
    });
    if let Some(q) = quit_at
        && time.elapsed().as_millis() as u64 >= q
    {
        exit.write(bevy::app::AppExit::Success);
    }
    if look == 0.0 {
        return;
    }
    angles.yaw += look * time.delta_secs();
    if let Ok(mut t) = camera.single_mut() {
        t.rotation = angles.rotation();
    }
    let mut s = shared.0.lock().unwrap();
    s.camera_yaw = angles.yaw;
    s.camera_pitch = angles.pitch;
}

#[cfg_attr(
    not(feature = "profiling"),
    allow(
        dead_code,
        reason = "read only by dump_flamegraph's profiling-gated body"
    )
)]
#[derive(Default)]
struct AutoProf {
    inited: bool,
    at: Vec<u64>,
    fired: usize,
    tag: String,
    pending: String,
}

fn dump_flamegraph(
    keys: Res<ButtonInput<KeyCode>>,
    mut auto: Local<AutoProf>,
    time: Res<Time<bevy::time::Real>>,
) {
    #[cfg(feature = "profiling")]
    {
        let mut path_owned = String::from("flamegraph.html");
        if !auto.inited {
            auto.inited = true;
            auto.tag = std::env::var("MC_PROF_TAG").unwrap_or_else(|_| "cap".into());
            if let Ok(v) = std::env::var("MC_PROF_AT") {
                auto.at = v
                    .split(',')
                    .filter_map(|x| x.trim().parse::<u64>().ok())
                    .collect();
                auto.at.sort_unstable();
            }
        }
        let elapsed_ms = time.elapsed().as_millis() as u64;
        let mut auto_fire = false;
        if auto.fired < auto.at.len() && elapsed_ms >= auto.at[auto.fired] {
            let at = auto.at[auto.fired];
            auto.fired += 1;
            auto.pending = format!("flame_{}_{}.html", auto.tag, at);
            auto_fire = true;
        }
        if !auto.pending.is_empty() {
            path_owned = auto.pending.clone();
        }
        let path: &str = &path_owned;
        if keys.just_pressed(KeyCode::F9) || auto_fire {
            let window = crate::diag::profiling::window();
            crate::diag::profiling::arm(window);
            let _ = crate::diag::thread_cpu();
            crate::log_info!("prof", "capturing {} ms", window.as_millis());
        }
        if crate::diag::profiling::capture_elapsed() {
            let cpu = crate::diag::thread_cpu();
            for t in cpu.iter().take(8) {
                crate::log_info!("prof", "cpu {:>6.1}%  {}", t.percent, t.name);
            }
            match crate::diag::profiling::dump_html(path, &cpu) {
                Ok(()) => crate::log_info!("prof", "wrote {path}"),
                Err(e) => crate::log_error!("prof", "failed to write {path}: {e}"),
            }
            auto.pending.clear();
        }
    }
    #[cfg(not(feature = "profiling"))]
    let _ = (&mut *auto, &time);
    #[cfg(not(feature = "profiling"))]
    if keys.just_pressed(KeyCode::F9) {
        crate::log_warn!(
            "prof",
            "F9 pressed but this build has no --features profiling"
        );
    }
}

fn withheld_features() -> Option<bevy::render::settings::WgpuFeatures> {
    #[cfg(target_os = "android")]
    {
        Some(bevy::render::settings::WgpuFeatures::BUFFER_BINDING_ARRAY)
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

fn constrained_limits() -> Option<bevy::render::settings::WgpuLimits> {
    None
}

#[cfg(target_os = "android")]
pub(crate) fn no_indirect_drawing() -> impl Bundle {
    bevy::render::view::NoIndirectDrawing
}

#[cfg(not(target_os = "android"))]
pub(crate) fn no_indirect_drawing() -> impl Bundle {}

fn toggle_prof_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<ProfVisible>,
    shared: ResMut<Shared>,
    state: Res<crate::gui::GuiState>,
) {
    let Some(modifier) = state
        .keybinds
        .key(crate::gui::keybinds::Action::DebugModifier)
    else {
        return;
    };
    if keys.just_pressed(modifier) {
        visible.used_as_modifier = false;
    }
    if keys.pressed(modifier) && keys.get_just_pressed().any(|k| *k != modifier) {
        visible.used_as_modifier = true;
    }
    if !keys.just_released(modifier) {
        return;
    }
    if visible.used_as_modifier {
        visible.used_as_modifier = false;
        return;
    }
    visible.shown = !visible.shown;
    if visible.shown {
        shared.0.lock().unwrap().profiling.reset_peaks();
    }
}

fn select_debug_page(
    keys: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<ProfVisible>,
    state: Res<crate::gui::GuiState>,
) {
    let held = state
        .keybinds
        .key(crate::gui::keybinds::Action::DebugModifier)
        .is_some_and(|modifier| keys.pressed(modifier));
    if !held {
        return;
    }
    const PAGES: [(KeyCode, crate::gui::hud::DebugPage); 7] = [
        (KeyCode::Digit1, crate::gui::hud::DebugPage::Overview),
        (KeyCode::Digit2, crate::gui::hud::DebugPage::Performance),
        (KeyCode::Digit3, crate::gui::hud::DebugPage::World),
        (KeyCode::Digit4, crate::gui::hud::DebugPage::Network),
        (KeyCode::Digit5, crate::gui::hud::DebugPage::Workers),
        (KeyCode::Digit6, crate::gui::hud::DebugPage::Memory),
        (KeyCode::Digit0, crate::gui::hud::DebugPage::Help),
    ];
    for (key, page) in PAGES {
        if keys.just_pressed(key) {
            visible.page = page;
            visible.shown = true;
        }
    }
}

pub fn run(
    shared: Arc<SharedMutex>,
    atlas: Image,
    tile_map: HashMap<String, u32>,
    gui_atlas: std::sync::Arc<crate::gui::atlas::GuiAtlas>,
    startup: crate::gui::render::StartupState,
    have_assets: bool,
) {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Torch Client".into(),
                    resolution: (1280u32, 720u32).into(),
                    #[cfg(target_os = "android")]
                    present_mode: bevy::window::PresentMode::AutoNoVsync,
                    #[cfg(not(target_os = "android"))]
                    present_mode: bevy::window::PresentMode::AutoVsync,
                    #[cfg(target_arch = "wasm32")]
                    canvas: Some("#canvas".into()),
                    #[cfg(target_arch = "wasm32")]
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                exit_condition: bevy::window::ExitCondition::OnPrimaryClosed,
                ..default()
            })
            .set(bevy::log::LogPlugin {
                custom_layer: crate::diag::custom_layers,
                ..default()
            })
            .set(bevy::render::RenderPlugin {
                render_creation: bevy::render::settings::RenderCreation::Automatic(
                    bevy::render::settings::WgpuSettings {
                        disabled_features: withheld_features(),
                        constrained_limits: constrained_limits(),
                        ..default()
                    },
                ),
                ..default()
            })
            .disable::<bevy::app::PanicHandlerPlugin>(),
    )
    .add_plugins((
        #[cfg(feature = "budget")]
        bevy::render::diagnostic::RenderDiagnosticsPlugin,
    ))
    .insert_resource(bevy::winit::WinitSettings {
        focused_mode: bevy::winit::UpdateMode::Continuous,
        unfocused_mode: unfocused_update_mode(),
    })
    .add_plugins(crate::gui::GuiPlugin {
        atlas: gui_atlas,
        shared: shared.clone(),
        startup,
    })
    .add_plugins(crate::blockentities::BlockEntityPlugin)
    .add_plugins(crate::renderer::player_model::PlayerModelPlugin)
    .add_plugins(crate::renderer::hand::HandPlugin)
    .add_plugins(crate::renderer::panorama::PanoramaPlugin)
    .add_plugins(crate::renderer::occlusion::OcclusionPlugin)
    .add_plugins(crate::renderer::terrain::TerrainMaterialPlugin)
    .add_plugins(crate::renderer::terrain_pool::TerrainPoolPlugin)
    .add_plugins(crate::renderer::entity_material::EntityMaterialPlugin)
    .add_plugins(crate::renderer::esp_material::EspMaterialPlugin)
    .add_plugins(crate::renderer::sky::SkyPlugin)
    .add_plugins(crate::renderer::ssaa::SsaaPlugin)
    .add_plugins(crate::renderer::clouds::CloudPlugin)
    .add_plugins(crate::renderer::post::PostPlugin)
    .add_plugins(crate::entities::EntityPlugin)
    .add_plugins(crate::util::particles::ParticlePlugin)
    .add_plugins(crate::renderer::world_text::WorldTextPlugin)
    .insert_resource(Shared(shared))
    .insert_resource(PendingAtlas(atlas))
    .insert_resource(BlockTileMap(tile_map))
    .insert_resource(ClearColor(SKY_COLOR))
    .init_resource::<ChunkIndex>()
    .init_resource::<CameraAngles>()
    .init_resource::<FreecamState>()
    .init_resource::<ThirdPersonState>()
    .init_resource::<ProfVisible>()
    .init_resource::<FrameGate>()
    .insert_resource(crate::renderer::item_assets::ItemAssets::new())
    .init_state::<AppState>()
    .add_systems(Startup, setup)
    .init_resource::<LightmapState>()
    .init_resource::<super::environment::Environment>()
    .init_resource::<FrameView>()
    .add_systems(
        Update,
        refresh_frame_view
            .in_set(FrameViewSystems)
            .run_if(not(in_state(AppState::InGame))),
    )
    .add_systems(
        Update,
        update_environment
            .in_set(FrameViewSystems)
            .after(refresh_frame_view)
            .after(poll_shared_state),
    )
    .add_systems(bevy::app::PostUpdate, skip_redundant_blits)
    .add_systems(Update, update_lightmap.after(FrameViewSystems))
    .add_systems(Update, sync_app_state.after(FrameViewSystems))
    .add_systems(Update, note_frame)
    .add_systems(
        Update,
        super::screenshot::warm_up.run_if(resource_added::<AssetsReady>),
    )
    .add_systems(First, limit_framerate.before(bevy::time::TimeSystems))
    .add_systems(
        Update,
        mesh_on_render_thread
            .before(poll_shared_state)
            .run_if(in_state(AppState::InGame)),
    )
    .add_systems(
        Update,
        collect_hud_info
            .after(FrameViewSystems)
            .before(crate::gui::render::draw_gui),
    )
    .add_systems(Update, apply_gui_nav)
    .add_systems(
        Update,
        apply_fov.after(FrameViewSystems).before(collect_hud_info),
    );
    #[cfg(feature = "audio")]
    app.add_systems(Update, sync_audio_listener);
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(Update, sync_vsync);
    app.add_systems(Last, trace_reextracted_meshes)
        .add_systems(Update, sync_smooth_lighting)
        .add_systems(Update, sync_lighting_enabled)
        .add_systems(Update, toggle_screen.after(apply_gui_nav))
        .add_systems(
            Update,
            mouse_look
                .after(apply_gui_nav)
                .after(toggle_screen)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            poll_shared_state
                .in_set(FrameViewSystems)
                .after(mouse_look)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(Update, toggle_advanced_tooltips)
        .add_systems(Update, toggle_chunk_borders)
        .add_systems(Update, reload_all_chunks.run_if(in_state(AppState::InGame)))
        .add_systems(
            Update,
            handle_keybinds
                .after(apply_gui_nav)
                .after(toggle_screen)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(Update, window_focus_cursor)
        .add_systems(Update, crate::renderer::input::track_lifecycle)
        .add_systems(Update, log_window_geometry)
        .add_systems(Update, web_fullscreen)
        .add_systems(
            Update,
            web_pointer_lock
                .after(apply_gui_nav)
                .after(toggle_screen)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(Update, {
            #[cfg(feature = "click_gui")]
            {
                super::overlays::update_esp
                    .after(FrameViewSystems)
                    .run_if(in_state(AppState::InGame))
            }
            #[cfg(not(feature = "click_gui"))]
            {
                || {}
            }
        })
        .add_systems(Update, toggle_freecam.run_if(in_state(AppState::InGame)))
        .add_systems(
            Update,
            super::input::poll_module_binds
                .before(toggle_freecam)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            toggle_third_person.run_if(in_state(AppState::InGame)),
        )
        .add_systems(Update, freecam_movement.run_if(in_state(AppState::InGame)))
        .add_systems(
            Update,
            flight_toggle_input.run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            bot_movement_input.run_if(in_state(AppState::InGame)),
        )
        .init_resource::<crate::renderer::intent::Intent>()
        .add_systems(
            Update,
            crate::renderer::intent::publish
                .after(bot_movement_input)
                .after(handle_mouse_input),
        )
        .add_systems(
            Update,
            crate::renderer::intent::release_on_suspend.after(crate::renderer::intent::publish),
        )
        .add_systems(Update, toggle_prof_overlay)
        .add_systems(Update, select_debug_page)
        .add_systems(Update, dump_flamegraph)
        .add_systems(Update, auto_harness)
        .add_systems(
            Update,
            handle_hotbar_keys
                .after(apply_gui_nav)
                .after(toggle_screen)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            handle_hotbar_scroll
                .after(apply_gui_nav)
                .after(toggle_screen)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            handle_spectator_fly_speed_scroll.run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            handle_spectator_action_key
                .after(apply_gui_nav)
                .after(toggle_screen)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            handle_mouse_input
                .after(apply_gui_nav)
                .after(toggle_screen)
                .after(web_pointer_lock)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                update_block_outline,
                update_chunk_borders,
                update_break_overlay,
            )
                .after(FrameViewSystems)
                .run_if(in_state(AppState::InGame)),
        );

    if have_assets {
        app.insert_resource(AssetsReady);
    }

    #[cfg(feature = "budget")]
    app.add_systems(First, budget_frame_start.after(limit_framerate))
        .add_systems(Update, report_render_spans)
        .add_systems(Last, budget_frame_end)
        .add_systems(
            PostUpdate,
            (
                budget_post_start
                    .after(crate::renderer::occlusion::update_occlusion)
                    .before(bevy::transform::TransformSystems::Propagate),
                budget_post_end.after(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            ),
        );
    #[cfg(feature = "budget")]
    if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
        use bevy::render::RenderSystems as Set;
        render_app.add_systems(
            bevy::render::Render,
            (
                budget_render_begin.before(Set::ExtractCommands),
                budget_mark_upload
                    .after(Set::PrepareMeshes)
                    .before(Set::ManageViews),
                budget_mark_acquire
                    .after(Set::ManageViews)
                    .before(Set::Queue),
                budget_mark_prepare.after(Set::Prepare).before(Set::Render),
                budget_mark_draw.in_set(Set::PostCleanup),
                budget_count_render_work
                    .after(Set::PhaseSort)
                    .before(Set::Render),
            ),
        );
    }

    #[cfg(target_arch = "wasm32")]
    app.init_resource::<GatedCameras>()
        .add_systems(First, (web_frame_gate, ungate_cameras).chain())
        .add_systems(Last, gate_cameras)
        .add_systems(Update, web_text_focus_pacing);

    #[cfg(target_os = "android")]
    app.add_systems(Update, track_native_window_size.before(log_window_geometry))
        .add_systems(Update, log_frame_rate);

    #[cfg(feature = "mobile_ui")]
    crate::mobile::register(&mut app);

    if !matches!(std::env::var("MC_EXEC").as_deref(), Ok("mt")) {
        use bevy::ecs::schedule::ExecutorKind;
        let mut schedules = app
            .world_mut()
            .resource_mut::<bevy::ecs::schedule::Schedules>();
        for (_, schedule) in schedules.iter_mut() {
            schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        }
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            let mut schedules = render_app
                .world_mut()
                .resource_mut::<bevy::ecs::schedule::Schedules>();
            for (_, schedule) in schedules.iter_mut() {
                schedule.set_executor_kind(ExecutorKind::SingleThreaded);
            }
        }
    }
    #[cfg(feature = "shader_support")]
    app.add_plugins(crate::renderer::packdraw::ShaderPackPlugin);

    #[cfg(target_arch = "wasm32")]
    app.add_systems(Last, super::input::run_exit_tasks_on_exit);

    app.run();

    #[cfg(not(target_arch = "wasm32"))]
    super::input::run_exit_tasks();
    #[cfg(resource_packs)]
    crate::resourcepacks::restart_if_requested();
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIXTY_HZ: Duration = Duration::from_nanos(16_666_666);

    #[test]
    fn a_callback_a_millisecond_early_still_draws() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(1);
        let (render, next) = frame_gate_decision(now, Some(deadline), SIXTY_HZ);
        assert!(render);
        assert_eq!(next, Some(deadline + SIXTY_HZ));
    }

    #[test]
    fn a_callback_half_a_period_early_is_gated() {
        let now = Instant::now();
        let period = Duration::from_nanos(33_333_333);
        let deadline = now + period / 2;
        let (render, next) = frame_gate_decision(now, Some(deadline), period);
        assert!(!render);
        assert_eq!(next, Some(deadline));
    }

    #[test]
    fn a_late_callback_draws_and_rebases_the_deadline() {
        let deadline = Instant::now();
        let now = deadline + Duration::from_millis(50);
        let (render, next) = frame_gate_decision(now, Some(deadline), SIXTY_HZ);
        assert!(render);
        assert!(next.unwrap() >= now);
    }

    #[test]
    fn the_first_callback_draws_and_starts_the_cadence() {
        let now = Instant::now();
        let (render, next) = frame_gate_decision(now, None, SIXTY_HZ);
        assert!(render);
        assert_eq!(next, Some(now + SIXTY_HZ));
    }
}
