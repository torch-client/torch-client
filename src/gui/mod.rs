use crate::renderer::clouds::CloudStatus;

pub mod accountlist;
pub mod advancements;
#[cfg(feature = "asset_download")]
pub mod assetdownload;
pub mod atlas;
pub mod atlas_writes;
pub mod banner_icons;
#[cfg(feature = "online_mode")]
pub mod bannotice;
pub mod book;
pub mod chat;
pub mod clickevent;
#[cfg(feature = "click_gui")]
pub mod clickgui;
pub mod command_block;
pub mod command_history;
pub mod configphase;
pub mod confirmlink;
pub mod container;
pub mod creative;
pub mod credits;
pub mod death;
pub mod dialog;
pub mod disconnected;
pub mod draw;
#[cfg(explore_modrinth)]
pub mod explore_packs;
pub mod focus;
pub mod gamemode_switcher;
pub mod health;
pub mod hud;
#[cfg(feature = "hud_editor")]
pub mod hud_editor;
pub mod hud_layout;
pub mod keybinds;
#[cfg(feature = "asset_download")]
pub mod language;
#[cfg(lua_mods)]
pub mod lua_list;
pub mod menu;
#[cfg(lua_mods)]
pub mod mod_images;
pub mod multiplayer;
#[cfg(feature = "audio")]
pub mod now_playing;
pub mod op_blocks;
pub mod options;
#[cfg(any(resource_packs, feature = "shader_support"))]
pub mod packdrop;
pub mod painter;
pub mod pause;
pub mod ping;
#[cfg(feature = "skins")]
pub mod player_faces;
pub mod pool;
pub mod profile;
pub mod puppet;
pub mod recipe_book;
pub mod recipe_book_ghost;
pub mod render;
#[cfg(resource_packs)]
pub mod resourcepacks;
pub mod screen_effects;
pub mod screens;
#[cfg(self_update)]
pub mod selfupdate;
pub mod serverlist;
pub mod serverpack;
#[cfg(feature = "shader_support")]
pub mod shaderpacks;
pub mod sign_edit;
pub mod signedchat;
#[cfg(eagler_skins)]
pub mod skinpick;
pub mod sleep;
pub mod slots;
pub mod spectator_menu;
pub mod stats;
#[cfg(feature = "audio")]
pub mod subtitles;
pub mod suggestions;
pub mod tablist;
pub mod title;
pub mod toast;
pub mod tooltip;
#[cfg(feature = "tutorial")]
pub mod tutorial;
pub mod widgets;
pub mod winscreen;

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
    #[cfg(self_update)]
    SelfUpdate,
    Accounts,
    EditProfile,
    Credits,
    #[cfg(feature = "asset_download")]
    Language,
    #[cfg(feature = "online_mode")]
    AddAccount,
    #[cfg(feature = "online_mode")]
    MicrosoftLogin,
    #[cfg(feature = "online_mode")]
    BanNotice,
    Multiplayer,
    ManageServer,
    DirectConnect,
    Disconnected,
    Pause,
    Options,
    VideoSettings,
    Controls,
    GameSettings,
    ManageStorage,
    #[cfg(feature = "audio")]
    AudioSettings,
    #[cfg(resource_packs)]
    ResourcePacks,
    #[cfg(explore_modrinth)]
    ExplorePacks,
    #[cfg(feature = "shader_support")]
    ShaderPacks,
    #[cfg(feature = "shader_support")]
    ShaderOptions,
    Dialog,
    Advancements,
    Statistics,

    Inventory,
    Creative,

    Container(ContainerKind),

    CommandBlock,
    StructureBlock,
    JigsawBlock,
    TestBlock,
    TestInstanceBlock,
    SignEdit,
    BookView,
    BookEdit,
    BookSign,
    Chat,
    Sleep,
    Death,
    SignedChatPrompt,
    ServerPackPrompt,
    WinGame,
    CodeOfConduct,
    Reconfiguring,
    GameModeSwitcher,

    #[cfg(feature = "click_gui")]
    ClickGui,

    #[cfg(feature = "hud_editor")]
    HudEditor,

    #[cfg(lua_mods)]
    LuaList,
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
                | Screen::Credits
                | Screen::Disconnected
        ) {
            return true;
        }
        #[cfg(feature = "asset_download")]
        if matches!(self, Screen::AssetDownload | Screen::Language) {
            return true;
        }
        #[cfg(self_update)]
        if self == Screen::SelfUpdate {
            return true;
        }
        #[cfg(feature = "online_mode")]
        {
            matches!(
                self,
                Screen::AddAccount | Screen::MicrosoftLogin | Screen::BanNotice
            )
        }
        #[cfg(not(feature = "online_mode"))]
        {
            false
        }
    }

    pub fn is_local_overlay(self) -> bool {
        if matches!(
            self,
            Screen::Dialog | Screen::Advancements | Screen::Statistics | Screen::GameModeSwitcher
        ) {
            return true;
        }
        #[cfg(lua_mods)]
        if self == Screen::LuaList {
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
        #[cfg(explore_modrinth)]
        if self == Screen::ExplorePacks {
            return true;
        }
        matches!(
            self,
            Screen::Options
                | Screen::VideoSettings
                | Screen::Controls
                | Screen::GameSettings
                | Screen::ManageStorage
        )
    }

    pub fn chat_open(self) -> bool {
        matches!(self, Screen::Chat | Screen::Sleep)
    }

    pub fn is_op_block_editor(self) -> bool {
        matches!(
            self,
            Screen::StructureBlock
                | Screen::JigsawBlock
                | Screen::TestBlock
                | Screen::TestInstanceBlock
        )
    }

    pub fn tab_navigates(self) -> bool {
        if self.is_options() {
            return true;
        }
        #[cfg(feature = "asset_download")]
        if matches!(self, Screen::AssetDownload | Screen::Language) {
            return true;
        }
        #[cfg(self_update)]
        if self == Screen::SelfUpdate {
            return true;
        }
        #[cfg(feature = "online_mode")]
        if matches!(
            self,
            Screen::AddAccount | Screen::MicrosoftLogin | Screen::BanNotice
        ) {
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
                | Screen::Credits
                | Screen::Disconnected
                | Screen::Death
                | Screen::SignedChatPrompt
                | Screen::ServerPackPrompt
                | Screen::Dialog
                | Screen::Advancements
                | Screen::Statistics
                | Screen::CodeOfConduct
                | Screen::Reconfiguring
                | Screen::StructureBlock
                | Screen::JigsawBlock
                | Screen::TestBlock
                | Screen::TestInstanceBlock
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TouchMovement {
    Buttons,
    Joystick,
    #[default]
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

    pub fn has_stick(self) -> bool {
        !matches!(self, TouchMovement::Buttons)
    }
}

crate::util::named_cycle::named_cycle!(TouchMovement);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LodQuality {
    Lowest,
    Low,
    #[default]
    Medium,
    High,
    Extreme,
}

impl LodQuality {
    pub const ALL: [LodQuality; 5] = [
        LodQuality::Lowest,
        LodQuality::Low,
        LodQuality::Medium,
        LodQuality::High,
        LodQuality::Extreme,
    ];

    pub fn caption(self) -> &'static str {
        match self {
            LodQuality::Lowest => "Lowest",
            LodQuality::Low => "Low",
            LodQuality::Medium => "Medium",
            LodQuality::High => "High",
            LodQuality::Extreme => "Extreme",
        }
    }

    pub fn serialized_name(self) -> &'static str {
        match self {
            LodQuality::Lowest => "lowest",
            LodQuality::Low => "low",
            LodQuality::Medium => "medium",
            LodQuality::High => "high",
            LodQuality::Extreme => "extreme",
        }
    }

    pub fn curve(self) -> (f64, i32) {
        match self {
            LodQuality::Lowest => (2.0, 4),
            LodQuality::Low => (2.0, 8),
            LodQuality::Medium => (2.0, 12),
            LodQuality::High => (2.2, 16),
            LodQuality::Extreme => (2.2, 20),
        }
    }
}

crate::util::named_cycle::named_cycle!(LodQuality);

pub const LOD_DISTANCE_DEFAULT: u32 = 256;

#[derive(Clone, Copy, Debug)]
pub struct GuiOptions {
    pub gui_scale: u32,
    pub fov: u32,
    pub fov_effects: u32,
    pub screen_effects: u32,
    pub damage_tilt: u32,
    pub clouds: crate::renderer::clouds::CloudStatus,
    pub particles: crate::util::particles::ParticleStatus,
    pub render_distance: u32,
    pub gamma: u32,
    pub antialiasing: bool,
    pub smooth_lighting: bool,
    pub lighting_enabled: bool,
    pub max_fps: u32,
    pub vsync: bool,
    pub inactivity_fps_limit: InactivityFpsLimit,
    pub auto_jump: bool,
    pub toggle_sprint: bool,
    pub main_hand_left: bool,
    #[cfg(feature = "skins")]
    pub skin_parts: u8,
    pub fps_counter: bool,
    pub shader_quality: crate::renderer::terrain::ShaderQuality,
    pub shaders_enabled: bool,
    pub player_shadows: bool,
    pub entity_shadows: bool,
    pub touch_movement: TouchMovement,
    pub lod_distance: u32,
    pub lod_quality: LodQuality,
    pub allow_signed_chat: bool,
    pub toast_advancement: bool,
    pub toast_recipe: bool,
    pub toast_system: bool,
    pub toast_time: u32,
    #[cfg(feature = "audio")]
    pub volumes: [u32; crate::audio::category::SoundCategory::COUNT],
    #[cfg(feature = "audio")]
    pub show_subtitles: bool,
    #[cfg(feature = "audio")]
    pub music_frequency: crate::audio::MusicFrequency,
    #[cfg(feature = "audio")]
    pub music_toast: now_playing::MusicToast,
    #[cfg(feature = "audio")]
    pub sound_device: Option<&'static str>,
}

#[cfg(feature = "audio")]
pub const VOLUME_DEFAULT: u32 = 100;

pub const TOAST_TIME_DEFAULT: u32 = 25;

pub const TOAST_TIME_MIN: i32 = 5;
pub const TOAST_TIME_MAX: i32 = 100;

pub const AUTO_JUMP_DEFAULT: bool = cfg!(feature = "mobile_ui");

pub const TOGGLE_SPRINT_DEFAULT: bool = cfg!(target_arch = "wasm32");

pub const FPS_COUNTER_DEFAULT: bool = true;

pub const MAX_FPS_MIN: u32 = 10;
pub const MAX_FPS_UNLIMITED: u32 = 260;
pub const MAX_FPS_DEFAULT: u32 = if cfg!(feature = "mobile_ui") { 30 } else { 60 };

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InactivityFpsLimit {
    #[default]
    Minimized,
    Afk,
}

impl InactivityFpsLimit {
    fn name(self) -> &'static str {
        match self {
            InactivityFpsLimit::Minimized => "minimized",
            InactivityFpsLimit::Afk => "afk",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        [InactivityFpsLimit::Minimized, InactivityFpsLimit::Afk]
            .into_iter()
            .find(|limit| limit.name() == name)
    }
}

pub const VSYNC_DEFAULT: bool = !cfg!(target_os = "android");

pub const RENDER_DISTANCE_DEFAULT: u32 = if cfg!(feature = "mobile_ui") { 2 } else { 8 };

pub const SMOOTH_LIGHTING_DEFAULT: bool = !cfg!(feature = "mobile_ui");

pub const LIGHTING_ENABLED_DEFAULT: bool = !cfg!(feature = "mobile_ui");

pub(crate) const GAMMA_DEFAULT: u32 = 50;

pub const GAMMA_MAX: u32 = 150;

pub const FOV_EFFECTS_DEFAULT: u32 = 100;

pub const SCREEN_EFFECTS_DEFAULT: u32 = 100;

pub const DAMAGE_TILT_DEFAULT: u32 = 100;

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
        let chosen = load_options().map(chosen_from_stored).unwrap_or_default();
        let options = GuiOptions::from_stored(Some(&serde_json::Value::Object(chosen.clone())));
        *CHOSEN.lock().unwrap_or_else(|e| e.into_inner()) = chosen;
        options
    }
}

