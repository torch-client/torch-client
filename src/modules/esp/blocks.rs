use std::borrow::Cow;
use std::sync::OnceLock;

use azalea::block::BlockState;

use crate::modules::list::{BitList, Built, Entry};

pub use crate::modules::list::NO_ROW;

struct Preset {
    name: &'static str,
    color: u32,
}

const fn p(name: &'static str, color: u32) -> Preset {
    Preset { name, color }
}

static ORE_PRESETS: &[Preset] = &[
    p("diamond_ore", 0x3A_ED_E4),
    p("deepslate_diamond_ore", 0x3A_ED_E4),
    p("emerald_ore", 0x17_DD_62),
    p("deepslate_emerald_ore", 0x17_DD_62),
    p("gold_ore", 0xFC_EE_4B),
    p("deepslate_gold_ore", 0xFC_EE_4B),
    p("iron_ore", 0xD8_AF_93),
    p("deepslate_iron_ore", 0xD8_AF_93),
    p("copper_ore", 0xE7_7C_56),
    p("deepslate_copper_ore", 0xE7_7C_56),
    p("redstone_ore", 0xE8_2A_2A),
    p("deepslate_redstone_ore", 0xE8_2A_2A),
    p("lapis_ore", 0x1B_4E_D8),
    p("deepslate_lapis_ore", 0x1B_4E_D8),
    p("ancient_debris", 0x8A_5A_3C),
    p("nether_gold_ore", 0xE8_B2_3A),
    p("nether_quartz_ore", 0xED_E4_DE),
];

static TINTS: &[Preset] = &[
    p("coal_ore", 0x4A_4A_4A),
    p("deepslate_coal_ore", 0x4A_4A_4A),
    p("budding_amethyst", 0xA5_66_E8),
    p("spawner", 0x2E_9E_9E),
    p("trial_spawner", 0x2E_9E_9E),
    p("vault", 0x3E_8E_C8),
    p("bookshelf", 0x9A_7A_4A),
    p("obsidian", 0x2A_1E_3E),
    p("crying_obsidian", 0x3A_1E_6E),
    p("tnt", 0xE0_3A_2A),
    p("bedrock", 0x33_33_33),
    p("water", 0x2E_5A_C8),
    p("lava", 0xE0_6A_1A),
];

static GROUPS: [&str; 5] = ["Ores", "Storage", "Spawners", "Notable", "Other"];

static STORAGE_GROUP: &[&str] = &[
    "chest",
    "barrel",
    "shulker_box",
    "hopper",
    "dispenser",
    "dropper",
    "furnace",
    "smoker",
    "brewing_stand",
    "campfire",
    "crafter",
    "decorated_pot",
    "chiseled_bookshelf",
];

static NOTABLE: &[&str] = &[
    "obsidian",
    "crying_obsidian",
    "tnt",
    "bedrock",
    "budding_amethyst",
    "bookshelf",
    "water",
    "lava",
    "sculk_shrieker",
    "sculk_catalyst",
    "reinforced_deepslate",
    "end_portal_frame",
];

fn group_of(name: &str) -> u8 {
    if name.ends_with("_ore") || name == "ancient_debris" {
        0
    } else if name.contains("spawner") || name == "vault" {
        2
    } else if STORAGE_GROUP.iter().any(|p| name.contains(p)) {
        1
    } else if NOTABLE.contains(&name) {
        3
    } else {
        4
    }
}

fn is_ore(name: &str) -> bool {
    ORE_PRESETS.iter().any(|p| p.name == name)
}

fn is_storage(name: &str) -> bool {
    matches!(
        name,
        "chest"
            | "trapped_chest"
            | "barrel"
            | "ender_chest"
            | "furnace"
            | "blast_furnace"
            | "smoker"
            | "hopper"
            | "dropper"
            | "dispenser"
            | "brewing_stand"
            | "campfire"
            | "soul_campfire"
            | "crafter"
            | "decorated_pot"
            | "chiseled_bookshelf"
    ) || name.ends_with("shulker_box")
        || name.ends_with("copper_chest")
}

fn storage_color(name: &str) -> u32 {
    if name == "ender_chest" {
        0x78_00_FF
    } else if name.ends_with("shulker_box") {
        0xC7_7D_FF
    } else if matches!(name, "chest" | "trapped_chest" | "barrel") || name.ends_with("copper_chest")
    {
        0xF2_C4_3D
    } else {
        0x8C_8C_8C
    }
}

const FALLBACK: u32 = 0xC8_C8_C8;

fn color_of(name: &str) -> u32 {
    if let Some(p) = ORE_PRESETS.iter().chain(TINTS).find(|p| p.name == name) {
        p.color
    } else if is_storage(name) {
        storage_color(name)
    } else {
        FALLBACK
    }
}

const PRESETS: &[(&str, bool)] = &[("Defaults", true), ("None", false)];

fn id_of(state: BlockState) -> &'static str {
    const PREFIX: usize = "minecraft:".len();
    &state.as_block_kind().to_str()[PREFIX..]
}

fn shared() -> &'static (Vec<&'static str>, Vec<(u32, u32)>) {
    static SHARED: OnceLock<(Vec<&'static str>, Vec<(u32, u32)>)> = OnceLock::new();
    SHARED.get_or_init(|| {
        let mut names: Vec<&'static str> = Vec::with_capacity(BlockState::MAX_STATE as usize + 1);
        for id in 0..=BlockState::MAX_STATE {
            let Ok(state) = BlockState::try_from(id) else {
                continue;
            };
            names.push(id_of(state));
        }
        names.sort_unstable();
        names.dedup();

        let mut keys: Vec<(u32, u32)> = Vec::with_capacity(BlockState::MAX_STATE as usize + 1);
        for id in 0..=BlockState::MAX_STATE {
            let Ok(state) = BlockState::try_from(id) else {
                continue;
            };
            if let Ok(row) = names.binary_search(&id_of(state)) {
                keys.push((u32::from(id), row as u32));
            }
        }
        (names, keys)
    })
}

fn build_with(on: fn(&str) -> bool, color: fn(&str) -> u32) -> Built {
    let (names, keys) = shared();
    let entries = names
        .iter()
        .map(|n| Entry {
            group: group_of(n),
            label: Cow::Borrowed(*n),
            on: on(n),
            color: color(n),
        })
        .collect();
    (entries, keys.clone())
}

fn build_xray() -> Built {
    build_with(is_ore, color_of)
}

fn build_ores() -> Built {
    build_with(is_ore, color_of)
}

fn build_storage() -> Built {
    build_with(is_storage, color_of)
}

pub static XRAY: BitList = BitList::new(
    "blocks", "Blocks", 214.0, &GROUPS, true, PRESETS, build_xray,
);

pub static ORES: BitList =
    BitList::new("ore_esp", "Ores", 214.0, &GROUPS, true, PRESETS, build_ores);

pub static STORAGE: BitList = BitList::new(
    "storage_esp",
    "Storage",
    214.0,
    &GROUPS,
    true,
    PRESETS,
    build_storage,
);
