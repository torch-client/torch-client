use std::ops::Range;

use super::auto_mace::{AimPoint, MaceSlot};
use super::auto_mine::{FluidRule, InvFull, LowHealth, MineMode, ReachMode};
use super::auto_totem::TotemMode;
use super::crystal_aura::CrystalSwitch;
use super::esp::Shape;
use super::extra_hud::{Durability, Shown};
use super::flight::AntiKick;
use super::fullbright::FullbrightMode;
use super::list::BitList;
use super::nametags::{Health, Sizing};
use super::scaffold::{BlockFilter, ScaffoldMode, Tower};
use crate::gui::keybinds::Bound;

#[derive(Clone, Copy)]
pub struct SettingDef {
    pub name: &'static str,
    pub tip: &'static str,
    pub kind: Kind,
    pub show: Show,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(not(lua_mods), allow(dead_code))]
pub enum Show {
    Always,
    On(u8),
    Is(u8, u32),
    Any(&'static [Show]),
    All(&'static [Show]),
}

impl Show {
    pub const fn is(slot: u16, options: &[u8]) -> Show {
        let mut mask = 0u32;
        let mut i = 0;
        while i < options.len() {
            mask |= 1 << options[i];
            i += 1;
        }
        Show::Is(offset_of(slot), mask)
    }
}

const fn offset_of(slot: u16) -> u8 {
    let mut i = 0;
    while i < COUNT {
        if slot >= BASE[i] && slot < BASE[i + 1] {
            return (slot - BASE[i]) as u8;
        }
        i += 1;
    }
    panic!("not a built-in setting slot")
}

#[derive(Clone, Copy)]
pub enum Kind {
    Toggle {
        on: bool,
    },
    Slider {
        min: f32,
        max: f32,
        value: f32,
        decimals: u8,
    },
    Range {
        min: f32,
        max: f32,
        low: f32,
        high: f32,
        decimals: u8,
        log: bool,
    },
    Enum {
        options: &'static [&'static str],
        index: u8,
    },
    List {
        list: &'static BitList,
    },
    Text {
        max_len: usize,
        default: &'static str,
        expand: bool,
        hint: &'static str,
    },
    #[cfg_attr(not(lua_mods), allow(dead_code))]
    Key {
        default: Bound,
    },
    Button {
        text: &'static str,
        key: &'static str,
    },
}

pub trait Options: Copy + 'static {
    const NAMES: &'static [&'static str];
    fn from_index(i: u8) -> Self;
}

macro_rules! options {
    (
        $(#[$m:meta])*
        $vis:vis enum $name:ident { $($(#[$vm:meta])* $v:ident = $label:literal),* $(,)? }
    ) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(u8)]
        $vis enum $name { $($(#[$vm])* $v),* }

        impl $crate::modules::registry::Options for $name {
            const NAMES: &'static [&'static str] = &[$($label),*];
            fn from_index(i: u8) -> Self {
                const ALL: &[$name] = &[$($name::$v),*];
                ALL[(i as usize).min(ALL.len() - 1)]
            }
        }
    };
}
pub(crate) use options;

pub mod handle {
    use std::marker::PhantomData;

    #[derive(Clone, Copy)]
    pub struct Toggle(pub(in crate::modules) u16);
    #[derive(Clone, Copy)]
    pub struct Slider(pub(in crate::modules) u16);
    #[derive(Clone, Copy)]
    pub struct Range(pub(in crate::modules) u16);
    #[derive(Clone, Copy)]
    pub struct Text(pub(in crate::modules) u16);
    #[derive(Clone, Copy)]
    pub struct Enum<T>(pub(in crate::modules) u16, PhantomData<T>);

    impl<T> Enum<T> {
        pub(in crate::modules) const fn new(slot: u16) -> Self {
            Enum(slot, PhantomData)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u8)]
pub enum Mode {
    Legit = 0,
    Normal = 1,
    Blatant = 2,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Legit, Mode::Normal, Mode::Blatant];
    pub const NAMES: [&'static str; 3] = ["Legit", "Normal", "Blatant"];

    pub fn step(self, by: i8) -> Mode {
        let i = (self as i8 + by).clamp(0, Self::ALL.len() as i8 - 1);
        Self::ALL[i as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Combat,
    Render,
    World,
    Movement,
    Misc,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Combat,
        Category::Render,
        Category::World,
        Category::Movement,
        Category::Misc,
    ];

    pub fn name(self) -> &'static str {
        ["Combat", "Render", "World", "Movement", "Misc"][self as usize]
    }

    pub fn modules(self) -> impl Iterator<Item = &'static ModuleDef> {
        modules().filter(move |m| m.category == self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModuleIx(pub u16);

impl From<Id> for ModuleIx {
    #[inline(always)]
    fn from(id: Id) -> ModuleIx {
        ModuleIx(id as u16)
    }
}

#[derive(Clone, Copy)]
pub struct ModuleDef {
    pub id: ModuleIx,
    pub name: &'static str,
    pub desc: &'static str,
    pub category: Category,
    pub mode: Mode,
    first: u16,
    len: u16,
    #[cfg(lua_mods)]
    own: &'static [SettingDef],
}

impl ModuleDef {
    pub fn slots(&self) -> Range<usize> {
        self.first as usize..(self.first + self.len) as usize
    }

    pub fn settings(&self) -> &'static [SettingDef] {
        #[cfg(lua_mods)]
        if self.id.0 as usize >= COUNT {
            return self.own;
        }
        &SETTINGS[self.slots()]
    }
}

pub fn module(id: impl Into<ModuleIx>) -> &'static ModuleDef {
    let i = id.into().0 as usize;
    #[cfg(lua_mods)]
    if i >= COUNT {
        return &scripts().modules[i - COUNT];
    }
    &MODULES[i]
}

pub fn modules() -> impl Iterator<Item = &'static ModuleDef> {
    #[cfg(lua_mods)]
    let extra = scripts().modules;
    #[cfg(not(lua_mods))]
    let extra: &'static [ModuleDef] = &[];
    MODULES.iter().chain(extra)
}

pub fn module_count() -> usize {
    #[cfg(lua_mods)]
    return COUNT + scripts().modules.len();
    #[cfg(not(lua_mods))]
    COUNT
}

pub fn setting(slot: usize) -> Option<&'static SettingDef> {
    if slot < SETTING_COUNT {
        return Some(&SETTINGS[slot]);
    }
    #[cfg(lua_mods)]
    return scripts().settings.get(slot - SETTING_COUNT);
    #[cfg(not(lua_mods))]
    None
}

#[cfg(lua_mods)]
pub(super) struct Extra {
    pub name: &'static str,
    pub desc: &'static str,
    pub category: Category,
    pub mode: Mode,
    pub settings: Vec<SettingDef>,
}

#[cfg(lua_mods)]
pub(super) struct Scripts {
    pub modules: &'static [ModuleDef],
    pub settings: &'static [SettingDef],
}

#[cfg(lua_mods)]
static SCRIPTS: std::sync::atomic::AtomicPtr<Scripts> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(lua_mods)]
static NO_SCRIPTS: Scripts = Scripts {
    modules: &[],
    settings: &[],
};

#[cfg(lua_mods)]
static GENERATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(lua_mods)]
pub(super) fn scripts() -> &'static Scripts {
    let table = SCRIPTS.load(std::sync::atomic::Ordering::Acquire);
    unsafe { table.as_ref() }.unwrap_or(&NO_SCRIPTS)
}

