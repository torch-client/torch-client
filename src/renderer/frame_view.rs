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
    pub player_pos: [f64; 3],
    pub player_lerp: [f64; 3],
    pub eye_height: f32,
    pub camera_eye_height: f32,
    pub sleeping: bool,
    pub bed_orientation: Option<Direction>,
    pub camera_distance: f32,
    pub local_body: crate::session::PlayerBody,
    pub nausea: f32,
    pub portal: f32,
    pub day_ticks: f64,
    pub day_rate: f32,
    pub biome: Option<BiomeLayer>,
    pub fov_modifier: f32,
    pub camera_yaw: f32,
    pub camera_pitch: f32,
    pub camera_world: DVec3,
    pub camera_render: Vec3,
    pub camera_rotation: Quat,
    pub in_world: bool,
    pub gamemode: Gamemode,
    pub flying: bool,
    pub hotbar_selected: u8,
    pub item_used: [u32; 2],
    pub item_activation: Option<crate::session::ItemActivation>,
    pub hands_busy: bool,
    pub crosshair_entity: Option<i32>,
    pub main_hand_left: bool,
    pub entities: Arc<Vec<EntityAnim>>,
    pub item_pickups: Vec<crate::play::item_pickup::ItemPickup>,
    pub block_entities:
        Arc<std::collections::HashMap<[i32; 3], crate::blockentities::feed::BlockEntityInfo>>,
    pub block_entities_version: u64,
    pub game_time: u64,
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
        self.nausea = crate::play::mob_effects::effect_blend_factor(
            &session.active_effects,
            "nausea",
            partial,
        );
        self.portal = session.portal_effect.sample(partial);
        self.day_ticks = session.day_clock.at(partial);
        self.day_rate = session.day_clock.rate;
        self.biome = match (session.env_prev, session.env) {
            (Some(from), Some(now)) => Some(BiomeLayer::lerp(partial, &from, &now)),
            (_, now) => now,
        };
        self.fov_modifier = session.fov.sample(partial);
        self.camera_yaw = s.camera_yaw;
        self.camera_pitch = s.camera_pitch;
        self.in_world = s.in_world;
        self.gamemode = session.gamemode;
        self.flying = session.flying;
        self.hotbar_selected = session.hotbar_selected;
        self.item_used = session.item_used;
        self.item_activation.clone_from(&session.item_activation);
        self.hands_busy = session.hands_busy;
        self.crosshair_entity = session.crosshair_entity;
        self.main_hand_left = s.skin_prefs.main_hand_left;
        self.entities = session.entities.clone();
        self.item_pickups.clone_from(&session.item_pickups);
        self.game_time = session.game_time;
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
