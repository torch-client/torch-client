use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

type Row = (
    &'static str,
    f64,
    f64,
    u32,
    Option<u32>,
    Option<u32>,
    &'static str,
    Option<u32>,
);

struct BiomeRow {
    name: String,
    temperature: f64,
    downfall: f64,
    water: u32,
    grass_override: Option<u32>,
    foliage_override: Option<u32>,
    modifier: String,
    dry_foliage_override: Option<u32>,
    env: BiomeEnv,
}

impl From<&Row> for BiomeRow {
    fn from(row: &Row) -> Self {
        BiomeRow {
            name: row.0.to_owned(),
            temperature: row.1,
            downfall: row.2,
            water: row.3,
            grass_override: row.4,
            foliage_override: row.5,
            modifier: row.6.to_owned(),
            dry_foliage_override: row.7,
            env: BiomeEnv {
                temperature: row.1 as f32,
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
            let modifier = effects
                .and_then(|e| e.get("grass_color_modifier"))
                .and_then(|v| v.as_str())
                .unwrap_or("none")
                .to_owned();
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

const BIOME_DATA: &[Row] = &[
    (
        "badlands",
        2.0,
        0.0,
        0x3f76e4,
        Some(0x90814d),
        Some(0x9e814d),
        "none",
        None,
    ),
    (
        "bamboo_jungle",
        0.95,
        0.9,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "basalt_deltas",
        2.0,
        0.0,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("beach", 0.8, 0.4, 0x3f76e4, None, None, "none", None),
    ("birch_forest", 0.6, 0.6, 0x3f76e4, None, None, "none", None),
    (
        "cherry_grove",
        0.5,
        0.8,
        0x5db7ef,
        Some(0xb6db61),
        Some(0xb6db61),
        "none",
        None,
    ),
    ("cold_ocean", 0.5, 0.5, 0x3d57d6, None, None, "none", None),
    (
        "crimson_forest",
        2.0,
        0.0,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "dark_forest",
        0.7,
        0.8,
        0x3f76e4,
        None,
        None,
        "dark_forest",
        Some(0x7b5334),
    ),
    (
        "deep_cold_ocean",
        0.5,
        0.5,
        0x3d57d6,
        None,
        None,
        "none",
        None,
    ),
    ("deep_dark", 0.8, 0.4, 0x3f76e4, None, None, "none", None),
    (
        "deep_frozen_ocean",
        0.5,
        0.5,
        0x3938c9,
        None,
        None,
        "none",
        None,
    ),
    (
        "deep_lukewarm_ocean",
        0.5,
        0.5,
        0x45adf2,
        None,
        None,
        "none",
        None,
    ),
    ("deep_ocean", 0.5, 0.5, 0x3f76e4, None, None, "none", None),
    ("desert", 2.0, 0.0, 0x3f76e4, None, None, "none", None),
    (
        "dripstone_caves",
        0.8,
        0.4,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("end_barrens", 0.5, 0.5, 0x3f76e4, None, None, "none", None),
    (
        "end_highlands",
        0.5,
        0.5,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("end_midlands", 0.5, 0.5, 0x3f76e4, None, None, "none", None),
    (
        "eroded_badlands",
        2.0,
        0.0,
        0x3f76e4,
        Some(0x90814d),
        Some(0x9e814d),
        "none",
        None,
    ),
    (
        "flower_forest",
        0.7,
        0.8,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("forest", 0.7, 0.8, 0x3f76e4, None, None, "none", None),
    ("frozen_ocean", 0.0, 0.5, 0x3938c9, None, None, "none", None),
    (
        "frozen_peaks",
        -0.7,
        0.9,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("frozen_river", 0.0, 0.5, 0x3938c9, None, None, "none", None),
    ("grove", -0.2, 0.8, 0x3f76e4, None, None, "none", None),
    ("ice_spikes", 0.0, 0.5, 0x3f76e4, None, None, "none", None),
    (
        "jagged_peaks",
        -0.7,
        0.9,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("jungle", 0.95, 0.9, 0x3f76e4, None, None, "none", None),
    (
        "lukewarm_ocean",
        0.5,
        0.5,
        0x45adf2,
        None,
        None,
        "none",
        None,
    ),
    ("lush_caves", 0.5, 0.5, 0x3f76e4, None, None, "none", None),
    (
        "mangrove_swamp",
        0.8,
        0.9,
        0x3a7a6a,
        None,
        Some(0x8db127),
        "swamp",
        Some(0x7b5334),
    ),
    ("meadow", 0.5, 0.8, 0x0e4ecf, None, None, "none", None),
    (
        "mushroom_fields",
        0.9,
        1.0,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "nether_wastes",
        2.0,
        0.0,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("ocean", 0.5, 0.5, 0x3f76e4, None, None, "none", None),
    (
        "old_growth_birch_forest",
        0.6,
        0.6,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "old_growth_pine_taiga",
        0.3,
        0.8,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "old_growth_spruce_taiga",
        0.25,
        0.8,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "pale_garden",
        0.7,
        0.8,
        0x76889d,
        Some(0x778272),
        Some(0x878d76),
        "none",
        Some(0xa0a69c),
    ),
    ("plains", 0.8, 0.4, 0x3f76e4, None, None, "none", None),
    ("river", 0.5, 0.5, 0x3f76e4, None, None, "none", None),
    ("savanna", 2.0, 0.0, 0x3f76e4, None, None, "none", None),
    (
        "savanna_plateau",
        2.0,
        0.0,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "small_end_islands",
        0.5,
        0.5,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("snowy_beach", 0.05, 0.3, 0x3d57d6, None, None, "none", None),
    ("snowy_plains", 0.0, 0.5, 0x3f76e4, None, None, "none", None),
    (
        "snowy_slopes",
        -0.3,
        0.9,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("snowy_taiga", -0.5, 0.4, 0x3d57d6, None, None, "none", None),
    (
        "soul_sand_valley",
        2.0,
        0.0,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "sparse_jungle",
        0.95,
        0.8,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    ("stony_peaks", 1.0, 0.3, 0x3f76e4, None, None, "none", None),
    ("stony_shore", 0.2, 0.3, 0x3f76e4, None, None, "none", None),
    (
        "sunflower_plains",
        0.8,
        0.4,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "swamp",
        0.8,
        0.9,
        0x617b64,
        None,
        Some(0x6a7039),
        "swamp",
        Some(0x7b5334),
    ),
    ("taiga", 0.25, 0.8, 0x3f76e4, None, None, "none", None),
    ("the_end", 0.5, 0.5, 0x3f76e4, None, None, "none", None),
    ("the_void", 0.5, 0.5, 0x3f76e4, None, None, "none", None),
    ("warm_ocean", 0.5, 0.5, 0x43d5ee, None, None, "none", None),
    (
        "warped_forest",
        2.0,
        0.0,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "windswept_forest",
        0.2,
        0.3,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "windswept_gravelly_hills",
        0.2,
        0.3,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "windswept_hills",
        0.2,
        0.3,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "windswept_savanna",
        2.0,
        0.0,
        0x3f76e4,
        None,
        None,
        "none",
        None,
    ),
    (
        "wooded_badlands",
        2.0,
        0.0,
        0x3f76e4,
        Some(0x90814d),
        Some(0x9e814d),
        "none",
        None,
    ),
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
    let grass = match row.modifier.as_str() {
        "dark_forest" => dark_forest_modifier(base_grass),
        "swamp" => hex_to_rgb(0x6a7039),
        _ => base_grass,
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
    any_env: bool,
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

    let ratios = absolute
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
    let any_env = envs.iter().any(|env| !env.is_identity());

    let _ = TABLE.set(Table {
        index_by_name,
        ratios,
        envs,
        any_env,
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
