use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use mlua::{Error, Lua, Table, Value};

use super::super::commands::norm;
use super::super::list::BitList;
use super::super::registry::{Kind, Show};

pub(super) const MAX_OPTIONS: usize = 32;

const OPTION_CHARS: usize = 32;

const MAX_DEPTH: usize = 4;

pub(super) fn options(what: &str, v: Option<&Value>) -> mlua::Result<Vec<String>> {
    let Some(Value::Table(t)) = v else {
        return Err(Error::runtime(format!(
            "{what}: `options` is required, a list of names"
        )));
    };
    let list: Vec<String> = t
        .sequence_values::<String>()
        .collect::<mlua::Result<_>>()
        .map_err(|_| Error::runtime(format!("{what}: `options` is a list of strings")))?;
    if !(2..=MAX_OPTIONS).contains(&list.len()) {
        return Err(Error::runtime(format!(
            "{what}: {} options; 2 to {MAX_OPTIONS}",
            list.len()
        )));
    }
    let mut seen = HashMap::new();
    for o in &list {
        let chars = o.chars().count();
        if !(1..=OPTION_CHARS).contains(&chars) {
            return Err(Error::runtime(format!(
                "{what}: option `{o}` is {chars} characters; 1 to {OPTION_CHARS}"
            )));
        }
        if !o
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
        {
            return Err(Error::runtime(format!(
                "{what}: option `{o}` has a character other than letters, digits, space, _ and -"
            )));
        }
        if let Some(other) = seen.insert(norm(o), o) {
            return Err(Error::runtime(format!(
                "{what}: options `{other}` and `{o}` read the same to `.set`"
            )));
        }
    }
    Ok(list)
}

pub(super) fn intern_options(options: Vec<String>) -> &'static [&'static str] {
    static SEEN: Mutex<Option<HashMap<Vec<String>, &'static [&'static str]>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    let seen = seen.get_or_insert_with(HashMap::new);
    if let Some(s) = seen.get(&options) {
        return s;
    }
    let slice: &'static [&'static str] = options
        .iter()
        .map(|o| super::api::intern(o.clone()))
        .collect::<Vec<_>>()
        .leak();
    seen.insert(options, slice);
    slice
}

pub(super) fn bare_block(id: &str) -> Option<&str> {
    let bare = id.strip_prefix("minecraft:").unwrap_or(id);
    super::super::esp::blocks::is_row(bare).then_some(bare)
}

