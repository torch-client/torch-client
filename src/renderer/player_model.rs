use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::anim::{AnimSample, ArmPose, Armor, HeldItem};
use super::entity_material::{ALPHA_CUTOUT, EntityMaterial, LightMode, Lit, diffuse_lights};
use super::item_assets::{ItemAssets, ItemUse, Owner};
use crate::entities::geom::{self, PartState};
use crate::entities::models::humanoid::armor as armor_mesh;
use crate::entities::models::humanoid::humanoid;
use crate::entities::render::held;
use crate::entities::render::humanoid::armor as armor_tex;
use crate::items::mesh::Transform as ItemTransform;
use crate::items::model::DisplayContext;

use crate::util::mth::{DEG_TO_RAD, lerp};

const FLIP_DEGREES: f32 = 90.0;

pub(crate) const AVATAR_SCALE: f32 = 0.937_5;

pub(crate) const MODEL_Y_OFFSET: f32 = -1.501;

const MAX_CROSSBOW_CHARGE_DURATION: f32 = crate::items::kinetic::CROSSBOW_CHARGE_TICKS;

fn part_at(pivot: [f32; 3]) -> PartState {
    PartState::from(geom::PartPose::offset(pivot[0], pivot[1], pivot[2]))
}

pub(crate) fn model_transform(part: &PartState) -> Transform {
    let mut t = part.transform();
    t.translation *= MODEL_UNITS_PER_BLOCK;
    t
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub head: PartState,
    pub body: PartState,
    pub right_arm: PartState,
    pub left_arm: PartState,
    pub right_leg: PartState,
    pub left_leg: PartState,
    #[cfg(feature = "skins")]
    pub cape: Quat,
    pub wings: [PartState; 2],
}

impl Pose {
    pub fn parts(&self) -> [PartState; 6] {
        [
            self.head,
            self.body,
            self.right_arm,
            self.left_arm,
            self.right_leg,
            self.left_leg,
        ]
    }
}

pub fn pose_humanoid(s: &AnimSample) -> Pose {
    let parts = player_parts(false);
    let mut l = humanoid::Limbs::from_parts(std::array::from_fn(|i| part_at(parts[i].pivot)));
    let poses = humanoid::ArmPoses {
        right: s.right_arm_pose,
        left: s.left_arm_pose,
    };
    let input = humanoid::HumanoidIn {
        x_rot: s.pitch,
        y_rot: s.head_yaw,
        walk_pos: s.walk_pos,
        walk_speed: s.walk_speed,
        speed_value: s.speed_value,
        passenger: s.passenger,
        left_handed: s.main_arm_left,
        using_item: s.using_item,
        use_offhand: s.using_offhand,
        poses,
        arm: humanoid::ArmInputs {
            ticks_using_item: s.ticks_using_item,
            max_crossbow_charge: MAX_CROSSBOW_CHARGE_DURATION,
            crouching: s.crouching,
            fall_flying: s.fall_flying,
            swim_amount: s.swim_amount,
            charging: {
                let (right, left) = if s.main_arm_left {
                    (&s.off_hand, &s.main_hand)
                } else {
                    (&s.main_hand, &s.off_hand)
                };
                crate::renderer::anim::sway_charging(s.using_item, s.using_offhand, right, left)
            },
        },
        age_ticks: s.age_ticks,
        attack_time: s.attack_time,
        swing_left: s.attack_left,
    };
    humanoid::pose_core(&mut l, &input, |l| {
        humanoid::attack_animation(l, s.attack_time, s.attack_left, 1.0, poses)
    });

    Pose {
        head: l.head,
        body: l.body,
        right_arm: l.right_arm,
        left_arm: l.left_arm,
        right_leg: l.right_leg,
        left_leg: l.left_leg,
        #[cfg(feature = "skins")]
        cape: cape_rotation(s.cape),
        wings: wing_parts(s),
    }
}

fn wing_parts(s: &AnimSample) -> [PartState; 2] {
    use crate::entities::models::humanoid::elytra;

    let (left, right) = elytra::wing_angles(s.elytra, s.crouching);
    let part = |x: f32, [y, x_rot, y_rot, z_rot]: [f32; 4]| PartState {
        x,
        y,
        z: 2.0,
        x_rot,
        y_rot,
        z_rot,
        ..PartState::default()
    };
    [part(5.0, left), part(-5.0, right)]
}

#[cfg(feature = "skins")]
fn cape_rotation(cape: [f32; 3]) -> Quat {
    let [flap, lean, lean2] = cape;
    Quat::from_rotation_y(PI)
        * Quat::from_rotation_y(-PI)
        * Quat::from_rotation_x((6.0 + lean / 2.0 + flap) * DEG_TO_RAD)
        * Quat::from_rotation_z(lean2 / 2.0 * DEG_TO_RAD)
        * Quat::from_rotation_y((180.0 - lean2 / 2.0) * DEG_TO_RAD)
}

pub struct AvatarRoot {
    pub rotation: Quat,
    pub model_offset: Vec3,
    pub world_offset: Vec3,
}

pub fn root_pose(s: &AnimSample, scale: f32) -> AvatarRoot {
    let living = crate::entities::living_rotations(
        &crate::entities::RotationState {
            body_rot: s.body_yaw,
            is_shaking: false,
            age_ticks: s.age_ticks,
            death_time: s.death_time,
            is_auto_spin_attack: s.spin_attack,
            x_rot: s.pitch,
            sleeping: s.sleeping,
            bed_orientation: s.bed_orientation,
            eye_height: crate::session::player_dimensions(azalea::entity::Pose::Standing, scale)
                .eye_height,
            is_upside_down: false,
            bounding_box_height: 0.0,
            scale: 1.0,
        },
        FLIP_DEGREES,
    );
    let mut rot = living.rotation * living.extra_rotation;
    let mut offset = Vec3::ZERO;

    if s.fall_flying {
        if !s.spin_attack {
            rot *= Quat::from_rotation_x((s.fall_flying_scale * (-90.0 - s.pitch)).to_radians());
        }
        if let Some(y_rot) = s.flying_y_rot {
            rot *= Quat::from_rotation_y(y_rot);
        }
    } else if s.swim_amount > 0.0 {
        rot *= Quat::from_rotation_x(lerp(s.swim_amount, 0.0, -90.0 - s.pitch).to_radians());
        if s.visually_swimming {
            offset = Vec3::new(0.0, -1.0, 0.3);
        }
    }

    AvatarRoot {
        rotation: rot,
        model_offset: offset,
        world_offset: living.world_offset,
    }
}

const MODEL_UNITS_PER_BLOCK: f32 = 16.0;

fn held_item_transform(t: &ItemTransform, left_hand: bool, pose: Mat4) -> Transform {
    let hand_rot = hand_rotation();
    let hand_off = hand_offset(left_hand, false);
    let t = handed(t, left_hand);
    let disp_rot = Quat::from_rotation_x(t.rotation[0] * DEG_TO_RAD)
        * Quat::from_rotation_y(t.rotation[1] * DEG_TO_RAD)
        * Quat::from_rotation_z(t.rotation[2] * DEG_TO_RAD);
    let disp_off = Vec3::from(t.translation) * MODEL_UNITS_PER_BLOCK;
    let scale = Vec3::from(t.scale);
    let centre = disp_rot * (scale * -0.5) * MODEL_UNITS_PER_BLOCK;

    if pose == Mat4::IDENTITY {
        return Transform {
            translation: hand_rot * (hand_off + disp_off + centre),
            rotation: hand_rot * disp_rot,
            scale: scale * MODEL_UNITS_PER_BLOCK,
        };
    }
    let pose = Mat4::from_scale(Vec3::splat(MODEL_UNITS_PER_BLOCK))
        * pose
        * Mat4::from_scale(Vec3::splat(1.0 / MODEL_UNITS_PER_BLOCK));
    Transform::from_matrix(
        Mat4::from_quat(hand_rot)
            * Mat4::from_translation(hand_off)
            * pose
            * Mat4::from_scale_rotation_translation(
                scale * MODEL_UNITS_PER_BLOCK,
                disp_rot,
                disp_off + centre,
            ),
    )
}

pub(crate) fn hand_rotation() -> Quat {
    Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_y(PI)
}

pub(crate) fn hand_offset(left_hand: bool, baby: bool) -> Vec3 {
    let (x, y, z) = if baby {
        (0.0, 1.0, -4.5)
    } else {
        (1.0, 2.0, -10.0)
    };
    Vec3::new(if left_hand { -x } else { x }, y, z)
}

