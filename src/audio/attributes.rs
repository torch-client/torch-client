use std::sync::Arc;

use azalea_core::registry_holder::RegistryHolder;
use parking_lot::Mutex;
use serde_json::Value;

use super::defs::Defs;
use crate::renderer::dimension::Dimension;
use crate::util::datapack::{bare_id, entries, entry};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Music {
    pub event: &'static str,
    pub min_delay: i32,
    pub max_delay: i32,
    pub replace: bool,
}

impl Music {
    pub const MENU: Music = Music {
        event: "music.menu",
        min_delay: 20,
        max_delay: 600,
        replace: true,
    };
    pub const CREDITS: Music = Music {
        event: "music.credits",
        min_delay: 0,
        max_delay: 0,
        replace: true,
    };
    pub const END_BOSS: Music = Music {
        event: "music.dragon",
        min_delay: 0,
        max_delay: 0,
        replace: true,
    };
}

#[derive(Clone, Copy, Default)]
pub(super) struct BackgroundMusic {
    default: Option<Music>,
    creative: Option<Music>,
    underwater: Option<Music>,
}

impl BackgroundMusic {
    pub fn select(self, creative: bool, underwater: bool) -> Option<Music> {
        match (underwater, self.underwater, creative, self.creative) {
            (true, Some(music), ..) => Some(music),
            (.., true, Some(music)) => Some(music),
            _ => self.default,
        }
    }
}

pub(super) struct Mood {
    pub event: &'static str,
    pub tick_delay: i32,
    pub block_search_extent: i32,
    pub offset: f64,
}

pub(super) struct Addition {
    pub event: &'static str,
    pub tick_chance: f64,
}

pub(super) struct AmbientSounds {
    pub looped: Option<&'static str>,
    pub mood: Option<Mood>,
    pub additions: Vec<Addition>,
}

#[derive(Default)]
struct Layer {
    music: Option<BackgroundMusic>,
    ambient: Option<AmbientSounds>,
    music_volume: Option<f32>,
}

pub(super) struct Synced {
    biomes: Vec<Option<Value>>,
    dimensions: [Option<Value>; 3],
}

static SYNCED: Mutex<Option<Arc<Synced>>> = Mutex::new(None);

pub(crate) fn publish_synced(registries: &RegistryHolder) {
    let biomes = registries
        .extra
        .get(&azalea::Identifier::from("worldgen/biome"));
    let Some(biomes) = biomes else {
        return;
    };
    let value =
        |tag: Option<&simdnbt::owned::NbtTag>| tag.and_then(|tag| serde_json::to_value(tag).ok());
    let dimension = |name: &str| {
        let element = registries
            .dimension_type
            .map
            .get(&azalea::Identifier::from(name));
        value(element.and_then(|element| element._extra.get("attributes")))
    };
    *SYNCED.lock() = Some(Arc::new(Synced {
        biomes: biomes
            .map
            .values()
            .map(|biome| value(biome.get("attributes")))
            .collect(),
        dimensions: [
            dimension("overworld"),
            dimension("the_nether"),
            dimension("the_end"),
        ],
    }));
}

pub(crate) fn clear_synced() {
    *SYNCED.lock() = None;
}

pub(super) fn synced() -> Option<Arc<Synced>> {
    SYNCED.lock().clone()
}

#[derive(Clone, Copy)]
pub(super) struct Cell {
    pub row: Option<u8>,
    pub id: Option<u32>,
}

pub(super) struct Attributes {
    dimensions: [Layer; 3],
    biomes: Vec<Layer>,
    synced_dimensions: [Option<Layer>; 3],
    synced_biomes: Vec<Option<Layer>>,
}

fn dimension_slot(dimension: Dimension) -> usize {
    match dimension {
        Dimension::Overworld => 0,
        Dimension::Nether => 1,
        Dimension::End => 2,
    }
}

