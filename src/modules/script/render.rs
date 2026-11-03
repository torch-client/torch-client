use std::sync::{Arc, Mutex, PoisonError};

use mlua::{Error, Lua, Table, UserData, UserDataMethods, Value};

use super::api::Ctx;
use super::game::game_fn;
use crate::renderer::world_draw::{self, Node, Shape};

const OWNER: &str = "lua";

const PER_MODULE: usize = 4096;
const LINE_WIDTH: f64 = 0.05;
const MAX_WIDTH: f64 = 1.0;
const DEFAULT_COLOR: u32 = 0x40FF_FFFF;

struct Entry {
    id: u64,
    owner: usize,
    node: Option<Node>,
}

struct Store {
    next: u64,
    entries: Vec<Entry>,
    counts: Vec<u32>,
    dirty: bool,
}

static STORE: Mutex<Store> = Mutex::new(Store {
    next: 0,
    entries: Vec::new(),
    counts: Vec::new(),
    dirty: false,
});

impl Store {
    fn find(&mut self, id: u64) -> Option<&mut Entry> {
        let i = self.entries.binary_search_by_key(&id, |e| e.id).ok()?;
        Some(&mut self.entries[i]).filter(|e| e.node.is_some())
    }

    fn count(&mut self, owner: usize) -> &mut u32 {
        if self.counts.len() <= owner {
            self.counts.resize(owner + 1, 0);
        }
        &mut self.counts[owner]
    }
}

fn store() -> std::sync::MutexGuard<'static, Store> {
    STORE.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn publish() {
    let mut s = store();
    if !std::mem::take(&mut s.dirty) {
        return;
    }
    let mut nodes = Vec::with_capacity(s.entries.len());
    s.entries
        .retain(|e| e.node.inspect(|n| nodes.push(*n)).is_some());
    world_draw::set(OWNER, nodes);
}

pub(super) fn drop_owner(owner: usize) {
    let mut s = store();
    if s.counts.get(owner).is_none_or(|&n| n == 0) {
        return;
    }
    s.counts[owner] = 0;
    for e in s.entries.iter_mut().filter(|e| e.owner == owner) {
        e.node = None;
    }
    s.dirty = true;
}

pub(super) fn clear() {
    let mut s = store();
    s.entries.clear();
    s.counts.clear();
    s.dirty = false;
    world_draw::clear(OWNER);
}

struct Handle(u64);

impl UserData for Handle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set", |_, h, spec: Table| {
            let mut s = store();
            let Some(e) = s.find(h.0) else {
                return Ok(false);
            };
            e.node = Some(read(&spec, e.node, "node:set")?);
            s.dirty = true;
            Ok(true)
        });
        methods.add_method("remove", |_, h, ()| {
            let mut s = store();
            let Some(e) = s.find(h.0) else {
                return Ok(false);
            };
            e.node = None;
            let owner = e.owner;
            *s.count(owner) -= 1;
            s.dirty = true;
            Ok(true)
        });
        methods.add_method("exists", |_, h, ()| Ok(store().find(h.0).is_some()));
    }
}

pub(super) fn number_at(t: &Table, name: &str, i: usize) -> mlua::Result<Option<f64>> {
    match t.raw_get::<Option<f64>>(name)? {
        Some(n) => Ok(Some(n)),
        None => t.raw_get(i),
    }
}

pub(super) fn point(v: Value, key: &str, what: &str) -> mlua::Result<[f64; 3]> {
    let bad = || {
        Error::runtime(format!(
            "{what}: `{key}` is {{ x, y, z }} with finite numbers"
        ))
    };
    let Value::Table(t) = v else {
        return Err(bad());
    };
    let mut out = [0.0; 3];
    for (i, (name, slot)) in ["x", "y", "z"].iter().zip(&mut out).enumerate() {
        let n = number_at(&t, name, i + 1)?.ok_or_else(bad)?;
        if !n.is_finite() {
            return Err(bad());
        }
        *slot = n;
    }
    Ok(out)
}

pub(super) fn known(spec: &Table, fields: &[&str], what: &str) -> mlua::Result<()> {
    for pair in spec.pairs::<String, Value>() {
        let (k, _) = pair?;
        if !fields.contains(&k.as_str()) {
            return Err(Error::runtime(format!(
                "{what}: `{k}` is not one of {}{}",
                fields.join(", "),
                super::api::suggest(&k, fields)
            )));
        }
    }
    Ok(())
}

const BOX_FIELDS: [&str; 5] = ["min", "max", "color", "outline", "through"];
const LINE_FIELDS: [&str; 5] = ["from", "to", "width", "color", "through"];

