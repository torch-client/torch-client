use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GrassModifier {
    None,
    DarkForest,
    Swamp,
}

impl GrassModifier {
    fn parse(name: &str) -> GrassModifier {
        match name {
            "dark_forest" => GrassModifier::DarkForest,
            "swamp" => GrassModifier::Swamp,
            _ => GrassModifier::None,
        }
    }
}

struct FallbackRow {
    name: &'static str,
    temperature: f64,
    downfall: f64,
    water: u32,
    grass_override: Option<u32>,
    foliage_override: Option<u32>,
    modifier: GrassModifier,
    dry_foliage_override: Option<u32>,
}

struct BiomeRow {
    name: String,
    temperature: f64,
    downfall: f64,
    water: u32,
    grass_override: Option<u32>,
    foliage_override: Option<u32>,
    modifier: GrassModifier,
    dry_foliage_override: Option<u32>,
    env: BiomeEnv,
}

impl From<&FallbackRow> for BiomeRow {
    fn from(row: &FallbackRow) -> Self {
        BiomeRow {
            name: row.name.to_owned(),
            temperature: row.temperature,
            downfall: row.downfall,
            water: row.water,
            grass_override: row.grass_override,
            foliage_override: row.foliage_override,
            modifier: row.modifier,
            dry_foliage_override: row.dry_foliage_override,
            env: BiomeEnv {
                temperature: row.temperature as f32,
                ..BiomeEnv::NONE
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorAttr {
    Set(u32),
    Multiply(u32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FloatAttr {
    Set(f32),
    Multiply(f32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BiomeEnv {
    pub sky_color: Option<ColorAttr>,
    pub fog_color: Option<ColorAttr>,
    pub water_fog_color: Option<ColorAttr>,
    pub fog_start_distance: Option<FloatAttr>,
    pub fog_end_distance: Option<FloatAttr>,
    pub sky_fog_end_distance: Option<FloatAttr>,
    pub water_fog_start_distance: Option<FloatAttr>,
    pub water_fog_end_distance: Option<FloatAttr>,
    pub has_precipitation: bool,
    pub frozen: bool,
    pub temperature: f32,
}

impl BiomeEnv {
    pub const NONE: BiomeEnv = BiomeEnv {
        sky_color: None,
        fog_color: None,
        water_fog_color: None,
        fog_start_distance: None,
        fog_end_distance: None,
        sky_fog_end_distance: None,
        water_fog_start_distance: None,
        water_fog_end_distance: None,
        has_precipitation: false,
        frozen: false,
        temperature: 0.0,
    };

    pub fn is_identity(&self) -> bool {
        self.sky_color.is_none()
            && self.fog_color.is_none()
            && self.water_fog_color.is_none()
            && self.fog_start_distance.is_none()
            && self.fog_end_distance.is_none()
            && self.sky_fog_end_distance.is_none()
            && self.water_fog_start_distance.is_none()
            && self.water_fog_end_distance.is_none()
    }
}

fn color_attr(attributes: Option<&Value>, key: &str) -> Option<ColorAttr> {
    let entry = attributes?.get(key)?;
    if let Some(argument) = entry.get("argument") {
        let value = crate::util::datapack::hex_rgb(argument)?;
        return match entry.get("modifier").and_then(Value::as_str) {
            Some("multiply_rgb" | "multiply_argb") => Some(ColorAttr::Multiply(value)),
            _ => None,
        };
    }
    crate::util::datapack::hex_rgb(entry).map(ColorAttr::Set)
}

fn float_attr(attributes: Option<&Value>, key: &str) -> Option<FloatAttr> {
    let entry = attributes?.get(key)?;
    if let Some(argument) = entry.get("argument") {
        let value = argument.as_f64()? as f32;
        return match entry.get("modifier").and_then(Value::as_str) {
            Some("multiply") => Some(FloatAttr::Multiply(value)),
            _ => None,
        };
    }
    entry.as_f64().map(|v| FloatAttr::Set(v as f32))
}

fn env_from_json(json: &Value, temperature: f64) -> BiomeEnv {
    let attributes = json.get("attributes");
    BiomeEnv {
        sky_color: color_attr(attributes, "minecraft:visual/sky_color"),
        fog_color: color_attr(attributes, "minecraft:visual/fog_color"),
        water_fog_color: color_attr(attributes, "minecraft:visual/water_fog_color"),
        fog_start_distance: float_attr(attributes, "minecraft:visual/fog_start_distance"),
        fog_end_distance: float_attr(attributes, "minecraft:visual/fog_end_distance"),
        sky_fog_end_distance: float_attr(attributes, "minecraft:visual/sky_fog_end_distance"),
        water_fog_start_distance: float_attr(
            attributes,
            "minecraft:visual/water_fog_start_distance",
        ),
        water_fog_end_distance: float_attr(attributes, "minecraft:visual/water_fog_end_distance"),
        has_precipitation: json
            .get("has_precipitation")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        frozen: json
            .get("temperature_modifier")
            .and_then(Value::as_str)
            .is_some_and(|m| m == "frozen"),
        temperature: temperature as f32,
    }
}

const DEFAULT_WATER_COLOR: u32 = 0x3f76e4;

fn rows_from_datapack() -> Vec<BiomeRow> {
    crate::util::datapack::entries("worldgen/biome")
        .into_iter()
        .filter_map(|(name, json)| {
            let temperature = json.get("temperature")?.as_f64()?;
            let downfall = json.get("downfall")?.as_f64()?;
            let effects = json.get("effects");
            let color = |key: &str| {
                effects
                    .and_then(|e| e.get(key))
                    .and_then(crate::util::datapack::hex_rgb)
            };
            let modifier = GrassModifier::parse(
                effects
                    .and_then(|e| e.get("grass_color_modifier"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("none"),
            );
            Some(BiomeRow {
                name,
                temperature,
                downfall,
                water: color("water_color").unwrap_or(DEFAULT_WATER_COLOR),
                grass_override: color("grass_color"),
                foliage_override: color("foliage_color"),
                modifier,
                dry_foliage_override: color("dry_foliage_color"),
                env: env_from_json(&json, temperature),
            })
        })
        .collect()
}

pub const GRASS_TINTED_BLOCKS: &[&str] = &[
    "grass_block",
    "short_grass",
    "fern",
    "tall_grass",
    "large_fern",
    "sugar_cane",
    "bush",
    "pink_petals",
    "wildflowers",
    "potted_fern",
];

#[derive(Clone, Copy, Default)]
struct BiomeTint {
    grass: [u8; 3],
    foliage: [u8; 3],
    water: [u8; 3],
    dry_foliage: [u8; 3],
}

#[derive(Clone, Copy)]
struct BiomeRatio {
    grass: [f32; 3],
    foliage: [f32; 3],
    water: [f32; 3],
    dry_foliage: [f32; 3],
}

const BIOME_DATA: &[FallbackRow] = &[
    FallbackRow {
        name: "badlands",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: Some(0x90814d),
        foliage_override: Some(0x9e814d),
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "bamboo_jungle",
        temperature: 0.95,
        downfall: 0.9,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "basalt_deltas",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "beach",
        temperature: 0.8,
        downfall: 0.4,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "birch_forest",
        temperature: 0.6,
        downfall: 0.6,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "cherry_grove",
        temperature: 0.5,
        downfall: 0.8,
        water: 0x5db7ef,
        grass_override: Some(0xb6db61),
        foliage_override: Some(0xb6db61),
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "cold_ocean",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3d57d6,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "crimson_forest",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "dark_forest",
        temperature: 0.7,
        downfall: 0.8,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::DarkForest,
        dry_foliage_override: Some(0x7b5334),
    },
    FallbackRow {
        name: "deep_cold_ocean",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3d57d6,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "deep_dark",
        temperature: 0.8,
        downfall: 0.4,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "deep_frozen_ocean",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3938c9,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "deep_lukewarm_ocean",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x45adf2,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "deep_ocean",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "desert",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "dripstone_caves",
        temperature: 0.8,
        downfall: 0.4,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "end_barrens",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "end_highlands",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "end_midlands",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "eroded_badlands",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: Some(0x90814d),
        foliage_override: Some(0x9e814d),
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "flower_forest",
        temperature: 0.7,
        downfall: 0.8,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "forest",
        temperature: 0.7,
        downfall: 0.8,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "frozen_ocean",
        temperature: 0.0,
        downfall: 0.5,
        water: 0x3938c9,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "frozen_peaks",
        temperature: -0.7,
        downfall: 0.9,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "frozen_river",
        temperature: 0.0,
        downfall: 0.5,
        water: 0x3938c9,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "grove",
        temperature: -0.2,
        downfall: 0.8,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "ice_spikes",
        temperature: 0.0,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "jagged_peaks",
        temperature: -0.7,
        downfall: 0.9,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "jungle",
        temperature: 0.95,
        downfall: 0.9,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "lukewarm_ocean",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x45adf2,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "lush_caves",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "mangrove_swamp",
        temperature: 0.8,
        downfall: 0.9,
        water: 0x3a7a6a,
        grass_override: None,
        foliage_override: Some(0x8db127),
        modifier: GrassModifier::Swamp,
        dry_foliage_override: Some(0x7b5334),
    },
    FallbackRow {
        name: "meadow",
        temperature: 0.5,
        downfall: 0.8,
        water: 0x0e4ecf,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "mushroom_fields",
        temperature: 0.9,
        downfall: 1.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "nether_wastes",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "ocean",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "old_growth_birch_forest",
        temperature: 0.6,
        downfall: 0.6,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "old_growth_pine_taiga",
        temperature: 0.3,
        downfall: 0.8,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "old_growth_spruce_taiga",
        temperature: 0.25,
        downfall: 0.8,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "pale_garden",
        temperature: 0.7,
        downfall: 0.8,
        water: 0x76889d,
        grass_override: Some(0x778272),
        foliage_override: Some(0x878d76),
        modifier: GrassModifier::None,
        dry_foliage_override: Some(0xa0a69c),
    },
    FallbackRow {
        name: "plains",
        temperature: 0.8,
        downfall: 0.4,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "river",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "savanna",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "savanna_plateau",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "small_end_islands",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "snowy_beach",
        temperature: 0.05,
        downfall: 0.3,
        water: 0x3d57d6,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "snowy_plains",
        temperature: 0.0,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "snowy_slopes",
        temperature: -0.3,
        downfall: 0.9,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "snowy_taiga",
        temperature: -0.5,
        downfall: 0.4,
        water: 0x3d57d6,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "soul_sand_valley",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "sparse_jungle",
        temperature: 0.95,
        downfall: 0.8,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "stony_peaks",
        temperature: 1.0,
        downfall: 0.3,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "stony_shore",
        temperature: 0.2,
        downfall: 0.3,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "sunflower_plains",
        temperature: 0.8,
        downfall: 0.4,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "swamp",
        temperature: 0.8,
        downfall: 0.9,
        water: 0x617b64,
        grass_override: None,
        foliage_override: Some(0x6a7039),
        modifier: GrassModifier::Swamp,
        dry_foliage_override: Some(0x7b5334),
    },
    FallbackRow {
        name: "taiga",
        temperature: 0.25,
        downfall: 0.8,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "the_end",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "the_void",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "warm_ocean",
        temperature: 0.5,
        downfall: 0.5,
        water: 0x43d5ee,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "warped_forest",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "windswept_forest",
        temperature: 0.2,
        downfall: 0.3,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "windswept_gravelly_hills",
        temperature: 0.2,
        downfall: 0.3,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "windswept_hills",
        temperature: 0.2,
        downfall: 0.3,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "windswept_savanna",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: None,
        foliage_override: None,
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
    FallbackRow {
        name: "wooded_badlands",
        temperature: 2.0,
        downfall: 0.0,
        water: 0x3f76e4,
        grass_override: Some(0x90814d),
        foliage_override: Some(0x9e814d),
        modifier: GrassModifier::None,
        dry_foliage_override: None,
    },
];

fn hex_to_rgb(v: u32) -> [u8; 3] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

fn colormap_sample(map: &image::RgbaImage, temp: f64, downfall: f64) -> [u8; 3] {
    let temp = temp.clamp(0.0, 1.0);
    let rain = downfall.clamp(0.0, 1.0) * temp;
    let x = ((1.0 - temp) * 255.0) as u32;
    let y = ((1.0 - rain) * 255.0) as u32;
    let px = map.get_pixel(x.min(map.width() - 1), y.min(map.height() - 1));
    [px[0], px[1], px[2]]
}

fn dark_forest_modifier(base: [u8; 3]) -> [u8; 3] {
    let base = u32::from_be_bytes([0, base[0], base[1], base[2]]);
    let mixed = ((base & 0x00FE_FEFE) + 0x0028_340a) >> 1;
    hex_to_rgb(mixed)
}

fn load_colormap(colormap_dir: &str, name: &str) -> Option<image::RgbaImage> {
    let path = format!("{}/{}.png", colormap_dir, name);
    let bytes = crate::platform::assets::read(&path)?;
    image::load_from_memory(&bytes)
        .ok()
        .map(|img| img.to_rgba8())
}

fn tint_for(
    row: &BiomeRow,
    grass_map: &image::RgbaImage,
    foliage_map: &image::RgbaImage,
    dry_foliage_map: &image::RgbaImage,
) -> BiomeTint {
    let base_grass = row
        .grass_override
        .map(hex_to_rgb)
        .unwrap_or_else(|| colormap_sample(grass_map, row.temperature, row.downfall));
    let grass = match row.modifier {
        GrassModifier::DarkForest => dark_forest_modifier(base_grass),
        GrassModifier::Swamp => hex_to_rgb(0x6a7039),
        GrassModifier::None => base_grass,
    };

    let foliage = row
        .foliage_override
        .map(hex_to_rgb)
        .unwrap_or_else(|| colormap_sample(foliage_map, row.temperature, row.downfall));

    let dry_foliage = row
        .dry_foliage_override
        .map(hex_to_rgb)
        .unwrap_or_else(|| colormap_sample(dry_foliage_map, row.temperature, row.downfall));

    BiomeTint {
        grass,
        foliage,
        water: hex_to_rgb(row.water),
        dry_foliage,
    }
}

fn ratio(color: [u8; 3], base: [u8; 3]) -> [f32; 3] {
    std::array::from_fn(|i| {
        let base = crate::util::mth::srgb_byte_to_linear(base[i]).max(1e-4);
        crate::util::mth::srgb_byte_to_linear(color[i]) / base
    })
}

struct Table {
    index_by_name: HashMap<String, u8>,
    ratios: Vec<BiomeRatio>,
    envs: Vec<BiomeEnv>,
    downfalls: Vec<f32>,
    any_env: bool,
    #[cfg(feature = "shader_support")]
    absolute: Vec<BiomeTint>,
}

static TABLE: OnceLock<Table> = OnceLock::new();

pub fn init(colormap_dir: &str) {
    if TABLE.get().is_some() {
        return;
    }
    let grass_map = load_colormap(colormap_dir, "grass");
    let foliage_map = load_colormap(colormap_dir, "foliage");
    let dry_foliage_map = load_colormap(colormap_dir, "dry_foliage").unwrap_or_else(|| {
        image::RgbaImage::from_pixel(1, 1, image::Rgba([0x5c, 0x3c, 0x32, 255]))
    });
    let (Some(grass_map), Some(foliage_map)) = (grass_map, foliage_map) else {
        eprintln!("[BiomeColor] Could not load colormap(s) in {colormap_dir}; biome tint disabled");
        return;
    };

    let datapack_rows = rows_from_datapack();
    let rows: Vec<BiomeRow> = if datapack_rows.is_empty() {
        eprintln!(
            "[BiomeColor] No worldgen/biome entries on disk; using the built-in fallback table"
        );
        BIOME_DATA.iter().map(BiomeRow::from).collect()
    } else {
        datapack_rows
    };
    crate::log_info!("biome", "{} biomes loaded", rows.len());

    let absolute: Vec<BiomeTint> = rows
        .iter()
        .map(|row| tint_for(row, &grass_map, &foliage_map, &dry_foliage_map))
        .collect();
    let plains = absolute
        .iter()
        .zip(&rows)
        .find(|(_, row)| row.name == "plains")
        .map(|(t, _)| *t)
        .unwrap_or_default();

    let ratios: Vec<BiomeRatio> = absolute
        .iter()
        .map(|t| BiomeRatio {
            grass: ratio(t.grass, plains.grass),
            foliage: ratio(t.foliage, plains.foliage),
            water: ratio(t.water, plains.water),
            dry_foliage: ratio(t.dry_foliage, plains.dry_foliage),
        })
        .collect();

    let index_by_name = rows
        .iter()
        .enumerate()
        .map(|(i, row)| (row.name.clone(), i as u8))
        .collect();

    let envs: Vec<BiomeEnv> = rows.iter().map(|row| row.env).collect();
    let downfalls: Vec<f32> = rows.iter().map(|row| row.downfall as f32).collect();
    let any_env = envs.iter().any(|env| !env.is_identity());

    let _ = TABLE.set(Table {
        index_by_name,
        ratios,
        envs,
        downfalls,
        any_env,
        #[cfg(feature = "shader_support")]
        absolute,
    });
    let _ = PLAINS_ABSOLUTE.set(plains);
}

static PLAINS_ABSOLUTE: OnceLock<BiomeTint> = OnceLock::new();

const UNTINTED: [u8; 4] = [255, 255, 255, 255];

pub fn plains_grass() -> [u8; 4] {
    match PLAINS_ABSOLUTE.get() {
        Some(t) => [t.grass[0], t.grass[1], t.grass[2], 255],
        None => UNTINTED,
    }
}

pub fn plains_foliage() -> [u8; 4] {
    match PLAINS_ABSOLUTE.get() {
        Some(t) => [t.foliage[0], t.foliage[1], t.foliage[2], 255],
        None => UNTINTED,
    }
}

pub fn plains_dry_foliage() -> [u8; 4] {
    match PLAINS_ABSOLUTE.get() {
        Some(t) => [t.dry_foliage[0], t.dry_foliage[1], t.dry_foliage[2], 255],
        None => UNTINTED,
    }
}

pub fn biome_index(name: &str) -> Option<u8> {
    TABLE.get()?.index_by_name.get(name).copied()
}

pub fn env(idx: u8) -> &'static BiomeEnv {
    static NONE: BiomeEnv = BiomeEnv::NONE;
    TABLE
        .get()
        .and_then(|t| t.envs.get(idx as usize))
        .unwrap_or(&NONE)
}

pub fn downfall(idx: u8) -> f32 {
    TABLE
        .get()
        .and_then(|t| t.downfalls.get(idx as usize))
        .copied()
        .unwrap_or(0.0)
}

pub fn any_biome_env() -> bool {
    TABLE.get().is_some_and(|t| t.any_env)
}

pub const NEUTRAL: [f32; 3] = [1.0, 1.0, 1.0];

fn lookup(idx: u8, pick: fn(&BiomeRatio) -> [f32; 3]) -> [f32; 3] {
    TABLE
        .get()
        .and_then(|t| t.ratios.get(idx as usize))
        .map(pick)
        .unwrap_or(NEUTRAL)
}

#[cfg(feature = "shader_support")]
fn pack_colour(idx: u8, pick: fn(&BiomeTint) -> [u8; 3]) -> [f32; 3] {
    let tint = TABLE
        .get()
        .and_then(|t| t.absolute.get(idx as usize))
        .or(PLAINS_ABSOLUTE.get())
        .map_or([255; 3], pick);
    tint.map(|c| f32::from(c) / 255.0)
}

#[cfg(feature = "shader_support")]
pub fn pack_grass(idx: u8) -> [f32; 3] {
    pack_colour(idx, |t| t.grass)
}
#[cfg(feature = "shader_support")]
pub fn pack_foliage(idx: u8) -> [f32; 3] {
    pack_colour(idx, |t| t.foliage)
}
#[cfg(feature = "shader_support")]
pub fn pack_water(idx: u8) -> [f32; 3] {
    pack_colour(idx, |t| t.water)
}
#[cfg(feature = "shader_support")]
pub fn pack_dry_foliage(idx: u8) -> [f32; 3] {
    pack_colour(idx, |t| t.dry_foliage)
}

pub fn grass_ratio(idx: u8) -> [f32; 3] {
    lookup(idx, |r| r.grass)
}
pub fn foliage_ratio(idx: u8) -> [f32; 3] {
    lookup(idx, |r| r.foliage)
}
pub fn water_ratio(idx: u8) -> [f32; 3] {
    lookup(idx, |r| r.water)
}
pub fn dry_foliage_ratio(idx: u8) -> [f32; 3] {
    lookup(idx, |r| r.dry_foliage)
}

pub fn redstone_ratio(power: u8) -> [f32; 3] {
    let p = power as f32 / 15.0;
    let red = p * 0.6 + if power > 0 { 0.4 } else { 0.3 };
    let green = (p * p * 0.7 - 0.5).clamp(0.0, 1.0);
    let blue = (p * p * 0.6 - 0.7).clamp(0.0, 1.0);
    [red, green, blue]
}

#[cfg(test)]
mod env_tests {
    use super::*;

    #[test]
    fn a_bare_string_sets_the_attribute() {
        let json = serde_json::json!({
            "attributes": { "minecraft:visual/sky_color": "#78a7ff" },
            "has_precipitation": true,
        });
        let env = env_from_json(&json, 0.8);
        assert_eq!(env.sky_color, Some(ColorAttr::Set(0x78_A7FF)));
        assert!(env.has_precipitation);
        assert!(!env.frozen);
        assert!(!env.is_identity());
    }

    #[test]
    fn a_modifier_object_multiplies() {
        let json = serde_json::json!({
            "attributes": {
                "minecraft:visual/water_fog_end_distance": {
                    "argument": 0.85,
                    "modifier": "multiply"
                }
            }
        });
        let env = env_from_json(&json, 0.8);
        assert_eq!(env.water_fog_end_distance, Some(FloatAttr::Multiply(0.85)));
    }

    #[test]
    fn an_unknown_modifier_reads_as_absent() {
        let json = serde_json::json!({
            "attributes": {
                "minecraft:visual/fog_color": { "argument": "#112233", "modifier": "subtract" },
                "minecraft:visual/fog_end_distance": { "argument": 4.0, "modifier": "maximum" }
            }
        });
        let env = env_from_json(&json, 0.0);
        assert_eq!(env.fog_color, None);
        assert_eq!(env.fog_end_distance, None);
        assert!(env.is_identity());
    }

    #[test]
    fn a_biome_with_no_attributes_is_the_identity() {
        let end = env_from_json(&serde_json::json!({ "temperature": 0.5 }), 0.5);
        assert!(end.is_identity());
        assert!(!end.has_precipitation);
        assert_eq!(end.temperature, 0.5);

        let frozen = env_from_json(
            &serde_json::json!({ "temperature_modifier": "frozen", "has_precipitation": true }),
            0.0,
        );
        assert!(frozen.frozen);
    }
}
