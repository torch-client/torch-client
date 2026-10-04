use crate::renderer::clouds::CloudStatus;

pub mod accountlist;
#[cfg(feature = "asset_download")]
pub mod assetdownload;
pub mod atlas;
pub mod atlas_writes;
pub mod banner_icons;
pub mod book;
pub mod chat;
pub mod clickevent;
#[cfg(feature = "click_gui")]
pub mod clickgui;
pub mod command_block;
pub mod command_history;
pub mod confirmlink;
pub mod container;
pub mod creative;
pub mod death;
pub mod dialog;
pub mod disconnected;
pub mod draw;
pub mod focus;
pub mod health;
pub mod hud;
#[cfg(feature = "hud_editor")]
pub mod hud_editor;
pub mod hud_layout;
pub mod keybinds;
pub mod menu;
pub mod multiplayer;
pub mod options;
pub mod painter;
pub mod pause;
pub mod ping;
#[cfg(feature = "skins")]
pub mod player_faces;
pub mod pool;
#[cfg(test)]
pub mod preview;
pub mod profile;
pub mod render;
#[cfg(resource_packs)]
pub mod resourcepacks;
pub mod screens;
pub mod serverlist;
#[cfg(feature = "shader_support")]
pub mod shaderpacks;
pub mod sign_edit;
pub mod signedchat;
pub mod sleep;
pub mod slots;
pub mod spectator_menu;
pub mod suggestions;
pub mod tablist;
pub mod title;
pub mod toast;
pub mod tooltip;
pub mod widgets;

use bevy::prelude::*;

use crate::session::{ContainerKind, Gamemode, SlotStack};
pub use render::{GuiInput, GuiPlugin};

#[inline]
pub fn keyboard_elsewhere() -> bool {
    false
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    #[default]
    None,
    Title,
    #[cfg(feature = "asset_download")]
    AssetDownload,
    Accounts,
    EditProfile,
    #[cfg(feature = "online_mode")]
    AddAccount,
    #[cfg(feature = "online_mode")]
    MicrosoftLogin,
    Multiplayer,
    ManageServer,
    DirectConnect,
    Disconnected,
    Pause,
    Options,
    VideoSettings,
    Controls,
    GameSettings,
    #[cfg(feature = "audio")]
    AudioSettings,
    #[cfg(resource_packs)]
    ResourcePacks,
    #[cfg(feature = "shader_support")]
    ShaderPacks,
    #[cfg(feature = "shader_support")]
    ShaderOptions,
    Dialog,

    Inventory,
    Creative,

    Container(ContainerKind),

    CommandBlock,
    SignEdit,
    BookView,
    BookEdit,
    BookSign,
    Chat,
    Sleep,
    Death,
    SignedChatPrompt,

    #[cfg(feature = "click_gui")]
    ClickGui,

    #[cfg(feature = "hud_editor")]
    HudEditor,
}

impl Screen {
    pub fn is_open(self) -> bool {
        self != Screen::None
    }

    pub fn is_modal(self) -> bool {
        matches!(self, Screen::Inventory | Screen::Creative) || self.is_container()
    }

    pub fn is_container(self) -> bool {
        matches!(self, Screen::Container(_))
    }

    pub fn is_menu(self) -> bool {
        if matches!(
            self,
            Screen::Title
                | Screen::Multiplayer
                | Screen::ManageServer
                | Screen::DirectConnect
                | Screen::Accounts
                | Screen::EditProfile
                | Screen::Disconnected
        ) {
            return true;
        }
        #[cfg(feature = "asset_download")]
        if self == Screen::AssetDownload {
            return true;
        }
        #[cfg(feature = "online_mode")]
        {
            matches!(self, Screen::AddAccount | Screen::MicrosoftLogin)
        }
        #[cfg(not(feature = "online_mode"))]
        {
            false
        }
    }

