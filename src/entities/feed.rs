use azalea_registry::builtin::EntityKind;

use super::physics::{Body, Level, Motion, Shape, Void};
use super::state::{Direction, EntityState, Extras, ExtrasShared, Pose};
use crate::util::interp::{Interpolation, Target};
use crate::util::javarandom::JavaRandom;
use crate::util::mth::ease::in_out_sine as ease_in_out_sine;
pub use crate::util::mth::{lerp, lerp_f64, rot_lerp, rot_lerp_capped, wrap_degrees};

#[derive(Clone, Copy, Debug, Default)]
pub struct WalkAnim {
    speed_old: f32,
    speed: f32,
    position: f32,
    position_scale: f32,
}

impl WalkAnim {
    pub fn new() -> WalkAnim {
        WalkAnim {
            speed_old: 0.0,
            speed: 0.0,
            position: 0.0,
            position_scale: 1.0,
        }
    }

    pub fn update(&mut self, target_speed: f32, factor: f32, position_scale: f32) {
        self.speed_old = self.speed;
        self.speed += (target_speed - self.speed) * factor;
        self.position += self.speed;
        self.position_scale = position_scale;
    }

    pub fn stop(&mut self) {
        self.speed_old = 0.0;
        self.speed = 0.0;
        self.position = 0.0;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn speed(&self, partial: f32) -> f32 {
        lerp(partial, self.speed_old, self.speed).min(1.0)
    }

    pub fn position(&self, partial: f32) -> f32 {
        (self.position - self.speed * (1.0 - partial)) * self.position_scale
    }
}

const BINARY_ANIMATOR_LENGTH: i32 = 10;

fn binary_animator_tick(ticks: i32, active: bool) -> i32 {
    if active {
        (ticks + 1).min(BINARY_ANIMATOR_LENGTH)
    } else {
        (ticks - 1).max(0)
    }
}

fn binary_animator_factor(old: i32, ticks: i32, partial: f32) -> f32 {
    ease_in_out_sine(lerp(partial, old as f32, ticks as f32) / BINARY_ANIMATOR_LENGTH as f32)
}

#[derive(Clone, Debug)]
pub struct TickInput {
    pub simulate: bool,
    pub pos: [f64; 3],
    pub y_rot: f32,
    pub head_rot: Option<f32>,
    pub x_rot: f32,
    pub is_living: bool,
    pub velocity: Option<([f64; 3], u64)>,
    pub pose: Pose,
    pub dead: bool,
    pub is_in_water: bool,
    pub is_in_lava: bool,
    pub is_fully_frozen: bool,
    pub is_auto_spin_attack: bool,
    pub is_invisible: bool,
    pub display_fire: bool,
    pub scale: f32,
    pub bounding_box_width: f32,
    pub bounding_box_height: f32,
    pub eye_height: f32,
    pub bed_orientation: Option<Direction>,
    pub walk_factor: f32,
    pub walk_halted: bool,
    pub is_passenger: bool,
    pub swell_dir: i32,
    pub ignited: bool,
    pub peek_target: f32,
    pub paddling_left: bool,
    pub paddling_right: bool,
    pub lying: bool,
    pub relax_state_one: bool,
    pub on_ground: bool,
    pub playing_dead: bool,
    pub open_mouth: bool,
    pub head_targets: [Option<[f64; 3]>; 2],
    pub shared: std::sync::Arc<ExtrasShared>,
    pub eating: bool,
}

impl TickInput {
    pub fn shared_mut(&mut self) -> &mut ExtrasShared {
        std::sync::Arc::make_mut(&mut self.shared)
    }
}

impl TickInput {
    pub fn new() -> TickInput {
        TickInput {
            simulate: true,
            pos: [0.0; 3],
            y_rot: 0.0,
            head_rot: None,
            x_rot: 0.0,
            is_living: true,
            velocity: None,
            pose: Pose::Standing,
            dead: false,
            is_in_water: false,
            is_in_lava: false,
            is_fully_frozen: false,
            is_auto_spin_attack: false,
            is_invisible: false,
            display_fire: false,
            scale: 1.0,
            bounding_box_width: 0.6,
            bounding_box_height: 1.8,
            eye_height: 1.62,
            bed_orientation: None,
            walk_factor: 0.4,
            walk_halted: false,
            is_passenger: false,
            swell_dir: -1,
            ignited: false,
            peek_target: 0.0,
            paddling_left: false,
            paddling_right: false,
            lying: false,
            relax_state_one: false,
            on_ground: false,
            playing_dead: false,
            open_mouth: false,
            head_targets: [None; 2],
            shared: std::sync::Arc::new(ExtrasShared::default()),
            eating: false,
        }
    }
}

pub fn walk_target(kind: EntityKind, old_pos: [f64; 3], pos: [f64; 3]) -> f32 {
    let use_y = matches!(kind, EntityKind::Parrot | EntityKind::Bee);
    let dx = (pos[0] - old_pos[0]) as f32;
    let dy = if use_y {
        (pos[1] - old_pos[1]) as f32
    } else {
        0.0
    };
    let dz = (pos[2] - old_pos[2]) as f32;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    match kind {
        EntityKind::Frog => (distance * 25.0).min(1.0),
        EntityKind::Creaking => (distance * 25.0).min(3.0),
        EntityKind::Camel | EntityKind::CamelHusk => (distance * 6.0).min(1.0),
        _ => (distance * 4.0).min(1.0),
    }
}

pub fn walk_factor(kind: EntityKind) -> f32 {
    match kind {
        EntityKind::Camel | EntityKind::CamelHusk => 0.2,
        _ => 0.4,
    }
}

const MAX_SWELL: i32 = 30;

const EAT_ANIMATION_TICKS: i32 = 40;

const PADDLE_STEP: f32 = std::f32::consts::FRAC_PI_8;

const WIGGLE_TICKS: i32 = 5;

const WARDEN_ANGRY_ANGER: f32 = 80.0;

const CAMEL_DASH_COOLDOWN: i32 = 55;

const ARROW_SHAKE_TICKS: i32 = 7;

const EVOKER_FANGS_LIFE: i32 = 22;

const RABBIT_JUMP_DURATION: i32 = 15;

const GOAT_LOWER_HEAD_TICKS: i32 = 20;

const PHANTOM_FLAP_OFFSET: i32 = 3;

pub fn sheep_wool_color(bits: u8) -> u8 {
    bits & 0x0f
}

pub fn horse_open_mouth(bits: u8) -> bool {
    bits & 0x40 != 0
}

pub fn dragon_phase_is_sitting(phase: i32) -> bool {
    matches!(phase, 5 | 6 | 7 | 10)
}

const LERP_HEAD_STEPS: i32 = 3;

const MAX_HEAD_ROTATION: f32 = 50.0;

const BODY_ROT_MOVEMENT_THRESHOLD: f32 = 0.002_500_000_2;

const MAX_INTERPOLATION_DISTANCE_SQR: f64 = 4096.0;

const SHULKER_TELEPORT_TICKS: i32 = 6;

fn interpolation_for(kind: EntityKind, is_living: bool) -> Option<Interpolation> {
    if matches!(kind, EntityKind::Shulker) {
        return None;
    }
    let interpolated = is_living
        || matches!(
            kind,
            EntityKind::OakBoat
                | EntityKind::SpruceBoat
                | EntityKind::BirchBoat
                | EntityKind::JungleBoat
                | EntityKind::AcaciaBoat
                | EntityKind::CherryBoat
                | EntityKind::DarkOakBoat
                | EntityKind::PaleOakBoat
                | EntityKind::MangroveBoat
                | EntityKind::OakChestBoat
                | EntityKind::SpruceChestBoat
                | EntityKind::BirchChestBoat
                | EntityKind::JungleChestBoat
                | EntityKind::AcaciaChestBoat
                | EntityKind::CherryChestBoat
                | EntityKind::DarkOakChestBoat
                | EntityKind::PaleOakChestBoat
                | EntityKind::MangroveChestBoat
                | EntityKind::BambooRaft
                | EntityKind::BambooChestRaft
                | EntityKind::Minecart
                | EntityKind::ChestMinecart
                | EntityKind::CommandBlockMinecart
                | EntityKind::FurnaceMinecart
                | EntityKind::HopperMinecart
                | EntityKind::SpawnerMinecart
                | EntityKind::TntMinecart
                | EntityKind::ExperienceOrb
                | EntityKind::FishingBobber
        );
    interpolated.then(Interpolation::new)
}

fn distance_sqr(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    dx * dx + dy * dy + dz * dz
}

fn block_pos(pos: [f64; 3]) -> [i32; 3] {
    [
        pos[0].floor() as i32,
        pos[1].floor() as i32,
        pos[2].floor() as i32,
    ]
}

fn unwind(old: &mut f32, new: f32) {
    while new - *old < -180.0 {
        *old -= 360.0;
    }
    while new - *old >= 180.0 {
        *old += 360.0;
    }
}

impl Default for TickInput {
    fn default() -> Self {
        TickInput::new()
    }
}

#[derive(Clone, Debug)]
struct KindAnim {
    attach_pos: Option<[i32; 3]>,
    teleport_interp: i32,
    swell: f32,
    swell_old: f32,
    peek: f32,
    peek_old: f32,
    jump: f32,
    jump_old: f32,
    swell_counter: i32,
    paddle: [f32; 2],
    paddle_old: [f32; 2],
    eat_ticks: i32,
    shaking: bool,
    shake_anim: f32,
    shake_anim_old: f32,
    interest: f32,
    interest_old: f32,
    attack_ticks: i32,
    stunned_ticks: i32,
    roar_ticks: i32,
    offer_flower_ticks: i32,
    tendril_ticks: i32,
    tendril_ticks_old: i32,
    heart_ticks: i32,
    heart_ticks_old: i32,
    wiggle_ticks: i32,
    swim: f32,
    swim_old: f32,
    elytra: [f32; 3],
    elytra_old: [f32; 3],
    use_ticks: i32,
    crouch: f32,
    crouch_old: f32,
    bear_stand: f32,
    bear_stand_old: f32,
    panda: [f32; 3],
    panda_old: [f32; 3],
    cat: [f32; 3],
    cat_old: [f32; 3],
    horse_eat: f32,
    horse_eat_old: f32,
    mouth_anim: f32,
    mouth_anim_old: f32,
    tentacle_speed: f32,
    tentacle_movement: f32,
    tentacle_angle: f32,
    tentacle_angle_old: f32,
    squid_x_rot: f32,
    squid_x_rot_old: f32,
    squid_z_rot: f32,
    squid_z_rot_old: f32,
    squid_rotate_speed: f32,
    squish: f32,
    squish_old: f32,
    target_squish: f32,
    was_on_ground: bool,
    tail_anim: f32,
    tail_anim_old: f32,
    tail_anim_speed: f32,
    spikes_anim: f32,
    spikes_anim_old: f32,
    flap_time: f32,
    flap_time_old: f32,
    flap: f32,
    flap_old: f32,
    flap_speed: f32,
    flap_speed_old: f32,
    flapping: f32,
    bee_roll: f32,
    bee_roll_old: f32,
    axolotl: [i32; 4],
    axolotl_old: [i32; 4],
    roll_counter: i32,
    jump_ticks: i32,
    jump_duration: i32,
    lowering_head: bool,
    lower_head_ticks: i32,
    holding_ticks: f32,
    holding_ticks_old: f32,
    dancing_ticks: f32,
    spinning_ticks: f32,
    spinning_ticks_old: f32,
    spinning: bool,
    dash_cooldown: i32,
    dash_old: bool,
    shake_time: i32,
    was_in_ground: bool,
    fangs_started: bool,
    fangs_life: i32,
    head_y_rots: [f32; 2],
    head_y_rots_old: [f32; 2],
    head_x_rots: [f32; 2],
    head_x_rots_old: [f32; 2],
}

impl KindAnim {
    const EMPTY: KindAnim = KindAnim {
        attach_pos: None,
        teleport_interp: 0,
        swell: 0.0,
        swell_old: 0.0,
        peek: 0.0,
        peek_old: 0.0,
        jump: 0.0,
        jump_old: 0.0,
        swell_counter: 0,
        paddle: [0.0; 2],
        paddle_old: [0.0; 2],
        eat_ticks: 0,
        shaking: false,
        shake_anim: 0.0,
        shake_anim_old: 0.0,
        interest: 0.0,
        interest_old: 0.0,
        attack_ticks: 0,
        stunned_ticks: 0,
        roar_ticks: 0,
        offer_flower_ticks: 0,
        tendril_ticks: 0,
        tendril_ticks_old: 0,
        heart_ticks: 0,
        heart_ticks_old: 0,
        wiggle_ticks: 0,
        swim: 0.0,
        swim_old: 0.0,
        elytra: [0.0; 3],
        elytra_old: [0.0; 3],
        use_ticks: 0,
        crouch: 0.0,
        crouch_old: 0.0,
        bear_stand: 0.0,
        bear_stand_old: 0.0,
        panda: [0.0; 3],
        panda_old: [0.0; 3],
        cat: [0.0; 3],
        cat_old: [0.0; 3],
        horse_eat: 0.0,
        horse_eat_old: 0.0,
        mouth_anim: 0.0,
        mouth_anim_old: 0.0,
        tentacle_speed: 0.0,
        tentacle_movement: 0.0,
        tentacle_angle: 0.0,
        tentacle_angle_old: 0.0,
        squid_x_rot: 0.0,
        squid_x_rot_old: 0.0,
        squid_z_rot: 0.0,
        squid_z_rot_old: 0.0,
        squid_rotate_speed: 0.0,
        squish: 0.0,
        squish_old: 0.0,
        target_squish: 0.0,
        was_on_ground: false,
        tail_anim: 0.0,
        tail_anim_old: 0.0,
        tail_anim_speed: 0.0,
        spikes_anim: 0.0,
        spikes_anim_old: 0.0,
        flap_time: 0.0,
        flap_time_old: 0.0,
        flap: 0.0,
        flap_old: 0.0,
        flap_speed: 0.0,
        flap_speed_old: 0.0,
        flapping: 0.0,
        bee_roll: 0.0,
        bee_roll_old: 0.0,
        axolotl: [0; 4],
        axolotl_old: [0; 4],
        roll_counter: 0,
        jump_ticks: 0,
        jump_duration: 0,
        lowering_head: false,
        lower_head_ticks: 0,
        holding_ticks: 0.0,
        holding_ticks_old: 0.0,
        dancing_ticks: 0.0,
        spinning_ticks: 0.0,
        spinning_ticks_old: 0.0,
        spinning: false,
        dash_cooldown: 0,
        dash_old: false,
        shake_time: 0,
        was_in_ground: false,
        fangs_started: false,
        fangs_life: 0,
        head_y_rots: [0.0; 2],
        head_y_rots_old: [0.0; 2],
        head_x_rots: [0.0; 2],
        head_x_rots_old: [0.0; 2],
    };
}

static EMPTY_KIND_ANIM: KindAnim = KindAnim::EMPTY;

fn has_kind_state(kind: EntityKind) -> bool {
    !matches!(
        kind,
        EntityKind::Tnt | EntityKind::FallingBlock | EntityKind::Item | EntityKind::ExperienceOrb
    )
}

#[derive(Clone, Debug)]
struct Sampled {
    shared: std::sync::Arc<ExtrasShared>,
    eating: bool,
    paddling_left: bool,
    paddling_right: bool,
    on_ground: bool,
    scale: f32,
    bounding_box_width: f32,
    bounding_box_height: f32,
    eye_height: f32,
    is_invisible: bool,
    is_in_water: bool,
    is_fully_frozen: bool,
    is_auto_spin_attack: bool,
    display_fire: bool,
    pose: Pose,
    bed_orientation: Option<Direction>,
}

impl Sampled {
    fn from_input(input: &TickInput) -> Sampled {
        Sampled {
            shared: input.shared.clone(),
            eating: input.eating,
            paddling_left: input.paddling_left,
            paddling_right: input.paddling_right,
            on_ground: input.on_ground,
            scale: input.scale,
            bounding_box_width: input.bounding_box_width,
            bounding_box_height: input.bounding_box_height,
            eye_height: input.eye_height,
            is_invisible: input.is_invisible,
            is_in_water: input.is_in_water,
            is_fully_frozen: input.is_fully_frozen,
            is_auto_spin_attack: input.is_auto_spin_attack,
            display_fire: input.display_fire,
            pose: input.pose,
            bed_orientation: input.bed_orientation,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EntityAnim {
    pub id: i32,
    pub kind: EntityKind,
    pos: [f64; 3],
    pos_old: [f64; 3],
    body_rot: f32,
    body_rot_old: f32,
    head_rot: f32,
    head_rot_old: f32,
    y_rot: f32,
    y_rot_old: f32,
    x_rot: f32,
    x_rot_old: f32,
    interp: Option<Interpolation>,
    is_living: bool,
    lerp_head_steps: i32,
    lerp_y_head_rot: f32,
    raw_pos: [f64; 3],
    raw_y_rot: f32,
    raw_x_rot: f32,
    raw_head_rot: f32,
    motion: Motion,
    body: Body,
    velocity_seq: u64,
    vehicle_body_rot: Option<(f32, f32)>,
    walk: WalkAnim,
    ticks: u32,
    death_time: u32,
    hurt_time: u32,
    swing_time: i32,
    swinging: bool,
    swing_left: bool,
    attack_anim: f32,
    attack_anim_old: f32,
    rng: JavaRandom,
    speed_sqr: f32,
    sampled: Sampled,
    started: bool,
    kind_state: Option<Box<KindAnim>>,
}

impl EntityAnim {
    pub fn new(id: i32, kind: EntityKind) -> EntityAnim {
        let mut rng = JavaRandom::new(id as i64);
        let tentacle_speed = 1.0 / (rng.next_f32() + 1.0) * 0.2;
        let tail_anim = rng.next_f32();
        EntityAnim {
            id,
            kind,
            pos: [0.0; 3],
            pos_old: [0.0; 3],
            body_rot: 0.0,
            body_rot_old: 0.0,
            head_rot: 0.0,
            head_rot_old: 0.0,
            y_rot: 0.0,
            y_rot_old: 0.0,
            x_rot: 0.0,
            x_rot_old: 0.0,
            interp: None,
            is_living: false,
            lerp_head_steps: 0,
            lerp_y_head_rot: 0.0,
            raw_pos: [0.0; 3],
            raw_y_rot: 0.0,
            raw_x_rot: 0.0,
            raw_head_rot: 0.0,
            motion: Motion::for_kind(kind),
            body: Body::default(),
            velocity_seq: 0,
            vehicle_body_rot: None,
            walk: WalkAnim::new(),
            ticks: 0,
            death_time: 0,
            hurt_time: 0,
            swing_time: -1,
            swinging: false,
            swing_left: false,
            attack_anim: 0.0,
            attack_anim_old: 0.0,
            rng,
            speed_sqr: 0.0,
            sampled: Sampled::from_input(&TickInput::new()),
            started: false,
            kind_state: has_kind_state(kind).then(|| {
                Box::new(KindAnim {
                    attach_pos: None,
                    teleport_interp: 0,
                    swell: 0.0,
                    swell_old: 0.0,
                    peek: 0.0,
                    peek_old: 0.0,
                    jump: 0.0,
                    jump_old: 0.0,
                    swell_counter: 0,
                    paddle: [0.0; 2],
                    paddle_old: [0.0; 2],
                    eat_ticks: 0,
                    shaking: false,
                    shake_anim: 0.0,
                    shake_anim_old: 0.0,
                    interest: 0.0,
                    interest_old: 0.0,
                    attack_ticks: 0,
                    stunned_ticks: 0,
                    roar_ticks: 0,
                    offer_flower_ticks: 0,
                    tendril_ticks: 0,
                    tendril_ticks_old: 0,
                    heart_ticks: 0,
                    heart_ticks_old: 0,
                    wiggle_ticks: WIGGLE_TICKS,
                    swim: 0.0,
                    swim_old: 0.0,
                    elytra: [0.0; 3],
                    elytra_old: [0.0; 3],
                    use_ticks: 0,
                    crouch: 0.0,
                    crouch_old: 0.0,
                    bear_stand: 0.0,
                    bear_stand_old: 0.0,
                    panda: [0.0; 3],
                    panda_old: [0.0; 3],
                    cat: [0.0; 3],
                    cat_old: [0.0; 3],
                    horse_eat: 0.0,
                    horse_eat_old: 0.0,
                    mouth_anim: 0.0,
                    mouth_anim_old: 0.0,
                    tentacle_speed,
                    tentacle_movement: 0.0,
                    tentacle_angle: 0.0,
                    tentacle_angle_old: 0.0,
                    squid_x_rot: 0.0,
                    squid_x_rot_old: 0.0,
                    squid_z_rot: 0.0,
                    squid_z_rot_old: 0.0,
                    squid_rotate_speed: 0.0,
                    squish: 0.0,
                    squish_old: 0.0,
                    target_squish: 0.0,
                    was_on_ground: false,
                    tail_anim,
                    tail_anim_old: tail_anim,
                    tail_anim_speed: 0.0,
                    spikes_anim: 0.0,
                    spikes_anim_old: 0.0,
                    flap_time: 0.0,
                    flap_time_old: 0.0,
                    flap: 0.0,
                    flap_old: 0.0,
                    flap_speed: 0.0,
                    flap_speed_old: 0.0,
                    flapping: 1.0,
                    bee_roll: 0.0,
                    bee_roll_old: 0.0,
                    axolotl: [0; 4],
                    axolotl_old: [0; 4],
                    roll_counter: 0,
                    jump_ticks: 0,
                    jump_duration: 0,
                    lowering_head: false,
                    lower_head_ticks: 0,
                    holding_ticks: 0.0,
                    holding_ticks_old: 0.0,
                    dancing_ticks: 0.0,
                    spinning_ticks: 0.0,
                    spinning_ticks_old: 0.0,
                    spinning: false,
                    dash_cooldown: 0,
                    dash_old: false,
                    shake_time: 0,
                    was_in_ground: false,
                    fangs_started: false,
                    fangs_life: EVOKER_FANGS_LIFE,
                    head_y_rots: [0.0; 2],
                    head_y_rots_old: [0.0; 2],
                    head_x_rots: [0.0; 2],
                    head_x_rots_old: [0.0; 2],
                })
            }),
        }
    }

    pub fn body_rot_pair(&self) -> (f32, f32) {
        (self.body_rot_old, self.body_rot)
    }

    pub fn is_living(&self) -> bool {
        self.is_living
    }

    pub fn set_vehicle_body_rot(&mut self, pair: Option<(f32, f32)>) {
        self.vehicle_body_rot = pair;
    }

    fn solve_body_rot(&self, head_rot: f32, partial: f32) -> f32 {
        let Some((old, new)) = self.vehicle_body_rot else {
            return rot_lerp(partial, self.body_rot_old, self.body_rot);
        };
        let mount = rot_lerp(partial, old, new);
        let head_diff = wrap_degrees(head_rot - mount).clamp(-85.0, 85.0);
        let body_rot = head_rot - head_diff;
        if head_diff.abs() > MAX_HEAD_ROTATION {
            body_rot + head_diff * 0.2
        } else {
            body_rot
        }
    }

    fn handle_movement_packets(&mut self, input: &TickInput) -> Option<([f64; 3], f32, f32)> {
        let mut deferred = None;
        if input.pos != self.raw_pos
            || input.y_rot != self.raw_y_rot
            || input.x_rot != self.raw_x_rot
        {
            self.raw_pos = input.pos;
            self.raw_y_rot = input.y_rot;
            self.raw_x_rot = input.x_rot;
            let too_big = distance_sqr(self.pos, input.pos) > MAX_INTERPOLATION_DISTANCE_SQR;
            match self.interp.take() {
                Some(mut interp) if too_big => {
                    self.set_pos(input.pos, input.is_passenger);
                    self.y_rot = input.y_rot;
                    self.x_rot = input.x_rot;
                    self.pos_old = self.pos;
                    self.y_rot_old = self.y_rot;
                    self.x_rot_old = self.x_rot;
                    interp.cancel();
                    self.interp = Some(interp);
                }
                Some(mut interp) => {
                    interp.interpolate_to(
                        Target {
                            pos: &mut self.pos,
                            y_rot: &mut self.y_rot,
                            x_rot: &mut self.x_rot,
                        },
                        input.pos,
                        input.y_rot,
                        input.x_rot,
                    );
                    self.interp = Some(interp);
                }
                None => {
                    deferred = Some((input.pos, input.y_rot % 360.0, input.x_rot % 360.0));
                    self.interp = None;
                }
            }
        }

        if self.is_living
            && let Some(head_rot) = input.head_rot
            && head_rot != self.raw_head_rot
        {
            self.raw_head_rot = head_rot;
            self.lerp_y_head_rot = head_rot;
            self.lerp_head_steps = LERP_HEAD_STEPS;
        }
        deferred
    }

    fn set_pos(&mut self, pos: [f64; 3], is_passenger: bool) {
        if !matches!(self.kind, EntityKind::Shulker) {
            self.pos = pos;
            return;
        }
        let old_block = block_pos(self.pos);
        self.pos = if is_passenger {
            pos
        } else {
            [
                pos[0].floor() + 0.5,
                (pos[1] + 0.5).floor(),
                pos[2].floor() + 0.5,
            ]
        };
        if self.ticks == 0 {
            return;
        }
        let new_block = block_pos(self.pos);
        let slid = new_block != old_block && !is_passenger;
        if let Some(k) = self.kind_state.as_deref_mut()
            && slid
            && Some(new_block) != k.attach_pos
        {
            k.attach_pos = Some(old_block);
            k.teleport_interp = SHULKER_TELEPORT_TICKS;
            self.pos_old = self.pos;
        }
    }

    fn tick_head_turn(&mut self, target: f32) {
        self.body_rot += wrap_degrees(target - self.body_rot) * 0.3;
        let head_diff = wrap_degrees(self.y_rot - self.body_rot);
        if head_diff.abs() > MAX_HEAD_ROTATION {
            self.body_rot += head_diff - head_diff.signum() * MAX_HEAD_ROTATION;
        }
    }

    #[allow(
        dead_code,
        reason = "the empty-world tick, used by this module's tests"
    )]
    pub fn tick(&mut self, input: TickInput) {
        self.tick_in(&input, &Void);
    }

    pub fn tick_in(&mut self, input: &TickInput, level: &dyn Level) {
        if !self.started {
            self.is_living = input.is_living;
            self.interp = interpolation_for(self.kind, input.is_living);
            self.set_pos(input.pos, input.is_passenger);
            self.y_rot = input.y_rot;
            self.x_rot = input.x_rot;
            self.body_rot = input.y_rot;
            self.head_rot = input.head_rot.unwrap_or(input.y_rot);
            self.pos_old = self.pos;
            self.raw_pos = input.pos;
            self.raw_y_rot = input.y_rot;
            self.raw_x_rot = input.x_rot;
            self.raw_head_rot = self.head_rot;
            if let Some(k) = self.kind_state.as_deref_mut() {
                k.peek = input.peek_target;
            }
            self.started = true;
        }

        let correction = self.handle_movement_packets(&input);

        if let Some((pos, y_rot, x_rot)) = correction {
            self.set_pos(pos, input.is_passenger);
            self.y_rot = y_rot;
            self.x_rot = x_rot;
        }

        if let Some((velocity, seq)) = input.velocity
            && seq != self.velocity_seq
        {
            self.velocity_seq = seq;
            self.body.velocity = velocity;
        }

        self.pos_old = self.pos;
        self.y_rot_old = self.y_rot;
        self.x_rot_old = self.x_rot;
        self.body_rot_old = self.body_rot;
        self.head_rot_old = self.head_rot;
        self.attack_anim_old = self.attack_anim;
        if let Some(k) = self.kind_state.as_deref_mut() {
            k.swell_old = k.swell;
            k.peek_old = k.peek;
            k.jump_old = k.jump;
            k.paddle_old = k.paddle;
        }

        if let Some(mut interp) = self.interp.take() {
            if interp.has_active_interpolation() {
                interp.interpolate(Target {
                    pos: &mut self.pos,
                    y_rot: &mut self.y_rot,
                    x_rot: &mut self.x_rot,
                });
            }
            self.interp = Some(interp);
        }
        if self.lerp_head_steps > 0 {
            self.head_rot = rot_lerp(
                1.0 / self.lerp_head_steps as f32,
                self.head_rot,
                self.lerp_y_head_rot,
            );
            self.lerp_head_steps -= 1;
        }

        if self.motion != Motion::Server && input.simulate {
            if input.shared.in_ground {
                self.body.in_ground = true;
            } else if self.k().was_in_ground {
                self.body.in_ground = false;
            }
            let shape = Shape {
                width: input.bounding_box_width as f64,
                height: input.bounding_box_height as f64,
                in_water: input.is_in_water,
                in_lava: input.is_in_lava,
                ticks: self.ticks,
                id: self.id,
            };
            self.pos = super::physics::step(self.motion, self.pos, &mut self.body, &shape, level);
        }

        if !self.is_living {
            self.body_rot = self.y_rot;
            self.head_rot = self.y_rot;
        }

        if let Some(k) = self.kind_state.as_deref_mut() {
            if k.teleport_interp > 0 {
                k.teleport_interp -= 1;
            } else {
                k.attach_pos = None;
            }

            if !input.dead {
                let dir = if input.ignited { 1 } else { input.swell_dir };
                k.swell_counter = (k.swell_counter + dir).clamp(0, MAX_SWELL);
            }
            k.swell = k.swell_counter as f32 / (MAX_SWELL - 2) as f32;

            if k.peek > input.peek_target {
                k.peek = (k.peek - 0.05).clamp(input.peek_target, 1.0);
            } else if k.peek < input.peek_target {
                k.peek = (k.peek + 0.05).clamp(0.0, input.peek_target);
            }

            if input.shared.rearing {
                k.jump = (k.jump + (1.0 - k.jump) * 0.4 + 0.05).min(1.0);
            } else {
                k.jump = (k.jump + (0.8 * k.jump * k.jump * k.jump - k.jump) * 0.6 - 0.05).max(0.0);
            }

            for (i, paddling) in [input.paddling_left, input.paddling_right]
                .into_iter()
                .enumerate()
            {
                if paddling {
                    k.paddle[i] += PADDLE_STEP;
                } else {
                    k.paddle[i] = 0.0;
                }
            }

            if k.eat_ticks > 0 {
                k.eat_ticks -= 1;
            }

            k.interest_old = k.interest;
            let interest_target = if input.shared.interested { 1.0 } else { 0.0 };
            k.interest += (interest_target - k.interest) * 0.4;

            k.shake_anim_old = k.shake_anim;
            if k.shaking {
                k.shake_anim += 0.05;
                if k.shake_anim_old >= 2.0 {
                    k.shaking = false;
                    k.shake_anim = 0.0;
                    k.shake_anim_old = 0.0;
                }
            }

            k.attack_ticks = (k.attack_ticks - 1).max(0);
            k.roar_ticks = (k.roar_ticks - 1).max(0);
            if k.stunned_ticks > 0 {
                k.stunned_ticks -= 1;
                if k.stunned_ticks == 0 {
                    k.roar_ticks = 20;
                }
            }
            k.offer_flower_ticks = (k.offer_flower_ticks - 1).max(0);

            let heart_beat_delay = 40
                - ((input.shared.anger_level as f32 / WARDEN_ANGRY_ANGER).clamp(0.0, 1.0) * 30.0)
                    .floor() as u32;
            if self.ticks % heart_beat_delay.max(1) == 0 {
                k.heart_ticks = 10;
            }
            k.tendril_ticks_old = k.tendril_ticks;
            k.tendril_ticks = (k.tendril_ticks - 1).max(0);
            k.heart_ticks_old = k.heart_ticks;
            k.heart_ticks = (k.heart_ticks - 1).max(0);

            k.wiggle_ticks = (k.wiggle_ticks + 1).min(WIGGLE_TICKS);

            k.swim_old = k.swim;
            k.swim = if input.pose == Pose::Swimming {
                (k.swim + 0.09).min(1.0)
            } else {
                (k.swim - 0.09).max(0.0)
            };

            k.elytra_old = k.elytra;
            let step = [
                self.pos[0] - self.pos_old[0],
                self.pos[1] - self.pos_old[1],
                self.pos[2] - self.pos_old[2],
            ];
            crate::entities::models::humanoid::elytra::tick_angles(
                &mut k.elytra,
                input.pose == Pose::FallFlying,
                input.pose == Pose::Crouching,
                step,
            );

            if input.shared.using_item {
                k.use_ticks += 1;
            } else {
                k.use_ticks = 0;
            }

            k.crouch_old = k.crouch;
            k.crouch = if input.shared.crouching {
                (k.crouch + 0.2).min(5.0)
            } else {
                0.0
            };

            k.bear_stand_old = k.bear_stand;
            k.bear_stand = if input.shared.standing {
                (k.bear_stand + 1.0).clamp(0.0, 6.0)
            } else {
                (k.bear_stand - 1.0).clamp(0.0, 6.0)
            };

            k.panda_old = k.panda;
            for (slot, active) in k.panda.iter_mut().zip([
                input.shared.sitting,
                input.shared.on_back,
                input.shared.panda_rolling,
            ]) {
                *slot = if active {
                    (*slot + 0.15).min(1.0)
                } else {
                    (*slot - 0.19).max(0.0)
                };
            }

            k.cat_old = k.cat;
            let cat_rates = [(0.15, 0.22), (0.08, 0.13), (0.1, 0.13)];
            let cat_flags = [input.lying, input.lying, input.relax_state_one];
            for ((slot, (up, down)), active) in k.cat.iter_mut().zip(cat_rates).zip(cat_flags) {
                *slot = if active {
                    (*slot + up).min(1.0)
                } else {
                    (*slot - down).max(0.0)
                };
            }

            k.horse_eat_old = k.horse_eat;
            if input.shared.rearing {
                k.horse_eat = 0.0;
                k.horse_eat_old = 0.0;
            } else if input.eating {
                k.horse_eat = (k.horse_eat + (1.0 - k.horse_eat) * 0.4 + 0.05).min(1.0);
            } else {
                k.horse_eat = (k.horse_eat + (0.0 - k.horse_eat) * 0.4 - 0.05).max(0.0);
            }

            k.mouth_anim_old = k.mouth_anim;
            if input.open_mouth {
                k.mouth_anim = (k.mouth_anim + (1.0 - k.mouth_anim) * 0.7 + 0.05).min(1.0);
            } else {
                k.mouth_anim = (k.mouth_anim + (0.0 - k.mouth_anim) * 0.7 - 0.05).max(0.0);
            }
        }

        let vel = [
            (self.pos[0] - self.pos_old[0]) as f32,
            (self.pos[1] - self.pos_old[1]) as f32,
            (self.pos[2] - self.pos_old[2]) as f32,
        ];
        self.speed_sqr = vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2];
        let horizontal_speed = (vel[0] * vel[0] + vel[2] * vel[2]).sqrt();

        if let Some(k) = self.kind_state.as_deref_mut() {
            k.tentacle_angle_old = k.tentacle_angle;
            k.squid_x_rot_old = k.squid_x_rot;
            k.squid_z_rot_old = k.squid_z_rot;
            k.tentacle_movement += k.tentacle_speed;
            if k.tentacle_movement > std::f32::consts::TAU {
                k.tentacle_movement = std::f32::consts::TAU;
            }
            if input.is_in_water {
                if k.tentacle_movement < std::f32::consts::PI {
                    let scale = k.tentacle_movement / std::f32::consts::PI;
                    k.tentacle_angle =
                        (scale * scale * std::f32::consts::PI).sin() * std::f32::consts::PI * 0.25;
                    if scale > 0.75 {
                        k.squid_rotate_speed = 1.0;
                    } else {
                        k.squid_rotate_speed *= 0.8;
                    }
                } else {
                    k.tentacle_angle = 0.0;
                    k.squid_rotate_speed *= 0.99;
                }
                k.squid_z_rot += std::f32::consts::PI * k.squid_rotate_speed * 1.5;
                k.squid_x_rot +=
                    (-horizontal_speed.atan2(vel[1]).to_degrees() - k.squid_x_rot) * 0.1;
            } else {
                k.tentacle_angle = k.tentacle_movement.sin().abs() * std::f32::consts::PI * 0.25;
                k.squid_x_rot += (-90.0 - k.squid_x_rot) * 0.02;
            }

            k.squish_old = k.squish;
            k.squish += (k.target_squish - k.squish) * 0.5;
            if input.on_ground && !k.was_on_ground {
                k.target_squish = -0.5;
            } else if !input.on_ground && k.was_on_ground {
                k.target_squish = 1.0;
            }
            k.was_on_ground = input.on_ground;
            k.target_squish *= match self.kind {
                EntityKind::MagmaCube => 0.9,
                _ => 0.6,
            };

            if !input.dead {
                k.tail_anim_old = k.tail_anim;
                if !input.is_in_water {
                    k.tail_anim_speed = 2.0;
                } else if input.shared.moving {
                    if k.tail_anim_speed < 0.5 {
                        k.tail_anim_speed = 4.0;
                    } else {
                        k.tail_anim_speed += (0.5 - k.tail_anim_speed) * 0.1;
                    }
                } else {
                    k.tail_anim_speed += (0.125 - k.tail_anim_speed) * 0.2;
                }
                k.tail_anim += k.tail_anim_speed;

                k.spikes_anim_old = k.spikes_anim;
                if !input.is_in_water {
                    k.spikes_anim = self.rng.next_f32();
                } else if input.shared.moving {
                    k.spikes_anim += (0.0 - k.spikes_anim) * 0.25;
                } else {
                    k.spikes_anim += (1.0 - k.spikes_anim) * 0.06;
                }
            }

            k.flap_time_old = k.flap_time;
            if !input.dead {
                let flap_speed = 0.2 / (horizontal_speed * 10.0 + 1.0) * 2.0f32.powf(vel[1]);
                k.flap_time += if dragon_phase_is_sitting(input.shared.dragon_phase) {
                    0.1
                } else {
                    flap_speed
                };
            }

            k.flap_old = k.flap;
            k.flap_speed_old = k.flap_speed;
            let wings_out = !input.on_ground
                && !(matches!(self.kind, EntityKind::Parrot) && input.is_passenger);
            let flap_step = if wings_out { 4.0 } else { -1.0 };
            k.flap_speed = (k.flap_speed + flap_step * 0.3).clamp(0.0, 1.0);
            if !input.on_ground && k.flapping < 1.0 {
                k.flapping = 1.0;
            }
            k.flapping *= 0.9;
            k.flap += k.flapping * 2.0;

            k.bee_roll_old = k.bee_roll;
            k.bee_roll = if input.shared.bee_rolling {
                (k.bee_roll + 0.2).min(1.0)
            } else {
                (k.bee_roll - 0.24).max(0.0)
            };

            let axolotl_moving = self.walk.speed > 1.0e-5
                || self.x_rot != self.x_rot_old
                || self.body_rot != self.body_rot_old;
            let in_water_state = !input.playing_dead && input.is_in_water;
            let on_ground_state = !input.playing_dead && !input.is_in_water && input.on_ground;
            k.axolotl_old = k.axolotl;
            for (slot, active) in k.axolotl.iter_mut().zip([
                input.playing_dead,
                in_water_state,
                on_ground_state,
                axolotl_moving,
            ]) {
                *slot = binary_animator_tick(*slot, active);
            }

            if input.shared.panda_rolling {
                k.roll_counter += 1;
                if k.roll_counter > 32 {
                    k.roll_counter = 0;
                }
            } else {
                k.roll_counter = 0;
            }

            if k.jump_ticks != k.jump_duration {
                k.jump_ticks += 1;
            } else if k.jump_duration != 0 {
                k.jump_ticks = 0;
                k.jump_duration = 0;
            }

            if k.lowering_head {
                k.lower_head_ticks += 1;
            } else {
                k.lower_head_ticks -= 2;
            }
            k.lower_head_ticks = k.lower_head_ticks.clamp(0, GOAT_LOWER_HEAD_TICKS);

            k.holding_ticks_old = k.holding_ticks;
            let holding_step = if input.shared.main_hand.is_empty() {
                -1.0
            } else {
                1.0
            };
            k.holding_ticks = (k.holding_ticks + holding_step).clamp(0.0, 5.0);
            if input.shared.dancing {
                k.dancing_ticks += 1.0;
                k.spinning_ticks_old = k.spinning_ticks;
                k.spinning = k.dancing_ticks % 55.0 < 15.0;
                let spin_step = if k.spinning { 1.0 } else { -1.0 };
                k.spinning_ticks = (k.spinning_ticks + spin_step).clamp(0.0, 15.0);
            } else {
                k.dancing_ticks = 0.0;
                k.spinning_ticks = 0.0;
                k.spinning_ticks_old = 0.0;
                k.spinning = false;
            }

            if self.started && input.shared.dash != k.dash_old && k.dash_cooldown == 0 {
                k.dash_cooldown = CAMEL_DASH_COOLDOWN;
            }
            k.dash_old = input.shared.dash;
            k.dash_cooldown = (k.dash_cooldown - 1).max(0);

            if input.shared.in_ground && !k.was_in_ground && self.started && k.shake_time <= 0 {
                k.shake_time = ARROW_SHAKE_TICKS;
            }
            k.was_in_ground = input.shared.in_ground;
            k.shake_time = (k.shake_time - 1).max(0);

            if k.fangs_started {
                k.fangs_life -= 1;
            }

            k.head_y_rots_old = k.head_y_rots;
            k.head_x_rots_old = k.head_x_rots;
            for i in 0..2 {
                match input.head_targets[i] {
                    Some(target) => {
                        let head_angle = (self.body_rot + 180.0 * i as f32).to_radians();
                        let hx = self.pos[0] + (head_angle.cos() * 1.3 * input.scale) as f64;
                        let hy = self.pos[1] + (2.2 * input.scale) as f64;
                        let hz = self.pos[2] + (head_angle.sin() * 1.3 * input.scale) as f64;
                        let dx = (target[0] - hx) as f32;
                        let dy = (target[1] - hy) as f32;
                        let dz = (target[2] - hz) as f32;
                        let flat = (dx * dx + dz * dz).sqrt();
                        let y_target = dz.atan2(dx).to_degrees() - 90.0;
                        let x_target = -dy.atan2(flat).to_degrees();
                        k.head_x_rots[i] = rot_lerp_capped(k.head_x_rots[i], x_target, 40.0);
                        k.head_y_rots[i] = rot_lerp_capped(k.head_y_rots[i], y_target, 10.0);
                    }
                    None => {
                        k.head_y_rots[i] = rot_lerp_capped(k.head_y_rots[i], self.body_rot, 10.0);
                    }
                }
            }
        }

        if input.is_passenger || input.dead {
            self.walk.stop();
        } else {
            let position_scale = if input.shared.is_baby { 3.0 } else { 1.0 };
            let target = if input.walk_halted {
                0.0
            } else {
                walk_target(self.kind, self.pos_old, self.pos)
            };
            self.walk.update(target, input.walk_factor, position_scale);
        }

        self.ticks = self.ticks.wrapping_add(1);
        if input.dead {
            self.death_time = (self.death_time + 1).min(20);
        } else {
            self.death_time = 0;
        }
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }

        const SWING_DURATION: i32 = 6;
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

        if self.is_living {
            let xd = (self.pos[0] - self.pos_old[0]) as f32;
            let zd = (self.pos[2] - self.pos_old[2]) as f32;
            let mut target = self.body_rot;
            if xd * xd + zd * zd > BODY_ROT_MOVEMENT_THRESHOLD {
                let walk_direction = zd.atan2(xd).to_degrees() - 90.0;
                let diff = (wrap_degrees(self.y_rot) - walk_direction).abs();
                target = if (95.0..265.0).contains(&diff) {
                    walk_direction - 180.0
                } else {
                    walk_direction
                };
            }
            if self.attack_anim > 0.0 {
                target = self.y_rot;
            }
            self.tick_head_turn(target);

            unwind(&mut self.y_rot_old, self.y_rot);
            unwind(&mut self.body_rot_old, self.body_rot);
            unwind(&mut self.x_rot_old, self.x_rot);
            unwind(&mut self.head_rot_old, self.head_rot);
        }

        self.sampled = Sampled::from_input(input);
    }

    fn k(&self) -> &KindAnim {
        self.kind_state.as_deref().unwrap_or(&EMPTY_KIND_ANIM)
    }

    fn with_kind(&mut self, f: impl FnOnce(&mut KindAnim)) {
        if let Some(k) = self.kind_state.as_deref_mut() {
            f(k);
        }
    }

