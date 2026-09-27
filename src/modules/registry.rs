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
        which: ListKind,
    },
    Text {
        max_len: usize,
        default: &'static str,
        expand: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Blocks,
    Ores,
    Storage,
    Entities,
    Items,
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

pub struct ModuleDef {
    pub name: &'static str,
    pub desc: &'static str,
    pub on: bool,
    pub mode: Mode,
    pub settings: &'static [SettingDef],
}

pub struct CategoryDef {
    pub name: &'static str,
    pub modules: &'static [ModuleDef],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Id {
    TriggerBot,
    AimAssist,
    Xray,
    OreEsp,
    StorageEsp,
    Nametags,
    Zoom,
    Freecam,
    AutoMine,
    Flight,
    Sneak,
    NoFall,
    AutoSell,
    Special,
}

pub const COUNT: usize = Id::Special as usize + 1;

impl Id {
    pub const ALL: [Id; COUNT] = [
        Id::TriggerBot,
        Id::AimAssist,
        Id::Xray,
        Id::OreEsp,
        Id::StorageEsp,
        Id::Nametags,
        Id::Zoom,
        Id::Freecam,
        Id::AutoMine,
        Id::Flight,
        Id::Sneak,
        Id::NoFall,
        Id::AutoSell,
        Id::Special,
    ];
}

pub fn module(id: Id) -> &'static ModuleDef {
    let mut left = id as usize;
    for c in CATEGORIES {
        if left < c.modules.len() {
            return &c.modules[left];
        }
        left -= c.modules.len();
    }
    unreachable!("Id past the end of CATEGORIES")
}

pub fn flat_modules() -> impl Iterator<Item = &'static ModuleDef> {
    CATEGORIES.iter().flat_map(|c| c.modules.iter())
}

pub fn flat_settings() -> impl Iterator<Item = &'static SettingDef> {
    flat_modules().flat_map(|m| m.settings.iter())
}

