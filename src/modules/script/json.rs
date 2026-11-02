use std::collections::HashSet;

use mlua::{Error, Lua, Table, Value};
use serde_json::{Map, Number, Value as Json};

const MAX_DEPTH: usize = 64;
const MAX_TEXT: usize = 4 << 20;
const EXACT: f64 = 9_007_199_254_740_992.0;

struct Markers {
    null: Table,
    array: Table,
}

pub(super) fn table(lua: &Lua) -> mlua::Result<Table> {
    let null = lua.create_table()?;
    null.set_readonly(true);
    let array = lua.create_table()?;
    array.raw_set("__metatable", "torch.json.array")?;
    array.set_readonly(true);

    let t = lua.create_table()?;
    t.raw_set("null", &null)?;
    t.raw_set("array", &array)?;
    let m = Markers {
        null: null.clone(),
        array: array.clone(),
    };
    t.raw_set(
        "encode",
        lua.create_function(move |_, (v, opts): (Value, Option<Table>)| {
            let pretty = match &opts {
                Some(o) => o.raw_get::<Option<bool>>("pretty")?.unwrap_or(false),
                None => false,
            };
            let mut seen = HashSet::new();
            let json = to_json(&m, &v, "value", 0, &mut seen)?;
            let text = if pretty {
                serde_json::to_string_pretty(&json)
            } else {
                serde_json::to_string(&json)
            }
            .map_err(|e| Error::runtime(format!("torch.json.encode: {e}")))?;
            if text.len() > MAX_TEXT {
                return Err(Error::runtime(format!(
                    "torch.json.encode: more than {} MiB",
                    MAX_TEXT >> 20
                )));
            }
            Ok(text)
        })?,
    )?;
    let m = Markers { null, array };
    t.raw_set(
        "decode",
        lua.create_function(move |lua, text: mlua::String| {
            let bytes = text.as_bytes();
            if bytes.len() > MAX_TEXT {
                return Err(Error::runtime(format!(
                    "torch.json.decode: more than {} MiB",
                    MAX_TEXT >> 20
                )));
            }
            let json: Json = serde_json::from_slice(&bytes)
                .map_err(|e| Error::runtime(format!("torch.json.decode: {e}")))?;
            to_lua(lua, &m, &json, 0)
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}

pub(super) fn plain_to_json(v: &Value, path: &str) -> mlua::Result<Json> {
    to_json_with(None, v, path, 0, &mut HashSet::new())
}

fn to_json(
    m: &Markers,
    v: &Value,
    path: &str,
    depth: usize,
    seen: &mut HashSet<usize>,
) -> mlua::Result<Json> {
    to_json_with(Some(m), v, path, depth, seen)
}

fn to_json_with(
    m: Option<&Markers>,
    v: &Value,
    path: &str,
    depth: usize,
    seen: &mut HashSet<usize>,
) -> mlua::Result<Json> {
    let bad = |why: &str| Error::runtime(format!("torch.json.encode: {path} {why}"));
    if depth > MAX_DEPTH {
        return Err(bad(&format!("is nested more than {MAX_DEPTH} deep")));
    }
    Ok(match v {
        Value::Nil => Json::Null,
        Value::Boolean(b) => Json::Bool(*b),
        Value::Integer(i) => number(*i as f64)
            .ok_or_else(|| bad("is NaN or infinite, which JSON has no value for"))?,
        Value::Number(n) => {
            number(*n).ok_or_else(|| bad("is NaN or infinite, which JSON has no value for"))?
        }
        Value::String(s) => Json::String(
            s.to_str()
                .map_err(|_| bad("is not valid UTF-8"))?
                .to_string(),
        ),
        Value::Table(t) if m.is_some_and(|m| *t == m.null) => Json::Null,
        Value::Table(t) => {
            let ptr = t.to_pointer() as usize;
            if !seen.insert(ptr) {
                return Err(bad("contains itself"));
            }
            let out = table_json(m, t, path, depth, seen)?;
            seen.remove(&ptr);
            out
        }
        other => {
            return Err(bad(&format!(
                "is a {}, which JSON cannot hold",
                other.type_name()
            )));
        }
    })
}

fn number(n: f64) -> Option<Json> {
    if !n.is_finite() {
        return None;
    }
    if n.fract() == 0.0 && n.abs() < EXACT {
        return Some(Json::Number((n as i64).into()));
    }
    Number::from_f64(n).map(Json::Number)
}

fn table_json(
    m: Option<&Markers>,
    t: &Table,
    path: &str,
    depth: usize,
    seen: &mut HashSet<usize>,
) -> mlua::Result<Json> {
    let marked = m.is_some_and(|m| t.metatable().is_some_and(|mt| mt == m.array));
    let mut ints = Vec::new();
    let mut strs = Vec::new();
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair?;
        match k {
            Value::Integer(i) if i >= 1 => ints.push((i as usize, v)),
            Value::Number(n) if n.fract() == 0.0 && n >= 1.0 && n < EXACT => {
                ints.push((n as usize, v))
            }
            Value::String(s) if !marked => {
                let key = s.to_str().map_err(|_| {
                    Error::runtime(format!(
                        "torch.json.encode: {path} has a key that is not UTF-8"
                    ))
                })?;
                strs.push((key.to_string(), v));
            }
            other => {
                return Err(Error::runtime(format!(
                    "torch.json.encode: {path} has the key {}, which is neither 1..n nor a string",
                    other
                        .to_string()
                        .unwrap_or_else(|_| other.type_name().to_string())
                )));
            }
        }
    }
    if !ints.is_empty() && !strs.is_empty() {
        return Err(Error::runtime(format!(
            "torch.json.encode: {path} mixes array and object keys"
        )));
    }
    if !ints.is_empty() || marked {
        ints.sort_by_key(|(i, _)| *i);
        if ints.iter().enumerate().any(|(n, (i, _))| *i != n + 1) {
            return Err(Error::runtime(format!(
                "torch.json.encode: {path} has a hole; an array's keys are 1..n"
            )));
        }
        let mut out = Vec::with_capacity(ints.len());
        for (i, v) in &ints {
            out.push(to_json_with(
                m,
                v,
                &format!("{path}[{i}]"),
                depth + 1,
                seen,
            )?);
        }
        return Ok(Json::Array(out));
    }
    let mut out = Map::new();
    for (k, v) in &strs {
        out.insert(
            k.clone(),
            to_json_with(m, v, &format!("{path}.{k}"), depth + 1, seen)?,
        );
    }
    Ok(Json::Object(out))
}

pub(super) fn plain_to_lua(lua: &Lua, j: &Json) -> mlua::Result<Value> {
    to_lua_with(lua, None, j, 0)
}

fn to_lua(lua: &Lua, m: &Markers, j: &Json, depth: usize) -> mlua::Result<Value> {
    to_lua_with(lua, Some(m), j, depth)
}

fn to_lua_with(lua: &Lua, m: Option<&Markers>, j: &Json, depth: usize) -> mlua::Result<Value> {
    if depth > MAX_DEPTH {
        return Err(Error::runtime(format!(
            "torch.json.decode: nested more than {MAX_DEPTH} deep"
        )));
    }
    Ok(match j {
        Json::Null => m.map_or(Value::Nil, |m| Value::Table(m.null.clone())),
        Json::Bool(b) => Value::Boolean(*b),
        Json::Number(n) => {
            let exact = n
                .as_i64()
                .map(|i| (i as f64).abs() < EXACT)
                .or_else(|| n.as_u64().map(|u| (u as f64) < EXACT));
            if exact == Some(false) && m.is_some() {
                return Err(Error::runtime(format!(
                    "torch.json.decode: {n} is past 2^53, which a Lua number cannot hold exactly"
                )));
            }
            Value::Number(n.as_f64().unwrap_or(0.0))
        }
        Json::String(s) => Value::String(lua.create_string(s)?),
        Json::Array(a) => {
            let t = lua.create_table_with_capacity(a.len(), 0)?;
            for (i, v) in a.iter().enumerate() {
                t.raw_set(i + 1, to_lua_with(lua, m, v, depth + 1)?)?;
            }
            if let Some(m) = m {
                t.set_metatable(Some(m.array.clone()))?;
            }
            Value::Table(t)
        }
        Json::Object(o) => {
            let t = lua.create_table_with_capacity(0, o.len())?;
            for (k, v) in o {
                t.raw_set(k.as_str(), to_lua_with(lua, m, v, depth + 1)?)?;
            }
            Value::Table(t)
        }
    })
}