    pub fn swing(&mut self, left_arm: bool) {
        if !self.swinging || self.swing_time >= 3 || self.swing_time < 0 {
            self.swing_time = -1;
            self.swinging = true;
            self.swing_left = left_arm;
        }
    }

    pub fn hurt(&mut self) {
        self.hurt_time = 10;
    }

    pub fn damage(&mut self) {
        self.walk.set_speed(1.5);
        self.hurt();
    }

    pub fn eat_grass(&mut self) {
        self.with_kind(|k| k.eat_ticks = EAT_ANIMATION_TICKS);
    }

    pub fn shake_wetness(&mut self) {
        self.with_kind(|k| {
            k.shaking = true;
            k.shake_anim = 0.0;
            k.shake_anim_old = 0.0;
        });
    }

    pub fn cancel_shake(&mut self) {
        self.with_kind(|k| {
            k.shaking = false;
            k.shake_anim = 0.0;
            k.shake_anim_old = 0.0;
        });
    }

    pub fn attack_animation(&mut self) {
        let fangs = matches!(self.kind, EntityKind::EvokerFangs);
        self.with_kind(|k| {
            if fangs {
                k.fangs_started = true;
            } else {
                k.attack_ticks = 10;
            }
        });
    }

    pub fn squid_anim_sync(&mut self) {
        self.with_kind(|k| k.tentacle_movement = 0.0);
    }