impl GuiOptions {
    pub fn defaults() -> Self {
        GuiOptions::from_stored(None)
    }

    fn from_stored(stored: Option<&serde_json::Value>) -> Self {
        let value = |key: &str| stored.and_then(|json| json.get(key).cloned());
        let effect_scale = |key: &str, default: u32| {
            value(key)
                .and_then(|v| v.as_u64())
                .map_or(default, |v| v as u32)
                .min(default)
        };
        GuiOptions {
            gui_scale: value("guiScale")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(0),
            fov: value("fov")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(crate::renderer::systems::VANILLA_FOV_DEGREES as u32),
            fov_effects: effect_scale("fovEffectScale", FOV_EFFECTS_DEFAULT),
            screen_effects: effect_scale("screenEffectScale", SCREEN_EFFECTS_DEFAULT),
            damage_tilt: effect_scale("damageTiltStrength", DAMAGE_TILT_DEFAULT),
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
            inactivity_fps_limit: value("inactivityFpsLimit")
                .and_then(|v| v.as_str().and_then(InactivityFpsLimit::from_name))
                .unwrap_or_default(),
            auto_jump: value("autoJump")
                .and_then(|v| v.as_bool())
                .unwrap_or(AUTO_JUMP_DEFAULT),
            toggle_sprint: value("toggleSprint")
                .and_then(|v| v.as_bool())
                .unwrap_or(TOGGLE_SPRINT_DEFAULT),
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
            entity_shadows: value("entityShadows")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            touch_movement: value("touchMovement")
                .and_then(|v| v.as_str().and_then(TouchMovement::from_serialized_name))
                .unwrap_or_default(),
            lod_distance: value("lodDistance")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(LOD_DISTANCE_DEFAULT),
            lod_quality: value("lodQuality")
                .and_then(|v| v.as_str().and_then(LodQuality::from_serialized_name))
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
            #[cfg(feature = "audio")]
            show_subtitles: value("showSubtitles")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            #[cfg(feature = "audio")]
            music_frequency: value("musicFrequency")
                .and_then(|v| {
                    v.as_str()
                        .and_then(crate::audio::MusicFrequency::from_serialized_name)
                })
                .unwrap_or_default(),
            #[cfg(feature = "audio")]
            music_toast: value("musicToast")
                .and_then(|v| v.as_str().and_then(now_playing::MusicToast::from_name))
                .unwrap_or_default(),
            #[cfg(feature = "audio")]
            sound_device: value("soundDevice").and_then(|v| {
                v.as_str()
                    .filter(|name| !name.is_empty())
                    .map(|name| crate::util::intern(name.to_owned()))
            }),
        }
    }
}