impl Attributes {
    pub fn load(defs: &Defs, synced: Option<&Synced>) -> Attributes {
        let mut biomes = Vec::new();
        for (name, json) in entries("worldgen/biome") {
            let Some(row) = crate::util::biome_color::biome_index(&name) else {
                continue;
            };
            let row = row as usize;
            if biomes.len() <= row {
                biomes.resize_with(row + 1, Layer::default);
            }
            biomes[row] = Layer::parse(&json, defs);
        }
        let dimension = |name: &str| {
            entry("dimension_type", name)
                .map_or_else(Layer::default, |json| Layer::parse(&json, defs))
        };
        let layer =
            |attributes: &Option<Value>| attributes.as_ref().map(|value| Layer::read(value, defs));
        Attributes {
            dimensions: [
                dimension("overworld"),
                dimension("the_nether"),
                dimension("the_end"),
            ],
            biomes,
            synced_dimensions: synced.map_or([None, None, None], |synced| {
                synced.dimensions.each_ref().map(layer)
            }),
            synced_biomes: synced
                .map_or_else(Vec::new, |synced| synced.biomes.iter().map(layer).collect()),
        }
    }

    fn read<'a, T>(
        &'a self,
        cell: Cell,
        dimension: Dimension,
        attribute: impl Fn(&'a Layer) -> Option<T>,
    ) -> Option<T> {
        let slot = dimension_slot(dimension);
        cell.id
            .and_then(|id| self.synced_biomes.get(id as usize)?.as_ref())
            .or_else(|| self.biomes.get(cell.row? as usize))
            .and_then(&attribute)
            .or_else(|| {
                attribute(
                    self.synced_dimensions[slot]
                        .as_ref()
                        .unwrap_or(&self.dimensions[slot]),
                )
            })
    }

    pub fn music(&self, cell: Cell, dimension: Dimension) -> Option<BackgroundMusic> {
        self.read(cell, dimension, |layer| layer.music)
    }

    pub fn ambient(&self, cell: Cell, dimension: Dimension) -> Option<&AmbientSounds> {
        self.read(cell, dimension, |layer| layer.ambient.as_ref())
    }

    pub fn music_volume(&self, cell: Cell, dimension: Dimension) -> f32 {
        self.read(cell, dimension, |layer| layer.music_volume)
            .unwrap_or(1.0)
    }
}

impl Layer {
    fn parse(json: &Value, defs: &Defs) -> Layer {
        json.get("attributes")
            .map_or_else(Layer::default, |attributes| Layer::read(attributes, defs))
    }

    fn read(attributes: &Value, defs: &Defs) -> Layer {
        Layer {
            music: attributes
                .get("minecraft:audio/background_music")
                .map(|value| BackgroundMusic {
                    default: parse_music(value.get("default"), defs),
                    creative: parse_music(value.get("creative"), defs),
                    underwater: parse_music(value.get("underwater"), defs),
                }),
            ambient: attributes
                .get("minecraft:audio/ambient_sounds")
                .map(|value| parse_ambient(value, defs)),
            music_volume: attributes
                .get("minecraft:audio/music_volume")
                .and_then(Value::as_f64)
                .map(|volume| volume as f32),
        }
    }
}

fn event(value: Option<&Value>, defs: &Defs) -> Option<&'static str> {
    let value = value?;
    defs.key(bare_id(value.get("sound_id").unwrap_or(value))?)
}

fn int(value: &Value, key: &str) -> Option<i32> {
    value.get(key)?.as_i64().map(|number| number as i32)
}

fn parse_music(value: Option<&Value>, defs: &Defs) -> Option<Music> {
    let value = value?;
    Some(Music {
        event: event(value.get("sound"), defs)?,
        min_delay: int(value, "min_delay")?,
        max_delay: int(value, "max_delay")?,
        replace: value.get("replace_current_music").is_some_and(|flag| {
            flag.as_bool()
                .unwrap_or_else(|| flag.as_i64().is_some_and(|byte| byte != 0))
        }),
    })
}

