use std::ops::Range;

use super::auto_mace::{AimPoint, MaceSlot};
use super::auto_totem::TotemMode;
use super::esp::Shape;
use super::flight::AntiKick;
use super::list::BitList;
use super::nametags::{Health, Sizing};

pub struct SettingDef {
    pub name: &'static str,
    pub tip: &'static str,
    pub kind: Kind,
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
    Rage = 2,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Legit, Mode::Normal, Mode::Rage];
    pub const NAMES: [&'static str; 3] = ["Legit", "Normal", "Rage"];

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
        MODULES.iter().filter(move |m| m.category == self)
    }
}

pub struct ModuleDef {
    pub id: Id,
    pub name: &'static str,
    pub desc: &'static str,
    pub category: Category,
    pub mode: Mode,
}

impl ModuleDef {
    pub fn slots(&self) -> Range<usize> {
        let i = self.id as usize;
        BASE[i] as usize..BASE[i + 1] as usize
    }

    pub fn settings(&self) -> &'static [SettingDef] {
        &SETTINGS[self.slots()]
    }
}

pub fn module(id: Id) -> &'static ModuleDef {
    &MODULES[id as usize]
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

macro_rules! handles {
    ($id:ident; $n:expr;) => {};
    ($id:ident; $n:expr; $konst:ident Enum<$t:ty>; $($rest:tt)*) => {
        pub const $konst: handle::Enum<$t> = handle::Enum::new(slot(Id::$id, $n));
        handles!($id; $n + 1; $($rest)*);
    };
    ($id:ident; $n:expr; $konst:ident List; $($rest:tt)*) => {
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
            $($konst:ident: $sname:literal, $tip:literal, $v:ident $(<$t:ty>)? { $($body:tt)* };)*
        }
    )*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(u8)]
        pub enum Id { $($id),* }

        pub const COUNT: usize = 0 $(+ one!($id))*;

        impl Id {
            pub const ALL: [Id; COUNT] = [$(Id::$id),*];
        }

        pub static MODULES: [ModuleDef; COUNT] = [$(
            ModuleDef {
                id: Id::$id,
                name: stringify!($id),
                desc: $desc,
                category: Category::$cat,
                mode: Mode::$mode,
            },
        )*];

        const LENS: [u16; COUNT] = [$(0 $(+ one!($konst))*),*];

        pub static SETTINGS: [SettingDef; SETTING_COUNT] = [$($(
            SettingDef { name: $sname, tip: $tip, kind: kind!($v $(<$t>)? { $($body)* }) },
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

modules! {
    TriggerBot / trigger_bot: Combat, Normal,
        "Attacks whatever the crosshair is on, as fast as you could" {
        TARGETS: "Targets",
            "Which entity kinds count as a target, shared with AimAssist",
            List { list: &super::entities::LIST };
        DELAY: "Delay",
            "Milliseconds between swings, rolled fresh inside this range every time",
            Range { min: 0.0, max: 250.0, low: 45.0, high: 110.0, decimals: 0 };
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
            Range { min: 0.0, max: 400.0, low: 100.0, high: 220.0, decimals: 0 };
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
            Range { min: 90.0, max: 1800.0, low: 600.0, high: 1000.0, decimals: 0 };
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
            Slider { min: 0.0, max: 36.0, value: 10.0, decimals: 0 };
        ELYTRA: "Elytra",
            "Smart: equip while gliding",
            Toggle { on: true };
        FALL: "Fall",
            "Smart: equip when the landing below would deal at least your health in fall damage",
            Toggle { on: true };
        EXPLOSION: "Explosion",
            "Smart: equip when crystals, lit TNT and swelling creepers in range could deal at least your health",
            Toggle { on: true };
        DELAY: "Delay",
            "Milliseconds after a move before the next one, rolled fresh inside this range",
            Range { min: 0.0, max: 500.0, low: 0.0, high: 100.0, decimals: 0 };
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
        "Enlarges player names and shows health, gamemode and distance" {
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

    Freecam / freecam: World, Legit,
        "Detaches the camera and leaves your body where it stands" {
        SPEED: "Speed",
            "Blocks per second the detached camera flies at",
            Slider { min: 1.0, max: 60.0, value: 5.0, decimals: 1 };
        SPRINT_SPEED: "Sprint speed", "",
            Slider { min: 1.0, max: 120.0, value: 20.0, decimals: 1 };
    }

    AutoMine / auto_mine: World, Normal,
        "Digs a two-block-tall corridor straight ahead, one column at a time" {
        LENGTH: "Length",
            "Columns to dig before switching off, or 0 to keep going",
            Slider { min: 0.0, max: 256.0, value: 0.0, decimals: 0 };
        TURN: "Turn speed",
            "Degrees per second the aim may travel, rolled fresh inside this range for every block",
            Range { min: 45.0, max: 900.0, low: 180.0, high: 400.0, decimals: 0 };
        DELAY: "Delay",
            "Milliseconds between one block breaking and the next being dug, rolled fresh inside this range",
            Range { min: 0.0, max: 600.0, low: 90.0, high: 260.0, decimals: 0 };
        HAZARDS: "Stop on hazard",
            "Switch off rather than open a wall with lava or water behind it",
            Toggle { on: true };
        IN_GUI: "Allow in GUI",
            "Keep digging while a screen is open, which no hand could do",
            Toggle { on: false };
    }

    Flight / flight: Movement, Rage,
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
            "Ticks each reset lasts, which under Drop is how far you sag",
            Slider { min: 1.0, max: 20.0, value: 1.0, decimals: 0 };
    }

    Sneak / sneak: Movement, Legit,
        "Holds sneak on whether or not the key is pressed" {}

    NoFall / no_fall: Movement, Rage,
        "Reports you as standing on the ground while you fall, so the server charges no fall damage" {
        PAUSE_ON_MACE: "Pause on mace",
            "Take the fall while holding a mace, whose smash attack is paid for with the distance fallen",
            Toggle { on: true };
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
        for m in &MODULES {
            assert!(std::ptr::eq(module(m.id), m));
        }
    }
}
