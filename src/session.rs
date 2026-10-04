use crate::renderer::{HumanoidAnim, PendingSection};

pub const MOVE_FORWARD: u8 = 1 << 0;
pub const MOVE_BACK: u8 = 1 << 1;
pub const MOVE_LEFT: u8 = 1 << 2;
pub const MOVE_RIGHT: u8 = 1 << 3;
pub const MOVE_JUMP: u8 = 1 << 4;
pub const MOVE_SPRINT: u8 = 1 << 5;
pub const MOVE_DESCEND: u8 = 1 << 6;
pub const MOVE_SNEAK: u8 = 1 << 7;

pub const MAX_AIR_SUPPLY: i32 = 300;

pub const VANILLA_MAX_HEALTH: f32 = 20.0;

#[derive(Default)]
pub struct SharedState {
    pub camera_yaw: f32,
    pub camera_pitch: f32,
    pub move_flags: u8,
    pub screen_open: bool,
    pub profiling: ChunkProfiling,
    pub quit_requested: bool,
    pub disconnect_requested: bool,
    pub disconnected_pending: bool,
    pub session_ended: bool,
    pub end_notice: Option<String>,
    pub disconnect_reason: Option<Vec<crate::text::Span>>,
    pub reload_chunks_requested: bool,
    pub leave_bed_requested: bool,
    pub respawn_requested: bool,
    pub in_world: bool,
    pub session: SessionState,
    pub render_distance_sent: Option<u32>,
    pub skin_prefs_sent: Option<SkinPrefs>,
    pub skin_prefs: SkinPrefs,
    pub chat_signing_allowed: bool,
}

impl SharedState {
    pub fn end_session(&mut self, status: String, reason: Option<Vec<crate::text::Span>>) -> bool {
        let first = !std::mem::replace(&mut self.session_ended, true);
        if first {
            self.disconnected_pending = true;
            self.end_notice = Some(match &reason {
                Some(spans) if !spans.is_empty() => {
                    spans.iter().map(|span| span.text.as_str()).collect()
                }
                _ => "Connection lost".to_string(),
            });
            self.disconnect_reason = reason;
        }
        self.clear_session(status);
        first
    }

    pub fn clear_session(&mut self, status: String) {
        self.in_world = false;
        let health_epoch = self.session.health_epoch;
        let block_entities_version = self.session.block_entities_version;
        self.session = SessionState::default();
        self.session.health_epoch = health_epoch;
        self.session.block_entities_version = block_entities_version.wrapping_add(1);
        self.session.clear_chunks = true;
        self.session.status = Some(status.into());
    }
}

#[derive(Default, Clone)]
pub struct ChunkProfiling {
    pub mesh_avg_ms: f32,
    pub mesh_peak_ms: f32,
    pub chunks_meshed: u64,
    pub queue_depth: usize,
    pub worker_count: usize,
    pub poll_avg_ms: f32,
    pub poll_peak_ms: f32,
    pub lock_wait_avg_ms: f32,
}

impl ChunkProfiling {
    pub fn record_mesh(&mut self, elapsed_ms: f32) {
        const A: f32 = 0.05;
        self.mesh_avg_ms = self.mesh_avg_ms * (1.0 - A) + elapsed_ms * A;
        if elapsed_ms > self.mesh_peak_ms {
            self.mesh_peak_ms = elapsed_ms;
        }
        self.chunks_meshed += 1;
    }

    pub fn record_poll(&mut self, elapsed_ms: f32) {
        const A: f32 = 0.1;
        self.poll_avg_ms = self.poll_avg_ms * (1.0 - A) + elapsed_ms * A;
        if elapsed_ms > self.poll_peak_ms {
            self.poll_peak_ms = elapsed_ms;
        }
    }

    pub fn record_lock_wait(&mut self, elapsed_ms: f32) {
        const A: f32 = 0.1;
        self.lock_wait_avg_ms = self.lock_wait_avg_ms * (1.0 - A) + elapsed_ms * A;
    }

    pub fn reset_peaks(&mut self) {
        self.mesh_peak_ms = 0.0;
        self.poll_peak_ms = 0.0;
    }
}

