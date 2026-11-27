use std::sync::{Arc, Mutex, PoisonError};

use mlua::{AnyUserData, Error, Lua, Table, Value};

use super::api::Ctx;
use super::game::game_fn;
use crate::modules::builder::{self, State};

static OWNER: Mutex<Option<usize>> = Mutex::new(None);

fn owner() -> std::sync::MutexGuard<'static, Option<usize>> {
    OWNER.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn drop_owner(k: usize) {
    let mut o = owner();
    if *o == Some(k) {
        *o = None;
        drop(o);
        builder::stop();
    }
}

pub(super) fn clear() {
    if owner().take().is_some() {
        builder::stop();
    }
}

fn state_name(s: State) -> &'static str {
    match s {
        State::Scanning => "scanning",
        State::Placing => "placing",
        State::Breaking => "breaking",
        State::Walking => "walking",
        State::Paused => "paused",
        State::Waiting => "waiting",
        State::Done => "done",
    }
}

fn regions(handles: Value, owner: usize, what: &str) -> mlua::Result<Vec<builder::Region>> {
    let one = |ud: &AnyUserData, n: Option<usize>| -> mlua::Result<builder::Region> {
        let at = n.map_or(String::new(), |n| format!(" {n}"));
        let id = super::ghost::handle_id(ud).ok_or_else(|| {
            Error::runtime(format!(
                "{what}: handle{at} is not a handle from torch.ghost.add"
            ))
        })?;
        super::ghost::region_for_build(id, owner).ok_or_else(|| {
            Error::runtime(format!(
                "{what}: region{at} was removed or belongs to another module"
            ))
        })
    };
    match handles {
        Value::UserData(ud) => Ok(vec![one(&ud, None)?]),
        Value::Table(t) => {
            let n = t.raw_len();
            let mut out = Vec::with_capacity(n);
            for i in 1..=n {
                match t.raw_get::<Value>(i)? {
                    Value::UserData(ud) => out.push(one(&ud, Some(i))?),
                    other => {
                        return Err(Error::runtime(format!(
                            "{what}: handle {i} is a {}, not a handle from torch.ghost.add",
                            other.type_name()
                        )));
                    }
                }
            }
            Ok(out)
        }
        other => Err(Error::runtime(format!(
            "{what} takes a handle from torch.ghost.add or a list of them, not a {}",
            other.type_name()
        ))),
    }
}

fn start(ctx: &Ctx, handles: Value) -> mlua::Result<(bool, Option<String>)> {
    let what = "torch.build.start";
    let (k, _) = ctx.caller(what)?;
    ctx.bot(what)?;
    let regions = regions(handles, k, what)?;
    match builder::start(regions) {
        Ok(()) => {
            *owner() = Some(k);
            Ok((true, None))
        }
        Err(why) => Ok((false, Some(why))),
    }
}

fn status(lua: &Lua) -> mlua::Result<Option<Table>> {
    let Some(st) = builder::status() else {
        return Ok(None);
    };
    let t = lua.create_table()?;
    t.raw_set("state", state_name(st.state))?;
    t.raw_set("note", st.note)?;
    t.raw_set("placed", st.placed)?;
    t.raw_set("broken", st.broken)?;
    t.raw_set("left", st.left)?;
    t.raw_set("skipped", st.skipped)?;
    t.raw_set("total", st.total as f64)?;
    let missing = lua.create_table_with_capacity(0, st.missing.len())?;
    for (item, n) in st.missing {
        missing.raw_set(item.to_str(), n)?;
    }
    t.raw_set("missing", missing)?;
    if let Some(p) = st.target {
        let target = lua.create_table_with_capacity(0, 3)?;
        target.raw_set("x", p.x)?;
        target.raw_set("y", p.y)?;
        target.raw_set("z", p.z)?;
        t.raw_set("target", target)?;
    }
    t.raw_set("on", builder::enabled())?;
    Ok(Some(t))
}

pub(super) fn table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set(
        "start",
        game_fn(lua, ctx, "torch.build.start", |_, ctx, handles: Value| {
            start(ctx, handles)
        })?,
    )?;
    for (name, on) in [("pause", true), ("resume", false)] {
        t.raw_set(
            name,
            game_fn(
                lua,
                ctx,
                if on {
                    "torch.build.pause"
                } else {
                    "torch.build.resume"
                },
                move |_, _, ()| {
                    builder::pause(on);
                    Ok(())
                },
            )?,
        )?;
    }
    t.raw_set(
        "stop",
        game_fn(lua, ctx, "torch.build.stop", |_, _, ()| {
            *owner() = None;
            builder::stop();
            Ok(())
        })?,
    )?;
    t.raw_set(
        "status",
        game_fn(lua, ctx, "torch.build.status", |lua, _, ()| status(lua))?,
    )?;
    t.set_readonly(true);
    Ok(t)
}
