use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EntryKind {
    File,
    Event,
}

#[derive(Clone, Debug)]
pub struct Entry {
    target: Arc<str>,
    pub volume: f32,
    pub pitch: f32,
    weight: u32,
    pub stream: bool,
    kind: EntryKind,
}

impl Entry {
    pub fn object_key(&self) -> Arc<str> {
        self.target.clone()
    }
}

struct Event {
    entries: Vec<Entry>,
    total_weight: u32,
    #[allow(dead_code, reason = "no subtitle renderer yet; see the field doc")]
    subtitle: Option<String>,
}

pub struct Defs {
    events: HashMap<String, Event>,
}

const MAX_INDIRECTION: usize = 3;

impl Defs {
    pub fn parse(text: &str) -> Result<Defs, String> {
        let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let object = json
            .as_object()
            .ok_or_else(|| "sounds.json is not an object".to_owned())?;

        let mut events = HashMap::with_capacity(object.len());
        for (name, body) in object {
            let entries: Vec<Entry> = body
                .get("sounds")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(parse_entry).collect())
                .unwrap_or_default();
            if entries.is_empty() {
                continue;
            }
            let total_weight = entries.iter().map(|e| e.weight).sum();
            events.insert(
                name.clone(),
                Event {
                    entries,
                    total_weight,
                    subtitle: body
                        .get("subtitle")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                },
            );
        }
        Ok(Defs { events })
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn object_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .events
            .values()
            .flat_map(|event| &event.entries)
            .filter(|entry| entry.kind == EntryKind::File)
            .map(|entry| entry.target.to_string())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    pub fn pick(&self, event: &str, roll: &mut impl FnMut(u32) -> u32) -> Option<&Entry> {
        let mut name = event;
        for _ in 0..MAX_INDIRECTION {
            let event = self.events.get(name)?;
            let mut remaining = roll(event.total_weight.max(1)) as i64;
            let mut chosen = event.entries.last()?;
            for entry in &event.entries {
                remaining -= entry.weight as i64;
                if remaining < 0 {
                    chosen = entry;
                    break;
                }
            }
            match chosen.kind {
                EntryKind::File => return Some(chosen),
                EntryKind::Event => name = &chosen.target,
            }
        }
        None
    }
}

fn parse_entry(value: &Value) -> Option<Entry> {
    if let Some(name) = value.as_str() {
        return Some(Entry {
            target: object_key(name),
            volume: 1.0,
            pitch: 1.0,
            weight: 1,
            stream: false,
            kind: EntryKind::File,
        });
    }
    let name = value.get("name")?.as_str()?;
    let kind = match value.get("type").and_then(Value::as_str) {
        Some("event") => EntryKind::Event,
        _ => EntryKind::File,
    };
    let number = |key: &str, default: f32| {
        value
            .get(key)
            .and_then(Value::as_f64)
            .map_or(default, |v| v as f32)
    };
    Some(Entry {
        target: match kind {
            EntryKind::File => object_key(name),
            EntryKind::Event => Arc::from(name),
        },
        volume: number("volume", 1.0),
        pitch: number("pitch", 1.0),
        weight: value
            .get("weight")
            .and_then(Value::as_u64)
            .map_or(1, |w| w.max(1) as u32),
        stream: value
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        kind,
    })
}

fn object_key(name: &str) -> Arc<str> {
    Arc::from(format!("sounds/{name}.ogg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(bound: u32) -> u32 {
        let _ = bound;
        0
    }

    #[test]
    fn a_bare_string_entry_is_every_default() {
        let defs = Defs::parse(r#"{"block.stone.break":{"sounds":["dig/stone1"]}}"#).unwrap();
        let entry = defs.pick("block.stone.break", &mut first).unwrap();
        assert_eq!(&*entry.object_key(), "sounds/dig/stone1.ogg");
        assert_eq!(entry.volume, 1.0);
        assert_eq!(entry.pitch, 1.0);
        assert!(!entry.stream);
    }

    #[test]
    fn an_object_entry_reads_its_volume_and_pitch() {
        let defs = Defs::parse(
            r#"{"e":{"sounds":[{"name":"a/b","volume":0.55,"pitch":1.5,"stream":true}]}}"#,
        )
        .unwrap();
        let entry = defs.pick("e", &mut first).unwrap();
        assert_eq!(entry.volume, 0.55);
        assert_eq!(entry.pitch, 1.5);
        assert!(entry.stream);
    }

    #[test]
    fn weight_decides_which_entry_a_roll_lands_on() {
        let defs = Defs::parse(
            r#"{"e":{"sounds":[
                {"name":"heavy","weight":10},
                {"name":"light","weight":1}
            ]}}"#,
        )
        .unwrap();
        for roll in [0u32, 5, 9] {
            let mut r = |_| roll;
            let got = defs.pick("e", &mut r).unwrap().object_key();
            assert_eq!(&*got, "sounds/heavy.ogg", "{roll}");
        }
        let mut r = |_| 10;
        assert_eq!(
            &*defs.pick("e", &mut r).unwrap().object_key(),
            "sounds/light.ogg"
        );
    }

    #[test]
    fn an_event_entry_resolves_to_the_event_it_names() {
        let defs = Defs::parse(
            r#"{
                "outer":{"sounds":[{"name":"inner","type":"event"}]},
                "inner":{"sounds":["real/file"]}
            }"#,
        )
        .unwrap();
        assert_eq!(
            &*defs.pick("outer", &mut first).unwrap().object_key(),
            "sounds/real/file.ogg"
        );
    }

    #[test]
    fn an_indirection_cycle_gives_up_instead_of_hanging() {
        let defs = Defs::parse(
            r#"{
                "a":{"sounds":[{"name":"b","type":"event"}]},
                "b":{"sounds":[{"name":"a","type":"event"}]}
            }"#,
        )
        .unwrap();
        assert!(defs.pick("a", &mut first).is_none());
    }

    #[test]
    fn an_event_with_no_entries_is_dropped_rather_than_kept_empty() {
        let defs = Defs::parse(r#"{"empty":{"sounds":[]},"full":{"sounds":["x"]}}"#).unwrap();
        assert_eq!(defs.len(), 1);
        assert!(defs.pick("empty", &mut first).is_none());
    }

    #[test]
    fn only_file_entries_are_asked_for_from_the_object_store() {
        let defs = Defs::parse(
            r#"{
                "outer":{"sounds":[{"name":"inner","type":"event"}]},
                "inner":{"sounds":["real/file","real/file"]}
            }"#,
        )
        .unwrap();
        assert_eq!(defs.object_keys(), vec!["sounds/real/file.ogg".to_owned()]);
    }
}
