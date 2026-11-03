use super::*;
use crate::renderer::{HumanoidAnim, PendingSection};

#[derive(Default)]
pub struct SessionState {
    pub pending_chunks: std::collections::VecDeque<PendingSection>,
    pub pending_edits: std::collections::VecDeque<PendingSection>,
    pub unloaded_chunks: Vec<(i32, i32)>,
    pub clear_chunks: bool,
    pub player_pos: [f64; 3],
    pub status: Option<std::sync::Arc<str>>,
    pub current_address: Option<String>,
    pub advanced_tooltips: bool,
    pub fly_toggle: bool,
    pub flying: bool,
    pub auto_jump: bool,
    pub fov: FovState,
    pub camera_eye: CameraEye,
    pub portal_effect: PortalEffect,
    pub other_players: Vec<OtherPlayerInfo>,
    pub menu_slots: std::sync::Arc<[SlotStack]>,
    pub hotbar: std::sync::Arc<[SlotStack]>,
    pub armor: crate::renderer::Armor,
    pub armor_stacks: std::sync::Arc<[SlotStack]>,
    pub head: crate::renderer::HeldItem,
    pub carried: SlotStack,
    pub container_id: i32,
    pub container_kind: ContainerKind,
    pub container_title: std::sync::Arc<[crate::text::Span]>,
    pub container_data: ContainerData,
    pub enchant_clues: [Option<Box<str>>; 3],
    pub merchant: Option<std::sync::Arc<MerchantOffers>>,
    pub merchant_hint: i32,
    pub merchant_future_xp: u32,
    pub stonecutter_recipes: std::sync::Arc<[StonecutterRecipe]>,
    pub recipes: crate::client::recipes::RecipeBookState,
    pub horse: Option<HorseMenu>,
    pub horse_published_state_id: u32,
    pub inv_actions: Vec<InvAction>,
    pub hotbar_selected: u8,
    pub health: f32,
    pub health_display: HealthDisplay,
    pub health_epoch: u32,
    pub dead: bool,
    pub death_cause: Option<Vec<crate::text::Span>>,
    pub spawn_pos: Option<azalea_core::position::GlobalPos>,
    pub last_death: Option<azalea_core::position::GlobalPos>,
    pub death_score: i32,
    pub show_death_screen: bool,
    pub elder_guardian_effects: u32,
    pub jump_charge: Option<f32>,
    pub jump_cooldown: bool,
    pub hands_busy: bool,
    pub vehicle_look: Option<VehicleLook>,
    pub food: u32,
    pub saturation: f32,
    pub air_supply: i32,
    pub active_effects: Vec<crate::play::mob_effects::MobEffectInstance>,
    pub gamemode: Gamemode,
    pub previous_gamemode: Option<Gamemode>,
    pub gamemode_request: Option<Gamemode>,
    pub crouching: bool,
    pub fall_flying: bool,
    pub swimming: bool,
    pub sleeping: bool,
    pub bed_orientation: Option<crate::blocks::direction::Direction>,
    pub camera_distance: Option<f32>,
    pub local_body: PlayerBody,
    pub sleep_timer: u32,
    pub player_pos_prev: [f64; 3],
    pub game_time: u64,
    pub border: azalea::world::WorldBorder,
    pub last_tick_time: Option<crate::platform::time::Instant>,
    pub hotbar_target: Option<u8>,
    pub spectator_teleport_target: Option<u128>,
    pub fly_speed_scroll: f32,
    pub pending_drops: Vec<DropRequest>,
    pub command_block_open: Option<CommandBlockOpen>,
    pub command_block_pos: Option<azalea_core::position::BlockPos>,
    pub command_block_data: Option<CommandBlockData>,
    pub command_block_update: Option<CommandBlockUpdate>,
    pub op_level: u8,
    pub sign_editor_open: Option<SignEditRequest>,
    pub sign_update: Option<SignUpdateRequest>,
    pub sign_edit_open_pos: Option<azalea_core::position::BlockPos>,
    pub book_open: Option<InteractionHand>,
    pub book_edit_open: Option<BookEditRequest>,
    pub edit_book: Option<EditBookRequest>,
    pub advancements: std::sync::Arc<crate::client::advancements::AdvancementTree>,
    pub advancement_ids: std::collections::HashMap<azalea_registry::identifier::Identifier, u32>,
    pub advancement_tab: Option<u32>,
    pub seen_advancements: Vec<Option<azalea_registry::identifier::Identifier>>,
    pub stats: std::sync::Arc<crate::client::stats::StatsCounter>,
    pub request_stats: bool,
    pub dialog_show: Option<std::sync::Arc<crate::play::dialog::Dialog>>,
    pub dialog_clear: bool,
    pub win_game: bool,
    pub code_of_conduct: Option<String>,
    pub reconfiguring: bool,
    pub dialog_submits: Vec<crate::play::dialog::Submit>,
    pub server_links: Vec<ServerLink>,
    pub server_packs: crate::client::server_packs::ServerPacks,
    pub targeted_block: Option<TargetedBlock>,
    pub targeted_entity: bool,
    pub crosshair_entity: Option<i32>,
    pub aim_target: Option<i32>,
    pub auto_mine: Option<MineIntent>,
    pub auto_mace: Option<MaceAim>,
    pub attack_strength: f32,
    pub attack_delay: f32,
    pub breaking: Vec<BreakingBlock>,
    pub remote_breaking: std::collections::HashMap<i32, RemoteBreak>,
    pub item_pickups: Vec<crate::play::item_pickup::ItemPickup>,
    pub particle_emits: Vec<ParticleEmit>,
    pub map_updates: Vec<MapUpdate>,
    pub clear_maps: bool,
    pub attack_held: bool,
    pub use_held: bool,
    pub attack_clicked: bool,
    pub use_clicked: bool,
    pub pick_clicked: bool,
    pub pick_include_data: bool,
    pub chat_incoming: Vec<ChatEntry>,
    pub toast_incoming: Vec<crate::gui::toast::ToastEvent>,
    pub outgoing_chat: Vec<String>,

