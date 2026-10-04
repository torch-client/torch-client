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

pub use crate::direction::Direction;

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
    pub text: Vec<crate::text::Span>,
    pub line_width: f32,
    pub background: u32,
    pub opacity: u8,
    pub flags: u8,
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
            text: Vec::new(),
            line_width: 200.0,
            background: 0x4000_0000,
            opacity: 255,
            flags: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Extras {
    pub swell: f32,
    pub peek: f32,
    pub render_offset: [f32; 3],
    pub eating: bool,
    pub jump_progress: f32,
    pub paddle_left: f32,
    pub paddle_right: f32,

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
    pub spikes_animation: f32,
    pub tail_animation: f32,
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
    pub item_frame_direction: Direction,
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
    pub carried_block: Option<String>,
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
    pub helmet: Option<&'static str>,
    pub chestplate: Option<&'static str>,
    pub leggings: Option<&'static str>,
    pub boots: Option<&'static str>,
    pub body_armor: Option<&'static str>,
    pub left_handed: bool,
    pub name: Option<String>,
    pub name_spans: Vec<crate::text::Span>,
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
    pub shows_bottom: bool,
    pub dark_ticks_remaining: i32,
    pub brightness_override: i32,
}

impl ExtrasShared {
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
            jump_progress: 0.0,
            paddle_left: 0.0,
            paddle_right: 0.0,
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
            spikes_animation: 0.0,
            tail_animation: 0.0,
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
            item_frame_direction: Direction::South,
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
            helmet: None,
            chestplate: None,
            leggings: None,
            boots: None,
            body_armor: None,
            left_handed: false,
            name: None,
            name_spans: Vec::new(),
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
            max_crossbow_charge: 25.0,
            aggressive: false,
            charging: false,
            admiring: false,
            crackiness: 0,
            invulnerable_ticks: 0.0,
            can_move: true,
            under_water: false,
            arrow_tipped: false,
            skull_dangerous: false,
            shows_bottom: true,
            dark_ticks_remaining: 0,
            brightness_override: -1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EntityState {
    pub id: i32,
    pub kind: EntityKind,
    pub pos: [f32; 3],
    pub body_rot: f32,
    pub y_rot: f32,
    pub x_rot: f32,
    pub age_ticks: f32,
    pub walk_pos: f32,
    pub walk_speed: f32,
    pub death_time: f32,
    pub scale: f32,
    pub age_scale: f32,
    #[allow(
        dead_code,
        reason = "half of the bounding box; the height is read and the width is carried with it"
    )]
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
}

impl EntityState {
    pub fn has_pose(&self, pose: Pose) -> bool {
        self.pose == pose
    }

    #[allow(dead_code, reason = "used by the per-family registry tests")]
    pub fn new(id: i32, kind: EntityKind) -> EntityState {
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
            eye_height: 1.62,
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
            extras: Extras::default(),
        }
    }
}