fn load_options() -> Option<serde_json::Value> {
    let text = crate::platform::storage::OPTIONS.load()?;
    serde_json::from_str(&text).ok()
}

const OPTIONS_FORMAT_KEY: &str = "optionsFormat";
const OPTIONS_FORMAT: u64 = 2;

const FORMAT_1_DEFAULTS: &[(&str, bool)] = &[("toggleSprint", true)];

static CHOSEN: std::sync::LazyLock<std::sync::Mutex<serde_json::Map<String, serde_json::Value>>> =
    std::sync::LazyLock::new(Default::default);

fn chosen_from_stored(stored: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let serde_json::Value::Object(mut map) = stored else {
        return serde_json::Map::new();
    };
    let format = map
        .remove(OPTIONS_FORMAT_KEY)
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    if format >= OPTIONS_FORMAT {
        return map;
    }
    let defaults = options_json(&GuiOptions::defaults());
    map.retain(|key, value| {
        let old_default = FORMAT_1_DEFAULTS
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| serde_json::Value::Bool(*v))
            .or_else(|| defaults.get(key.as_str()).cloned());
        old_default.as_ref() != Some(value)
    });
    map
}

pub fn reset_options(options: &mut GuiOptions, is_key: impl Fn(&str) -> bool) {
    let defaults = options_json(&GuiOptions::defaults());
    let mut json = options_json(options);
    if let (Some(map), Some(defaults)) = (json.as_object_mut(), defaults.as_object()) {
        for (key, value) in map.iter_mut() {
            if is_key(key) {
                if let Some(default) = defaults.get(key) {
                    *value = default.clone();
                }
            }
        }
    }
    *options = GuiOptions::from_stored(Some(&json));
    CHOSEN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|key, _| !is_key(key));
}