    pub chat_signing_session: bool,
    pub chat_signing_prompt: bool,
    pub chat_signing_declined: bool,
    pub chat_signing_wait: u32,
    pub chat_ack_pending: u32,
    pub chat_ack_last: Option<[u8; 256]>,
    pub chat_send_error: Option<String>,
    pub hardcore: bool,
    pub local_anim: HumanoidAnim,
    #[cfg(feature = "skins")]
    pub local_skin: crate::client::skins::SkinState,
    pub item_used: [u32; 2],
    pub item_activation: Option<ItemActivation>,
    pub swap_hands: bool,
    pub command_tree: Option<std::sync::Arc<crate::play::commands::CommandTree>>,
    pub suggestion_request: Option<(u32, String)>,
    pub suggestion_reply: Option<(u32, crate::play::commands::Suggestions)>,
    pub player_names: Vec<String>,
    #[cfg(lua_mods)]
    pub player_profiles: std::sync::Arc<Vec<crate::client::tablist::Profile>>,
    pub tab_list: std::sync::Arc<crate::client::tablist::TabList>,
    pub sidebar: std::sync::Arc<crate::client::tablist::Sidebar>,
    pub boss_bars: std::sync::Arc<Vec<crate::client::bossbar::BossBar>>,
    pub esp: std::sync::Arc<crate::modules::esp::EspFrame>,
    pub custom_completions: Vec<String>,
    pub entities: std::sync::Arc<Vec<crate::entities::feed::EntityAnim>>,
    pub day_clock: WorldClock,
    pub env: Option<crate::renderer::environment::BiomeLayer>,
    pub env_prev: Option<crate::renderer::environment::BiomeLayer>,
    pub block_entities: std::sync::Arc<crate::blockentities::feed::BlockEntityMap>,
    pub block_entities_version: u64,
    pub render_distance_request: Option<u32>,
    pub skin_prefs_request: Option<SkinPrefs>,
    pub xp_progress: f32,
    pub xp_level: u32,
    pub xp_seen: bool,
    pub xp_display_tick: i64,
    pub waypoints: std::sync::Arc<Vec<WaypointInfo>>,
    pub action_bar: Option<Vec<crate::text::Span>>,
    pub title_text: Option<Vec<crate::text::Span>>,
    pub subtitle_text: Option<Vec<crate::text::Span>>,
    pub title_times: Option<(Option<u32>, Option<u32>, Option<u32>)>,
    pub titles_clear: Option<bool>,
}

impl SessionState {
    pub fn clear_clicks(&mut self) {
        self.attack_held = false;
        self.use_held = false;
        self.attack_clicked = false;
        self.use_clicked = false;
    }

