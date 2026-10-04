pub mod banner;
pub mod feed;
pub mod models;
pub mod render;
pub mod text;

use std::collections::HashMap;
use std::sync::Arc;

use azalea_registry::builtin::BlockEntityKind;
use bevy::prelude::*;

use crate::entities::Blend;
use crate::entities::geom::{BakedModel, LayerDef, PartState, bake};
use crate::renderer::entity_material::{EntityMaterial, LightMode, Lit, diffuse_lights};
use feed::{BlockEntityData, BlockEntityInfo, BlockStateInfo};

pub struct BeState {
    pub pos: [i32; 3],
    pub kind: BlockEntityKind,
    pub state: Arc<BlockStateInfo>,
    pub data: Arc<BlockEntityData>,
    pub open: f32,
    pub anim: f32,
    pub draw_outline: bool,
    pub phase: f32,
}

#[derive(Clone, Copy)]
pub struct GeomKey(u64);

impl GeomKey {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    pub fn new() -> GeomKey {
        GeomKey(GeomKey::OFFSET)
    }

    pub fn str(mut self, part: &str) -> GeomKey {
        for byte in part.as_bytes() {
            self = self.byte(*byte);
        }
        self.byte(0xff)
    }

    pub fn mix(mut self, value: u64) -> GeomKey {
        for byte in value.to_le_bytes() {
            self = self.byte(byte);
        }
        self
    }

    fn byte(mut self, byte: u8) -> GeomKey {
        self.0 ^= byte as u64;
        self.0 = self.0.wrapping_mul(GeomKey::PRIME);
        self
    }

    pub fn finish(self) -> u64 {
        self.0
    }
}

impl Default for GeomKey {
    fn default() -> GeomKey {
        GeomKey::new()
    }
}

pub struct BeModelGeom {
    pub key: fn(&BeState) -> Option<u64>,
    pub layer: fn(&BeState) -> LayerDef,
    pub texture: fn(&BeState) -> String,
    pub setup: fn(&BakedModel, &mut [PartState], &BeState),
}

pub struct BeBuiltGeom {
    pub key: fn(&BeState) -> Option<u64>,
    pub build: fn(&BeState, &crate::gui::atlas::GuiAtlas) -> Option<Mesh>,
    pub texture: BuiltTexture,
}

pub enum BuiltTexture {
    #[allow(
        dead_code,
        reason = "the entity-texture arm, unused while sign text is the only built geometry"
    )]
    Entity(fn(&BeState) -> String),
    Font,
}

pub enum BeGeom {
    Model(BeModelGeom),
    Built(BeBuiltGeom),
}

impl BeGeom {
    fn key(&self, st: &BeState) -> Option<u64> {
        match self {
            BeGeom::Model(geom) => (geom.key)(st),
            BeGeom::Built(geom) => (geom.key)(st),
        }
    }
}

pub struct BeSpec {
    pub name: &'static str,
    pub geom: BeGeom,
    pub transform: fn(&BeState) -> Transform,
    pub blend: Blend,
    pub full_bright: fn(&BeState) -> bool,
    pub tint: fn(&BeState) -> [f32; 4],
    pub view_distance: f32,
    pub depth_bias: i32,
}

pub const DEFAULT_VIEW_DISTANCE: f32 = 64.0;

impl BeSpec {
    pub fn model(
        name: &'static str,
        key: fn(&BeState) -> Option<u64>,
        layer: fn(&BeState) -> LayerDef,
        texture: fn(&BeState) -> String,
        setup: fn(&BakedModel, &mut [PartState], &BeState),
    ) -> BeSpec {
        BeSpec::new(
            name,
            BeGeom::Model(BeModelGeom {
                key,
                layer,
                texture,
                setup,
            }),
        )
    }

    pub fn built(
        name: &'static str,
        key: fn(&BeState) -> Option<u64>,
        build: fn(&BeState, &crate::gui::atlas::GuiAtlas) -> Option<Mesh>,
        texture: BuiltTexture,
    ) -> BeSpec {
        BeSpec::new(
            name,
            BeGeom::Built(BeBuiltGeom {
                key,
                build,
                texture,
            }),
        )
    }