#[cfg(lua_mods)]
pub fn generation() -> u32 {
    GENERATION.load(std::sync::atomic::Ordering::Acquire)
}

#[cfg(lua_mods)]
pub(super) fn install(extra: Vec<Extra>) {
    let total: usize = extra.iter().map(|e| e.settings.len()).sum();
    assert!(
        COUNT + extra.len() <= u16::MAX as usize,
        "script modules overflow ModuleIx"
    );
    assert!(
        SETTING_COUNT + total <= u16::MAX as usize,
        "script settings overflow a slot"
    );

    let settings: &'static [SettingDef] = extra
        .iter()
        .flat_map(|e| e.settings.iter().copied())
        .collect::<Vec<_>>()
        .leak();
    let mut modules = Vec::with_capacity(extra.len());
    let mut at = 0;
    for (k, e) in extra.iter().enumerate() {
        let len = e.settings.len();
        modules.push(ModuleDef {
            id: ModuleIx((COUNT + k) as u16),
            name: e.name,
            desc: e.desc,
            category: e.category,
            mode: e.mode,
            first: (SETTING_COUNT + at) as u16,
            len: len as u16,
            own: &settings[at..at + len],
        });
        at += len;
    }
    let table = Box::into_raw(Box::new(Scripts {
        modules: modules.leak(),
        settings,
    }));
    SCRIPTS.store(table, std::sync::atomic::Ordering::Release);
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::Release);
}

macro_rules! one {
    ($x:tt) => {
        1
    };
}

macro_rules! kind {
    (Enum<$t:ty> { default: $d:expr }) => {
        Kind::Enum {
            options: <$t as Options>::NAMES,
            index: $d as u8,
        }
    };
    ($v:ident { $($b:tt)* }) => {
        Kind::$v { $($b)* }
    };
}

macro_rules! show {
    () => {
        Show::Always
    };
    ($e:expr) => {
        $e
    };
}

macro_rules! handles {
    ($id:ident; $n:expr;) => {};
    ($id:ident; $n:expr; $konst:ident Enum<$t:ty>; $($rest:tt)*) => {
        pub const $konst: handle::Enum<$t> = handle::Enum::new(slot(Id::$id, $n));
        handles!($id; $n + 1; $($rest)*);
    };
    ($id:ident; $n:expr; $konst:ident List; $($rest:tt)*) => {
        handles!($id; $n + 1; $($rest)*);
    };
    ($id:ident; $n:expr; $konst:ident Button; $($rest:tt)*) => {
        handles!($id; $n + 1; $($rest)*);
    };
    ($id:ident; $n:expr; $konst:ident $v:ident; $($rest:tt)*) => {
        pub const $konst: handle::$v = handle::$v(slot(Id::$id, $n));
        handles!($id; $n + 1; $($rest)*);
    };
}

macro_rules! modules {
    ($(
        $id:ident / $index:ident: $cat:ident, $mode:ident, $desc:literal {
            $($konst:ident: $sname:literal, $tip:literal, $v:ident $(<$t:ty>)? { $($body:tt)* } $(if $show:expr)?;)*
        }
    )*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(u8)]
        pub enum Id { $($id),* }

        pub const COUNT: usize = 0 $(+ one!($id))*;

        impl Id {
            #[cfg(test)]
            pub const ALL: [Id; COUNT] = [$(Id::$id),*];
        }

        pub static MODULES: [ModuleDef; COUNT] = [$(
            ModuleDef {
                id: ModuleIx(Id::$id as u16),
                name: stringify!($id),
                desc: $desc,
                category: Category::$cat,
                mode: Mode::$mode,
                first: BASE[Id::$id as usize],
                len: LENS[Id::$id as usize],
                #[cfg(lua_mods)]
                own: &[],
            },
        )*];

        const LENS: [u16; COUNT] = [$(0 $(+ one!($konst))*),*];

        pub static SETTINGS: [SettingDef; SETTING_COUNT] = [$($(
            SettingDef {
                name: $sname,
                tip: $tip,
                kind: kind!($v $(<$t>)? { $($body)* }),
                show: show!($($show)?),
            },
        )*)*];

        $(
            #[doc = concat!("Handles to ", stringify!($id), "'s settings.")]
            pub mod $index {
                #[allow(unused_imports)]
                use super::*;
                handles!($id; 0; $($konst $v $(<$t>)?;)*);
            }
        )*
    };
}

const BASE: [u16; COUNT + 1] = {
    let mut b = [0; COUNT + 1];
    let mut i = 0;
    while i < COUNT {
        b[i + 1] = b[i] + LENS[i];
        i += 1;
    }
    b
};

