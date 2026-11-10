use azalea_registry::builtin::EntityKind;

use crate::renderer::anim::HeldItem;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Pose {
    #[default]
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Crouching,
    LongJumping,
    Dying,
    Croaking,
    UsingTongue,
    Sitting,
    Roaring,
    Sniffing,
    Emerging,
    Digging,
    Sliding,
    Shooting,
    Inhaling,
}

pub use crate::blocks::direction::Direction;
pub use crate::entities::keyframe::AnimationState;

pub mod anim_slot {
    pub const COUNT: usize = 7;

    pub mod warden {
        pub const ROAR: usize = 0;
        pub const SNIFF: usize = 1;
        pub const EMERGE: usize = 2;
        pub const DIGGING: usize = 3;
        pub const ATTACK: usize = 4;
        pub const SONIC_BOOM: usize = 5;
    }

    pub mod breeze {
        pub const IDLE: usize = 0;
        pub const SLIDE: usize = 1;
        pub const SLIDE_BACK: usize = 2;
        pub const LONG_JUMP: usize = 3;
        pub const SHOOT: usize = 4;
        pub const INHALE: usize = 5;
    }

    pub mod sniffer {
        pub const FEELING_HAPPY: usize = 0;
        pub const SCENTING: usize = 1;
        pub const SNIFFING: usize = 2;
        pub const DIGGING: usize = 3;
        pub const RISING: usize = 4;
    }

    pub mod rabbit {
        pub const HOP: usize = 0;
        pub const IDLE_HEAD_TILT: usize = 1;
    }

    pub mod creaking {
        pub const ATTACK: usize = 0;
        pub const INVULNERABILITY: usize = 1;
        pub const DEATH: usize = 2;
    }

    pub mod armadillo {
        pub const ROLL_OUT: usize = 0;
        pub const ROLL_UP: usize = 1;
        pub const PEEK: usize = 2;
    }

    pub mod copper_golem {
        pub const IDLE: usize = 0;
        pub const INTERACTION_GET_ITEM: usize = 1;
        pub const INTERACTION_GET_NO_ITEM: usize = 2;
        pub const INTERACTION_DROP_ITEM: usize = 3;
        pub const INTERACTION_DROP_NO_ITEM: usize = 4;
    }

    pub mod camel {
        pub const SIT: usize = 0;
        pub const SIT_POSE: usize = 1;
        pub const SIT_UP: usize = 2;
        pub const IDLE: usize = 3;
        pub const DASH: usize = 4;
    }

    pub mod axolotl {
        pub const SWIM: usize = 0;
        pub const WALK: usize = 1;
        pub const WALK_UNDER_WATER: usize = 2;
        pub const IDLE_UNDER_WATER: usize = 3;
        pub const IDLE_UNDER_WATER_ON_GROUND: usize = 4;
        pub const IDLE_ON_GROUND: usize = 5;
        pub const PLAY_DEAD: usize = 6;
    }

    pub mod frog {
        pub const JUMP: usize = 0;
        pub const CROAK: usize = 1;
        pub const TONGUE: usize = 2;
        pub const SWIM_IDLE: usize = 3;
    }