    fn new(name: &'static str, geom: BeGeom) -> BeSpec {
        BeSpec {
            name,
            geom,
            transform: identity_transform,
            blend: Blend::Cutout,
            full_bright: never_full_bright,
            tint: untinted,
            view_distance: DEFAULT_VIEW_DISTANCE,
            depth_bias: 0,
        }
    }

    pub fn with_transform(mut self, transform: fn(&BeState) -> Transform) -> BeSpec {
        self.transform = transform;
        self
    }

    pub fn with_blend(mut self, blend: Blend) -> BeSpec {
        self.blend = blend;
        self
    }

    pub fn with_full_bright(mut self, full_bright: fn(&BeState) -> bool) -> BeSpec {
        self.full_bright = full_bright;
        self
    }

    pub fn with_tint(mut self, tint: fn(&BeState) -> [f32; 4]) -> BeSpec {
        self.tint = tint;
        self
    }

    pub fn with_depth_bias(mut self, depth_bias: i32) -> BeSpec {
        self.depth_bias = depth_bias;
        self
    }

    fn light_mode(&self) -> LightMode {
        match &self.geom {
            BeGeom::Model(_) => LightMode::Cardinal,
            BeGeom::Built(_) => LightMode::Flat,
        }
    }
}

fn identity_transform(_st: &BeState) -> Transform {
    Transform::IDENTITY
}

fn never_full_bright(_st: &BeState) -> bool {
    false
}

fn untinted(_st: &BeState) -> [f32; 4] {
    [1.0; 4]
}

#[derive(Resource)]
pub struct BeRegistry {
    pub specs: Vec<BeSpec>,
    by_kind: HashMap<BlockEntityKind, Vec<usize>>,
    empty: Vec<usize>,
    max_view_distance: f32,
}

impl BeRegistry {
    pub fn build() -> BeRegistry {
        let mut registry = BeRegistry {
            specs: Vec::new(),
            by_kind: HashMap::new(),
            empty: Vec::new(),
            max_view_distance: 0.0,
        };
        render::register_all(&mut registry);
        registry.max_view_distance = registry
            .specs
            .iter()
            .map(|spec| spec.view_distance)
            .fold(0.0, f32::max);
        registry
    }

    pub fn add(&mut self, kind: BlockEntityKind, spec: BeSpec) {
        let idx = self.push(spec);
        self.by_kind.entry(kind).or_default().push(idx);
    }

    pub fn add_many(&mut self, kinds: &[BlockEntityKind], spec: BeSpec) {
        let idx = self.push(spec);
        for kind in kinds {
            self.by_kind.entry(*kind).or_default().push(idx);
        }
    }

    fn push(&mut self, spec: BeSpec) -> usize {
        debug_assert!(
            !self.specs.iter().any(|s| s.name == spec.name),
            "duplicate block entity spec name: {}",
            spec.name
        );
        self.specs.push(spec);
        self.specs.len() - 1
    }

    pub fn specs_for(&self, kind: BlockEntityKind) -> &[usize] {
        self.by_kind.get(&kind).unwrap_or(&self.empty)
    }

    #[allow(dead_code, reason = "read by the block entity coverage test")]
    pub fn registered_kinds(&self) -> impl Iterator<Item = BlockEntityKind> + '_ {
        self.by_kind.keys().copied()
    }
}

