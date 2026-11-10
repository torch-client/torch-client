use std::sync::Arc;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::blocks::direction::Direction;
use crate::entities::feed::EntityAnim;
use crate::renderer::anim::AnimSample;
use crate::renderer::environment::BiomeLayer;
use crate::session::{BreakingBlock, Gamemode, SharedState, TargetedBlock};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrameViewSystems;

#[derive(Resource, Default)]
pub struct FrameView {
    pub partial: f32,
    pub no_render: crate::modules::no_render::Hide,
    pub player_pos: [f64; 3],
    pub descending: bool,
    pub player_lerp: [f64; 3],
    pub eye_height: f32,
    pub camera_eye_height: f32,
    pub sleeping: bool,
    pub bed_orientation: Option<Direction>,
    pub camera_distance: f32,
    pub local_body: crate::session::PlayerBody,
    pub nausea: f32,
    pub night_vision: Option<f32>,
    pub conduit_power: bool,
    pub fire_resistance: bool,
    pub darkness: f32,
    pub has_darkness: bool,
    pub blindness: Option<i32>,
    pub boss_darken: bool,
    pub boss_fog: bool,
    pub sky_flash: bool,
    pub head_camera_overlay: bool,
    #[cfg(feature = "audio")]
    pub boss_music: bool,
    #[cfg(feature = "audio")]
    pub reconfiguring: bool,
    pub portal: f32,
    pub spin_degrees: f32,
    pub day_ticks: f64,
    pub day_rate: f32,
    pub end_ticks: f64,
    pub biome: Option<BiomeLayer>,
    pub fov_modifier: f32,
    pub camera_yaw: f32,
    pub camera_pitch: f32,
    pub camera_world: DVec3,
    pub camera_render: Vec3,
    pub camera_rotation: Quat,
    pub hurt_tilt: Quat,
    pub in_world: bool,
    pub gamemode: Gamemode,
    pub health_epoch: u32,
    pub flying: bool,
    pub hotbar_selected: u8,
    pub item_used: [u32; 2],
    pub item_activation: Option<crate::session::ItemActivation>,
    pub hands_busy: bool,
    pub item_swap: f32,
    pub crosshair_entity: Option<i32>,
    pub main_hand_left: bool,
    pub entities: Arc<Vec<EntityAnim>>,
    pub item_pickups: Vec<crate::play::item_pickup::ItemPickup>,
    pub block_entities: Arc<crate::blockentities::feed::BlockEntityMap>,
    pub block_entities_version: u64,
    pub game_time: u64,
    pub compass_spawn: Option<[i32; 3]>,
    pub compass_death: Option<[i32; 3]>,
    pub border: azalea::world::WorldBorder,
    pub op_level: u8,
    pub local: AnimSample,
    pub breaking: Vec<BreakingBlock>,
    pub targeted_block: Option<TargetedBlock>,
    #[cfg(feature = "skins")]
    pub local_skin: crate::client::skins::SkinState,
    #[cfg(feature = "click_gui")]
    pub esp: Arc<crate::modules::esp::EspFrame>,
}

