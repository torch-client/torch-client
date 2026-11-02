use std::f32::consts::PI;

use bevy::asset::embedded_asset;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{Camera, ClearColorConfig, PerspectiveProjection, Projection};
use bevy::light::NotShadowCaster;
use bevy::light::cluster::ClusterConfig;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::{ShaderDefVal, ShaderRef};

use crate::items::mesh::Transform as ItemTransform;
use crate::items::model::DisplayContext;
use crate::renderer::anim::{
    AnimSample, HeldItem, UseAnimation, rot_lerp, use_animation, wrap_degrees,
};
use crate::renderer::item_assets::{ItemAssets, ItemGpu};
use crate::session::Gamemode;

pub const HAND_LAYER: usize = 2;

const HAND_CAMERA_ORDER: isize = 1;

const HAND_NEAR: f32 = 0.05;
const HAND_FAR: f32 = 100.0;

pub(crate) const HAND_FOV_DEGREES: f32 = 70.0;

pub const HAND_DEPTH_BAND: f32 = 1.0 / 64.0;

pub fn band_floor_def() -> ShaderDefVal {
    ShaderDefVal::UInt(
        "HAND_BAND_FLOOR_BITS".into(),
        (1.0 - HAND_DEPTH_BAND).to_bits(),
    )
}

pub type HandMaterial = ExtendedMaterial<StandardMaterial, HandProjection>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct HandProjection {
    #[uniform(100)]
    pub glint: Vec4,
    #[texture(101)]
    #[sampler(102)]
    pub glint_texture: Option<Handle<Image>>,
}

impl MaterialExtension for HandProjection {
    fn vertex_shader() -> ShaderRef {
        super::embedded_shader("renderer/hand_projection.wgsl")
    }

    fn fragment_shader() -> ShaderRef {
        super::embedded_shader("renderer/hand_glint.wgsl")
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let bits = |name: &str, value: f32| ShaderDefVal::UInt(name.into(), value.to_bits());
        descriptor.vertex.shader_defs.extend([
            bits("HAND_NEAR_BITS", HAND_NEAR),
            bits("HAND_FOV_BITS", HAND_FOV_DEGREES.to_radians()),
            bits("HAND_BAND_BITS", HAND_DEPTH_BAND),
        ]);
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.extend(super::glint::shader_defs());
        }
        Ok(())
    }
}

fn hand_material(base: StandardMaterial) -> HandMaterial {
    ExtendedMaterial {
        base: StandardMaterial {
            fog_enabled: false,
            ..base
        },
        extension: HandProjection::default(),
    }
}

pub(super) fn cutout_material(texture: Option<Handle<Image>>) -> HandMaterial {
    hand_material(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: texture,
        unlit: true,
        alpha_mode: AlphaMode::Mask(0.1),
        double_sided: true,
        cull_mode: None,
        ..default()
    })
}

use crate::items::kinetic::hit_feedback_amount;
use crate::util::mth::ease::{in_out_back, in_out_expo, in_out_sine, out_back};
use crate::util::mth::{DEG_TO_RAD, progress};

const ITEM_POS_X: f32 = 0.56;
const ITEM_POS_Y: f32 = -0.52;
const ITEM_POS_Z: f32 = -0.72;
const ITEM_HEIGHT_SCALE: f32 = -0.6;

const ITEM_SWING_X_POS_SCALE: f32 = -0.4;
const ITEM_SWING_Y_POS_SCALE: f32 = 0.2;
const ITEM_SWING_Z_POS_SCALE: f32 = -0.2;
const ITEM_PRESWING_ROT_Y: f32 = 45.0;
const ITEM_SWING_X_ROT_AMOUNT: f32 = -80.0;
const ITEM_SWING_Y_ROT_AMOUNT: f32 = -20.0;
const ITEM_SWING_Z_ROT_AMOUNT: f32 = -20.0;

const ARM_POS_X: f32 = 0.640_000_05;
const ARM_POS_Y: f32 = -0.6;
const ARM_POS_Z: f32 = -0.719_999_97;
const ARM_HEIGHT_SCALE: f32 = -0.6;
const ARM_SWING_X_POS_SCALE: f32 = -0.3;
const ARM_SWING_Y_POS_SCALE: f32 = 0.4;
const ARM_SWING_Z_POS_SCALE: f32 = -0.4;
const ARM_PRESWING_ROT_Y: f32 = 45.0;
const ARM_SWING_Y_ROT_AMOUNT: f32 = 70.0;
const ARM_SWING_Z_ROT_AMOUNT: f32 = -20.0;
const ARM_PREROTATION_X_OFFSET: f32 = -1.0;
const ARM_PREROTATION_Y_OFFSET: f32 = 3.6;
const ARM_PREROTATION_Z_OFFSET: f32 = 3.5;
const ARM_POSTROTATION_X_OFFSET: f32 = 5.6;
const ARM_ROT_X: f32 = 200.0;
const ARM_ROT_Y: f32 = -135.0;
const ARM_ROT_Z: f32 = 120.0;

const EAT_JIGGLE_X_ROT_AMOUNT: f32 = 10.0;
const EAT_JIGGLE_Y_ROT_AMOUNT: f32 = 90.0;
const EAT_JIGGLE_Z_ROT_AMOUNT: f32 = 30.0;
const EAT_JIGGLE_X_POS_SCALE: f32 = 0.6;
const EAT_JIGGLE_Y_POS_SCALE: f32 = -0.5;
const EAT_JIGGLE_Z_POS_SCALE: f32 = 0.0;
const EAT_JIGGLE_EXPONENT: f32 = 27.0;
const EAT_EXTRA_JIGGLE_CUTOFF: f32 = 0.8;
const EAT_EXTRA_JIGGLE_SCALE: f32 = 0.1;

const BOW_CHARGE_Z_POS_SCALE: f32 = 0.04;
const BOW_CHARGE_SHAKE_Y_SCALE: f32 = 0.004;
const BOW_CHARGE_Z_SCALE: f32 = 0.2;
const BOW_MIN_SHAKE_CHARGE: f32 = 0.1;

const TRIDENT_CHARGE_Z_POS_SCALE: f32 = 0.2;

const EQUIP_STEP: f32 = 0.4;

const EQUIP_SWAP_THRESHOLD: f32 = 0.1;

const CROSSBOW_CHARGE_TICKS: f32 = 25.0;

const BRUSH_USE_DURATION_TICKS: f32 = 200.0;

const VIEW_LAG_SCALE: f32 = 0.1;

const VIEW_BOB_RATE: f32 = 0.5;

const DEFAULT_CONSUME_TICKS: f32 = 32.0;

const MAP_EDGE_PIXELS: f32 = crate::client::tracking::MAP_EDGE as f32;

const MAP_Z_OFFSET: f32 = -0.01;

const MAP_BORDER_PIXELS: f32 = 7.0;

const DECORATION_Z: f32 = -0.02;
const DECORATION_Z_OFFSET: f32 = -0.001;

const DECORATION_SCALE: [f32; 3] = [4.0, 4.0, 3.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    Main,
    Off,
}

impl Hand {
    pub fn is_left(self, main_arm_left: bool) -> bool {
        (self == Hand::Main) == main_arm_left
    }

    pub fn invert(self, main_arm_left: bool) -> f32 {
        if self.is_left(main_arm_left) {
            -1.0
        } else {
            1.0
        }
    }

    fn display_context(self, main_arm_left: bool) -> DisplayContext {
        if self.is_left(main_arm_left) {
            DisplayContext::FirstPersonLeftHand
        } else {
            DisplayContext::FirstPersonRightHand
        }
    }