#[derive(Default)]
pub struct SessionState {
    pub pending_chunks: Vec<PendingSection>,
    pub pending_edits: Vec<PendingSection>,
    pub unloaded_chunks: Vec<(i32, i32)>,
    pub clear_chunks: bool,
    pub player_pos: [f32; 3],
    pub status: Option<std::sync::Arc<str>>,
    pub current_address: Option<String>,
    pub advanced_tooltips: bool,
    pub fly_toggle: bool,
    pub flying: bool,
    pub auto_jump: bool,
    pub fov: FovState,
    pub other_players: Vec<OtherPlayerInfo>,
    pub menu_slots: Vec<SlotStack>,
    pub hotbar: std::sync::Arc<[SlotStack]>,
    pub armor: crate::renderer::Armor,
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
    pub horse: Option<HorseMenu>,
    pub horse_published_state_id: u32,
    pub inv_actions: Vec<InvAction>,
    pub hotbar_selected: u8,
    pub health: f32,
    pub health_display: HealthDisplay,
    pub health_epoch: u32,
    pub dead: bool,
    pub death_cause: Option<Vec<crate::text::Span>>,
    pub death_score: i32,
    pub show_death_screen: bool,
    pub jump_charge: Option<f32>,
    pub food: u32,
    pub saturation: f32,
    pub air_supply: i32,
    pub active_effects: Vec<crate::play::mob_effects::MobEffectInstance>,
    pub gamemode: Gamemode,
    pub crouching: bool,
    pub fall_flying: bool,
    pub swimming: bool,
    pub sleeping: bool,
    pub bed_orientation: Option<crate::direction::Direction>,
    pub sleep_timer: u32,
    pub player_pos_prev: [f32; 3],
    pub game_time: u64,
    pub last_tick_time: Option<crate::platform::time::Instant>,
    pub hotbar_target: Option<u8>,
    pub spectator_teleport_target: Option<u128>,
    pub fly_speed_scroll: f32,
    pub pending_drop: Option<DropRequest>,
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
    pub dialog_show: Option<std::sync::Arc<crate::play::dialog::Dialog>>,
    pub dialog_clear: bool,
    pub dialog_submits: Vec<crate::play::dialog::Submit>,
    pub server_links: Vec<ServerLink>,
    pub targeted_block: Option<TargetedBlock>,
    pub targeted_entity: bool,
    pub crosshair_entity: Option<i32>,
    pub aim_target: Option<i32>,
    pub auto_mine: Option<MineIntent>,
    pub auto_mace: Option<MaceAim>,
    pub attack_strength: f32,
    pub attack_delay: f32,
    pub breaking: Option<BreakingBlock>,
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
    pub enforces_secure_chat: bool,
    pub hardcore: bool,
    pub local_anim: HumanoidAnim,
    #[cfg(feature = "skins")]
    pub local_skin: crate::client::skins::SkinState,
    pub item_used: [u32; 2],
    pub swap_hands: bool,
    pub command_tree: Option<std::sync::Arc<crate::play::commands::CommandTree>>,
    pub suggestion_request: Option<(u32, String)>,
    pub suggestion_reply: Option<(u32, crate::play::commands::Suggestions)>,
    pub player_names: Vec<String>,
    pub tab_list: std::sync::Arc<crate::client::tablist::TabList>,
    pub sidebar: std::sync::Arc<crate::client::tablist::Sidebar>,
    pub boss_bars: std::sync::Arc<Vec<crate::client::bossbar::BossBar>>,
    pub esp: std::sync::Arc<crate::modules::esp::EspFrame>,
    pub custom_completions: Vec<String>,
    pub entities: std::sync::Arc<Vec<crate::entities::feed::EntityAnim>>,
    pub day_clock: WorldClock,
    pub env: Option<crate::renderer::environment::BiomeLayer>,
    pub env_prev: Option<crate::renderer::environment::BiomeLayer>,
    pub block_entities: std::sync::Arc<
        std::collections::HashMap<[i32; 3], crate::blockentities::feed::BlockEntityInfo>,
    >,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChatTag {
    #[default]
    Secure,
    Modified,
    NotSecure,
    System,
    Error,
}

impl ChatTag {
    pub fn indicator_color(self) -> Option<u32> {
        match self {
            ChatTag::Secure => None,
            ChatTag::Modified => Some(0x606060),
            ChatTag::NotSecure | ChatTag::System => Some(0xD0D0D0),
            ChatTag::Error => Some(0xFF5555),
        }
    }