impl FrameView {
    pub(crate) fn fill(&mut self, s: &SharedState, in_game: bool) {
        let session = &s.session;
        let partial = super::systems::partial_ticks(s);
        self.partial = partial;
        let (prev, cur) = (session.player_pos_prev, session.player_pos);
        self.player_pos = cur;
        self.descending = s.move_flags & crate::session::MOVE_SNEAK != 0;
        self.player_lerp =
            std::array::from_fn(|i| prev[i] + (cur[i] - prev[i]) * f64::from(partial));
        self.eye_height = super::systems::session_eye_height(session);
        self.camera_eye_height = session.camera_eye.sample(partial);
        self.sleeping = session.sleeping;
        self.bed_orientation = session.bed_orientation;
        self.camera_distance = session
            .camera_distance
            .unwrap_or(crate::client::tracking::DEFAULT_CAMERA_DISTANCE as f32);
        self.local_body = session.local_body;
        {
            use crate::play::mob_effects::{effect_blend_factor, find, has, night_vision_scale};
            let effects = &session.active_effects;
            self.nausea = effect_blend_factor(effects, "nausea", partial);
            self.night_vision =
                find(effects, "night_vision").map(|e| night_vision_scale(e.duration, partial));
            self.conduit_power = has(effects, "conduit_power");
            self.fire_resistance = has(effects, "fire_resistance");
            self.has_darkness = has(effects, "darkness");
            self.darkness = effect_blend_factor(effects, "darkness", partial);
            self.blindness = find(effects, "blindness").map(|e| e.duration);
        }
        self.boss_darken = session.boss_bars.iter().any(|b| b.darken);
        self.boss_fog = session.boss_bars.iter().any(|b| b.fog);
        self.sky_flash = session.sky_flash_time > 0;
        self.head_camera_overlay = session.head.id == "carved_pumpkin";
        #[cfg(feature = "audio")]
        {
            self.boss_music = session.boss_bars.iter().any(|b| b.music);
            self.reconfiguring = session.reconfiguring;
        }
        self.portal = session.portal_effect.sample(partial);
        self.spin_degrees = session.spinning_effect.sample(partial);
        self.hide_effects();
        self.day_ticks = session.day_clock.at(partial);
        self.day_rate = session.day_clock.rate;
        self.end_ticks = session.end_clock.ticks() as f64 + f64::from(partial);
        self.biome = match (session.env_prev, session.env) {
            (Some(from), Some(now)) => Some(BiomeLayer::lerp(partial, &from, &now)),
            (_, now) => now,
        };
        self.fov_modifier = session.fov.sample(partial);
        self.camera_yaw = s.camera_yaw;
        self.camera_pitch = s.camera_pitch;
        self.in_world = s.in_world;
        self.gamemode = session.gamemode;
        self.health_epoch = session.health_epoch;
        self.flying = session.flying;
        self.hotbar_selected = session.hotbar_selected;
        self.item_used = session.item_used;
        self.item_activation.clone_from(&session.item_activation);
        self.hands_busy = session.hands_busy;
        self.item_swap = session.item_swap;
        self.crosshair_entity = session.crosshair_entity;
        self.main_hand_left = s.skin_prefs.main_hand_left;
        self.entities = session.entities.clone();
        self.item_pickups.clone_from(&session.item_pickups);
        self.game_time = session.game_time;
        let here = |at: &Option<azalea_core::position::GlobalPos>| {
            at.as_ref()
                .filter(|at| crate::renderer::dimension::is_current_level(&at.dimension))
                .map(|at| [at.pos.x, at.pos.y, at.pos.z])
        };
        self.compass_spawn = here(&session.spawn_pos);
        self.compass_death = here(&session.last_death);
        self.border = session.border;
        self.op_level = session.op_level;
        #[cfg(feature = "skins")]
        {
            self.local_skin = session.local_skin.clone();
        }
        #[cfg(feature = "click_gui")]
        {
            self.esp = session.esp.clone();
        }
        if in_game {
            if self.block_entities_version != session.block_entities_version {
                self.block_entities_version = session.block_entities_version;
                self.block_entities = session.block_entities.clone();
            }
            self.local = session.local_anim.sample(partial);
            if self.breaking != session.breaking {
                self.breaking = session.breaking.clone();
            }
            if self.targeted_block != session.targeted_block {
                self.targeted_block = session.targeted_block.clone();
            }
        }
    }

    fn hide_effects(&mut self) {
        use crate::modules::no_render::{Part, config};
        let hide = config();
        self.no_render = hide;
        if hide == crate::modules::no_render::Hide::NONE {
            return;
        }
        if hide.has(Part::Portal) {
            self.portal = 0.0;
            self.nausea = 0.0;
        }
        if hide.has(Part::Blindness) {
            self.blindness = None;
            self.darkness = 0.0;
            self.has_darkness = false;
        }
        if hide.has(Part::BossSky) {
            self.boss_darken = false;
            self.boss_fog = false;
        }
        if hide.has(Part::Lightning) {
            self.sky_flash = false;
        }
        if hide.has(Part::Pumpkin) {
            self.head_camera_overlay = false;
        }
    }

    pub fn tick_eye(&self) -> [f64; 3] {
        [
            self.player_pos[0],
            self.player_pos[1] + f64::from(self.eye_height),
            self.player_pos[2],
        ]
    }

    pub fn lerp_eye(&self) -> [f64; 3] {
        [
            self.player_lerp[0],
            self.player_lerp[1] + f64::from(self.eye_height),
            self.player_lerp[2],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_tick_yet_reads_the_previous_position() {
        let mut s = SharedState::default();
        s.session.player_pos_prev = [1.0, 2.0, 3.0];
        s.session.player_pos = [5.0, 6.0, 7.0];
        let mut view = FrameView::default();
        view.fill(&s, true);
        assert_eq!(view.partial, 0.0);
        assert_eq!(view.player_lerp, [1.0, 2.0, 3.0]);
        assert_eq!(view.player_pos, [5.0, 6.0, 7.0]);
        assert_eq!(view.day_ticks, s.session.day_clock.ticks);
        let eye = view.tick_eye();
        assert_eq!(eye[1], 6.0 + view.eye_height as f64);
    }

    #[test]
    fn a_stale_tick_reads_the_current_position() {
        let Some(then) = crate::platform::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(200))
        else {
            return;
        };
        let mut s = SharedState::default();
        s.session.player_pos_prev = [0.0, 0.0, 0.0];
        s.session.player_pos = [4.0, 8.0, 12.0];
        s.session.last_tick_time = Some(then);
        let mut view = FrameView::default();
        view.fill(&s, true);
        assert_eq!(view.partial, 1.0);
        assert_eq!(view.player_lerp, [4.0, 8.0, 12.0]);
    }

    #[test]
    fn a_menu_frame_leaves_the_world_only_fields() {
        let s = SharedState::default();
        let mut view = FrameView {
            hotbar_selected: 3,
            ..FrameView::default()
        };
        view.local.walk_pos = 9.0;
        view.fill(&s, false);
        assert_eq!(view.local.walk_pos, 9.0);
        assert_eq!(view.hotbar_selected, 0);
    }
}