    pub fn is_local_overlay(self) -> bool {
        if self == Screen::Dialog {
            return true;
        }
        #[cfg(feature = "hud_editor")]
        if self == Screen::HudEditor {
            return true;
        }
        #[cfg(feature = "click_gui")]
        {
            self == Screen::ClickGui
        }
        #[cfg(not(feature = "click_gui"))]
        {
            false
        }
    }

    pub fn is_options(self) -> bool {
        #[cfg(feature = "shader_support")]
        if matches!(self, Screen::ShaderPacks | Screen::ShaderOptions) {
            return true;
        }
        #[cfg(feature = "audio")]
        if self == Screen::AudioSettings {
            return true;
        }
        #[cfg(resource_packs)]
        if self == Screen::ResourcePacks {
            return true;
        }
        matches!(
            self,
            Screen::Options | Screen::VideoSettings | Screen::Controls | Screen::GameSettings
        )
    }

    pub fn tab_navigates(self) -> bool {
        if self.is_options() {
            return true;
        }
        #[cfg(feature = "asset_download")]
        if self == Screen::AssetDownload {
            return true;
        }
        #[cfg(feature = "online_mode")]
        if matches!(self, Screen::AddAccount | Screen::MicrosoftLogin) {
            return true;
        }
        matches!(
            self,
            Screen::Container(ContainerKind::Lectern)
                | Screen::Title
                | Screen::Pause
                | Screen::Multiplayer
                | Screen::ManageServer
                | Screen::DirectConnect
                | Screen::Accounts
                | Screen::EditProfile
                | Screen::Disconnected
                | Screen::Death
                | Screen::SignedChatPrompt
                | Screen::Dialog
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TouchMovement {
    #[default]
    Buttons,
    Joystick,
    PadStick,
}

impl TouchMovement {
    pub const ALL: [TouchMovement; 3] = [
        TouchMovement::Buttons,
        TouchMovement::Joystick,
        TouchMovement::PadStick,
    ];

    pub fn caption(self) -> &'static str {
        match self {
            TouchMovement::Buttons => "Buttons",
            TouchMovement::Joystick => "Joystick",
            TouchMovement::PadStick => "Pad Joystick",
        }
    }

    pub fn serialized_name(self) -> &'static str {
        match self {
            TouchMovement::Buttons => "buttons",
            TouchMovement::Joystick => "joystick",
            TouchMovement::PadStick => "pad_joystick",
        }
    }

    pub fn from_serialized_name(name: &str) -> Option<TouchMovement> {
        TouchMovement::ALL
            .into_iter()
            .find(|m| m.serialized_name() == name)
    }

    pub fn next(self) -> TouchMovement {
        let i = TouchMovement::ALL
            .iter()
            .position(|m| *m == self)
            .unwrap_or(0);
        TouchMovement::ALL[(i + 1) % TouchMovement::ALL.len()]
    }

    pub fn has_stick(self) -> bool {
        !matches!(self, TouchMovement::Buttons)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GuiOptions {
    pub gui_scale: u32,
    pub fov: u32,
    pub fov_effects: u32,
    pub clouds: crate::renderer::clouds::CloudStatus,
    pub particles: crate::util::particles::ParticleStatus,
    pub render_distance: u32,
    pub gamma: u32,
    pub antialiasing: bool,
    pub smooth_lighting: bool,
    pub lighting_enabled: bool,
    pub max_fps: u32,
    pub vsync: bool,
    pub auto_jump: bool,
    pub main_hand_left: bool,
    #[cfg(feature = "skins")]
    pub skin_parts: u8,
    pub fps_counter: bool,
    pub shader_quality: crate::renderer::terrain::ShaderQuality,
    pub shaders_enabled: bool,
    pub player_shadows: bool,
    pub touch_movement: TouchMovement,
    pub allow_signed_chat: bool,
    pub toast_advancement: bool,
    pub toast_recipe: bool,
    pub toast_system: bool,
    pub toast_time: u32,
    #[cfg(feature = "audio")]
    pub volumes: [u32; crate::audio::category::SoundCategory::COUNT],
}

#[cfg(feature = "audio")]
pub const VOLUME_DEFAULT: u32 = 100;

pub const TOAST_TIME_DEFAULT: u32 = 25;

pub const TOAST_TIME_MIN: i32 = 5;
pub const TOAST_TIME_MAX: i32 = 100;

pub const AUTO_JUMP_DEFAULT: bool = cfg!(feature = "mobile_ui");

pub const FPS_COUNTER_DEFAULT: bool = true;

pub const MAX_FPS_MIN: u32 = 10;
pub const MAX_FPS_UNLIMITED: u32 = 260;
pub const MAX_FPS_DEFAULT: u32 = if cfg!(feature = "mobile_ui") { 30 } else { 60 };

pub const VSYNC_DEFAULT: bool = !cfg!(target_os = "android");

pub const RENDER_DISTANCE_DEFAULT: u32 = if cfg!(feature = "mobile_ui") { 2 } else { 8 };

pub const SMOOTH_LIGHTING_DEFAULT: bool = !cfg!(feature = "mobile_ui");

pub const LIGHTING_ENABLED_DEFAULT: bool = !cfg!(feature = "mobile_ui");

pub(crate) const GAMMA_DEFAULT: u32 = 50;

pub const GAMMA_MAX: u32 = 150;

pub const FOV_EFFECTS_DEFAULT: u32 = 100;

#[cfg(feature = "skins")]
pub const MODEL_PART_KEYS: [(&str, u8); 7] = [
    ("modelPart_cape", 0),
    ("modelPart_jacket", 1),
    ("modelPart_left_sleeve", 2),
    ("modelPart_right_sleeve", 3),
    ("modelPart_left_pants_leg", 4),
    ("modelPart_right_pants_leg", 5),
    ("modelPart_hat", 6),
];

impl Default for GuiOptions {
    fn default() -> Self {
        let stored = load_options();
        let value = |key: &str| stored.as_ref().and_then(|json| json.get(key).cloned());
        GuiOptions {
            gui_scale: value("guiScale")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(0),
            fov: value("fov")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(crate::renderer::systems::VANILLA_FOV_DEGREES as u32),
            fov_effects: value("fovEffectScale")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(FOV_EFFECTS_DEFAULT)
                .min(FOV_EFFECTS_DEFAULT),
            clouds: value("renderClouds")
                .and_then(|v| v.as_str().and_then(CloudStatus::from_serialized_name))
                .unwrap_or(if cfg!(feature = "mobile_ui") {
                    CloudStatus::Fast
                } else {
                    CloudStatus::default()
                }),
            particles: value("particles")
                .and_then(|v| {
                    v.as_str()
                        .and_then(crate::util::particles::ParticleStatus::from_serialized_name)
                })
                .unwrap_or(if cfg!(feature = "mobile_ui") {
                    crate::util::particles::ParticleStatus::Decreased
                } else {
                    crate::util::particles::ParticleStatus::All
                }),
            render_distance: value("renderDistance")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(RENDER_DISTANCE_DEFAULT),
            gamma: value("gamma")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(GAMMA_DEFAULT)
                .min(GAMMA_MAX),
            antialiasing: value("antialiasing")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            smooth_lighting: value("ao")
                .and_then(|v| v.as_bool())
                .unwrap_or(SMOOTH_LIGHTING_DEFAULT),
            lighting_enabled: value("lightingEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(LIGHTING_ENABLED_DEFAULT),
            max_fps: value("maxFps")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(MAX_FPS_DEFAULT)
                .clamp(MAX_FPS_MIN, MAX_FPS_UNLIMITED),
            vsync: value("vsync")
                .and_then(|v| v.as_bool())
                .unwrap_or(VSYNC_DEFAULT),
            auto_jump: value("autoJump")
                .and_then(|v| v.as_bool())
                .unwrap_or(AUTO_JUMP_DEFAULT),
            main_hand_left: value("mainHand").and_then(|v| v.as_str().map(|s| s == "left"))
                == Some(true),
            #[cfg(feature = "skins")]
            skin_parts: {
                let mut parts = 0u8;
                for (key, bit) in MODEL_PART_KEYS {
                    if value(key).and_then(|v| v.as_bool()).unwrap_or(true) {
                        parts |= 1 << bit;
                    }
                }
                parts
            },
            fps_counter: value("fpsCounter")
                .and_then(|v| v.as_bool())
                .unwrap_or(FPS_COUNTER_DEFAULT),
            shader_quality: value("shaderQuality")
                .and_then(|v| {
                    v.as_str()
                        .and_then(crate::renderer::terrain::ShaderQuality::from_serialized_name)
                })
                .unwrap_or_default(),
            shaders_enabled: value("shaders").and_then(|v| v.as_bool()).unwrap_or(false),
            player_shadows: value("playerShadows")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            touch_movement: value("touchMovement")
                .and_then(|v| v.as_str().and_then(TouchMovement::from_serialized_name))
                .unwrap_or_default(),
            allow_signed_chat: value("allowSignedChat")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            toast_advancement: value("toastAdvancement")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            toast_recipe: value("toastRecipe")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            toast_system: value("toastSystem")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            toast_time: value("toastDisplayTime")
                .and_then(|v| v.as_f64())
                .map(|v| (v * 10.0).round() as u32)
                .unwrap_or(TOAST_TIME_DEFAULT)
                .clamp(TOAST_TIME_MIN as u32, TOAST_TIME_MAX as u32),
            #[cfg(feature = "audio")]
            volumes: {
                let mut volumes = [VOLUME_DEFAULT; crate::audio::category::SoundCategory::COUNT];
                for category in crate::audio::category::SoundCategory::ALL {
                    volumes[category.index()] = value(&format!("soundCategory_{}", category.key()))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32)
                        .unwrap_or(VOLUME_DEFAULT)
                        .min(100);
                }
                volumes
            },
        }
    }
}

fn load_options() -> Option<serde_json::Value> {
    let text = crate::platform::storage::OPTIONS.load()?;
    serde_json::from_str(&text).ok()
}

pub fn save_options(options: &GuiOptions) {
    let mut json = serde_json::json!({
        "guiScale": options.gui_scale,
        "fov": options.fov,
        "fovEffectScale": options.fov_effects,
        "renderClouds": options.clouds.serialized_name(),
        "particles": options.particles.serialized_name(),
        "renderDistance": options.render_distance,
        "gamma": options.gamma,
        "antialiasing": options.antialiasing,
        "ao": options.smooth_lighting,
        "lightingEnabled": options.lighting_enabled,
        "maxFps": options.max_fps,
        "vsync": options.vsync,
        "autoJump": options.auto_jump,
        "mainHand": if options.main_hand_left { "left" } else { "right" },
        "fpsCounter": options.fps_counter,
        "shaders": options.shaders_enabled,
        "shaderQuality": options.shader_quality.serialized_name(),
        "playerShadows": options.player_shadows,
        "touchMovement": options.touch_movement.serialized_name(),
        "allowSignedChat": options.allow_signed_chat,
        "toastAdvancement": options.toast_advancement,
        "toastRecipe": options.toast_recipe,
        "toastSystem": options.toast_system,
        "toastDisplayTime": options.toast_time as f64 / 10.0,
    });
    #[cfg(feature = "skins")]
    {
        let map = json.as_object_mut().expect("json! object literal");
        for (key, bit) in MODEL_PART_KEYS {
            map.insert(
                key.to_string(),
                serde_json::json!(options.skin_parts & (1 << bit) != 0),
            );
        }
    }
    #[cfg(feature = "audio")]
    {
        let map = json.as_object_mut().expect("json! object literal");
        for category in crate::audio::category::SoundCategory::ALL {
            map.insert(
                format!("soundCategory_{}", category.key()),
                serde_json::json!(options.volumes[category.index()]),
            );
        }
    }
    crate::platform::storage::OPTIONS.store(&json.to_string());
}

#[derive(Resource, Default)]
pub struct GuiState {
    pub screen: Screen,
    pub creative: creative::CreativeState,
    pub chat: chat::ChatState,
    pub sign_edit: sign_edit::SignEditState,
    pub command_block: command_block::CommandBlockState,
    pub book: book::BookState,
    pub dialog: dialog::DialogState,
    pub advanced_tooltips: bool,
    pub tab_list_held: bool,
    pub tablist: tablist::TabListState,
    pub hearts: health::HeartAnim,
    pub chunk_borders: bool,
    pub hide_gui: bool,
    pub slots: slots::SlotState,
    pub container: container::ContainerUi,
    #[cfg(feature = "click_gui")]
    pub clickgui: clickgui::ClickGuiState,
    #[cfg(feature = "click_gui")]
    pub clickgui_parent: Screen,
    pub hud_state: hud_layout::HudState,
    pub options: GuiOptions,
    pub keybinds: keybinds::KeyBinds,
    pub controls_scroll: f32,
    pub controls_scroll_drag: Option<f32>,
    pub nav: Option<Screen>,
    pub suppress_click: bool,
    pub death: death::DeathState,
    pub quit: bool,
    pub disconnect: bool,
    pub menu: multiplayer::MenuState,
    pub profile: profile::ProfileState,
    pub video_scroll: f32,
    pub video_scroll_drag: Option<f32>,
    pub game_scroll: f32,
    pub game_scroll_drag: Option<f32>,
    #[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
    pub confirm_clear_assets: bool,
    #[cfg(feature = "shader_support")]
    pub shaderpacks: shaderpacks::State,
    #[cfg(resource_packs)]
    pub resourcepacks: resourcepacks::State,
    pub options_parent: Screen,
    pub disconnect_reason: Option<Vec<crate::text::Span>>,
    pub connect: Option<String>,
    pub hud: hud::HudInfo,
    pub hud_overlays: hud::HudOverlays,
    pub toasts: toast::Toasts,
    pub spectator_menu: spectator_menu::SpectatorMenuState,
    #[cfg(feature = "mobile_ui")]
    pub mobile: crate::mobile::MobileUi,
}

impl GuiState {
    pub fn anvil_typing(&self) -> bool {
        self.screen == Screen::Container(ContainerKind::Anvil) && self.container.anvil.editable()
    }

    pub fn in_menu(&self) -> bool {
        if self.screen.is_menu() || (self.screen.is_options() && self.options_parent.is_menu()) {
            return true;
        }
        if self.screen == Screen::Dialog && self.dialog.parent().is_menu() {
            return true;
        }
        #[cfg(feature = "hud_editor")]
        if self.screen == Screen::HudEditor && self.hud_state.over_menu {
            return true;
        }
        #[cfg(feature = "click_gui")]
        {
            self.screen == Screen::ClickGui && self.clickgui_parent.is_menu()
        }
        #[cfg(not(feature = "click_gui"))]
        {
            false
        }
    }
}

#[derive(Default)]
pub struct Snapshot {
    pub action_bar: Option<Vec<crate::text::Span>>,
    pub title_text: Option<Vec<crate::text::Span>>,
    pub subtitle_text: Option<Vec<crate::text::Span>>,
    pub title_times: Option<(Option<u32>, Option<u32>, Option<u32>)>,
    pub titles_clear: Option<bool>,
    pub menu_slots: Vec<SlotStack>,
    pub attack_strength: f32,
    pub attack_delay: f32,
    pub targeted_entity: bool,
    pub hotbar: std::sync::Arc<[SlotStack]>,
    pub carried: SlotStack,
    pub selected: u8,
    pub gamemode: Gamemode,
    pub health: f32,
    pub food: u32,
    pub saturation: f32,
    pub health_display: crate::session::HealthDisplay,
    pub hardcore: bool,
    pub health_epoch: u32,
    pub jump_charge: Option<f32>,
    pub air_supply: i32,
    pub eye_in_water: bool,
    pub chat_incoming: Vec<crate::session::ChatEntry>,
    pub toast_incoming: Vec<toast::ToastEvent>,
    pub chat_error: Option<String>,
    pub enforces_secure_chat: bool,
    pub chat_signing: bool,
    pub container_title: std::sync::Arc<[crate::text::Span]>,
    pub container_id: i32,
    pub container_data: crate::session::ContainerData,
    pub enchant_clues: [Option<Box<str>>; 3],
    pub merchant: Option<std::sync::Arc<crate::session::MerchantOffers>>,
    pub merchant_future_xp: u32,
    pub stonecutter_recipes: std::sync::Arc<[crate::session::StonecutterRecipe]>,
    pub active_effects: Vec<crate::play::mob_effects::MobEffectInstance>,
    pub xp_progress: f32,
    pub xp_level: u32,
    pub xp_recent_gain: bool,
    pub waypoints: std::sync::Arc<Vec<crate::session::WaypointInfo>>,
    pub eye_pos: [f32; 3],
    pub camera_yaw: f32,
    pub camera_pitch: f32,
    pub sleep_timer: u32,
    pub tab_list: std::sync::Arc<crate::client::tablist::TabList>,
    pub sidebar: std::sync::Arc<crate::client::tablist::Sidebar>,
    pub boss_bars: std::sync::Arc<Vec<crate::client::bossbar::BossBar>>,
    #[allow(
        dead_code,
        reason = "published session state the sleep screen does not read yet"
    )]
    pub sleeping: bool,
}

impl Snapshot {
    pub fn is_creative(&self) -> bool {
        self.gamemode == Gamemode::Creative
    }