pub(crate) fn handed(t: &ItemTransform, left_hand: bool) -> ItemTransform {
    if !left_hand {
        return *t;
    }
    ItemTransform {
        rotation: [t.rotation[0], -t.rotation[1], -t.rotation[2]],
        translation: [-t.translation[0], t.translation[1], t.translation[2]],
        scale: t.scale,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SpearState {
    pub arm_poses: [ArmPose; 2],
    pub charging: [Option<crate::items::kinetic::Kinetic>; 2],
    pub attack_time: f32,
    pub attack_left: bool,
    pub main_kinetic: Option<crate::items::kinetic::Kinetic>,
    pub ticks_using_item: f32,
    pub ticks_since_kinetic_hit: f32,
}

impl SpearState {
    fn of(s: &AnimSample) -> SpearState {
        SpearState {
            arm_poses: [s.right_arm_pose, s.left_arm_pose],
            charging: s.charging(),
            attack_time: s.attack_time,
            attack_left: s.attack_left,
            main_kinetic: s.main_hand.kinetic,
            ticks_using_item: s.ticks_using_item,
            ticks_since_kinetic_hit: s.ticks_since_kinetic_hit,
        }
    }
}

pub(crate) fn spear_item_pose(s: &SpearState, right: bool) -> Mat4 {
    use crate::items::kinetic::hit_feedback_amount;
    use crate::util::mth::ease::{in_out_expo, in_quad, out_back};
    use crate::util::mth::progress;

    let pose = s.arm_poses[usize::from(!right)];
    let charging = s.charging[usize::from(!right)];
    let stabbing = s.attack_time > 0.0 && s.attack_left != right && pose == ArmPose::Spear;
    if !stabbing && charging.is_none() {
        return Mat4::IDENTITY;
    }

    let attack = in_quad(progress(s.attack_time, 0.05, 0.2));
    let retract = in_out_expo(progress(s.attack_time, 0.4, 1.0));
    let around = |q: Quat, pivot: Vec3| crate::entities::rotate_around(q, pivot).to_matrix();
    let mut m = Mat4::IDENTITY;

    if stabbing {
        let jet = s.main_kinetic.map_or(0.0, |k| k.forward_movement);
        m *= around(
            Quat::from_rotation_x(-70.0 * (attack - retract) * DEG_TO_RAD),
            Vec3::new(0.0, -0.125, 0.125),
        );
        m *= Mat4::from_translation(Vec3::new(0.0, jet * (attack - retract), 0.0));
    }

    if let Some(kinetic) = charging
        && pose == ArmPose::Spear
        && s.ticks_using_item != 0.0
    {
        let p = kinetic.use_params(s.ticks_using_item);
        let invert = if right { 1.0 } else { -1.0 };
        let raise_modified = 1.0 - out_back(1.0 - p.raise);
        let hit = hit_feedback_amount(s.ticks_since_kinetic_hit);
        m *= Mat4::from_translation(Vec3::new(
            0.0,
            -hit * 0.4,
            -kinetic.forward_movement * (raise_modified - p.raise_back) + hit,
        ));
        m *= around(
            Quat::from_rotation_x(
                -(70.0 * (p.raise - p.raise_back) - 40.0 * (attack - retract)) * DEG_TO_RAD,
            ),
            Vec3::new(0.0, -0.031_25, 0.125),
        );
        m *= around(
            Quat::from_rotation_y(
                invert * 90.0 * (p.raise - p.sway + 3.0 * retract + attack) * DEG_TO_RAD,
            ),
            Vec3::new(0.0, 0.0, 0.125),
        );
    }
    m
}

#[derive(Clone, Copy, PartialEq)]
enum ItemSlot {
    Hand { left: bool },
    Head,
}

impl ItemSlot {
    fn context(self) -> DisplayContext {
        match self {
            ItemSlot::Hand { left } => held_context(left),
            ItemSlot::Head => DisplayContext::Head,
        }
    }

    fn transform(self, display: &ItemTransform, pose: Mat4, skull: bool) -> Transform {
        match self {
            ItemSlot::Hand { left } => held_item_transform(display, left, pose),
            ItemSlot::Head => {
                let local = if skull {
                    crate::entities::SKULL_ITEM_UNDO
                } else {
                    crate::entities::item_display_transform(display)
                };
                Transform::from_matrix(
                    Mat4::from_scale(Vec3::splat(MODEL_UNITS_PER_BLOCK))
                        * crate::entities::head_item_pose(
                            crate::entities::HeadTransforms::DEFAULT,
                            skull,
                        )
                        * local.to_matrix(),
                )
            }
        }
    }
}

fn held_context(left_hand: bool) -> DisplayContext {
    if left_hand {
        DisplayContext::ThirdPersonLeftHand
    } else {
        DisplayContext::ThirdPersonRightHand
    }
}

#[cfg(feature = "skins")]
fn head_skin(item: &HeldItem, items: &mut ItemCtx) -> Option<Handle<Image>> {
    let skin = item.profile_skin.as_ref()?;
    items
        .skins
        .profile_texture(skin, items.entity_images, items.images)
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PartDef {
    name: &'static str,
    pub(crate) tex: [f32; 2],
    pub(crate) origin: [f32; 3],
    pub(crate) size: [f32; 3],
    pub(crate) grow: f32,
    pub(crate) pivot: [f32; 3],
    parent: Option<usize>,
}

const PART_TREE: [(&str, Option<usize>); 12] = [
    ("head", None),
    ("body", None),
    ("right_arm", None),
    ("left_arm", None),
    ("right_leg", None),
    ("left_leg", None),
    ("hat", Some(0)),
    ("jacket", Some(1)),
    ("right_sleeve", Some(2)),
    ("left_sleeve", Some(3)),
    ("right_pants", Some(4)),
    ("left_pants", Some(5)),
];

pub(crate) fn player_parts(slim: bool) -> &'static [PartDef; 12] {
    static PARTS: std::sync::OnceLock<[[PartDef; 12]; 2]> = std::sync::OnceLock::new();
    &PARTS.get_or_init(|| [flatten_player_mesh(false), flatten_player_mesh(true)])
        [usize::from(slim)]
}

fn flatten_player_mesh(slim: bool) -> [PartDef; 12] {
    let mesh = humanoid::player_mesh(geom::Grow::NONE, slim);
    PART_TREE.map(|(name, parent)| {
        let node = mesh
            .find(name)
            .unwrap_or_else(|| panic!("player_mesh has no {name}"));
        let [cube] = node.cubes() else {
            panic!("player_mesh's {name} is not one box")
        };
        debug_assert!(!cube.mirror && cube.grow.0 == cube.grow.1 && cube.grow.0 == cube.grow.2);
        let pose = node.pose();
        PartDef {
            name,
            tex: cube.tex,
            origin: cube.origin,
            size: cube.size,
            grow: cube.grow.0,
            pivot: [pose.x, pose.y, pose.z],
            parent,
        }
    })
}

#[cfg(feature = "skins")]
const CAPE: PartDef = PartDef {
    name: "cape",
    tex: [0.0, 0.0],
    origin: [-5.0, 0.0, -1.0],
    size: [10.0, 16.0, 1.0],
    grow: 0.0,
    pivot: [0.0, 0.0, 2.0],
    parent: Some(1),
};

fn box_mesh(def: &PartDef) -> Mesh {
    box_mesh_on(super::skin::SKIN_SHEET, def)
}

fn box_mesh_on(sheet: [f32; 2], def: &PartDef) -> Mesh {
    super::skin::box_mesh(
        sheet,
        def.tex,
        def.origin,
        def.size,
        def.grow,
        1.0,
        Some([1.0; 4]),
    )
}

#[cfg(feature = "skins")]
fn slim_meshes(wide: &[Handle<Mesh>], meshes: &mut Assets<Mesh>) -> Vec<Handle<Mesh>> {
    let mut slim = wide.to_vec();
    for (i, (def, wide_def)) in player_parts(true)
        .iter()
        .zip(player_parts(false))
        .enumerate()
    {
        if def != wide_def {
            slim[i] = meshes.add(box_mesh(def));
        }
    }
    slim
}

pub(crate) const ARMOR_LAYERS: [fn() -> geom::LayerDef; 4] = [
    armor_mesh::humanoid_helmet,
    armor_mesh::humanoid_chestplate,
    armor_mesh::humanoid_leggings,
    armor_mesh::humanoid_boots,
];

pub(crate) const ARMOR_PARTS: [(&str, usize); 7] = [
    ("head", 0),
    ("hat", 0),
    ("body", 1),
    ("right_arm", 2),
    ("left_arm", 3),
    ("right_leg", 4),
    ("left_leg", 5),
];

fn bake_wings(meshes: &mut Assets<Mesh>) -> [Handle<Mesh>; 2] {
    let baked = geom::bake(&crate::entities::models::humanoid::elytra::layer());
    ["left_wing", "right_wing"].map(|name| {
        let mesh = part_in_model_units(&baked, name).unwrap_or_else(|| {
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
        });
        meshes.add(mesh)
    })
}

fn part_in_model_units(baked: &geom::BakedModel, name: &str) -> Option<Mesh> {
    let mut mesh = baked
        .find(name)
        .and_then(|id| baked.parts[id].mesh.clone())?;
    mesh.asset_usage = RenderAssetUsages::default();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for p in positions.iter_mut() {
            *p = p.map(|c| c * 16.0);
        }
    }
    Some(mesh)
}

const LEGGINGS_SLOT: usize = 2;

const CHEST_SLOT: usize = 1;

const ARMOR_SLOT_NAMES: [&str; 4] = ["helmet", "chestplate", "leggings", "boots"];

struct ArmorMesh {
    part: usize,
    mesh: Handle<Mesh>,
}

fn bake_armor(layer: geom::LayerDef, meshes: &mut Assets<Mesh>) -> Vec<ArmorMesh> {
    let baked = geom::bake(&layer);
    let mut out = Vec::new();
    for (name, part) in ARMOR_PARTS {
        let Some(mesh) = part_in_model_units(&baked, name) else {
            continue;
        };
        out.push(ArmorMesh {
            part,
            mesh: meshes.add(mesh),
        });
    }
    out
}

pub(crate) fn armor_texture(slot: usize, item: &str) -> Option<String> {
    if slot == LEGGINGS_SLOT {
        armor_tex::has_layer(Some(item), armor_tex::LAYER_HUMANOID_LEGGINGS)
            .then(|| armor_tex::leggings_layer_texture(Some(item)))
    } else {
        armor_tex::has_layer(Some(item), armor_tex::LAYER_HUMANOID)
            .then(|| armor_tex::humanoid_texture(Some(item)))
    }
}

#[derive(Resource)]
struct PlayerModelAssets {
    meshes: Vec<Handle<Mesh>>,
    #[cfg(feature = "skins")]
    slim: Vec<Handle<Mesh>>,
    #[cfg(feature = "skins")]
    cape: Handle<Mesh>,
    wings: [Handle<Mesh>; 2],
    skin: Handle<Image>,
    no_item: Handle<Mesh>,
    armor: [Vec<ArmorMesh>; 4],
    skulls: [geom::BakedModel; 2],
}

const WORN_SKULLS: [&str; 2] = ["dragon", "piglin"];

#[derive(Component)]
struct RigRoot;

#[derive(Component)]
struct RigFlip;

#[derive(Component)]
struct RigPart;

#[derive(Component)]
struct RigArmor;

#[derive(Component)]
struct HeldItemNode {
    slot: ItemSlot,
    shown: HeldItem,
    #[cfg(feature = "skins")]
    skin_generation: u32,
    branch: u32,
    display: Option<ItemTransform>,
    pose: Mat4,
}

const PART_HEAD: usize = 0;
#[cfg(feature = "skins")]
const PART_BODY: usize = 1;
pub(crate) const PART_RIGHT_ARM: usize = 2;
pub(crate) const PART_LEFT_ARM: usize = 3;
pub(crate) const PART_RIGHT_SLEEVE: usize = 8;
pub(crate) const PART_LEFT_SLEEVE: usize = 9;
#[cfg(feature = "skins")]
const FIRST_OVERLAY: usize = 6;

struct ArmorSlotRig {
    boxes: Vec<Entity>,
    material: Handle<EntityMaterial>,
    worn: (Option<&'static str>, bool),
}

struct HeldRig {
    node: Entity,
    material: Handle<EntityMaterial>,
}

struct SkullRig {
    node: Entity,
    mesh: Handle<Mesh>,
    material: Handle<EntityMaterial>,
    shown: Option<(&'static str, f32)>,
    states: Vec<PartState>,
    posed: Vec<geom::PosedPart>,
}

#[cfg(feature = "skins")]
struct CapeRig {
    node: Entity,
    material: Handle<EntityMaterial>,
    ready: bool,
    offset: Vec3,
}

struct WingsRig {
    nodes: [Entity; 2],
    material: Handle<EntityMaterial>,
    worn: Option<(Option<&'static str>, bool, u32)>,
}

struct Rig {
    id: Option<i32>,
    root: Entity,
    flip: Entity,
    parts: [Entity; 6],
    #[cfg(feature = "skins")]
    overlays: [Entity; 6],
    held: [HeldRig; 2],
    head_item: HeldRig,
    skull: SkullRig,
    base: Handle<EntityMaterial>,
    overlay: Handle<EntityMaterial>,
    armor: [ArmorSlotRig; 4],
    lit: Lit,
    body_hidden: bool,
    glow: u32,
    #[cfg(feature = "skins")]
    cape: CapeRig,
    wings: WingsRig,
    #[cfg(feature = "skins")]
    worn: crate::client::skins::SkinState,
    #[cfg(feature = "skins")]
    seen_textures: u32,
}

#[derive(Resource, Default)]
pub(crate) struct PlayerRigs {
    entries: Vec<Rig>,
    local: Option<Rig>,
    scratch: SlotScratch,
}

#[derive(Default)]
struct SlotScratch {
    slot_of: HashMap<i32, usize>,
    taken: Vec<bool>,
    assign: Vec<Option<usize>>,
    unmatched: Vec<usize>,
    free: Vec<usize>,
}

impl PlayerRigs {
    #[cfg(feature = "shader_support")]
    pub(crate) fn local_root(&self) -> Option<Entity> {
        self.local.as_ref().map(|rig| rig.root)
    }
}

const POOL_SLACK: usize = 4;

pub struct PlayerModelPlugin;

impl Plugin for PlayerModelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerRigs>()
            .add_systems(
                Update,
                setup_player_model.run_if(resource_added::<crate::renderer::systems::AssetsReady>),
            )
            .add_systems(
                Update,
                sync_player_models
                    .after(crate::renderer::frame_view::FrameViewSystems)
                    .run_if(in_state(crate::renderer::AppState::InGame)),
            );

        #[cfg(feature = "skins")]
        app.init_resource::<super::skin::SkinTextures>()
            .add_systems(Update, super::skin::upload_skins);

        #[cfg(feature = "builtin_shaders")]
        app.add_systems(Update, sync_player_shadows);
    }
}

#[cfg(feature = "builtin_shaders")]
#[allow(clippy::type_complexity)]
fn sync_player_shadows(
    mut commands: Commands,
    gui: Res<crate::gui::GuiState>,
    nodes: Query<
        (Entity, Has<bevy::light::NotShadowCaster>),
        Or<(With<RigPart>, With<RigArmor>, With<HeldItemNode>)>,
    >,
) {
    let cast = gui.options.player_shadows;
    for (entity, blocked) in &nodes {
        if cast && blocked {
            commands
                .entity(entity)
                .remove::<bevy::light::NotShadowCaster>();
        } else if !cast && !blocked {
            commands.entity(entity).insert(bevy::light::NotShadowCaster);
        }
    }
}

fn setup_player_model(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<EntityMaterial>>,
    mut rigs: ResMut<PlayerRigs>,
) {
    let Some(skin) = super::skin::load_default_skin(&mut images, "player model") else {
        return;
    };

    let mut empty = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    empty.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    empty.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    empty.insert_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new());
    empty.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    empty.insert_indices(Indices::U32(Vec::new()));

    let wide: Vec<Handle<Mesh>> = player_parts(false)
        .iter()
        .map(|def| meshes.add(box_mesh(def)))
        .collect();
    let assets = PlayerModelAssets {
        #[cfg(feature = "skins")]
        slim: slim_meshes(&wide, &mut meshes),
        #[cfg(feature = "skins")]
        cape: meshes.add(box_mesh_on(super::skin::CAPE_SHEET, &CAPE)),
        wings: bake_wings(&mut meshes),
        meshes: wide,
        skin,
        no_item: meshes.add(empty),
        armor: ARMOR_LAYERS.map(|layer| bake_armor(layer(), &mut meshes)),
        skulls: WORN_SKULLS
            .map(|kind| geom::bake(&crate::blockentities::render::skull::layer_for_type(kind))),
    };

    rigs.local = Some(spawn_rig(
        &mut commands,
        &assets,
        &mut materials,
        &mut meshes,
    ));
    commands.insert_resource(assets);
}

fn spawn_rig(
    commands: &mut Commands,
    assets: &PlayerModelAssets,
    materials: &mut Assets<EntityMaterial>,
    meshes: &mut Assets<Mesh>,
) -> Rig {
    let lit = Lit::full_bright();
    let skin_material = |materials: &mut Assets<EntityMaterial>| {
        materials.add(EntityMaterial::cutout(
            Some(assets.skin.clone()),
            LightMode::Cardinal,
            lit,
        ))
    };
    let base = skin_material(materials);
    let overlay = skin_material(materials);
    let held_materials =
        [(); 3].map(|_| materials.add(EntityMaterial::cutout(None, LightMode::Flat, lit)));

    let root = commands
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            RigRoot,
            Name::new("player_model"),
        ))
        .id();

    let s = AVATAR_SCALE / 16.0;
    let flip = commands
        .spawn((
            Transform {
                translation: Vec3::new(0.0, -AVATAR_SCALE * MODEL_Y_OFFSET, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::new(-s, -s, s),
            },
            Visibility::Inherited,
            RigFlip,
            Name::new("model_root"),
        ))
        .id();
    commands.entity(root).add_child(flip);

    let parts = player_parts(false);
    let mut entities = Vec::with_capacity(parts.len());
    for (i, def) in parts.iter().enumerate() {
        let entity = commands
            .spawn((
                Mesh3d(assets.meshes[i].clone()),
                MeshMaterial3d(if def.parent.is_some() {
                    overlay.clone()
                } else {
                    base.clone()
                }),
                Transform::from_translation(Vec3::from(def.pivot)),
                Visibility::Inherited,
                RigPart,
                Name::new(def.name),
            ))
            .id();
        let parent = match def.parent {
            Some(p) => entities[p],
            None => flip,
        };
        commands.entity(parent).add_child(entity);
        entities.push(entity);
    }

    let mut item_node =
        |part: usize, slot: ItemSlot, name: &'static str, material: &Handle<EntityMaterial>| {
            let entity = commands
                .spawn((
                    Mesh3d(assets.no_item.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    HeldItemNode {
                        slot,
                        shown: HeldItem::default(),
                        #[cfg(feature = "skins")]
                        skin_generation: 0,
                        branch: 0,
                        display: None,
                        pose: Mat4::IDENTITY,
                    },
                    Name::new(name),
                ))
                .id();
            commands.entity(entities[part]).add_child(entity);
            HeldRig {
                node: entity,
                material: material.clone(),
            }
        };
    let [right_material, left_material, head_material] = &held_materials;
    let held = [
        item_node(
            PART_RIGHT_ARM,
            ItemSlot::Hand { left: false },
            "right_hand_item",
            right_material,
        ),
        item_node(
            PART_LEFT_ARM,
            ItemSlot::Hand { left: true },
            "left_hand_item",
            left_material,
        ),
    ];
    let head_item = item_node(PART_HEAD, ItemSlot::Head, "head_item", head_material);

    let skull_mesh = meshes.add(Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ));
    let skull_material = materials.add(EntityMaterial::cutout(None, LightMode::Cardinal, lit));
    let skull_node = commands
        .spawn((
            Mesh3d(skull_mesh.clone()),
            MeshMaterial3d(skull_material.clone()),
            Transform::default(),
            Visibility::Hidden,
            bevy::camera::primitives::Aabb::from_min_max(Vec3::splat(-32.0), Vec3::splat(32.0)),
            RigPart,
            Name::new("head_skull"),
        ))
        .id();
    commands.entity(entities[PART_HEAD]).add_child(skull_node);
    let skull = SkullRig {
        node: skull_node,
        mesh: skull_mesh,
        material: skull_material,
        shown: None,
        states: Vec::new(),
        posed: Vec::new(),
    };

    #[cfg(feature = "skins")]
    let cape_material = materials.add(EntityMaterial::cutout(None, LightMode::Cardinal, lit));
    #[cfg(feature = "skins")]
    let cape = {
        let entity = commands
            .spawn((
                Mesh3d(assets.cape.clone()),
                MeshMaterial3d(cape_material.clone()),
                Transform::from_translation(Vec3::from(CAPE.pivot)),
                Visibility::Hidden,
                RigPart,
                Name::new(CAPE.name),
            ))
            .id();
        commands.entity(entities[PART_BODY]).add_child(entity);
        CapeRig {
            node: entity,
            material: cape_material,
            ready: false,
            offset: Vec3::ZERO,
        }
    };

    let wings_material = materials.add(EntityMaterial::cutout(None, LightMode::Cardinal, lit));
    let wings = [0usize, 1].map(|i| {
        let entity = commands
            .spawn((
                Mesh3d(assets.wings[i].clone()),
                MeshMaterial3d(wings_material.clone()),
                Transform::default(),
                Visibility::Hidden,
                RigPart,
                Name::new(if i == 0 { "left_wing" } else { "right_wing" }),
            ))
            .id();
        commands.entity(flip).add_child(entity);
        entity
    });

    let armor_materials =
        [(); 4].map(|_| materials.add(EntityMaterial::cutout(None, LightMode::Cardinal, lit)));
    let armor = std::array::from_fn(|slot| ArmorSlotRig {
        boxes: assets.armor[slot]
            .iter()
            .map(|piece| {
                let entity = commands
                    .spawn((
                        Mesh3d(piece.mesh.clone()),
                        MeshMaterial3d(armor_materials[slot].clone()),
                        Transform::default(),
                        Visibility::Hidden,
                        RigArmor,
                        Name::new(ARMOR_SLOT_NAMES[slot]),
                    ))
                    .id();
                commands.entity(entities[piece.part]).add_child(entity);
                entity
            })
            .collect(),
        material: armor_materials[slot].clone(),
        worn: (None, false),
    });

    Rig {
        id: None,
        root,
        flip,
        parts: [
            entities[0],
            entities[1],
            entities[2],
            entities[3],
            entities[4],
            entities[5],
        ],
        #[cfg(feature = "skins")]
        overlays: [
            entities[6],
            entities[7],
            entities[8],
            entities[9],
            entities[10],
            entities[11],
        ],
        held,
        head_item,
        skull,
        base,
        overlay,
        armor,
        lit,
        body_hidden: false,
        glow: 0,
        #[cfg(feature = "skins")]
        cape,
        wings: WingsRig {
            nodes: wings,
            material: wings_material,
            worn: None,
        },
        #[cfg(feature = "skins")]
        worn: crate::client::skins::SkinState {
            parts: u8::MAX,
            default_index: (crate::gui::tablist::DEFAULT_SKINS.len() / 2) as u8,
            ..Default::default()
        },
        #[cfg(feature = "skins")]
        seen_textures: 0,
    }
}

type RootQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Visibility),
    (With<RigRoot>, Without<HeldItemNode>),
>;

type FlipQuery<'w, 's> =
    Query<'w, 's, &'static mut Transform, (With<RigFlip>, Without<RigRoot>, Without<HeldItemNode>)>;

type PartQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Transform,
    (
        With<RigPart>,
        Without<RigRoot>,
        Without<RigFlip>,
        Without<HeldItemNode>,
    ),
>;

type HeldQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Visibility,
        &'static mut Mesh3d,
        &'static mut HeldItemNode,
    ),
    (Without<RigRoot>, Without<RigFlip>, Without<RigPart>),
>;

type ArmorQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (With<RigArmor>, Without<RigRoot>, Without<HeldItemNode>),
>;

type RigQueries<'w, 's> = (
    RootQuery<'w, 's>,
    FlipQuery<'w, 's>,
    PartQuery<'w, 's>,
    HeldQuery<'w, 's>,
    ArmorQuery<'w, 's>,
    LayerQuery<'w, 's>,
);

type LayerQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Visibility, &'static mut Mesh3d),
    (
        With<RigPart>,
        Without<RigArmor>,
        Without<RigRoot>,
        Without<HeldItemNode>,
    ),
>;

struct ItemCtx<'a> {
    assets: &'a mut ItemAssets,
    meshes: &'a mut Assets<Mesh>,
    images: &'a mut Assets<Image>,
    entity_materials: &'a mut Assets<EntityMaterial>,
    no_item: Handle<Mesh>,
    skulls: &'a [geom::BakedModel; 2],
    entity_images: &'a mut crate::entities::EntityImages,
    armor_glint: Option<Handle<Image>>,
    #[cfg(feature = "skins")]
    skins: &'a mut super::skin::SkinTextures,
    #[cfg(feature = "skins")]
    model: &'a PlayerModelAssets,
}