    pub fn tooltip(self) -> Option<&'static str> {
        match self {
            ChatTag::Secure => Some("Verified message. Signed by the sender:"),
            ChatTag::Modified => Some("Message modified by the server. Original:"),
            ChatTag::NotSecure => Some("Unverified message. Cannot be reported."),
            ChatTag::System => Some("Server message. Cannot be reported."),
            ChatTag::Error => Some("Chat Error"),
        }
    }
}

#[derive(Clone)]
pub struct ChatEntry {
    pub spans: Vec<crate::text::Span>,
    pub events: crate::text::events::Events,
    pub tag: ChatTag,
    pub original: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct TargetedBlock {
    pub pos: [i32; 3],
    pub boxes: Vec<[f32; 6]>,
}

#[derive(Clone, PartialEq)]
pub struct BreakingBlock {
    pub pos: [i32; 3],
    pub stage: u8,
    pub boxes: Vec<[f32; 6]>,
}

#[derive(Clone)]
pub enum ParticleEmit {
    Block(ParticleSpawn),
    Level(LevelParticle),
    Event(LevelEventEmit),
}

#[derive(Clone, Copy)]
pub struct LevelEventEmit {
    pub id: u32,
    pub pos: [i32; 3],
    pub data: u32,
    pub global: bool,
}

pub const MAX_PENDING_PARTICLE_EMITS: usize = 256;

pub fn push_particle_emit(queue: &mut Vec<ParticleEmit>, emit: ParticleEmit) {
    if queue.len() < MAX_PENDING_PARTICLE_EMITS {
        queue.push(emit);
    }
}

#[derive(Clone, Copy)]
pub struct ParticleSpawn {
    pub pos: [i32; 3],
    pub state: azalea::block::BlockState,
    pub mining: bool,
    pub face: [f32; 3],
}

#[derive(Clone)]
pub struct LevelParticle {
    pub particle: azalea::entity::particle::Particle,
    pub pos: [f64; 3],
    pub dist: [f32; 3],
    pub max_speed: f32,
    pub count: u32,
    pub override_limiter: bool,
    pub always_show: bool,
}

#[derive(Clone)]
pub struct MapUpdate {
    pub id: i32,
    pub colors: std::sync::Arc<Vec<u8>>,
    pub decorations: Option<std::sync::Arc<Vec<MapDecoration>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapDecoration {
    pub kind: u8,
    pub x: i8,
    pub y: i8,
    pub rot: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotStack {
    pub item: &'static str,
    pub count: u8,
    pub cooldown: u8,
    pub damage: u16,
    pub max_damage: u16,
    pub custom_name: Option<Vec<crate::text::Span>>,
    pub lore: Vec<Vec<crate::text::Span>>,
    pub enchantments: Vec<(String, i32)>,
    pub potion: Option<crate::items::potions::PotionContents>,
    pub unbreakable: bool,
    pub charged: bool,
    pub map_id: Option<i32>,
    pub component_count: u16,
    pub nbt_bytes: usize,
    pub banner_patterns: Option<Box<[Box<str>]>>,
    pub book: Option<Box<Book>>,
    pub banner_layers: Option<Box<[crate::blockentities::banner::BannerLayer]>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub pages: Box<[Vec<crate::text::Span>]>,
    pub title: String,
    pub author: String,
    pub generation: u8,
    pub signed: bool,
}

impl Book {
    pub fn raw_pages(&self) -> Vec<String> {
        self.pages
            .iter()
            .map(|spans| spans.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }
}

impl SlotStack {
    pub fn is_empty(&self) -> bool {
        self.item.is_empty() || self.count == 0
    }

    pub fn model_key(&self) -> std::borrow::Cow<'_, str> {
        use crate::items::potions;
        use std::borrow::Cow;

        if let Some(layers) = &self.banner_layers
            && !layers.is_empty()
        {
            return Cow::Owned(crate::blockentities::banner::model_key(self.item, layers));
        }

        match &self.potion {
            Some(contents) if potions::is_potion_item(self.item) => {
                Cow::Owned(potions::tint_key(self.item, contents.color()))
            }
            _ => Cow::Borrowed(self.item),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkinPrefs {
    pub main_hand_left: bool,
    #[cfg(feature = "skins")]
    pub skin_parts: u8,
}

impl Default for SkinPrefs {
    fn default() -> Self {
        Self {
            main_hand_left: false,
            #[cfg(feature = "skins")]
            skin_parts: crate::client::skins::ALL_PARTS,
        }
    }
}

#[derive(Clone, Default)]
pub struct OtherPlayerInfo {
    pub id: i32,
    pub username: String,
    pub name_tag: Vec<crate::text::Span>,
    pub hidden_by_team: bool,
    pub health: Option<f32>,
    pub max_health: f32,
    pub gamemode: Option<Gamemode>,
    pub below_name: Vec<crate::text::Span>,
    pub discrete: bool,
    #[cfg(feature = "skins")]
    pub skin: crate::client::skins::SkinState,
    pub anim: HumanoidAnim,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ContainerKind {
    #[default]
    None,
    Chest(u8),
    Crafting,
    Furnace(FurnaceKind),
    Hopper,
    Grindstone,
    Enchantment,
    Loom,
    Stonecutter,
    Cartography,
    Smithing,
    Beacon,
    BrewingStand,
    Merchant,
    Lectern,
    Horse(u8),
    Anvil,
}

impl ContainerKind {
    pub fn is_some(self) -> bool {
        self != ContainerKind::None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FurnaceKind {
    Furnace,
    BlastFurnace,
    Smoker,
}

impl FurnaceKind {
    pub fn texture_key(self) -> &'static str {
        match self {
            FurnaceKind::Furnace => "furnace",
            FurnaceKind::BlastFurnace => "blast_furnace",
            FurnaceKind::Smoker => "smoker",
        }
    }
}

pub type ContainerData = [i16; CONTAINER_DATA_LEN];

pub const CONTAINER_DATA_LEN: usize = 10;

#[derive(Clone, Default)]
pub struct MerchantOffers {
    pub offers: Vec<MerchantOffer>,
    pub level: u32,
    pub xp: u32,
    pub show_progress: bool,
    pub can_restock: bool,
}

impl MerchantOffers {
    pub fn future_xp(&self, buy_a: &SlotStack, buy_b: &SlotStack, hint: i32) -> u32 {
        self.active_offer(buy_a, buy_b, hint)
            .map_or(0, |i| self.offers[i].xp)
    }

    pub fn active_offer(&self, buy_a: &SlotStack, buy_b: &SlotStack, hint: i32) -> Option<usize> {
        let empty = SlotStack::default();
        let (buy_a, buy_b) = if buy_a.is_empty() {
            (buy_b, &empty)
        } else {
            (buy_a, buy_b)
        };
        self.recipe_for(buy_a, buy_b, hint)
            .filter(|i| !self.offers[*i].out_of_stock)
            .or_else(|| self.recipe_for(buy_b, buy_a, hint))
            .filter(|i| !self.offers[*i].out_of_stock)
    }

    pub fn notify_trade(&mut self, index: usize) {
        let Some(offer) = self.offers.get_mut(index) else {
            return;
        };
        offer.uses = offer.uses.saturating_add(1);
        offer.out_of_stock = offer.uses >= offer.max_uses;
        self.xp = self.xp.saturating_add(offer.xp);
    }

    fn recipe_for(&self, buy_a: &SlotStack, buy_b: &SlotStack, hint: i32) -> Option<usize> {
        if hint > 0 && (hint as usize) < self.offers.len() {
            let i = hint as usize;
            return self.offers[i].satisfied_by(buy_a, buy_b).then_some(i);
        }
        self.offers
            .iter()
            .position(|o| o.satisfied_by(buy_a, buy_b))
    }
}

pub fn empty_spans() -> std::sync::Arc<[crate::text::Span]> {
    EMPTY_SPANS.clone()
}

static EMPTY_SPANS: std::sync::LazyLock<std::sync::Arc<[crate::text::Span]>> =
    std::sync::LazyLock::new(|| std::sync::Arc::from(Vec::new()));

#[derive(Clone, Default)]
pub struct MerchantOffer {
    pub cost_a: SlotStack,
    pub base_cost_a: SlotStack,
    pub cost_b: SlotStack,
    pub result: SlotStack,
    pub out_of_stock: bool,
    pub uses: u32,
    pub max_uses: u32,
    pub xp: u32,
}

impl MerchantOffer {
    fn satisfied_by(&self, buy_a: &SlotStack, buy_b: &SlotStack) -> bool {
        if self.cost_a.item != buy_a.item || buy_a.count < self.cost_a.count {
            return false;
        }
        if self.cost_b.is_empty() {
            buy_b.is_empty()
        } else {
            self.cost_b.item == buy_b.item && buy_b.count >= self.cost_b.count
        }
    }
}

#[derive(Clone, Default)]
pub struct HorseMenu {
    pub container_id: i32,
    pub columns: u8,
    pub state_id: u32,
    pub slots: Vec<SlotStack>,
}

impl HorseMenu {
    pub fn player_slots_start(&self) -> usize {
        2 + self.columns as usize * 3
    }

    pub fn player_slots(&self) -> &[SlotStack] {
        let start = self.player_slots_start();
        self.slots.get(start..).unwrap_or(&[])
    }
}

pub struct StonecutterRecipe {
    pub input: Box<[Box<str>]>,
    pub result: SlotStack,
}

#[derive(Clone, Copy, Default)]
pub struct HealthDisplay {
    pub max_health: f32,
    pub absorption: f32,
    pub armor: u8,
    pub kind: HeartKind,
    pub regenerating: bool,
    pub hunger_effect: bool,
    pub ticks_frozen: i32,
    pub vehicle: Option<(f32, f32)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum HeartKind {
    #[default]
    Normal,
    Poisoned,
    Withered,
    Frozen,
}

#[derive(Clone, Copy)]
pub struct MineIntent {
    pub aim: Option<[f32; 3]>,
    pub forward: bool,
    pub turn: f32,
}

#[derive(Clone, Copy)]
pub struct MaceAim {
    pub aim: [f32; 3],
    pub turn: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Gamemode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl Gamemode {
    #[cfg(feature = "shader_support")]
    pub fn is_survival(self) -> bool {
        matches!(self, Gamemode::Survival | Gamemode::Adventure)
    }

    pub fn from_azalea(mode: azalea_core::game_type::GameMode) -> Gamemode {
        match mode {
            azalea_core::game_type::GameMode::Survival => Gamemode::Survival,
            azalea_core::game_type::GameMode::Creative => Gamemode::Creative,
            azalea_core::game_type::GameMode::Adventure => Gamemode::Adventure,
            azalea_core::game_type::GameMode::Spectator => Gamemode::Spectator,
        }
    }
}

pub enum InvAction {
    Click(azalea_inventory::operations::ClickOperation),
    CreativeSet {
        slot: u16,
        stack: SlotStack,
    },
    CreativeDrop {
        stack: SlotStack,
    },
    ButtonClick(u8),
    SelectTrade(u32),
    SetBeacon {
        primary: Option<u32>,
        secondary: Option<u32>,
    },
    RenameItem(String),
    Close,
}

#[derive(Clone, Copy)]
pub enum DropRequest {
    HeldItem,
    HeldStack,
}

#[derive(Clone, Copy)]
pub struct CommandBlockOpen {
    pub pos: azalea_core::position::BlockPos,
    pub mode: CommandBlockMode,
    pub conditional: bool,
}

pub use azalea_protocol::packets::game::s_set_command_block::Mode as CommandBlockMode;

#[derive(Clone, Default)]
pub struct CommandBlockData {
    pub command: String,
    pub track_output: bool,
    pub last_output: String,
    pub automatic: bool,
}

pub struct CommandBlockUpdate {
    pub pos: azalea_core::position::BlockPos,
    pub command: String,
    pub mode: CommandBlockMode,
    pub track_output: bool,
    pub conditional: bool,
    pub automatic: bool,
}

#[derive(Clone)]
pub struct SignEditRequest {
    pub pos: azalea_core::position::BlockPos,
    pub front: bool,
    pub kind: SignEditKind,
    pub wood: &'static str,
    pub face: crate::blockentities::feed::SignFace,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignEditKind {
    Standing { wall: bool },
    Hanging,
}

impl SignEditKind {
    pub fn from_block(block: &str) -> Option<(SignEditKind, &'static str)> {
        use crate::blockentities::render::sign;
        use crate::blockentities::text::SignKind;
        let kind = match SignKind::of(crate::blockentities::feed::kind_for_block(block)?)? {
            SignKind::Standing => SignEditKind::Standing {
                wall: sign::is_wall(block),
            },
            SignKind::Hanging => SignEditKind::Hanging,
        };
        Some((kind, sign::known_wood(block).unwrap_or("oak")))
    }

    pub fn sign_kind(self) -> crate::blockentities::text::SignKind {
        match self {
            SignEditKind::Standing { .. } => crate::blockentities::text::SignKind::Standing,
            SignEditKind::Hanging => crate::blockentities::text::SignKind::Hanging,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServerLink {
    pub label: Vec<crate::text::Span>,
    pub url: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractionHand {
    Main,
    Off,
}

impl InteractionHand {
    pub fn edit_book_slot(self, selected: u8) -> u32 {
        match self {
            InteractionHand::Main => selected as u32,
            InteractionHand::Off => 40,
        }
    }

    pub fn hotbar_index(self, selected: u8) -> usize {
        match self {
            InteractionHand::Main => selected as usize,
            InteractionHand::Off => 9,
        }
    }
}

#[derive(Clone)]
pub struct BookEditRequest {
    pub hand: InteractionHand,
    pub author: String,
}

#[derive(Clone)]
pub struct EditBookRequest {
    pub slot: u32,
    pub pages: Vec<String>,
    pub title: Option<String>,
}

#[derive(Clone)]
pub struct SignUpdateRequest {
    pub pos: azalea_core::position::BlockPos,
    pub front: bool,
    pub lines: [String; 4],
}

pub struct SharedMutex(parking_lot::Mutex<SharedState>);

const TOAST_QUEUE_MAX: usize = 32;

pub fn queue_toasts(
    shared: &std::sync::Arc<SharedMutex>,
    events: impl IntoIterator<Item = crate::gui::toast::ToastEvent>,
) {
    let mut s = shared.lock().unwrap();
    let queue = &mut s.session.toast_incoming;
    for event in events {
        if queue.len() >= TOAST_QUEUE_MAX {
            break;
        }
        queue.push(event);
    }
}

#[derive(Debug)]
pub struct WouldBlock;

pub struct SideMutex<T>(parking_lot::Mutex<T>);

impl<T> SideMutex<T> {
    pub const fn new(value: T) -> Self {
        SideMutex(parking_lot::Mutex::new(value))
    }

    #[allow(clippy::result_unit_err, reason = "mirrors `std::sync::Mutex::lock`")]
    pub fn lock(&self) -> Result<parking_lot::MutexGuard<'_, T>, std::convert::Infallible> {
        Ok(self.0.lock())
    }

    pub fn try_lock(&self) -> Result<parking_lot::MutexGuard<'_, T>, WouldBlock> {
        self.0.try_lock().ok_or(WouldBlock)
    }
}

impl<T: Default> Default for SideMutex<T> {
    fn default() -> Self {
        SideMutex::new(T::default())
    }
}

impl SharedMutex {
    pub const fn new(state: SharedState) -> Self {
        SharedMutex(parking_lot::Mutex::new(state))
    }

    #[allow(clippy::result_unit_err, reason = "mirrors `std::sync::Mutex::lock`")]
    pub fn lock(
        &self,
    ) -> Result<parking_lot::MutexGuard<'_, SharedState>, std::convert::Infallible> {
        Ok(self.0.lock())
    }

    pub fn try_lock(&self) -> Result<parking_lot::MutexGuard<'_, SharedState>, WouldBlock> {
        self.0.try_lock().ok_or(WouldBlock)
    }

    pub fn try_lock_for(
        &self,
        timeout: std::time::Duration,
    ) -> Result<parking_lot::MutexGuard<'_, SharedState>, WouldBlock> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = timeout;
            self.try_lock()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.0.try_lock_for(timeout).ok_or(WouldBlock)
        }
    }
}

impl Default for SharedMutex {
    fn default() -> Self {
        SharedMutex::new(SharedState::default())
    }
}

#[cfg(test)]
mod merchant_tests {
    use super::*;

    fn cost(item: &'static str, count: u8) -> SlotStack {
        SlotStack {
            item,
            count,
            ..SlotStack::default()
        }
    }

    fn offer(a: SlotStack, b: SlotStack, xp: u32) -> MerchantOffer {
        MerchantOffer {
            cost_a: a,
            cost_b: b,
            xp,
            ..MerchantOffer::default()
        }
    }

    fn offers(list: Vec<MerchantOffer>) -> MerchantOffers {
        MerchantOffers {
            offers: list,
            ..MerchantOffers::default()
        }
    }

    #[test]
    fn a_trade_is_satisfied_by_its_price() {
        let o = offer(cost("emerald", 3), SlotStack::default(), 2);
        assert!(o.satisfied_by(&cost("emerald", 3), &SlotStack::default()));
        assert!(o.satisfied_by(&cost("emerald", 9), &SlotStack::default()));
        assert!(!o.satisfied_by(&cost("emerald", 2), &SlotStack::default()));
        assert!(!o.satisfied_by(&cost("diamond", 9), &SlotStack::default()));
        assert!(!o.satisfied_by(&cost("emerald", 3), &cost("book", 1)));

        let two = offer(cost("emerald", 3), cost("book", 1), 5);
        assert!(two.satisfied_by(&cost("emerald", 3), &cost("book", 1)));
        assert!(!two.satisfied_by(&cost("emerald", 3), &SlotStack::default()));
    }

    #[test]
    fn future_xp_finds_the_trade_whichever_slot_was_filled() {
        let list = offers(vec![offer(cost("emerald", 3), cost("book", 1), 5)]);
        let (em, bk) = (cost("emerald", 3), cost("book", 1));
        assert_eq!(list.future_xp(&em, &bk, -1), 5);
        assert_eq!(list.future_xp(&bk, &em, -1), 5);

        let single = offers(vec![offer(cost("emerald", 1), SlotStack::default(), 1)]);
        assert_eq!(
            single.future_xp(&SlotStack::default(), &cost("emerald", 1), -1),
            1
        );
        assert_eq!(
            single.future_xp(&SlotStack::default(), &SlotStack::default(), -1),
            0
        );
    }

    #[test]
    fn taking_a_trade_uses_it_up_and_pays_the_trader() {
        let mut list = offers(vec![MerchantOffer {
            cost_a: cost("emerald", 1),
            max_uses: 2,
            xp: 3,
            ..MerchantOffer::default()
        }]);
        list.notify_trade(0);
        assert_eq!(list.offers[0].uses, 1);
        assert!(!list.offers[0].out_of_stock);
        assert_eq!(list.xp, 3);
        list.notify_trade(0);
        assert!(list.offers[0].out_of_stock);
        assert_eq!(list.xp, 6);
        assert_eq!(
            list.active_offer(&cost("emerald", 1), &SlotStack::default(), -1),
            None
        );
        list.notify_trade(9);
        assert_eq!(list.xp, 6);
    }

    #[test]
    fn future_xp_honours_stock_and_the_selection_hint() {
        let mut sold_out = offer(cost("emerald", 1), SlotStack::default(), 4);
        sold_out.out_of_stock = true;
        let list = offers(vec![
            sold_out,
            offer(cost("emerald", 1), SlotStack::default(), 7),
        ]);
        let em = cost("emerald", 1);
        assert_eq!(list.future_xp(&em, &SlotStack::default(), 0), 0);
        assert_eq!(list.future_xp(&em, &SlotStack::default(), 1), 7);
    }
}