struct GpuModel {
    model: BakedModel,
    meshes: Vec<Option<Handle<Mesh>>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BeMaterialKey {
    texture: Option<AssetId<Image>>,
    tint: [u32; 4],
    mode: u32,
    blend: Blend,
    cutoff: u32,
    levels: Option<(u8, u8)>,
    dirs: [[u32; 3]; 2],
    depth_bias: i32,
}

struct SharedMaterial {
    handle: Handle<EntityMaterial>,
    epoch: u64,
}

#[derive(Resource, Default)]
struct BeAssets {
    models: HashMap<(usize, u64), GpuModel>,
    images: HashMap<String, Option<Handle<Image>>>,
    materials: HashMap<BeMaterialKey, SharedMaterial>,
    epoch: u64,
}

impl BeAssets {
    fn model(
        &mut self,
        key: (usize, u64),
        geom: &BeModelGeom,
        st: &BeState,
        meshes: &mut Assets<Mesh>,
    ) -> &GpuModel {
        self.models.entry(key).or_insert_with(|| {
            let mut baked = bake(&(geom.layer)(st));
            let handles = baked
                .parts
                .iter_mut()
                .map(|part| part.mesh.take().map(|mesh| meshes.add(mesh)))
                .collect();
            GpuModel {
                model: baked,
                meshes: handles,
            }
        })
    }

    fn image(&mut self, path: &str, images: &mut Assets<Image>) -> Option<Handle<Image>> {
        if let Some(cached) = self.images.get(path) {
            return cached.clone();
        }
        let handle = crate::entities::load_entity_image(path, images);
        if handle.is_none() {
            warn!("block entity texture not found: textures/{path}.png");
        }
        self.images.insert(path.to_string(), handle.clone());
        handle
    }

    fn material(
        &mut self,
        spec: &BeSpec,
        texture: Option<Handle<Image>>,
        levels: Option<(u8, u8)>,
        lit: Lit,
        materials: &mut Assets<EntityMaterial>,
    ) -> Handle<EntityMaterial> {
        let (alpha_mode, cutoff) = blend_modes(spec);
        let key = BeMaterialKey {
            texture: texture.as_ref().map(Handle::id),
            tint: lit.tint.map(f32::to_bits),
            mode: spec.light_mode() as u32,
            blend: spec.blend,
            cutoff: cutoff.to_bits(),
            levels,
            dirs: lit.dirs.map(|d| d.to_array().map(f32::to_bits)),
            depth_bias: spec.depth_bias,
        };
        if let Some(shared) = self.materials.get_mut(&key) {
            if shared.epoch != self.epoch
                && let Some(material) = materials.get_mut(&shared.handle)
            {
                material.params.set(lit);
                shared.epoch = self.epoch;
            }
            return shared.handle.clone();
        }
        if self.materials.len() >= MAX_SHARED_MATERIALS {
            self.materials.clear();
        }
        let handle = materials.add(EntityMaterial::new(
            texture,
            spec.light_mode(),
            alpha_mode,
            cutoff,
            lit,
            spec.depth_bias,
        ));
        self.materials.insert(
            key,
            SharedMaterial {
                handle: handle.clone(),
                epoch: self.epoch,
            },
        );
        handle
    }
}

const MAX_SHARED_MATERIALS: usize = 1024;

struct PartNodes {
    node: Entity,
    geom: Option<Entity>,
}

enum RigBody {
    Parts(Vec<PartNodes>),
    Built {
        #[allow(
            dead_code,
            reason = "holding the handle is the whole of its job; see the field comment"
        )]
        mesh: Option<Handle<Mesh>>,
        node: Entity,
    },
}

struct LayerRig {
    spec: usize,
    root: Entity,
    node: Entity,
    body: RigBody,
    key: u64,
    texture: Option<Handle<Image>>,
    material: Option<Handle<EntityMaterial>>,
    states: Vec<PartState>,
}

struct BeRig {
    kind: BlockEntityKind,
    layers: Vec<LayerRig>,
    keyed: Option<(u64, bool, Option<u64>)>,
}

#[derive(Resource, Default)]
pub struct BeRigs {
    live: HashMap<[i32; 3], BeRig>,
    snapshot: Arc<HashMap<[i32; 3], BlockEntityInfo>>,
    version: u64,
    lights: crate::renderer::lightmap::LightCells,
    preview: Option<(u64, u64, Arc<BlockEntityData>)>,
}

impl BeRigs {
    pub fn len(&self) -> usize {
        self.live.len()
    }
}

#[derive(Component)]
pub(crate) struct BeRigNode;

pub struct BlockEntityPlugin;

