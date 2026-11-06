use std::borrow::Cow;
use std::collections::hash_map::Entry as Slot;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

use parking_lot::Mutex;
use serde_json::Value;

static NAMES: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);

fn intern(name: &str) -> &'static str {
    let mut names = NAMES.lock();
    if let Some(&known) = names.get(name) {
        return known;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.insert(leaked);
    leaked
}

pub(super) fn known(name: &str) -> Option<&'static str> {
    NAMES.lock().get(name).copied()
}

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
    pub attenuation: f32,
    kind: EntryKind,
}

impl Entry {
    pub fn object_key(&self) -> Arc<str> {
        self.target.clone()
    }
}

#[derive(Default)]
struct Event {
    entries: Vec<Entry>,
    total_weight: u32,
    subtitle: Option<&'static str>,
}

struct Registration {
    entries: Vec<Entry>,
    replace: bool,
    subtitle: Option<&'static str>,
}

#[derive(Default)]
pub struct Defs {
    events: HashMap<&'static str, Event>,
}

const DEFAULT_ATTENUATION: f32 = 16.0;

const MAX_INDIRECTION: usize = 8;

impl Defs {
    pub fn parse(text: &str) -> Result<Defs, String> {
        let mut defs = Defs::default();
        defs.merge("minecraft", text, &|_| true)?;
        defs.finish();
        Ok(defs)
    }

    pub fn merge(
        &mut self,
        namespace: &str,
        text: &str,
        exists: &dyn Fn(&str) -> bool,
    ) -> Result<(), String> {
        let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let object = json.as_object().ok_or("sounds.json is not an object")?;
        let mut parsed = Vec::with_capacity(object.len());
        for (name, body) in object {
            let registration = parse_event(body).map_err(|e| format!("{name}: {e}"))?;
            match event_key(namespace, name) {
                Some(key) => parsed.push((key, registration)),
                None => crate::log_warn!("audio", "bad event name in sounds.json: {name}"),
            }
        }
        for (key, registration) in parsed {
            let Registration {
                entries,
                replace,
                subtitle,
            } = registration;
            let event = match self.events.entry(intern(&key)) {
                Slot::Occupied(slot) => {
                    let event = slot.into_mut();
                    if replace {
                        *event = Event {
                            subtitle,
                            ..Event::default()
                        };
                    }
                    event
                }
                Slot::Vacant(slot) => slot.insert(Event {
                    subtitle,
                    ..Event::default()
                }),
            };
            event.entries.extend(entries.into_iter().filter(|entry| {
                let found = entry.kind == EntryKind::Event || exists(&entry.target);
                if !found {
                    crate::log_warn!("audio", "{key} names a file no pack has: {}", entry.target);
                }
                found
            }));
        }
        Ok(())
    }

    pub fn finish(&mut self) {
        let mut weights: HashMap<&'static str, Option<u32>> =
            HashMap::with_capacity(self.events.len());
        let names: Vec<&'static str> = self.events.keys().copied().collect();
        for name in &names {
            self.weigh(name, &mut weights, 0);
        }
        for event in self.events.values_mut() {
            for entry in &mut event.entries {
                if entry.kind == EntryKind::Event {
                    entry.weight = weights.get(&*entry.target).copied().flatten().unwrap_or(0);
                }
            }
            event.total_weight = sum_weights(event.entries.iter().map(|entry| entry.weight));
        }
        self.events.retain(|_, event| event.total_weight > 0);
    }

    fn weigh(
        &self,
        name: &str,
        weights: &mut HashMap<&'static str, Option<u32>>,
        depth: usize,
    ) -> u32 {
        if let Some(seen) = weights.get(name) {
            return seen.unwrap_or(0);
        }
        let Some((key, event)) = self.events.get_key_value(name) else {
            return 0;
        };
        if depth >= MAX_INDIRECTION {
            return 0;
        }
        let key = *key;
        weights.insert(key, None);
        let total = sum_weights(event.entries.iter().map(|entry| match entry.kind {
            EntryKind::File => entry.weight,
            EntryKind::Event => self.weigh(&entry.target, weights, depth + 1),
        }));
        weights.insert(key, Some(total));
        total
    }

