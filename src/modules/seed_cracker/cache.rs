use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering::Relaxed};

use seedcrack::{
    Cuboid, DecoratorKind, DecoratorObservation, Observation, PillarObservation,
    StructureObservation, StructureSet,
};
use serde_json::{Value, json};

use crate::platform::storage::SEED_CRACKER_CACHE;
use crate::renderer::dimension::Dimension;

const FORMAT: u64 = 1;

const FEEDBACK_SECS: f32 = 2.0;

#[derive(Clone)]
pub(super) struct Drawn {
    pub dimension: Dimension,
    pub boxes: Vec<Cuboid>,
    pub color: u32,
}

pub(super) struct Cache {
    pub hash: i64,
    pub seed: Option<i64>,
    pub finds: Vec<(Observation, Drawn)>,
}

static SHARE: Mutex<String> = Mutex::new(String::new());

static PRESSED_AT: AtomicU32 = AtomicU32::new(0);
static PRESSED: AtomicU8 = AtomicU8::new(0);

fn dimension_name(d: Dimension) -> &'static str {
    match d {
        Dimension::Overworld => "overworld",
        Dimension::Nether => "nether",
        Dimension::End => "end",
    }
}

fn dimension_of(name: &str) -> Option<Dimension> {
    match name {
        "overworld" => Some(Dimension::Overworld),
        "nether" => Some(Dimension::Nether),
        "end" => Some(Dimension::End),
        _ => None,
    }
}

impl Cache {
    pub fn to_json(&self) -> String {
        let finds: Vec<Value> = self
            .finds
            .iter()
            .filter_map(|(o, d)| find_to_json(o, d))
            .collect();
        let mut doc = json!({
            "seedcracker": FORMAT,
            "hash": self.hash.to_string(),
            "finds": finds,
        });
        if let Some(seed) = self.seed {
            doc["seed"] = Value::String(seed.to_string());
        }
        doc.to_string()
    }

    pub fn parse(text: &str) -> Result<Cache, String> {
        let doc: Value =
            serde_json::from_str(text.trim()).map_err(|e| format!("not JSON ({e})"))?;
        match doc.get("seedcracker").and_then(Value::as_u64) {
            Some(FORMAT) => {}
            Some(v) => return Err(format!("format {v}, this client reads {FORMAT}")),
            None => return Err("no \"seedcracker\" key".into()),
        }
        let hash = seed_of(doc.get("hash")).ok_or("no \"hash\"")?;
        let seed = seed_of(doc.get("seed"));
        let finds = doc
            .get("finds")
            .and_then(Value::as_array)
            .ok_or("no \"finds\" list")?
            .iter()
            .map(find_from_json)
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Cache { hash, seed, finds })
    }
}

fn seed_of(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::String(s) => s.trim().parse().ok(),
        v => v.as_i64(),
    }
}

fn find_to_json(o: &Observation, d: &Drawn) -> Option<Value> {
    let mut v = match o {
        Observation::Structure(s) => {
            json!({ "structure": s.set.name(), "chunk": [s.chunk_x, s.chunk_z] })
        }
        Observation::Decorator(dec) => {
            let mut v = json!({ "decorator": dec.kind.name(), "at": [dec.x, dec.y, dec.z] });
            if let Some(above) = dec.height_above_surface {
                v["above"] = json!(above);
            }
            v
        }
        Observation::Pillars(p) => json!({ "pillars": p.heights }),
        Observation::HashedSeed(_) => return None,
    };
    v["dim"] = json!(dimension_name(d.dimension));
    v["color"] = json!(d.color);
    let boxes: Vec<[i32; 6]> = d
        .boxes
        .iter()
        .map(|c| [c.min[0], c.min[1], c.min[2], c.max[0], c.max[1], c.max[2]])
        .collect();
    v["boxes"] = json!(boxes);
    Some(v)
}

fn ints<const N: usize>(v: Option<&Value>, what: &str) -> Result<[i32; N], String> {
    let list = v.and_then(Value::as_array).filter(|l| l.len() == N);
    let list = list.ok_or_else(|| format!("{what} wants {N} numbers"))?;
    let mut out = [0; N];
    for (o, v) in out.iter_mut().zip(list) {
        *o = v
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| format!("{what} wants whole numbers"))?;
    }
    Ok(out)
}