    pub fn jump(&mut self) {
        self.with_kind(|k| {
            k.jump_duration = RABBIT_JUMP_DURATION;
            k.jump_ticks = 0;
        });
    }

    pub fn lower_head(&mut self, lowering: bool) {
        self.with_kind(|k| k.lowering_head = lowering);
    }

    pub fn stunned(&mut self) {
        self.with_kind(|k| k.stunned_ticks = 40);
    }

    pub fn offer_flower(&mut self, offering: bool) {
        self.with_kind(|k| k.offer_flower_ticks = if offering { 400 } else { 0 });
    }

    pub fn tendril_shiver(&mut self) {
        self.with_kind(|k| k.tendril_ticks = 10);
    }

    pub fn wiggle(&mut self) {
        self.with_kind(|k| k.wiggle_ticks = 0);
    }

    pub fn position(&self, partial: f32) -> [f32; 3] {
        [
            lerp_f64(partial, self.pos_old[0], self.pos[0]) as f32,
            lerp_f64(partial, self.pos_old[1], self.pos[1]) as f32,
            lerp_f64(partial, self.pos_old[2], self.pos[2]) as f32,
        ]
    }

    pub fn look(&self, partial: f32) -> (f32, f32) {
        (
            rot_lerp(partial, self.y_rot_old, self.y_rot),
            lerp(partial, self.x_rot_old, self.x_rot),
        )
    }