    pub fn key(&self, event: &str) -> Option<&'static str> {
        self.events.get_key_value(event).map(|(key, _)| *key)
    }

    pub fn subtitle(&self, event: &str) -> Option<&'static str> {
        self.events.get(event)?.subtitle
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn object_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .events
            .values()
            .flat_map(|event| &event.entries)
            .filter(|entry| entry.kind == EntryKind::File && entry.target.starts_with("sounds/"))
            .map(|entry| entry.target.to_string())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        keys
    }

    #[cfg(test)]
    pub fn pick(&self, event: &str, roll: &mut impl FnMut(u32) -> u32) -> Option<Entry> {
        self.resolve(event, roll).map(|(_, entry)| entry)
    }

    pub fn resolve(
        &self,
        event: &str,
        roll: &mut impl FnMut(u32) -> u32,
    ) -> Option<(&'static str, Entry)> {
        let (key, mut event) = self.events.get_key_value(event)?;
        let (mut volume, mut pitch, mut stream) = (1.0, 1.0, false);
        for _ in 0..MAX_INDIRECTION {
            let mut remaining = roll(event.total_weight.max(1)) as i64;
            let mut chosen = event.entries.last()?;
            for entry in &event.entries {
                remaining -= entry.weight as i64;
                if remaining < 0 {
                    chosen = entry;
                    break;
                }
            }
            volume *= chosen.volume;
            pitch *= chosen.pitch;
            stream |= chosen.stream;
            match chosen.kind {
                EntryKind::File => {
                    return Some((
                        *key,
                        Entry {
                            volume,
                            pitch,
                            stream,
                            ..chosen.clone()
                        },
                    ));
                }
                EntryKind::Event => event = self.events.get(&*chosen.target)?,
            }
        }
        None
    }
}

fn sum_weights(weights: impl Iterator<Item = u32>) -> u32 {
    weights.fold(0u32, u32::saturating_add).min(i32::MAX as u32)
}

fn valid_namespace(text: &str) -> bool {
    text.bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
}

fn valid_path(text: &str) -> bool {
    text.bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'/'))
}

fn event_key(namespace: &str, path: &str) -> Option<String> {
    (valid_namespace(namespace) && valid_path(path)).then(|| match namespace {
        "minecraft" => path.to_owned(),
        _ => format!("{namespace}:{path}"),
    })
}

fn identifier(text: &str) -> Result<(&str, &str), String> {
    let (namespace, path) = match text.split_once(':') {
        Some(("", path)) => ("minecraft", path),
        Some(pair) => pair,
        None => ("minecraft", text),
    };
    if valid_namespace(namespace) && valid_path(path) {
        Ok((namespace, path))
    } else {
        Err(format!("bad name {text}"))
    }
}

fn file_target(namespace: &str, path: &str) -> Arc<str> {
    Arc::from(match namespace {
        "minecraft" => format!("sounds/{path}.ogg"),
        _ => format!("assets/{namespace}/sounds/{path}.ogg"),
    })
}

#[cfg(any(resource_packs, test))]
pub(crate) fn layer_key(target: &str) -> Cow<'_, str> {
    if target.starts_with("sounds/") {
        Cow::Owned(format!("assets/minecraft/{target}"))
    } else {
        Cow::Borrowed(target)
    }
}

pub(super) fn language_key(target: &str) -> String {
    let (namespace, path) = match target
        .strip_prefix("assets/")
        .and_then(|rest| rest.split_once("/sounds/"))
    {
        Some((namespace, path)) => (Some(namespace), path),
        None => (None, target.strip_prefix("sounds/").unwrap_or(target)),
    };
    let path = path.strip_suffix(".ogg").unwrap_or(path).replace('/', ".");
    namespace.map_or_else(|| path.clone(), |namespace| format!("{namespace}.{path}"))
}

fn text(value: &Value) -> Option<Cow<'_, str>> {
    match value {
        Value::String(text) => Some(Cow::Borrowed(text)),
        Value::Number(number) => Some(Cow::Owned(number.to_string())),
        Value::Bool(flag) => Some(Cow::Owned(flag.to_string())),
        _ => None,
    }
}

fn flag(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => Some(text.eq_ignore_ascii_case("true")),
        Value::Number(_) => Some(false),
        _ => None,
    }
}

fn parse_event(body: &Value) -> Result<Registration, String> {
    let object = body.as_object().ok_or("the event is not an object")?;
    let replace = match object.get("replace") {
        None => false,
        Some(value) => flag(value).ok_or("replace is not a boolean")?,
    };
    let subtitle = match object.get("subtitle") {
        None => None,
        Some(value) => Some(intern(&text(value).ok_or("subtitle is not a string")?)),
    };
    let entries = match object.get("sounds") {
        None => Vec::new(),
        Some(sounds) => {
            let list = sounds.as_array().ok_or("sounds is not an array")?;
            list.iter()
                .map(parse_entry)
                .collect::<Result<Vec<Entry>, String>>()?
        }
    };
    Ok(Registration {
        entries,
        replace,
        subtitle,
    })
}