fn find_from_json(v: &Value) -> Result<(Observation, Drawn), String> {
    let (observation, default_dim) = if let Some(name) = v.get("structure").and_then(Value::as_str)
    {
        let set =
            StructureSet::from_name(name).ok_or_else(|| format!("unknown structure {name:?}"))?;
        let [chunk_x, chunk_z] = ints::<2>(v.get("chunk"), "chunk")?;
        let dim = if set == StructureSet::EndCities {
            Dimension::End
        } else {
            Dimension::Overworld
        };
        (
            Observation::Structure(StructureObservation {
                set,
                chunk_x,
                chunk_z,
            }),
            dim,
        )
    } else if let Some(name) = v.get("decorator").and_then(Value::as_str) {
        let kind = DecoratorKind::ALL
            .into_iter()
            .find(|k| k.name() == name)
            .ok_or_else(|| format!("unknown decoration {name:?}"))?;
        let [x, y, z] = ints::<3>(v.get("at"), "at")?;
        let height_above_surface = v.get("above").and_then(Value::as_i64).map(|n| n as i32);
        let dim = if kind == DecoratorKind::EndGateway {
            Dimension::End
        } else {
            Dimension::Overworld
        };
        (
            Observation::Decorator(DecoratorObservation {
                kind,
                x,
                y,
                z,
                height_above_surface,
            }),
            dim,
        )
    } else if let Some(list) = v.get("pillars").and_then(Value::as_array) {
        if list.len() != 10 {
            return Err("pillars wants 10 heights".into());
        }
        let mut heights = [None; 10];
        for (h, v) in heights.iter_mut().zip(list) {
            *h = v.as_i64().map(|n| n as i32);
        }
        (
            Observation::Pillars(PillarObservation { heights }),
            Dimension::End,
        )
    } else {
        return Err("a find that is not a structure, decoration or pillars".into());
    };
    let dimension = v
        .get("dim")
        .and_then(Value::as_str)
        .and_then(dimension_of)
        .unwrap_or(default_dim);
    let color = v
        .get("color")
        .and_then(Value::as_u64)
        .map_or(0xFF_FF_FF, |c| c as u32);
    let boxes = v
        .get("boxes")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|b| ints::<6>(Some(b), "box").ok())
                .map(|b| Cuboid::new([b[0], b[1], b[2]], [b[3], b[4], b[5]]))
                .collect()
        })
        .unwrap_or_default();
    Ok((
        observation,
        Drawn {
            dimension,
            boxes,
            color,
        },
    ))
}

pub(super) fn load() -> Option<Cache> {
    let text = SEED_CRACKER_CACHE.load()?;
    match Cache::parse(&text) {
        Ok(c) => Some(c),
        Err(e) => {
            crate::log_warn!("seed_cracker", "ignoring .seedcracker_cache: {e}");
            None
        }
    }
}

pub(super) fn save(cache: &Cache) {
    let text = cache.to_json();
    SEED_CRACKER_CACHE.store(&text);
    *SHARE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = text;
}

pub(crate) fn share_pressed(now: f32) {
    let text = {
        let held = SHARE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if held.is_empty() {
            None
        } else {
            Some(held.clone())
        }
    };
    let text = text
        .or_else(|| SEED_CRACKER_CACHE.load())
        .filter(|t| !t.trim().is_empty());
    let what = match text {
        Some(t) => {
            crate::platform::clipboard::set(&t);
            1
        }
        None => 2,
    };
    PRESSED_AT.store(now.to_bits(), Relaxed);
    PRESSED.store(what, Relaxed);
}

pub(crate) fn share_label(text: &'static str, now: f32) -> &'static str {
    let what = PRESSED.load(Relaxed);
    let at = f32::from_bits(PRESSED_AT.load(Relaxed));
    if what == 0 || !(0.0..FEEDBACK_SECS).contains(&(now - at)) {
        return text;
    }
    if what == 1 { "Copied" } else { "Nothing yet" }
}
