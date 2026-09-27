use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use serde_json::Value;

use crate::util::datapack;

use super::timeline::Argb;

static DIMENSION_TYPES: OnceLock<Vec<(String, Value)>> = OnceLock::new();

fn loaded() -> &'static [(String, Value)] {
    DIMENSION_TYPES.get_or_init(|| {
        let entries = datapack::entries("dimension_type");
        crate::log_info!(
            "dimension",
            "loaded {} dimension type(s) from the datapack",
            entries.len()
        );
        entries
    })
}

fn opaque(rgb: u32) -> Argb {
    Argb(0xFF00_0000 | rgb)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Dimension {
    #[default]
    Overworld,
    Nether,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Skybox {
    Overworld,
    None,
    End,
}

pub const DEFAULT_CARDINAL: [f32; 6] = [0.5, 1.0, 0.8, 0.8, 0.6, 0.6];
pub const NETHER_CARDINAL: [f32; 6] = [0.9, 0.9, 0.8, 0.8, 0.6, 0.6];

pub const UP: usize = 1;

impl Dimension {
    pub fn from_path(path: &str) -> Self {
        match path {
            "the_nether" => Dimension::Nether,
            "the_end" => Dimension::End,
            _ => Dimension::Overworld,
        }
    }

    fn registry_id(self) -> &'static str {
        match self {
            Dimension::Overworld => "overworld",
            Dimension::Nether => "the_nether",
            Dimension::End => "the_end",
        }
    }

    fn json(self) -> Option<&'static Value> {
        loaded()
            .iter()
            .find(|(id, _)| id == self.registry_id())
            .map(|(_, json)| json)
    }

    fn attributes(self) -> &'static Attributes {
        static ALL: OnceLock<[Attributes; 3]> = OnceLock::new();
        &ALL.get_or_init(|| {
            [
                resolve(Dimension::Overworld),
                resolve(Dimension::Nether),
                resolve(Dimension::End),
            ]
        })[self as usize]
    }
}

struct Attributes {
    ambient_light_color: Argb,
    sky_light_color: Argb,
    sky_light_factor: f32,
    sky_color: Argb,
    fog_color: Argb,
    fog_distance: (f32, f32),
    water_fog_color: Argb,
    water_fog_distance: (f32, f32),
    sky_fog_end: f32,
    has_skylight: bool,
    has_ceiling: bool,
    cardinal: &'static [f32; 6],
}

fn resolve(dimension: Dimension) -> Attributes {
    let json = dimension.json();
    let attribute = |key: &str| json.and_then(|j| j.get("attributes")?.get(key));
    let color = |key: &str, fallback: u32| {
        attribute(key)
            .and_then(datapack::hex_rgb)
            .map_or(opaque(fallback), opaque)
    };
    let number = |key: &str, fallback: f32| {
        attribute(key)
            .and_then(Value::as_f64)
            .map_or(fallback, |v| v as f32)
    };

    let ambient_fallback = match dimension {
        Dimension::Overworld => 0x0A_0A0A,
        Dimension::Nether => 0x30_2821,
        Dimension::End => 0x3F_473F,
    };
    let fog_fallback = match dimension {
        Dimension::Overworld => 0xC0_D8FF,
        Dimension::Nether => 0x33_0808,
        Dimension::End => 0x18_1318,
    };
    let sky_light_fallback = match dimension {
        Dimension::Overworld => 0xFF_FFFF,
        Dimension::Nether => 0x7A_7AFF,
        Dimension::End => 0xAC_60CD,
    };
    let fog_distance_fallback = match dimension {
        Dimension::Nether => (10.0, 96.0),
        Dimension::Overworld | Dimension::End => (0.0, 1024.0),
    };
    let (skylight_fallback, ceiling_fallback) = match dimension {
        Dimension::Nether => (false, true),
        Dimension::Overworld | Dimension::End => (true, false),
    };

    Attributes {
        ambient_light_color: color("minecraft:visual/ambient_light_color", ambient_fallback),
        sky_light_color: color("minecraft:visual/sky_light_color", sky_light_fallback),
        sky_light_factor: number("minecraft:visual/sky_light_factor", 0.0),
        sky_color: color("minecraft:visual/sky_color", 0x00_0000),
        fog_color: color("minecraft:visual/fog_color", fog_fallback),
        fog_distance: (
            number(
                "minecraft:visual/fog_start_distance",
                fog_distance_fallback.0,
            ),
            number("minecraft:visual/fog_end_distance", fog_distance_fallback.1),
        ),
        water_fog_color: color("minecraft:visual/water_fog_color", 0x05_0533),
        water_fog_distance: (
            number("minecraft:visual/water_fog_start_distance", -8.0),
            number("minecraft:visual/water_fog_end_distance", 96.0),
        ),
        sky_fog_end: number("minecraft:visual/sky_fog_end_distance", 512.0),
        has_skylight: json
            .and_then(|j| j.get("has_skylight"))
            .and_then(Value::as_bool)
            .unwrap_or(skylight_fallback),
        has_ceiling: json
            .and_then(|j| j.get("has_ceiling"))
            .and_then(Value::as_bool)
            .unwrap_or(ceiling_fallback),
        cardinal: match json
            .and_then(|j| j.get("cardinal_light"))
            .and_then(datapack::bare_id)
        {
            Some("nether") => &NETHER_CARDINAL,
            Some("default") => &DEFAULT_CARDINAL,
            _ if matches!(dimension, Dimension::Nether) => &NETHER_CARDINAL,
            _ => &DEFAULT_CARDINAL,
        },
    }
}