    pub mod bat {
        pub const FLY: usize = 0;
        pub const REST: usize = 1;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Billboard {
    #[default]
    Fixed,
    Vertical,
    Horizontal,
    Center,
}

impl Billboard {
    pub fn from_id(id: u8) -> Billboard {
        match id {
            1 => Billboard::Vertical,
            2 => Billboard::Horizontal,
            3 => Billboard::Center,
            _ => Billboard::Fixed,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Display {
    pub billboard: Billboard,
    pub translation: [f32; 3],
    pub scale: [f32; 3],
    pub left_rotation: [f32; 4],
    pub right_rotation: [f32; 4],
    pub view_range: f32,
    pub width: f32,
    pub height: f32,
    pub text: Vec<crate::text::Span>,
    pub line_width: f32,
    pub background: u32,
    pub opacity: u8,
    pub flags: u8,
    pub item_context: &'static str,
}

impl Display {
    pub fn uniform_scale(&self) -> bool {
        self.scale[0] == self.scale[1] && self.scale[1] == self.scale[2]
    }

    pub fn no_right_rotation(&self) -> bool {
        self.right_rotation == [0.0, 0.0, 0.0, 1.0]
    }

    pub const FLAG_SHADOW: u8 = 1;
    pub const FLAG_SEE_THROUGH: u8 = 2;
    pub const FLAG_DEFAULT_BACKGROUND: u8 = 4;
    pub const FLAG_ALIGN_LEFT: u8 = 8;
    pub const FLAG_ALIGN_RIGHT: u8 = 16;
}

impl Default for Display {
    fn default() -> Display {
        Display {
            billboard: Billboard::Fixed,
            translation: [0.0; 3],
            scale: [1.0; 3],
            left_rotation: [0.0, 0.0, 0.0, 1.0],
            right_rotation: [0.0, 0.0, 0.0, 1.0],
            view_range: 1.0,
            width: 0.0,
            height: 0.0,
            text: Vec::new(),
            line_width: 200.0,
            background: 0x4000_0000,
            opacity: 255,
            flags: 0,
            item_context: "none",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Extras {
    pub swell: f32,
    pub peek: f32,
    pub render_offset: [f32; 3],
    pub ticks_since_kinetic_hit: f32,
    pub eating: bool,
    pub jump_progress: f32,
    pub paddle_left: f32,
    pub paddle_right: f32,
    pub bubble_angle: f32,
    pub vehicle_hurt: f32,
    pub vehicle_damage: f32,

    pub head_eat_position_scale: f32,
    pub head_eat_angle_scale: f32,
    pub flap: f32,
    pub flap_speed: f32,
    pub head_roll_angle: f32,
    pub crouch_amount: f32,
    pub shake_anim: f32,
    pub wet_shade: f32,
    pub lie_down_amount: f32,
    pub lie_down_amount_tail: f32,
    pub relax_state_one_amount: f32,
    pub sit_amount: f32,
    pub lie_on_back_amount: f32,
    pub roll_amount: f32,
    pub roll_time: f32,
    pub stand_scale: f32,
    pub eat_animation: f32,
    pub stand_animation: f32,
    pub feeding_animation: f32,
    pub jump_cooldown: f32,
    pub ramming_x_head_rot: f32,
    pub attack_animation_remaining_ticks: i32,
    pub jump_completion: f32,
    pub hiding_in_shell: bool,
    pub anim: [AnimationState; anim_slot::COUNT],
    pub tentacle_angle: f32,
    pub squid_x_body_rot: f32,
    pub squid_z_body_rot: f32,
    pub playing_dead_factor: f32,
    pub in_water_factor: f32,
    pub on_ground_factor: f32,
    pub moving_factor: f32,
    pub bee_roll: f32,
    pub on_ground: bool,
    pub flap_angle: f32,
    pub parrot_pose: u8,
    pub spinning: bool,
    pub spinning_progress: f32,
    pub holding_progress: f32,
    pub swim_amount: f32,
    pub elytra: [f32; 3],
    pub ticks_using_item: f32,
    pub wiggle: f32,
    pub squish: f32,
    pub fuse_remaining: f32,
    pub minecart_fuse: f32,
    pub spikes_animation: f32,
    pub tail_animation: f32,
    pub guardian_attack_time: f32,
    pub attack_target_body: Option<TargetBody>,
    pub look_at: Option<[f64; 3]>,
    pub attack_ticks: f32,
    pub stunned_ticks: f32,
    pub roar_animation: f32,
    pub offer_flower_ticks: i32,
    pub flap_time: f32,
    pub tendril_animation: f32,
    pub heart_animation: f32,
    pub arrow_shake: f32,
    pub bite_progress: f32,
    pub head_y_rots: [f32; 2],
    pub head_x_rots: [f32; 2],
    pub dragon: Option<Box<DragonPose>>,
    shared: std::sync::Arc<ExtrasShared>,
}

#[derive(Clone, Debug)]
pub struct ExtrasShared {
    pub is_baby: bool,
    pub powered: bool,
    pub size: i32,
    pub sheared: bool,
    pub wool_color: u8,
    pub tame: bool,
    pub sitting: bool,
    pub collar_color: u8,
    pub variant: Option<String>,
    pub variant_id: i32,
    pub attach_face: Direction,
    pub color: u8,
    pub item: HeldItem,
    pub item_rotation: i32,
    pub hanging_direction: Direction,
    pub xp_value: i32,
    pub hook_owner: Option<i32>,
    pub villager_kind: Option<&'static str>,
    pub villager_profession: Option<&'static str>,
    pub villager_level: u32,
    pub interested: bool,
    pub anger_ticks: i32,
    pub crouching: bool,
    pub sleeping: bool,
    pub pouncing: bool,
    pub faceplanted: bool,
    pub on_back: bool,
    pub panda_rolling: bool,
    pub unhappy_counter: i32,
    pub left_horn: bool,
    pub right_horn: bool,
    pub standing: bool,
    pub anger_level: i32,
    pub has_pumpkin: bool,
    pub charged: bool,
    pub moving: bool,
    pub attack_target: i32,
    pub resting: bool,
    pub dancing: bool,
    pub saddled: bool,
    pub has_chest: bool,
    pub rearing: bool,
    pub dash: bool,
    pub sniffer_state: u8,
    pub armadillo_state: u8,
    pub copper_golem_state: u8,
    pub weather_state: u8,
    pub has_egg: bool,
    pub laying_egg: bool,
    pub tearing_down: bool,
    pub active: bool,
    pub puff_state: i32,
    pub carried_block: Option<u32>,
    pub creepy: bool,
    pub spell: u8,
    pub charging_crossbow: bool,
    pub celebrating: bool,
    pub dragon_phase: i32,
    pub bubble_time: f32,
    pub hurt_time: f32,
    pub hurt_dir: f32,
    pub damage: f32,
    pub display_block: Option<u32>,
    pub display_offset: i32,
    pub display: Option<Box<Display>>,
    pub block_state: Option<u32>,
    pub fuse: i32,
    pub in_ground: bool,
    pub small: bool,
    pub show_arms: bool,
    pub show_base_plate: bool,
    pub stand_pose: [[f32; 3]; 6],
    pub main_hand: HeldItem,
    pub off_hand: HeldItem,
    pub head: HeldItem,
    pub chestplate: Option<&'static str>,
    pub leggings: Option<&'static str>,
    pub boots: Option<&'static str>,
    pub armor_foil: [bool; 4],
    pub armor_trim: [Option<&'static str>; 4],
    pub body_armor: Option<&'static str>,
    pub left_handed: bool,
    pub name: Option<String>,
    pub name_spans: std::sync::Arc<[crate::text::Span]>,
    pub name_visible: bool,
    pub tail_angle: f32,
    pub sprinting: bool,
    pub lying_on_sleeping_player: bool,
    pub sneeze_time: i32,
    pub sneezing: bool,
    pub scared: bool,
    pub ridden: bool,
    pub on_land: bool,
    pub suffocating: bool,
    pub has_stinger: bool,
    pub has_nectar: bool,
    pub bee_rolling: bool,
    pub fall_flying: bool,
    pub passenger: bool,
    pub speed_value: f32,
    pub using_item: bool,
    pub use_offhand: bool,
    pub max_crossbow_charge: f32,
    pub aggressive: bool,
    pub charging: bool,
    pub admiring: bool,
    pub crackiness: u8,
    pub invulnerable_ticks: f32,
    pub can_move: bool,
    pub under_water: bool,
    pub arrow_tipped: bool,
    pub skull_dangerous: bool,
    pub shot_at_angle: bool,
    pub trident_foil: bool,
    pub shows_bottom: bool,
    pub beam_target: Option<[i32; 3]>,
    pub dark_ticks_remaining: i32,
    pub brightness_override: i32,
}

impl ExtrasShared {
    pub fn helmet(&self) -> Option<&'static str> {
        (!self.head.is_empty()).then_some(self.head.id)
    }

    pub fn bee_angry(&self) -> bool {
        self.anger_ticks > 0
    }

    pub fn magic_name_toast(&self) -> bool {
        self.name.as_deref() == Some("Toast")
    }

    pub fn magic_name_jeb(&self) -> bool {
        self.name.as_deref() == Some("jeb_")
    }
}

impl std::ops::Deref for Extras {
    type Target = ExtrasShared;

    fn deref(&self) -> &ExtrasShared {
        &self.shared
    }
}

impl Extras {
    #[cfg(test)]
    pub fn shared_mut(&mut self) -> &mut ExtrasShared {
        std::sync::Arc::make_mut(&mut self.shared)
    }

    pub fn with_shared(shared: std::sync::Arc<ExtrasShared>) -> Extras {
        Extras {
            swell: 0.0,
            peek: 0.0,
            render_offset: [0.0; 3],
            eating: false,
            ticks_since_kinetic_hit: 0.0,
            jump_progress: 0.0,
            paddle_left: 0.0,
            paddle_right: 0.0,
            bubble_angle: 0.0,
            vehicle_hurt: 0.0,
            vehicle_damage: 0.0,
            head_eat_position_scale: 0.0,
            head_eat_angle_scale: 0.0,
            flap: 0.0,
            flap_speed: 1.0,
            head_roll_angle: 0.0,
            crouch_amount: 0.0,
            shake_anim: 0.0,
            wet_shade: 1.0,
            lie_down_amount: 0.0,
            lie_down_amount_tail: 0.0,
            relax_state_one_amount: 0.0,
            sit_amount: 0.0,
            lie_on_back_amount: 0.0,
            roll_amount: 0.0,
            roll_time: 0.0,
            stand_scale: 0.0,
            eat_animation: 0.0,
            stand_animation: 0.0,
            feeding_animation: 0.0,
            jump_cooldown: 0.0,
            ramming_x_head_rot: 0.0,
            attack_animation_remaining_ticks: 0,
            jump_completion: 0.0,
            hiding_in_shell: false,
            anim: [AnimationState::new(); anim_slot::COUNT],
            tentacle_angle: 0.0,
            squid_x_body_rot: 0.0,
            squid_z_body_rot: 0.0,
            playing_dead_factor: 0.0,
            in_water_factor: 1.0,
            on_ground_factor: 0.0,
            moving_factor: 0.0,
            bee_roll: 0.0,
            on_ground: false,
            flap_angle: 0.0,
            parrot_pose: 1,
            spinning: false,
            spinning_progress: 0.0,
            holding_progress: 0.0,
            swim_amount: 0.0,
            elytra: [0.0; 3],
            ticks_using_item: 0.0,
            wiggle: 5.0,
            squish: 0.0,
            fuse_remaining: 0.0,
            minecart_fuse: -1.0,
            spikes_animation: 0.0,
            tail_animation: 0.0,
            guardian_attack_time: 0.0,
            attack_target_body: None,
            look_at: None,
            attack_ticks: 0.0,
            stunned_ticks: 0.0,
            roar_animation: 0.0,
            offer_flower_ticks: 0,
            flap_time: 0.0,
            tendril_animation: 0.0,
            heart_animation: 0.0,
            arrow_shake: 0.0,
            bite_progress: 0.0,
            head_y_rots: [0.0; 2],
            head_x_rots: [0.0; 2],
            dragon: None,
            shared,
        }
    }
}

impl Default for Extras {
    fn default() -> Self {
        static SHARED: std::sync::LazyLock<std::sync::Arc<ExtrasShared>> =
            std::sync::LazyLock::new(|| std::sync::Arc::new(ExtrasShared::default()));
        Extras::with_shared(SHARED.clone())
    }
}

impl Default for ExtrasShared {
    fn default() -> Self {
        ExtrasShared {
            is_baby: false,
            powered: false,
            size: 1,
            sheared: false,
            wool_color: 0,
            tame: false,
            sitting: false,
            collar_color: 14,
            variant: None,
            variant_id: 0,
            attach_face: Direction::Down,
            color: 16,
            item: HeldItem::default(),
            item_rotation: 0,
            hanging_direction: Direction::South,
            xp_value: 0,
            hook_owner: None,
            villager_kind: None,
            villager_profession: None,
            villager_level: 1,
            interested: false,
            anger_ticks: 0,
            crouching: false,
            sleeping: false,
            pouncing: false,
            faceplanted: false,
            on_back: false,
            panda_rolling: false,
            unhappy_counter: 0,
            left_horn: true,
            right_horn: true,
            standing: false,
            anger_level: 0,
            has_pumpkin: true,
            charged: false,
            moving: false,
            attack_target: 0,
            resting: false,
            dancing: false,
            saddled: false,
            has_chest: false,
            rearing: false,
            dash: false,
            sniffer_state: 0,
            armadillo_state: 0,
            copper_golem_state: 0,
            weather_state: 0,
            has_egg: false,
            laying_egg: false,
            tearing_down: false,
            active: false,
            puff_state: 0,
            carried_block: None,
            creepy: false,
            spell: 0,
            charging_crossbow: false,
            celebrating: false,
            dragon_phase: 0,
            bubble_time: 0.0,
            hurt_time: 0.0,
            hurt_dir: 1.0,
            damage: 0.0,
            display_block: None,
            display_offset: 0,
            display: None,
            block_state: None,
            fuse: 0,
            in_ground: false,
            small: false,
            show_arms: false,
            show_base_plate: true,
            stand_pose: [[0.0; 3]; 6],
            main_hand: HeldItem::default(),
            off_hand: HeldItem::default(),
            head: HeldItem::default(),
            chestplate: None,
            leggings: None,
            boots: None,
            armor_foil: [false; 4],
            armor_trim: [None; 4],
            body_armor: None,
            left_handed: false,
            name: None,
            name_spans: std::sync::Arc::default(),
            name_visible: false,
            tail_angle: 0.628_318_55,
            sprinting: false,
            lying_on_sleeping_player: false,
            sneeze_time: 0,
            sneezing: false,
            scared: false,
            ridden: false,
            on_land: true,
            suffocating: false,
            has_stinger: true,
            has_nectar: false,
            bee_rolling: false,
            fall_flying: false,
            passenger: false,
            speed_value: 1.0,
            using_item: false,
            use_offhand: false,
            max_crossbow_charge: crate::items::kinetic::CROSSBOW_CHARGE_TICKS,
            aggressive: false,
            charging: false,
            admiring: false,
            crackiness: 0,
            invulnerable_ticks: 0.0,
            can_move: true,
            under_water: false,
            arrow_tipped: false,
            skull_dangerous: false,
            shot_at_angle: false,
            trident_foil: false,
            shows_bottom: true,
            beam_target: None,
            dark_ticks_remaining: 0,
            brightness_override: -1,
        }
    }
}

pub const DRAGON_DELAYS: usize = 24;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DragonSample {
    pub y: f64,
    pub y_rot: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragonHeadMode {
    LandingOrTakingOff { distance_to_egg: f64 },
    Sitting,
    Flying,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DragonPose {
    pub history: [DragonSample; DRAGON_DELAYS],
    pub head_mode: DragonHeadMode,
    pub nearest_crystal: Option<i32>,
}

impl DragonPose {
    pub const REST: DragonPose = DragonPose {
        history: [DragonSample { y: 0.0, y_rot: 0.0 }; DRAGON_DELAYS],
        head_mode: DragonHeadMode::Flying,
        nearest_crystal: None,
    };

    pub fn historical(&self, delay: usize) -> DragonSample {
        self.history[delay]
    }

    pub fn head_part_y_offset(
        &self,
        part: usize,
        body: DragonSample,
        part_pos: DragonSample,
    ) -> f32 {
        let result = match self.head_mode {
            DragonHeadMode::LandingOrTakingOff { distance_to_egg } => {
                part as f64 / (distance_to_egg / 4.0).max(1.0)
            }
            DragonHeadMode::Sitting => part as f64,
            DragonHeadMode::Flying if part == 6 => 0.0,
            DragonHeadMode::Flying => part_pos.y - body.y,
        };
        result as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetBody {
    pub feet: [f64; 3],
    pub height: f32,
    pub eye_height: f32,
}

impl TargetBody {
    pub fn eye(&self) -> [f64; 3] {
        [
            self.feet[0],
            self.feet[1] + f64::from(self.eye_height),
            self.feet[2],
        ]
    }

    pub fn middle(&self) -> [f64; 3] {
        [
            self.feet[0],
            self.feet[1] + f64::from(self.height) * 0.5,
            self.feet[2],
        ]
    }
}

#[derive(Clone, Debug)]
pub struct EntityState {
    pub id: i32,
    pub kind: EntityKind,
    pub pos: [f64; 3],
    pub body_rot: f32,
    pub y_rot: f32,
    pub x_rot: f32,
    pub age_ticks: f32,
    pub walk_pos: f32,
    pub walk_speed: f32,
    pub death_time: f32,
    pub scale: f32,
    pub age_scale: f32,
    pub bounding_box_width: f32,
    pub bounding_box_height: f32,
    pub eye_height: f32,
    pub is_invisible: bool,
    pub is_in_water: bool,
    pub is_fully_frozen: bool,
    pub is_upside_down: bool,
    pub is_auto_spin_attack: bool,
    pub has_red_overlay: bool,
    pub display_fire: bool,
    pub pose: Pose,
    pub bed_orientation: Option<Direction>,
    pub attack_time: f32,
    pub swing_left: bool,
    pub extras: Extras,
    pub carrier: Option<Carrier>,
    pub frozen_light: Option<(u8, u8)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Carrier {
    pub pose: bevy::prelude::Transform,
    pub specs: Option<Carried>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Carried {
    FixedItem,
    ShelfItemBottom,
    ShelfItemCentered,
    GroundItemCluster,
    MovingBlock,
}

impl Carried {
    pub const ALL: [Carried; 5] = [
        Carried::FixedItem,
        Carried::ShelfItemBottom,
        Carried::ShelfItemCentered,
        Carried::GroundItemCluster,
        Carried::MovingBlock,
    ];
}

impl EntityState {
    pub fn has_pose(&self, pose: Pose) -> bool {
        self.pose == pose
    }

    pub fn new(id: i32, kind: EntityKind) -> EntityState {
        EntityState::with_extras(id, kind, Extras::default())
    }

    pub fn with_extras(id: i32, kind: EntityKind, extras: Extras) -> EntityState {
        EntityState {
            id,
            kind,
            pos: [0.0; 3],
            body_rot: 0.0,
            y_rot: 0.0,
            x_rot: 0.0,
            age_ticks: 0.0,
            walk_pos: 0.0,
            walk_speed: 0.0,
            death_time: 0.0,
            scale: 1.0,
            age_scale: 1.0,
            bounding_box_width: 0.6,
            bounding_box_height: 1.8,
            eye_height: crate::session::PlayerBody::STANDING.eye_height,
            is_invisible: false,
            is_in_water: false,
            is_fully_frozen: false,
            is_upside_down: false,
            is_auto_spin_attack: false,
            has_red_overlay: false,
            display_fire: false,
            pose: Pose::Standing,
            bed_orientation: None,
            attack_time: 0.0,
            swing_left: false,
            extras,
            carrier: None,
            frozen_light: None,
        }
    }
}
