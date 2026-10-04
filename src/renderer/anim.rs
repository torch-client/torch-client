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
        _ => UseAnimation::None,
    }
}

pub type Armor = [Option<&'static str>; 4];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeldItem {
    pub id: &'static str,
    pub charged: bool,
    pub count: u8,
    pub tint: Option<u32>,
    pub map_id: Option<i32>,
    pub layers: Option<Box<[crate::blockentities::banner::BannerLayer]>>,
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
            UseAnimation::None | UseAnimation::Eat | UseAnimation::Drink => {}
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
    pub has_red_overlay: bool,
    pub crouching: bool,
    #[allow(
        dead_code,
        reason = "no sprint pose in HumanoidModel.setupAnim; see the field comment"
    )]
    pub sprinting: bool,
    pub fall_flying: bool,
    pub visually_swimming: bool,
    pub spin_attack: bool,
    pub passenger: bool,
    pub sleeping: bool,
    pub bed_orientation: Option<crate::direction::Direction>,
    pub using_item: bool,
    pub main_arm_left: bool,
    pub ticks_using_item: f32,
    pub right_arm_pose: ArmPose,
    pub left_arm_pose: ArmPose,
    pub main_hand: HeldItem,
    pub off_hand: HeldItem,
    pub armor: Armor,
    pub using_offhand: bool,
    #[cfg(feature = "skins")]
    pub cape: [f32; 3],
    pub elytra: [f32; 3],
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
    pub bed_orientation: Option<crate::direction::Direction>,
    pub using_item: bool,
    pub using_offhand: bool,
    pub passenger: bool,
    pub health: f32,
    pub main_hand: HeldItem,
    pub off_hand: HeldItem,
    pub armor: Armor,
    pub main_arm_left: bool,
    pub interpolated: bool,
    #[cfg(feature = "skins")]
    pub on_ground: bool,
}

const SWING_DURATION: i32 = 6;

const MAX_HEAD_ROTATION: f32 = 50.0;

const LERP_HEAD_STEPS: i32 = 3;

const MAX_INTERPOLATION_DISTANCE_SQR: f64 = 4096.0;

fn distance_sqr(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    dx * dx + dy * dy + dz * dz
}

fn unwind(old: &mut f32, new: f32) {
    while new - *old < -180.0 {
        *old -= 360.0;
    }
    while new - *old >= 180.0 {
        *old += 360.0;
    }
}

#[derive(Clone, Debug, Default)]
pub struct HumanoidAnim {
    walk_pos: f32,
    walk_speed: f32,
    walk_speed_old: f32,

    body_yaw: f32,
    body_yaw_old: f32,
    head_yaw: f32,
    head_yaw_old: f32,
    y_rot: f32,
    y_rot_old: f32,
    pitch: f32,
    pitch_old: f32,

    interp: Option<crate::util::interp::Interpolation>,
    lerp_head_steps: i32,
    lerp_y_head_rot: f32,
    raw_pos: [f64; 3],
    raw_y_rot: f32,
    raw_pitch: f32,
    raw_head_yaw: f32,

    swing_time: i32,
    swinging: bool,
    swing_left: bool,
    attack_anim: f32,
    attack_anim_old: f32,

    swim: f32,
    swim_old: f32,

    ticks: u32,
    death_time: u32,
    hurt_time: u32,
    use_ticks: u32,
    use_ticks_old: u32,

    pos: [f64; 3],
    pos_old: [f64; 3],

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
    #[cfg(feature = "skins")]
    fall_fly_ticks: u32,
    elytra: [f32; 3],
    elytra_old: [f32; 3],

    flags: AnimInput,
    started: bool,
}

impl HumanoidAnim {
    pub fn use_ticks(&self) -> u32 {
        self.use_ticks
    }

