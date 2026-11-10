#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ArmPose {
    #[default]
    Empty,
    Item,
    Block,
    BowAndArrow,
    ThrowTrident,
    CrossbowCharge,
    CrossbowHold,
    Spyglass,
    TootHorn,
    Brush,
    Spear,
}

impl ArmPose {
    pub fn two_handed(self) -> bool {
        matches!(
            self,
            ArmPose::BowAndArrow | ArmPose::CrossbowCharge | ArmPose::CrossbowHold
        )
    }

    pub fn affects_offhand_pose(self) -> bool {
        matches!(
            self,
            ArmPose::BowAndArrow
                | ArmPose::ThrowTrident
                | ArmPose::CrossbowCharge
                | ArmPose::CrossbowHold
                | ArmPose::Spear
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UseAnimation {
    None,
    Eat,
    Drink,
    Block,
    Bow,
    Trident,
    Crossbow,
    Spyglass,
    TootHorn,
    Brush,
    Spear,
    Bundle,
}

const SPEARS: &[&str] = &[
    "copper_spear",
    "diamond_spear",
    "golden_spear",
    "iron_spear",
    "netherite_spear",
    "stone_spear",
    "wooden_spear",
];

const FOODS: &[&str] = &[
    "apple",
    "baked_potato",
    "beef",
    "beetroot",
    "beetroot_soup",
    "bread",
    "carrot",
    "chicken",
    "chorus_fruit",
    "cod",
    "cooked_beef",
    "cooked_chicken",
    "cooked_cod",
    "cooked_mutton",
    "cooked_porkchop",
    "cooked_rabbit",
    "cooked_salmon",
    "cookie",
    "dried_kelp",
    "enchanted_golden_apple",
    "glow_berries",
    "golden_apple",
    "golden_carrot",
    "melon_slice",
    "mushroom_stew",
    "mutton",
    "poisonous_potato",
    "porkchop",
    "potato",
    "pufferfish",
    "pumpkin_pie",
    "rabbit",
    "rabbit_stew",
    "rotten_flesh",
    "salmon",
    "spider_eye",
    "suspicious_stew",
    "sweet_berries",
    "tropical_fish",
];

const DRINKS: &[&str] = &["honey_bottle", "milk_bucket", "ominous_bottle", "potion"];

pub fn use_animation(id: &str) -> UseAnimation {
    match id {
        "shield" => UseAnimation::Block,
        "bow" => UseAnimation::Bow,
        "trident" => UseAnimation::Trident,
        "crossbow" => UseAnimation::Crossbow,
        "spyglass" => UseAnimation::Spyglass,
        "goat_horn" => UseAnimation::TootHorn,
        "brush" => UseAnimation::Brush,
        _ if DRINKS.contains(&id) => UseAnimation::Drink,
        _ if FOODS.contains(&id) => UseAnimation::Eat,
        _ if SPEARS.contains(&id) => UseAnimation::Spear,
        "bundle" => UseAnimation::Bundle,
        _ if id.ends_with("_bundle") => UseAnimation::Bundle,
        _ => UseAnimation::None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Armor {
    pub items: [Option<&'static str>; 4],
    pub foil: [bool; 4],
    pub trim: [Option<&'static str>; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeldItem {
    pub id: &'static str,
    pub charged: bool,
    pub count: u8,
    pub tint: Option<u32>,
    pub map_id: Option<i32>,
    pub layers: Option<Box<[crate::blockentities::banner::BannerLayer]>>,
    pub glint: bool,
    pub death_protection: bool,
    pub kinetic: Option<crate::items::kinetic::Kinetic>,
    pub swing_ticks: u8,
    pub rocket: bool,
    pub profile_skin: Option<ProfileSkin>,
    pub trim: u8,
    pub block_state: u8,
    pub has_lodestone: bool,
    pub lodestone: Option<[i32; 3]>,
    pub broken: bool,
    pub use_duration: i32,
    pub quick_charge: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProfileSkin {
    Url(std::sync::Arc<str>),
    Resource(std::sync::Arc<str>),
    Default(u8),
    Lookup { key: ProfileKey, fallback: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProfileKey {
    Name(std::sync::Arc<str>),
    Id(u128),
}

impl AnimSample {
    pub fn charging(&self) -> [Option<crate::items::kinetic::Kinetic>; 2] {
        charging_arms(
            self.using_item,
            self.using_offhand,
            self.main_arm_left,
            &self.main_hand,
            &self.off_hand,
        )
    }

    pub fn render_rot(&self) -> (f32, f32) {
        if self.layers.upside_down {
            (-self.pitch, -self.head_yaw)
        } else {
            (self.pitch, self.head_yaw)
        }
    }

    pub fn is_scoping(&self) -> bool {
        let held = if self.using_offhand {
            &self.off_hand
        } else {
            &self.main_hand
        };
        self.using_item && use_animation(held.id) == UseAnimation::Spyglass
    }
}

pub fn sway_charging(
    using_item: bool,
    using_offhand: bool,
    right: &HeldItem,
    left: &HeldItem,
) -> [Option<crate::items::kinetic::Kinetic>; 2] {
    match (using_item, using_offhand) {
        (false, _) => [None; 2],
        (true, false) => [right.kinetic, None],
        (true, true) => [None, left.kinetic],
    }
}

pub fn charging_arms(
    using_item: bool,
    using_offhand: bool,
    main_arm_left: bool,
    main_hand: &HeldItem,
    off_hand: &HeldItem,
) -> [Option<crate::items::kinetic::Kinetic>; 2] {
    if !using_item {
        return [None; 2];
    }
    let item = if using_offhand { off_hand } else { main_hand };
    if using_offhand != main_arm_left {
        [None, item.kinetic]
    } else {
        [item.kinetic, None]
    }
}

impl HeldItem {
    pub fn is_empty(&self) -> bool {
        self.id.is_empty()
    }

    pub fn model_key(&self) -> String {
        if let Some(layers) = &self.layers
            && !layers.is_empty()
        {
            return crate::blockentities::banner::model_key(self.id, layers);
        }
        match self.tint {
            Some(color) => crate::items::potions::tint_key(self.id, color),
            None => self.id.to_string(),
        }
    }

    pub fn same_model(&self, other: &HeldItem) -> bool {
        fn layers(item: &HeldItem) -> Option<&[crate::blockentities::banner::BannerLayer]> {
            item.layers.as_deref().filter(|l| !l.is_empty())
        }
        if self.id != other.id {
            return false;
        }
        match (layers(self), layers(other)) {
            (Some(a), Some(b)) => a == b,
            (None, None) => self.tint == other.tint,
            _ => false,
        }
    }
}

pub fn arm_pose_for(item: &HeldItem, using: bool, swinging: bool) -> ArmPose {
    if item.is_empty() {
        return ArmPose::Empty;
    }
    if !swinging && item.id == "crossbow" && item.charged {
        return ArmPose::CrossbowHold;
    }
    if using {
        match use_animation(&item.id) {
            UseAnimation::Block => return ArmPose::Block,
            UseAnimation::Bow => return ArmPose::BowAndArrow,
            UseAnimation::Trident => return ArmPose::ThrowTrident,
            UseAnimation::Crossbow => return ArmPose::CrossbowCharge,
            UseAnimation::Spyglass => return ArmPose::Spyglass,
            UseAnimation::TootHorn => return ArmPose::TootHorn,
            UseAnimation::Brush => return ArmPose::Brush,
            UseAnimation::Spear => return ArmPose::Spear,
            UseAnimation::None | UseAnimation::Eat | UseAnimation::Drink | UseAnimation::Bundle => {
            }
        }
    }
    if SPEARS.contains(&item.id) {
        return ArmPose::Spear;
    }
    ArmPose::Item
}

pub fn arm_poses(
    main: &HeldItem,
    off: &HeldItem,
    using_hand: Option<bool>,
    swinging: bool,
) -> (ArmPose, ArmPose) {
    let main_pose = arm_pose_for(main, using_hand == Some(false), swinging);
    let mut off_pose = arm_pose_for(off, using_hand == Some(true), swinging);
    if main_pose.two_handed() {
        off_pose = if off.is_empty() {
            ArmPose::Empty
        } else {
            ArmPose::Item
        };
    }
    (main_pose, off_pose)
}

#[derive(Clone, Debug, Default)]
pub struct AnimSample {
    pub walk_pos: f32,
    pub walk_speed: f32,
    pub body_yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
    pub attack_time: f32,
    pub attack_left: bool,
    pub swim_amount: f32,
    pub age_ticks: f32,
    pub death_time: f32,
    pub hurt_time: f32,
    pub hurt_dir: f32,
    pub has_red_overlay: bool,
    pub crouching: bool,
    #[allow(
        dead_code,
        reason = "no sprint pose in HumanoidModel.setupAnim; see the field comment"
    )]
    pub sprinting: bool,
    pub fall_flying: bool,
    pub fall_flying_scale: f32,
    pub flying_y_rot: Option<f32>,
    pub speed_value: f32,
    pub visually_swimming: bool,
    pub spin_attack: bool,
    pub passenger: bool,
    pub sleeping: bool,
    pub bed_orientation: Option<crate::blocks::direction::Direction>,
    pub using_item: bool,
    pub main_arm_left: bool,
    pub ticks_using_item: f32,
    pub right_arm_pose: ArmPose,
    pub left_arm_pose: ArmPose,
    pub main_hand: HeldItem,
    pub off_hand: HeldItem,
    pub armor: Armor,
    pub head: HeldItem,
    pub using_offhand: bool,
    pub ticks_since_kinetic_hit: f32,
    #[cfg(feature = "skins")]
    pub cape: [f32; 3],
    pub elytra: [f32; 3],
    pub layers: AvatarLayers,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AvatarLayers {
    pub id: i32,
    pub arrows: u32,
    pub stingers: u32,
    pub parrots: [Option<u32>; 2],
    pub extra_ears: bool,
    pub upside_down: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AnimInput {
    pub pos: [f64; 3],
    pub pitch: f32,
    pub head_yaw: f32,
    pub y_rot: f32,
    pub crouching: bool,
    pub sprinting: bool,
    pub fall_flying: bool,
    pub visually_swimming: bool,
    pub spin_attack: bool,
    pub sleeping: bool,
    pub bed_orientation: Option<crate::blocks::direction::Direction>,
    pub using_item: bool,
    pub using_offhand: bool,
    pub passenger: bool,
    pub health: f32,
    pub main_hand: HeldItem,
    pub off_hand: HeldItem,
    pub armor: Armor,
    pub head: HeldItem,
    pub main_arm_left: bool,
    pub interpolated: bool,
    #[cfg(feature = "skins")]
    pub on_ground: bool,
    pub layers: AvatarLayers,
}

#[derive(Clone, Debug, Default)]
pub struct HumanoidAnim {
    core: LivingCore,
    swim: SwimAmount,
    hurt_dir: f32,
    ticks: u32,
    use_ticks: u32,
    kinetic_hit_at: Option<u32>,

    #[cfg(feature = "skins")]
    cloak: [f64; 3],
    #[cfg(feature = "skins")]
    cloak_old: [f64; 3],
    #[cfg(feature = "skins")]
    bob: f32,
    #[cfg(feature = "skins")]
    bob_old: f32,
    #[cfg(feature = "skins")]
    walk_dist: f32,
    #[cfg(feature = "skins")]
    walk_dist_old: f32,
    fall_fly_ticks: u32,
    step: [f64; 3],
    step_old: [f64; 3],
    elytra: [f32; 3],
    elytra_old: [f32; 3],

    flags: AnimInput,
    started: bool,
}

impl HumanoidAnim {
    pub fn use_ticks(&self) -> u32 {
        self.use_ticks
    }

    pub fn held(&self) -> [&HeldItem; 2] {
        [&self.flags.main_hand, &self.flags.off_hand]
    }

    pub fn tick(&mut self, input: AnimInput) {
        if !self.started {
            self.started = true;
            self.core.start(
                input.pos,
                input.y_rot,
                input.pitch,
                input.head_yaw,
                input
                    .interpolated
                    .then(crate::util::interp::Interpolation::new),
            );
        }

        #[cfg(feature = "skins")]
        self.move_cloak();

        let self_move = self.handle_movement_packets(&input);

        self.core.latch();
        self.flags = input;
        self.ticks = self.ticks.wrapping_add(1);

        if let Some((pos, y_rot, pitch)) = self_move {
            self.core.pos = pos;
            self.core.y_rot = y_rot;
            self.core.x_rot = pitch;
        }

        self.core.step_interpolation();

        let alive = self.flags.health > 0.0;
        let swing_item = if self.core.swing_left() == self.flags.main_arm_left {
            &self.flags.main_hand
        } else {
            &self.flags.off_hand
        };
        self.core.set_swing_duration(swing_item.swing_ticks);
        self.core.tick_counters(!alive);
        self.swim.tick(self.flags.visually_swimming);

        if self.flags.passenger || !alive {
            self.core.walk.stop();
        } else {
            let target = crate::entities::feed::walk_target(
                azalea_registry::builtin::EntityKind::Player,
                self.core.pos_old,
                self.core.pos,
            );
            self.core.walk.update(target, 0.4, 1.0);
        }

        let step: [f64; 3] = std::array::from_fn(|i| self.core.pos[i] - self.core.pos_old[i]);

        #[cfg(feature = "skins")]
        {
            let (dx, dz) = (step[0] as f32, step[2] as f32);
            if !self.flags.interpolated {
                self.walk_dist_old = self.walk_dist;
                self.walk_dist += dx.hypot(dz) * 0.6;
            }
            if self.flags.passenger {
                self.bob_old = self.bob;
                self.bob = 0.0;
            } else {
                let target = if self.flags.on_ground && alive && !self.flags.visually_swimming {
                    dx.hypot(dz).min(0.1)
                } else {
                    0.0
                };
                self.bob_old = self.bob;
                self.bob += (target - self.bob) * 0.4;
            }
        }

        self.fall_fly_ticks = if self.flags.fall_flying {
            self.fall_fly_ticks.saturating_add(1)
        } else {
            0
        };
        self.step_old = self.step;
        self.step = step;

        self.elytra_old = self.elytra;
        crate::entities::models::humanoid::elytra::tick_angles(
            &mut self.elytra,
            self.flags.fall_flying,
            self.flags.crouching,
            step,
        );

        self.use_ticks = if self.flags.using_item {
            self.use_ticks + 1
        } else {
            0
        };

        self.core.turn_body();
    }

    fn handle_movement_packets(&mut self, input: &AnimInput) -> Option<([f64; 3], f32, f32)> {
        let deferred = self
            .core
            .handle_movement_packets(input.pos, input.y_rot, input.pitch);
        let lerp = self.core.interp.is_some();
        self.core.rotate_head(input.head_yaw, lerp);
        deferred
    }

    pub fn layers(&self) -> AvatarLayers {
        self.flags.layers
    }

    pub fn body_yaw_pair(&self) -> (f32, f32) {
        self.core.body_rot_pair()
    }

    pub fn swing(&mut self, left_arm: bool) {
        self.core.swing(left_arm);
    }

    pub fn set_swing_effect(&mut self, ticks: i32) {
        self.core.set_swing_effect(ticks);
    }

    pub fn swing_hand(&mut self, off_hand: bool) {
        self.core.swing(off_hand != self.flags.main_arm_left);
    }

    pub fn kinetic_hit(&mut self) -> Option<azalea_registry::builtin::SoundEvent> {
        if self.kinetic_hit_at.is_some_and(|at| {
            self.ticks.wrapping_sub(at) <= crate::items::kinetic::HIT_FEEDBACK_TICKS
        }) {
            return None;
        }
        self.kinetic_hit_at = Some(self.ticks);
        if !self.flags.using_item {
            return None;
        }
        let item = if self.flags.using_offhand {
            &self.flags.off_hand
        } else {
            &self.flags.main_hand
        };
        item.kinetic.and_then(|k| k.hit_sound)
    }

    pub fn hurt(&mut self) {
        self.core.hurt();
    }

    pub fn hurt_from(&mut self, yaw: f32) {
        self.hurt_dir = yaw;
        self.core.hurt();
    }

    pub fn reset_hurt(&mut self) {
        self.hurt_dir = 0.0;
        self.core.clear_hurt();
    }

    pub fn damage(&mut self) {
        self.core.damage();
    }

    pub fn position(&self, partial: f32) -> [f64; 3] {
        self.core.position(partial.clamp(0.0, 1.0))
    }

    pub fn seat_body(&self, scale: f64) -> crate::entities::feed::SeatBody {
        crate::entities::feed::SeatBody {
            pos: self.core.pos,
            y_rot: self.core.y_rot,
            scale,
            width: 0.6 * scale,
            stand: 0.0,
        }
    }

    pub fn set_seated_pos(&mut self, pos: [f64; 3]) {
        self.core.seat(pos);
    }

    #[cfg(feature = "skins")]
    fn move_cloak(&mut self) {
        const TELEPORT: f64 = 10.0;

        self.cloak_old = self.cloak;
        for axis in 0..3 {
            let delta = self.core.pos[axis] - self.cloak[axis];
            if delta.abs() > TELEPORT {
                self.cloak[axis] = self.core.pos[axis];
                self.cloak_old[axis] = self.cloak[axis];
            } else {
                self.cloak[axis] += delta * 0.25;
            }
        }
    }

    #[cfg(feature = "skins")]
    fn cape(&self, t: f32) -> [f32; 3] {
        let lerp = |old: f64, new: f64| crate::util::mth::lerp_f64(t, old, new);
        let c = &self.core;
        let dx = lerp(self.cloak_old[0], self.cloak[0]) - lerp(c.pos_old[0], c.pos[0]);
        let dy = lerp(self.cloak_old[1], self.cloak[1]) - lerp(c.pos_old[1], c.pos[1]);
        let dz = lerp(self.cloak_old[2], self.cloak[2]) - lerp(c.pos_old[2], c.pos[2]);

        let body_yaw = rot_lerp(t, c.body_rot_old, c.body_rot) * DEG_TO_RAD;
        let (forward_x, forward_z) = (body_yaw.sin() as f64, -body_yaw.cos() as f64);

        let fly_scale = self.fall_flying_scale(t);

        let mut flap = ((dy * 10.0) as f32).clamp(-6.0, 32.0);
        let lean = ((dx * forward_x + dz * forward_z) as f32 * 100.0 * (1.0 - fly_scale))
            .clamp(0.0, 150.0);
        let lean2 = ((dx * forward_z - dz * forward_x) as f32 * 100.0).clamp(-20.0, 20.0);

        let walk = self.walk_dist_old + (self.walk_dist - self.walk_dist_old) * t;
        let bob = self.bob_old + (self.bob - self.bob_old) * t;
        flap += (walk * 6.0).sin() * 32.0 * bob;
        [flap, lean, lean2]
    }

    fn fall_flying_scale(&self, t: f32) -> f32 {
        let fly = self.fall_fly_ticks as f32 + t;
        (fly * fly / 100.0).clamp(0.0, 1.0)
    }

    fn flying_y_rot(&self, t: f32) -> Option<f32> {
        const MIN_HORIZONTAL_SQR: f64 = 9.999_999_747_378_752e-6;

        let c = &self.core;
        let (yaw, pitch) = if self.flags.interpolated {
            (
                rot_lerp(t, c.head_rot_old, c.head_rot),
                crate::util::mth::lerp(t, c.x_rot_old, c.x_rot),
            )
        } else {
            (c.head_rot, c.x_rot)
        };
        let look = azalea::entity::view_vector(azalea::entity::LookDirection::new(yaw, pitch));
        let movement: [f64; 3] =
            std::array::from_fn(|i| crate::util::mth::lerp_f64(t, self.step_old[i], self.step[i]));

        let move_sqr = movement[0] * movement[0] + movement[2] * movement[2];
        let look_sqr = look.x * look.x + look.z * look.z;
        if move_sqr <= MIN_HORIZONTAL_SQR || look_sqr <= MIN_HORIZONTAL_SQR {
            return None;
        }
        let (move_len, look_len) = (move_sqr.sqrt(), look_sqr.sqrt());
        let dot = (movement[0] / move_len) * (look.x / look_len)
            + (movement[2] / move_len) * (look.z / look_len);
        let sign = movement[0] * look.z - movement[2] * look.x;
        let sign = if sign == 0.0 { 0.0 } else { sign.signum() };
        Some((sign * dot.abs().min(1.0).acos()) as f32)
    }

    fn speed_value(&self) -> f32 {
        if !self.flags.fall_flying {
            return 1.0;
        }
        let [x, y, z] = self.step;
        let ratio = (x * x + y * y + z * z) as f32 / 0.2;
        (ratio * ratio * ratio).max(1.0)
    }

    pub fn sample(&self, partial: f32) -> AnimSample {
        let t = partial.clamp(0.0, 1.0);
        let c = &self.core;
        let body_yaw = rot_lerp(t, c.body_rot_old, c.body_rot);
        let head_yaw = rot_lerp(t, c.head_rot_old, c.head_rot);
        let using_hand = self.flags.using_item.then_some(self.flags.using_offhand);
        let (main_pose, off_pose) = arm_poses(
            &self.flags.main_hand,
            &self.flags.off_hand,
            using_hand,
            c.swinging(),
        );
        let (right, left) = if self.flags.main_arm_left {
            (off_pose, main_pose)
        } else {
            (main_pose, off_pose)
        };
        AnimSample {
            walk_pos: c.walk.position(t),
            walk_speed: c.walk.speed(t),
            body_yaw,
            head_yaw: wrap_degrees(head_yaw - body_yaw),
            pitch: crate::util::mth::lerp(t, c.x_rot_old, c.x_rot),
            attack_time: c.attack_time(t),
            attack_left: c.swing_left(),
            swim_amount: self.swim.get(t),
            age_ticks: self.ticks as f32 + t,
            death_time: c.death_time(t),
            hurt_time: c.hurt_time(t),
            hurt_dir: self.hurt_dir,
            has_red_overlay: c.has_red_overlay(),
            #[cfg(feature = "skins")]
            cape: self.cape(t),
            elytra: std::array::from_fn(|i| {
                self.elytra_old[i] + (self.elytra[i] - self.elytra_old[i]) * t
            }),
            crouching: self.flags.crouching,
            sprinting: self.flags.sprinting,
            fall_flying: self.flags.fall_flying,
            fall_flying_scale: self.fall_flying_scale(t),
            flying_y_rot: if self.flags.fall_flying {
                self.flying_y_rot(t)
            } else {
                None
            },
            speed_value: self.speed_value(),
            visually_swimming: self.flags.visually_swimming,
            spin_attack: self.flags.spin_attack,
            passenger: self.flags.passenger,
            sleeping: self.flags.sleeping,
            bed_orientation: self.flags.bed_orientation,
            using_item: self.flags.using_item,
            main_arm_left: self.flags.main_arm_left,
            ticks_using_item: if self.flags.using_item {
                self.use_ticks as f32 + t
            } else {
                0.0
            },
            right_arm_pose: right,
            left_arm_pose: left,
            main_hand: self.flags.main_hand.clone(),
            off_hand: self.flags.off_hand.clone(),
            armor: self.flags.armor,
            head: self.flags.head.clone(),
            using_offhand: self.flags.using_offhand,
            ticks_since_kinetic_hit: self
                .kinetic_hit_at
                .map_or(0.0, |at| self.ticks.wrapping_sub(at) as f32 + t),
            layers: self.flags.layers,
        }
    }
}

pub use crate::util::mth::{rot_lerp, wrap_degrees};

use crate::entities::living::{HURT_DURATION, LivingCore, SwimAmount};

pub fn hurt_tilt(hurt: f32, dir: f32, death: f32, strength: f32) -> bevy::math::Quat {
    use bevy::math::Quat;

    let roll = Quat::from_rotation_z((40.0 - 8000.0 / (death.min(20.0) + 200.0)).to_radians());
    if hurt < 0.0 {
        return roll;
    }
    let h = hurt / HURT_DURATION as f32;
    let tilt = -(h * h * h * h * std::f32::consts::PI).sin() * 14.0 * strength;
    let turn = Quat::from_rotation_y((-dir).to_radians());
    roll * turn * Quat::from_rotation_z(tilt.to_radians()) * turn.inverse()
}

#[cfg(feature = "skins")]
use crate::util::mth::DEG_TO_RAD;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::living::MAX_HEAD_ROTATION;

    fn input(x: f64, z: f64) -> AnimInput {
        AnimInput {
            pos: [x, 64.0, z],
            health: 20.0,
            interpolated: false,
            ..Default::default()
        }
    }

    #[test]
    fn a_hit_flashes_a_player_red_for_ten_ticks_and_kicks_the_limbs() {
        let mut a = HumanoidAnim::default();
        a.tick(input(0.0, 0.0));
        a.damage();
        a.tick(input(0.0, 0.0));
        assert!(a.sample(1.0).walk_speed > 0.0);
        for _ in 0..9 {
            assert!(a.sample(0.0).has_red_overlay);
            a.tick(input(0.0, 0.0));
        }
        assert!(!a.sample(0.0).has_red_overlay);
    }

    fn roll_degrees(q: bevy::math::Quat) -> f32 {
        let (axis, angle) = q.to_axis_angle();
        angle.to_degrees() * axis.z.signum()
    }

    #[test]
    fn hurt_tilt_peaks_at_fourteen_degrees() {
        let peak = 10.0 * 0.5f32.powf(0.25);
        assert!((roll_degrees(hurt_tilt(peak, 0.0, 0.0, 1.0)) + 14.0).abs() < 1e-3);
        assert!((roll_degrees(hurt_tilt(peak, 0.0, 0.0, 0.5)) + 7.0).abs() < 1e-3);
    }

    #[test]
    fn hurt_tilt_at_no_strength_is_the_identity() {
        let peak = 10.0 * 0.5f32.powf(0.25);
        assert!(hurt_tilt(peak, 90.0, 0.0, 0.0).angle_between(bevy::math::Quat::IDENTITY) < 1e-3);
        assert!(hurt_tilt(-0.5, 90.0, 0.0, 1.0).angle_between(bevy::math::Quat::IDENTITY) < 1e-3);
    }

    #[test]
    fn hurt_tilt_conjugates_by_the_direction() {
        use bevy::math::Quat;
        let dir = 37.0f32;
        let turn = Quat::from_rotation_y((-dir).to_radians());
        let want = turn * hurt_tilt(8.0, 0.0, 0.0, 1.0) * turn.inverse();
        assert!(hurt_tilt(8.0, dir, 0.0, 1.0).angle_between(want) < 1e-3);
    }

    #[test]
    fn the_death_roll_settles_at_twenty_ticks() {
        assert!((roll_degrees(hurt_tilt(-1.0, 0.0, 20.0, 1.0)) - 3.636).abs() < 1e-3);
        assert!((roll_degrees(hurt_tilt(-1.0, 0.0, 35.0, 1.0)) - 3.636).abs() < 1e-3);
    }

    #[test]
    fn hurt_direction_comes_from_the_animation_alone() {
        let mut a = HumanoidAnim::default();
        a.hurt_from(42.0);
        assert_eq!(a.sample(0.0).hurt_dir, 42.0);
        assert_eq!(a.sample(0.5).hurt_time, 9.5);
        a.damage();
        a.hurt();
        assert_eq!(a.sample(0.0).hurt_dir, 42.0);
        a.reset_hurt();
        assert_eq!(a.sample(0.0).hurt_dir, 0.0);
        assert_eq!(a.sample(0.0).hurt_time, 0.0);
    }

    #[test]
    fn a_corpse_stays_red_past_the_hurt_window() {
        let mut a = HumanoidAnim::default();
        let dead = AnimInput {
            pos: [0.0, 64.0, 0.0],
            health: 0.0,
            interpolated: false,
            ..Default::default()
        };
        for _ in 0..15 {
            a.tick(dead.clone());
        }
        assert!(a.sample(0.0).has_red_overlay);
    }

    #[test]
    fn a_remote_player_walks_a_third_of_a_packet_per_tick() {
        let mut a = HumanoidAnim::default();
        let mut i = AnimInput {
            pos: [0.0, 64.0, 0.0],
            health: 20.0,
            interpolated: true,
            ..Default::default()
        };
        a.tick(i.clone());
        i.pos = [3.0, 64.0, 0.0];
        a.tick(i.clone());
        assert!(
            (a.position(1.0)[0] - 1.0).abs() < 1e-5,
            "{}",
            a.position(1.0)[0]
        );
        a.tick(i.clone());
        a.tick(i);
        assert!(
            (a.position(1.0)[0] - 3.0).abs() < 1e-5,
            "{}",
            a.position(1.0)[0]
        );
    }

    #[test]
    fn the_local_player_takes_its_own_movement_whole() {
        let mut a = HumanoidAnim::default();
        a.tick(input(0.0, 0.0));
        a.tick(input(0.0, 0.3));
        assert!((a.position(1.0)[2] - 0.3).abs() < 1e-5);
        assert!((a.position(0.0)[2] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn a_remote_head_yaw_takes_three_ticks() {
        let mut a = HumanoidAnim::default();
        let mut i = AnimInput {
            pos: [0.0, 64.0, 0.0],
            health: 20.0,
            interpolated: true,
            ..Default::default()
        };
        a.tick(i.clone());
        i.head_yaw = 90.0;
        a.tick(i.clone());
        assert!(
            (a.sample(1.0).head_yaw - 30.0).abs() < 1e-3,
            "{}",
            a.sample(1.0).head_yaw
        );
        a.tick(i.clone());
        a.tick(i);
        assert!(
            (a.sample(1.0).head_yaw - 90.0).abs() < 1e-3,
            "{}",
            a.sample(1.0).head_yaw
        );
    }

    fn item(id: &'static str) -> HeldItem {
        HeldItem {
            id,
            count: 1,
            ..HeldItem::default()
        }
    }

    #[test]
    fn walk_cycle_ramps_toward_the_target_speed() {
        let mut a = HumanoidAnim::default();
        a.tick(input(0.0, 0.0));
        a.tick(input(0.0, 0.2));
        let first = a.sample(1.0);
        assert!(
            (first.walk_speed - 0.32).abs() < 1e-4,
            "{}",
            first.walk_speed
        );
        a.tick(input(0.0, 0.4));
        let second = a.sample(1.0);
        assert!(second.walk_speed > first.walk_speed);
        assert!(second.walk_pos > first.walk_pos);
    }

    #[test]
    fn standing_still_decays_the_amplitude() {
        let mut a = HumanoidAnim::default();
        for i in 0..10 {
            a.tick(input(0.0, i as f64 * 0.2));
        }
        let moving = a.sample(1.0).walk_speed;
        for _ in 0..20 {
            a.tick(input(0.0, 1.8));
        }
        assert!(a.sample(1.0).walk_speed < moving * 0.05);
    }

    #[test]
    fn swing_runs_six_ticks_and_ignores_an_early_retrigger() {
        let mut a = HumanoidAnim::default();
        a.swing(false);
        a.tick(input(0.0, 0.0));
        assert_eq!(a.sample(1.0).attack_time, 0.0);
        a.tick(input(0.0, 0.0));
        let t2 = a.sample(1.0).attack_time;
        assert!((t2 - 1.0 / 6.0).abs() < 1e-6, "{t2}");

        a.swing(false);
        a.tick(input(0.0, 0.0));
        assert!(a.sample(1.0).attack_time > t2);

        for _ in 0..5 {
            a.tick(input(0.0, 0.0));
        }
        assert_eq!(a.sample(1.0).attack_time, 0.0);
    }

    #[test]
    fn head_yaw_is_relative_and_wrapped() {
        let mut a = HumanoidAnim::default();
        let mut i = input(0.0, 0.0);
        i.y_rot = 170.0;
        i.head_yaw = -170.0;
        a.tick(i.clone());
        a.tick(i);
        assert!((a.sample(1.0).head_yaw - 20.0).abs() < 1e-3);
    }

    #[test]
    fn use_animations_split_food_from_drink() {
        assert_eq!(use_animation("bread"), UseAnimation::Eat);
        assert_eq!(use_animation("cooked_beef"), UseAnimation::Eat);
        assert_eq!(use_animation("honey_bottle"), UseAnimation::Drink);
        assert_eq!(use_animation("potion"), UseAnimation::Drink);
        assert_eq!(use_animation("shield"), UseAnimation::Block);
        assert_eq!(use_animation("stone"), UseAnimation::None);
    }

    #[test]
    fn shield_only_blocks_while_it_is_the_hand_in_use() {
        let shield = item("shield");
        let empty = HeldItem::default();
        assert_eq!(arm_poses(&shield, &empty, None, false).0, ArmPose::Item);
        assert_eq!(
            arm_poses(&shield, &empty, Some(false), false).0,
            ArmPose::Block
        );
        let (right, left) = arm_poses(&empty, &shield, Some(true), false);
        assert_eq!((right, left), (ArmPose::Empty, ArmPose::Block));
    }

    #[test]
    fn a_two_handed_pose_overrides_the_off_arm() {
        let bow = item("bow");
        let shield = item("shield");
        let (right, left) = arm_poses(&bow, &shield, Some(false), false);
        assert_eq!(right, ArmPose::BowAndArrow);
        assert_eq!(left, ArmPose::Item);
        let (_, left) = arm_poses(&bow, &HeldItem::default(), Some(false), false);
        assert_eq!(left, ArmPose::Empty);
    }

    #[test]
    fn a_charged_crossbow_is_held_unless_the_arm_is_swinging() {
        let loaded = HeldItem {
            id: "crossbow",
            charged: true,
            count: 1,
            ..HeldItem::default()
        };
        let unloaded = item("crossbow");
        let empty = HeldItem::default();
        assert_eq!(
            arm_poses(&loaded, &empty, None, false).0,
            ArmPose::CrossbowHold
        );
        assert_eq!(arm_poses(&loaded, &empty, None, true).0, ArmPose::Item);
        assert_eq!(
            arm_poses(&unloaded, &empty, Some(false), false).0,
            ArmPose::CrossbowCharge
        );
    }

    #[test]
    fn the_body_eases_toward_the_direction_of_travel() {
        let mut a = HumanoidAnim::default();
        let mut i = AnimInput {
            pos: [0.0, 64.0, 0.0],
            health: 20.0,
            ..Default::default()
        };
        i.head_yaw = 180.0;
        i.y_rot = 180.0;
        a.tick(i.clone());
        let first = a.sample(1.0).body_yaw;
        for step in 1..12 {
            i.pos[0] = step as f64 * 0.2;
            a.tick(i.clone());
        }
        let after = a.sample(1.0).body_yaw;
        assert_ne!(first, after, "the body never turned");
        assert!(
            wrap_degrees(after - 180.0).abs() <= MAX_HEAD_ROTATION + 0.01,
            "body_yaw {after}"
        );
    }

    #[test]
    fn the_body_is_dragged_when_the_head_exceeds_fifty_degrees() {
        let mut a = HumanoidAnim::default();
        let mut i = AnimInput {
            pos: [0.0, 64.0, 0.0],
            health: 20.0,
            ..Default::default()
        };
        a.tick(i.clone());
        i.head_yaw = 180.0;
        i.y_rot = 180.0;
        a.tick(i);
        let relative = a.sample(1.0).head_yaw;
        assert!(
            relative.abs() <= MAX_HEAD_ROTATION + 0.01,
            "head twisted {relative} past the limit"
        );
    }

    #[test]
    fn item_use_ticks_count_up_and_reset() {
        let mut a = HumanoidAnim::default();
        let mut i = input(0.0, 0.0);
        i.using_item = true;
        for _ in 0..5 {
            a.tick(i.clone());
        }
        assert_eq!(a.sample(0.0).ticks_using_item, 5.0);
        assert_eq!(a.sample(0.5).ticks_using_item, 5.5);
        i.using_item = false;
        a.tick(i);
        assert_eq!(a.sample(1.0).ticks_using_item, 0.0);
    }
}