impl Plugin for BlockEntityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BeAssets>()
            .init_resource::<BeRigs>()
            .insert_resource(BeRegistry::build())
            .add_systems(
                Update,
                sync_block_entities.run_if(in_state(crate::renderer::AppState::InGame)),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_block_entities(
    mut commands: Commands,
    shared: Res<crate::renderer::Shared>,
    registry: Res<BeRegistry>,
    mut assets: ResMut<BeAssets>,
    mut rigs: ResMut<BeRigs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    gui: Option<Res<crate::gui::render::GuiAssets>>,
    camera_q: Query<&GlobalTransform, With<crate::renderer::systems::WorldCamera>>,
    mut nodes: Query<(&mut Transform, &mut Visibility), With<BeRigNode>>,
    mut mats: Query<&mut MeshMaterial3d<EntityMaterial>>,
    lightmap: Res<crate::renderer::systems::LightmapState>,
    gui_state: Option<Res<crate::gui::GuiState>>,
    #[cfg(feature = "skins")] skins: Res<crate::renderer::skin::SkinTextures>,
) {
    crate::prof_span!("render:sync_block_entities");
    let Some(gui) = gui else { return };
    let camera = camera_q
        .iter()
        .next()
        .map(|t| t.translation())
        .unwrap_or_default();

    let (partial, game_time) = {
        let state = shared.0.lock().unwrap();
        if state.session.block_entities_version != rigs.version {
            rigs.version = state.session.block_entities_version;
            rigs.snapshot = state.session.block_entities.clone();
        }
        (
            crate::renderer::systems::partial_ticks(&state),
            state.session.game_time,
        )
    };
    let snapshot = rigs.snapshot.clone();

    let rigs = &mut *rigs;
    let live = &mut rigs.live;
    let lights = &mut rigs.lights;
    let preview_cache = &mut rigs.preview;
    let preview = gui_state.as_deref().and_then(|gui| gui.sign_edit.preview());
    if preview.is_none() {
        *preview_cache = None;
    }

    let light_map = crate::client::worldsync::light_map().read();
    lights.begin(&lightmap.current);
    assets.epoch = assets.epoch.wrapping_add(1);

    let dirs = diffuse_lights(crate::renderer::dimension::current());

    let drop_distance = registry.max_view_distance * DESPAWN_MARGIN;
    let drop_sq = drop_distance * drop_distance;
    let center_of = |pos: &[i32; 3]| {
        Vec3::new(
            pos[0] as f32 + 0.5,
            pos[1] as f32 + 0.5,
            pos[2] as f32 + 0.5,
        )
    };

    live.retain(|pos, rig| {
        let keep = snapshot.get(pos).map(|info| info.kind) == Some(rig.kind)
            && camera.distance_squared(center_of(pos)) < drop_sq;
        if !keep {
            for layer in &rig.layers {
                commands.entity(layer.root).despawn();
            }
        }
        keep
    });

    for (pos, info) in snapshot.iter() {
        let spec_ids = registry.specs_for(info.kind);
        if spec_ids.is_empty() {
            continue;
        }
        let distance_sq = camera.distance_squared(center_of(pos));
        if distance_sq >= drop_sq {
            continue;
        }
        let draw_outline = distance_sq < SIGN_OUTLINE_DISTANCE * SIGN_OUTLINE_DISTANCE;
        let preview = preview.filter(|p| p.pos == *pos && p.applies(info.rev));
        let data = match preview {
            Some(p) => {
                let fresh = matches!(
                    preview_cache,
                    Some((generation, rev, _)) if *generation == p.generation && *rev == info.rev
                );
                if !fresh {
                    *preview_cache = Some((p.generation, info.rev, Arc::new(p.apply(&info.data))));
                }
                preview_cache
                    .as_ref()
                    .map_or_else(|| info.data.clone(), |(_, _, data)| data.clone())
            }
            None => info.data.clone(),
        };
        let st = BeState {
            pos: *pos,
            kind: info.kind,
            state: info.state.clone(),
            data,
            open: combined_openness(info, &snapshot, partial),
            anim: skull_anim(info, partial),
            draw_outline,
            phase: banner_phase(info, *pos, game_time, partial),
        };

        let levels = block_entity_levels(&light_map, info, *pos);
        let cell = lights.get(&lightmap.current, levels.0, levels.1);

        let rig = live.entry(*pos).or_insert_with(|| BeRig {
            kind: info.kind,
            layers: Vec::new(),
            keyed: None,
        });
        let want_keyed = (info.rev, draw_outline, preview.map(|p| p.generation));
        let keys_may_have_changed = rig.keyed != Some(want_keyed);
        rig.keyed = Some(want_keyed);

        for &idx in spec_ids.iter().filter(|_| keys_may_have_changed) {
            let spec = &registry.specs[idx];
            let key = spec.geom.key(&st);
            let existing = rig.layers.iter().position(|l| l.spec == idx);
            if existing.map(|i| rig.layers[i].key) == key {
                continue;
            }
            if let Some(old) = existing {
                let layer = rig.layers.remove(old);
                commands.entity(layer.root).despawn();
            }
            let Some(key) = key else { continue };
            let layer = spawn_layer(
                &mut commands,
                idx,
                spec,
                key,
                &st,
                lit_for(spec, &st, cell, dirs),
                (!(spec.full_bright)(&st)).then_some(levels),
                LayerSinks {
                    assets: &mut assets,
                    meshes: &mut meshes,
                    images: &mut images,
                    materials: &mut materials,
                    gui: &*gui,
                },
            );
            if let Some(layer) = layer {
                rig.layers.push(layer);
            }
        }

        for layer in &mut rig.layers {
            let spec = &registry.specs[layer.spec];
            let show = distance_sq < spec.view_distance * spec.view_distance;
            if let Ok((_, mut visibility)) = nodes.get_mut(layer.root) {
                let want = if show {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *visibility != want {
                    *visibility = want;
                }
            }
            if !show {
                continue;
            }

            #[cfg(feature = "skins")]
            if let crate::blockentities::feed::BlockEntityData::Skull(skull) = &*st.data
                && let Some(url) = &skull.skin
                && let Some(handle) = skins.get(url)
                && layer.texture.as_ref() != Some(handle)
            {
                layer.texture = Some(handle.clone());
            }

            let levels = (!(spec.full_bright)(&st)).then_some(levels);
            let material = assets.material(
                spec,
                layer.texture.clone(),
                levels,
                lit_for(spec, &st, cell, dirs),
                &mut materials,
            );
            if layer.material.as_ref() != Some(&material) {
                set_layer_material(layer, &material, &mut mats);
            }

            if let (BeGeom::Model(geom), RigBody::Parts(_)) = (&spec.geom, &layer.body) {
                if let Some(gpu) = assets.models.get(&(layer.spec, layer.key)) {
                    gpu.model.reset_pose(&mut layer.states);
                    (geom.setup)(&gpu.model, &mut layer.states, &st);
                }
            }

            apply_layer(layer, &(spec.transform)(&st), *pos, &mut nodes);
        }
    }
}

const DESPAWN_MARGIN: f32 = 1.1;

const SIGN_OUTLINE_DISTANCE: f32 = 16.0;

fn lit_for(
    spec: &BeSpec,
    st: &BeState,
    cell: crate::renderer::lightmap::CellLight,
    dirs: [Vec3; 2],
) -> Lit {
    Lit {
        tint: (spec.tint)(st),
        light: if (spec.full_bright)(st) {
            crate::renderer::lightmap::CellLight::full_bright()
        } else {
            cell
        },
        dirs,
        overlay_alpha: 1.0,
        overlay_white: false,
    }
}

fn set_layer_material(
    layer: &mut LayerRig,
    material: &Handle<EntityMaterial>,
    mats: &mut Query<&mut MeshMaterial3d<EntityMaterial>>,
) {
    let mut point_at = |entity: Entity| {
        if let Ok(mut slot) = mats.get_mut(entity) {
            if slot.0 != *material {
                slot.0 = material.clone();
            }
        }
    };
    match &layer.body {
        RigBody::Parts(parts) => {
            for part in parts {
                if let Some(geom) = part.geom {
                    point_at(geom);
                }
            }
        }
        RigBody::Built { node, .. } => point_at(*node),
    }
    layer.material = Some(material.clone());
}

fn combined_openness(
    info: &BlockEntityInfo,
    all: &HashMap<[i32; 3], BlockEntityInfo>,
    partial: f32,
) -> f32 {
    let own = openness(info, partial);
    match info.partner.and_then(|pos| all.get(&pos)) {
        Some(other) => own.max(openness(other, partial)),
        None => own,
    }
}

fn openness(info: &BlockEntityInfo, partial: f32) -> f32 {
    info.open_prev + (info.open - info.open_prev) * partial
}

fn skull_anim(info: &BlockEntityInfo, partial: f32) -> f32 {
    if info.animating {
        info.animation_tick as f32 + partial
    } else {
        info.animation_tick as f32
    }
}

fn banner_phase(info: &BlockEntityInfo, pos: [i32; 3], game_time: u64, partial: f32) -> f32 {
    if info.kind != BlockEntityKind::Banner {
        return 0.0;
    }
    let hash = (pos[0] as i64) * 7 + (pos[1] as i64) * 9 + (pos[2] as i64) * 13;
    let ticks = hash.wrapping_add(game_time as i64).rem_euclid(100);
    (ticks as f32 + partial) / 100.0
}

fn block_entity_levels(
    light: &crate::lighting::level::LightMap,
    info: &BlockEntityInfo,
    pos: [i32; 3],
) -> (u8, u8) {
    let at = |pos: [i32; 3]| {
        crate::renderer::lightmap::resolved_levels(light.at(pos[0], pos[1], pos[2]))
    };
    let (mut block, mut sky) = at(pos);
    if let Some(partner) = info.partner {
        let (other_block, other_sky) = at(partner);
        block = block.max(other_block);
        sky = sky.max(other_sky);
    }
    (block, sky)
}

struct LayerSinks<'a> {
    assets: &'a mut BeAssets,
    meshes: &'a mut Assets<Mesh>,
    images: &'a mut Assets<Image>,
    materials: &'a mut Assets<EntityMaterial>,
    gui: &'a crate::gui::render::GuiAssets,
}

#[allow(clippy::too_many_arguments)]
fn spawn_layer(
    commands: &mut Commands,
    idx: usize,
    spec: &BeSpec,
    key: u64,
    st: &BeState,
    lit: Lit,
    levels: Option<(u8, u8)>,
    sinks: LayerSinks<'_>,
) -> Option<LayerRig> {
    let LayerSinks {
        assets,
        meshes,
        images,
        materials,
        gui,
    } = sinks;
    match &spec.geom {
        BeGeom::Model(geom) => {
            let texture = (geom.texture)(st);
            let image = assets.image(&texture, images)?;
            let (root, node) = spawn_roots(commands, spec, st);
            let material = assets.material(spec, Some(image.clone()), levels, lit, materials);
            let gpu = assets.model((idx, key), geom, st, meshes);
            let states = gpu.model.rest_pose();
            let mut parts: Vec<PartNodes> = Vec::with_capacity(gpu.model.parts.len());
            for (i, part) in gpu.model.parts.iter().enumerate() {
                let part_node = commands
                    .spawn((
                        states[i].transform(),
                        Visibility::Inherited,
                        BeRigNode,
                        Name::new(part.name.clone()),
                    ))
                    .id();
                let geom_node = gpu.meshes[i].as_ref().map(|mesh| {
                    let mut spawned = commands.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::default(),
                        Visibility::Inherited,
                        BeRigNode,
                    ));
                    #[cfg(feature = "builtin_shaders")]
                    spawned.insert(bevy::light::NotShadowCaster);
                    let entity = spawned.id();
                    commands.entity(part_node).add_child(entity);
                    entity
                });
                parts.push(PartNodes {
                    node: part_node,
                    geom: geom_node,
                });
            }
            for (i, part) in gpu.model.parts.iter().enumerate() {
                let parent = match part.parent {
                    Some(p) => parts[p].node,
                    None => node,
                };
                commands.entity(parent).add_child(parts[i].node);
            }
            Some(LayerRig {
                spec: idx,
                root,
                node,
                body: RigBody::Parts(parts),
                key,
                texture: Some(image),
                material: Some(material),
                states,
            })
        }
        BeGeom::Built(geom) => {
            let mesh = (geom.build)(st, &gui.atlas)?;
            let image = match &geom.texture {
                BuiltTexture::Entity(path) => assets.image(&(path)(st), images)?,
                BuiltTexture::Font => gui.image.clone(),
            };
            let (root, node) = spawn_roots(commands, spec, st);
            let material = assets.material(spec, Some(image.clone()), levels, lit, materials);
            let handle = meshes.add(mesh);
            let mut spawned = commands.spawn((
                Mesh3d(handle.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Visibility::Inherited,
                BeRigNode,
            ));
            #[cfg(feature = "builtin_shaders")]
            spawned.insert(bevy::light::NotShadowCaster);
            let geom_node = spawned.id();
            commands.entity(node).add_child(geom_node);
            Some(LayerRig {
                spec: idx,
                root,
                node,
                body: RigBody::Built {
                    mesh: Some(handle),
                    node: geom_node,
                },
                key,
                texture: Some(image),
                material: Some(material),
                states: Vec::new(),
            })
        }
    }
}