    pub fn bounding_box_height(&self) -> f32 {
        self.sampled.bounding_box_height
    }

    pub fn is_invisible(&self) -> bool {
        self.sampled.is_invisible
    }

    pub fn shared(&self) -> &crate::entities::state::ExtrasShared {
        &self.sampled.shared
    }

    fn render_offset(&self, partial: f32) -> [f32; 3] {
        let k = self.k();
        let Some(old) = k.attach_pos.filter(|_| k.teleport_interp > 0) else {
            return [0.0; 3];
        };
        let mut scale = (k.teleport_interp as f32 - partial) / SHULKER_TELEPORT_TICKS as f32;
        scale *= scale;
        let current = block_pos(self.pos);
        [
            -((current[0] - old[0]) as f32 * scale),
            -((current[1] - old[1]) as f32 * scale),
            -((current[2] - old[2]) as f32 * scale),
        ]
    }

    pub fn sample(&self, partial: f32) -> EntityState {
        let head_rot = rot_lerp(partial, self.head_rot_old, self.head_rot);
        let body_rot = self.solve_body_rot(head_rot, partial);
        let x_rot = lerp(partial, self.x_rot_old, self.x_rot);
        let mut extras = Extras::with_shared(self.sampled.shared.clone());
        extras.eating = self.sampled.eating;
        extras.parrot_pose = if self.sampled.shared.sitting {
            2
        } else if self.sampled.on_ground {
            1
        } else {
            0
        };
        extras.on_ground = self.sampled.on_ground && self.speed_sqr < 1.0e-7;

        if let Some(k) = self.kind_state.as_deref() {
            extras.swell = lerp(partial, k.swell_old, k.swell);
            extras.peek = lerp(partial, k.peek_old, k.peek);
            extras.jump_progress = lerp(partial, k.jump_old, k.jump);
            extras.paddle_left = if self.sampled.paddling_left {
                lerp(partial, k.paddle_old[0], k.paddle[0])
            } else {
                0.0
            };
            extras.paddle_right = if self.sampled.paddling_right {
                lerp(partial, k.paddle_old[1], k.paddle[1])
            } else {
                0.0
            };
            extras.eating |= k.eat_ticks > 0;
            extras.render_offset = self.render_offset(partial);

            let eat = k.eat_ticks as f32;
            extras.head_eat_position_scale = if k.eat_ticks <= 0 {
                0.0
            } else if (4..=36).contains(&k.eat_ticks) {
                1.0
            } else if k.eat_ticks < 4 {
                (eat - partial) / 4.0
            } else {
                -((eat - 40.0) - partial) / 4.0
            };
            extras.head_eat_angle_scale = if k.eat_ticks > 4 && k.eat_ticks <= 36 {
                let scale = (eat - 4.0 - partial) / 32.0;
                0.628_318_55 + 0.219_911_49 * (scale * 28.7).sin()
            } else if k.eat_ticks > 0 {
                0.628_318_55
            } else {
                x_rot.to_radians()
            };

            extras.shake_anim = lerp(partial, k.shake_anim_old, k.shake_anim);
            extras.wet_shade = if k.shaking {
                (0.75 + extras.shake_anim / 2.0 * 0.25).min(1.0)
            } else {
                1.0
            };
            extras.head_roll_angle =
                lerp(partial, k.interest_old, k.interest) * 0.15 * std::f32::consts::PI;

            extras.attack_ticks = if k.attack_ticks > 0 {
                k.attack_ticks as f32 - partial
            } else {
                0.0
            };
            extras.attack_animation_remaining_ticks = k.attack_ticks;
            extras.stunned_ticks = if k.stunned_ticks > 0 {
                k.stunned_ticks as f32 - partial
            } else {
                0.0
            };
            extras.roar_animation = if k.roar_ticks > 0 {
                ((20 - k.roar_ticks) as f32 + partial) / 20.0
            } else {
                0.0
            };
            extras.offer_flower_ticks = k.offer_flower_ticks;

            extras.tendril_animation =
                lerp(partial, k.tendril_ticks_old as f32, k.tendril_ticks as f32) / 10.0;
            extras.heart_animation =
                lerp(partial, k.heart_ticks_old as f32, k.heart_ticks as f32) / 10.0;

            extras.wiggle = k.wiggle_ticks as f32 + partial;

            extras.swim_amount = lerp(partial, k.swim_old, k.swim);
            extras.elytra = std::array::from_fn(|i| lerp(partial, k.elytra_old[i], k.elytra[i]));
            extras.ticks_using_item = if extras.using_item {
                k.use_ticks as f32 + partial
            } else {
                0.0
            };
            extras.crouch_amount = lerp(partial, k.crouch_old, k.crouch);
            extras.stand_scale = lerp(partial, k.bear_stand_old, k.bear_stand) / 6.0;
            extras.sit_amount = lerp(partial, k.panda_old[0], k.panda[0]);
            extras.lie_on_back_amount = lerp(partial, k.panda_old[1], k.panda[1]);
            extras.roll_amount = if extras.is_baby {
                0.0
            } else {
                lerp(partial, k.panda_old[2], k.panda[2])
            };
            extras.lie_down_amount = lerp(partial, k.cat_old[0], k.cat[0]);
            extras.lie_down_amount_tail = lerp(partial, k.cat_old[1], k.cat[1]);
            extras.relax_state_one_amount = lerp(partial, k.cat_old[2], k.cat[2]);
            extras.eat_animation = lerp(partial, k.horse_eat_old, k.horse_eat);
            extras.stand_animation = extras.jump_progress;
            extras.feeding_animation = lerp(partial, k.mouth_anim_old, k.mouth_anim);

            extras.tentacle_angle = lerp(partial, k.tentacle_angle_old, k.tentacle_angle);
            extras.squid_x_body_rot = lerp(partial, k.squid_x_rot_old, k.squid_x_rot);
            extras.squid_z_body_rot = lerp(partial, k.squid_z_rot_old, k.squid_z_rot);

            extras.squish = lerp(partial, k.squish_old, k.squish);

            extras.spikes_animation = lerp(partial, k.spikes_anim_old, k.spikes_anim);
            extras.tail_animation = lerp(partial, k.tail_anim_old, k.tail_anim);

            extras.flap_time = if matches!(self.kind, EntityKind::Phantom) {
                (self.id * PHANTOM_FLAP_OFFSET) as f32 + self.ticks as f32 + partial
            } else {
                lerp(partial, k.flap_time_old, k.flap_time)
            };

            extras.flap = lerp(partial, k.flap_old, k.flap);
            extras.flap_speed = lerp(partial, k.flap_speed_old, k.flap_speed);
            extras.flap_angle = (extras.flap.sin() + 1.0) * extras.flap_speed;
            extras.bee_roll = lerp(partial, k.bee_roll_old, k.bee_roll);

            extras.playing_dead_factor =
                binary_animator_factor(k.axolotl_old[0], k.axolotl[0], partial);
            extras.in_water_factor =
                binary_animator_factor(k.axolotl_old[1], k.axolotl[1], partial);
            extras.on_ground_factor =
                binary_animator_factor(k.axolotl_old[2], k.axolotl[2], partial);
            extras.moving_factor = binary_animator_factor(k.axolotl_old[3], k.axolotl[3], partial);

            extras.roll_time = if k.roll_counter > 0 {
                k.roll_counter as f32 + partial
            } else {
                0.0
            };

            extras.jump_completion = if k.jump_duration == 0 {
                0.0
            } else {
                (k.jump_ticks as f32 + partial) / k.jump_duration as f32
            };

            let max_ramming = if extras.is_baby { 52.5 } else { 30.0 };
            extras.ramming_x_head_rot = (k.lower_head_ticks as f32 / GOAT_LOWER_HEAD_TICKS as f32
                * max_ramming)
                .to_radians();

            extras.holding_progress = lerp(partial, k.holding_ticks_old, k.holding_ticks) / 5.0;
            extras.spinning_progress = lerp(partial, k.spinning_ticks_old, k.spinning_ticks) / 15.0;
            extras.spinning = k.spinning;

            extras.jump_cooldown = (k.dash_cooldown as f32 - partial).max(0.0);

            extras.arrow_shake = k.shake_time as f32 - partial;

            extras.bite_progress = if !k.fangs_started {
                0.0
            } else {
                let remaining = k.fangs_life - 2;
                if remaining <= 0 {
                    1.0
                } else {
                    1.0 - (remaining as f32 - partial) / 20.0
                }
            };

            for i in 0..2 {
                extras.head_y_rots[i] = rot_lerp(partial, k.head_y_rots_old[i], k.head_y_rots[i]);
                extras.head_x_rots[i] = rot_lerp(partial, k.head_x_rots_old[i], k.head_x_rots[i]);
            }
        }

        EntityState {
            id: self.id,
            kind: self.kind,
            pos: self.position(partial),
            body_rot,
            y_rot: wrap_degrees(head_rot - body_rot),
            x_rot,
            age_ticks: self.ticks as f32 + partial,
            walk_pos: self.walk.position(partial),
            walk_speed: self.walk.speed(partial),
            death_time: if self.death_time > 0 {
                self.death_time as f32 + partial
            } else {
                0.0
            },
            scale: self.sampled.scale,
            age_scale: if self.sampled.shared.is_baby {
                0.5
            } else {
                1.0
            },
            bounding_box_width: self.sampled.bounding_box_width,
            bounding_box_height: self.sampled.bounding_box_height,
            eye_height: self.sampled.eye_height,
            is_invisible: self.sampled.is_invisible,
            is_in_water: self.sampled.is_in_water,
            is_fully_frozen: self.sampled.is_fully_frozen,
            is_upside_down: matches!(extras.name.as_deref(), Some("Dinnerbone") | Some("Grumm")),
            is_auto_spin_attack: self.sampled.is_auto_spin_attack,
            has_red_overlay: self.hurt_time > 0 || self.death_time > 0,
            display_fire: self.sampled.display_fire,
            pose: self.sampled.pose,
            bed_orientation: self.sampled.bed_orientation,
            attack_time: lerp(partial, self.attack_anim_old, self.attack_anim),
            swing_left: self.swing_left,
            extras,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mob_walks_a_third_of_a_packet_per_tick() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        input.pos = [3.0, 0.0, 0.0];
        anim.tick(input.clone());
        assert!((anim.position(1.0)[0] - 1.0).abs() < 1e-5);
        anim.tick(input.clone());
        assert!((anim.position(1.0)[0] - 2.0).abs() < 1e-5);
        anim.tick(input);
        assert!((anim.position(1.0)[0] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn a_packet_correction_is_a_snap_and_not_a_lerp() {
        let mut anim = EntityAnim::new(1, EntityKind::ItemFrame);
        let mut input = TickInput::new();
        input.is_living = false;
        anim.tick(input.clone());
        input.pos = [3.0, 0.0, 0.0];
        anim.tick(input);
        assert!(
            (anim.position(0.0)[0] - 3.0).abs() < 1e-5,
            "{}",
            anim.position(0.0)[0]
        );
        assert!(
            (anim.position(1.0)[0] - 3.0).abs() < 1e-5,
            "{}",
            anim.position(1.0)[0]
        );
    }

    #[test]
    fn an_arrow_flies_on_the_velocity_it_was_given() {
        let mut anim = EntityAnim::new(1, EntityKind::Arrow);
        let mut input = TickInput::new();
        input.is_living = false;
        input.velocity = Some(([1.0, 0.0, 0.0], 1));
        anim.tick(input.clone());
        assert!(
            (anim.position(1.0)[0] - 0.99).abs() < 1e-9,
            "{}",
            anim.position(1.0)[0]
        );
        assert!(
            (anim.position(0.5)[0] - 0.495).abs() < 1e-9,
            "{}",
            anim.position(0.5)[0]
        );
        anim.tick(input);
        assert!(anim.position(1.0)[1] < 0.0, "{}", anim.position(1.0)[1]);
    }

    #[test]
    fn a_jump_past_sixty_four_blocks_snaps() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        input.pos = [100.0, 0.0, 0.0];
        anim.tick(input);
        assert_eq!(anim.position(0.0)[0], 100.0);
        assert_eq!(anim.position(1.0)[0], 100.0);
    }

    #[test]
    fn a_head_yaw_packet_takes_three_ticks() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        let mut input = TickInput::new();
        input.head_rot = Some(0.0);
        anim.tick(input.clone());
        input.head_rot = Some(90.0);
        anim.tick(input.clone());
        let head = |anim: &EntityAnim| anim.sample(1.0).y_rot + anim.sample(1.0).body_rot;
        assert!((head(&anim) - 30.0).abs() < 1e-3, "{}", head(&anim));
        anim.tick(input.clone());
        assert!((head(&anim) - 60.0).abs() < 1e-3, "{}", head(&anim));
        anim.tick(input);
        assert!((head(&anim) - 90.0).abs() < 1e-3, "{}", head(&anim));
    }

    #[test]
    fn a_standing_mob_turns_its_body_only_as_far_as_the_clamp() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        input.y_rot = 90.0;
        for _ in 0..10 {
            anim.tick(input.clone());
        }
        assert!(
            (anim.body_rot_pair().1 - 40.0).abs() < 1e-3,
            "{}",
            anim.body_rot_pair().1
        );
    }

    #[test]
    fn the_body_follows_the_direction_of_travel() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        for step in 1..30 {
            input.pos[0] = step as f64;
            anim.tick(input.clone());
        }
        assert!(
            (anim.body_rot_pair().1 + 50.0).abs() < 1e-3,
            "{}",
            anim.body_rot_pair().1
        );
    }