pub fn save_options(options: &GuiOptions) {
    let current = options_json(options);
    let defaults = options_json(&GuiOptions::defaults());
    let mut chosen = CHOSEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(current) = current.as_object() {
        for (key, value) in current {
            if chosen.contains_key(key) || defaults.get(key) != Some(value) {
                chosen.insert(key.clone(), value.clone());
            }
        }
    }
    let mut file = chosen.clone();
    file.insert(OPTIONS_FORMAT_KEY.to_owned(), OPTIONS_FORMAT.into());
    let text = serde_json::Value::Object(file).to_string();
    if widgets::slider_dragging() {
        *PENDING_OPTIONS.lock().unwrap_or_else(|e| e.into_inner()) = Some(text);
        return;
    }
    PENDING_OPTIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    crate::platform::storage::OPTIONS.store(&text);
}

static PENDING_OPTIONS: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub(in crate::gui) fn flush_pending_options() {
    let pending = PENDING_OPTIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(text) = pending {
        crate::platform::storage::OPTIONS.store(&text);
    }
}

fn options_json(options: &GuiOptions) -> serde_json::Value {
    let mut json = serde_json::json!({
        "guiScale": options.gui_scale,
        "fov": options.fov,
        "fovEffectScale": options.fov_effects,
        "screenEffectScale": options.screen_effects,
        "damageTiltStrength": options.damage_tilt,
        "renderClouds": options.clouds.serialized_name(),
        "particles": options.particles.serialized_name(),
        "renderDistance": options.render_distance,
        "gamma": options.gamma,
        "antialiasing": options.antialiasing,
        "ao": options.smooth_lighting,
        "lightingEnabled": options.lighting_enabled,
        "maxFps": options.max_fps,
        "vsync": options.vsync,
        "inactivityFpsLimit": options.inactivity_fps_limit.name(),
        "autoJump": options.auto_jump,
        "toggleSprint": options.toggle_sprint,
        "mainHand": if options.main_hand_left { "left" } else { "right" },
        "fpsCounter": options.fps_counter,
        "shaders": options.shaders_enabled,
        "shaderQuality": options.shader_quality.serialized_name(),
        "playerShadows": options.player_shadows,
        "entityShadows": options.entity_shadows,
        "touchMovement": options.touch_movement.serialized_name(),
        "lodDistance": options.lod_distance,
        "lodQuality": options.lod_quality.serialized_name(),
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
        map.insert(
            "showSubtitles".to_owned(),
            serde_json::json!(options.show_subtitles),
        );
        map.insert(
            "musicFrequency".to_owned(),
            serde_json::json!(options.music_frequency.serialized_name()),
        );
        map.insert(
            "musicToast".to_owned(),
            serde_json::json!(options.music_toast.serialized_name()),
        );
        map.insert(
            "soundDevice".to_owned(),
            serde_json::json!(options.sound_device.unwrap_or("")),
        );
    }
    json
}

#[derive(Resource, Default)]
pub struct GuiState {
    pub screen: Screen,
    pub creative: creative::CreativeState,
    pub puppet: puppet::Worn,
    pub chat: chat::ChatState,
    pub sign_edit: sign_edit::SignEditState,
    pub command_block: command_block::CommandBlockState,
    pub op_blocks: op_blocks::OpBlocksState,
    pub book: book::BookState,
    pub dialog: dialog::DialogState,
    pub advanced_tooltips: bool,
    pub tab_list_held: bool,
    pub tablist: tablist::TabListState,
    pub hearts: health::HeartAnim,
    pub screen_effects: screen_effects::ScreenEffects,
    pub chunk_borders: bool,
    pub hide_gui: bool,
    pub slots: slots::SlotState,
    pub container: container::ContainerUi,
    pub recipe_book: recipe_book::RecipeBookUi,
    pub ghost: recipe_book_ghost::GhostSlots,
    pub book_requests: Vec<crate::client::recipes::BookRequest>,
    pub book_clock: Option<f32>,
    #[cfg(feature = "click_gui")]
    pub clickgui: clickgui::ClickGuiState,
    #[cfg(feature = "click_gui")]
    pub clickgui_parent: Screen,
    #[cfg(lua_mods)]
    pub lua_list: lua_list::LuaListState,
    pub hud_state: hud_layout::HudState,
    #[cfg(feature = "tutorial")]
    pub tutorial: tutorial::Tutorial,
    pub options: GuiOptions,
    pub keybinds: keybinds::KeyBinds,
    pub controls_scroll: f32,
    pub controls_scroll_drag: Option<f32>,
    pub nav: Option<Screen>,
    pub suppress_click: bool,
    pub death: death::DeathState,
    pub win: winscreen::WinState,
    pub config_phase: configphase::State,
    pub advancements: advancements::AdvancementsUi,
    pub stats: stats::StatsUi,
    pub credits_parent: Screen,
    pub gamemode_switcher: gamemode_switcher::SwitcherState,
    pub quit: bool,
    pub disconnect: bool,
    pub menu: multiplayer::MenuState,
    pub profile: profile::ProfileState,
    pub video_scroll: f32,
    pub video_scroll_drag: Option<f32>,
    pub game_scroll: f32,
    pub game_scroll_drag: Option<f32>,
    #[cfg(feature = "asset_download")]
    pub confirm_clear_assets: bool,
    #[cfg(feature = "asset_download")]
    pub storage_nothing_missing: bool,
    #[cfg(feature = "asset_download")]
    pub language: language::State,
    #[cfg(feature = "shader_support")]
    pub shaderpacks: shaderpacks::State,
    #[cfg(resource_packs)]
    pub resourcepacks: resourcepacks::State,
    #[cfg(explore_modrinth)]
    pub explore_packs: explore_packs::State,
    pub options_parent: Screen,
    pub server_pack_parent: Screen,
    pub disconnect_reason: Option<Vec<crate::text::Span>>,
    pub connect: Option<String>,
    pub hud: hud::HudInfo,
    pub hud_overlays: hud::HudOverlays,
    pub toasts: toast::Toasts,
    pub spectator_menu: spectator_menu::SpectatorMenuState,
    #[cfg(feature = "audio")]
    pub subtitles: subtitles::Subtitles,
    #[cfg(feature = "mobile_ui")]
    pub mobile: crate::mobile::MobileUi,
}

impl GuiState {
    pub fn anvil_typing(&self) -> bool {
        self.screen == Screen::Container(ContainerKind::Anvil) && self.container.anvil.editable()
    }

    pub fn screen_is_menu(&self) -> bool {
        if self.screen == Screen::Credits {
            return self.credits_parent.is_menu();
        }
        self.screen.is_menu()
    }

    pub fn in_menu(&self) -> bool {
        if self.screen_is_menu() || (self.screen.is_options() && self.options_parent.is_menu()) {
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
    pub action_bar: Option<(Vec<crate::text::Span>, bool)>,
    pub title_text: Option<Vec<crate::text::Span>>,
    pub subtitle_text: Option<Vec<crate::text::Span>>,
    pub title_times: Option<(Option<u32>, Option<u32>, Option<u32>)>,
    pub titles_clear: Option<bool>,
    pub menu_slots: std::sync::Arc<[SlotStack]>,
    pub attack_strength: f32,
    pub attack_delay: f32,
    pub targeted_entity: bool,
    pub hotbar: std::sync::Arc<[SlotStack]>,
    pub armor: std::sync::Arc<[SlotStack]>,
    pub carried: SlotStack,
    pub creative_cursor: Option<crate::session::CreativeAnswer>,
    pub selected: u8,
    pub gamemode: Gamemode,
    pub health: f32,
    pub food: u32,
    pub saturation: f32,
    pub health_display: crate::session::HealthDisplay,
    pub hardcore: bool,
    pub health_epoch: u32,
    pub jump_charge: Option<f32>,
    pub jump_cooldown: bool,
    pub air_supply: i32,
    pub eye_in_water: bool,
    pub chat_incoming: Vec<crate::session::ChatEntry>,
    pub toast_incoming: Vec<toast::ToastEvent>,
    pub chat_error: Option<String>,
    pub container_title: std::sync::Arc<[crate::text::Span]>,
    pub container_id: i32,
    pub container_data: crate::session::ContainerData,
    pub enchant_clues: [Option<Box<str>>; 3],
    pub merchant: Option<std::sync::Arc<crate::session::MerchantOffers>>,
    pub merchant_future_xp: u32,
    pub stonecutter_recipes: std::sync::Arc<[crate::session::StonecutterRecipe]>,
    pub advancements: Option<std::sync::Arc<crate::client::advancements::AdvancementTree>>,
    pub advancement_tab: Option<u32>,
    pub tick_rate: crate::session::TickRate,
    pub active_effects: Vec<crate::play::mob_effects::MobEffectInstance>,
    pub xp_progress: f32,
    pub xp_level: u32,
    pub xp_recent_gain: bool,
    pub waypoints: std::sync::Arc<Vec<crate::session::WaypointInfo>>,
    pub eye_pos: [f64; 3],
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
    pub recipe_book: Option<std::sync::Arc<crate::client::recipes::RecipeBook>>,
    pub recipe_ghost: Option<crate::client::recipes::ResolvedDisplay>,
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
            menu_slots: menu_slots.into(),
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

    pub fn with_input<'b>(&self, input: &'b GuiInput) -> ScreenCtx<'b> {
        ScreenCtx {
            input,
            vw: self.vw,
            vh: self.vh,
        }
    }

    pub fn hovering(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        match self.input.mouse {
            Some(m) => m.x >= x && m.x < x + w && m.y >= y && m.y < y + h,
            None => false,
        }
    }
}