#[derive(Clone, Default)]
struct WornSkin {
    #[cfg(feature = "skins")]
    state: crate::client::skins::SkinState,
}

fn sync_player_models(
    shared: Res<crate::renderer::systems::Shared>,
    view: Res<crate::renderer::frame_view::FrameView>,
    camera_view: Res<crate::renderer::input::CameraView>,
    assets: Option<Res<PlayerModelAssets>>,
    mut rigs: ResMut<PlayerRigs>,
    mut commands: Commands,
    mut queries: RigQueries,
    mut item_assets: ResMut<ItemAssets>,
    mut item_meshes: ResMut<Assets<Mesh>>,
    mut item_images: ResMut<Assets<Image>>,
    mut entity_materials: ResMut<Assets<EntityMaterial>>,
    mut entity_images: ResMut<crate::entities::EntityImages>,
    #[cfg(feature = "skins")] mut skin_textures: ResMut<super::skin::SkinTextures>,
    lightmap: Res<crate::renderer::systems::LightmapState>,
    origin: Res<crate::renderer::RenderOrigin>,
    armor_glint: Option<Res<super::glint::ArmorGlintTexture>>,
) {
    crate::prof_span!("render:sync_player_models");
    let Some(assets) = assets else { return };
    let mut items = ItemCtx {
        assets: &mut item_assets,
        meshes: &mut item_meshes,
        images: &mut item_images,
        entity_materials: &mut entity_materials,
        no_item: assets.no_item.clone(),
        skulls: &assets.skulls,
        entity_images: &mut entity_images,
        armor_glint: armor_glint.map(|glint| glint.0.clone()),
        #[cfg(feature = "skins")]
        skins: &mut skin_textures,
        #[cfg(feature = "skins")]
        model: &assets,
    };
    let dirs = diffuse_lights(crate::renderer::dimension::current());

    let partial = view.partial;
    let players: Vec<(i32, [f64; 3], AnimSample, WornSkin, Body)> = {
        let s = shared.0.lock().unwrap();
        s.session
            .other_players
            .iter()
            .map(|p| {
                (
                    p.id,
                    p.anim.position(partial),
                    p.anim.sample(partial),
                    WornSkin {
                        #[cfg(feature = "skins")]
                        state: p.skin.clone(),
                    },
                    Body::of(&p.body, view.gamemode),
                )
            })
            .collect()
    };
    let mut local_pose = view.local.clone();
    let live_head = crate::renderer::anim::wrap_degrees(-view.camera_yaw - 180.0);
    local_pose.head_yaw = crate::renderer::anim::wrap_degrees(live_head - local_pose.body_yaw);
    local_pose.pitch = -view.camera_pitch;
    let local_pos = view.player_lerp;
    let local_skin = WornSkin {
        #[cfg(feature = "skins")]
        state: view.local_skin.clone(),
    };

    let mut scratch = std::mem::take(&mut rigs.scratch);
    let SlotScratch {
        slot_of,
        taken,
        assign,
        unmatched,
        free,
    } = &mut scratch;
    slot_of.clear();
    for (i, rig) in rigs.entries.iter().enumerate() {
        if let Some(id) = rig.id {
            slot_of.insert(id, i);
        }
    }
    taken.clear();
    taken.resize(rigs.entries.len(), false);
    assign.clear();
    unmatched.clear();
    for (k, (id, _, _, _, _)) in players.iter().enumerate() {
        match slot_of.get(id) {
            Some(&i) if !taken[i] => {
                taken[i] = true;
                assign.push(Some(i));
            }
            _ => {
                unmatched.push(k);
                assign.push(None);
            }
        }
    }

    free.clear();
    free.extend((0..taken.len()).filter(|i| !taken[*i]));
    for &k in unmatched.iter() {
        let slot = match free.pop() {
            Some(i) => i,
            None => {
                rigs.entries.push(spawn_rig(
                    &mut commands,
                    &assets,
                    items.entity_materials,
                    items.meshes,
                ));
                taken.push(false);
                rigs.entries.len() - 1
            }
        };
        taken[slot] = true;
        assign[k] = Some(slot);
    }

    let light = crate::client::worldsync::light_map().read();
    for ((id, feet, sample, worn, body), slot) in players.iter().zip(assign.iter()) {
        let Some(slot) = *slot else { continue };
        rigs.entries[slot].id = Some(*id);
        let lit = player_lit(&lightmap.current, &light, *feet, sample, body.scale, dirs);
        let owner = Owner {
            pos: *feet,
            yaw: sample.body_yaw,
            local: false,
            seed: *id,
        };
        apply_rig(
            &mut rigs.entries[slot],
            origin.to_render_array(*feet),
            sample,
            owner,
            worn,
            *body,
            RigVisibility::Shown,
            lit,
            &mut queries,
            &mut items,
        );
        let rig = &mut rigs.entries[slot];
        super::outline::set_glowing(&mut commands, rig.root, &mut rig.glow, body.outline);
    }

    for i in 0..rigs.entries.len() {
        if taken[i] {
            continue;
        }
        rigs.entries[i].id = None;
        let rig = &mut rigs.entries[i];
        super::outline::set_glowing(&mut commands, rig.root, &mut rig.glow, 0);
        if let Ok((_, mut vis)) = queries.0.get_mut(rigs.entries[i].root) {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
    rigs.scratch = scratch;

    #[cfg(feature = "shader_support")]
    let pose_hidden = crate::renderer::packvertex::player_shadow();
    #[cfg(not(feature = "shader_support"))]
    let pose_hidden = false;
    if let Some(local) = &mut rigs.local {
        let lit = player_lit(
            &lightmap.current,
            &light,
            local_pos,
            &local_pose,
            view.local_body.scale,
            dirs,
        );
        let shown = if camera_view.detached() {
            RigVisibility::Shown
        } else if pose_hidden {
            RigVisibility::HiddenPosed
        } else {
            RigVisibility::Hidden
        };
        let body = Body::of(&view.local_body, view.gamemode);
        let owner = items.assets.local_owner().unwrap_or_default();
        apply_rig(
            local,
            origin.to_render_array(local_pos),
            &local_pose,
            owner,
            &local_skin,
            body,
            shown,
            lit,
            &mut queries,
            &mut items,
        );
        super::outline::set_glowing(&mut commands, local.root, &mut local.glow, body.outline);
    }
    drop(light);

    let mut surplus = rigs
        .entries
        .len()
        .saturating_sub(players.len() + POOL_SLACK);
    if surplus > 0 {
        let mut doomed: Vec<Entity> = Vec::new();
        rigs.entries.retain(|rig| {
            if rig.id.is_none() && surplus > 0 {
                surplus -= 1;
                doomed.push(rig.root);
                false
            } else {
                true
            }
        });
        for root in doomed {
            commands.entity(root).despawn();
        }
    }
}

fn update_held_item(
    entity: Entity,
    item: &HeldItem,
    use_: ItemUse,
    pose: Mat4,
    material: &Handle<EntityMaterial>,
    items: &mut ItemCtx,
    query: &mut Query<
        (
            &mut Transform,
            &mut Visibility,
            &mut Mesh3d,
            &mut HeldItemNode,
        ),
        (Without<RigRoot>, Without<RigFlip>, Without<RigPart>),
    >,
) {
    let Ok((mut transform, mut vis, mut mesh, mut node)) = query.get_mut(entity) else {
        return;
    };
    let skull = node.slot == ItemSlot::Head && crate::entities::is_skull(item.id);
    let context = node.slot.context();
    let slot = context.name();
    let branch = items.assets.branch(item.id, slot, use_);
    if node.branch == branch
        && node.shown.same_model(item)
        && node.shown.profile_skin == item.profile_skin
    {
        if node.pose != pose {
            node.pose = pose;
            if let Some(display) = node.display {
                *transform = node.slot.transform(&display, pose, skull);
            }
        }
        #[cfg(feature = "skins")]
        if node.display.is_some()
            && node.shown.profile_skin.is_some()
            && node.skin_generation != items.skins.generation()
        {
            node.skin_generation = items.skins.generation();
            if let Some(skin) = head_skin(&node.shown, items)
                && let Some(material) = items.entity_materials.get_mut(material)
                && material.texture.as_ref() != Some(&skin)
            {
                material.texture = Some(skin);
            }
        }
        return;
    }
    node.shown = item.clone();
    node.branch = branch;
    node.pose = pose;
    let key = item.model_key();
    let gpu = items
        .assets
        .get(&key, slot, use_, items.meshes, items.images)
        .cloned();
    match gpu {
        Some(gpu) => {
            let display = gpu.for_context(context);
            node.display = Some(display);
            *transform = node.slot.transform(&display, pose, skull);
            mesh.0 = gpu.mesh;
            #[cfg(feature = "skins")]
            let texture = {
                node.skin_generation = items.skins.generation();
                head_skin(item, items).unwrap_or(gpu.texture)
            };
            #[cfg(not(feature = "skins"))]
            let texture = gpu.texture;
            if let Some(material) = items.entity_materials.get_mut(material) {
                material.texture = Some(texture);
            }
            *vis = Visibility::Inherited;
        }
        None => {
            node.display = None;
            mesh.0 = items.no_item.clone();
            *vis = Visibility::Hidden;
        }
    }
}

fn update_wings(
    rig: &mut Rig,
    armor: &Armor,
    items: &mut ItemCtx,
    layers: &mut LayerQuery<'_, '_>,
) {
    let chest = armor.items[CHEST_SLOT];
    let glint = armor.foil[CHEST_SLOT] && items.armor_glint.is_some();
    #[cfg(feature = "skins")]
    let generation = items.skins.generation();
    #[cfg(not(feature = "skins"))]
    let generation = 0;
    if rig.wings.worn == Some((chest, glint, generation)) {
        return;
    }
    rig.wings.worn = Some((chest, glint, generation));

    let worn = crate::entities::render::humanoid::armor::has_layer(
        chest,
        crate::entities::render::humanoid::armor::LAYER_WINGS,
    );
    let texture = worn.then(|| {
        #[cfg(feature = "skins")]
        {
            let own = rig
                .worn
                .refs
                .elytra
                .as_ref()
                .or(if rig.worn.parts & (1 << CAPE_BIT) != 0 {
                    rig.worn.refs.cape.as_ref()
                } else {
                    None
                })
                .and_then(|url| items.skins.get(url).cloned());
            if own.is_some() {
                return own;
            }
        }
        let path = crate::entities::render::humanoid::armor::slot_texture(
            chest,
            crate::entities::render::humanoid::armor::LAYER_WINGS,
        );
        items.entity_images.get(&path, items.images)
    });
    let texture = texture.flatten();

    if let Some(material) = items.entity_materials.get_mut(&rig.wings.material) {
        material.texture = texture.clone();
        material.set_glint(super::glint::armor_scale(glint), items.armor_glint.clone());
    }
    let shown = if texture.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for entity in rig.wings.nodes {
        if let Ok((mut vis, _)) = layers.get_mut(entity) {
            *vis = shown;
        }
    }

    #[cfg(feature = "skins")]
    {
        use crate::entities::render::humanoid::armor;

        let over_chestplate = armor::has_layer(chest, armor::LAYER_HUMANOID);
        rig.cape.offset = if over_chestplate {
            Vec3::new(0.0, -0.053_125 * 16.0, 0.068_75 * 16.0)
        } else {
            Vec3::ZERO
        };
        if let Ok((mut vis, _)) = layers.get_mut(rig.cape.node) {
            *vis = if rig.cape.ready && !worn {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub(crate) const OVERLAY_BITS: [u8; 6] = [6, 1, 3, 2, 5, 4];

#[cfg(feature = "skins")]
const CAPE_BIT: u8 = 0;

#[cfg(feature = "skins")]
fn update_skin(
    rig: &mut Rig,
    skin: &crate::client::skins::SkinState,
    items: &mut ItemCtx,
    layers: &mut LayerQuery<'_, '_>,
) {
    let generation = items.skins.generation();
    if rig.worn == *skin && rig.seen_textures == generation {
        return;
    }
    let slim_changed = rig.worn.slim() != skin.slim();
    rig.worn = skin.clone();
    rig.seen_textures = generation;

    let downloaded = skin
        .refs
        .body
        .as_ref()
        .and_then(|url| items.skins.get(url).cloned());
    let body = match downloaded {
        Some(handle) => Some(handle),
        None => items.skins.default_skin(skin.default_index, items.images),
    }
    .unwrap_or_else(|| items.model.skin.clone());
    for handle in [&rig.base, &rig.overlay] {
        if let Some(material) = items.entity_materials.get_mut(handle) {
            material.texture = Some(body.clone());
        }
    }

    let cape = skin
        .refs
        .cape
        .as_ref()
        .and_then(|url| items.skins.get(url).cloned());
    if let Some(material) = items.entity_materials.get_mut(&rig.cape.material) {
        material.texture = cape.clone();
    }
    rig.cape.ready = cape.is_some() && skin.parts & (1 << CAPE_BIT) != 0;
    rig.wings.worn = None;

    for (entity, bit) in rig.overlays.iter().zip(OVERLAY_BITS) {
        if let Ok((mut vis, _)) = layers.get_mut(*entity) {
            *vis = if skin.parts & (1 << bit) != 0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }

    if !slim_changed {
        return;
    }
    let set = if skin.slim() {
        &items.model.slim
    } else {
        &items.model.meshes
    };
    for (entity, index) in [
        (rig.parts[PART_RIGHT_ARM], PART_RIGHT_ARM),
        (rig.parts[PART_LEFT_ARM], PART_LEFT_ARM),
        (
            rig.overlays[PART_RIGHT_SLEEVE - FIRST_OVERLAY],
            PART_RIGHT_SLEEVE,
        ),
        (
            rig.overlays[PART_LEFT_SLEEVE - FIRST_OVERLAY],
            PART_LEFT_SLEEVE,
        ),
    ] {
        if let Ok((_, mut mesh)) = layers.get_mut(entity) {
            mesh.0 = set[index].clone();
        }
    }
}

pub(crate) fn player_eye(feet: [f64; 3], sample: &AnimSample, scale: f32) -> [f64; 3] {
    let eye_height = player_eye_height(sample, scale);
    [feet[0], feet[1] + f64::from(eye_height), feet[2]]
}

pub(crate) fn player_eye_height(sample: &AnimSample, scale: f32) -> f32 {
    use azalea::entity::Pose;
    let pose = if sample.sleeping {
        Pose::Sleeping
    } else if sample.fall_flying {
        Pose::FallFlying
    } else if sample.visually_swimming {
        Pose::Swimming
    } else if sample.spin_attack {
        Pose::SpinAttack
    } else if sample.crouching {
        Pose::Crouching
    } else {
        Pose::Standing
    };
    crate::session::player_dimensions(pose, scale).eye_height
}

fn player_lit(
    lm: &crate::renderer::lightmap::State,
    light: &crate::lighting::level::LightMap,
    feet: [f64; 3],
    sample: &AnimSample,
    scale: f32,
    dirs: [Vec3; 2],
) -> Lit {
    let eye = player_eye(feet, sample, scale);
    Lit {
        tint: [1.0; 4],
        light: crate::renderer::lightmap::entity_light_in(lm, light, eye, false),
        dirs,
        overlay_alpha: crate::entities::hurt_overlay_alpha(sample.has_red_overlay),
        overlay_white: false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RigVisibility {
    Shown,
    Hidden,
    HiddenPosed,
}

#[derive(Clone, Copy)]
struct Body {
    hidden: bool,
    scale: f32,
    outline: u32,
}

impl Body {
    fn of(body: &crate::session::PlayerBody, viewer: crate::session::Gamemode) -> Body {
        Body {
            hidden: body.hidden_to(viewer),
            scale: body.scale,
            outline: body.outline,
        }
    }
}

const BODY_HIDDEN_CUTOFF: f32 = 2.0;

fn hide_body(rig: &mut Rig, hidden: bool, materials: &mut Assets<EntityMaterial>) {
    if rig.body_hidden == hidden {
        return;
    }
    rig.body_hidden = hidden;
    let cutoff = if hidden {
        BODY_HIDDEN_CUTOFF
    } else {
        ALPHA_CUTOUT
    };
    #[cfg(feature = "skins")]
    let cape = Some(&rig.cape.material);
    #[cfg(not(feature = "skins"))]
    let cape = None;
    for handle in [&rig.base, &rig.overlay].into_iter().chain(cape) {
        if let Some(material) = materials.get_mut(handle) {
            material.params.light.w = cutoff;
        }
    }
}

#[cfg_attr(not(feature = "skins"), allow(unused_variables))]
fn apply_rig(
    rig: &mut Rig,
    feet: Vec3,
    sample: &AnimSample,
    owner: Owner,
    worn: &WornSkin,
    body: Body,
    shown: RigVisibility,
    lit: Lit,
    queries: &mut RigQueries,
    items: &mut ItemCtx,
) {
    let root = root_pose(sample, body.scale);
    let Ok((mut transform, mut vis)) = queries.0.get_mut(rig.root) else {
        return;
    };
    let translation = feet + root.world_offset;
    let scale = Vec3::splat(body.scale);
    if transform.translation != translation
        || transform.rotation != root.rotation
        || transform.scale != scale
    {
        transform.translation = translation;
        transform.rotation = root.rotation;
        transform.scale = scale;
    }
    vis.set_if_neq(if shown == RigVisibility::Shown {
        Visibility::Visible
    } else {
        Visibility::Hidden
    });
    if shown == RigVisibility::Hidden {
        return;
    }
    hide_body(rig, body.hidden, items.entity_materials);
    if let Ok(mut transform) = queries.1.get_mut(rig.flip) {
        transform.translation =
            root.model_offset + Vec3::new(0.0, -AVATAR_SCALE * MODEL_Y_OFFSET, 0.0);
    }
    let pose = pose_humanoid(sample);
    for (entity, part) in rig.parts.iter().zip(pose.parts().iter()) {
        if let Ok(mut transform) = queries.2.get_mut(*entity) {
            *transform = model_transform(part);
        }
    }

    for (entity, part) in rig.wings.nodes.iter().zip(pose.wings.iter()) {
        if let Ok(mut transform) = queries.2.get_mut(*entity) {
            *transform = model_transform(part);
        }
    }

    #[cfg(feature = "skins")]
    {
        update_skin(rig, &worn.state, items, &mut queries.5);
        if worn.state.refs.cape.is_some() {
            if let Ok(mut transform) = queries.2.get_mut(rig.cape.node) {
                transform.rotation = pose.cape;
                transform.translation = Vec3::from(CAPE.pivot) + rig.cape.offset;
            }
        }
    }

    let (right_item, left_item) = if sample.main_arm_left {
        (&sample.off_hand, &sample.main_hand)
    } else {
        (&sample.main_hand, &sample.off_hand)
    };
    let using_left = sample.using_offhand != sample.main_arm_left;
    let use_of = |item: &HeldItem, left: bool| ItemUse {
        owner: Some(owner),
        ..ItemUse::of(
            item,
            sample.using_item && using_left == left,
            sample.ticks_using_item,
        )
    };
    let [right_held, left_held] = &rig.held;
    update_held_item(
        right_held.node,
        right_item,
        use_of(right_item, false),
        spear_item_pose(&SpearState::of(sample), true),
        &right_held.material,
        items,
        &mut queries.3,
    );
    update_held_item(
        left_held.node,
        left_item,
        use_of(left_item, true),
        spear_item_pose(&SpearState::of(sample), false),
        &left_held.material,
        items,
        &mut queries.3,
    );

    let empty = HeldItem::default();
    let head = if held::head_item_drawn(sample.head.id) {
        &sample.head
    } else {
        &empty
    };
    update_held_item(
        rig.head_item.node,
        head,
        ItemUse {
            owner: Some(owner),
            ..ItemUse::of(head, false, 0.0)
        },
        Mat4::IDENTITY,
        &rig.head_item.material,
        items,
        &mut queries.3,
    );
    update_worn_skull(
        &mut rig.skull,
        sample.head.id,
        sample.walk_pos,
        items,
        &mut queries.5,
    );

    update_armor(rig, &sample.armor, items, &mut queries.4);
    update_wings(rig, &sample.armor, items, &mut queries.5);

    if lit != rig.lit {
        #[cfg(feature = "skins")]
        let cape = Some(&rig.cape.material);
        #[cfg(not(feature = "skins"))]
        let cape = None;
        for handle in [&rig.base, &rig.overlay, &rig.wings.material]
            .into_iter()
            .chain(cape)
            .chain(rig.held.iter().map(|held| &held.material))
            .chain([&rig.head_item.material, &rig.skull.material])
            .chain(rig.armor.iter().map(|slot| &slot.material))
        {
            if let Some(material) = items.entity_materials.get_mut(handle) {
                material.params.set(lit);
            }
        }
        rig.lit = lit;
    }
}

fn update_armor(
    rig: &mut Rig,
    armor: &Armor,
    items: &mut ItemCtx,
    visibility: &mut Query<
        &mut Visibility,
        (With<RigArmor>, Without<RigRoot>, Without<HeldItemNode>),
    >,
) {
    for (index, slot) in rig.armor.iter_mut().enumerate() {
        let item = armor.items[index];
        let glint = armor.foil[index] && items.armor_glint.is_some();
        if slot.worn == (item, glint) {
            continue;
        }
        slot.worn = (item, glint);

        let texture = match item.and_then(|item| armor_texture(index, item)) {
            Some(path) => items.entity_images.get(&path, items.images),
            None => None,
        };
        if let Some(material) = items.entity_materials.get_mut(&slot.material) {
            material.texture = texture.clone();
            material.set_glint(super::glint::armor_scale(glint), items.armor_glint.clone());
        }
        let shown = if texture.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        for entity in &slot.boxes {
            if let Ok(mut vis) = visibility.get_mut(*entity) {
                *vis = shown;
            }
        }
    }
}

fn update_worn_skull(
    skull: &mut SkullRig,
    head: &str,
    animation: f32,
    items: &mut ItemCtx,
    layers: &mut LayerQuery<'_, '_>,
) {
    use crate::blockentities::render::skull as skulls;

    let kind = held::animated_skull(head).then(|| skulls::skull_type(head));
    let index = kind.and_then(|k| WORN_SKULLS.iter().position(|w| *w == k));
    let Some((kind, index)) = kind.zip(index) else {
        if skull.shown.take().is_some()
            && let Ok((mut vis, _)) = layers.get_mut(skull.node)
        {
            *vis = Visibility::Hidden;
        }
        return;
    };
    let was = skull.shown.replace((kind, animation));
    if was == Some((kind, animation)) {
        return;
    }
    if was.is_none_or(|(k, _)| k != kind) {
        let texture = items
            .entity_images
            .get(skulls::texture_for_type(kind), items.images);
        if let Some(material) = items.entity_materials.get_mut(&skull.material) {
            material.texture = texture;
        }
        if let Ok((mut vis, _)) = layers.get_mut(skull.node) {
            *vis = Visibility::Inherited;
        }
    }
    let model = &items.skulls[index];
    skull.states.clear();
    skull
        .states
        .extend(model.parts.iter().map(|p| PartState::from(p.initial)));
    skulls::pose_for_type(kind, model, &mut skull.states, animation);
    let base = Mat4::from_scale(Vec3::splat(MODEL_UNITS_PER_BLOCK))
        * crate::entities::head_item_pose(crate::entities::HeadTransforms::DEFAULT, true);
    model.pose_parts(&skull.states, base, &mut skull.posed);
    if let Some(mesh) = items.meshes.get_mut(&skull.mesh) {
        crate::entities::bake::layer(mesh, model, &skull.posed, &skull.states);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::models::humanoid::humanoid::{
        HORIZONTAL_SHIELD_MOVEMENT_LIMIT, LOWEST_SHIELD_BLOCKING_ANGLE,
    };
    use crate::renderer::skin::{SKIN_H, SKIN_W};

    #[test]
    fn body_front_face_lands_on_the_torso_texels() {
        let body = &player_parts(false)[1];
        assert_eq!(body.name, "body");
        let mesh = box_mesh(body);
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() {
            bevy::mesh::VertexAttributeValues::Float32x2(v) => v.clone(),
            other => panic!("unexpected UV storage: {other:?}"),
        };
        let north = &uvs[12..16];
        let px = |uv: &[f32; 2]| (uv[0] * SKIN_W, uv[1] * SKIN_H);
        assert_eq!(px(&north[0]), (28.0, 20.0));
        assert_eq!(px(&north[1]), (20.0, 20.0));
        assert_eq!(px(&north[2]), (20.0, 32.0));
        assert_eq!(px(&north[3]), (28.0, 32.0));
    }

    #[test]
    fn head_uvs_cover_the_first_thirty_two_columns() {
        let mesh = box_mesh(&player_parts(false)[0]);
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() {
            bevy::mesh::VertexAttributeValues::Float32x2(v) => v.clone(),
            other => panic!("unexpected UV storage: {other:?}"),
        };
        let (mut min_u, mut max_u, mut max_v) = (f32::MAX, 0.0f32, 0.0f32);
        for uv in &uvs {
            min_u = min_u.min(uv[0] * SKIN_W);
            max_u = max_u.max(uv[0] * SKIN_W);
            max_v = max_v.max(uv[1] * SKIN_H);
        }
        assert_eq!(min_u, 0.0);
        assert_eq!(max_u, 32.0);
        assert_eq!(max_v, 16.0);
    }

    #[test]
    fn player_parts_match_create_mesh() {
        let wide = player_parts(false);
        let slim = player_parts(true);
        let pivots: Vec<_> = wide[..6].iter().map(|d| d.pivot).collect();
        assert_eq!(
            pivots,
            [
                [0.0; 3],
                [0.0; 3],
                [-5.0, 2.0, 0.0],
                [5.0, 2.0, 0.0],
                [-1.9, 12.0, 0.0],
                [1.9, 12.0, 0.0]
            ]
        );
        assert_eq!(wide[6].grow, 0.5);
        assert!(
            wide[7..]
                .iter()
                .all(|d| d.grow == 0.25 && d.pivot == [0.0; 3])
        );
        assert_eq!(wide[PART_RIGHT_ARM].origin, [-3.0, -2.0, -2.0]);
        assert_eq!(slim[PART_RIGHT_ARM].origin, [-2.0, -2.0, -2.0]);
        assert_eq!(slim[PART_LEFT_SLEEVE].origin, [-1.0, -2.0, -2.0]);
        assert_eq!(slim[PART_RIGHT_SLEEVE].size, [3.0, 12.0, 4.0]);
        let differ: Vec<_> = (0..12).filter(|&i| wide[i] != slim[i]).collect();
        assert_eq!(
            differ,
            [
                PART_RIGHT_ARM,
                PART_LEFT_ARM,
                PART_RIGHT_SLEEVE,
                PART_LEFT_SLEEVE
            ]
        );
        assert_eq!(wide[PART_LEFT_ARM].tex, [32.0, 48.0]);
        assert_eq!(wide[PART_LEFT_SLEEVE].tex, [48.0, 48.0]);
    }

    #[test]
    fn resting_pose_keeps_the_createmesh_pivots() {
        let s = AnimSample {
            age_ticks: 40.0,
            ..Default::default()
        };
        let p = pose_humanoid(&s);
        assert_eq!((p.head.x, p.head.y, p.head.z), (0.0, 0.0, 0.0));
        assert_eq!((p.right_arm.x, p.right_arm.y), (-5.0, 2.0));
        assert_eq!((p.left_arm.x, p.left_arm.y), (5.0, 2.0));
        assert_eq!((p.right_leg.x, p.right_leg.y), (-1.9, 12.0));
        assert_eq!((p.left_leg.x, p.left_leg.y), (1.9, 12.0));
        assert_eq!(p.body, part_at([0.0, 0.0, 0.0]));
        assert!((p.right_arm.z_rot + p.left_arm.z_rot).abs() < 1e-6);
        assert!(p.right_arm.z_rot.abs() > 1e-3);
    }

    #[test]
    fn crouching_leans_the_body_and_shifts_the_head() {
        let s = AnimSample {
            crouching: true,
            ..Default::default()
        };
        let p = pose_humanoid(&s);
        assert_eq!(p.body.x_rot, 0.5);
        assert_eq!(p.head.y, 4.2);
        assert_eq!(p.body.y, 3.2);
        assert_eq!(p.right_arm.y, 2.0 + 3.2);
        assert_eq!(p.right_leg.z, 4.0);
        assert_eq!(p.left_leg.z, 4.0);
        assert!((p.left_arm.x_rot - 0.4).abs() < 0.1);
    }

    #[test]
    fn attack_swing_twists_the_body_and_moves_the_shoulder() {
        let s = AnimSample {
            attack_time: 0.5,
            ..Default::default()
        };
        let p = pose_humanoid(&s);
        assert!(p.body.y_rot.abs() > 1e-3, "{}", p.body.y_rot);
        assert!(
            p.right_arm.x > -5.0 && p.right_arm.x < -4.9,
            "{}",
            p.right_arm.x
        );
        assert!(p.right_arm.z.abs() > 1e-3);
        assert!(p.right_arm.x_rot < -0.5, "{}", p.right_arm.x_rot);
        assert!(p.left_arm.x_rot.abs() < 0.3);
    }

    #[test]
    fn walk_cycle_swings_limbs_in_antiphase() {
        let s = AnimSample {
            walk_pos: 3.0,
            walk_speed: 1.0,
            ..Default::default()
        };
        let p = pose_humanoid(&s);
        assert!(p.right_leg.x_rot * p.left_leg.x_rot < 0.0);
        assert!(p.right_leg.x_rot * p.right_arm.x_rot < 0.0);
        assert!(p.right_leg.x_rot.abs() > p.left_arm.x_rot.abs());
    }

    #[cfg(feature = "skins")]
    #[test]
    fn cape_hangs_six_degrees_off_the_back_at_rest() {
        let rest = cape_rotation([0.0, 0.0, 0.0]);
        let expect = Quat::from_rotation_x(6.0 * DEG_TO_RAD) * Quat::from_rotation_y(PI);
        assert!(
            rest.abs_diff_eq(expect, 1e-5),
            "cape at rest is {rest:?}, expected {expect:?}"
        );

        let moving = cape_rotation([32.0, 150.0, 20.0]);
        let expect = Quat::from_rotation_x((6.0 + 75.0 + 32.0) * DEG_TO_RAD)
            * Quat::from_rotation_z(10.0 * DEG_TO_RAD)
            * Quat::from_rotation_y(170.0 * DEG_TO_RAD);
        assert!(
            moving.abs_diff_eq(expect, 1e-5),
            "cape in motion is {moving:?}, expected {expect:?}"
        );
    }

    #[test]
    fn raised_shield_clamps_the_arm_at_the_blocking_window() {
        let s = AnimSample {
            pitch: 89.0,
            head_yaw: 80.0,
            using_item: true,
            right_arm_pose: ArmPose::Block,
            ..Default::default()
        };
        let p = pose_humanoid(&s);
        let expect_x = -0.942_477_9 + LOWEST_SHIELD_BLOCKING_ANGLE;
        assert!(
            (p.right_arm.x_rot - expect_x).abs() < 1e-6,
            "{}",
            p.right_arm.x_rot
        );
        let expect_y = -30.0 * DEG_TO_RAD + HORIZONTAL_SHIELD_MOVEMENT_LIMIT;
        assert!(
            (p.right_arm.y_rot - expect_y).abs() < 1e-6,
            "{}",
            p.right_arm.y_rot
        );
        assert_eq!(p.left_arm.y_rot, 0.0);
    }

    #[test]
    fn drawn_bow_poses_both_arms_and_keeps_the_offhand_yaw() {
        let s = AnimSample {
            using_item: true,
            right_arm_pose: ArmPose::BowAndArrow,
            ..Default::default()
        };
        let p = pose_humanoid(&s);
        assert!(
            (p.right_arm.x_rot + FRAC_PI_2).abs() < 1e-6,
            "{}",
            p.right_arm.x_rot
        );
        assert!(
            (p.left_arm.x_rot + FRAC_PI_2).abs() < 1e-6,
            "{}",
            p.left_arm.x_rot
        );
        assert!(
            (p.right_arm.y_rot + 0.1).abs() < 1e-6,
            "{}",
            p.right_arm.y_rot
        );
        assert!(
            (p.left_arm.y_rot - 0.5).abs() < 1e-6,
            "{}",
            p.left_arm.y_rot
        );
    }

    #[test]
    fn crossbow_hold_braces_the_far_arm_under_the_stock() {
        let s = AnimSample {
            pitch: 30.0,
            right_arm_pose: ArmPose::CrossbowHold,
            ..Default::default()
        };
        let p = pose_humanoid(&s);
        let head_x = 30.0 * DEG_TO_RAD;
        assert!((p.right_arm.x_rot - (-FRAC_PI_2 + head_x + 0.1)).abs() < 1e-6);
        assert!((p.left_arm.x_rot - (-1.5 + head_x)).abs() < 1e-6);
        assert!(
            (p.right_arm.y_rot + 0.3).abs() < 1e-6,
            "{}",
            p.right_arm.y_rot
        );
        assert!(
            (p.left_arm.y_rot - 0.6).abs() < 1e-6,
            "{}",
            p.left_arm.y_rot
        );
    }

    #[test]
    fn an_untransformed_item_centres_on_the_fist() {
        let right = held_item_transform(&ItemTransform::NONE, false, Mat4::IDENTITY);
        assert_eq!(right.scale, Vec3::splat(MODEL_UNITS_PER_BLOCK));
        let centre = right.transform_point(Vec3::splat(0.5));
        assert!(
            centre.abs_diff_eq(Vec3::new(-1.0, 10.0, -2.0), 1e-4),
            "{centre:?}"
        );
        let left = held_item_transform(&ItemTransform::NONE, true, Mat4::IDENTITY)
            .transform_point(Vec3::splat(0.5));
        assert!(
            left.abs_diff_eq(Vec3::new(1.0, 10.0, -2.0), 1e-4),
            "{left:?}"
        );
    }

    #[test]
    fn a_spear_pose_moves_the_item_in_blocks() {
        let base = held_item_transform(&ItemTransform::NONE, false, Mat4::IDENTITY);
        let pushed =
            held_item_transform(&ItemTransform::NONE, false, Mat4::from_translation(Vec3::Y));
        let hand_rot = Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_y(PI);
        let moved =
            pushed.transform_point(Vec3::splat(0.5)) - base.transform_point(Vec3::splat(0.5));
        assert!(
            moved.abs_diff_eq(hand_rot * Vec3::new(0.0, MODEL_UNITS_PER_BLOCK, 0.0), 1e-3),
            "{moved:?}"
        );
        assert!(pushed.scale.abs_diff_eq(base.scale, 1e-4));
    }

    #[test]
    fn spear_item_pose_is_identity_without_a_spear() {
        let s = AnimSample {
            attack_time: 0.5,
            using_item: true,
            ticks_using_item: 10.0,
            ..AnimSample::default()
        };
        let s = SpearState::of(&s);
        assert_eq!(spear_item_pose(&s, true), Mat4::IDENTITY);
        assert_eq!(spear_item_pose(&s, false), Mat4::IDENTITY);
    }

    #[test]
    fn death_tips_the_model_a_quarter_turn() {
        let alive = AnimSample::default();
        let root = root_pose(&alive, 1.0);
        assert_eq!(root.model_offset, Vec3::ZERO);
        assert_eq!(root.world_offset, Vec3::ZERO);
        let rot = root.rotation;
        assert!(rot.abs_diff_eq(Quat::from_rotation_y(PI), 1e-6), "{rot:?}");

        let dead = AnimSample {
            death_time: 20.0,
            ..Default::default()
        };
        let rot = root_pose(&dead, 1.0).rotation;
        let expect = Quat::from_rotation_y(PI) * Quat::from_rotation_z(FRAC_PI_2);
        assert!(rot.abs_diff_eq(expect, 1e-6), "{rot:?} != {expect:?}");
    }

    #[test]
    fn sleeping_shoves_the_body_along_the_bed_instead_of_turning_it() {
        let asleep = AnimSample {
            sleeping: true,
            bed_orientation: Some(crate::blocks::direction::Direction::South),
            body_yaw: 123.0,
            ..Default::default()
        };
        let root = root_pose(&asleep, 1.0);
        assert!(
            root.world_offset
                .abs_diff_eq(Vec3::new(0.0, 0.0, -1.52), 1e-5),
            "{:?}",
            root.world_offset
        );
        assert_eq!(root.model_offset, Vec3::ZERO);
        let mut spun = asleep;
        spun.body_yaw = -47.0;
        assert!(
            root.rotation
                .abs_diff_eq(root_pose(&spun, 1.0).rotation, 1e-6)
        );

        let unknown_orientation = AnimSample {
            sleeping: true,
            bed_orientation: None,
            ..Default::default()
        };
        assert_eq!(
            root_pose(&unknown_orientation, 1.0).world_offset,
            Vec3::ZERO
        );
    }
}