    #[test]
    fn a_shulker_snaps_to_the_block_centre_and_slides() {
        let mut anim = EntityAnim::new(1, EntityKind::Shulker);
        let mut input = TickInput::new();
        input.pos = [0.2, 64.0, 0.7];
        anim.tick(input.clone());
        assert_eq!(anim.position(1.0), [0.5, 64.0, 0.5]);
        input.pos = [3.2, 64.0, 0.7];
        anim.tick(input);
        assert_eq!(anim.position(1.0), [3.5, 64.0, 0.5]);
        let offset = anim.sample(1.0).extras.render_offset;
        let scale = (4.0f32 / 6.0) * (4.0 / 6.0);
        assert!((offset[0] + 3.0 * scale).abs() < 1e-4, "{offset:?}");
        for _ in 0..6 {
            anim.tick(TickInput {
                pos: [3.2, 64.0, 0.7],
                ..TickInput::new()
            });
        }
        assert_eq!(anim.sample(1.0).extras.render_offset, [0.0; 3]);
    }

    #[test]
    fn walk_position_trails_the_tick_by_one_speed_step() {
        let mut walk = WalkAnim::new();
        walk.update(1.0, 0.4, 1.0);
        assert!((walk.position(1.0) - 0.4).abs() < 1e-6);
        assert!((walk.position(0.0) - 0.0).abs() < 1e-6);
        assert!((walk.speed(1.0) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn a_new_entity_does_not_interpolate_out_of_the_origin() {
        let mut anim = EntityAnim::new(7, EntityKind::Pig);
        let mut input = TickInput::new();
        input.pos = [10.0, 64.0, -3.0];
        anim.tick(input);
        assert_eq!(anim.position(0.0), [10.0, 64.0, -3.0]);
    }

    #[test]
    fn head_yaw_is_sampled_relative_to_the_body() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        let mut input = TickInput::new();
        input.y_rot = 350.0;
        input.head_rot = Some(10.0);
        anim.tick(input.clone());
        anim.tick(input);
        let st = anim.sample(1.0);
        assert!((st.y_rot - 20.0).abs() < 1e-3, "{}", st.y_rot);
    }

    #[test]
    fn the_sheep_wool_nibble_survives_the_sheared_bit() {
        assert_eq!(sheep_wool_color(0x00), 0);
        assert_eq!(sheep_wool_color(0x10), 0);
        assert_eq!(sheep_wool_color(0x0e), 14);
        assert_eq!(sheep_wool_color(0x1f), 15);
    }

    #[test]
    fn walk_target_is_four_times_the_horizontal_step_clamped_to_one() {
        let t = walk_target(EntityKind::Pig, [0.0, 64.0, 0.0], [0.1, 64.0, 0.0]);
        assert!((t - 0.4).abs() < 1e-6, "{t}");
        let t = walk_target(EntityKind::Pig, [0.0, 64.0, 0.0], [0.5, 64.0, 0.0]);
        assert!((t - 1.0).abs() < 1e-6, "{t}");
    }

    #[test]
    fn only_a_flying_animal_counts_its_vertical_step() {
        let up = |kind| walk_target(kind, [0.0, 0.0, 0.0], [0.0, 0.1, 0.0]);
        assert_eq!(up(EntityKind::Pig), 0.0);
        assert!((up(EntityKind::Parrot) - 0.4).abs() < 1e-6);
        assert!((up(EntityKind::Bee) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn the_creaking_clamps_its_walk_target_to_three() {
        let t = walk_target(EntityKind::Creaking, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        assert!((t - 3.0).abs() < 1e-6, "{t}");
    }

    #[test]
    fn a_passenger_has_no_walk_cycle_at_all() {
        let mut anim = EntityAnim::new(1, EntityKind::Pig);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        walk_by(&mut anim, &input, 1.0, 6);
        assert!(anim.sample(1.0).walk_speed > 0.0);
        input.is_passenger = true;
        anim.tick(input);
        let st = anim.sample(1.0);
        assert_eq!(st.walk_speed, 0.0);
        assert_eq!(st.walk_pos, 0.0);
    }

    #[test]
    fn a_baby_runs_its_walk_phase_three_times_as_fast() {
        let mut adult = EntityAnim::new(1, EntityKind::Pig);
        let mut baby = EntityAnim::new(2, EntityKind::Pig);
        let mut input = TickInput::new();
        adult.tick(input.clone());
        walk_by(&mut adult, &input, 1.0, 3);
        input.shared_mut().is_baby = true;
        baby.tick(input.clone());
        walk_by(&mut baby, &input, 1.0, 3);
        assert!((baby.sample(1.0).walk_pos - adult.sample(1.0).walk_pos * 3.0).abs() < 1e-6);
    }

    #[test]
    fn a_swing_lasts_six_ticks_and_can_be_restarted_halfway() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        anim.swing(false);
        for expected in [0.0, 1.0, 2.0, 3.0, 4.0, 5.0] {
            anim.tick(TickInput::new());
            let st = anim.sample(1.0);
            assert!(
                (st.attack_time - expected / 6.0).abs() < 1e-6,
                "{} vs {}",
                st.attack_time,
                expected / 6.0
            );
        }
        anim.tick(TickInput::new());
        assert_eq!(anim.sample(1.0).attack_time, 0.0);
    }

    #[test]
    fn a_restart_before_the_halfway_point_is_ignored() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        anim.swing(false);
        anim.tick(TickInput::new());
        anim.tick(TickInput::new());
        anim.swing(true);
        assert!(!anim.sample(1.0).swing_left);
        for _ in 0..3 {
            anim.tick(TickInput::new());
        }
        anim.swing(true);
        assert!(anim.sample(1.0).swing_left);
    }

    #[test]
    fn a_hit_flashes_red_for_ten_ticks_and_kicks_the_limbs() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        anim.damage();
        anim.tick(TickInput::new());
        assert!(anim.sample(1.0).walk_speed > 0.0);
        for _ in 0..9 {
            assert!(anim.sample(0.0).has_red_overlay);
            anim.tick(TickInput::new());
        }
        assert!(!anim.sample(0.0).has_red_overlay);
    }

    #[test]
    fn the_creeper_fuse_reaches_one_two_ticks_before_the_explosion() {
        let mut anim = EntityAnim::new(1, EntityKind::Creeper);
        let mut input = TickInput::new();
        input.swell_dir = 1;
        for _ in 0..28 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(1.0).extras.swell - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_shulker_shell_eases_toward_its_peek_target() {
        let mut anim = EntityAnim::new(1, EntityKind::Shulker);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        input.peek_target = 1.0;
        anim.tick(input.clone());
        assert!((anim.sample(1.0).extras.peek - 0.05).abs() < 1e-6);
        for _ in 0..19 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(1.0).extras.peek - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_resting_paddle_reads_zero_rather_than_its_phase() {
        let mut anim = EntityAnim::new(1, EntityKind::OakBoat);
        let mut input = TickInput::new();
        input.paddling_left = true;
        anim.tick(input.clone());
        anim.tick(input.clone());
        let phase = anim.sample(1.0).extras.paddle_left;
        assert!(
            (phase - 2.0 * std::f32::consts::FRAC_PI_8).abs() < 1e-6,
            "{phase}"
        );
        assert_eq!(anim.sample(1.0).extras.paddle_right, 0.0);
        input.paddling_left = false;
        anim.tick(input);
        assert_eq!(anim.sample(1.0).extras.paddle_left, 0.0);
    }

    #[test]
    fn grazing_holds_the_eating_flag_for_forty_ticks() {
        let mut anim = EntityAnim::new(1, EntityKind::Sheep);
        anim.eat_grass();
        for _ in 0..40 {
            assert!(anim.sample(0.0).extras.eating);
            anim.tick(TickInput::new());
        }
        assert!(!anim.sample(0.0).extras.eating);
    }

    #[test]
    fn a_wolf_shake_runs_for_forty_ticks_and_shades_the_coat() {
        let mut anim = EntityAnim::new(1, EntityKind::Wolf);
        anim.shake_wetness();
        for _ in 0..20 {
            anim.tick(TickInput::new());
        }
        let st = anim.sample(1.0);
        assert!(
            (st.extras.shake_anim - 1.0).abs() < 1e-5,
            "{}",
            st.extras.shake_anim
        );
        assert!(
            (st.extras.wet_shade - 0.875).abs() < 1e-5,
            "{}",
            st.extras.wet_shade
        );
        for _ in 0..20 {
            anim.tick(TickInput::new());
        }
        assert!(anim.sample(1.0).extras.shake_anim > 1.99);
        anim.tick(TickInput::new());
        anim.tick(TickInput::new());
        let st = anim.sample(1.0);
        assert_eq!(st.extras.shake_anim, 0.0);
        assert_eq!(st.extras.wet_shade, 1.0);
    }

    #[test]
    fn the_swim_blend_takes_twelve_ticks_each_way() {
        let mut anim = EntityAnim::new(1, EntityKind::Drowned);
        let mut input = TickInput::new();
        input.pose = Pose::Swimming;
        for _ in 0..12 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(1.0).extras.swim_amount - 1.0).abs() < 1e-6);
        input.pose = Pose::Standing;
        for _ in 0..12 {
            anim.tick(input.clone());
        }
        assert_eq!(anim.sample(1.0).extras.swim_amount, 0.0);
    }

    #[test]
    fn a_ravager_stun_decays_into_a_roar() {
        let mut anim = EntityAnim::new(1, EntityKind::Ravager);
        anim.stunned();
        for _ in 0..40 {
            anim.tick(TickInput::new());
        }
        assert_eq!(anim.sample(0.0).extras.stunned_ticks, 0.0);
        anim.tick(TickInput::new());
        let st = anim.sample(0.0);
        assert!(
            (st.extras.roar_animation - 0.05).abs() < 1e-6,
            "{}",
            st.extras.roar_animation
        );
        for _ in 0..19 {
            anim.tick(TickInput::new());
        }
        assert_eq!(anim.sample(0.0).extras.roar_animation, 0.0);
    }

    fn tick_twice(anim: &mut EntityAnim, input: &TickInput, step: [f64; 3]) {
        let mut first = input.clone();
        first.pos = [0.0, 64.0, 0.0];
        anim.tick(first.clone());
        let mut second = first;
        second.pos = [step[0] * 3.0, 64.0 + step[1] * 3.0, step[2] * 3.0];
        anim.tick(second);
    }

    fn walk_by(anim: &mut EntityAnim, input: &TickInput, step: f64, ticks: usize) {
        let mut moving = input.clone();
        for _ in 0..ticks {
            moving.pos[0] += step;
            anim.tick(moving.clone());
        }
    }

    #[test]
    fn java_random_reproduces_the_jdk_lcg() {
        let mut rng = JavaRandom::new(0);
        let f = rng.next_f32();
        assert!((f - 12_263_604.0 / 16_777_216.0).abs() < 1e-7, "{f}");
    }

    #[test]
    fn the_squid_tentacle_speed_comes_off_the_id_seeded_stream() {
        let anim = EntityAnim::new(0, EntityKind::Squid);
        let expected = 1.0 / (12_263_604.0 / 16_777_216.0 + 1.0) * 0.2;
        assert!(
            (anim.k().tentacle_speed - expected).abs() < 1e-7,
            "{}",
            anim.k().tentacle_speed
        );
    }

    #[test]
    fn a_beached_squid_pitches_nose_down_two_percent_a_tick() {
        let mut anim = EntityAnim::new(1, EntityKind::Squid);
        anim.tick(TickInput::new());
        assert!((anim.sample(1.0).extras.squid_x_body_rot + 1.8).abs() < 1e-5);
        anim.tick(TickInput::new());
        let expected = -1.8 + (-90.0 + 1.8) * 0.02;
        assert!((anim.sample(1.0).extras.squid_x_body_rot - expected).abs() < 1e-4);
        let phase = 2.0 * anim.k().tentacle_speed;
        let angle = phase.sin().abs() * std::f32::consts::FRAC_PI_4;
        assert!((anim.sample(1.0).extras.tentacle_angle - angle).abs() < 1e-6);
    }

    #[test]
    fn a_diving_squid_pitches_toward_its_direction_of_travel() {
        let mut anim = EntityAnim::new(1, EntityKind::Squid);
        let mut input = TickInput::new();
        input.is_in_water = true;
        tick_twice(&mut anim, &input, [0.0, -1.0, 0.0]);
        assert!((anim.sample(1.0).extras.squid_x_body_rot + 18.0).abs() < 1e-3);
        assert_eq!(anim.sample(1.0).extras.squid_z_body_rot, 0.0);
    }

    #[test]
    fn the_squid_stroke_kicks_past_three_quarters_and_then_rolls() {
        let mut anim = EntityAnim::new(1, EntityKind::Squid);
        let mut input = TickInput::new();
        input.is_in_water = true;
        let ticks = (0.75 * std::f32::consts::PI / anim.k().tentacle_speed).ceil() as i32;
        for _ in 0..ticks {
            anim.tick(input.clone());
        }
        assert_eq!(anim.k().squid_rotate_speed, 1.0);
        let before = anim.sample(1.0).extras.squid_z_body_rot;
        anim.tick(input);
        let after = anim.sample(1.0).extras.squid_z_body_rot;
        assert!((after - before - std::f32::consts::PI * 1.5).abs() < 1e-4);
    }

    #[test]
    fn entity_event_19_restarts_the_tentacle_stroke() {
        let mut anim = EntityAnim::new(1, EntityKind::Squid);
        let mut input = TickInput::new();
        input.is_in_water = true;
        for _ in 0..40 {
            anim.tick(input.clone());
        }
        assert!(anim.k().tentacle_movement > 0.0);
        anim.squid_anim_sync();
        assert_eq!(anim.k().tentacle_movement, 0.0);
    }

    #[test]
    fn landing_flattens_a_slime_and_leaving_the_ground_stretches_it() {
        let mut anim = EntityAnim::new(1, EntityKind::Slime);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        input.on_ground = true;
        anim.tick(input.clone());
        assert_eq!(anim.sample(1.0).extras.squish, 0.0);
        anim.tick(input.clone());
        assert!((anim.sample(1.0).extras.squish + 0.15).abs() < 1e-6);
        input.on_ground = false;
        anim.tick(input.clone());
        anim.tick(input);
        assert!(anim.sample(1.0).extras.squish > 0.0);
    }

    #[test]
    fn a_magma_cube_holds_its_squish_longer_than_a_slime() {
        let land = |kind| {
            let mut anim = EntityAnim::new(1, kind);
            let mut input = TickInput::new();
            anim.tick(input.clone());
            input.on_ground = true;
            for _ in 0..4 {
                anim.tick(input.clone());
            }
            anim.sample(1.0).extras.squish
        };
        assert!(land(EntityKind::MagmaCube) < land(EntityKind::Slime));
    }

    #[test]
    fn an_idle_guardian_eases_its_tail_and_raises_its_spikes() {
        let mut anim = EntityAnim::new(1, EntityKind::Guardian);
        let start = anim.k().tail_anim;
        let mut input = TickInput::new();
        input.is_in_water = true;
        anim.tick(input.clone());
        let st = anim.sample(1.0);
        assert!((st.extras.tail_animation - (start + 0.025)).abs() < 1e-6);
        assert!((st.extras.spikes_animation - 0.06).abs() < 1e-6);
    }

    #[test]
    fn a_swimming_guardian_snaps_its_tail_speed_to_four() {
        let mut anim = EntityAnim::new(1, EntityKind::Guardian);
        let start = anim.k().tail_anim;
        let mut input = TickInput::new();
        input.is_in_water = true;
        input.shared_mut().moving = true;
        anim.tick(input.clone());
        assert!((anim.sample(1.0).extras.tail_animation - (start + 4.0)).abs() < 1e-5);
        assert_eq!(anim.sample(1.0).extras.spikes_animation, 0.0);
    }

    #[test]
    fn a_flopping_guardian_beats_its_tail_at_a_fixed_rate() {
        let mut anim = EntityAnim::new(1, EntityKind::Guardian);
        let start = anim.k().tail_anim;
        anim.tick(TickInput::new());
        anim.tick(TickInput::new());
        assert!((anim.sample(1.0).extras.tail_animation - (start + 4.0)).abs() < 1e-5);
    }

    #[test]
    fn an_airborne_parrot_reaches_full_wing_speed_in_one_tick() {
        let mut anim = EntityAnim::new(1, EntityKind::Parrot);
        let input = TickInput::new();
        anim.tick(input.clone());
        let st = anim.sample(1.0);
        assert!((st.extras.flap_speed - 1.0).abs() < 1e-6);
        assert!((st.extras.flap - 1.8).abs() < 1e-6);
        assert!((st.extras.flap_angle - (1.8f32.sin() + 1.0)).abs() < 1e-6);
        assert_eq!(st.extras.parrot_pose, 0);
    }

    #[test]
    fn a_grounded_chicken_folds_its_wings() {
        let mut anim = EntityAnim::new(1, EntityKind::Chicken);
        let mut input = TickInput::new();
        input.on_ground = true;
        anim.tick(input.clone());
        assert_eq!(anim.sample(1.0).extras.flap_speed, 0.0);
        assert_eq!(anim.sample(1.0).extras.flap_angle, 0.0);
        assert!(anim.sample(1.0).extras.flap > 0.0);
    }

    #[test]
    fn a_riding_parrot_folds_its_wings_but_a_riding_chicken_does_not() {
        let speed = |kind| {
            let mut anim = EntityAnim::new(1, kind);
            let mut input = TickInput::new();
            input.is_passenger = true;
            anim.tick(input);
            anim.sample(1.0).extras.flap_speed
        };
        assert_eq!(speed(EntityKind::Parrot), 0.0);
        assert!((speed(EntityKind::Chicken) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_sitting_parrot_reads_the_sitting_pose() {
        let mut anim = EntityAnim::new(1, EntityKind::Parrot);
        let mut input = TickInput::new();
        input.shared_mut().sitting = true;
        anim.tick(input.clone());
        assert_eq!(anim.sample(1.0).extras.parrot_pose, 2);
        input.shared_mut().sitting = false;
        input.on_ground = true;
        anim.tick(input);
        assert_eq!(anim.sample(1.0).extras.parrot_pose, 1);
    }

    #[test]
    fn the_axolotl_water_blend_is_a_sine_ease_over_ten_ticks() {
        let mut anim = EntityAnim::new(1, EntityKind::Axolotl);
        let mut input = TickInput::new();
        input.is_in_water = true;
        for _ in 0..5 {
            anim.tick(input.clone());
        }
        let st = anim.sample(1.0);
        assert!(
            (st.extras.in_water_factor - 0.5).abs() < 1e-6,
            "{}",
            st.extras.in_water_factor
        );
        assert_eq!(st.extras.playing_dead_factor, 0.0);
        assert_eq!(st.extras.on_ground_factor, 0.0);
        for _ in 0..5 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(1.0).extras.in_water_factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn playing_dead_takes_the_axolotl_out_of_every_other_state() {
        let mut anim = EntityAnim::new(1, EntityKind::Axolotl);
        let mut input = TickInput::new();
        input.is_in_water = true;
        for _ in 0..10 {
            anim.tick(input.clone());
        }
        input.playing_dead = true;
        for _ in 0..10 {
            anim.tick(input.clone());
        }
        let st = anim.sample(1.0);
        assert!((st.extras.playing_dead_factor - 1.0).abs() < 1e-6);
        assert_eq!(st.extras.in_water_factor, 0.0);
    }

    #[test]
    fn an_axolotl_on_land_uses_the_ground_blend() {
        let mut anim = EntityAnim::new(1, EntityKind::Axolotl);
        let mut input = TickInput::new();
        input.on_ground = true;
        for _ in 0..10 {
            anim.tick(input.clone());
        }
        let st = anim.sample(1.0);
        assert!((st.extras.on_ground_factor - 1.0).abs() < 1e-6);
        assert_eq!(st.extras.in_water_factor, 0.0);
        assert_eq!(st.extras.moving_factor, 0.0);
    }

    #[test]
    fn a_walking_axolotl_raises_the_moving_blend() {
        let mut anim = EntityAnim::new(1, EntityKind::Axolotl);
        let mut input = TickInput::new();
        input.on_ground = true;
        anim.tick(input.clone());
        walk_by(&mut anim, &input, 1.0, 12);
        assert!((anim.sample(1.0).extras.moving_factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_phantom_flap_phase_is_offset_by_three_ticks_per_id() {
        let mut anim = EntityAnim::new(7, EntityKind::Phantom);
        anim.tick(TickInput::new());
        assert!((anim.sample(0.0).extras.flap_time - 22.0).abs() < 1e-6);
    }

    #[test]
    fn a_hovering_dragon_flaps_at_a_tenth_a_tick() {
        let mut anim = EntityAnim::new(1, EntityKind::EnderDragon);
        let mut input = TickInput::new();
        input.shared_mut().dragon_phase = 10;
        anim.tick(input.clone());
        anim.tick(input);
        assert!((anim.sample(1.0).extras.flap_time - 0.2).abs() < 1e-6);
    }

    #[test]
    fn a_still_dragon_flaps_at_the_full_two_tenths() {
        let mut anim = EntityAnim::new(1, EntityKind::EnderDragon);
        let mut input = TickInput::new();
        input.shared_mut().dragon_phase = 0;
        anim.tick(input.clone());
        anim.tick(input);
        assert!((anim.sample(1.0).extras.flap_time - 0.4).abs() < 1e-6);
    }

    #[test]
    fn dragon_sitting_phases_are_the_three_sitting_ordinals_and_the_hover() {
        for phase in [5, 6, 7, 10] {
            assert!(dragon_phase_is_sitting(phase), "{phase}");
        }
        for phase in [0, 1, 2, 3, 4, 8, 9] {
            assert!(!dragon_phase_is_sitting(phase), "{phase}");
        }
    }

    #[test]
    fn a_knocked_over_bee_rolls_in_five_ticks_and_back_out_in_five() {
        let mut anim = EntityAnim::new(1, EntityKind::Bee);
        let mut input = TickInput::new();
        input.shared_mut().bee_rolling = true;
        for _ in 0..5 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(1.0).extras.bee_roll - 1.0).abs() < 1e-6);
        input.shared_mut().bee_rolling = false;
        for _ in 0..5 {
            anim.tick(input.clone());
        }
        assert_eq!(anim.sample(1.0).extras.bee_roll, 0.0);
    }

    #[test]
    fn a_bee_only_counts_as_standing_when_it_is_still() {
        let mut anim = EntityAnim::new(1, EntityKind::Bee);
        let mut input = TickInput::new();
        input.on_ground = true;
        tick_twice(&mut anim, &input, [0.0, 0.0, 0.0]);
        assert!(anim.sample(1.0).extras.on_ground);
        tick_twice(&mut anim, &input, [0.1, 0.0, 0.0]);
        assert!(!anim.sample(1.0).extras.on_ground);
    }

    #[test]
    fn a_rolling_panda_counts_its_roll_out_over_thirty_two_ticks() {
        let mut anim = EntityAnim::new(1, EntityKind::Panda);
        let mut input = TickInput::new();
        input.shared_mut().panda_rolling = true;
        anim.tick(input.clone());
        assert!((anim.sample(0.5).extras.roll_time - 1.5).abs() < 1e-6);
        for _ in 0..31 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(0.0).extras.roll_time - 32.0).abs() < 1e-6);
        anim.tick(input);
        assert_eq!(anim.sample(0.0).extras.roll_time, 0.0);
    }

    #[test]
    fn the_horse_mouth_opens_on_flag_bit_forty() {
        assert!(horse_open_mouth(0x40));
        assert!(!horse_open_mouth(0x3a));
        let mut anim = EntityAnim::new(1, EntityKind::Horse);
        let mut input = TickInput::new();
        input.open_mouth = true;
        anim.tick(input.clone());
        assert!((anim.sample(1.0).extras.feeding_animation - 0.75).abs() < 1e-6);
        input.open_mouth = false;
        for _ in 0..10 {
            anim.tick(input.clone());
        }
        assert_eq!(anim.sample(1.0).extras.feeding_animation, 0.0);
    }

    #[test]
    fn entity_event_one_starts_a_fifteen_tick_rabbit_hop() {
        let mut anim = EntityAnim::new(1, EntityKind::Rabbit);
        anim.jump();
        anim.tick(TickInput::new());
        assert!((anim.sample(0.0).extras.jump_completion - 1.0 / 15.0).abs() < 1e-6);
        for _ in 0..14 {
            anim.tick(TickInput::new());
        }
        assert!((anim.sample(0.0).extras.jump_completion - 1.0).abs() < 1e-6);
        anim.tick(TickInput::new());
        assert_eq!(anim.sample(0.0).extras.jump_completion, 0.0);
    }

    #[test]
    fn a_goat_lowers_its_head_over_twenty_ticks_and_lifts_it_in_ten() {
        let mut anim = EntityAnim::new(1, EntityKind::Goat);
        anim.lower_head(true);
        for _ in 0..20 {
            anim.tick(TickInput::new());
        }
        let full = 30.0f32.to_radians();
        assert!((anim.sample(0.0).extras.ramming_x_head_rot - full).abs() < 1e-6);
        anim.lower_head(false);
        for _ in 0..10 {
            anim.tick(TickInput::new());
        }
        assert_eq!(anim.sample(0.0).extras.ramming_x_head_rot, 0.0);
    }

    #[test]
    fn a_kid_lowers_its_head_further_than_an_adult() {
        let mut anim = EntityAnim::new(1, EntityKind::Goat);
        anim.lower_head(true);
        let mut input = TickInput::new();
        input.shared_mut().is_baby = true;
        for _ in 0..20 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(0.0).extras.ramming_x_head_rot - 52.5f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn a_dancing_allay_spins_for_fifteen_ticks_in_every_fifty_five() {
        let mut anim = EntityAnim::new(1, EntityKind::Allay);
        let mut input = TickInput::new();
        input.shared_mut().dancing = true;

        for _ in 0..14 {
            anim.tick(input.clone());
        }
        let st = anim.sample(1.0);
        assert!(st.extras.spinning);
        assert!((st.extras.spinning_progress - 14.0 / 15.0).abs() < 1e-6);

        anim.tick(input.clone());
        assert!(!anim.sample(1.0).extras.spinning);

        for _ in 0..15 {
            anim.tick(input.clone());
        }
        let st = anim.sample(1.0);
        assert!(!st.extras.spinning);
        assert_eq!(st.extras.spinning_progress, 0.0);

        let mut spinning = 0;
        for _ in 0..(55 - 30) {
            anim.tick(input.clone());
        }
        for _ in 0..55 {
            anim.tick(input.clone());
            if anim.sample(1.0).extras.spinning {
                spinning += 1;
            }
        }
        assert_eq!(spinning, 15);
    }

    #[test]
    fn an_allay_raises_its_arms_to_a_held_item_over_five_ticks() {
        let mut anim = EntityAnim::new(1, EntityKind::Allay);
        let mut input = TickInput::new();
        input.shared_mut().main_hand = crate::renderer::anim::HeldItem {
            id: "amethyst_shard",
            count: 1,
            ..crate::renderer::anim::HeldItem::default()
        };
        for _ in 0..5 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(1.0).extras.holding_progress - 1.0).abs() < 1e-6);
        input.shared_mut().main_hand = crate::renderer::anim::HeldItem::default();
        for _ in 0..5 {
            anim.tick(input.clone());
        }
        assert_eq!(anim.sample(1.0).extras.holding_progress, 0.0);
    }

    #[test]
    fn a_camel_dash_arms_a_fifty_five_tick_cooldown() {
        let mut anim = EntityAnim::new(1, EntityKind::Camel);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        assert_eq!(anim.sample(0.0).extras.jump_cooldown, 0.0);
        input.shared_mut().dash = true;
        anim.tick(input);
        assert!((anim.sample(0.0).extras.jump_cooldown - 54.0).abs() < 1e-6);
    }

    #[test]
    fn an_arrow_wobbles_for_seven_ticks_after_it_sticks() {
        let mut anim = EntityAnim::new(1, EntityKind::Arrow);
        let mut input = TickInput::new();
        anim.tick(input.clone());
        assert!(anim.sample(0.0).extras.arrow_shake <= 0.0);
        input.shared_mut().in_ground = true;
        anim.tick(input.clone());
        assert!((anim.sample(0.0).extras.arrow_shake - 6.0).abs() < 1e-6);
        for _ in 0..6 {
            anim.tick(input.clone());
        }
        assert!(anim.sample(0.0).extras.arrow_shake <= 0.0);
    }

    #[test]
    fn evoker_fangs_draw_nothing_until_entity_event_four() {
        let mut anim = EntityAnim::new(1, EntityKind::EvokerFangs);
        for _ in 0..5 {
            anim.tick(TickInput::new());
        }
        assert_eq!(anim.sample(0.0).extras.bite_progress, 0.0);
        anim.attack_animation();
        anim.tick(TickInput::new());
        assert!((anim.sample(0.0).extras.bite_progress - 0.05).abs() < 1e-6);
        for _ in 0..20 {
            anim.tick(TickInput::new());
        }
        assert!((anim.sample(0.0).extras.bite_progress - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_wither_side_head_with_no_target_follows_the_body() {
        let mut anim = EntityAnim::new(1, EntityKind::Wither);
        let mut input = TickInput::new();
        input.y_rot = 90.0;
        anim.tick(input.clone());
        assert!((anim.sample(1.0).extras.head_y_rots[0] - 10.0).abs() < 1e-4);
        for _ in 0..8 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(1.0).extras.head_y_rots[0] - 90.0).abs() < 1e-3);
    }

    #[test]
    fn a_wither_side_head_turns_toward_its_own_target() {
        let mut anim = EntityAnim::new(1, EntityKind::Wither);
        let mut input = TickInput::new();
        input.pos = [0.0, 0.0, 0.0];
        input.head_targets[0] = Some([10.0, 2.2, 0.0]);
        anim.tick(input.clone());
        let st = anim.sample(1.0);
        assert!(
            (st.extras.head_y_rots[0] + 10.0).abs() < 1e-4,
            "{}",
            st.extras.head_y_rots[0]
        );
        assert!(st.extras.head_x_rots[0].abs() < 1e-4);
        for _ in 0..9 {
            anim.tick(input.clone());
        }
        assert!((anim.sample(1.0).extras.head_y_rots[0] + 90.0).abs() < 1e-3);
    }

    #[test]
    fn a_wither_head_above_its_target_pitches_down() {
        let mut anim = EntityAnim::new(1, EntityKind::Wither);
        let mut input = TickInput::new();
        input.head_targets[1] = Some([0.0, 0.0, 0.0]);
        input.pos = [0.0, 10.0, 0.0];
        anim.tick(input);
        assert!(anim.sample(1.0).extras.head_x_rots[1] > 0.0);
    }

    #[test]
    fn death_time_counts_up_and_stops_at_twenty() {
        let mut anim = EntityAnim::new(1, EntityKind::Zombie);
        let mut input = TickInput::new();
        input.dead = true;
        for _ in 0..30 {
            anim.tick(input.clone());
        }
        assert_eq!(anim.sample(0.0).death_time, 20.0);
    }
}
