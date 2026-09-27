use std::collections::HashSet;

use serde_json::{Map, Value as Json, json};

use super::list::{self, BitList};
use super::registry::{Kind, Mode};
use super::{Store, Value, flat_modules};
use crate::gui::keybinds::Bound;
use crate::platform::storage;

pub fn load(s: &Store) {
    let Some(text) = storage::MODULES.load() else {
        return;
    };
    let Ok(Json::Object(root)) = serde_json::from_str::<Json>(&text) else {
        return;
    };

    if let Some(name) = root.get("profile").and_then(Json::as_str)
        && let Some(i) = Mode::NAMES.iter().position(|n| *n == name)
    {
        s.set_profile(Mode::ALL[i]);
    }

    if let Some(mods) = root.get("modules").and_then(Json::as_object) {
        for (i, m) in flat_modules().enumerate() {
            let Some(saved) = mods.get(m.name).and_then(Json::as_object) else {
                continue;
            };
            if let Some(on) = saved.get("on").and_then(Json::as_bool) {
                s.set_enabled_at(i, on);
            }
            if let Some(key) = saved.get("bind").and_then(Json::as_str) {
                s.set_bind_at(i, Bound::from_serialized_name(key));
            }
            let Some(vals) = saved.get("settings").and_then(Json::as_object) else {
                continue;
            };
            let base = s.settings_of(i).start;
            for (n, def) in m.settings.iter().enumerate() {
                if let Some(v) = vals.get(def.name) {
                    read_setting(s, base + n, def.kind, v);
                }
            }
        }
    }

    for l in list::ALL {
        load_list(l, &root);
    }
}

fn read_setting(s: &Store, idx: usize, kind: Kind, v: &Json) {
    match kind {
        Kind::Toggle { .. } => {
            if let Some(b) = v.as_bool() {
                s.set_value(idx, Value::Bool(b));
            }
        }
        Kind::Slider { min, max, .. } => {
            if let Some(x) = v.as_f64() {
                s.set_value(idx, Value::Num((x as f32).clamp(min, max)));
            }
        }
        Kind::Range { min, max, .. } => {
            if let Some(a) = v.as_array()
                && let [lo, hi] = a.as_slice()
                && let (Some(lo), Some(hi)) = (lo.as_f64(), hi.as_f64())
            {
                let (lo, hi) = ((lo as f32).clamp(min, max), (hi as f32).clamp(min, max));
                s.set_value(idx, Value::Range(lo.min(hi), lo.max(hi)));
            }
        }
        Kind::Enum { options, .. } => {
            if let Some(name) = v.as_str()
                && let Some(c) = options.iter().position(|o| *o == name)
            {
                s.set_value(idx, Value::Choice(c as u8));
            }
        }
        Kind::Text { max_len, .. } => {
            if let Some(t) = v.as_str() {
                s.set_text_at(idx, &t.chars().take(max_len).collect::<String>());
            }
        }
        Kind::List { .. } => {}
    }
}

pub fn save(s: &Store) {
    let mut mods = Map::new();
    for (i, m) in flat_modules().enumerate() {
        let base = s.settings_of(i).start;
        let mut vals = Map::new();
        for (n, def) in m.settings.iter().enumerate() {
            if let Some(v) = write_setting(s, base + n, def.kind) {
                vals.insert(def.name.to_string(), v);
            }
        }
        mods.insert(
            m.name.to_string(),
            json!({
                "on": s.armed_at(i),
                "bind": s.bind_at(i).serialized_name(),
                "settings": vals,
            }),
        );
    }

    let mut root = Map::new();
    root.insert(
        "profile".to_string(),
        json!(Mode::NAMES[s.profile() as usize]),
    );
    root.insert("modules".to_string(), Json::Object(mods));
    for l in list::ALL {
        if let Some(rows) = save_list(l) {
            root.insert(l.key.to_string(), rows);
        }
    }
    storage::MODULES.store(&Json::Object(root).to_string());
}

fn write_setting(s: &Store, idx: usize, kind: Kind) -> Option<Json> {
    Some(match (kind, s.value(idx)) {
        (Kind::Toggle { .. }, Value::Bool(b)) => json!(b),
        (Kind::Slider { .. }, Value::Num(x)) => json!(x),
        (Kind::Range { .. }, Value::Range(lo, hi)) => json!([lo, hi]),
        (Kind::Enum { options, .. }, Value::Choice(c)) => json!(options.get(c as usize)?),
        (Kind::Text { .. }, _) => json!(s.text_at(idx)),
        (Kind::List { .. }, _) => return None,
        _ => return None,
    })
}

fn save_list(l: &BitList) -> Option<Json> {
    if !l.touched() {
        return None;
    }
    if l.colored {
        let mut rows = Map::new();
        for i in 0..l.count() {
            if !l.is_default(i) {
                rows.insert(
                    l.label(i).to_string(),
                    json!({ "on": l.enabled(i), "color": l.color(i) }),
                );
            }
        }
        (!rows.is_empty()).then_some(Json::Object(rows))
    } else {
        let rows: Vec<Json> = (0..l.count())
            .filter(|&i| l.enabled(i) != l.default_on(i))
            .map(|i| json!(l.label(i)))
            .collect();
        (!rows.is_empty()).then_some(Json::Array(rows))
    }
}

fn load_list(l: &BitList, root: &Map<String, Json>) {
    let Some(saved) = root.get(l.key) else {
        return;
    };
    if l.colored {
        let Some(saved) = saved.as_object() else {
            return;
        };
        for i in 0..l.count() {
            let Some(row) = saved.get(l.label(i)).and_then(Json::as_object) else {
                continue;
            };
            if let Some(on) = row.get("on").and_then(Json::as_bool) {
                l.set_enabled(i, on);
            }
            if let Some(c) = row.get("color").and_then(Json::as_u64) {
                l.set_color(i, c as u32);
            }
        }
    } else {
        let Some(saved) = saved.as_array() else {
            return;
        };
        let saved: HashSet<&str> = saved.iter().filter_map(Json::as_str).collect();
        for i in 0..l.count() {
            if saved.contains(l.label(i)) {
                l.set_enabled(i, !l.default_on(i));
            }
        }
    }
}