    pub fn tick(&mut self, input: AnimInput) {
        if !self.started {
            self.started = true;
            self.interp = input
                .interpolated
                .then(crate::util::interp::Interpolation::new);
            self.pos = input.pos;
            self.y_rot = input.y_rot;
            self.body_yaw = input.y_rot;
            self.head_yaw = input.head_yaw;
            self.pitch = input.pitch;
            self.raw_pos = input.pos;
            self.raw_y_rot = input.y_rot;
            self.raw_pitch = input.pitch;
            self.raw_head_yaw = input.head_yaw;
        }

        #[cfg(feature = "skins")]
        self.move_cloak();

        let self_move = self.handle_movement_packets(&input);

        self.pos_old = self.pos;
        self.y_rot_old = self.y_rot;
        self.head_yaw_old = self.head_yaw;
        self.pitch_old = self.pitch;
        self.body_yaw_old = self.body_yaw;
        self.flags = input;
        self.ticks = self.ticks.wrapping_add(1);

        if let Some((pos, y_rot, pitch)) = self_move {
            self.pos = pos;
            self.y_rot = y_rot;
            self.pitch = pitch;
        }

        if let Some(mut interp) = self.interp.take() {
            if interp.has_active_interpolation() {
                interp.interpolate(crate::util::interp::Target {
                    pos: &mut self.pos,
                    y_rot: &mut self.y_rot,
                    x_rot: &mut self.pitch,
                });
            }
            self.interp = Some(interp);
        }
        if self.lerp_head_steps > 0 {
            self.head_yaw = rot_lerp(
                1.0 / self.lerp_head_steps as f32,
                self.head_yaw,
                self.lerp_y_head_rot,
            );
            self.lerp_head_steps -= 1;
        }

        let alive = self.flags.health > 0.0;
        if alive {
            self.death_time = 0;
        } else if self.death_time < 20 {
            self.death_time += 1;
        }
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }

        self.attack_anim_old = self.attack_anim;
        if self.swinging {
            self.swing_time += 1;
            if self.swing_time >= SWING_DURATION {
                self.swing_time = 0;
                self.swinging = false;
            }
        } else {
            self.swing_time = 0;
        }
        self.attack_anim = self.swing_time as f32 / SWING_DURATION as f32;

        self.swim_old = self.swim;
        self.swim = if self.flags.visually_swimming {
            (self.swim + 0.09).min(1.0)
        } else {
            (self.swim - 0.09).max(0.0)
        };

        self.walk_speed_old = self.walk_speed;
        if self.flags.passenger || !alive {
            self.walk_pos = 0.0;
            self.walk_speed = 0.0;
            self.walk_speed_old = 0.0;
        } else {
            let dx = (self.pos[0] - self.pos_old[0]) as f32;
            let dz = (self.pos[2] - self.pos_old[2]) as f32;
            let target = (dx.hypot(dz) * 4.0).min(1.0);
            self.walk_speed += (target - self.walk_speed) * 0.4;
            self.walk_pos += self.walk_speed;
        }

        #[cfg(feature = "skins")]
        {
            if !self.flags.interpolated {
                let dx = (self.pos[0] - self.pos_old[0]) as f32;
                let dz = (self.pos[2] - self.pos_old[2]) as f32;
                self.walk_dist_old = self.walk_dist;
                self.walk_dist += dx.hypot(dz) * 0.6;
            }
            if self.flags.passenger {
                self.bob_old = self.bob;
                self.bob = 0.0;
            } else {
                let dx = (self.pos[0] - self.pos_old[0]) as f32;
                let dz = (self.pos[2] - self.pos_old[2]) as f32;
                let target = if self.flags.on_ground && alive && !self.flags.visually_swimming {
                    dx.hypot(dz).min(0.1)
                } else {
                    0.0
                };
                self.bob_old = self.bob;
                self.bob += (target - self.bob) * 0.4;
            }
            self.fall_fly_ticks = if self.flags.fall_flying {
                self.fall_fly_ticks.saturating_add(1)
            } else {
                0
            };
        }

        self.elytra_old = self.elytra;
        crate::entities::models::humanoid::elytra::tick_angles(
            &mut self.elytra,
            self.flags.fall_flying,
            self.flags.crouching,
            [
                self.pos[0] - self.pos_old[0],
                self.pos[1] - self.pos_old[1],
                self.pos[2] - self.pos_old[2],
            ],
        );

        self.use_ticks_old = self.use_ticks;
        self.use_ticks = if self.flags.using_item {
            self.use_ticks + 1
        } else {
            0
        };

        self.turn_body();