fn spawn_roots(commands: &mut Commands, spec: &BeSpec, st: &BeState) -> (Entity, Entity) {
    let root = commands
        .spawn((
            Transform::from_xyz(st.pos[0] as f32, st.pos[1] as f32, st.pos[2] as f32),
            Visibility::Hidden,
            BeRigNode,
            Name::new(spec.name),
        ))
        .id();
    let node = commands
        .spawn((Transform::default(), Visibility::Inherited, BeRigNode))
        .id();
    commands.entity(root).add_child(node);
    (root, node)
}

fn blend_modes(spec: &BeSpec) -> (AlphaMode, f32) {
    let (alpha_mode, cutoff) = match spec.blend {
        Blend::Cutout => (AlphaMode::Mask(0.1), 0.1),
        Blend::Translucent => (AlphaMode::Blend, 0.1),
        Blend::Additive => (AlphaMode::Add, 0.0),
    };
    let cutoff = match &spec.geom {
        BeGeom::Built(_) => 0.0,
        BeGeom::Model(_) => cutoff,
    };
    (alpha_mode, cutoff)
}

fn apply_layer(
    layer: &LayerRig,
    transform: &Transform,
    pos: [i32; 3],
    nodes: &mut Query<(&mut Transform, &mut Visibility), With<BeRigNode>>,
) {
    if let Ok((mut t, _)) = nodes.get_mut(layer.root) {
        let want = Vec3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32);
        if t.translation != want {
            t.translation = want;
        }
    }
    if let Ok((mut t, _)) = nodes.get_mut(layer.node) {
        if *t != *transform {
            *t = *transform;
        }
    }
    let RigBody::Parts(parts) = &layer.body else {
        return;
    };
    for (part, state) in parts.iter().zip(&layer.states) {
        if let Ok((mut t, mut visibility)) = nodes.get_mut(part.node) {
            let pose = state.transform();
            if *t != pose {
                *t = pose;
            }
            let want = if state.visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != want {
                *visibility = want;
            }
        }
        if let Some(geom) = part.geom
            && let Ok((_, mut visibility)) = nodes.get_mut(geom)
        {
            let want = if state.skip_draw {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
            if *visibility != want {
                *visibility = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_bakes_and_its_texture_exists() {
        let registry = BeRegistry::build();
        let mut missing: Vec<String> = Vec::new();
        let mut baked_specs: Vec<&str> = Vec::new();
        for st in render::test_states() {
            for &idx in registry.specs_for(st.kind) {
                let spec = &registry.specs[idx];
                let BeGeom::Model(geom) = &spec.geom else {
                    continue;
                };
                if (geom.key)(&st).is_none() {
                    continue;
                }
                let baked = bake(&(geom.layer)(&st));
                assert!(
                    baked.parts.len() > 1,
                    "{} baked to nothing but a root",
                    spec.name
                );
                if !baked_specs.contains(&spec.name) {
                    baked_specs.push(spec.name);
                }
                let path = (geom.texture)(&st);
                let file = crate::assets_root().join(format!("textures/{path}.png"));
                if !file.exists() {
                    missing.push(format!("{} -> textures/{path}.png", spec.name));
                }
            }
        }
        for spec in &registry.specs {
            if matches!(spec.geom, BeGeom::Model(_)) {
                assert!(
                    baked_specs.contains(&spec.name),
                    "{} was never baked by any test state",
                    spec.name
                );
            }
        }
        assert!(
            missing.is_empty(),
            "missing block entity textures:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn every_baked_model_carries_the_entity_vertex_layout() {
        let required = [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_COLOR,
        ];
        let registry = BeRegistry::build();
        for st in render::test_states() {
            for &idx in registry.specs_for(st.kind) {
                let spec = &registry.specs[idx];
                let BeGeom::Model(geom) = &spec.geom else {
                    continue;
                };
                for part in &bake(&(geom.layer)(&st)).parts {
                    let Some(mesh) = &part.mesh else { continue };
                    for attribute in &required {
                        assert!(
                            mesh.attribute(attribute.id).is_some(),
                            "{}: part mesh has no {}",
                            spec.name,
                            attribute.name,
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "coverage report, not a regression gate"]
    fn every_vanilla_renderer_is_accounted_for() {
        const VANILLA: &[BlockEntityKind] = &[
            BlockEntityKind::Sign,
            BlockEntityKind::HangingSign,
            BlockEntityKind::MobSpawner,
            BlockEntityKind::Piston,
            BlockEntityKind::Chest,
            BlockEntityKind::EnderChest,
            BlockEntityKind::TrappedChest,
            BlockEntityKind::EnchantingTable,
            BlockEntityKind::Lectern,
            BlockEntityKind::EndPortal,
            BlockEntityKind::EndGateway,
            BlockEntityKind::Beacon,
            BlockEntityKind::Skull,
            BlockEntityKind::Banner,
            BlockEntityKind::StructureBlock,
            BlockEntityKind::TestInstanceBlock,
            BlockEntityKind::ShulkerBox,
            BlockEntityKind::Bed,
            BlockEntityKind::Conduit,
            BlockEntityKind::Bell,
            BlockEntityKind::Campfire,
            BlockEntityKind::BrushableBlock,
            BlockEntityKind::DecoratedPot,
            BlockEntityKind::TrialSpawner,
            BlockEntityKind::Vault,
            BlockEntityKind::CopperGolemStatue,
            BlockEntityKind::Shelf,
        ];

        let registry = BeRegistry::build();
        let drawn: Vec<BlockEntityKind> = registry.registered_kinds().collect();
        let missing: Vec<&str> = VANILLA
            .iter()
            .filter(|kind| !drawn.contains(kind))
            .map(|kind| {
                use azalea_registry::Registry as _;
                kind.to_str()
            })
            .collect();
        assert!(
            missing.is_empty(),
            "{} vanilla block entity renderers are not ported:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }

    #[test]
    fn geometry_keys_separate_the_models_they_bake() {
        let registry = BeRegistry::build();
        let mut seen: HashMap<(usize, u64), usize> = HashMap::new();
        for st in render::test_states() {
            for &idx in registry.specs_for(st.kind) {
                let spec = &registry.specs[idx];
                let BeGeom::Model(geom) = &spec.geom else {
                    continue;
                };
                let Some(key) = (geom.key)(&st) else {
                    continue;
                };
                let key = (idx, key);
                let parts = bake(&(geom.layer)(&st)).parts.len();
                match seen.get(&key) {
                    Some(previous) => assert_eq!(
                        *previous, parts,
                        "{} key {:#x} bakes two different models",
                        spec.name, key.1
                    ),
                    None => {
                        seen.insert(key, parts);
                    }
                }
            }
        }
    }
}