    fn index(self) -> usize {
        match self {
            Hand::Main => 0,
            Hand::Off => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandRenderSelection {
    pub main: bool,
    pub off: bool,
}

impl HandRenderSelection {
    const BOTH: HandRenderSelection = HandRenderSelection {
        main: true,
        off: true,
    };
    const MAIN_ONLY: HandRenderSelection = HandRenderSelection {
        main: true,
        off: false,
    };
    const OFF_ONLY: HandRenderSelection = HandRenderSelection {
        main: false,
        off: true,
    };
}

pub fn which_hands_to_render(s: &AnimSample) -> HandRenderSelection {
    let bow_like = |item: &HeldItem| item.id == "bow" || item.id == "crossbow";
    if !bow_like(&s.main_hand) && !bow_like(&s.off_hand) {
        return HandRenderSelection::BOTH;
    }
    let charged_crossbow = |item: &HeldItem| item.id == "crossbow" && item.charged;
    if s.using_item {
        let used = if s.using_offhand {
            &s.off_hand
        } else {
            &s.main_hand
        };
        if bow_like(used) {
            return if s.using_offhand {
                HandRenderSelection::OFF_ONLY
            } else {
                HandRenderSelection::MAIN_ONLY
            };
        }
        if !s.using_offhand && charged_crossbow(&s.off_hand) {
            return HandRenderSelection::MAIN_ONLY;
        }
        return HandRenderSelection::BOTH;
    }
    if charged_crossbow(&s.main_hand) {
        HandRenderSelection::MAIN_ONLY
    } else {
        HandRenderSelection::BOTH
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Chain {
    mats: [Mat4; 2],
    active: usize,
}

impl Chain {
    pub(super) fn new() -> Chain {
        Chain {
            mats: [Mat4::IDENTITY; 2],
            active: 0,
        }
    }

    pub(super) fn translate(&mut self, x: f32, y: f32, z: f32) {
        self.mats[self.active] *= Mat4::from_translation(Vec3::new(x, y, z));
    }

    pub(super) fn rot_x(&mut self, deg: f32) {
        self.mats[self.active] *= Mat4::from_rotation_x(deg * DEG_TO_RAD);
    }

    pub(super) fn rot_y(&mut self, deg: f32) {
        self.mats[self.active] *= Mat4::from_rotation_y(deg * DEG_TO_RAD);
    }

    pub(super) fn rot_z(&mut self, deg: f32) {
        self.mats[self.active] *= Mat4::from_rotation_z(deg * DEG_TO_RAD);
    }

    fn rotate(&mut self, q: Quat) {
        self.mats[self.active] *= Mat4::from_quat(q);
    }

    fn rotate_around(&mut self, q: Quat, pivot: Vec3) {
        self.mats[self.active] *= crate::entities::rotate_around(q, pivot).to_matrix();
    }

    pub(super) fn scale(&mut self, x: f32, y: f32, z: f32) {
        self.mats[self.active] *= Mat4::from_scale(Vec3::new(x, y, z));
        self.active = 1;
    }

    pub(super) fn transforms(&self) -> (Transform, Transform) {
        (
            Transform::from_matrix(self.mats[0]),
            Transform::from_matrix(self.mats[1]),
        )
    }
}

fn apply_item_arm_transform(c: &mut Chain, invert: f32, inverse_arm_height: f32) {
    c.translate(
        invert * ITEM_POS_X,
        ITEM_POS_Y + inverse_arm_height * ITEM_HEIGHT_SCALE,
        ITEM_POS_Z,
    );
}

fn apply_item_arm_attack_transform(c: &mut Chain, invert: f32, attack: f32) {
    let y_swing_rotation = (attack * attack * PI).sin();
    c.rot_y(invert * (ITEM_PRESWING_ROT_Y + y_swing_rotation * ITEM_SWING_Y_ROT_AMOUNT));
    let xz_swing_rotation = (attack.sqrt() * PI).sin();
    c.rot_z(invert * xz_swing_rotation * ITEM_SWING_Z_ROT_AMOUNT);
    c.rot_x(xz_swing_rotation * ITEM_SWING_X_ROT_AMOUNT);
    c.rot_y(invert * -ITEM_PRESWING_ROT_Y);
}

fn swing_arm(c: &mut Chain, attack: f32, invert: f32) {
    let x_swing_position = ITEM_SWING_X_POS_SCALE * (attack.sqrt() * PI).sin();
    let y_swing_position = ITEM_SWING_Y_POS_SCALE * (attack.sqrt() * 2.0 * PI).sin();
    let z_swing_position = ITEM_SWING_Z_POS_SCALE * (attack * PI).sin();
    c.translate(
        invert * x_swing_position,
        y_swing_position,
        z_swing_position,
    );
    apply_item_arm_attack_transform(c, invert, attack);
}

fn apply_eat_transform(c: &mut Chain, invert: f32, curr_usage_time: f32, use_duration: f32) {
    let scaled_usage_time = curr_usage_time / use_duration;
    if scaled_usage_time < EAT_EXTRA_JIGGLE_CUTOFF {
        let jiggle = ((curr_usage_time / 4.0 * PI).cos() * EAT_EXTRA_JIGGLE_SCALE).abs();
        c.translate(0.0, jiggle, 0.0);
    }
    let eat_jiggle = 1.0 - scaled_usage_time.powf(EAT_JIGGLE_EXPONENT);
    c.translate(
        eat_jiggle * EAT_JIGGLE_X_POS_SCALE * invert,
        eat_jiggle * EAT_JIGGLE_Y_POS_SCALE,
        eat_jiggle * EAT_JIGGLE_Z_POS_SCALE,
    );
    c.rot_y(invert * eat_jiggle * EAT_JIGGLE_Y_ROT_AMOUNT);
    c.rot_x(eat_jiggle * EAT_JIGGLE_X_ROT_AMOUNT);
    c.rot_z(invert * eat_jiggle * EAT_JIGGLE_Z_ROT_AMOUNT);
}

fn apply_brush_transform(
    c: &mut Chain,
    hand: Hand,
    main_arm_left: bool,
    remaining_ticks: f32,
    frame_interp: f32,
) {
    let delta_since_last_update = remaining_ticks % 10.0 - frame_interp + 1.0;
    let scaled_usage_time = 1.0 - delta_since_last_update / 10.0;
    let swipe_angle = -15.0 + 75.0 * (scaled_usage_time * 2.0 * PI).cos();
    if hand.is_left(main_arm_left) {
        c.translate(0.1, 0.83, 0.35);
        c.rot_x(-80.0);
        c.rot_y(-90.0);
        c.rot_x(swipe_angle);
        c.translate(-0.3, 0.22, 0.35);
    } else {
        c.translate(-0.25, 0.22, 0.35);
        c.rot_x(-80.0);
        c.rot_y(90.0);
        c.rot_x(swipe_angle);
    }
}

fn apply_charge_shake(c: &mut Chain, time_held: f32, power: f32, z_translate: f32) {
    if power > BOW_MIN_SHAKE_CHARGE {
        let shake =
            ((time_held - BOW_MIN_SHAKE_CHARGE) * 1.3).sin() * (power - BOW_MIN_SHAKE_CHARGE);
        c.translate(0.0, shake * BOW_CHARGE_SHAKE_Y_SCALE, 0.0);
    }
    c.translate(0.0, 0.0, power * z_translate);
    c.scale(1.0, 1.0, 1.0 + power * BOW_CHARGE_Z_SCALE);
}

fn spear_attack_transform(c: &mut Chain, invert: f32, attack: f32) {
    let starting_amount = in_out_sine(progress(attack, 0.0, 0.05));
    let middle_amount = out_back(progress(attack, 0.05, 0.2));
    let ending_amount = in_out_expo(progress(attack, 0.4, 1.0));
    c.translate(
        invert * 0.1 * (starting_amount - middle_amount),
        -0.075 * (starting_amount - ending_amount),
        0.65 * (starting_amount - middle_amount),
    );
    c.rot_x(-70.0 * (starting_amount - ending_amount));
    c.translate(0.0, 0.0, -0.25 * (ending_amount - middle_amount));
}

fn spear_use_transform(c: &mut Chain, f: &HandFrame) {
    let Some(kinetic) = f.item.kinetic else {
        return;
    };
    let p = kinetic.use_params(f.time_held());
    let invert = f.invert();
    c.translate(
        invert * (p.raise * 0.15 + p.raise_end * -0.05 + p.sway * -0.1 + p.sway_slow * 0.005),
        p.raise * -0.075 + p.raise_middle * 0.075 + p.sway_fast * 0.01,
        p.raise_start * 0.05 + p.raise_end * -0.05 + p.sway_slow * 0.005,
    );
    let pitch =
        -65.0 * in_out_back(p.raise) - 35.0 * p.lower + 100.0 * p.raise_back - 0.5 * p.sway_fast;
    c.rotate_around(
        Quat::from_rotation_x(pitch * DEG_TO_RAD),
        Vec3::new(0.0, 0.1, 0.0),
    );
    let yaw = invert * (-90.0 * progress(p.raise, 0.5, 0.55) + 90.0 * p.sway + 2.0 * p.sway_slow);
    c.rotate_around(
        Quat::from_rotation_y(-yaw * DEG_TO_RAD),
        Vec3::new(invert * 0.15, 0.0, 0.0),
    );
    c.translate(0.0, -hit_feedback_amount(f.ticks_since_kinetic_hit), 0.0);
}

#[derive(Clone, Debug)]
pub struct HandFrame {
    pub hand: Hand,
    pub main_arm_left: bool,
    pub item: HeldItem,
    pub attack: f32,
    pub inverse_arm_height: f32,
    pub using: bool,
    pub ticks_using: f32,
    pub ticks_since_kinetic_hit: f32,
    pub spin_attack: bool,
    pub view_lag_x: f32,
    pub view_lag_y: f32,
    pub view_x_rot: f32,
}

impl HandFrame {
    fn invert(&self) -> f32 {
        self.hand.invert(self.main_arm_left)
    }

    fn is_left(&self) -> bool {
        self.hand.is_left(self.main_arm_left)
    }

    fn curr_usage_time(&self, use_duration: f32) -> f32 {
        use_duration - self.ticks_using
    }

    fn time_held(&self) -> f32 {
        self.ticks_using.max(0.0)
    }

    fn use_remaining_ticks(&self, use_duration: f32) -> f32 {
        (use_duration - self.ticks_using.floor() - 1.0).max(0.0)
    }

    fn using_now(&self, animation: UseAnimation) -> bool {
        if !self.using {
            return false;
        }
        match animation {
            UseAnimation::Eat | UseAnimation::Drink => {
                self.use_remaining_ticks(use_duration_ticks(self.item.id)) > 0.0
            }
            _ => true,
        }
    }

    fn frame_interp(&self) -> f32 {
        self.ticks_using.fract()
    }
}

fn use_duration_ticks(id: &str) -> f32 {
    match id {
        "honey_bottle" => 40.0,
        "dried_kelp" => 16.0,
        _ => DEFAULT_CONSUME_TICKS,
    }
}

fn arm_with_item(f: &HandFrame) -> Chain {
    let mut c = Chain::new();
    let invert = f.invert();
    apply_view_lag(&mut c, f);

    if f.item.id == "crossbow" {
        apply_item_arm_transform(&mut c, invert, f.inverse_arm_height);
        if f.using && !f.item.charged {
            c.translate(invert * -0.478_568_2, -0.094_387, 0.057_315_31);
            c.rot_x(-11.935);
            c.rot_y(invert * 65.3);
            c.rot_z(invert * -9.785);
            let time_held = f.time_held();
            let power = (time_held / CROSSBOW_CHARGE_TICKS).min(1.0);
            apply_charge_shake(&mut c, time_held, power, BOW_CHARGE_Z_POS_SCALE);
            c.rot_y(-invert * ITEM_PRESWING_ROT_Y);
        } else {
            swing_arm(&mut c, f.attack, invert);
            if f.item.charged && f.attack < 0.001 && f.hand == Hand::Main {
                c.translate(invert * -0.641_864, 0.0, 0.0);
                c.rot_y(invert * 10.0);
            }
        }
        return c;
    }

    let animation = use_animation(f.item.id);
    if f.using_now(animation) {
        if !matches!(
            animation,
            UseAnimation::Eat | UseAnimation::Drink | UseAnimation::Spear
        ) {
            apply_item_arm_transform(&mut c, invert, f.inverse_arm_height);
        }
        match animation {
            UseAnimation::Eat | UseAnimation::Drink => {
                let duration = use_duration_ticks(f.item.id);
                apply_eat_transform(&mut c, invert, f.curr_usage_time(duration), duration);
                apply_item_arm_transform(&mut c, invert, f.inverse_arm_height);
            }
            UseAnimation::Block => {
                if f.item.id != "shield" {
                    c.translate(invert * -0.141_421_36, 0.08, 0.141_421_36);
                    c.rot_x(-102.25);
                    c.rot_y(invert * 13.365);
                    c.rot_z(invert * 78.05);
                }
            }
            UseAnimation::Bow => {
                c.translate(invert * -0.278_568_2, 0.183_443_87, 0.157_315_31);
                c.rot_x(-13.935);
                c.rot_y(invert * 35.3);
                c.rot_z(invert * -9.785);
                let time_held = f.time_held();
                let linear = time_held / 20.0;
                let power = ((linear * linear + linear * 2.0) / 3.0).min(1.0);
                apply_charge_shake(&mut c, time_held, power, BOW_CHARGE_Z_POS_SCALE);
                c.rot_y(-invert * ITEM_PRESWING_ROT_Y);
            }
            UseAnimation::Trident => {
                c.translate(invert * -0.5, 0.7, 0.1);
                c.rot_x(-55.0);
                c.rot_y(invert * 35.3);
                c.rot_z(invert * -9.785);
                let time_held = f.time_held();
                let power = (time_held / 10.0).min(1.0);
                apply_charge_shake(&mut c, time_held, power, TRIDENT_CHARGE_Z_POS_SCALE);
                c.rot_y(-invert * ITEM_PRESWING_ROT_Y);
            }
            UseAnimation::Brush => apply_brush_transform(
                &mut c,
                f.hand,
                f.main_arm_left,
                f.use_remaining_ticks(BRUSH_USE_DURATION_TICKS),
                f.frame_interp(),
            ),
            UseAnimation::Spear => {
                c.translate(invert * ITEM_POS_X, ITEM_POS_Y, ITEM_POS_Z);
                spear_use_transform(&mut c, f);
            }
            UseAnimation::None
            | UseAnimation::Crossbow
            | UseAnimation::Spyglass
            | UseAnimation::TootHorn => {}
        }
        return c;
    }

    if f.spin_attack {
        apply_item_arm_transform(&mut c, invert, f.inverse_arm_height);
        c.translate(invert * -0.4, 0.8, 0.3);
        c.rot_y(invert * 65.0);
        c.rot_z(invert * -85.0);
        return c;
    }

    apply_item_arm_transform(&mut c, invert, f.inverse_arm_height);
    if animation == UseAnimation::Spear {
        spear_attack_transform(&mut c, invert, f.attack);
    } else {
        swing_arm(&mut c, f.attack, invert);
    }
    c
}

fn apply_view_lag(c: &mut Chain, f: &HandFrame) {
    c.rot_x(f.view_lag_x);
    c.rot_y(f.view_lag_y);
}

pub(super) fn apply_display_transform(c: &mut Chain, t: &ItemTransform, left_hand: bool) {
    let (tx, ry, rz) = if left_hand {
        (-t.translation[0], -t.rotation[1], -t.rotation[2])
    } else {
        (t.translation[0], t.rotation[1], t.rotation[2])
    };
    c.translate(tx, t.translation[1], t.translation[2]);
    c.rotate(
        Quat::from_rotation_x(t.rotation[0] * DEG_TO_RAD)
            * Quat::from_rotation_y(ry * DEG_TO_RAD)
            * Quat::from_rotation_z(rz * DEG_TO_RAD),
    );
    c.scale(t.scale[0], t.scale[1], t.scale[2]);
    c.translate(-0.5, -0.5, -0.5);
}

fn player_arm(f: &HandFrame) -> Chain {
    let mut c = Chain::new();
    apply_view_lag(&mut c, f);
    apply_player_arm(
        &mut c,
        f.hand,
        f.main_arm_left,
        f.attack,
        f.inverse_arm_height,
    );
    c
}

fn apply_player_arm(
    c: &mut Chain,
    hand: Hand,
    main_arm_left: bool,
    attack: f32,
    inverse_arm_height: f32,
) {
    let invert = hand.invert(main_arm_left);
    let sqrt_attack = attack.sqrt();
    let x_swing_position = ARM_SWING_X_POS_SCALE * (sqrt_attack * PI).sin();
    let y_swing_position = ARM_SWING_Y_POS_SCALE * (sqrt_attack * 2.0 * PI).sin();
    let z_swing_position = ARM_SWING_Z_POS_SCALE * (attack * PI).sin();
    c.translate(
        invert * (x_swing_position + ARM_POS_X),
        y_swing_position + ARM_POS_Y + inverse_arm_height * ARM_HEIGHT_SCALE,
        z_swing_position + ARM_POS_Z,
    );
    c.rot_y(invert * ARM_PRESWING_ROT_Y);

    let z_swing_rotation = (attack * attack * PI).sin();
    let y_swing_rotation = (sqrt_attack * PI).sin();
    c.rot_y(invert * y_swing_rotation * ARM_SWING_Y_ROT_AMOUNT);
    c.rot_z(invert * z_swing_rotation * ARM_SWING_Z_ROT_AMOUNT);

    c.translate(
        invert * ARM_PREROTATION_X_OFFSET,
        ARM_PREROTATION_Y_OFFSET,
        ARM_PREROTATION_Z_OFFSET,
    );
    c.rot_z(invert * ARM_ROT_Z);
    c.rot_x(ARM_ROT_X);
    c.rot_y(invert * ARM_ROT_Y);
    c.translate(invert * ARM_POSTROTATION_X_OFFSET, 0.0, 0.0);
    apply_arm_part(c, hand, main_arm_left);
}

fn apply_arm_part(c: &mut Chain, hand: Hand, main_arm_left: bool) {
    let arm = arm_def(hand, main_arm_left);
    c.translate(
        arm.pivot[0] / 16.0,
        arm.pivot[1] / 16.0,
        arm.pivot[2] / 16.0,
    );
    c.rot_z(arm.z_rot.to_degrees());
}

fn map_tilt(x_rot: f32) -> f32 {
    let raised = (1.0 - x_rot / 45.0 + 0.1).clamp(0.0, 1.0);
    -(raised * PI).cos() * 0.5 + 0.5
}

fn two_handed_map_base(f: &HandFrame) -> Chain {
    let mut c = Chain::new();
    apply_view_lag(&mut c, f);
    let sqrt_attack = f.attack.sqrt();
    let y_swing_position = -0.2 * (f.attack * PI).sin();
    let z_swing_position = -0.4 * (sqrt_attack * PI).sin();
    c.translate(0.0, -y_swing_position / 2.0, z_swing_position);
    let tilt = map_tilt(f.view_x_rot);
    c.translate(0.0, 0.04 + f.inverse_arm_height * -1.2 + tilt * -0.5, -0.72);
    c.rot_x(tilt * -85.0);
    c
}

fn map_hand(base: &Chain, arm: Hand, main_arm_left: bool) -> Chain {
    let mut c = *base;
    c.rot_y(90.0);
    let invert = arm.invert(main_arm_left);
    c.rot_y(92.0);
    c.rot_x(45.0);
    c.rot_z(invert * -41.0);
    c.translate(invert * 0.3, -1.1, 0.45);
    apply_arm_part(&mut c, arm, main_arm_left);
    c
}

fn two_handed_map(base: &Chain, attack: f32) -> Chain {
    let mut c = *base;
    c.rot_x((attack.sqrt() * PI).sin() * 20.0);
    c.scale(2.0, 2.0, 2.0);
    apply_map_transform(&mut c);
    c
}

fn one_handed_map_arm(f: &HandFrame) -> Chain {
    let mut c = Chain::new();
    apply_view_lag(&mut c, f);
    let invert = f.invert();
    c.translate(invert * 0.125, -0.125, 0.0);
    c.rot_z(invert * 10.0);
    apply_player_arm(
        &mut c,
        f.hand,
        f.main_arm_left,
        f.attack,
        f.inverse_arm_height,
    );
    c
}

fn one_handed_map(f: &HandFrame) -> Chain {
    let mut c = Chain::new();
    apply_view_lag(&mut c, f);
    let invert = f.invert();
    c.translate(invert * 0.125, -0.125, 0.0);
    c.translate(invert * 0.51, -0.08 + f.inverse_arm_height * -1.2, -0.75);
    let sqrt_attack = f.attack.sqrt();
    let x_swing = (sqrt_attack * PI).sin();
    let x_swing_position = -0.5 * x_swing;
    let y_swing_position = 0.4 * (sqrt_attack * 2.0 * PI).sin();
    let z_swing_position = -0.3 * (f.attack * PI).sin();
    c.translate(
        invert * x_swing_position,
        y_swing_position - 0.3 * x_swing,
        z_swing_position,
    );
    c.rot_x(x_swing * -45.0);
    c.rot_y(invert * x_swing * -30.0);
    apply_map_transform(&mut c);
    c
}

fn apply_map_transform(c: &mut Chain) {
    c.rot_y(180.0);
    c.rot_z(180.0);
    c.scale(0.38, 0.38, 0.38);
    c.translate(-0.5, -0.5, 0.0);
    c.scale(
        1.0 / MAP_EDGE_PIXELS,
        1.0 / MAP_EDGE_PIXELS,
        1.0 / MAP_EDGE_PIXELS,
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EquipRamp {
    pub height: f32,
    pub old: f32,
}

impl EquipRamp {
    pub fn advance(&mut self, target: f32) {
        self.old = self.height;
        self.height += (target - self.height).clamp(-EQUIP_STEP, EQUIP_STEP);
    }

    pub fn inverse_arm_height(&self, partial: f32) -> f32 {
        1.0 - (self.old + (self.height - self.old) * partial)
    }

    pub fn hidden_enough(&self) -> bool {
        self.height < EQUIP_SWAP_THRESHOLD
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewBob {
    pub angle: f32,
    pub old: f32,
}

impl ViewBob {
    pub fn advance(&mut self, rot: f32) {
        self.old = self.angle;
        self.angle = rot_lerp(VIEW_BOB_RATE, self.angle, rot);
    }

    pub fn lag(&self, rot: f32, partial: f32) -> f32 {
        let smoothed = rot_lerp(partial, self.old, self.angle);
        wrap_degrees(rot - smoothed) * VIEW_LAG_SCALE
    }
}

struct ArmDef {
    tex: [f32; 2],
    sleeve_tex: [f32; 2],
    origin: [f32; 3],
    pivot: [f32; 3],
    z_rot: f32,
}

const OVERLAY_SCALE: f32 = 0.25;

const ARM_SIZE: [f32; 3] = [4.0, 12.0, 4.0];

fn arm_def(hand: Hand, main_arm_left: bool) -> ArmDef {
    if hand.is_left(main_arm_left) {
        LEFT_ARM
    } else {
        RIGHT_ARM
    }
}

const RIGHT_ARM: ArmDef = ArmDef {
    tex: [40.0, 16.0],
    sleeve_tex: [40.0, 32.0],
    origin: [-3.0, -2.0, -2.0],
    pivot: [-5.0, 2.0, 0.0],
    z_rot: 0.1,
};

const LEFT_ARM: ArmDef = ArmDef {
    tex: [32.0, 48.0],
    sleeve_tex: [48.0, 48.0],
    origin: [-1.0, -2.0, -2.0],
    pivot: [5.0, 2.0, 0.0],
    z_rot: -0.1,
};

const ARM_COLOR: Option<[f32; 4]> = if cfg!(feature = "shader_support") {
    Some([1.0; 4])
} else {
    None
};

fn arm_box_mesh(tex: [f32; 2], origin: [f32; 3], size: [f32; 3], grow: f32) -> Mesh {
    super::skin::box_mesh(
        super::skin::SKIN_SHEET,
        tex,
        origin,
        size,
        grow,
        super::skin::MODEL_TO_BLOCKS,
        ARM_COLOR,
    )
}

#[cfg(feature = "skins")]
const SLIM_ARM_SIZE: [f32; 3] = [3.0, 12.0, 4.0];

#[cfg(feature = "skins")]
fn slim_origin(left: bool, wide: [f32; 3]) -> [f32; 3] {
    if left {
        wide
    } else {
        [wide[0] + 1.0, wide[1], wide[2]]
    }
}

fn map_quad(min: f32, max: f32, z: f32) -> Mesh {
    let mut out = crate::entities::quads::QuadMesh::new();
    out.quad(
        [[min, max, z], [max, max, z], [max, min, z], [min, min, z]],
        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        [0.0, 0.0, 1.0],
    );
    out.finish()
}

fn map_background_mesh() -> Mesh {
    map_quad(-MAP_BORDER_PIXELS, MAP_EDGE_PIXELS + MAP_BORDER_PIXELS, 0.0)
}

fn map_picture_mesh() -> Mesh {
    map_quad(0.0, MAP_EDGE_PIXELS, MAP_Z_OFFSET)
}

fn decoration_mesh(decorations: &[crate::session::MapDecoration]) -> Mesh {
    let mut out = crate::entities::quads::QuadMesh::new();
    for (index, decoration) in decorations.iter().enumerate() {
        let Some((u0, u1)) = crate::renderer::maps::decoration_uv(decoration.kind) else {
            continue;
        };
        let turn = decoration.rot as f32 * 360.0 / 16.0;
        let pose = Mat4::from_translation(Vec3::new(
            decoration.x as f32 / 2.0 + MAP_EDGE_PIXELS / 2.0,
            decoration.y as f32 / 2.0 + MAP_EDGE_PIXELS / 2.0,
            DECORATION_Z,
        )) * Mat4::from_rotation_z(turn * DEG_TO_RAD)
            * Mat4::from_scale(Vec3::from(DECORATION_SCALE))
            * Mat4::from_translation(Vec3::new(-0.125, 0.125, 0.0));
        let z = index as f32 * DECORATION_Z_OFFSET;
        let corner = |x: f32, y: f32| pose.transform_point3(Vec3::new(x, y, z)).to_array();
        out.quad(
            [
                corner(-1.0, 1.0),
                corner(1.0, 1.0),
                corner(1.0, -1.0),
                corner(-1.0, -1.0),
            ],
            [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]],
            [0.0, 0.0, 1.0],
        );
    }
    out.finish()
}

#[derive(Component)]
#[cfg_attr(
    feature = "shader_support",
    derive(Clone, bevy::render::extract_component::ExtractComponent)
)]
pub struct HandCamera;

#[derive(Component)]
pub struct HandRoot;

#[derive(Component)]
struct HandNode;

type NodeQuery<'w, 's> =
    Query<'w, 's, (&'static mut Transform, &'static mut Visibility), With<HandNode>>;

#[derive(Component)]
struct HandItemNode;

#[derive(Component)]
struct HandMapNode;

struct HandEntities {
    pose: Entity,
    item: Entity,
    arm: Entity,
    sleeve: Entity,
    map: Entity,
    map_background: Entity,
    map_picture: Entity,
    map_decorations: Entity,
}

struct UploadedItem {
    item: HeldItem,
    using: bool,
    main_arm_left: bool,
    transform: Option<ItemTransform>,
}

struct HandState {
    nodes: HandEntities,
    ramp: EquipRamp,
    shown: HeldItem,
    uploaded: Option<UploadedItem>,
    item_used_seen: u32,
    map_bound: Option<Handle<Image>>,
    decorations_built: Option<(i32, u32)>,
}

impl HandState {
    fn new(nodes: HandEntities) -> HandState {
        HandState {
            nodes,
            ramp: EquipRamp::default(),
            shown: HeldItem::default(),
            uploaded: None,
            item_used_seen: 0,
            map_bound: None,
            decorations_built: None,
        }
    }
}

#[derive(Resource)]
struct HandRig {
    hands: [HandState; 2],
    x_bob: ViewBob,
    y_bob: ViewBob,
    shown_slot: u8,
    last_tick: Option<f32>,
    main_arm_worn: Option<bool>,
    #[cfg(feature = "skins")]
    skin_worn: Option<crate::client::skins::SkinState>,
    #[cfg(feature = "skins")]
    skin_seen: u32,
}

impl HandRig {
    fn new(hands: [HandEntities; 2]) -> HandRig {
        HandRig {
            hands: hands.map(HandState::new),
            x_bob: ViewBob::default(),
            y_bob: ViewBob::default(),
            shown_slot: 0,
            last_tick: None,
            main_arm_worn: None,
            #[cfg(feature = "skins")]
            skin_worn: None,
            #[cfg(feature = "skins")]
            skin_seen: 0,
        }
    }
}

#[derive(Resource)]
struct HandAssets {
    arm: [Handle<Mesh>; 2],
    sleeve: [Handle<Mesh>; 2],
    #[cfg(feature = "skins")]
    slim_arm: [Handle<Mesh>; 2],
    #[cfg(feature = "skins")]
    slim_sleeve: [Handle<Mesh>; 2],
    base: Handle<HandMaterial>,
    overlay: Handle<HandMaterial>,
    map_background: Handle<HandMaterial>,
    map_checkerboard: Handle<HandMaterial>,
    map_picture: [Handle<HandMaterial>; 2],
    decorations: Handle<HandMaterial>,
    item: [Handle<HandMaterial>; 2],
}

pub struct HandPlugin;

impl Plugin for HandPlugin {
    fn build(&self, app: &mut App) {
        use crate::renderer::AppState;
        embedded_asset!(app, "hand_projection.wgsl");
        embedded_asset!(app, "hand_glint.wgsl");
        app.add_plugins(MaterialPlugin::<HandMaterial>::default());
        crate::renderer::reload::touch_on_reload::<HandMaterial>(app);
        app.add_systems(
            Update,
            setup_hand
                .after(super::glint::GlintLoad)
                .run_if(resource_added::<crate::renderer::systems::AssetsReady>),
        )
        .add_systems(
            Update,
            sync_hands
                .after(crate::renderer::frame_view::FrameViewSystems)
                .run_if(in_state(AppState::InGame)),
        );

        app.add_systems(
            Update,
            bind_hand_arms
                .after(crate::renderer::frame_view::FrameViewSystems)
                .before(sync_hands)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

fn bind_hand_arms(
    view: Res<crate::renderer::frame_view::FrameView>,
    assets: Option<Res<HandAssets>>,
    rig: Option<ResMut<HandRig>>,
    #[cfg(feature = "skins")] mut skins: ResMut<super::skin::SkinTextures>,
    #[cfg(feature = "skins")] mut materials: ResMut<Assets<HandMaterial>>,
    #[cfg(feature = "skins")] mut images: ResMut<Assets<Image>>,
    mut meshes: Query<&mut Mesh3d>,
) {
    let (Some(assets), Some(mut rig)) = (assets, rig) else {
        return;
    };
    #[cfg(feature = "skins")]
    let generation = skins.generation();
    let main_arm_left = view.main_hand_left;
    #[cfg(feature = "skins")]
    let skin = &view.local_skin;

    let arms_settled = rig.main_arm_worn == Some(main_arm_left);
    #[cfg(feature = "skins")]
    let skin_settled = rig.skin_worn.as_ref() == Some(skin) && rig.skin_seen == generation;
    #[cfg(not(feature = "skins"))]
    let skin_settled = true;
    if arms_settled && skin_settled {
        return;
    }

    #[cfg(feature = "skins")]
    let slim = skin.slim();
    #[cfg(feature = "skins")]
    let slim_changed = rig.skin_worn.as_ref().map(|w| w.slim()) != Some(slim);
    #[cfg(not(feature = "skins"))]
    let slim_changed = false;

    #[cfg(feature = "skins")]
    {
        rig.skin_seen = generation;
        let downloaded = skin
            .refs
            .body
            .as_ref()
            .and_then(|url| skins.get(url).cloned());
        let texture = match downloaded {
            Some(handle) => Some(handle),
            None => skins.default_skin(skin.default_index, &mut images),
        };
        if let Some(texture) = texture {
            for handle in [&assets.base, &assets.overlay] {
                if let Some(material) = materials.get_mut(handle) {
                    material.base.base_color_texture = Some(texture.clone());
                }
            }
        }
        rig.skin_worn = Some(skin.clone());
    }

    if arms_settled && !slim_changed {
        return;
    }
    rig.main_arm_worn = Some(main_arm_left);
    #[cfg(feature = "skins")]
    let (arms, sleeves) = if slim {
        (&assets.slim_arm, &assets.slim_sleeve)
    } else {
        (&assets.arm, &assets.sleeve)
    };
    #[cfg(not(feature = "skins"))]
    let (arms, sleeves) = (&assets.arm, &assets.sleeve);
    for (hand, state) in [Hand::Main, Hand::Off].into_iter().zip(rig.hands.iter()) {
        let nodes = &state.nodes;
        let side = hand.is_left(main_arm_left) as usize;
        if let Ok(mut mesh) = meshes.get_mut(nodes.arm) {
            mesh.0 = arms[side].clone();
        }
        if let Ok(mut mesh) = meshes.get_mut(nodes.sleeve) {
            mesh.0 = sleeves[side].clone();
        }
    }
}

fn setup_hand(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<HandMaterial>>,
    glint: Option<Res<super::glint::GlintTexture>>,
) {
    let Some(skin) = super::skin::load_default_skin(&mut images, "first-person hand") else {
        return;
    };

    let mut cutout = |texture: Option<Handle<Image>>| materials.add(cutout_material(texture));
    let base = cutout(Some(skin.clone()));
    let overlay = cutout(Some(skin));
    let paper = |images: &mut Assets<Image>, name: &str| {
        super::skin::load_texture(images, &format!("textures/map/{name}.png"), "held map")
    };

    let assets = HandAssets {
        arm: [RIGHT_ARM, LEFT_ARM]
            .map(|def| meshes.add(arm_box_mesh(def.tex, def.origin, ARM_SIZE, 0.0))),
        sleeve: [RIGHT_ARM, LEFT_ARM].map(|def| {
            meshes.add(arm_box_mesh(
                def.sleeve_tex,
                def.origin,
                ARM_SIZE,
                OVERLAY_SCALE,
            ))
        }),
        #[cfg(feature = "skins")]
        slim_arm: [(false, RIGHT_ARM), (true, LEFT_ARM)].map(|(left, def)| {
            meshes.add(arm_box_mesh(
                def.tex,
                slim_origin(left, def.origin),
                SLIM_ARM_SIZE,
                0.0,
            ))
        }),
        #[cfg(feature = "skins")]
        slim_sleeve: [(false, RIGHT_ARM), (true, LEFT_ARM)].map(|(left, def)| {
            meshes.add(arm_box_mesh(
                def.sleeve_tex,
                slim_origin(left, def.origin),
                SLIM_ARM_SIZE,
                OVERLAY_SCALE,
            ))
        }),
        map_background: cutout(paper(&mut images, "map_background")),
        map_checkerboard: cutout(paper(&mut images, "map_background_checkerboard")),
        map_picture: std::array::from_fn(|_| cutout(None)),
        decorations: cutout(Some(crate::renderer::maps::build_decoration_sheet(
            &mut images,
        ))),
        item: std::array::from_fn(|_| cutout(None)),
        base,
        overlay,
    };
    let glint_texture = glint.map(|g| g.0.clone());
    for handle in &assets.item {
        if let Some(material) = materials.get_mut(handle) {
            material.extension.glint_texture = glint_texture.clone();
        }
    }
    let paper_mesh = meshes.add(map_background_mesh());
    let picture_mesh = meshes.add(map_picture_mesh());

    let root = commands
        .spawn((
            Transform::IDENTITY,
            Visibility::Inherited,
            HandRoot,
            Name::new("hand_root"),
        ))
        .id();

    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        ClusterConfig::None,
        crate::renderer::systems::no_indirect_drawing(),
        Camera {
            order: HAND_CAMERA_ORDER,
            clear_color: ClearColorConfig::None,
            is_active: false,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: HAND_FOV_DEGREES.to_radians(),
            near: HAND_NEAR,
            far: HAND_FAR,
            ..default()
        }),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Transform::default(),
        RenderLayers::layer(HAND_LAYER),
        HandCamera,
        Name::new("hand_camera"),
    ));

    let hands = [Hand::Main, Hand::Off].map(|hand| {
        let name = match hand {
            Hand::Main => "main_hand",
            Hand::Off => "off_hand",
        };
        let pose = commands
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                HandNode,
                Name::new(format!("{name}_pose")),
            ))
            .id();
        let index = hand.index();
        let item = commands
            .spawn((
                Mesh3d::default(),
                MeshMaterial3d(assets.item[index].clone()),
                Transform::default(),
                Visibility::Inherited,
                RenderLayers::layer(HAND_LAYER),
                NoFrustumCulling,
                NotShadowCaster,
                HandNode,
                HandItemNode,
                Name::new(format!("{name}_item")),
            ))
            .id();
        commands.entity(pose).add_child(item);

        let side = hand.is_left(false) as usize;
        let arm = commands
            .spawn((
                Mesh3d(assets.arm[side].clone()),
                MeshMaterial3d(assets.base.clone()),
                Transform::default(),
                Visibility::Hidden,
                RenderLayers::layer(HAND_LAYER),
                NoFrustumCulling,
                NotShadowCaster,
                HandNode,
                Name::new(format!("{name}_arm")),
            ))
            .id();
        let sleeve = commands
            .spawn((
                Mesh3d(assets.sleeve[side].clone()),
                MeshMaterial3d(assets.overlay.clone()),
                Transform::default(),
                Visibility::Inherited,
                RenderLayers::layer(HAND_LAYER),
                NoFrustumCulling,
                NotShadowCaster,
                Name::new(format!("{name}_sleeve")),
            ))
            .id();
        commands.entity(arm).add_child(sleeve);

        let map = commands
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                HandNode,
                Name::new(format!("{name}_map")),
            ))
            .id();
        let mut map_piece = |mesh: Handle<Mesh>, material: Handle<HandMaterial>, part: &str| {
            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::default(),
                    Visibility::Inherited,
                    RenderLayers::layer(HAND_LAYER),
                    NoFrustumCulling,
                    NotShadowCaster,
                    HandNode,
                    HandMapNode,
                    Name::new(format!("{name}_map_{part}")),
                ))
                .id();
            commands.entity(map).add_child(entity);
            entity
        };
        let map_background = map_piece(
            paper_mesh.clone(),
            assets.map_background.clone(),
            "background",
        );
        let map_picture = map_piece(
            picture_mesh.clone(),
            assets.map_picture[index].clone(),
            "picture",
        );
        let map_decorations = map_piece(
            meshes.add(decoration_mesh(&[])),
            assets.decorations.clone(),
            "decorations",
        );
        commands.entity(pose).add_child(map);
        commands.entity(root).add_children(&[pose, arm]);

        HandEntities {
            pose,
            item,
            arm,
            sleeve,
            map,
            map_background,
            map_picture,
            map_decorations,
        }
    });