        unwind(&mut self.y_rot_old, self.y_rot);
        unwind(&mut self.body_yaw_old, self.body_yaw);
        unwind(&mut self.pitch_old, self.pitch);
        unwind(&mut self.head_yaw_old, self.head_yaw);
    }

    fn handle_movement_packets(&mut self, input: &AnimInput) -> Option<([f64; 3], f32, f32)> {
        let mut deferred = None;
        if input.pos != self.raw_pos
            || input.y_rot != self.raw_y_rot
            || input.pitch != self.raw_pitch
        {
            self.raw_pos = input.pos;
            self.raw_y_rot = input.y_rot;
            self.raw_pitch = input.pitch;
            let too_big = distance_sqr(self.pos, input.pos) > MAX_INTERPOLATION_DISTANCE_SQR;
            match self.interp.take() {
                Some(mut interp) if too_big => {
                    self.pos = input.pos;
                    self.y_rot = input.y_rot;
                    self.pitch = input.pitch;
                    self.pos_old = self.pos;
                    self.y_rot_old = self.y_rot;
                    self.pitch_old = self.pitch;
                    interp.cancel();
                    self.interp = Some(interp);
                }
                Some(mut interp) => {
                    interp.interpolate_to(
                        crate::util::interp::Target {
                            pos: &mut self.pos,
                            y_rot: &mut self.y_rot,
                            x_rot: &mut self.pitch,
                        },
                        input.pos,
                        input.y_rot,
                        input.pitch,
                    );
                    self.interp = Some(interp);
                }
                None => deferred = Some((input.pos, input.y_rot, input.pitch)),
            }
        }

        if input.head_yaw != self.raw_head_yaw {
            self.raw_head_yaw = input.head_yaw;
            if self.interp.is_some() {
                self.lerp_y_head_rot = input.head_yaw;
                self.lerp_head_steps = LERP_HEAD_STEPS;
            } else {
                self.head_yaw = input.head_yaw;
            }
        }
        deferred
    }

    pub fn body_yaw_pair(&self) -> (f32, f32) {
        (self.body_yaw_old, self.body_yaw)
    }

    fn turn_body(&mut self) {
        let dx = self.pos[0] - self.pos_old[0];
        let dz = self.pos[2] - self.pos_old[2];
        let mut target = self.body_yaw;
        if (dx * dx + dz * dz) as f32 > 0.002_500_000_2 {
            let walk = (dz.atan2(dx) as f32).to_degrees() - 90.0;
            let diff = (wrap_degrees(self.y_rot) - walk).abs();
            target = if (95.0..265.0).contains(&diff) {
                walk - 180.0
            } else {
                walk
            };
        }
        if self.attack_anim > 0.0 {
            target = self.y_rot;
        }

        self.body_yaw += wrap_degrees(target - self.body_yaw) * 0.3;
        let head_diff = wrap_degrees(self.y_rot - self.body_yaw);
        if head_diff.abs() > MAX_HEAD_ROTATION {
            self.body_yaw += head_diff - head_diff.signum() * MAX_HEAD_ROTATION;
        }
    }

    pub fn swing(&mut self, left_arm: bool) {
        if !self.swinging || self.swing_time >= SWING_DURATION / 2 || self.swing_time < 0 {
            self.swing_time = -1;
            self.swinging = true;
            self.swing_left = left_arm;
        }
    }

    pub fn hurt(&mut self) {
        self.hurt_time = 10;
    }

    pub fn damage(&mut self) {
        self.walk_speed = 1.5;
        self.hurt();
    }

    pub fn position(&self, partial: f32) -> [f32; 3] {
        let t = partial.clamp(0.0, 1.0) as f64;
        [
            (self.pos_old[0] + (self.pos[0] - self.pos_old[0]) * t) as f32,
            (self.pos_old[1] + (self.pos[1] - self.pos_old[1]) * t) as f32,
            (self.pos_old[2] + (self.pos[2] - self.pos_old[2]) * t) as f32,
        ]
    }

    #[cfg(feature = "skins")]
    fn move_cloak(&mut self) {
        const TELEPORT: f64 = 10.0;

        self.cloak_old = self.cloak;
        for axis in 0..3 {
            let delta = self.pos[axis] - self.cloak[axis];
            if delta.abs() > TELEPORT {
                self.cloak[axis] = self.pos[axis];
                self.cloak_old[axis] = self.cloak[axis];
            } else {
                self.cloak[axis] += delta * 0.25;
            }
        }
    }

    #[cfg(feature = "skins")]
    fn cape(&self, t: f32) -> [f32; 3] {
        let td = t as f64;
        let lerp = |old: f64, new: f64| old + (new - old) * td;
        let dx = lerp(self.cloak_old[0], self.cloak[0]) - lerp(self.pos_old[0], self.pos[0]);
        let dy = lerp(self.cloak_old[1], self.cloak[1]) - lerp(self.pos_old[1], self.pos[1]);
        let dz = lerp(self.cloak_old[2], self.cloak[2]) - lerp(self.pos_old[2], self.pos[2]);

        let body_yaw = rot_lerp(t, self.body_yaw_old, self.body_yaw) * DEG_TO_RAD;
        let (forward_x, forward_z) = (body_yaw.sin() as f64, -body_yaw.cos() as f64);

        let fly = self.fall_fly_ticks as f32 + t;
        let fly_scale = (fly * fly / 100.0).clamp(0.0, 1.0);

        let mut flap = ((dy * 10.0) as f32).clamp(-6.0, 32.0);
        let lean = ((dx * forward_x + dz * forward_z) as f32 * 100.0 * (1.0 - fly_scale))
            .clamp(0.0, 150.0);
        let lean2 = ((dx * forward_z - dz * forward_x) as f32 * 100.0).clamp(-20.0, 20.0);

        let walk = self.walk_dist_old + (self.walk_dist - self.walk_dist_old) * t;
        let bob = self.bob_old + (self.bob - self.bob_old) * t;
        flap += (walk * 6.0).sin() * 32.0 * bob;
        [flap, lean, lean2]
    }

    pub fn sample(&self, partial: f32) -> AnimSample {
        let t = partial.clamp(0.0, 1.0);
        let body_yaw = rot_lerp(t, self.body_yaw_old, self.body_yaw);
        let head_yaw = rot_lerp(t, self.head_yaw_old, self.head_yaw);
        let using_hand = self.flags.using_item.then_some(self.flags.using_offhand);
        let (main_pose, off_pose) = arm_poses(
            &self.flags.main_hand,
            &self.flags.off_hand,
            using_hand,
            self.swinging,
        );
        let (right, left) = if self.flags.main_arm_left {
            (off_pose, main_pose)
        } else {
            (main_pose, off_pose)
        };
        AnimSample {
            walk_pos: self.walk_pos - self.walk_speed * (1.0 - t),
            walk_speed: (self.walk_speed_old + (self.walk_speed - self.walk_speed_old) * t)
                .min(1.0),
            body_yaw,
            head_yaw: wrap_degrees(head_yaw - body_yaw),
            pitch: self.pitch_old + (self.pitch - self.pitch_old) * t,
            attack_time: self.attack_anim_old + (self.attack_anim - self.attack_anim_old) * t,
            attack_left: self.swing_left,
            swim_amount: self.swim_old + (self.swim - self.swim_old) * t,
            age_ticks: self.ticks as f32 + t,
            death_time: if self.death_time > 0 {
                self.death_time as f32 + t
            } else {
                0.0
            },
            has_red_overlay: self.hurt_time > 0 || self.death_time > 0,
            #[cfg(feature = "skins")]
            cape: self.cape(t),
            elytra: std::array::from_fn(|i| {
                self.elytra_old[i] + (self.elytra[i] - self.elytra_old[i]) * t
            }),
            crouching: self.flags.crouching,
            sprinting: self.flags.sprinting,
            fall_flying: self.flags.fall_flying,
            visually_swimming: self.flags.visually_swimming,
            spin_attack: self.flags.spin_attack,
            passenger: self.flags.passenger,
            sleeping: self.flags.sleeping,
            bed_orientation: self.flags.bed_orientation,
            using_item: self.flags.using_item,
            main_arm_left: self.flags.main_arm_left,
            ticks_using_item: self.use_ticks_old as f32
                + (self.use_ticks as f32 - self.use_ticks_old as f32) * t,
            right_arm_pose: right,
            left_arm_pose: left,
            main_hand: self.flags.main_hand.clone(),
            off_hand: self.flags.off_hand.clone(),
            armor: self.flags.armor,
            using_offhand: self.flags.using_offhand,
        }
    }
}

pub use crate::util::mth::{rot_lerp, wrap_degrees};

#[cfg(feature = "skins")]
use crate::util::mth::DEG_TO_RAD;

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(a.sample(1.0).ticks_using_item, 5.0);
        i.using_item = false;
        a.tick(i);
        assert_eq!(a.sample(1.0).ticks_using_item, 0.0);
    }
}
