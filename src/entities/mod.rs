pub mod bake;
pub mod blockmodel;
pub mod feed;
pub mod geom;
pub mod keyframe;
pub mod models;
pub mod physics;
pub mod quads;
pub mod registry;
pub mod render;
pub mod state;

use bevy::platform::collections::HashMap;

use azalea_registry::builtin::EntityKind;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

pub use registry::Registry;
pub use state::{Billboard, Direction, EntityState, Pose};

pub use crate::renderer::entity_material::LightMode;
use crate::renderer::entity_material::{EntityMaterial, Lit, diffuse_lights};
use geom::{BakedModel, LayerDef, PartState, PosedPart};

pub const MODEL_Y_OFFSET: f32 = -1.501;

#[derive(Clone, Copy, Debug)]
pub struct RootPose {
    pub world_offset: Vec3,
    pub rotation: Quat,
    pub scale: f32,
    pub extra_rotation: Quat,
    pub extra_offset: Vec3,
    pub hook: Transform,
}

impl Default for RootPose {
    fn default() -> Self {
        RootPose {
            world_offset: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: 1.0,
            extra_rotation: Quat::IDENTITY,
            extra_offset: Vec3::ZERO,
            hook: Transform::IDENTITY,
        }
    }
}

pub fn hook_scale_then_translate(scale: Vec3, translate: Vec3) -> Transform {
    Transform {
        translation: scale * translate,
        rotation: Quat::IDENTITY,
        scale,
    }
}

pub fn setup_rotations(st: &EntityState, flip_degrees: f32) -> RootPose {
    let mut pose = RootPose {
        scale: st.scale,
        ..RootPose::default()
    };

    let mut body_rot = st.body_rot;
    if st.is_fully_frozen {
        body_rot += (st.age_ticks.floor() * 3.25).cos() * std::f32::consts::PI * 0.4;
    }

    if !st.has_pose(Pose::Sleeping) {
        pose.rotation = Quat::from_rotation_y((180.0 - body_rot).to_radians());
    }

    if st.death_time > 0.0 {
        let fall = (((st.death_time - 1.0) / 20.0 * 1.6).sqrt()).min(1.0);
        pose.extra_rotation = Quat::from_rotation_z((fall * flip_degrees).to_radians());
    } else if st.is_auto_spin_attack {
        pose.extra_rotation = Quat::from_rotation_x((-90.0 - st.x_rot).to_radians())
            * Quat::from_rotation_y((st.age_ticks * -75.0).to_radians());
    } else if st.has_pose(Pose::Sleeping) {
        let angle = match st.bed_orientation {
            Some(Direction::South) => 90.0,
            Some(Direction::West) => 0.0,
            Some(Direction::North) => 270.0,
            Some(Direction::East) => 180.0,
            _ => body_rot,
        };
        pose.extra_rotation = Quat::from_rotation_y(angle.to_radians())
            * Quat::from_rotation_z(flip_degrees.to_radians())
            * Quat::from_rotation_y(270.0_f32.to_radians());
        if let Some(dir) = st.bed_orientation {
            let head_offset = st.eye_height - 0.1;
            pose.world_offset = Vec3::new(
                -dir.step_x() * head_offset,
                0.0,
                -dir.step_z() * head_offset,
            );
        }
    } else if st.is_upside_down {
        pose.extra_offset = Vec3::new(0.0, (st.bounding_box_height + 0.1) / st.scale, 0.0);
        pose.extra_rotation = Quat::from_rotation_z(std::f32::consts::PI);
    }

    pose.world_offset += Vec3::from(st.extras.render_offset);

    pose
}

pub fn default_root(st: &EntityState) -> RootPose {
    setup_rotations(st, 90.0)
}

#[allow(
    dead_code,
    reason = "the root hook for a non-living entity; no ported renderer needs it yet"
)]
pub fn yaw_only_root(st: &EntityState) -> RootPose {
    RootPose {
        rotation: Quat::from_rotation_y((180.0 - st.body_rot).to_radians()),
        scale: st.scale,
        ..RootPose::default()
    }
}

pub fn always_visible(_st: &EntityState) -> bool {
    true
}

pub fn no_tint(_st: &EntityState) -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn hurt_overlay_alpha(st: &EntityState) -> f32 {
    if st.has_red_overlay { 0.698 } else { 1.0 }
}

fn tnt_flash_alpha(st: &EntityState) -> Option<f32> {
    (st.kind == EntityKind::Tnt && (st.extras.fuse / 5) % 2 == 0).then_some(0.25)
}

fn overlay_alpha(st: &EntityState) -> f32 {
    tnt_flash_alpha(st).unwrap_or_else(|| hurt_overlay_alpha(st))
}