macro_rules! settings {
    (
        $(#[$doc:meta])*
        $array:ident / $index:ident {
            $($konst:ident: $name:literal, $tip:literal, $kind:expr;)*
        }
    ) => {
        $(#[$doc])*
        const $array: &[SettingDef] = &[$(
            SettingDef { name: $name, tip: $tip, kind: $kind },
        )*];

        #[doc = concat!("Which of [`", stringify!($array), "`]'s settings is which.")]
        pub mod $index {
            settings!(@index 0; $($konst)*);
        }
    };
    (@index $i:expr; ) => {};
    (@index $i:expr; $head:ident $($tail:ident)*) => {
        #[allow(dead_code, reason = "a list setting is named but never read back")]
        pub const $head: usize = $i;
        settings!(@index $i + 1; $($tail)*);
    };
}

settings! {
    TRIGGERBOT / trigger_bot {
        TARGETS: "Targets",
            "Which entity kinds count as a target, shared with AimAssist",
            Kind::List { which: ListKind::Entities };
        DELAY: "Delay",
            "Milliseconds between swings, rolled fresh inside this range every time",
            Kind::Range { min: 0.0, max: 250.0, low: 45.0, high: 110.0, decimals: 0 };
        WAIT_FOR_COOLDOWN: "Wait for cooldown",
            "Only swing once the attack indicator has fully recharged",
            Kind::Toggle { on: true };
        REQUIRE_WEAPON: "Require weapon",
            "Only swing while holding a sword, axe, mace or trident",
            Kind::Toggle { on: false };
    }
}

settings! {
    AIM_ASSIST / aim_assist {
        TARGETS: "Targets",
            "Which entity kinds may be aimed at, shared with TriggerBot",
            Kind::List { which: ListKind::Entities };
        RANGE: "Range",
            "How far away in blocks a target may be",
            Kind::Slider { min: 1.0, max: 12.0, value: 4.5, decimals: 1 };
        FOV: "FOV",
            "Width in degrees of the cone ahead of you a target has to be inside",
            Kind::Slider { min: 5.0, max: 180.0, value: 45.0, decimals: 0 };
        STRENGTH: "Strength",
            "Percent of your own mouse movement added on top of it toward the target, so 100 turns twice as fast",
            Kind::Slider { min: 0.0, max: 300.0, value: 100.0, decimals: 0 };
        FRICTION: "Friction",
            "Percent of your own mouse movement swallowed when you drag off a target, full on it and none at the edge of the cone",
            Kind::Slider { min: 0.0, max: 90.0, value: 40.0, decimals: 0 };
        DEADZONE: "Deadzone",
            "Degrees short of the target the pull stops, so the last of the aim is always yours",
            Kind::Slider { min: 0.0, max: 5.0, value: 1.0, decimals: 1 };
        REACTION: "Reaction",
            "Milliseconds a newly acquired target is left alone for, rolled inside this range",
            Kind::Range { min: 0.0, max: 400.0, low: 100.0, high: 220.0, decimals: 0 };
        IGNORE_WALLS: "Ignore walls",
            "Keep pulling toward a target that has stepped behind blocks",
            Kind::Toggle { on: false };
    }
}

macro_rules! esp_settings {
    ($array:ident / $index:ident, $list:expr, $what:literal, $fill:literal) => {
        settings! {
            $array / $index {
                BLOCKS: "Blocks",
                    $what,
                    Kind::List { which: $list };
                RANGE: "Range",
                    "How far from you in blocks the sweep looks",
                    Kind::Slider { min: 16.0, max: 256.0, value: 64.0, decimals: 0 };
                SHAPE: "Shape",
                    "Lines draws the outline of a shape, Sides fills its faces, Both does each",
                    Kind::Enum { options: &["Lines", "Sides", "Both"], index: 2 };
                FILL: "Fill",
                    "How solid the filled faces are, 0 to 255",
                    Kind::Slider { min: 0.0, max: 255.0, value: $fill, decimals: 0 };
            }
        }
    };
}

esp_settings!(
    XRAY / xray,
    ListKind::Blocks,
    "Which blocks are drawn through terrain, and in what colour",
    25.0
);

esp_settings!(
    ORE_ESP / ore_esp,
    ListKind::Ores,
    "Which ores are drawn through terrain, and in what colour",
    25.0
);

esp_settings!(
    STORAGE_ESP / storage_esp,
    ListKind::Storage,
    "Which containers are drawn through terrain, and in what colour",
    50.0
);

settings! {
    ZOOM / zoom {
        FOV: "Fov",
            "Degrees the camera eases to while zoomed in",
            Kind::Slider { min: 1.0, max: 90.0, value: 30.0, decimals: 0 };
        DURATION: "Duration",
            "Seconds the zoom takes to ease in or out",
            Kind::Slider { min: 0.05, max: 2.0, value: 0.3, decimals: 2 };
    }
}

settings! {
    NAMETAGS / nametags {
        SCALE: "Scale", "",
            Kind::Slider { min: 0.5, max: 4.0, value: 1.6, decimals: 1 };
        SIZING: "Sizing",
            "Whether a tag shrinks with distance like vanilla or stays one size at any range",
            Kind::Enum { options: &["Perspective", "Constant"], index: 0 };
        HEALTH: "Health",
            "How much health is left, as a number after the name, a bar above it, or both",
            Kind::Enum { options: &["Off", "Number", "Bar", "Both"], index: 1 };
        GAMEMODE: "Gamemode",
            "Show each player's gamemode, and mark one the tab list has never heard of",
            Kind::Toggle { on: true };
        DISTANCE: "Distance", "", Kind::Toggle { on: true };
    }
}

settings! {
    FREECAM / freecam {
        SPEED: "Speed",
            "Blocks per second the detached camera flies at",
            Kind::Slider { min: 1.0, max: 60.0, value: 5.0, decimals: 1 };
        SPRINT_SPEED: "Sprint speed", "",
            Kind::Slider { min: 1.0, max: 120.0, value: 20.0, decimals: 1 };
    }
}

settings! {
    FLIGHT / flight {
        SPEED: "Speed",
            "Flying speed, where 0.05 is what creative mode gives you and sprinting doubles it",
            Kind::Slider { min: 0.0, max: 0.2, value: 0.05, decimals: 2 };
        ANTI_KICK: "Anti kick",
            "How the server's floating counter is reset: Drop falls for a tick, Packet reports a fall that did not happen",
            Kind::Enum { options: &["Off", "Drop", "Packet"], index: 2 };
        EVERY: "Anti kick every",
            "Ticks of flight between resets, well under the 80 a server allows",
            Kind::Slider { min: 1.0, max: 200.0, value: 20.0, decimals: 0 };
        FOR: "Anti kick for",
            "Ticks each reset lasts, which under Drop is how far you sag",
            Kind::Slider { min: 1.0, max: 20.0, value: 1.0, decimals: 0 };
    }
}

settings! {
    AUTO_SELL / auto_sell {
        ITEMS: "Items",
            "Which items are sold whenever one turns up in your inventory",
            Kind::List { which: ListKind::Items };
        COMMAND: "Command",
            "Sent once per sale, %s replaced with the item's id. A leading / is optional and stripped either way",
            Kind::Text { max_len: 128, default: "sellall inventory %s", expand: false };
    }
}

settings! {
    AUTO_MINE / auto_mine {
        LENGTH: "Length",
            "Columns to dig before switching off, or 0 to keep going",
            Kind::Slider { min: 0.0, max: 256.0, value: 0.0, decimals: 0 };
        TURN: "Turn speed",
            "Degrees per second the aim may travel, rolled fresh inside this range for every block",
            Kind::Range { min: 45.0, max: 900.0, low: 180.0, high: 400.0, decimals: 0 };
        DELAY: "Delay",
            "Milliseconds between one block breaking and the next being dug, rolled fresh inside this range",
            Kind::Range { min: 0.0, max: 600.0, low: 90.0, high: 260.0, decimals: 0 };
        HAZARDS: "Stop on hazard",
            "Switch off rather than open a wall with lava or water behind it",
            Kind::Toggle { on: true };
        IN_GUI: "Allow in GUI",
            "Keep digging while a screen is open, which no hand could do",
            Kind::Toggle { on: false };
    }
}

pub static CATEGORIES: &[CategoryDef] = &[
    CategoryDef {
        name: "Combat",
        modules: &[
            ModuleDef {
                name: "TriggerBot",
                desc: "Attacks whatever the crosshair is on, as fast as you could",
                on: false,
                mode: Mode::Normal,
                settings: TRIGGERBOT,
            },
            ModuleDef {
                name: "AimAssist",
                desc: "Pulls your own turn toward the target you are already turning at",
                on: false,
                mode: Mode::Normal,
                settings: AIM_ASSIST,
            },
        ],
    },
    CategoryDef {
        name: "Render",
        modules: &[
            ModuleDef {
                name: "Xray",
                desc: "Draws chosen blocks through terrain, each in its own colour",
                on: false,
                mode: Mode::Legit,
                settings: XRAY,
            },
            ModuleDef {
                name: "OreEsp",
                desc: "Draws every ore through terrain, each in its own colour",
                on: false,
                mode: Mode::Legit,
                settings: ORE_ESP,
            },
            ModuleDef {
                name: "StorageEsp",
                desc: "Draws chests, shulkers and furnaces through terrain",
                on: false,
                mode: Mode::Legit,
                settings: STORAGE_ESP,
            },
            ModuleDef {
                name: "Nametags",
                desc: "Enlarges player names and shows health, gamemode and distance",
                on: false,
                mode: Mode::Legit,
                settings: NAMETAGS,
            },
            ModuleDef {
                name: "Zoom",
                desc: "Eases the camera's field of view in on toggle, and back out again",
                on: false,
                mode: Mode::Legit,
                settings: ZOOM,
            },
        ],
    },
    CategoryDef {
        name: "World",
        modules: &[
            ModuleDef {
                name: "Freecam",
                desc: "Detaches the camera and leaves your body where it stands",
                on: false,
                mode: Mode::Legit,
                settings: FREECAM,
            },
            ModuleDef {
                name: "AutoMine",
                desc: "Digs a two-block-tall corridor straight ahead, one column at a time",
                on: false,
                mode: Mode::Normal,
                settings: AUTO_MINE,
            },
        ],
    },
    CategoryDef {
        name: "Movement",
        modules: &[
            ModuleDef {
                name: "Flight",
                desc: "Flies without the server having granted it, at creative speed",
                on: false,
                mode: Mode::Rage,
                settings: FLIGHT,
            },
            ModuleDef {
                name: "Sneak",
                desc: "Holds sneak on whether or not the key is pressed",
                on: false,
                mode: Mode::Legit,
                settings: &[],
            },
            ModuleDef {
                name: "NoFall",
                desc: "Reports you as standing on the ground while you fall, so the server charges no fall damage",
                on: false,
                mode: Mode::Rage,
                settings: NO_FALL,
            },
        ],
    },
    CategoryDef {
        name: "Misc",
        modules: &[
            ModuleDef {
                name: "AutoSell",
                desc: "Types the sell command whenever a chosen item is in your inventory",
                on: false,
                mode: Mode::Normal,
                settings: AUTO_SELL,
            },
            ModuleDef {
                name: "Special",
                desc: "Cuts a typed phrase out of chat lines, formatting codes ignored",
                on: false,
                mode: Mode::Legit,
                settings: SPECIAL,
            },
        ],
    },
];

settings! {
    NO_FALL / no_fall {
        PAUSE_ON_MACE: "Pause on mace",
            "Take the fall while holding a mace, whose smash attack is paid for with the distance fallen",
            Kind::Toggle { on: true };
    }
}

settings! {
    SPECIAL / special {
        PHRASE: "Phrase",
            "Cut out of every chat line, wherever it appears",
            Kind::Text { max_len: 64, default: "", expand: true };
    }
}