impl Dimension {
    pub fn y_range(self) -> (i32, i32) {
        match self {
            Dimension::Overworld => (-64, 320),
            Dimension::Nether | Dimension::End => (0, 256),
        }
    }

    pub fn ambient_light_color(self) -> Argb {
        self.attributes().ambient_light_color
    }

    pub fn sky_light_color(self) -> Option<Argb> {
        match self {
            Dimension::Overworld => None,
            Dimension::Nether | Dimension::End => Some(self.attributes().sky_light_color),
        }
    }

    pub fn sky_light_factor(self) -> Option<f32> {
        match self {
            Dimension::Overworld => None,
            Dimension::Nether | Dimension::End => Some(self.attributes().sky_light_factor),
        }
    }

    pub fn has_sun(self) -> bool {
        match self.sky_light_factor() {
            Some(factor) => factor > 0.0,
            None => true,
        }
    }

    pub fn cardinal(self) -> &'static [f32; 6] {
        self.attributes().cardinal
    }

    pub fn skybox(self) -> Skybox {
        match self {
            Dimension::Overworld => Skybox::Overworld,
            Dimension::Nether => Skybox::None,
            Dimension::End => Skybox::End,
        }
    }

    pub fn base_sky_color(self) -> Argb {
        self.attributes().sky_color
    }

    pub fn base_fog_color(self) -> Argb {
        self.attributes().fog_color
    }

    pub fn base_water_fog_color(self) -> Argb {
        self.attributes().water_fog_color
    }

    pub fn base_fog_distance(self) -> (f32, f32) {
        self.attributes().fog_distance
    }

    pub fn base_sky_fog_end(self) -> f32 {
        self.attributes().sky_fog_end
    }

    pub fn base_water_fog_distance(self) -> (f32, f32) {
        self.attributes().water_fog_distance
    }

    pub fn runs_day_timeline(self) -> bool {
        matches!(self, Dimension::Overworld)
    }

    pub fn can_have_weather(self) -> bool {
        let attributes = self.attributes();
        attributes.has_skylight && !attributes.has_ceiling && !matches!(self, Dimension::End)
    }

    pub fn cloud_color(self) -> Option<Argb> {
        match self {
            Dimension::Overworld => None,
            Dimension::Nether | Dimension::End => Some(Argb(0x0000_0000)),
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Dimension {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Dimension::Nether,
        2 => Dimension::End,
        _ => Dimension::Overworld,
    }
}

pub fn set_current(dim: Dimension) {
    CURRENT.store(id_of(dim), Ordering::Relaxed);
}

pub fn id_of(dim: Dimension) -> u8 {
    match dim {
        Dimension::Overworld => 0,
        Dimension::Nether => 1,
        Dimension::End => 2,
    }
}

#[cfg(target_arch = "wasm32")]
pub fn from_id(v: u8) -> Dimension {
    match v {
        1 => Dimension::Nether,
        2 => Dimension::End,
        _ => Dimension::Overworld,
    }
}