fn read(spec: &Table, base: Option<Node>, what: &str) -> mlua::Result<Node> {
    let fields: &[&str] = match base.map(|n| n.shape) {
        Some(Shape::Line { .. }) => &LINE_FIELDS,
        _ => &BOX_FIELDS,
    };
    known(spec, fields, what)?;
    let opt_point = |k: &str| -> mlua::Result<Option<[f64; 3]>> {
        match spec.raw_get::<Value>(k)? {
            Value::Nil => Ok(None),
            v => point(v, k, what).map(Some),
        }
    };
    let corners = |lo: [f64; 3], hi: [f64; 3]| Shape::Box {
        min: [lo[0].min(hi[0]), lo[1].min(hi[1]), lo[2].min(hi[2])],
        max: [lo[0].max(hi[0]), lo[1].max(hi[1]), lo[2].max(hi[2])],
    };
    let shape = match base.map(|n| n.shape) {
        None => {
            let (Some(lo), Some(hi)) = (opt_point("min")?, opt_point("max")?) else {
                return Err(Error::runtime(format!("{what}: a box needs min and max")));
            };
            corners(lo, hi)
        }
        Some(Shape::Box { min, max }) => corners(
            opt_point("min")?.unwrap_or(min),
            opt_point("max")?.unwrap_or(max),
        ),
        Some(Shape::Line { from, to, width }) => Shape::Line {
            from: opt_point("from")?.unwrap_or(from),
            to: opt_point("to")?.unwrap_or(to),
            width: width_of(spec, width, what)?,
        },
    };
    let argb = match spec.raw_get::<Option<f64>>("color")? {
        Some(c) if (0.0..=f64::from(u32::MAX)).contains(&c) && c.fract() == 0.0 => c as u32,
        Some(c) => {
            return Err(Error::runtime(format!(
                "{what}: a colour is 0xAARRGGBB, not {c}"
            )));
        }
        None => base.map_or(DEFAULT_COLOR, |n| n.argb),
    };
    let flag = |k: &str, default: bool| -> mlua::Result<bool> {
        Ok(spec.raw_get::<Option<bool>>(k)?.unwrap_or(default))
    };
    Ok(Node {
        shape,
        argb,
        outline: flag("outline", base.map_or(true, |n| n.outline))?,
        through: flag("through", base.is_some_and(|n| n.through))?,
    })
}

fn width_of(spec: &Table, default: f64, what: &str) -> mlua::Result<f64> {
    match spec.raw_get::<Option<f64>>("width")? {
        None => Ok(default),
        Some(w) if w > 0.0 && w <= MAX_WIDTH => Ok(w),
        Some(w) => Err(Error::runtime(format!(
            "{what}: width is above 0 and at most {MAX_WIDTH}, not {w}"
        ))),
    }
}

fn add(ctx: &Ctx, what: &str, node: Node) -> mlua::Result<Handle> {
    let (k, _) = ctx.caller(what)?;
    let mut s = store();
    let count = s.count(k);
    if *count as usize >= PER_MODULE {
        return Err(Error::runtime(format!(
            "{what}: more than {PER_MODULE} nodes in one module"
        )));
    }
    *count += 1;
    s.next += 1;
    let id = s.next;
    s.entries.push(Entry {
        id,
        owner: k,
        node: Some(node),
    });
    s.dirty = true;
    Ok(Handle(id))
}

pub(super) fn table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set(
        "box",
        game_fn(lua, ctx, "torch.render.box", |_, ctx, spec: Table| {
            let what = "torch.render.box";
            add(ctx, what, read(&spec, None, what)?)
        })?,
    )?;
    t.raw_set(
        "line",
        game_fn(lua, ctx, "torch.render.line", |_, ctx, spec: Table| {
            let what = "torch.render.line";
            let from = point(spec.raw_get("from")?, "from", what)?;
            let to = point(spec.raw_get("to")?, "to", what)?;
            let base = Node {
                shape: Shape::Line {
                    from,
                    to,
                    width: LINE_WIDTH,
                },
                argb: 0xFFFF_FFFF,
                outline: false,
                through: false,
            };
            add(ctx, what, read(&spec, Some(base), what)?)
        })?,
    )?;
    t.raw_set("capture", super::capture::function(lua, ctx)?)?;
    t.raw_set(
        "clear",
        game_fn(lua, ctx, "torch.render.clear", |_, ctx, ()| {
            let (k, _) = ctx.caller("torch.render.clear")?;
            drop_owner(k);
            Ok(())
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}