pub const SETTING_COUNT: usize = BASE[COUNT] as usize;

const fn slot(id: Id, n: u16) -> u16 {
    BASE[id as usize] + n
}

const SMART: Show = Show::is(auto_totem::MODE.0, &[TotemMode::Smart as u8]);

const SCAFFOLD_LEGIT: Show = Show::is(scaffold::MODE.0, &[ScaffoldMode::Legit as u8]);
const SCAFFOLD_BRIDGE: Show = Show::is(
    scaffold::MODE.0,
    &[
        ScaffoldMode::GodBridge as u8,
        ScaffoldMode::TellyBridge as u8,
    ],
);
const SCAFFOLD_TELLY: Show = Show::is(scaffold::MODE.0, &[ScaffoldMode::TellyBridge as u8]);
const SCAFFOLD_FILTERED: Show = Show::is(
    scaffold::BLOCKS.0,
    &[BlockFilter::Blacklist as u8, BlockFilter::Whitelist as u8],
);

const EXTRA_HUD_WARN: Show = Show::On(offset_of(extra_hud::WARNING.0));
const EXTRA_HUD_EMPTY: Show = Show::is(
    extra_hud::SHOWN.0,
    &[Shown::Always as u8, Shown::IfAny as u8],
);

const AUTO_MINE_AREA: Show = Show::is(auto_mine::MODE.0, &[MineMode::Area as u8]);
const AUTO_MINE_CORRIDOR: Show = Show::is(auto_mine::MODE.0, &[MineMode::Corridor as u8]);
const AUTO_MINE_DISCONNECT: Show =
    Show::is(auto_mine::LOW_HEALTH.0, &[LowHealth::Disconnect as u8]);

const TYPED_MAX: &[(u16, f32)] = &[(auto_mine::HEALTH.0, i16::MAX as f32)];

pub fn typed_max(slot: usize, max: f32) -> f32 {
    TYPED_MAX
        .iter()
        .find(|(s, _)| *s as usize == slot)
        .map_or(max, |(_, m)| *m)
}

const NAMETAGS_ARMOR: Show = Show::On(offset_of(nametags::ARMOR.0));