pub(super) fn block_ids(what: &str, t: &Table) -> mlua::Result<Vec<String>> {
    let ids: Vec<String> = t
        .sequence_values::<String>()
        .collect::<mlua::Result<_>>()
        .map_err(|_| {
            Error::runtime(format!(
                "{what}: a list of block ids, {{ \"minecraft:stone\", ... }}"
            ))
        })?;
    let mut out = Vec::with_capacity(ids.len());
    for id in &ids {
        let Some(bare) = bare_block(id) else {
            return Err(Error::runtime(format!("{what}: `{id}` is not a block")));
        };
        out.push(bare.to_string());
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

pub(super) fn block_list(
    mod_id: &str,
    key: &str,
    title: &'static str,
    defaults: &[String],
) -> &'static BitList {
    type Shape = (String, String, &'static str, Vec<String>);
    static LISTS: Mutex<Vec<(Shape, &'static BitList)>> = Mutex::new(Vec::new());
    let shape: Shape = (
        mod_id.to_string(),
        key.to_string(),
        title,
        defaults.to_vec(),
    );
    let mut lists = LISTS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, l)) = lists.iter().find(|(s, _)| *s == shape) {
        return l;
    }
    let list_key = super::api::intern(format!("mod:{mod_id}/{key}"));
    let list = super::super::esp::blocks::mod_list(list_key, title, |label| {
        defaults.binary_search_by(|d| d.as_str().cmp(label)).is_ok()
    });
    lists.push((shape, list));
    list
}

pub(super) fn block_set(lua: &Lua, list: &BitList) -> mlua::Result<Table> {
    let t = lua.create_table_with_capacity(0, list.selected())?;
    for i in (0..list.count()).filter(|&i| list.enabled(i)) {
        t.raw_set(format!("minecraft:{}", list.label(i)), true)?;
    }
    Ok(t)
}

#[derive(Clone, Debug)]
pub(super) enum Visible {
    On(String),
    Is(String, Vec<String>),
    Any(Vec<Visible>),
    All(Vec<Visible>),
}

const VISIBLE_HELP: &str = "`visible` is { setting = \"key\" } for a toggle, { setting = \"key\", is = \"Option\" or { ... } } for a choice, or { any = { ... } } / { all = { ... } }";

pub(super) fn visible(what: &str, v: &Value) -> mlua::Result<Visible> {
    visible_at(what, v, 0)
}

fn visible_at(what: &str, v: &Value, depth: usize) -> mlua::Result<Visible> {
    let wrong = || Error::runtime(format!("{what}: {VISIBLE_HELP}"));
    let Value::Table(t) = v else {
        return Err(wrong());
    };
    if depth > MAX_DEPTH {
        return Err(Error::runtime(format!(
            "{what}: `visible` nests deeper than {MAX_DEPTH}"
        )));
    }
    let pairs: Vec<(Value, Value)> = t.pairs::<Value, Value>().collect::<mlua::Result<_>>()?;
    let mut setting = None;
    let mut is = None;
    let mut group = None;
    for (k, v) in pairs {
        let Value::String(k) = k else {
            return Err(wrong());
        };
        match &*k.to_string_lossy() {
            "setting" => match v {
                Value::String(s) => setting = Some(s.to_string_lossy()),
                _ => return Err(wrong()),
            },
            "is" => {
                is = Some(match v {
                    Value::String(s) => vec![s.to_string_lossy()],
                    Value::Table(t) => t
                        .sequence_values::<String>()
                        .collect::<mlua::Result<_>>()
                        .map_err(|_| wrong())?,
                    _ => return Err(wrong()),
                })
            }
            k @ ("any" | "all") => {
                let Value::Table(list) = v else {
                    return Err(wrong());
                };
                let items = list
                    .sequence_values::<Value>()
                    .map(|c| visible_at(what, &c?, depth + 1))
                    .collect::<mlua::Result<Vec<_>>>()?;
                if items.is_empty() {
                    return Err(Error::runtime(format!("{what}: `visible.{k}` is empty")));
                }
                group = Some(if k == "any" {
                    Visible::Any(items)
                } else {
                    Visible::All(items)
                });
            }
            _ => return Err(wrong()),
        }
    }
    match (setting, is, group) {
        (Some(s), None, None) => Ok(Visible::On(s)),
        (Some(s), Some(is), None) if !is.is_empty() => Ok(Visible::Is(s, is)),
        (None, None, Some(g)) => Ok(g),
        _ => Err(wrong()),
    }
}

pub(super) fn resolve(
    what: &str,
    v: &Visible,
    own: usize,
    keys: &[(&str, Kind)],
) -> mlua::Result<Show> {
    let find = |key: &str| {
        let at = keys.iter().position(|(k, _)| *k == key);
        match at {
            Some(i) if i == own => Err(Error::runtime(format!(
                "{what}: `visible` names the setting itself"
            ))),
            Some(i) if i > u8::MAX as usize => Err(Error::runtime(format!(
                "{what}: `visible` can name only the module's first 256 settings"
            ))),
            Some(i) => Ok((i as u8, keys[i].1)),
            None => Err(Error::runtime(format!(
                "{what}: `visible` names `{key}`, which this module has no setting called"
            ))),
        }
    };
    Ok(match v {
        Visible::On(key) => match find(key)? {
            (i, Kind::Toggle { .. }) => Show::On(i),
            _ => {
                return Err(Error::runtime(format!(
                    "{what}: `visible = {{ setting = \"{key}\" }}` needs a toggle; for a choice add `is`"
                )));
            }
        },
        Visible::Is(key, want) => match find(key)? {
            (i, Kind::Enum { options, .. }) => {
                let mut mask = 0u32;
                for w in want {
                    let Some(at) = options.iter().position(|o| *o == w.as_str()) else {
                        return Err(Error::runtime(format!(
                            "{what}: `{w}` is not one of {key}'s options ({})",
                            options.join(", ")
                        )));
                    };
                    mask |= 1 << at;
                }
                Show::Is(i, mask)
            }
            _ => {
                return Err(Error::runtime(format!(
                    "{what}: `visible.is` needs `{key}` to be a choice"
                )));
            }
        },
        Visible::Any(list) | Visible::All(list) => {
            let shows = list
                .iter()
                .map(|c| resolve(what, c, own, keys))
                .collect::<mlua::Result<Vec<_>>>()?;
            let shows = intern_shows(shows);
            if matches!(v, Visible::Any(_)) {
                Show::Any(shows)
            } else {
                Show::All(shows)
            }
        }
    })
}

fn intern_shows(shows: Vec<Show>) -> &'static [Show] {
    static SEEN: Mutex<Vec<&'static [Show]>> = Mutex::new(Vec::new());
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(s) = seen.iter().copied().find(|s| *s == shows.as_slice()) {
        return s;
    }
    let s: &'static [Show] = shows.leak();
    seen.push(s);
    s
}