    pub fn set_action_bar(&mut self, spans: Vec<crate::text::Span>) {
        self.action_bar = Some(spans);
    }

    pub fn set_title(&mut self, spans: Vec<crate::text::Span>) {
        self.title_text = Some(spans);
        self.titles_clear = None;
    }

    pub fn set_subtitle(&mut self, spans: Vec<crate::text::Span>) {
        self.subtitle_text = Some(spans);
    }

    pub fn set_title_times(&mut self, fade_in: i32, stay: i32, fade_out: i32) {
        let (mut a, mut b, mut c) = self.title_times.unwrap_or_default();
        if fade_in >= 0 {
            a = Some(fade_in as u32);
        }
        if stay >= 0 {
            b = Some(stay as u32);
        }
        if fade_out >= 0 {
            c = Some(fade_out as u32);
        }
        self.title_times = Some((a, b, c));
    }

    pub fn clear_titles(&mut self, reset_times: bool) {
        self.title_text = None;
        self.subtitle_text = None;
        if reset_times {
            self.title_times = None;
        }
        self.titles_clear = Some(reset_times);
    }
}

pub struct FovState {
    pub prev: f32,
    pub cur: f32,
    pub effects: f32,
    pub first_person: bool,
}

impl Default for FovState {
    fn default() -> Self {
        Self {
            prev: 1.0,
            cur: 1.0,
            effects: 1.0,
            first_person: true,
        }
    }
}

impl FovState {
    pub fn sample(&self, partial: f32) -> f32 {
        self.prev + (self.cur - self.prev) * partial
    }
}

pub struct CameraEye {
    pub prev: f32,
    pub cur: f32,
}

impl Default for CameraEye {
    fn default() -> Self {
        let eye = PlayerBody::STANDING.eye_height;
        Self {
            prev: eye,
            cur: eye,
        }
    }
}

impl CameraEye {
    pub fn tick(&mut self, target: f32) {
        self.prev = self.cur;
        self.cur += (target - self.cur) * 0.5;
    }

    pub fn sample(&self, partial: f32) -> f32 {
        self.prev + (self.cur - self.prev) * partial
    }
}

#[derive(Default, Clone, Copy)]
pub struct PortalEffect {
    pub prev: f32,
    pub cur: f32,
}

impl PortalEffect {
    const STEP_IN: f32 = 0.0125;
    const STEP_OUT: f32 = 0.05;

    pub fn tick(&mut self, inside: bool) {
        self.prev = self.cur;
        let step = if inside {
            Self::STEP_IN
        } else {
            -Self::STEP_OUT
        };
        self.cur = (self.cur + step).clamp(0.0, 1.0);
    }

    pub fn sample(&self, partial: f32) -> f32 {
        self.prev + (self.cur - self.prev) * partial
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum WaypointKey {
    Uuid(u128),
    Name(String),
}

#[derive(Clone, Copy, PartialEq)]
pub enum WaypointPos {
    Block([i32; 3]),
    Chunk { x: i32, z: i32 },
    Azimuth(f32),
}

#[derive(Clone, PartialEq)]
pub struct WaypointInfo {
    pub id: WaypointKey,
    pub style: String,
    pub color: Option<[f32; 3]>,
    pub pos: WaypointPos,
}

#[derive(Clone, Copy, Debug)]
pub struct WorldClock {
    pub ticks: f64,
    pub rate: f32,
    pub drift: f64,
}

impl Default for WorldClock {
    fn default() -> Self {
        WorldClock {
            ticks: 6000.0,
            rate: 1.0,
            drift: 0.0,
        }
    }
}

const RESYNC_SNAP_TICKS: f64 = 40.0;

const DRIFT_PAYOFF: f64 = 0.05;

impl WorldClock {
    pub fn at(&self, partial: f32) -> f64 {
        self.ticks + (self.rate * partial) as f64
    }

    pub fn advance(&mut self) {
        let slice = self.drift * DRIFT_PAYOFF;
        self.drift -= slice;
        self.ticks += (self.rate as f64 + slice).max(0.0);
    }

    pub fn resync(&mut self, server: WorldClock) {
        let drift = server.ticks - self.ticks;
        if drift.abs() >= RESYNC_SNAP_TICKS {
            *self = server;
            return;
        }
        self.rate = server.rate;
        self.drift = drift;
    }
}