fn parse_entry(value: &Value) -> Result<Entry, String> {
    if let Some(name) = value.as_str() {
        let (namespace, path) = identifier(name)?;
        return Ok(Entry {
            target: file_target(namespace, path),
            volume: 1.0,
            pitch: 1.0,
            weight: 1,
            stream: false,
            attenuation: DEFAULT_ATTENUATION,
            kind: EntryKind::File,
        });
    }
    let object = value
        .as_object()
        .ok_or("a sound is neither a string nor an object")?;
    let name = object
        .get("name")
        .and_then(text)
        .ok_or("a sound has no name")?;
    let (namespace, path) = identifier(&name)?;
    let kind = match object.get("type").map(Value::as_str) {
        None | Some(Some("file")) => EntryKind::File,
        Some(Some("event")) => EntryKind::Event,
        Some(_) => return Err(format!("{name} has an invalid type")),
    };
    let number = |key: &str, default: f64| match object.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_f64()
            .ok_or_else(|| format!("{name}: {key} is not a number")),
    };
    let positive = |key: &str| match number(key, 1.0)? {
        v if v > 0.0 => Ok(v),
        _ => Err(format!("{name}: invalid {key}")),
    };
    let stream = match object.get("stream") {
        None => false,
        Some(value) => flag(value).ok_or_else(|| format!("{name}: stream is not a boolean"))?,
    };
    Ok(Entry {
        target: match kind {
            EntryKind::File => file_target(namespace, path),
            EntryKind::Event => {
                Arc::from(event_key(namespace, path).ok_or_else(|| format!("bad name {name}"))?)
            }
        },
        volume: positive("volume")? as f32,
        pitch: positive("pitch")? as f32,
        weight: match number("weight", 1.0)? as i32 {
            weight if weight > 0 => weight as u32,
            _ => return Err(format!("{name}: invalid weight")),
        },
        stream,
        attenuation: number("attenuation_distance", f64::from(DEFAULT_ATTENUATION))? as i32 as f32,
        kind,
    })
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

    fn stack(files: &[(&str, &str)]) -> Defs {
        let mut defs = Defs::default();
        for (namespace, text) in files {
            defs.merge(namespace, text, &|key| !key.contains("missing"))
                .unwrap();
        }
        defs.finish();
        defs
    }

    #[test]
    fn a_pack_appends_and_the_first_subtitle_stays() {
        let defs = stack(&[
            ("minecraft", r#"{"e":{"subtitle":"base","sounds":["a"]}}"#),
            ("minecraft", r#"{"e":{"subtitle":"pack","sounds":["b"]}}"#),
        ]);
        let mut second = |_| 1;
        assert_eq!(
            &*defs.pick("e", &mut second).unwrap().object_key(),
            "sounds/b.ogg"
        );
        assert_eq!(defs.subtitle("e"), Some("base"));
    }

    #[test]
    fn replace_starts_the_list_and_the_subtitle_over() {
        let defs = stack(&[
            ("minecraft", r#"{"e":{"subtitle":"base","sounds":["a"]}}"#),
            (
                "minecraft",
                r#"{"e":{"replace":true,"subtitle":"pack","sounds":["b"]}}"#,
            ),
        ]);
        assert_eq!(
            &*defs.pick("e", &mut first).unwrap().object_key(),
            "sounds/b.ogg"
        );
        assert_eq!(defs.subtitle("e"), Some("pack"));
    }

    #[test]
    fn replace_with_no_sounds_silences_the_event() {
        let defs = stack(&[
            ("minecraft", r#"{"e":{"sounds":["a"]}}"#),
            ("minecraft", r#"{"e":{"replace":true,"sounds":[]}}"#),
        ]);
        assert!(defs.pick("e", &mut first).is_none());
    }

    #[test]
    fn an_empty_event_keeps_its_subtitle_for_a_pack_to_fill() {
        let defs = stack(&[
            ("minecraft", r#"{"e":{"subtitle":"base","sounds":[]}}"#),
            ("minecraft", r#"{"e":{"subtitle":"pack","sounds":["a"]}}"#),
        ]);
        assert_eq!(defs.subtitle("e"), Some("base"));
    }

    #[test]
    fn an_event_entry_can_reach_an_event_a_later_pack_adds() {
        let defs = stack(&[
            (
                "minecraft",
                r#"{"outer":{"sounds":[{"name":"inner","type":"event"}]}}"#,
            ),
            ("minecraft", r#"{"inner":{"sounds":["a"]}}"#),
        ]);
        assert_eq!(
            &*defs.pick("outer", &mut first).unwrap().object_key(),
            "sounds/a.ogg"
        );
    }

    #[test]
    fn events_keep_the_files_namespace_and_names_default_to_minecraft() {
        let defs = stack(&[("mypack", r#"{"e":{"sounds":["mypack:a/b","c"]}}"#)]);
        assert!(defs.key("e").is_none());
        let event = "mypack:e";
        assert_eq!(
            &*defs.pick(event, &mut first).unwrap().object_key(),
            "assets/mypack/sounds/a/b.ogg"
        );
        let mut second = |_| 1;
        assert_eq!(
            &*defs.pick(event, &mut second).unwrap().object_key(),
            "sounds/c.ogg"
        );
    }

    #[test]
    fn a_file_no_pack_has_is_skipped() {
        let defs = stack(&[("minecraft", r#"{"e":{"sounds":["missing","a"]}}"#)]);
        assert_eq!(
            &*defs.pick("e", &mut first).unwrap().object_key(),
            "sounds/a.ogg"
        );
    }

    #[test]
    fn a_bad_file_changes_nothing() {
        let mut defs = Defs::default();
        for bad in [
            r#"{"new":{"sounds":["a"]},"e":{"sounds":[{"name":"b","weight":0}]}}"#,
            r#"{"new":{"sounds":["a"]},"e":{"sounds":[{"name":"b","volume":-1}]}}"#,
            r#"{"new":{"sounds":["a"]},"e":{"sounds":[{"name":"b","type":"sound"}]}}"#,
            r#"{"new":{"sounds":["a"]},"e":{"sounds":["Upper/Case"]}}"#,
            r#"{"new":{"sounds":["a"]},"e":{"sounds":"b"}}"#,
        ] {
            assert!(defs.merge("minecraft", bad, &|_| true).is_err(), "{bad}");
        }
        defs.finish();
        assert_eq!(defs.len(), 0);
    }

    #[test]
    fn a_bad_event_name_skips_that_event_alone() {
        let defs = stack(&[(
            "minecraft",
            r#"{"Bad.Name":{"sounds":["a"]},"e":{"sounds":["a"]}}"#,
        )]);
        assert_eq!(defs.len(), 1);
        assert!(defs.key("e").is_some());
    }

    #[test]
    fn primitives_read_as_gson_reads_them() {
        let defs = stack(&[
            ("minecraft", r#"{"e":{"sounds":["a"]}}"#),
            (
                "minecraft",
                r#"{"e":{"replace":"TRUE","subtitle":12,"sounds":[{"name":34,"stream":"yes"}]}}"#,
            ),
        ]);
        let entry = defs.pick("e", &mut first).unwrap();
        assert_eq!(&*entry.object_key(), "sounds/34.ogg");
        assert!(!entry.stream);
        assert_eq!(defs.subtitle("e"), Some("12"));
    }

    #[test]
    fn an_event_entry_weighs_what_it_reaches_and_multiplies_through() {
        let defs = Defs::parse(
            r#"{
                "outer":{"sounds":[
                    {"name":"inner","type":"event","weight":7,"volume":0.5,"pitch":2.0,"stream":true},
                    "last"
                ]},
                "inner":{"sounds":[
                    {"name":"a","volume":0.5,"pitch":0.5,"attenuation_distance":8},
                    "b"
                ]}
            }"#,
        )
        .unwrap();
        let mut third = |bound| if bound == 3 { 2 } else { 0 };
        assert_eq!(
            &*defs.pick("outer", &mut third).unwrap().object_key(),
            "sounds/last.ogg"
        );
        let entry = defs.pick("outer", &mut first).unwrap();
        assert_eq!(&*entry.object_key(), "sounds/a.ogg");
        assert_eq!((entry.volume, entry.pitch), (0.25, 1.0));
        assert!(entry.stream);
        assert_eq!(entry.attenuation, 8.0);
    }

    #[test]
    fn a_stop_finds_a_name_the_current_table_has_lost() {
        let _ = stack(&[("minecraft", r#"{"defs.test.gone":{"sounds":["a"]}}"#)]);
        assert_eq!(known("defs.test.gone"), Some("defs.test.gone"));
        assert_eq!(known("defs.test.never"), None);
    }

    #[test]
    fn file_keys_map_to_pack_paths_and_language_keys() {
        assert_eq!(
            layer_key("sounds/a/b.ogg"),
            "assets/minecraft/sounds/a/b.ogg"
        );
        assert_eq!(
            layer_key("assets/ns/sounds/a.ogg"),
            "assets/ns/sounds/a.ogg"
        );
        assert_eq!(language_key("sounds/music/game/a.ogg"), "music.game.a");
        assert_eq!(language_key("assets/ns/sounds/x/y.ogg"), "ns.x.y");
    }
}