    commands.insert_resource(HandRig::new(hands));
    commands.insert_resource(assets);
}

fn light_hand(
    lightmap: &crate::renderer::systems::LightmapState,
    eye: [f64; 3],
    #[cfg(feature = "builtin_shaders")] daylight: Option<&crate::renderer::terrain::Daylight>,
    assets: &HandAssets,
    materials: &mut Assets<HandMaterial>,
) {
    let cell = crate::renderer::lightmap::entity_light(&lightmap.current, eye, false);

    #[cfg(not(feature = "builtin_shaders"))]
    let lit = cell.color;

    #[cfg(feature = "builtin_shaders")]
    let lit = if !crate::renderer::terrain::builtin_shaders_enabled() {
        cell.color
    } else {
        use crate::renderer::terrain::BLOCK_GAIN;
        let sky = Vec3::from(cell.sky);
        let mut sum = Vec3::from(cell.floor) + Vec3::from(cell.block) * BLOCK_GAIN;
        if let Some(day) = daylight {
            let n_o_l = day.to_light.y.max(0.0);
            sum += sky * (day.ambient + day.sun * n_o_l);
        }
        sum.to_array()
    };

    let want = Color::linear_rgb(lit[0], lit[1], lit[2]);
    for handle in [
        &assets.base,
        &assets.overlay,
        &assets.map_background,
        &assets.map_checkerboard,
        &assets.map_picture[0],
        &assets.map_picture[1],
        &assets.decorations,
    ] {
        if materials.get(handle).map(|m| m.base.base_color) == Some(want) {
            continue;
        }
        if let Some(material) = materials.get_mut(handle) {
            material.base.base_color = want;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_hands(
    frame_view: Res<crate::renderer::frame_view::FrameView>,
    view: (
        Res<crate::renderer::input::CameraView>,
        Res<crate::gui::GuiState>,
    ),
    rig: Option<ResMut<HandRig>>,
    hand_assets: Option<Res<HandAssets>>,
    mut item_assets: ResMut<ItemAssets>,
    map_textures: Res<crate::renderer::maps::MapTextures>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut hand_materials: ResMut<Assets<HandMaterial>>,
    lightmap: Res<crate::renderer::systems::LightmapState>,
    #[cfg(feature = "builtin_shaders")] daylight: Option<Res<crate::renderer::terrain::Daylight>>,
    mut nodes: NodeQuery,
    mut item_nodes: Query<&mut Mesh3d, With<HandItemNode>>,
    mut map_nodes: Query<
        (&mut Mesh3d, &mut MeshMaterial3d<HandMaterial>),
        (With<HandMapNode>, Without<HandItemNode>),
    >,
) {
    let (camera_view, gui) = view;
    crate::prof_span!("render:sync_hands");
    #[cfg(feature = "budget")]
    let _t = crate::diag::budget::timed(crate::diag::budget::Slot::Hand);
    let Some(mut rig) = rig else { return };

    let sample = &frame_view.local;
    let partial = frame_view.partial;
    let slot = frame_view.hotbar_selected;
    let spectator = frame_view.gamemode == Gamemode::Spectator;
    let item_used = frame_view.item_used;
    let live_pitch = -frame_view.camera_pitch;
    let live_yaw = wrap_degrees(-frame_view.camera_yaw - 180.0);
    let eye = frame_view.tick_eye();

    if let Some(hand_assets) = hand_assets.as_deref() {
        #[cfg(feature = "builtin_shaders")]
        light_hand(
            &lightmap,
            eye,
            daylight.as_deref(),
            hand_assets,
            &mut hand_materials,
        );
        #[cfg(not(feature = "builtin_shaders"))]
        light_hand(&lightmap, eye, hand_assets, &mut hand_materials);
    }

    let scoping = sample.using_item
        && use_animation(
            &held(
                sample,
                if sample.using_offhand {
                    Hand::Off
                } else {
                    Hand::Main
                },
            )
            .id,
        ) == UseAnimation::Spyglass;
    let view_yaw = wrap_degrees(sample.body_yaw + sample.head_yaw);
    advance_ticked_state(
        &mut rig,
        sample,
        slot,
        view_yaw,
        item_used,
        frame_view.hands_busy,
    );
    let view_lag_x = rig.x_bob.lag(live_pitch, partial);
    let view_lag_y = rig.y_bob.lag(live_yaw, partial);

    if camera_view.detached() || spectator || scoping || sample.sleeping || gui.hide_gui {
        for hand in [Hand::Main, Hand::Off] {
            hide(&mut nodes, &rig.hands[hand.index()].nodes);
        }
        return;
    }

    let selection = which_hands_to_render(sample);
    let arm_shown = !frame_view.local_body.invisible;
    let map_in_both_hands = rig.hands[Hand::Main.index()].shown.map_id.is_some()
        && rig.hands[Hand::Off.index()].shown.is_empty();
    for hand in [Hand::Off, Hand::Main] {
        let index = hand.index();
        if !match hand {
            Hand::Main => selection.main,
            Hand::Off => selection.off,
        } {
            hide(&mut nodes, &rig.hands[index].nodes);
            continue;
        }

        let frame = HandFrame {
            hand,
            main_arm_left: sample.main_arm_left,
            item: rig.hands[index].shown.clone(),
            attack: if (hand == Hand::Off) == sample.attack_left {
                sample.attack_time
            } else {
                0.0
            },
            inverse_arm_height: item_assets.swap_animation_scale(rig.hands[index].shown.id)
                * rig.hands[index].ramp.inverse_arm_height(partial),
            using: sample.using_item && sample.using_offhand == (hand == Hand::Off),
            ticks_using: sample.ticks_using_item - 1.0,
            ticks_since_kinetic_hit: sample.ticks_since_kinetic_hit,
            spin_attack: sample.spin_attack,
            view_lag_x,
            view_lag_y,
            view_x_rot: live_pitch,
        };

        if frame.item.is_empty() {
            set_visible(&mut nodes, rig.hands[index].nodes.pose, false);
            if hand == Hand::Main {
                let chain = player_arm(&frame);
                let (arm_transform, _) = chain.transforms();
                write(
                    &mut nodes,
                    rig.hands[index].nodes.arm,
                    arm_transform,
                    arm_shown,
                );
            } else {
                set_visible(&mut nodes, rig.hands[index].nodes.arm, false);
            }
            continue;
        }
        if let Some(map_id) = frame.item.map_id {
            let (pose_node, item_node, arm_node, map_node) = {
                let hands = &rig.hands[index].nodes;
                (hands.pose, hands.item, hands.arm, hands.map)
            };
            set_visible(&mut nodes, item_node, false);
            let chain = if hand == Hand::Main && map_in_both_hands {
                let base = two_handed_map_base(&frame);
                for arm in [Hand::Main, Hand::Off] {
                    let (transform, _) = map_hand(&base, arm, frame.main_arm_left).transforms();
                    write(
                        &mut nodes,
                        rig.hands[arm.index()].nodes.arm,
                        transform,
                        arm_shown,
                    );
                }
                two_handed_map(&base, frame.attack)
            } else {
                let (arm_transform, _) = one_handed_map_arm(&frame).transforms();
                write(&mut nodes, arm_node, arm_transform, arm_shown);
                one_handed_map(&frame)
            };
            let (pose_transform, map_transform) = chain.transforms();
            write(&mut nodes, pose_node, pose_transform, true);
            write(&mut nodes, map_node, map_transform, true);
            bind_map(
                &mut rig.hands[index],
                index,
                map_id,
                &map_textures,
                hand_assets.as_deref(),
                &mut meshes,
                &mut hand_materials,
                &mut nodes,
                &mut map_nodes,
            );
            continue;
        }
        let state = &mut rig.hands[index];
        set_visible(&mut nodes, state.nodes.arm, false);
        set_visible(&mut nodes, state.nodes.map, false);
        set_visible(&mut nodes, state.nodes.item, true);

        let cached = state
            .uploaded
            .as_ref()
            .filter(|up| {
                up.using == frame.using
                    && up.main_arm_left == frame.main_arm_left
                    && up.item.same_model(&frame.item)
            })
            .map(|up| up.transform);
        let display = match cached {
            Some(display) => display,
            None => {
                let key = frame.item.model_key();
                let gpu = item_assets.get(
                    &key,
                    hand.display_context(frame.main_arm_left).name(),
                    frame.using,
                    &mut meshes,
                    &mut images,
                );
                let display = gpu.map(|gpu| display_transform(gpu, hand, frame.main_arm_left));
                if let Some(gpu) = gpu
                    && let Ok(mut mesh) = item_nodes.get_mut(state.nodes.item)
                {
                    mesh.0 = gpu.mesh.clone();
                    if let Some(assets) = hand_assets.as_deref() {
                        let handle = &assets.item[index];
                        let stale = hand_materials.get(handle).is_some_and(|m| {
                            m.base.base_color_texture.as_ref() != Some(&gpu.texture)
                        });
                        if stale && let Some(material) = hand_materials.get_mut(handle) {
                            material.base.base_color_texture = Some(gpu.texture.clone());
                        }
                    }
                }
                state.uploaded = Some(UploadedItem {
                    item: frame.item.clone(),
                    using: frame.using,
                    main_arm_left: frame.main_arm_left,
                    transform: display,
                });
                display
            }
        };
        if let Some(assets) = hand_assets.as_deref() {
            let handle = &assets.item[index];
            let stale = hand_materials.get(handle).and_then(|m| {
                let glint = frame.item.glint && m.extension.glint_texture.is_some();
                let want = super::glint::scale(glint, m.base.base_color_texture.as_ref(), &images);
                (m.extension.glint != want).then_some(want)
            });
            if let Some(want) = stale
                && let Some(material) = hand_materials.get_mut(handle)
            {
                material.extension.glint = want;
            }
        }
        let Some(display) = display else {
            set_visible(&mut nodes, state.nodes.pose, false);
            continue;
        };

        let mut chain = arm_with_item(&frame);
        apply_display_transform(&mut chain, &display, frame.is_left());
        let (pose_transform, item_transform) = chain.transforms();
        write(&mut nodes, state.nodes.pose, pose_transform, true);
        write(&mut nodes, state.nodes.item, item_transform, true);
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_map(
    state: &mut HandState,
    index: usize,
    map_id: i32,
    map_textures: &crate::renderer::maps::MapTextures,
    assets: Option<&HandAssets>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<HandMaterial>,
    nodes: &mut NodeQuery,
    map_nodes: &mut Query<
        (&mut Mesh3d, &mut MeshMaterial3d<HandMaterial>),
        (With<HandMapNode>, Without<HandItemNode>),
    >,
) {
    let Some(assets) = assets else { return };
    let hands = &state.nodes;
    let (background, picture, decorations) = (
        hands.map_background,
        hands.map_picture,
        hands.map_decorations,
    );
    let image = map_textures.image(map_id);

    if let Ok((_, mut material)) = map_nodes.get_mut(background) {
        let want = match image {
            Some(_) => &assets.map_checkerboard,
            None => &assets.map_background,
        };
        if material.0 != *want {
            material.0 = want.clone();
        }
    }

    let Some(image) = image else {
        set_visible(nodes, picture, false);
        set_visible(nodes, decorations, false);
        state.map_bound = None;
        state.decorations_built = None;
        return;
    };
    set_visible(nodes, picture, true);
    if state.map_bound.as_ref() != Some(&image) {
        if let Some(material) = materials.get_mut(&assets.map_picture[index]) {
            material.base.base_color_texture = Some(image.clone());
        }
        state.map_bound = Some(image);
    }

    let (list, revision) = map_textures.decorations(map_id).unwrap_or((&[], 0));
    set_visible(nodes, decorations, !list.is_empty());
    if state.decorations_built != Some((map_id, revision)) {
        if let Ok((mut mesh, _)) = map_nodes.get_mut(decorations) {
            mesh.0 = meshes.add(decoration_mesh(list));
        }
        state.decorations_built = Some((map_id, revision));
    }
}

fn held(s: &AnimSample, hand: Hand) -> &HeldItem {
    match hand {
        Hand::Main => &s.main_hand,
        Hand::Off => &s.off_hand,
    }
}

fn display_transform(gpu: &ItemGpu, hand: Hand, main_arm_left: bool) -> ItemTransform {
    gpu.for_context(hand.display_context(main_arm_left))
}

fn advance_ticked_state(
    rig: &mut HandRig,
    sample: &AnimSample,
    slot: u8,
    view_yaw: f32,
    item_used: [u32; 2],
    hands_busy: bool,
) {
    let tick = sample.age_ticks.floor();
    let steps = match rig.last_tick {
        Some(last) => ((tick - last).max(0.0) as u32).min(20),
        None => {
            for hand in [Hand::Main, Hand::Off] {
                let state = &mut rig.hands[hand.index()];
                state.shown = held(sample, hand).clone();
                state.ramp = EquipRamp {
                    height: 1.0,
                    old: 1.0,
                };
                state.item_used_seen = item_used[hand.index()];
            }
            rig.shown_slot = slot;
            rig.x_bob = ViewBob {
                angle: sample.pitch,
                old: sample.pitch,
            };
            rig.y_bob = ViewBob {
                angle: view_yaw,
                old: view_yaw,
            };
            0
        }
    };
    rig.last_tick = Some(tick);

    for _ in 0..steps {
        rig.x_bob.advance(sample.pitch);
        rig.y_bob.advance(view_yaw);
        for hand in [Hand::Main, Hand::Off] {
            let next = held(sample, hand);
            let shown_slot = rig.shown_slot;
            let state = &mut rig.hands[hand.index()];
            let changed = state.shown.id != next.id || (hand == Hand::Main && shown_slot != slot);
            if !changed {
                state.shown = next.clone();
            }
            let target = if hands_busy || changed { 0.0 } else { 1.0 };
            state.ramp.advance(target);
            if state.ramp.hidden_enough() {
                state.shown = next.clone();
                if hand == Hand::Main {
                    rig.shown_slot = slot;
                }
            }
        }
    }

    for hand in [Hand::Main, Hand::Off] {
        let index = hand.index();
        let state = &mut rig.hands[index];
        if state.item_used_seen != item_used[index] {
            state.item_used_seen = item_used[index];
            state.ramp.height = 0.0;
        }
    }
}

fn hide(nodes: &mut NodeQuery, hand: &HandEntities) {
    set_visible(nodes, hand.pose, false);
    set_visible(nodes, hand.arm, false);
}

fn set_visible(nodes: &mut NodeQuery, entity: Entity, visible: bool) {
    if let Ok((_, mut visibility)) = nodes.get_mut(entity) {
        let wanted = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

fn write(nodes: &mut NodeQuery, entity: Entity, transform: Transform, visible: bool) {
    if let Ok((mut current, mut visibility)) = nodes.get_mut(entity) {
        *current = transform;
        let wanted = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(hand: Hand, id: &'static str, attack: f32) -> HandFrame {
        HandFrame {
            hand,
            item: HeldItem {
                id,
                count: 1,
                ..HeldItem::default()
            },
            attack,
            inverse_arm_height: 0.0,
            using: false,
            ticks_using: 0.0,
            ticks_since_kinetic_hit: 0.0,
            spin_attack: false,
            view_lag_x: 0.0,
            view_lag_y: 0.0,
            view_x_rot: 0.0,
            main_arm_left: false,
        }
    }

    fn left_handed_frame(hand: Hand, id: &'static str, attack: f32) -> HandFrame {
        HandFrame {
            main_arm_left: true,
            ..frame(hand, id, attack)
        }
    }

    fn map_frame(hand: Hand, x_rot: f32) -> HandFrame {
        let mut f = frame(hand, "filled_map", 0.0);
        f.item.map_id = Some(1);
        f.view_x_rot = x_rot;
        f
    }

    fn item_origin(chain: &Chain) -> Vec3 {
        let (pose, item) = chain.transforms();
        (pose.compute_affine() * item.compute_affine())
            .translation
            .into()
    }

    #[test]
    fn hand_equip_ramp_rises_in_tick_steps() {
        let mut ramp = EquipRamp::default();
        ramp.advance(1.0);
        assert!((ramp.height - 0.4).abs() < 1e-6, "{}", ramp.height);
        ramp.advance(1.0);
        assert!((ramp.height - 0.8).abs() < 1e-6, "{}", ramp.height);
        ramp.advance(1.0);
        assert!((ramp.height - 1.0).abs() < 1e-6, "{}", ramp.height);
        assert!(ramp.inverse_arm_height(1.0).abs() < 1e-6);
        assert!((ramp.inverse_arm_height(0.5) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn hand_equip_ramp_drops_on_a_swap() {
        let mut ramp = EquipRamp {
            height: 1.0,
            old: 1.0,
        };
        ramp.advance(0.0);
        assert!((ramp.height - 0.6).abs() < 1e-6, "{}", ramp.height);
        ramp.advance(0.0);
        assert!(!ramp.hidden_enough(), "0.2 is still on screen");
        ramp.advance(0.0);
        assert!(ramp.hidden_enough());
        let mut chain = Chain::new();
        apply_item_arm_transform(&mut chain, 1.0, ramp.inverse_arm_height(1.0));
        assert!((item_origin(&chain).y - (ITEM_POS_Y + ITEM_HEIGHT_SCALE)).abs() < 1e-6);
    }

    #[test]
    fn hand_rest_pose_is_the_vanilla_offset() {
        let chain = arm_with_item(&frame(Hand::Main, "diamond_sword", 0.0));
        let origin = item_origin(&chain);
        assert!((origin - Vec3::new(ITEM_POS_X, ITEM_POS_Y, ITEM_POS_Z)).length() < 1e-6);
        let off = item_origin(&arm_with_item(&frame(Hand::Off, "diamond_sword", 0.0)));
        assert!((off - Vec3::new(-ITEM_POS_X, ITEM_POS_Y, ITEM_POS_Z)).length() < 1e-6);
    }

    #[test]
    fn a_left_main_hand_mirrors_both_hands() {
        let right_main = item_origin(&arm_with_item(&frame(Hand::Main, "diamond_sword", 0.0)));
        let left_main = item_origin(&arm_with_item(&left_handed_frame(
            Hand::Main,
            "diamond_sword",
            0.0,
        )));
        assert!(
            (left_main - Vec3::new(-right_main.x, right_main.y, right_main.z)).length() < 1e-6,
            "{right_main} vs {left_main}"
        );

        let left_off = item_origin(&arm_with_item(&left_handed_frame(
            Hand::Off,
            "diamond_sword",
            0.0,
        )));
        assert!(
            left_off.x > 0.0 && left_main.x < 0.0,
            "{left_off} {left_main}"
        );
    }

    #[test]
    fn arm_def_follows_the_main_arm() {
        assert_eq!(arm_def(Hand::Main, false).tex, RIGHT_ARM.tex);
        assert_eq!(arm_def(Hand::Off, false).tex, LEFT_ARM.tex);
        assert_eq!(arm_def(Hand::Main, true).tex, LEFT_ARM.tex);
        assert_eq!(arm_def(Hand::Off, true).tex, RIGHT_ARM.tex);
    }

    #[test]
    fn hand_swing_transform_at_quarter_progress() {
        let chain = arm_with_item(&frame(Hand::Main, "diamond_sword", 0.25));
        let expected = Vec3::new(
            ITEM_POS_X + ITEM_SWING_X_POS_SCALE,
            ITEM_POS_Y,
            ITEM_POS_Z + ITEM_SWING_Z_POS_SCALE * (0.25 * PI).sin(),
        );
        assert!(
            (item_origin(&chain) - expected).length() < 1e-5,
            "{}",
            item_origin(&chain)
        );

        let (pose, _) = chain.transforms();
        let swung = pose.rotation * Vec3::Z;
        let unswung = arm_with_item(&frame(Hand::Main, "diamond_sword", 0.0))
            .transforms()
            .0
            .rotation
            * Vec3::Z;
        assert!(
            swung.angle_between(unswung) > 1.0,
            "the swing must actually turn the item"
        );
    }

    #[test]
    fn hand_swing_returns_to_rest_at_both_ends() {
        for attack in [0.0_f32, 1.0] {
            let origin = item_origin(&arm_with_item(&frame(Hand::Main, "diamond_sword", attack)));
            assert!(
                (origin - Vec3::new(ITEM_POS_X, ITEM_POS_Y, ITEM_POS_Z)).length() < 1e-6,
                "attack {attack} left the item off its rest position"
            );
        }
    }

    #[test]
    fn hand_eat_transform_ends_where_it_started() {
        let mut chain = Chain::new();
        apply_eat_transform(
            &mut chain,
            1.0,
            DEFAULT_CONSUME_TICKS,
            DEFAULT_CONSUME_TICKS,
        );
        assert!(item_origin(&chain).length() < 1e-6);

        let mut mid = Chain::new();
        apply_eat_transform(
            &mut mid,
            1.0,
            DEFAULT_CONSUME_TICKS / 2.0,
            DEFAULT_CONSUME_TICKS,
        );
        let origin = item_origin(&mid);
        assert!(origin.x > 0.5, "{origin}");
        assert!(origin.y < 0.0, "{origin}");
    }

    #[test]
    fn hand_bow_draw_stretches_and_turns() {
        let mut f = frame(Hand::Main, "bow", 0.0);
        f.using = true;
        f.ticks_using = 21.0;
        let chain = arm_with_item(&f);
        let (pose, item) = chain.transforms();
        assert!(pose.scale.z > 1.1, "{}", pose.scale.z);
        assert!((pose.scale.x - 1.0).abs() < 1e-6);
        assert!(item.rotation.angle_between(Quat::IDENTITY) > 0.1);
    }

    #[test]
    fn hand_crossbow_hold_only_offsets_the_loaded_main_hand() {
        let mut loaded = frame(Hand::Main, "crossbow", 0.0);
        loaded.item.charged = true;
        let charged = item_origin(&arm_with_item(&loaded));
        let empty = item_origin(&arm_with_item(&frame(Hand::Main, "crossbow", 0.0)));
        assert!((charged - empty).length() > 0.5, "{charged} vs {empty}");
    }

    #[test]
    fn hand_selection_drops_the_hand_that_is_not_aiming() {
        let mut s = AnimSample::default();
        assert_eq!(which_hands_to_render(&s), HandRenderSelection::BOTH);

        s.main_hand = HeldItem {
            id: "bow",
            count: 1,
            ..HeldItem::default()
        };
        s.using_item = true;
        assert_eq!(which_hands_to_render(&s), HandRenderSelection::MAIN_ONLY);

        s.using_offhand = true;
        s.off_hand = HeldItem {
            id: "bow",
            count: 1,
            ..HeldItem::default()
        };
        assert_eq!(which_hands_to_render(&s), HandRenderSelection::OFF_ONLY);

        let mut idle = AnimSample::default();
        idle.main_hand = HeldItem {
            id: "crossbow",
            charged: true,
            count: 1,
            ..HeldItem::default()
        };
        assert_eq!(which_hands_to_render(&idle), HandRenderSelection::MAIN_ONLY);
    }

    #[test]
    fn hand_display_transform_mirrors_for_the_left_hand() {
        let t = ItemTransform {
            rotation: [0.0, 90.0, 0.0],
            translation: [0.125, 0.25, 0.0625],
            scale: [0.68, 0.68, 0.68],
        };
        let mut right = Chain::new();
        apply_display_transform(&mut right, &t, false);
        let mut left = Chain::new();
        apply_display_transform(&mut left, &t, true);
        assert!(
            (item_origin(&right).x + item_origin(&left).x).abs() < 1e-6,
            "{} vs {}",
            item_origin(&right).x,
            item_origin(&left).x
        );
    }

    #[test]
    fn hand_use_timers_match_the_vanilla_countdown() {
        let duration = DEFAULT_CONSUME_TICKS;
        for (k, partial) in [(1u32, 0.0_f32), (1, 0.5), (7, 0.25), (32, 0.0)] {
            let mut f = frame(Hand::Main, "potion", 0.0);
            f.ticks_using = (k - 1) as f32 + partial;
            let remaining = duration - k as f32;
            assert!(
                (f.curr_usage_time(duration) - (remaining - partial + 1.0)).abs() < 1e-5,
                "curr_usage_time at k={k} partial={partial}"
            );
            assert!(
                (f.time_held() - (duration - (remaining - partial + 1.0))).abs() < 1e-5,
                "time_held at k={k} partial={partial}"
            );
            assert!((f.use_remaining_ticks(duration) - remaining).abs() < 1e-5);
            assert!((f.frame_interp() - partial).abs() < 1e-5);
        }
    }

    #[test]
    fn hand_bow_reaches_full_draw_after_twenty_ticks() {
        let power = |ticks_using: f32| {
            let mut f = frame(Hand::Main, "bow", 0.0);
            f.using = true;
            f.ticks_using = ticks_using;
            let linear = f.time_held() / 20.0;
            ((linear * linear + linear * 2.0) / 3.0).min(1.0)
        };
        assert!((power(20.0) - 1.0).abs() < 1e-6);
        assert!(power(19.0) < 1.0);
        assert!(power(0.0).abs() < 1e-6);
    }

    #[test]
    fn hand_brush_sweep_runs_off_the_remaining_ticks() {
        let angle = |ticks_using: f32| {
            let mut f = frame(Hand::Main, "brush", 0.0);
            f.using = true;
            f.ticks_using = ticks_using;
            let mut c = Chain::new();
            apply_brush_transform(
                &mut c,
                Hand::Main,
                f.main_arm_left,
                f.use_remaining_ticks(BRUSH_USE_DURATION_TICKS),
                f.frame_interp(),
            );
            c.transforms().0.rotation
        };
        let top = angle(10.0);
        assert!(top.angle_between(angle(20.0)) < 1e-4);
        assert!(top.angle_between(angle(30.0)) < 1e-4);
        assert!(
            top.angle_between(angle(15.0)) > 0.5,
            "the sweep has to move"
        );
    }

    #[test]
    fn hand_spear_use_ignores_the_equip_drop() {
        let mut f = frame(Hand::Main, "iron_spear", 0.0);
        f.using = true;
        f.ticks_using = 5.0;
        f.inverse_arm_height = 1.0;
        let origin = item_origin(&arm_with_item(&f));
        assert!(
            (origin - Vec3::new(ITEM_POS_X, ITEM_POS_Y, ITEM_POS_Z)).length() < 1e-6,
            "{origin}"
        );
    }

    #[test]
    fn hand_spear_charge_raises_the_spear() {
        let mut f = frame(Hand::Main, "iron_spear", 0.0);
        f.item.kinetic = Some(crate::items::kinetic::Kinetic {
            delay_ticks: 12,
            dismount_ticks: 50,
            knockback_ticks: 135,
            damage_ticks: 225,
            forward_movement: 0.38,
            ..Default::default()
        });
        f.using = true;
        let rest = Vec3::new(ITEM_POS_X, ITEM_POS_Y, ITEM_POS_Z);
        f.ticks_using = 0.0;
        let start = item_origin(&arm_with_item(&f));
        assert!((start - rest).length() < 1e-6, "{start}");
        f.ticks_using = 12.0;
        let raised = item_origin(&arm_with_item(&f));
        assert!((raised - rest).length() > 0.05, "{raised}");
    }

    #[test]
    fn hand_spear_attack_thrusts_and_returns() {
        for attack in [0.0_f32, 1.0] {
            let origin = item_origin(&arm_with_item(&frame(Hand::Main, "iron_spear", attack)));
            assert!(
                (origin - Vec3::new(ITEM_POS_X, ITEM_POS_Y, ITEM_POS_Z)).length() < 1e-5,
                "attack {attack} left the spear off its rest position"
            );
        }
        let spear = item_origin(&arm_with_item(&frame(Hand::Main, "iron_spear", 0.1)));
        let sword = item_origin(&arm_with_item(&frame(Hand::Main, "diamond_sword", 0.1)));
        assert!(spear.z > ITEM_POS_Z, "{spear}");
        assert!(sword.x < spear.x, "{sword} vs {spear}");
    }

    #[test]
    fn hand_view_lag_follows_the_camera_and_settles() {
        let mut bob = ViewBob {
            angle: 0.0,
            old: 0.0,
        };
        bob.advance(40.0);
        assert!((bob.angle - 20.0).abs() < 1e-5, "{}", bob.angle);
        assert!(
            (bob.lag(40.0, 1.0) - 2.0).abs() < 1e-5,
            "{}",
            bob.lag(40.0, 1.0)
        );
        for _ in 0..20 {
            bob.advance(40.0);
        }
        assert!(bob.lag(40.0, 1.0).abs() < 1e-3);

        let mut seam = ViewBob {
            angle: 179.0,
            old: 179.0,
        };
        seam.advance(-179.0);
        assert!((seam.angle - 180.0).abs() < 1e-4 || (seam.angle + 180.0).abs() < 1e-4);
        assert!(
            seam.lag(-179.0, 1.0).abs() < 0.2,
            "{}",
            seam.lag(-179.0, 1.0)
        );
    }

    #[test]
    fn hand_view_lag_turns_both_hands_alike() {
        let mut f = frame(Hand::Main, "diamond_sword", 0.0);
        let rest = item_origin(&arm_with_item(&f));
        f.view_lag_y = 10.0;
        let swayed = item_origin(&arm_with_item(&f));
        assert!((swayed.length() - rest.length()).abs() < 1e-5);
        assert!((swayed - rest).length() > 0.05, "{swayed} vs {rest}");

        let mut empty = frame(Hand::Main, "", 0.0);
        let straight = player_arm(&empty).transforms().0.translation;
        empty.view_lag_y = 10.0;
        let turned = player_arm(&empty).transforms().0.translation;
        assert!((turned.length() - straight.length()).abs() < 1e-5);
        assert!((turned - straight).length() > 0.05);
    }

    #[test]
    fn hand_bare_arm_lands_in_front_of_the_camera() {
        let (arm, _) = player_arm(&frame(Hand::Main, "", 0.0)).transforms();
        assert!(arm.translation.length() < 1.5, "{}", arm.translation);
        assert!(arm.translation.x > 0.0, "{}", arm.translation);
        assert!(arm.translation.z < 0.0, "{}", arm.translation);

        let (off, _) = player_arm(&frame(Hand::Off, "", 0.0)).transforms();
        assert!((off.translation.x + arm.translation.x).abs() < 1e-5);
        assert!((off.translation.y - arm.translation.y).abs() < 1e-5);
    }

    #[test]
    fn hand_swing_reaches_the_first_person_item() {
        use crate::renderer::anim::{AnimInput, HumanoidAnim};

        let mut anim = HumanoidAnim::default();
        let rest = item_origin(&arm_with_item(&frame(Hand::Main, "stone", 0.0)));

        anim.swing(false);
        let mut moved = false;
        for _ in 0..3 {
            anim.tick(AnimInput {
                health: 20.0,
                ..Default::default()
            });
            let sample = anim.sample(1.0);
            assert!(!sample.attack_left, "the main hand is the right arm");
            let attack = if (Hand::Main == Hand::Off) == sample.attack_left {
                sample.attack_time
            } else {
                0.0
            };
            let swung = item_origin(&arm_with_item(&frame(Hand::Main, "stone", attack)));
            moved |= (swung - rest).length() > 0.05;
        }
        assert!(
            moved,
            "a swing on the local anim state never moved the held item"
        );
    }

    #[test]
    fn hand_item_used_drops_the_equip_ramp() {
        let mut rig = HandRig::new(std::array::from_fn(|_| HandEntities {
            pose: Entity::PLACEHOLDER,
            item: Entity::PLACEHOLDER,
            arm: Entity::PLACEHOLDER,
            sleeve: Entity::PLACEHOLDER,
            map: Entity::PLACEHOLDER,
            map_background: Entity::PLACEHOLDER,
            map_picture: Entity::PLACEHOLDER,
            map_decorations: Entity::PLACEHOLDER,
        }));
        for state in &mut rig.hands {
            state.ramp = EquipRamp {
                height: 1.0,
                old: 1.0,
            };
        }
        rig.last_tick = Some(0.0);
        let sample = AnimSample {
            age_ticks: 1.0,
            ..Default::default()
        };

        advance_ticked_state(&mut rig, &sample, 0, 0.0, [1, 0], false);
        assert_eq!(rig.hands[0].ramp.height, 0.0);
        assert_eq!(
            rig.hands[0].ramp.old, 1.0,
            "the tick the use lands on keeps its old height"
        );
        assert_eq!(
            rig.hands[1].ramp.height, 1.0,
            "the offhand did not use anything"
        );

        let sample = AnimSample {
            age_ticks: 2.0,
            ..Default::default()
        };
        advance_ticked_state(&mut rig, &sample, 0, 0.0, [1, 0], false);
        assert!(
            rig.hands[0].ramp.height > 0.0,
            "the ramp must climb back out of view"
        );
    }

    #[test]
    fn hand_map_tilt_rises_as_the_player_looks_up() {
        assert_eq!(map_tilt(90.0), 0.0);
        assert!(map_tilt(49.5).abs() < 1e-6);
        assert!((map_tilt(-90.0) - 1.0).abs() < 1e-6);
        assert!((map_tilt(0.0) - 1.0).abs() < 1e-6);
        let mut previous = -1.0;
        for step in 0..=45 {
            let tilt = map_tilt(90.0 - step as f32 * 2.0);
            assert!(tilt >= previous, "tilt fell at {step}");
            previous = tilt;
        }
    }

    #[test]
    fn hand_two_handed_map_is_centred_between_two_arms() {
        let base = two_handed_map_base(&map_frame(Hand::Main, 90.0));
        let map = two_handed_map(&base, 0.0);
        let (pose, tail) = map.transforms();
        let centre = (pose.compute_affine() * tail.compute_affine()).transform_point3(Vec3::new(
            MAP_EDGE_PIXELS / 2.0,
            MAP_EDGE_PIXELS / 2.0,
            0.0,
        ));
        assert!(centre.x.abs() < 1e-5, "off centre: {centre}");
        assert!(centre.z < 0.0, "behind the eye: {centre}");

        let right = map_hand(&base, Hand::Main, false)
            .transforms()
            .0
            .translation;
        let left = map_hand(&base, Hand::Off, false).transforms().0.translation;
        assert!(right.x > 0.0 && left.x < 0.0, "{right} {left}");
        assert!((right.x + left.x).abs() < 1e-5, "the pair is not symmetric");
    }

    #[test]
    fn hand_one_handed_map_is_held_to_one_side() {
        let main = one_handed_map(&map_frame(Hand::Main, 0.0));
        let off = one_handed_map(&map_frame(Hand::Off, 0.0));
        let (main_pose, _) = main.transforms();
        let (off_pose, _) = off.transforms();
        assert!((main_pose.translation.x - 0.635).abs() < 1e-5);
        assert!((main_pose.translation.x + off_pose.translation.x).abs() < 1e-5);
        assert!((main_pose.translation.z - -0.75).abs() < 1e-5);
        let looking_up = one_handed_map(&map_frame(Hand::Main, -90.0));
        assert_eq!(looking_up.transforms().0.translation, main_pose.translation);
    }

    #[test]
    fn hand_decoration_mesh_has_one_quad_per_marker() {
        use crate::session::MapDecoration;

        let marker = |kind: u8| MapDecoration {
            kind,
            x: 0,
            y: 0,
            rot: 0,
        };
        assert_eq!(decoration_mesh(&[]).count_vertices(), 0);
        assert_eq!(decoration_mesh(&[marker(0)]).count_vertices(), 4);
        assert_eq!(
            decoration_mesh(&[marker(0), marker(34)]).count_vertices(),
            8
        );
        assert_eq!(decoration_mesh(&[marker(35)]).count_vertices(), 0);
    }

    #[test]
    fn hand_decoration_sits_at_the_map_centre() {
        use crate::session::MapDecoration;

        let mesh = decoration_mesh(&[MapDecoration {
            kind: 0,
            x: 0,
            y: 0,
            rot: 0,
        }]);
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => panic!("no positions"),
        };
        let xs: Vec<f32> = positions.iter().map(|p| p[0]).collect();
        let (min, max) = (
            xs.iter().copied().fold(f32::MAX, f32::min),
            xs.iter().copied().fold(f32::MIN, f32::max),
        );
        assert!((max - min - 8.0).abs() < 1e-5, "{min}..{max}");
        assert!(((min + max) / 2.0 - (MAP_EDGE_PIXELS / 2.0 - 0.5)).abs() < 1e-5);
    }

    fn hand_clip(p: Vec3, aspect: f32) -> Vec4 {
        let f = 1.0 / (HAND_FOV_DEGREES.to_radians() * 0.5).tan();
        let w = -p.z;
        Vec4::new(
            p.x * f / aspect,
            p.y * f,
            w * (1.0 - HAND_DEPTH_BAND) + HAND_DEPTH_BAND * HAND_NEAR,
            w,
        )
    }

    fn kept(clip: Vec4) -> bool {
        clip.z >= 0.0 && clip.z <= clip.w
    }

    #[test]
    fn hand_depth_stays_in_the_band_and_keeps_its_order() {
        let floor = 1.0 - HAND_DEPTH_BAND;
        let near = hand_clip(Vec3::new(0.0, 0.0, -HAND_NEAR), 16.0 / 9.0);
        assert!((near.z / near.w - 1.0).abs() < 1e-6, "{near}");

        let mut last = f32::INFINITY;
        let steps = 2000;
        for i in 1..=steps {
            let w = HAND_NEAR + (HAND_FAR - HAND_NEAR) * i as f32 / steps as f32;
            let clip = hand_clip(Vec3::new(0.3, -0.4, -w), 16.0 / 9.0);
            let depth = clip.z / clip.w;
            assert!(kept(clip), "w {w}: {clip}");
            assert!((floor..=1.0).contains(&depth), "w {w}: depth {depth}");
            if w <= 2.0 {
                assert!(depth < last, "w {w}: {depth} after {last}");
            } else {
                assert!(depth <= last + f32::EPSILON, "w {w}: {depth} after {last}");
            }
            last = depth;
        }
    }

    #[test]
    fn hand_geometry_inside_the_near_plane_is_clipped() {
        for w in [0.049, 0.01, 0.0, -0.5] {
            assert!(!kept(hand_clip(Vec3::new(0.0, 0.0, -w), 1.0)), "w {w}");
        }
    }

    #[test]
    fn hand_projection_uses_its_own_field_of_view() {
        let half = (HAND_FOV_DEGREES * 0.5).to_radians();
        let top = hand_clip(Vec3::new(0.0, half.tan(), -1.0), 2.0);
        assert!((top.y / top.w - 1.0).abs() < 1e-5, "{top}");
        let side = hand_clip(Vec3::new(2.0 * half.tan(), 0.0, -1.0), 2.0);
        assert!((side.x / side.w - 1.0).abs() < 1e-5, "{side}");
    }

    #[test]
    fn hand_band_sits_in_front_of_the_world() {
        let world_near = crate::renderer::systems::WORLD_NEAR;
        let floor = 1.0 - HAND_DEPTH_BAND;
        let reach = world_near / floor - world_near;
        assert!(reach < 0.002, "{reach}");
        assert!(world_near / (world_near + 0.002) < floor);
    }
}