    pub fn hotbar(&self, i: usize) -> &SlotStack {
        static EMPTY: std::sync::LazyLock<SlotStack> = std::sync::LazyLock::new(SlotStack::default);
        self.hotbar.get(i).unwrap_or(&EMPTY)
    }

    pub fn offhand(&self) -> &SlotStack {
        static EMPTY: std::sync::LazyLock<SlotStack> = std::sync::LazyLock::new(SlotStack::default);
        self.hotbar.get(9).unwrap_or(&EMPTY)
    }

    pub fn slot(&self, i: usize) -> &SlotStack {
        static EMPTY: std::sync::LazyLock<SlotStack> = std::sync::LazyLock::new(SlotStack::default);
        self.menu_slots.get(i).unwrap_or(&EMPTY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &'static str) -> SlotStack {
        SlotStack {
            item,
            count: 1,
            ..Default::default()
        }
    }

    #[test]
    fn hotbar_and_offhand_ignore_menu_slots_shape() {
        let mut menu_slots = vec![SlotStack::default(); 46];
        menu_slots[45] = stack("stone");
        let mut hotbar = vec![SlotStack::default(); 10];
        hotbar[3] = stack("diamond_pickaxe");
        hotbar[9] = stack("shield");

        let snap = Snapshot {
            menu_slots,
            hotbar: hotbar.into(),
            ..Default::default()
        };

        assert_eq!(snap.hotbar(3).item, "diamond_pickaxe");
        assert_eq!(snap.offhand().item, "shield");
        assert!(snap.hotbar(0).is_empty());
    }
}

pub struct ScreenCtx<'a> {
    pub input: &'a GuiInput,
    pub vw: f32,
    pub vh: f32,
}

impl ScreenCtx<'_> {
    pub fn mouse(&self) -> Option<Vec2> {
        self.input.mouse
    }

    pub fn hovering(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        match self.input.mouse {
            Some(m) => m.x >= x && m.x < x + w && m.y >= y && m.y < y + h,
            None => false,
        }
    }
}