fn parse_ambient(value: &Value, defs: &Defs) -> AmbientSounds {
    let addition = |value: &Value| {
        Some(Addition {
            event: event(value.get("sound"), defs)?,
            tick_chance: value.get("tick_chance")?.as_f64()?,
        })
    };
    AmbientSounds {
        looped: event(value.get("loop"), defs),
        mood: value.get("mood").and_then(|mood| {
            Some(Mood {
                event: event(mood.get("sound"), defs)?,
                tick_delay: int(mood, "tick_delay")?,
                block_search_extent: int(mood, "block_search_extent")?,
                offset: mood.get("offset")?.as_f64()?,
            })
        }),
        additions: match value.get("additions") {
            Some(Value::Array(list)) => list.iter().filter_map(addition).collect(),
            Some(one) => addition(one).into_iter().collect(),
            None => Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn defs() -> &'static Defs {
        let text = json!({
            "music.game": { "sounds": ["a"] },
            "music.creative": { "sounds": ["b"] },
            "music.under_water": { "sounds": ["c"] },
            "ambient.add": { "sounds": ["d"] },
        })
        .to_string();
        Box::leak(Box::new(Defs::parse(&text).unwrap()))
    }

    fn track(sound: &str) -> Value {
        json!({ "sound": sound, "min_delay": 12000, "max_delay": 24000 })
    }

    #[test]
    fn select_prefers_underwater_then_creative_then_default() {
        let music = parse_music(Some(&track("minecraft:music.game")), defs());
        let background = BackgroundMusic {
            default: music,
            creative: Some(Music {
                event: "creative",
                ..music.unwrap()
            }),
            underwater: Some(Music {
                event: "underwater",
                ..music.unwrap()
            }),
        };
        assert_eq!(background.select(true, true).unwrap().event, "underwater");
        assert_eq!(background.select(true, false).unwrap().event, "creative");
        assert_eq!(background.select(false, false).unwrap().event, "music.game");

        let only_default = BackgroundMusic {
            default: music,
            ..BackgroundMusic::default()
        };
        assert_eq!(only_default.select(true, true).unwrap().event, "music.game");
    }

    #[test]
    fn a_biome_value_replaces_the_dimension_value_whole() {
        let defs = defs();
        let layer = |value: Value| {
            Layer::parse(
                &json!({ "attributes": { "minecraft:audio/background_music": value } }),
                defs,
            )
        };
        let attributes = Attributes {
            dimensions: [
                layer(json!({
                    "default": track("minecraft:music.game"),
                    "creative": track("minecraft:music.creative"),
                })),
                Layer::default(),
                Layer::default(),
            ],
            biomes: vec![
                Layer::default(),
                layer(json!({ "default": track("minecraft:music.under_water") })),
            ],
            synced_dimensions: [None, None, None],
            synced_biomes: Vec::new(),
        };
        let creative = |row| {
            let cell = Cell { row, id: None };
            attributes
                .music(cell, Dimension::Overworld)
                .and_then(|music| music.select(true, false))
        };
        assert_eq!(creative(Some(0)).unwrap().event, "music.creative");
        assert_eq!(creative(None).unwrap().event, "music.creative");
        assert_eq!(creative(Some(1)).unwrap().event, "music.under_water");
    }

    #[test]
    fn a_synced_biome_answers_first() {
        let defs = defs();
        let music = |name: &str| {
            Layer::read(
                &json!({ "minecraft:audio/background_music": { "default": track(name) } }),
                defs,
            )
        };
        let attributes = Attributes {
            dimensions: [Layer::default(), Layer::default(), Layer::default()],
            biomes: vec![music("minecraft:music.game")],
            synced_dimensions: [None, None, None],
            synced_biomes: vec![Some(music("minecraft:music.creative")), None],
        };
        let event = |row, id| {
            let cell = Cell { row, id };
            attributes
                .music(cell, Dimension::Overworld)
                .and_then(|music| music.select(false, false))
                .map(|m| m.event)
        };
        assert_eq!(event(Some(0), Some(0)), Some("music.creative"));
        assert_eq!(event(None, Some(0)), Some("music.creative"));
        assert_eq!(event(Some(0), Some(1)), Some("music.game"));
        assert_eq!(event(None, Some(1)), None);
    }

    #[test]
    fn network_values_read_like_datapack_ones() {
        let value = json!({
            "sound": { "sound_id": "minecraft:music.game", "range": 16.0 },
            "min_delay": 1,
            "max_delay": 2,
            "replace_current_music": 1,
        });
        let music = parse_music(Some(&value), defs()).unwrap();
        assert_eq!(music.event, "music.game");
        assert!(music.replace);
    }

    #[test]
    fn additions_read_as_one_object_or_a_list() {
        let one = json!({ "sound": "minecraft:ambient.add", "tick_chance": 0.0111 });
        let single = parse_ambient(&json!({ "additions": one }), defs());
        assert_eq!(single.additions.len(), 1);
        let list = parse_ambient(&json!({ "additions": [one, one] }), defs());
        assert_eq!(list.additions.len(), 2);
    }
}