#[derive(Clone, Copy, Debug)]
pub struct CameraView {
    pub orientation: Quat,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl CameraView {
    pub fn new(orientation: Quat, position: Vec3) -> CameraView {
        let (yaw, pitch, _) = orientation.to_euler(EulerRot::YXZ);
        CameraView {
            orientation,
            position,
            yaw,
            pitch,
        }
    }
}

impl Default for CameraView {
    fn default() -> Self {
        CameraView {
            orientation: Quat::IDENTITY,
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

pub const VIEW_RANGE_BLOCKS: f32 = 64.0;

pub fn display_quat(q: [f32; 4]) -> Quat {
    Quat::from_xyzw(q[0], q[1], q[2], q[3])
}

fn within_view_range(st: &EntityState, camera: &CameraView) -> bool {
    let Some(display) = st.extras.display.as_deref() else {
        return true;
    };
    let limit = display.view_range * VIEW_RANGE_BLOCKS;
    Vec3::from(st.pos).distance_squared(camera.position) <= limit * limit
}

pub fn display_orientation(
    billboard: Billboard,
    entity_yaw: f32,
    entity_pitch: f32,
    camera: &CameraView,
) -> Quat {
    let yaw = match billboard {
        Billboard::Center | Billboard::Vertical => camera.yaw,
        Billboard::Fixed | Billboard::Horizontal => -entity_yaw.to_radians(),
    };
    let pitch = match billboard {
        Billboard::Center | Billboard::Horizontal => camera.pitch,
        Billboard::Fixed | Billboard::Vertical => entity_pitch.to_radians(),
    };
    Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)
}

pub fn display_transform(
    display: &state::Display,
    outer: Quat,
    inner: Quat,
) -> (Transform, Option<Quat>) {
    let left = display_quat(display.left_rotation);
    let below = display_quat(display.right_rotation) * inner;
    let (rotation, leftover) = if display.uniform_scale() {
        (left * below, None)
    } else if display.no_right_rotation() && inner == Quat::IDENTITY {
        (left, None)
    } else {
        (left, Some(below))
    };
    let transform = Transform {
        translation: outer * Vec3::from(display.translation),
        rotation: outer * rotation,
        scale: Vec3::from(display.scale),
    };
    (transform, leftover)
}

pub fn display_root(
    st: &EntityState,
    camera: &CameraView,
    inner: Quat,
) -> (Transform, Option<Quat>) {
    let Some(display) = st.extras.display.as_deref() else {
        return (Transform::IDENTITY, None);
    };
    let orientation = display_orientation(display.billboard, st.body_rot, st.x_rot, camera);
    display_transform(display, orientation, inner)
}

pub enum RootHook {
    Plain(fn(&EntityState) -> RootPose),
    Camera(fn(&EntityState, &CameraView) -> RootPose),
}

impl RootHook {
    pub(crate) fn pose(&self, st: &EntityState, camera: &CameraView) -> RootPose {
        match self {
            RootHook::Plain(f) => f(st),
            RootHook::Camera(f) => f(st, camera),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Blend {
    Cutout,
    Translucent,
    Additive,
}

pub type TexturePath = std::borrow::Cow<'static, str>;

pub struct ModelGeom {
    pub layer: fn() -> LayerDef,
    pub texture: fn(&EntityState) -> TexturePath,
    pub setup: fn(&BakedModel, &mut [PartState], &EntityState),
}

pub struct ItemGeom {
    pub item: fn(&EntityState) -> TexturePath,
    pub display: &'static str,
    pub inner: fn(&EntityState) -> Transform,
    pub cluster: Option<ItemCluster>,
    pub hover: bool,
}

pub struct ItemCluster {
    pub count: fn(&EntityState) -> u32,
    pub seed: fn(&EntityState) -> i64,
}

pub fn rendered_amount(count: u32) -> usize {
    match count {
        0..=1 => 1,
        2..=16 => 2,
        17..=32 => 3,
        33..=48 => 4,
        _ => 5,
    }
}

const ITEM_MIN_HOVER_HEIGHT: f32 = 0.0625;

const ITEM_BUNDLE_OFFSET_SCALE: f32 = 0.15;

const FLAT_ITEM_DEPTH_THRESHOLD: f32 = 0.0625;

const ITEM_COPIES_MAX: usize = 5;

fn cluster_offsets(count: usize, depth: f32, seed: i64, mut out: impl FnMut(Vec3)) {
    let mut rng = crate::util::javarandom::JavaRandom::new(seed);
    let signed = |rng: &mut crate::util::javarandom::JavaRandom| rng.next_f32() * 2.0 - 1.0;
    if depth > FLAT_ITEM_DEPTH_THRESHOLD {
        out(Vec3::ZERO);
        for _ in 1..count {
            let x = signed(&mut rng) * ITEM_BUNDLE_OFFSET_SCALE;
            let y = signed(&mut rng) * ITEM_BUNDLE_OFFSET_SCALE;
            let z = signed(&mut rng) * ITEM_BUNDLE_OFFSET_SCALE;
            out(Vec3::new(x, y, z));
        }
    } else {
        let step = depth * 1.5;
        let mut z = -step * (count as f32 - 1.0) / 2.0;
        out(Vec3::new(0.0, 0.0, z));
        for _ in 1..count {
            z += step;
            let x = signed(&mut rng) * ITEM_BUNDLE_OFFSET_SCALE * 0.5;
            let y = signed(&mut rng) * ITEM_BUNDLE_OFFSET_SCALE * 0.5;
            out(Vec3::new(x, y, z));
        }
    }
}

pub fn display_bounds(bounds: (Vec3, Vec3), display: &Transform) -> (Vec3, Vec3) {
    let (lo, hi) = bounds;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for i in 0..8 {
        let corner = Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        );
        let moved = display.transform_point(corner);
        min = min.min(moved);
        max = max.max(moved);
    }
    (min, max)
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BlockRef {
    State(u32),
    Model(FrameModel),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FrameModel {
    Frame,
    FrameMap,
    GlowFrame,
    GlowFrameMap,
}

impl FrameModel {
    pub fn path(self) -> &'static str {
        match self {
            FrameModel::Frame => "block/item_frame",
            FrameModel::FrameMap => "block/item_frame_map",
            FrameModel::GlowFrame => "block/glow_item_frame",
            FrameModel::GlowFrameMap => "block/glow_item_frame_map",
        }
    }
}

pub struct BlockGeom {
    pub block: fn(&EntityState) -> Option<BlockRef>,
}

pub struct BuiltGeom {
    pub key: fn(&EntityState) -> TexturePath,
    pub build: fn(&EntityState) -> Mesh,
    pub texture: fn(&EntityState) -> Option<TexturePath>,
}

pub enum Geom {
    Model(ModelGeom),
    Item(ItemGeom),
    Block(BlockGeom),
    Built(BuiltGeom),
}

pub struct RenderSpec {
    pub name: &'static str,
    pub geom: Geom,
    pub root: RootHook,
    pub visible: fn(&EntityState) -> bool,
    pub blend: Blend,
    pub depth_bias: i32,
    pub tint: fn(&EntityState) -> [f32; 4],
    pub respects_invisibility: bool,
    pub light: Option<LightMode>,
}

impl RenderSpec {
    pub fn new(
        name: &'static str,
        layer: fn() -> LayerDef,
        texture: fn(&EntityState) -> TexturePath,
        setup: fn(&BakedModel, &mut [PartState], &EntityState),
    ) -> RenderSpec {
        RenderSpec::with_geom(
            name,
            Geom::Model(ModelGeom {
                layer,
                texture,
                setup,
            }),
        )
    }

    pub fn item(
        name: &'static str,
        item: fn(&EntityState) -> TexturePath,
        display: &'static str,
        inner: fn(&EntityState) -> Transform,
    ) -> RenderSpec {
        RenderSpec::with_geom(
            name,
            Geom::Item(ItemGeom {
                item,
                display,
                inner,
                cluster: None,
                hover: false,
            }),
        )
    }

    pub fn as_item_cluster(
        mut self,
        count: fn(&EntityState) -> u32,
        seed: fn(&EntityState) -> i64,
    ) -> RenderSpec {
        if let Geom::Item(geom) = &mut self.geom {
            geom.cluster = Some(ItemCluster { count, seed });
            geom.hover = true;
        }
        self
    }

    pub fn block(name: &'static str, block: fn(&EntityState) -> Option<BlockRef>) -> RenderSpec {
        RenderSpec::with_geom(name, Geom::Block(BlockGeom { block }))
    }

    pub fn built(
        name: &'static str,
        key: fn(&EntityState) -> TexturePath,
        build: fn(&EntityState) -> Mesh,
        texture: fn(&EntityState) -> Option<TexturePath>,
    ) -> RenderSpec {
        RenderSpec::with_geom(
            name,
            Geom::Built(BuiltGeom {
                key,
                build,
                texture,
            }),
        )
    }

    fn with_geom(name: &'static str, geom: Geom) -> RenderSpec {
        RenderSpec {
            name,
            geom,
            root: RootHook::Plain(default_root),
            visible: always_visible,
            blend: Blend::Cutout,
            depth_bias: 0,
            tint: no_tint,
            respects_invisibility: true,
            light: None,
        }
    }

    pub fn with_root(mut self, root: fn(&EntityState) -> RootPose) -> RenderSpec {
        self.root = RootHook::Plain(root);
        self
    }

    pub fn with_camera_root(
        mut self,
        root: fn(&EntityState, &CameraView) -> RootPose,
    ) -> RenderSpec {
        self.root = RootHook::Camera(root);
        self
    }

    pub fn with_visible(mut self, visible: fn(&EntityState) -> bool) -> RenderSpec {
        self.visible = visible;
        self
    }

    pub fn with_blend(mut self, blend: Blend) -> RenderSpec {
        self.blend = blend;
        self
    }

    pub fn with_depth_bias(mut self, depth_bias: i32) -> RenderSpec {
        self.depth_bias = depth_bias;
        self
    }

    pub fn with_tint(mut self, tint: fn(&EntityState) -> [f32; 4]) -> RenderSpec {
        self.tint = tint;
        self
    }

    pub fn with_light(mut self, light: LightMode) -> RenderSpec {
        self.light = Some(light);
        self
    }

    pub fn default_light(&self) -> LightMode {
        match (self.blend, &self.geom) {
            (Blend::Additive, _) => LightMode::Emissive,
            (_, Geom::Model(_) | Geom::Built(_)) => LightMode::Cardinal,
            (_, Geom::Item(_) | Geom::Block(_)) => LightMode::Flat,
        }
    }

    pub fn light_mode(&self) -> LightMode {
        self.light.unwrap_or_else(|| self.default_light())
    }

    pub fn effective_light_mode(&self) -> LightMode {
        match self.light_mode() {
            LightMode::Cardinal if !crate::renderer::lighting_enabled() => LightMode::Flat,
            mode => mode,
        }
    }

    pub fn ignoring_invisibility(mut self) -> RenderSpec {
        self.respects_invisibility = false;
        self
    }

    #[allow(dead_code, reason = "read by the entity registry test")]
    pub fn model(&self) -> Option<&ModelGeom> {
        match &self.geom {
            Geom::Model(model) => Some(model),
            _ => None,
        }
    }
}

pub fn no_inner(_st: &EntityState) -> Transform {
    Transform::IDENTITY
}

pub struct GpuSpec {
    pub model: BakedModel,
}

#[derive(Resource, Default)]
pub struct EntityAssets {
    specs: Vec<Option<GpuSpec>>,
    images: HashMap<String, Option<Handle<Image>>>,
    block_atlas: Option<Handle<Image>>,
    block_meshes: HashMap<BlockRef, Option<Handle<Mesh>>>,
    built_meshes: HashMap<(usize, String), Handle<Mesh>>,
    materials: HashMap<MaterialKey, SharedMaterial>,
    epoch: u64,
    lights: crate::renderer::lightmap::LightCells,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct MaterialKey {
    texture: Option<AssetId<Image>>,
    mode: u32,
    blend: Blend,
    depth_bias: i32,
    tint: [u32; 4],
    levels: (u8, u8),
    dirs: [[u32; 3]; 2],
    overlay_alpha: u32,
    overlay_white: bool,
}

impl MaterialKey {
    fn new(texture: Option<&Handle<Image>>, spec: &RenderSpec, lit: Lit, levels: (u8, u8)) -> Self {
        MaterialKey {
            texture: texture.map(Handle::id),
            mode: spec.effective_light_mode() as u32,
            blend: spec.blend,
            depth_bias: spec.depth_bias,
            tint: lit.tint.map(f32::to_bits),
            levels,
            dirs: lit.dirs.map(|d| d.to_array().map(f32::to_bits)),
            overlay_alpha: lit.overlay_alpha.to_bits(),
            overlay_white: lit.overlay_white,
        }
    }
}

struct SharedMaterial {
    handle: Handle<EntityMaterial>,
    epoch: u64,
}

const MAX_SHARED_MATERIALS: usize = 4096;

fn max_rendered_entities() -> usize {
    static MAX: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MAX.get_or_init(|| {
        std::env::var("MC_MAX_ENTITIES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(10_000)
    })
}

impl EntityAssets {
    fn spec(&mut self, idx: usize, model: &ModelGeom) -> &GpuSpec {
        if self.specs.len() <= idx {
            self.specs.resize_with(idx + 1, || None);
        }
        self.specs[idx].get_or_insert_with(|| GpuSpec {
            model: geom::bake(&(model.layer)()),
        })
    }

    fn image(&mut self, path: &str, images: &mut Assets<Image>) -> Option<Handle<Image>> {
        if let Some(cached) = self.images.get(path) {
            return cached.clone();
        }
        let handle = load_entity_image(path, images);
        if handle.is_none() {
            match path.strip_prefix(crate::entities::render::humanoid::armor::STACK_PREFIX) {
                Some(rest) => warn!("equipment stack would not compose: {rest}"),
                None => warn!("entity texture not found: textures/{path}.png"),
            }
        }
        self.images.insert(path.to_string(), handle.clone());
        handle
    }

    fn material(
        &mut self,
        texture: Option<Handle<Image>>,
        spec: &RenderSpec,
        lit: Lit,
        levels: (u8, u8),
        materials: &mut Assets<EntityMaterial>,
    ) -> Handle<EntityMaterial> {
        let key = MaterialKey::new(texture.as_ref(), spec, lit, levels);
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
        let handle = materials.add(entity_material(texture, spec, lit));
        self.materials.insert(
            key,
            SharedMaterial {
                handle: handle.clone(),
                epoch: self.epoch,
            },
        );
        handle
    }

    fn layer_material(
        &mut self,
        path: &str,
        spec: &RenderSpec,
        lit: Lit,
        levels: (u8, u8),
        images: &mut Assets<Image>,
        materials: &mut Assets<EntityMaterial>,
    ) -> Option<(Handle<EntityMaterial>, Handle<Image>)> {
        let texture = self.image(path, images)?;
        let material = self.material(Some(texture.clone()), spec, lit, levels, materials);
        Some((material, texture))
    }

    fn block_atlas(
        &mut self,
        chunk_material: &crate::renderer::systems::ChunkMaterial,
        materials: &Assets<StandardMaterial>,
    ) -> Option<Handle<Image>> {
        if self.block_atlas.is_none() {
            self.block_atlas = materials
                .get(&chunk_material.0)
                .and_then(|m| m.base_color_texture.clone());
        }
        self.block_atlas.clone()
    }

    fn block_material(
        &mut self,
        spec: &RenderSpec,
        lit: Lit,
        levels: (u8, u8),
        chunk_material: &crate::renderer::systems::ChunkMaterial,
        standard: &Assets<StandardMaterial>,
        materials: &mut Assets<EntityMaterial>,
    ) -> Option<(Handle<EntityMaterial>, Handle<Image>)> {
        let atlas = self.block_atlas(chunk_material, standard)?;
        let material = self.material(Some(atlas.clone()), spec, lit, levels, materials);
        Some((material, atlas))
    }

    fn block_mesh(&mut self, block: &BlockRef, meshes: &mut Assets<Mesh>) -> Option<Handle<Mesh>> {
        if let Some(cached) = self.block_meshes.get(block) {
            return cached.clone();
        }
        let built = blockmodel::mesh(*block).map(|mesh| meshes.add(mesh));
        self.block_meshes.insert(*block, built.clone());
        built
    }

    fn built_mesh(
        &mut self,
        idx: usize,
        key: &str,
        geom: &BuiltGeom,
        st: &EntityState,
        meshes: &mut Assets<Mesh>,
    ) -> Handle<Mesh> {
        if let Some(cached) = self.built_meshes.get(&(idx, key.to_string())) {
            return cached.clone();
        }
        let handle = meshes.add((geom.build)(st));
        self.built_meshes
            .insert((idx, key.to_string()), handle.clone());
        handle
    }
}

pub(crate) fn load_entity_image(path: &str, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let decoded = match path.strip_prefix(crate::entities::render::humanoid::armor::STACK_PREFIX) {
        Some(rest) => crate::entities::render::humanoid::armor::stack(rest)?,
        None => {
            let file = crate::assets_root().join(format!("textures/{path}.png"));
            crate::platform::assets::open_image(&file).ok()?.to_rgba8()
        }
    };
    let (w, h) = decoded.dimensions();
    let mut image = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        decoded.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    Some(images.add(image))
}

fn entity_material(texture: Option<Handle<Image>>, spec: &RenderSpec, lit: Lit) -> EntityMaterial {
    let cutoff = match spec.blend {
        Blend::Cutout | Blend::Translucent => 0.1,
        Blend::Additive => 0.0,
    };
    let alpha_mode = match spec.blend {
        Blend::Cutout => AlphaMode::Mask(cutoff),
        Blend::Translucent => AlphaMode::Blend,
        Blend::Additive => AlphaMode::Add,
    };
    EntityMaterial::new(
        texture,
        spec.effective_light_mode(),
        alpha_mode,
        cutoff,
        lit,
        spec.depth_bias,
    )
}

fn entity_levels(light: &crate::lighting::level::LightMap, st: &EntityState) -> (u8, u8) {
    if matches!(
        st.kind,
        EntityKind::FallingBlock | EntityKind::Tnt | EntityKind::BlockDisplay
    ) && st.extras.brightness_override == -1
    {
        return block_model_levels(light, st);
    }
    let (block, sky) = crate::renderer::lightmap::resolved_levels(light.at(
        st.pos[0].floor() as i32,
        (st.pos[1] + st.eye_height).floor() as i32,
        st.pos[2].floor() as i32,
    ));
    if st.extras.brightness_override != -1 {
        let packed = st.extras.brightness_override;
        return (((packed >> 4) & 15) as u8, ((packed >> 20) & 15) as u8);
    }
    (block_light_level(st, block), sky)
}

fn block_model_levels(light: &crate::lighting::level::LightMap, st: &EntityState) -> (u8, u8) {
    let x = st.pos[0].floor() as i32;
    let y = (st.pos[1] + st.bounding_box_height).floor() as i32;
    let z = st.pos[2].floor() as i32;
    let cells = [
        (x, y, z),
        (x + 1, y, z),
        (x - 1, y, z),
        (x, y + 1, z),
        (x, y - 1, z),
        (x, y, z + 1),
        (x, y, z - 1),
    ];
    crate::renderer::lightmap::resolved_levels(light.avg_at(&cells))
}

fn block_light_level(st: &EntityState, cell: u8) -> u8 {
    let base = if st.display_fire { 15 } else { cell };
    match st.kind {
        EntityKind::MagmaCube
        | EntityKind::Vex
        | EntityKind::Wither
        | EntityKind::WitherSkull
        | EntityKind::ShulkerBullet
        | EntityKind::Blaze
        | EntityKind::Allay
        | EntityKind::DragonFireball
        | EntityKind::EyeOfEnder
        | EntityKind::Fireball
        | EntityKind::SmallFireball => 15,
        EntityKind::GlowSquid => {
            let glow = glow_squid_light(st.extras.dark_ticks_remaining);
            if glow == 15 { 15 } else { glow.max(base) }
        }
        EntityKind::GlowItemFrame => base.max(5),
        EntityKind::ExperienceOrb => (base + 7).min(15),
        _ => base,
    }
}

fn glow_squid_light(dark_ticks: i32) -> u8 {
    let factor = 1.0 - dark_ticks as f32 / 10.0;
    let level = if factor < 0.0 {
        0.0
    } else if factor > 1.0 {
        15.0
    } else {
        factor * 15.0
    };
    level as u8
}

#[cfg(test)]
mod display_tests {
    use super::*;

    fn reference(display: &state::Display, outer: Quat) -> bevy::math::Mat4 {
        use bevy::math::Mat4;
        Mat4::from_quat(outer)
            * Mat4::from_translation(Vec3::from(display.translation))
            * Mat4::from_quat(display_quat(display.left_rotation))
            * Mat4::from_scale(Vec3::from(display.scale))
            * Mat4::from_quat(display_quat(display.right_rotation))
    }

    fn drawn(display: &state::Display, outer: Quat) -> bevy::math::Mat4 {
        let (transform, leftover) = display_transform(display, outer, Quat::IDENTITY);
        let mut m = transform.to_matrix();
        if let Some(right) = leftover {
            m *= bevy::math::Mat4::from_quat(right);
        }
        m
    }

    fn close(a: bevy::math::Mat4, b: bevy::math::Mat4) -> bool {
        a.to_cols_array()
            .iter()
            .zip(b.to_cols_array().iter())
            .all(|(x, y)| (x - y).abs() < 1.0e-5)
    }

    #[test]
    fn a_uniform_scale_folds_the_right_rotation_away() {
        let right = Quat::from_rotation_x(0.7);
        let display = state::Display {
            scale: [2.0; 3],
            right_rotation: right.to_array(),
            translation: [1.0, 2.0, 3.0],
            ..state::Display::default()
        };
        let outer = Quat::from_rotation_y(0.3);
        let (_, leftover) = display_transform(&display, outer, Quat::IDENTITY);
        assert!(leftover.is_none());
        assert!(close(drawn(&display, outer), reference(&display, outer)));
    }

    #[test]
    fn no_right_rotation_folds_at_any_scale() {
        let display = state::Display {
            scale: [3.0, 0.5, 1.0],
            left_rotation: Quat::from_rotation_z(0.4).to_array(),
            ..state::Display::default()
        };
        let outer = Quat::from_rotation_y(1.1);
        let (_, leftover) = display_transform(&display, outer, Quat::IDENTITY);
        assert!(leftover.is_none());
        assert!(close(drawn(&display, outer), reference(&display, outer)));
    }

    #[test]
    fn a_shear_is_left_over_and_reproduces_the_reference() {
        let display = state::Display {
            scale: [2.0, 0.5, 1.0],
            right_rotation: Quat::from_rotation_y(0.9).to_array(),
            translation: [0.0, 1.0, 0.0],
            ..state::Display::default()
        };
        let outer = Quat::from_rotation_x(0.2);
        let (_, leftover) = display_transform(&display, outer, Quat::IDENTITY);
        assert!(leftover.is_some(), "a shear must not be folded away");
        assert!(close(drawn(&display, outer), reference(&display, outer)));
        let folded = Transform {
            translation: outer * Vec3::from(display.translation),
            rotation: outer * display_quat(display.right_rotation),
            scale: Vec3::from(display.scale),
        };
        assert!(!close(folded.to_matrix(), reference(&display, outer)));
    }

    #[test]
    fn view_range_culls_only_displays() {
        let mut st = EntityState::new(0, EntityKind::BlockDisplay);
        let camera = CameraView::default();
        st.pos = [0.0, 0.0, 100.0];
        assert!(within_view_range(&st, &camera), "a pig is never culled");

        st.extras.shared_mut().display = Some(Box::new(state::Display {
            view_range: 1.0,
            ..state::Display::default()
        }));
        assert!(within_view_range(&st, &camera));
        st.pos = [0.0, 0.0, 65.0];
        assert!(!within_view_range(&st, &camera));
    }
}

#[cfg(test)]
mod light_tests {
    use super::*;

    #[test]
    fn an_unhit_glow_squid_is_full_bright() {
        assert_eq!(glow_squid_light(0), 15);
    }

    #[test]
    fn a_hit_glow_squid_stays_dark_until_the_last_ten_ticks() {
        assert_eq!(glow_squid_light(100), 0);
        assert_eq!(glow_squid_light(11), 0);
        assert_eq!(glow_squid_light(10), 0);
        assert_eq!(glow_squid_light(5), 7);
        assert_eq!(glow_squid_light(1), 13);
    }

    #[test]
    fn a_display_brightness_override_unpacks_to_its_two_levels() {
        let packed = (7i32 << 4) | (3i32 << 20);
        assert_eq!((packed >> 4) & 15, 7);
        assert_eq!((packed >> 20) & 15, 3);
    }
}

struct SingleNode {
    node: Entity,
    copies: Vec<Entity>,
    key: GeomKey,
    display: Transform,
    bounds: Option<(Vec3, Vec3)>,
    material: Option<Handle<EntityMaterial>>,
    collapsed: bool,
}

#[derive(PartialEq)]
enum GeomKey {
    Unresolved,
    Named(Box<str>),
    Block(BlockRef),
}

impl GeomKey {
    fn resolved(&self) -> bool {
        match self {
            GeomKey::Unresolved => false,
            GeomKey::Named(name) => !name.is_empty(),
            GeomKey::Block(_) => true,
        }
    }

    fn is_named(&self, name: &str) -> bool {
        matches!(self, GeomKey::Named(held) if &**held == name)
    }
}

enum RigBody {
    Baked(BakedRig),
    Single(SingleNode),
}

struct BakedRig {
    node: Entity,
    mesh: Handle<Mesh>,
    baked: Option<BakedPose>,
}

#[derive(PartialEq)]
struct BakedPose {
    states: Vec<PartState>,
    hook: Transform,
}

struct LayerRig {
    spec: usize,
    pose_root: Entity,
    extra: Entity,
    hook: Entity,
    body: RigBody,
    texture: TexturePath,
    material: Option<Handle<EntityMaterial>>,
    tex: Option<Handle<Image>>,
    lit: Lit,
    states: Vec<PartState>,
}

struct EntityRig {
    kind: EntityKind,
    layers: Vec<Option<LayerRig>>,
}

#[derive(Resource, Default)]
pub struct EntityRigs {
    live: HashMap<i32, EntityRig>,
}

impl EntityRigs {
    pub fn len(&self) -> usize {
        self.live.len()
    }

    #[cfg(feature = "shader_support")]
    pub(crate) fn roots(&self) -> impl Iterator<Item = (Entity, EntityKind)> + '_ {
        self.live.values().flat_map(|rig| {
            rig.layers
                .iter()
                .flatten()
                .map(move |layer| (layer.pose_root, rig.kind))
        })
    }
}

#[derive(Component)]
pub struct EntityRigNode;

pub struct EntityPlugin;

impl Plugin for EntityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EntityAssets>()
            .init_resource::<EntityRigs>()
            .init_resource::<crate::renderer::maps::MapTextures>()
            .insert_resource(Registry::build())
            .add_systems(
                Update,
                (crate::renderer::maps::upload_maps, sync_entities)
                    .chain()
                    .after(crate::renderer::FrameViewSystems)
                    .run_if(in_state(crate::renderer::AppState::InGame)),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_entities(
    mut commands: Commands,
    view: Res<crate::renderer::FrameView>,
    registry: Res<Registry>,
    mut assets: ResMut<EntityAssets>,
    mut rigs: ResMut<EntityRigs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut item_assets: ResMut<crate::renderer::item_assets::ItemAssets>,
    (map_textures, chunk_material, lightmap): (
        Res<crate::renderer::maps::MapTextures>,
        Res<crate::renderer::systems::ChunkMaterial>,
        Res<crate::renderer::systems::LightmapState>,
    ),
    camera_q: Query<
        &GlobalTransform,
        (
            With<crate::renderer::systems::WorldCamera>,
            Without<EntityRigNode>,
        ),
    >,
    mut nodes: Query<(&mut Transform, &mut Visibility), With<EntityRigNode>>,
    mut globals: Query<&mut GlobalTransform, With<EntityRigNode>>,
    mut mats: Query<&mut MeshMaterial3d<EntityMaterial>>,
    (mut variant_tick, mut frame, mut seen, mut idx): (
        Local<u32>,
        Local<Vec<EntityState>>,
        Local<HashMap<i32, EntityKind>>,
        Local<Vec<usize>>,
    ),
) {
    crate::prof_span!("render:sync_entities");
    *variant_tick = variant_tick.wrapping_add(1);
    let recompute_variants = *variant_tick % 4 == 0;
    let camera = camera_q
        .iter()
        .next()
        .map(|t| CameraView::new(t.rotation(), t.translation()))
        .unwrap_or_default();

    let cap = max_rendered_entities();
    let anims = &*view.entities;
    let partial = view.partial;
    let frame = &mut *frame;
    frame.clear();
    if anims.len() <= cap {
        frame.extend(anims.iter().map(|e| e.sample(partial)));
    } else {
        let eye = camera.position;
        let key = |i: usize| {
            let d = (Vec3::from(anims[i].position(partial)) - eye).length();
            ((d * 0.5) as i32, anims[i].id)
        };
        let idx = &mut *idx;
        idx.clear();
        idx.extend(0..anims.len());
        idx.select_nth_unstable_by_key(cap, |&i| key(i));
        idx.truncate(cap);
        frame.extend(idx.iter().map(|&i| anims[i].sample(partial)));
    }

    let seen = &mut *seen;
    seen.clear();
    seen.reserve(frame.len());
    for st in frame.iter() {
        seen.insert(st.id, st.kind);
    }
    rigs.live.retain(|id, rig| {
        let keep = seen.get(id) == Some(&rig.kind);
        if !keep {
            for layer in rig.layers.iter().flatten() {
                commands.entity(layer.pose_root).despawn();
            }
        }
        keep
    });

    let dirs = diffuse_lights(crate::renderer::dimension::current());
    let light = crate::client::worldsync::light_map().read();
    assets.lights.begin(&lightmap.current);
    assets.epoch = assets.epoch.wrapping_add(1);

    let mut copy_locals: Vec<Transform> = Vec::new();
    let mut posed: Vec<PosedPart> = Vec::new();

    for st in frame.iter() {
        let spec_ids = registry.specs_for(st.kind);
        if spec_ids.is_empty() {
            continue;
        }
        let in_range = within_view_range(st, &camera);
        let flash = tnt_flash_alpha(st);
        let levels = entity_levels(&light, st);
        let lit = Lit {
            tint: [1.0; 4],
            light: assets.lights.get(&lightmap.current, levels.0, levels.1),
            dirs,
            overlay_alpha: flash.unwrap_or(1.0),
            overlay_white: flash.is_some(),
        };

        let rig = rigs.live.entry(st.id).or_insert_with(|| EntityRig {
            kind: st.kind,
            layers: spec_ids.iter().map(|_| None).collect(),
        });

        for (slot, &idx) in rig.layers.iter_mut().zip(spec_ids) {
            let spec = &registry.specs[idx];
            let show =
                in_range && (!st.is_invisible || !spec.respects_invisibility) && (spec.visible)(st);
            if slot.is_none() {
                if !show {
                    continue;
                }
                *slot = Some(spawn_layer(
                    &mut commands,
                    &mut assets,
                    idx,
                    spec,
                    st,
                    lit,
                    levels,
                    &mut meshes,
                    &mut images,
                    &mut materials,
                ));
                continue;
            }
            let layer = slot.as_mut().unwrap();
            if let Ok((_, mut visibility)) = nodes.get_mut(layer.pose_root) {
                set_visibility(&mut visibility, shown(show));
            }
            if !show {
                continue;
            }

            let lit = Lit {
                tint: (spec.tint)(st),
                overlay_alpha: overlay_alpha(st),
                ..lit
            };
            let mut placement = Transform::IDENTITY;
            copy_locals.clear();
            let root = spec.root.pose(st, &camera);
            match (&spec.geom, &mut layer.body) {
                (Geom::Model(model), RigBody::Baked(baked)) => {
                    if recompute_variants {
                        let texture = (model.texture)(st);
                        if texture != layer.texture {
                            if let Some((material, tex)) = assets.layer_material(
                                &texture,
                                spec,
                                lit,
                                levels,
                                &mut images,
                                &mut materials,
                            ) {
                                if let Ok(mut slot) = mats.get_mut(baked.node) {
                                    slot.0 = material.clone();
                                } else {
                                    commands
                                        .entity(baked.node)
                                        .insert(MeshMaterial3d(material.clone()));
                                }
                                layer.material = Some(material);
                                layer.tex = Some(tex);
                                layer.lit = lit;
                                layer.texture = texture;
                            }
                        }
                    }
                    let gpu = assets.specs[layer.spec]
                        .as_ref()
                        .expect("a baked layer's spec is baked when the layer spawns");
                    gpu.model.reset_pose(&mut layer.states);
                    (model.setup)(&gpu.model, &mut layer.states, st);

                    let stale = baked
                        .baked
                        .as_ref()
                        .is_none_or(|b| b.states != layer.states || b.hook != root.hook);
                    if stale && let Some(mesh) = meshes.get_mut(&baked.mesh) {
                        let base = Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
                            * root.hook.to_matrix()
                            * Mat4::from_translation(Vec3::new(0.0, MODEL_Y_OFFSET, 0.0));
                        gpu.model.pose_parts(&layer.states, base, &mut posed);
                        let aabb = bake::layer(mesh, &gpu.model, &posed, &layer.states);
                        commands.entity(baked.node).insert(aabb);
                        let prev = baked.baked.get_or_insert_with(|| BakedPose {
                            states: Vec::new(),
                            hook: Transform::IDENTITY,
                        });
                        prev.states.clear();
                        prev.states.extend_from_slice(&layer.states);
                        prev.hook = root.hook;
                    }
                }
                (Geom::Item(geom), RigBody::Single(single)) => {
                    if recompute_variants {
                        let item = (geom.item)(st);
                        if !single.key.is_named(&item) {
                            single.key = GeomKey::Named(Box::from(&*item));
                            let built = item_assets
                                .get(
                                    &item,
                                    geom.display,
                                    false,
                                    &mut meshes,
                                    &mut images,
                                    &mut standard_materials,
                                )
                                .map(|gpu| {
                                    (
                                        gpu.mesh.clone(),
                                        gpu.texture.clone(),
                                        gpu.transform(geom.display),
                                        gpu.bounds,
                                    )
                                });
                            let built = built.map(|(mesh, texture, display, bounds)| {
                                let material = assets.material(
                                    Some(texture.clone()),
                                    spec,
                                    lit,
                                    levels,
                                    &mut materials,
                                );
                                (mesh, material, texture, display, bounds)
                            });
                            match built {
                                Some((mesh, material, texture, display, bounds)) => {
                                    single.display = item_display_transform(&display);
                                    single.bounds = Some(display_bounds(bounds, &single.display));
                                    layer.material = Some(material.clone());
                                    layer.tex = Some(texture);
                                    layer.lit = lit;
                                    insert_copies(&mut commands, &single.copies, &mesh, &material);
                                }
                                None => {
                                    single.display = Transform::IDENTITY;
                                    single.bounds = None;
                                    clear_copies(&mut commands, &single.copies);
                                }
                            }
                        }
                    }
                    placement = (geom.inner)(st);
                    if let Some((min, max)) = single.bounds {
                        if geom.hover {
                            placement.translation.y += -min.y + ITEM_MIN_HOVER_HEIGHT;
                        }
                        let (count, seed) = match &geom.cluster {
                            Some(cluster) => {
                                (rendered_amount((cluster.count)(st)), (cluster.seed)(st))
                            }
                            None => (1, 0),
                        };
                        let display = single.display;
                        cluster_offsets(count, max.z - min.z, seed, |offset| {
                            copy_locals
                                .push(Transform::from_translation(offset).mul_transform(display));
                        });
                    }
                }
                (Geom::Block(geom), RigBody::Single(single)) => {
                    if recompute_variants {
                        let block = (geom.block)(st);
                        let key = match block {
                            Some(block) => GeomKey::Block(block),
                            None => GeomKey::Unresolved,
                        };
                        if key != single.key {
                            single.key = key;
                            let built = block.as_ref().and_then(|b| {
                                let mesh = assets.block_mesh(b, &mut meshes)?;
                                let (material, atlas) = assets.block_material(
                                    spec,
                                    lit,
                                    levels,
                                    &chunk_material,
                                    &standard_materials,
                                    &mut materials,
                                )?;
                                Some((mesh, material, atlas))
                            });
                            match built {
                                Some((mesh, material, atlas)) => {
                                    layer.material = Some(material.clone());
                                    layer.tex = Some(atlas);
                                    layer.lit = lit;
                                    insert_copies(&mut commands, &single.copies, &mesh, &material);
                                }
                                None => {
                                    single.key = GeomKey::Unresolved;
                                    clear_copies(&mut commands, &single.copies);
                                }
                            }
                        }
                    }
                    if single.key.resolved() {
                        copy_locals.push(Transform::IDENTITY);
                    }
                }
                (Geom::Built(geom), RigBody::Single(single)) => {
                    if recompute_variants {
                        let texture = (geom.texture)(st);
                        let key = (geom.key)(st);
                        if !single.key.is_named(&key) {
                            single.key = GeomKey::Named(Box::from(&*key));
                            let mesh = assets.built_mesh(layer.spec, &key, geom, st, &mut meshes);
                            let image = match &texture {
                                Some(path)
                                    if path
                                        .starts_with(crate::renderer::maps::MAP_TEXTURE_PREFIX) =>
                                {
                                    map_textures.get(path)
                                }
                                Some(path) => assets.image(path, &mut images),
                                None => None,
                            };
                            if texture.is_some() && image.is_none() {
                                single.key = GeomKey::Unresolved;
                            }
                            let material = single.material.clone().unwrap_or_else(|| {
                                let handle =
                                    materials.add(entity_material(image.clone(), spec, lit));
                                single.material = Some(handle.clone());
                                layer.material = Some(handle.clone());
                                handle
                            });
                            if let Some(handle) = materials.get_mut(&material) {
                                handle.texture = image;
                            }
                            insert_copies(&mut commands, &single.copies, &mesh, &material);
                            layer.texture = texture.unwrap_or_default();
                        }
                    }
                    if single.key.resolved() {
                        copy_locals.push(Transform::IDENTITY);
                    }
                }
                _ => continue,
            }

            if lit != layer.lit {
                match &spec.geom {
                    Geom::Built(_) => {
                        if let Some(material) =
                            layer.material.as_ref().and_then(|h| materials.get_mut(h))
                        {
                            material.params.set(lit);
                        }
                    }
                    _ => {
                        if layer.material.is_some() {
                            let material = assets.material(
                                layer.tex.clone(),
                                spec,
                                lit,
                                levels,
                                &mut materials,
                            );
                            set_layer_material(layer, &material, &mut mats);
                        }
                    }
                }
                layer.lit = lit;
            }

            apply_layer(
                layer,
                &root,
                st,
                &placement,
                &copy_locals,
                &mut nodes,
                &mut globals,
            );
        }
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
        RigBody::Baked(baked) => point_at(baked.node),
        RigBody::Single(single) => {
            for &copy in &single.copies {
                point_at(copy);
            }
        }
    }
    layer.material = Some(material.clone());
}

fn insert_copies(
    commands: &mut Commands,
    copies: &[Entity],
    mesh: &Handle<Mesh>,
    material: &Handle<EntityMaterial>,
) {
    for &copy in copies {
        commands
            .entity(copy)
            .insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
    }
}

fn clear_copies(commands: &mut Commands, copies: &[Entity]) {
    for &copy in copies {
        commands
            .entity(copy)
            .remove::<Mesh3d>()
            .remove::<MeshMaterial3d<EntityMaterial>>();
    }
}

fn item_display_transform(t: &crate::items::mesh::Transform) -> Transform {
    const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;
    let rotation = Quat::from_rotation_x(t.rotation[0] * DEG_TO_RAD)
        * Quat::from_rotation_y(t.rotation[1] * DEG_TO_RAD)
        * Quat::from_rotation_z(t.rotation[2] * DEG_TO_RAD);
    let scale = Vec3::from(t.scale);
    Transform {
        translation: Vec3::from(t.translation) + rotation * (scale * -0.5),
        rotation,
        scale,
    }
}

fn spawn_chain(commands: &mut Commands) -> (Entity, Entity, Entity) {
    let pose_root = commands
        .spawn((Transform::default(), Visibility::Hidden, EntityRigNode))
        .id();
    let extra = commands
        .spawn((Transform::default(), Visibility::Inherited, EntityRigNode))
        .id();
    let hook = commands
        .spawn((Transform::default(), Visibility::Inherited, EntityRigNode))
        .id();
    commands.entity(pose_root).add_child(extra);
    (pose_root, extra, hook)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per asset store it writes"
)]
fn spawn_layer(
    commands: &mut Commands,
    assets: &mut EntityAssets,
    idx: usize,
    spec: &RenderSpec,
    st: &EntityState,
    lit: Lit,
    levels: (u8, u8),
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<EntityMaterial>,
) -> LayerRig {
    match &spec.geom {
        Geom::Model(model) => {
            let texture = (model.texture)(st);
            let lit = Lit {
                tint: (spec.tint)(st),
                overlay_alpha: overlay_alpha(st),
                ..lit
            };
            let (material, tex) =
                match assets.layer_material(&texture, spec, lit, levels, images, materials) {
                    Some((material, tex)) => (Some(material), Some(tex)),
                    None => (None, None),
                };
            let states = assets.spec(idx, model).model.rest_pose();
            spawn_model_rig(
                commands, idx, states, spec.name, texture, lit, material, tex, meshes,
            )
        }
        _ => {
            let copies = match &spec.geom {
                Geom::Item(geom) if geom.cluster.is_some() => ITEM_COPIES_MAX,
                _ => 1,
            };
            spawn_single_rig(commands, idx, spec.name, copies)
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the spawn inputs, each resolved by a different caller step"
)]
fn spawn_model_rig(
    commands: &mut Commands,
    idx: usize,
    states: Vec<PartState>,
    name: &'static str,
    texture: TexturePath,
    lit: Lit,
    material: Option<Handle<EntityMaterial>>,
    tex: Option<Handle<Image>>,
    meshes: &mut Assets<Mesh>,
) -> LayerRig {
    let mesh = meshes.add(Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ));

    let mut node = commands.spawn((
        Mesh3d(mesh.clone()),
        Transform::default(),
        Visibility::Hidden,
        EntityRigNode,
        Name::new(name),
        bake::empty_aabb(),
    ));
    #[cfg(feature = "builtin_shaders")]
    node.insert(bevy::light::NotShadowCaster);
    if let Some(material) = &material {
        node.insert(MeshMaterial3d(material.clone()));
    }
    let node = node.id();

    LayerRig {
        spec: idx,
        pose_root: node,
        extra: node,
        hook: node,
        body: RigBody::Baked(BakedRig {
            node,
            mesh,
            baked: None,
        }),
        texture,
        material,
        tex,
        lit,
        states,
    }
}

fn spawn_single_rig(
    commands: &mut Commands,
    idx: usize,
    name: &'static str,
    copy_count: usize,
) -> LayerRig {
    let collapsed = copy_count == 1;
    if collapsed {
        let mut node = commands.spawn((
            Transform::default(),
            Visibility::Hidden,
            EntityRigNode,
            Name::new(name),
        ));
        #[cfg(feature = "builtin_shaders")]
        node.insert(bevy::light::NotShadowCaster);
        let node = node.id();
        return LayerRig {
            spec: idx,
            pose_root: node,
            extra: node,
            hook: node,
            body: RigBody::Single(SingleNode {
                node,
                copies: vec![node],
                key: GeomKey::Unresolved,
                display: Transform::IDENTITY,
                bounds: None,
                material: None,
                collapsed: true,
            }),
            texture: TexturePath::Borrowed(""),
            material: None,
            tex: None,
            lit: Lit::full_bright(),
            states: Vec::new(),
        };
    }

    let (pose_root, extra, hook) = spawn_chain(commands);
    let node = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            EntityRigNode,
            Name::new(name),
        ))
        .id();
    commands.entity(extra).add_child(hook);
    commands.entity(hook).add_child(node);

    let copies: Vec<Entity> = (0..copy_count)
        .map(|_| {
            let mut copy =
                commands.spawn((Transform::default(), Visibility::Hidden, EntityRigNode));
            #[cfg(feature = "builtin_shaders")]
            copy.insert(bevy::light::NotShadowCaster);
            let copy = copy.id();
            commands.entity(node).add_child(copy);
            copy
        })
        .collect();

    LayerRig {
        spec: idx,
        pose_root,
        extra,
        hook,
        body: RigBody::Single(SingleNode {
            node,
            copies,
            collapsed: false,
            key: GeomKey::Unresolved,
            display: Transform::IDENTITY,
            bounds: None,
            material: None,
        }),
        texture: TexturePath::Borrowed(""),
        material: None,
        tex: None,
        lit: Lit::full_bright(),
        states: Vec::new(),
    }
}

#[inline]
fn set_transform(slot: &mut Mut<'_, Transform>, want: Transform) {
    if **slot != want {
        **slot = want;
    }
}

fn display_leftover(st: &EntityState) -> Option<Quat> {
    if st.kind != EntityKind::BlockDisplay {
        return None;
    }
    let display = st.extras.display.as_deref()?;
    (!display.no_right_rotation() && !display.uniform_scale())
        .then(|| display_quat(display.right_rotation))
}

fn write_sheared(
    globals: &mut Query<&mut GlobalTransform, With<EntityRigNode>>,
    node: Entity,
    composed: Transform,
    right: Quat,
) {
    let Ok(mut global) = globals.get_mut(node) else {
        return;
    };
    let want = GlobalTransform::from(composed).mul_transform(Transform::from_rotation(right));
    if *global != want {
        *global = want;
    }
}

fn set_visibility(slot: &mut Mut<'_, Visibility>, want: Visibility) {
    if **slot != want {
        **slot = want;
    }
}

fn shown(show: bool) -> Visibility {
    if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

fn apply_layer(
    layer: &LayerRig,
    root: &RootPose,
    st: &EntityState,
    placement: &Transform,
    copy_locals: &[Transform],
    nodes: &mut Query<(&mut Transform, &mut Visibility), With<EntityRigNode>>,
    globals: &mut Query<&mut GlobalTransform, With<EntityRigNode>>,
) {
    if let RigBody::Baked(baked) = &layer.body {
        if let Ok((mut t, _)) = nodes.get_mut(baked.node) {
            let composed = Transform {
                translation: Vec3::from(st.pos) + root.world_offset,
                rotation: root.rotation,
                scale: Vec3::splat(root.scale),
            }
            .mul_transform(Transform {
                translation: root.extra_offset,
                rotation: root.extra_rotation,
                scale: Vec3::ONE,
            });
            set_transform(&mut t, composed);
        }
        return;
    }

    if let RigBody::Single(single) = &layer.body
        && single.collapsed
    {
        let Ok((mut t, mut visibility)) = nodes.get_mut(single.node) else {
            return;
        };
        let want = match copy_locals.first() {
            Some(local) => {
                let composed = Transform {
                    translation: Vec3::from(st.pos) + root.world_offset,
                    rotation: root.rotation,
                    scale: Vec3::splat(root.scale),
                }
                .mul_transform(Transform {
                    translation: root.extra_offset,
                    rotation: root.extra_rotation,
                    scale: Vec3::ONE,
                })
                .mul_transform(root.hook)
                .mul_transform(*placement)
                .mul_transform(*local);
                match display_leftover(st) {
                    None => set_transform(&mut t, composed),
                    Some(right) => write_sheared(globals, single.node, composed, right),
                }
                Visibility::Inherited
            }
            None => Visibility::Hidden,
        };
        set_visibility(&mut visibility, want);
        return;
    }

    if let Ok((mut t, _)) = nodes.get_mut(layer.pose_root) {
        let scale = Vec3::splat(root.scale);
        set_transform(
            &mut t,
            Transform {
                translation: Vec3::from(st.pos) + root.world_offset,
                rotation: root.rotation,
                scale,
            },
        );
    }
    if let Ok((mut t, _)) = nodes.get_mut(layer.extra) {
        let scale = t.scale;
        set_transform(
            &mut t,
            Transform {
                translation: root.extra_offset,
                rotation: root.extra_rotation,
                scale,
            },
        );
    }
    if let Ok((mut t, _)) = nodes.get_mut(layer.hook) {
        set_transform(&mut t, root.hook);
    }

    match &layer.body {
        RigBody::Baked(_) => {}
        RigBody::Single(single) => {
            if let Ok((mut t, _)) = nodes.get_mut(single.node) {
                set_transform(&mut t, *placement);
            }
            for (i, &copy) in single.copies.iter().enumerate() {
                let Ok((mut t, mut visibility)) = nodes.get_mut(copy) else {
                    continue;
                };
                let want = match copy_locals.get(i) {
                    Some(local) => {
                        set_transform(&mut t, *local);
                        Visibility::Inherited
                    }
                    None => Visibility::Hidden,
                };
                set_visibility(&mut visibility, want);
            }
        }
    }
}