modules! {
    TriggerBot / trigger_bot: Combat, Normal,
        "Attacks whatever the crosshair is on, as fast as you could" {
        TARGETS: "Targets",
            "Which entity kinds count as a target, shared with AimAssist",
            List { list: &super::entities::LIST };
        DELAY: "Delay",
            "Milliseconds between swings, rolled fresh inside this range every time",
            Range { min: 0.0, max: 250.0, low: 45.0, high: 110.0, decimals: 0, log: false };
        WAIT_FOR_COOLDOWN: "Wait for cooldown",
            "Only swing once the attack indicator has fully recharged",
            Toggle { on: true };
        REQUIRE_WEAPON: "Require weapon",
            "Only swing while holding a sword, axe, mace or trident",
            Toggle { on: false };
    }

    AimAssist / aim_assist: Combat, Normal,
        "Pulls your own turn toward the target you are already turning at" {
        TARGETS: "Targets",
            "Which entity kinds may be aimed at, shared with TriggerBot",
            List { list: &super::entities::LIST };
        RANGE: "Range",
            "How far away in blocks a target may be",
            Slider { min: 1.0, max: 12.0, value: 4.5, decimals: 1 };
        FOV: "FOV",
            "Width in degrees of the cone ahead of you a target has to be inside",
            Slider { min: 5.0, max: 180.0, value: 45.0, decimals: 0 };
        STRENGTH: "Strength",
            "Percent of your own mouse movement added on top of it toward the target, so 100 turns twice as fast",
            Slider { min: 0.0, max: 300.0, value: 100.0, decimals: 0 };
        FRICTION: "Friction",
            "Percent of your own mouse movement swallowed when you drag off a target, full on it and none at the edge of the cone",
            Slider { min: 0.0, max: 90.0, value: 40.0, decimals: 0 };
        DEADZONE: "Deadzone",
            "Degrees short of the target the pull stops, so the last of the aim is always yours",
            Slider { min: 0.0, max: 5.0, value: 1.0, decimals: 1 };
        REACTION: "Reaction",
            "Milliseconds a newly acquired target is left alone for, rolled inside this range",
            Range { min: 0.0, max: 400.0, low: 100.0, high: 220.0, decimals: 0, log: false };
        IGNORE_WALLS: "Ignore walls",
            "Keep pulling toward a target that has stepped behind blocks",
            Toggle { on: false };
    }

    AutoMace / auto_mace: Combat, Normal,
        "Smashes the nearest target with your mace while you fall, swapping to it for the hit" {
        TARGETS: "Targets",
            "Which entity kinds may be hit, shared with TriggerBot and AimAssist",
            List { list: &super::entities::LIST };
        MIN_FALL: "Min fall",
            "Blocks fallen before it swings. The server only smashes past 1.5",
            Slider { min: 1.5, max: 20.0, value: 3.0, decimals: 1 };
        RANGE: "Range",
            "Blocks from your eye to the target's hitbox, where vanilla reach is 3",
            Slider { min: 1.0, max: 6.0, value: 3.0, decimals: 1 };
        FOV: "FOV",
            "Width in degrees of the cone around your view a target must be inside, 360 for anywhere",
            Slider { min: 30.0, max: 360.0, value: 360.0, decimals: 0 };
        COOLDOWN: "Cooldown",
            "Percent the held item's attack must have recharged. Over 90 lands a critical hit",
            Slider { min: 0.0, max: 100.0, value: 95.0, decimals: 0 };
        SILENT: "Silent",
            "Turn the look sent to the server, a tick before the hit, and leave the camera alone. Off turns the camera",
            Toggle { on: true };
        TURN: "Turn speed",
            "Degrees per second the aim may travel, rolled fresh inside this range for every target",
            Range { min: 90.0, max: 1800.0, low: 600.0, high: 1000.0, decimals: 0, log: false };
        AIM: "Aim point",
            "Where on the hitbox the aim goes. Nearest turns the least",
            Enum<AimPoint> { default: AimPoint::Body };
        MOVE_FIX: "Move fix",
            "While the sent look is off your camera, re-pick the walk keys so you keep heading the same way",
            Toggle { on: true };
        SLOT: "Mace slot",
            "Hotbar slot the mace is taken from. Auto keeps a held one, else takes the first",
            Enum<MaceSlot> { default: MaceSlot::Auto };
        SWAP_BACK: "Swap back",
            "Go back to the item you were holding once the hit is sent",
            Toggle { on: true };
        WALLS: "Through walls",
            "Hit a target with blocks in between, which no hand could aim at",
            Toggle { on: false };
    }

    AutoTotem / auto_totem: Combat, Normal,
        "Keeps a Totem of Undying in your offhand, always or when the next hit could kill" {
        MODE: "Mode",
            "Always refills the offhand whenever it has no totem. Smart waits until one of the checks below fires",
            Enum<TotemMode> { default: TotemMode::Always };
        HEALTH: "Health",
            "Smart: equip at or under this much health, absorption included. 2 is one heart",
            Slider { min: 0.0, max: 36.0, value: 10.0, decimals: 0 } if SMART;
        ELYTRA: "Elytra",
            "Smart: equip while gliding",
            Toggle { on: true } if SMART;
        FALL: "Fall",
            "Smart: equip when the landing below would deal at least your health in fall damage",
            Toggle { on: true } if SMART;
        EXPLOSION: "Explosion",
            "Smart: equip when crystals, lit TNT and swelling creepers in range could deal at least your health",
            Toggle { on: true } if SMART;
        DELAY: "Delay",
            "Milliseconds after a move before the next one, rolled fresh inside this range",
            Range { min: 0.0, max: 500.0, low: 0.0, high: 100.0, decimals: 0, log: false };
    }

    CrystalAura / crystal_aura: Combat, Blatant,
        "Places end crystals where they hurt a target most and breaks them" {
        TARGETS: "Targets",
            "Which entity kinds may be targeted, shared with TriggerBot and AimAssist",
            List { list: &super::entities::LIST };
        STRICT: "Strict",
            "Only clicks a hand could make: the look the server has is on the block or crystal, survival reach, nothing through walls. Off lifts all three",
            Toggle { on: true };
        TARGET_RANGE: "Target range",
            "Blocks from you a target may stand",
            Slider { min: 2.0, max: 16.0, value: 10.0, decimals: 0 };
        MIN_DAMAGE: "Min damage",
            "Least a crystal must deal the target, where 2 is one heart",
            Slider { min: 0.0, max: 36.0, value: 6.0, decimals: 1 };
        MAX_SELF: "Max self damage",
            "Most a crystal may deal you",
            Slider { min: 0.0, max: 36.0, value: 6.0, decimals: 1 };
        ANTI_SUICIDE: "Anti suicide",
            "Never place or break a crystal that would take all your health, absorption included",
            Toggle { on: true };
        PLACE: "Place",
            "Place crystals on obsidian and bedrock",
            Toggle { on: true };
        PLACE_DELAY: "Place delay",
            "Milliseconds between placements, rolled fresh inside this range",
            Range { min: 0.0, max: 1000.0, low: 100.0, high: 150.0, decimals: 0, log: false } if Show::On(offset_of(crystal_aura::PLACE.0));
        PLACE_RANGE: "Place range",
            "Blocks from your eye to the clicked block. Strict holds it to 4.5, survival block reach",
            Slider { min: 1.0, max: 6.0, value: 4.5, decimals: 1 } if Show::On(offset_of(crystal_aura::PLACE.0));
        BREAK: "Break",
            "Hit crystals that hurt a target enough",
            Toggle { on: true };
        BREAK_DELAY: "Break delay",
            "Milliseconds between hits, rolled fresh inside this range",
            Range { min: 0.0, max: 1000.0, low: 50.0, high: 100.0, decimals: 0, log: false } if Show::On(offset_of(crystal_aura::BREAK.0));
        BREAK_RANGE: "Break range",
            "Blocks from your eye to the crystal's hitbox. Strict holds it to 3, survival attack reach",
            Slider { min: 1.0, max: 6.0, value: 3.0, decimals: 1 } if Show::On(offset_of(crystal_aura::BREAK.0));
        FAST_BREAK: "Fast break",
            "Hit a crystal you placed as soon as it appears, without waiting out Break delay",
            Toggle { on: true } if Show::On(offset_of(crystal_aura::BREAK.0));
        ATTEMPTS: "Break attempts",
            "Hits on one crystal before giving up on it",
            Slider { min: 1.0, max: 5.0, value: 2.0, decimals: 0 } if Show::On(offset_of(crystal_aura::BREAK.0));
        FACE_PLACE: "Face place",
            "Drop Min damage to 1.5 against a target at or under Face health, or wearing a nearly broken piece",
            Toggle { on: true };
        FACE_HEALTH: "Face health",
            "Target health, absorption included, at or under which Face place starts",
            Slider { min: 1.0, max: 36.0, value: 8.0, decimals: 0 } if Show::On(offset_of(crystal_aura::FACE_PLACE.0));
        FACE_DURABILITY: "Face durability",
            "Percent durability left on any worn piece at or under which Face place starts",
            Slider { min: 0.0, max: 100.0, value: 2.0, decimals: 0 } if Show::On(offset_of(crystal_aura::FACE_PLACE.0));
        SWITCH: "Switch",
            "Normal moves to the crystals and stays. Silent goes back afterwards. Off only places while crystals are held",
            Enum<CrystalSwitch> { default: CrystalSwitch::Silent };
        ANTI_WEAKNESS: "Anti weakness",
            "Under Weakness, take a weapon from the hotbar for the hit, since a bare hand deals nothing",
            Toggle { on: true };
        TURN: "Turn speed",
            "Degrees per second the sent look may travel, rolled fresh inside this range for every new spot",
            Range { min: 90.0, max: 1800.0, low: 600.0, high: 1000.0, decimals: 0, log: false };
        MOVE_FIX: "Move fix",
            "While the sent look is off your camera, re-pick the walk keys so you keep heading the same way",
            Toggle { on: true };
        PAUSE_EAT: "Pause while using",
            "Do nothing while you eat, drink or hold any use",
            Toggle { on: true };
        RENDER: "Render",
            "Outline the block the next crystal goes on",
            Toggle { on: true };
    }

    Xray / xray: Render, Legit,
        "Draws chosen blocks through terrain, each in its own colour" {
        BLOCKS: "Blocks",
            "Which blocks are drawn through terrain, and in what colour",
            List { list: &super::esp::blocks::XRAY };
        RANGE: "Range",
            "How far from you in blocks the sweep looks",
            Slider { min: 16.0, max: 256.0, value: 64.0, decimals: 0 };
        SHAPE: "Shape",
            "Lines draws the outline of a shape, Sides fills its faces, Both does each",
            Enum<Shape> { default: Shape::Both };
        FILL: "Fill",
            "How solid the filled faces are, 0 to 255",
            Slider { min: 0.0, max: 255.0, value: 25.0, decimals: 0 };
    }

    OreEsp / ore_esp: Render, Legit,
        "Draws every ore through terrain, each in its own colour" {
        BLOCKS: "Blocks",
            "Which ores are drawn through terrain, and in what colour",
            List { list: &super::esp::blocks::ORES };
        RANGE: "Range",
            "How far from you in blocks the sweep looks",
            Slider { min: 16.0, max: 256.0, value: 64.0, decimals: 0 };
        SHAPE: "Shape",
            "Lines draws the outline of a shape, Sides fills its faces, Both does each",
            Enum<Shape> { default: Shape::Both };
        FILL: "Fill",
            "How solid the filled faces are, 0 to 255",
            Slider { min: 0.0, max: 255.0, value: 25.0, decimals: 0 };
    }

    SeedXray / seed_xray: Render, Legit,
        "Works out where the ores are from the world seed, hidden ones included" {
        SEED: "Seed",
            "The world's seed, a number or the text it was made from",
            Text { max_len: 32, default: "", expand: false, hint: "Type a seed" };
        BLOCKS: "Ores",
            "Which ores are drawn, and in what colour",
            List { list: &super::seed_xray::ORES };
        RANGE: "Range",
            "How far from you in blocks ores are worked out and drawn",
            Slider { min: 16.0, max: 256.0, value: 64.0, decimals: 0 };
        SHAPE: "Shape",
            "Lines draws the outline of a shape, Sides fills its faces, Both does each",
            Enum<Shape> { default: Shape::Both };
        FILL: "Fill",
            "How solid the filled faces are, 0 to 255",
            Slider { min: 0.0, max: 255.0, value: 25.0, decimals: 0 };
    }

    SeedCracker / seed_cracker: World, Legit,
        "Works out the world seed from structures, End pillars and decorations" {
        FINDERS: "Finders",
            "Which structures and decorations are looked for",
            List { list: &super::seed_cracker::FINDERS };
        SHOW_BOXES: "Show boxes",
            "Draw what each finder matched, in its colour",
            Toggle { on: true };
        SEND: "Send to Seed Xray",
            "Put a found seed into Seed Xray's Seed setting",
            Toggle { on: true };
        HELP: "Help",
            "What to look for, and which finds are key",
            Button { text: "Open Help", key: "help" };
        HUD: "HUD",
            "Show progress in a HUD panel instead of chat; the found seed still goes to chat to copy",
            Toggle { on: true };
        SHARE: "Share",
            "Copy this world's finds as text, for a friend to import and search with",
            Button { text: "Copy", key: "share" };
        FRIEND: "Friend's share",
            "Paste a friend's copied finds here, then press Import",
            Text { max_len: 65536, default: "", expand: true, hint: "Paste a share" };
        IMPORT: "Import",
            "Same hashed seed as yours: adds their finds. Another: replaces yours with theirs",
            Button { text: "Import", key: "import" };
    }

    StorageEsp / storage_esp: Render, Legit,
        "Draws chests, shulkers and furnaces through terrain" {
        BLOCKS: "Blocks",
            "Which containers are drawn through terrain, and in what colour",
            List { list: &super::esp::blocks::STORAGE };
        RANGE: "Range",
            "How far from you in blocks the sweep looks",
            Slider { min: 16.0, max: 256.0, value: 64.0, decimals: 0 };
        SHAPE: "Shape",
            "Lines draws the outline of a shape, Sides fills its faces, Both does each",
            Enum<Shape> { default: Shape::Both };
        FILL: "Fill",
            "How solid the filled faces are, 0 to 255",
            Slider { min: 0.0, max: 255.0, value: 50.0, decimals: 0 };
    }

    Nametags / nametags: Render, Legit,
        "Enlarges player names and shows health, gamemode, distance and armor" {
        SCALE: "Scale", "",
            Slider { min: 0.5, max: 4.0, value: 1.6, decimals: 1 };
        SIZING: "Sizing",
            "Whether a tag shrinks with distance like vanilla or stays one size at any range",
            Enum<Sizing> { default: Sizing::Perspective };
        HEALTH: "Health",
            "How much health is left, as a number after the name, a bar above it, or both",
            Enum<Health> { default: Health::Number };
        GAMEMODE: "Gamemode",
            "Show each player's gamemode, and mark one the tab list has never heard of",
            Toggle { on: true };
        DISTANCE: "Distance", "", Toggle { on: true };
        ARMOR: "Armor",
            "Draw each player's armor above the name, as Meteor does",
            Toggle { on: true };
        DURABILITY: "Durability",
            "The item's own bar only, or the bar with the uses left or the percentage left written above it",
            Enum<Durability> { default: Durability::Percent } if NAMETAGS_ARMOR;
        ENCHANTS: "Enchants",
            "List each piece's enchantments above it, cut to three letters and a level",
            Toggle { on: true } if NAMETAGS_ARMOR;
    }

    Zoom / zoom: Render, Legit,
        "Eases the camera's field of view in on toggle, and back out again" {
        FOV: "Fov",
            "Degrees the camera eases to while zoomed in",
            Slider { min: 1.0, max: 90.0, value: 30.0, decimals: 0 };
        DURATION: "Duration",
            "Seconds the zoom takes to ease in or out",
            Slider { min: 0.05, max: 2.0, value: 0.15, decimals: 2 };
    }

    ExtraHud / extra_hud: Render, Legit,
        "Adds HUD panels: your armor, its durability and a low durability warning" {
        SHOWN: "Shown",
            "Which armor slots get a box: all four, all four while any is worn, worn pieces, or pieces below the warning",
            Enum<Shown> { default: Shown::NotEmpty };
        VERTICAL: "Vertical", "Stack the slots in a column instead of a row",
            Toggle { on: false };
        REVERSED: "Reversed", "Boots first instead of the helmet",
            Toggle { on: false };
        BACKGROUND: "Background", "Draw the hotbar's slot frame behind the pieces",
            Toggle { on: true };
        EMPTY_ICONS: "Empty icons", "Draw the inventory's outline icon in an empty slot",
            Toggle { on: true } if EXTRA_HUD_EMPTY;
        DURABILITY: "Durability",
            "The item's own bar only, or the bar with the uses left or the percentage left written beside it",
            Enum<Durability> { default: Durability::Bar };
        WARNING: "Warning", "Show a warning icon over a piece about to break",
            Toggle { on: true };
        MIN_LEFT: "Uses left", "Warn at or below this many uses left",
            Slider { min: 0.0, max: 500.0, value: 20.0, decimals: 0 } if EXTRA_HUD_WARN;
        MIN_PERCENT: "Percent left", "Warn at or below this percentage of durability left",
            Slider { min: 0.0, max: 100.0, value: 10.0, decimals: 0 } if EXTRA_HUD_WARN;
        BOB: "Bob", "How many pixels the warning icon jitters by, 0 for still",
            Slider { min: 0.0, max: 8.0, value: 3.0, decimals: 0 } if EXTRA_HUD_WARN;
    }

    Fullbright / fullbright: Render, Legit,
        "Draws the world at full light, either baked into the meshes or as a night vision floor on the lightmap" {
        MODE: "Mode",
            "Lighting bakes full light into every chunk mesh and remeshes the world on a switch. Night Vision raises the lightmap floor instead, with no remesh",
            Enum<FullbrightMode> { default: FullbrightMode::NightVision };
    }

    NoRender / no_render: Render, Legit,
        "Hides overlays, HUD elements and world effects you pick" {
        HURT_CAM: "Hurt camera", "No camera tilt when you take damage or die",
            Toggle { on: false };
        PORTAL: "Portal and nausea", "No portal swirl, nausea overlay or screen warp",
            Toggle { on: false };
        FIRE: "Fire", "No fire overlay while burning",
            Toggle { on: false };
        LIQUID_OVERLAY: "Liquid overlay", "No water texture over the screen underwater",
            Toggle { on: false };
        IN_WALL: "In wall", "No black overlay with your head inside a block",
            Toggle { on: false };
        PUMPKIN: "Pumpkin", "No carved pumpkin blur while wearing one",
            Toggle { on: false };
        VIGNETTE: "Vignette", "No world border warning tint",
            Toggle { on: false };
        TOTEM: "Totem animation", "No item popping up in front of you when a totem triggers",
            Toggle { on: false };
        BOSS_BARS: "Boss bars", "No boss bars at the top of the screen",
            Toggle { on: false };
        SCOREBOARD: "Scoreboard", "No scoreboard sidebar",
            Toggle { on: false };
        POTION_ICONS: "Potion icons", "No effect icons in the top right corner",
            Toggle { on: false };
        TITLES: "Titles", "No title and subtitle text in the middle of the screen",
            Toggle { on: false };
        FOG: "Fog", "No distance fog in air, including a boss's world fog",
            Toggle { on: false };
        LIQUID_FOG: "Liquid fog", "No fog under water or in lava",
            Toggle { on: false };
        BLINDNESS: "Blindness", "No fog, dark sky or dimming from blindness and darkness",
            Toggle { on: false };
        BOSS_SKY: "Boss sky", "No sky darkening or world fog from a boss bar",
            Toggle { on: false };
        WEATHER: "Weather", "No rain or snow and no rain splashes; the rain sound and the darker sky stay",
            Toggle { on: false };
        LIGHTNING: "Lightning flash", "No sky flash when lightning strikes",
            Toggle { on: false };
    }

    Freecam / freecam: World, Legit,
        "Detaches the camera and leaves your body where it stands" {
        SPEED: "Speed",
            "Blocks per second the detached camera flies at",
            Slider { min: 1.0, max: 60.0, value: 5.0, decimals: 1 };
        SPRINT_SPEED: "Sprint speed", "",
            Slider { min: 1.0, max: 120.0, value: 20.0, decimals: 1 };
        PAUSE_CLICKS: "Pause clicks",
            "Swallow left and right click while detached. Off, a click acts on the block under the freecam's crosshair, within your body's reach",
            Toggle { on: true };
    }

    AutoMine / auto_mine: World, Normal,
        "Digs a corridor straight ahead, or clears a box you mark out" {
        MODE: "Mode",
            "Corridor digs straight ahead. Area clears the box you mark with Define area, top down",
            Enum<MineMode> { default: MineMode::Corridor };
        DEFINE: "Define area",
            "Closes this screen; left click a block for the first corner, right click for the second",
            Button { text: "Define area", key: "define" } if AUTO_MINE_AREA;
        REACH: "Reach",
            "Adjacent mines only the blocks touching you. All mines everything in reach. 3x2 clears a 3 wide, 2 tall slice around you",
            Enum<ReachMode> { default: ReachMode::Wide } if AUTO_MINE_AREA;
        LENGTH: "Length",
            "Columns to dig before switching off, or 0 to keep going",
            Slider { min: 0.0, max: 256.0, value: 0.0, decimals: 0 } if AUTO_MINE_CORRIDOR;
        TURN: "Turn speed",
            "Degrees per second the aim may travel, rolled fresh inside this range for every block",
            Range { min: 45.0, max: 900.0, low: 180.0, high: 400.0, decimals: 0, log: false };
        DELAY: "Delay",
            "Milliseconds between one block breaking and the next being dug, rolled fresh inside this range",
            Range { min: 0.0, max: 600.0, low: 90.0, high: 260.0, decimals: 0, log: false };
        FLUIDS: "Fluids",
            "A block with lava or water behind it: Skip leaves it standing, Stop switches off. A corridor always stops",
            Enum<FluidRule> { default: FluidRule::Skip };
        INV_FULL: "Inventory full",
            "Stop switches off with a chat line once nothing more fits; Ignore keeps mining and leaves the drops",
            Enum<InvFull> { default: InvFull::Stop } if AUTO_MINE_AREA;
        LOW_HEALTH: "On low health",
            "Disconnect leaves the server once health drops below the Health setting",
            Enum<LowHealth> { default: LowHealth::Ignore };
        HEALTH: "Health",
            "Disconnect below this many half hearts. The bar stops at 20; type a larger number for more",
            Slider { min: 1.0, max: 20.0, value: 6.0, decimals: 0 } if AUTO_MINE_DISCONNECT;
        PROGRESS: "Progress HUD",
            "Show a bar of how much of the area is cleared",
            Toggle { on: false } if AUTO_MINE_AREA;
        IN_GUI: "Allow in GUI",
            "Keep digging while a screen is open, which no hand could do",
            Toggle { on: false };
    }

    Scaffold / scaffold: World, Blatant,
        "Places blocks under you as you walk off edges" {
        MODE: "Mode",
            "Legit only sneaks at edges. GodBridge places under you with a look the camera does not show. TellyBridge jumps and fills your starting layer from the air",
            Enum<ScaffoldMode> { default: ScaffoldMode::GodBridge };
        DELAY: "Delay",
            "Ticks to wait after a placement before the next",
            Slider { min: 0.0, max: 10.0, value: 0.0, decimals: 0 };
        AUTO_BLOCK: "AutoBlock",
            "Select the hotbar slot with the most usable blocks when you are not holding any, and go back on disable",
            Toggle { on: true };
        EXPAND: "Expand",
            "How close in blocks you must be to a side of your block before the hole beside it is filled. 0 never",
            Slider { min: 0.0, max: 0.5, value: 0.3, decimals: 2 };
        TOWER: "Tower",
            "How you climb while jump is held: Jump and Step jump every ground tick, Motion launches at 0.42 and pulls down past a block, Smooth jumps every third ground tick",
            Enum<Tower> { default: Tower::Off };
        BLOCKS: "Blocks",
            "Which blocks may be used: any, all but the filter, or only the filter",
            Enum<BlockFilter> { default: BlockFilter::All };
        FILTER: "Filter blocks",
            "The blocks Blacklist leaves out or Whitelist allows",
            List { list: &super::esp::blocks::SCAFFOLD } if SCAFFOLD_FILTERED;
        PITCH_CHECK: "Pitch check",
            "Only work while you look down at least Pitch angle",
            Toggle { on: false };
        PITCH_ANGLE: "Pitch angle",
            "Degrees below level your camera must look",
            Slider { min: 0.0, max: 90.0, value: 60.0, decimals: 0 } if Show::On(offset_of(scaffold::PITCH_CHECK.0));
        BLOCK_COUNT: "Block count",
            "Show how many of the held block you carry, under the crosshair",
            Toggle { on: false };
        SNEAK_DELAY: "Sneak delay",
            "Milliseconds the edge sneak is held after the edge, rolled fresh inside this range for each sneak",
            Range { min: 0.0, max: 1000.0, low: 90.0, high: 210.0, decimals: 0, log: false } if SCAFFOLD_LEGIT;
        REQUIRE_SNEAK: "Require sneak",
            "Only sneak at edges while you hold sneak, and lift your sneak away from them",
            Toggle { on: false } if SCAFFOLD_LEGIT;
        ACTIVATION: "Activation blocks",
            "Blocks you must place by hand under you before it takes over. 0 starts at once",
            Slider { min: 0.0, max: 10.0, value: 0.0, decimals: 0 } if SCAFFOLD_BRIDGE;
        REQUIRE_RIGHT_CLICK: "Require right click",
            "Only bridge while the use key is held",
            Toggle { on: false } if SCAFFOLD_TELLY;
        Y_INCREASE: "Y increase",
            "Blocks a jump may climb above the starting layer before placing stops, one more or less now and then",
            Slider { min: 0.0, max: 10.0, value: 3.0, decimals: 0 } if SCAFFOLD_TELLY;
        TURN: "Turn speed",
            "Degrees per tick the sent look may turn, rolled fresh inside this range every tick",
            Range { min: 1.0, max: 90.0, low: 30.0, high: 55.0, decimals: 0, log: false };
        REACH: "Reach",
            "Blocks from your eye a click may reach. The server allows your block reach, 4.5 in survival, plus 1",
            Slider { min: 2.0, max: 6.0, value: 4.4, decimals: 1 };
    }

    Flight / flight: Movement, Blatant,
        "Flies without the server having granted it, at creative speed" {
        SPEED: "Speed",
            "Flying speed, where 0.05 is what creative mode gives you and sprinting doubles it",
            Slider { min: 0.0, max: 0.2, value: 0.05, decimals: 2 };
        ANTI_KICK: "Anti kick",
            "How the server's floating counter is reset: Drop falls for a tick, Packet reports a fall that did not happen",
            Enum<AntiKick> { default: AntiKick::Packet };
        EVERY: "Anti kick every",
            "Ticks of flight between resets, well under the 80 a server allows",
            Slider { min: 1.0, max: 200.0, value: 20.0, decimals: 0 };
        FOR: "Anti kick for",
            "Ticks each reset lasts, which under Drop is how far you sag. Packet always uses at least 3",
            Slider { min: 1.0, max: 20.0, value: 1.0, decimals: 0 };
    }

    Sneak / sneak: Movement, Legit,
        "Holds sneak on whether or not the key is pressed" {}

    AutoWalk / auto_walk: Movement, Legit,
        "Holds forward on whether or not the key is pressed" {
        PAUSE_ON_SCREEN: "Pause on screen",
            "Stop walking while a screen is open, as a hand on the keys would",
            Toggle { on: false };
        SPRINT: "Sprint", "Hold the sprint key too",
            Toggle { on: true };
        AUTO_JUMP: "Auto jump",
            "Jump up one-block steps in the way, as the Auto-Jump option does, whatever that option is set to",
            Toggle { on: true };
    }

    NoFall / no_fall: Movement, Blatant,
        "Reports you as standing on the ground while you fall, so the server charges no fall damage" {
        PAUSE_ON_MACE: "Pause on mace",
            "Take the fall while holding a mace, whose smash attack is paid for with the distance fallen",
            Toggle { on: true };
    }

    VClip / vclip: Movement, Blatant,
        "Moves you straight up or down by a set number of blocks, then switches itself off" {
        DISTANCE: "Distance",
            "Whole blocks to move, negative for down. Past 20 a vanilla server puts you back",
            Slider { min: -50.0, max: 50.0, value: 5.0, decimals: 0 };
    }

    Velocity / velocity: Movement, Blatant,
        "Ignores knockback, cobwebs and water drag" {
        KNOCKBACK: "Attack knockback",
            "Drop the motion the server sends when you are hit, which also covers wind charges, fishing rods and launch pads",
            Toggle { on: true };
        EXPLOSIONS: "Explosion knockback",
            "Drop the push from TNT, creepers, crystals and beds, which some servers also use for hits",
            Toggle { on: true };
        NO_WEB: "No web",
            "Walk and fall through cobwebs at full speed",
            Toggle { on: true };
        NO_WATER_DRAG: "No water drag",
            "Move through water at land speed, as three levels of Depth Strider would on the bottom",
            Toggle { on: false };
    }

    AutoSell / auto_sell: Misc, Normal,
        "Types the sell command whenever a chosen item is in your inventory" {
        ITEMS: "Items",
            "Which items are sold whenever one turns up in your inventory",
            List { list: &super::items::LIST };
        COMMAND: "Command",
            "Sent once per sale, %s replaced with the item's id. A leading / is optional and stripped either way",
            Text { max_len: 128, default: "sellall inventory %s", expand: false, hint: "Type a command" };
    }

    Special / special: Misc, Legit,
        "Cuts a typed phrase out of chat lines, formatting codes ignored" {
        PHRASE: "Phrase",
            "Cut out of every chat line, wherever it appears",
            Text { max_len: 64, default: "", expand: true, hint: "Type a phrase" };
    }

    Commands / commands: Misc, Legit,
        "Runs chat lines that start with the prefix as client commands instead of sending them" {
        PREFIX: "Prefix",
            "What a line starts with to be run here rather than sent. Never /",
            Text { max_len: 4, default: ".", expand: false, hint: "." };
    }

    BetterTooltips / better_tooltips: Misc, Legit,
        "Adds extra rows to item tooltips" {
        FOOD_VALUES: "Food values",
            "Show a food's hunger and saturation as rows of icons at the bottom of its tooltip, as AppleSkin does",
            Toggle { on: true };
    }

    Spam / spam: Misc, Blatant,
        "Sends a message over and over, a delay apart, with random text around it if you like" {
        MESSAGE: "Message",
            "Sent every time. A leading / runs it as a command",
            Text { max_len: 256, default: "", expand: true, hint: "Type a message" };
        DELAY: "Delay",
            "Ticks between messages, 20 to a second, rolled fresh inside this range. Under 20 for long gets you kicked, and a lagging server needs more",
            Range { min: 1.0, max: 1200.0, low: 40.0, high: 80.0, decimals: 0, log: true };
        PREFIX: "Prefix",
            "Put random letters and digits and a space before the message. Skipped for a command",
            Toggle { on: false };
        PREFIX_LENGTH: "Prefix length",
            "How many random characters go in front, rolled inside this range",
            Range { min: 1.0, max: 16.0, low: 4.0, high: 8.0, decimals: 0, log: false };
        SUFFIX: "Suffix",
            "Put a space and random letters and digits after the message",
            Toggle { on: false };
        SUFFIX_LENGTH: "Suffix length",
            "How many random characters go after, rolled inside this range",
            Range { min: 1.0, max: 16.0, low: 4.0, high: 8.0, decimals: 0, log: false };
    }

    Speed / speed: Movement, Blatant, "Modifies your movement speed" {
        SPEED: "Speed",
            "Blocks per second",
            Slider { min: 0.0, max: 20.0, value: 5.6, decimals: 1 };
        IN_LIQUIDS: "In liquids",
            "Toggle to apply while in water or lava",
            Toggle { on: false };
        ONLY_ON_GROUND: "Only on ground",
            "Only apply while standing on a block",
            Toggle { on: false };
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_point_at_their_rows() {
        let name = |i: u16| SETTINGS[i as usize].name;
        assert_eq!(name(trigger_bot::REQUIRE_WEAPON.0), "Require weapon");
        assert_eq!(name(aim_assist::IGNORE_WALLS.0), "Ignore walls");
        assert_eq!(name(auto_mace::WALLS.0), "Through walls");
        assert_eq!(name(auto_totem::DELAY.0), "Delay");
        assert_eq!(name(xray::RANGE.0), "Range");
        assert_eq!(name(nametags::DISTANCE.0), "Distance");
        assert_eq!(name(freecam::SPRINT_SPEED.0), "Sprint speed");
        assert_eq!(name(flight::FOR.0), "Anti kick for");
        assert_eq!(name(special::PHRASE.0), "Phrase");
        assert_eq!(name(auto_sell::COMMAND.0), "Command");
        assert_eq!(name(spam::SUFFIX_LENGTH.0), "Suffix length");
        assert_eq!(name(vclip::DISTANCE.0), "Distance");
        assert_eq!(name(commands::PREFIX.0), "Prefix");
        for m in &MODULES {
            assert!(std::ptr::eq(module(m.id), m));
        }
    }
}
