use std::collections::{BTreeMap, HashMap, HashSet};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::Duration;

use azalea::Client;
use azalea::block::{BlockState, BlockTrait};
use azalea_core::position::{BlockPos, ChunkPos};
use azalea_registry::builtin::BlockKind;
use bevy::prelude::Mesh;
use mlua::{AnyUserData, Error, Lua, Table, UserData, UserDataMethods, Value};

use super::api::Ctx;
use super::game::game_fn;
use super::nbt::Indices;
use crate::blocks::{BakedBlock, Blocks};
use crate::entities::blockmodel::{push_at, state_model};
use crate::entities::quads::QuadMesh;
use crate::platform::time::Instant;

const PER_MODULE: usize = 64;
const MAX_PALETTE: usize = 1 << 16;
const MAX_SIDE: i64 = 1 << 16;
const MAX_VOLUME: i64 = 1 << 31;
const MAX_COORD: i64 = 30_000_000;
const MAX_RANGE: i64 = 32;
const DEFAULT_RANGE: i32 = 8;
const DEFAULT_ALPHA: f64 = 0.5;
const BUDGET: Duration = Duration::from_millis(4);
const UNKNOWN_LISTED: usize = 16;
const DIRS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];
const WRONG_INFLATE: f32 = 0.003;
const GRID: i32 = 18;

pub type Key = (u64, [i32; 3]);

pub enum Change {
    Set {
        key: Key,
        ghost: Option<Mesh>,
        wrong: Option<Mesh>,
        alpha: u8,
    },
    Remove(Key),
    Clear,
}

struct Region {
    owner: usize,
    origin: [i32; 3],
    size: [i32; 3],
    blocks: Indices,
    palette: Vec<u16>,
    alpha: u8,
    range: i32,
}

struct Built {
    fresh: bool,
    missing: Vec<(u16, u32)>,
    wrong: u32,
}

#[derive(Default)]
struct Store {
    next: u64,
    regions: BTreeMap<u64, Region>,
    built: HashMap<Key, Built>,
    generation: u64,
    wanted: Vec<Key>,
    wanted_for: Option<([i32; 2], u32, u64)>,
}

impl Store {
    fn forget_region(&mut self, id: u64, changes: &mut Vec<Change>) {
        self.built.retain(|k, _| {
            let keep = k.0 != id;
            if !keep {
                changes.push(Change::Remove(*k));
            }
            keep
        });
        self.generation += 1;
    }
}

static STORE: LazyLock<Mutex<Store>> = LazyLock::new(Mutex::default);
static CHANGES: Mutex<Vec<Change>> = Mutex::new(Vec::new());
static DIRTY: Mutex<Vec<[i32; 3]>> = Mutex::new(Vec::new());
static DIRTY_COLUMNS: Mutex<Vec<[i32; 2]>> = Mutex::new(Vec::new());
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn take_changes() -> Vec<Change> {
    std::mem::take(&mut *CHANGES.lock().unwrap_or_else(PoisonError::into_inner))
}

pub fn block_changed(x: i32, y: i32, z: i32) {
    if !ACTIVE.load(Relaxed) {
        return;
    }
    let s = [x >> 4, y >> 4, z >> 4];
    let mut d = DIRTY.lock().unwrap_or_else(PoisonError::into_inner);
    let mut push = |sec: [i32; 3]| {
        if !d.iter().rev().take(RECENT).any(|&q| q == sec) {
            d.push(sec);
        }
    };
    push(s);
    for (axis, v) in [x, y, z].into_iter().enumerate() {
        let mut n = s;
        match v & 15 {
            0 => n[axis] -= 1,
            15 => n[axis] += 1,
            _ => continue,
        }
        push(n);
    }
}

const RECENT: usize = 4;

pub fn chunk_changed(cx: i32, cz: i32) {
    if ACTIVE.load(Relaxed) {
        DIRTY_COLUMNS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push([cx, cz]);
    }
}

pub(super) fn drop_owner(owner: usize) {
    let mut s = STORE.lock().unwrap_or_else(PoisonError::into_inner);
    let ids: Vec<u64> = s
        .regions
        .iter()
        .filter(|(_, r)| r.owner == owner)
        .map(|(&id, _)| id)
        .collect();
    if ids.is_empty() {
        return;
    }
    let mut changes = CHANGES.lock().unwrap_or_else(PoisonError::into_inner);
    for id in ids {
        s.regions.remove(&id);
        s.forget_region(id, &mut changes);
    }
    ACTIVE.store(!s.regions.is_empty(), Relaxed);
}

pub(super) fn clear() {
    let mut s = STORE.lock().unwrap_or_else(PoisonError::into_inner);
    *s = Store {
        next: s.next,
        ..Store::default()
    };
    ACTIVE.store(false, Relaxed);
    DIRTY.lock().unwrap_or_else(PoisonError::into_inner).clear();
    DIRTY_COLUMNS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    let mut changes = CHANGES.lock().unwrap_or_else(PoisonError::into_inner);
    changes.clear();
    changes.push(Change::Clear);
}

pub(super) fn tick(bot: &Client) {
    if !ACTIVE.load(Relaxed) {
        return;
    }
    let (Ok(world), Ok(pos)) = (bot.world(), bot.position()) else {
        return;
    };
    let player = [(pos.x.floor() as i32) >> 4, (pos.z.floor() as i32) >> 4];
    let dirty: HashSet<[i32; 3]> =
        std::mem::take(&mut *DIRTY.lock().unwrap_or_else(PoisonError::into_inner))
            .into_iter()
            .collect();
    let columns: HashSet<[i32; 2]> =
        std::mem::take(&mut *DIRTY_COLUMNS.lock().unwrap_or_else(PoisonError::into_inner))
            .into_iter()
            .collect();

    let mut guard = STORE.lock().unwrap_or_else(PoisonError::into_inner);
    let s = &mut *guard;
    if columns.is_empty() && dirty.len() * s.regions.len() < s.built.len() {
        for sec in &dirty {
            for &id in s.regions.keys() {
                if let Some(b) = s.built.get_mut(&(id, *sec)) {
                    b.fresh = false;
                }
            }
        }
    } else if !dirty.is_empty() || !columns.is_empty() {
        for (key, b) in s.built.iter_mut() {
            if dirty.contains(&key.1) || columns.contains(&[key.1[0], key.1[2]]) {
                b.fresh = false;
            }
        }
    }
    let w = world.read();
    let stamp = (
        player,
        crate::client::worldsync::chunk_generation(),
        s.generation,
    );
    if s.wanted_for != Some(stamp) {
        s.wanted = wanted(&s.regions, &w, player);
        s.wanted_for = Some(stamp);
        let keep: HashSet<Key> = s.wanted.iter().copied().collect();
        let mut changes = CHANGES.lock().unwrap_or_else(PoisonError::into_inner);
        s.built.retain(|k, _| {
            let kept = keep.contains(k);
            if !kept {
                changes.push(Change::Remove(*k));
            }
            kept
        });
    }

    let start = Instant::now();
    let blocks = crate::blocks::registry();
    let mut models = Models::default();
    let mut out = Vec::new();
    let Store {
        regions,
        built,
        wanted,
        ..
    } = s;
    for key in wanted.iter() {
        if built.get(key).is_some_and(|b| b.fresh) {
            continue;
        }
        if start.elapsed() > BUDGET {
            break;
        }
        let Some(r) = regions.get(&key.0) else {
            continue;
        };
        let (ghost, wrong, counted) = build(r, key.1, &w, &blocks, &mut models);
        out.push(Change::Set {
            key: *key,
            ghost,
            wrong,
            alpha: r.alpha,
        });
        built.insert(*key, counted);
    }
    CHANGES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .extend(out);
}

fn wanted(regions: &BTreeMap<u64, Region>, w: &azalea_world::World, player: [i32; 2]) -> Vec<Key> {
    let mut loaded: HashMap<[i32; 2], bool> = HashMap::new();
    let mut out: Vec<(i64, Key)> = Vec::new();
    for (&id, r) in regions {
        let lo = r.origin.map(|v| v >> 4);
        let hi = [0, 1, 2].map(|a| (r.origin[a] + r.size[a] - 1) >> 4);
        let (x0, x1) = (
            lo[0].max(player[0] - r.range),
            hi[0].min(player[0] + r.range),
        );
        let (z0, z1) = (
            lo[2].max(player[1] - r.range),
            hi[2].min(player[1] + r.range),
        );
        for cx in x0..=x1 {
            for cz in z0..=z1 {
                let here = *loaded
                    .entry([cx, cz])
                    .or_insert_with(|| w.chunks.get(&ChunkPos::new(cx, cz)).is_some());
                if !here {
                    continue;
                }
                let (dx, dz) = (i64::from(cx - player[0]), i64::from(cz - player[1]));
                for cy in lo[1]..=hi[1] {
                    out.push((dx * dx + dz * dz, (id, [cx, cy, cz])));
                }
            }
        }
    }
    out.sort_by_key(|&(d, k)| (d, k.1[1]));
    out.into_iter().map(|(_, k)| k).collect()
}

#[derive(Default)]
struct Models(HashMap<u16, Option<Arc<BakedBlock>>>);

impl Models {
    fn get(&mut self, blocks: &Blocks, id: u16) -> Option<Arc<BakedBlock>> {
        self.0
            .entry(id)
            .or_insert_with(|| state_model(blocks, u32::from(id)))
            .clone()
    }

    fn solid(&mut self, blocks: &Blocks, id: u16) -> bool {
        id != 0 && self.get(blocks, id).is_some_and(|b| b.is_solid)
    }
}

fn replaceable(s: BlockState) -> bool {
    s.is_air() || matches!(BlockKind::from(s), BlockKind::Water | BlockKind::Lava)
}

fn cell(x: i32, y: i32, z: i32) -> usize {
    ((x + 1) + GRID * ((z + 1) + GRID * (y + 1))) as usize
}

fn build(
    r: &Region,
    sec: [i32; 3],
    w: &azalea_world::World,
    blocks: &Blocks,
    models: &mut Models,
) -> (Option<Mesh>, Option<Mesh>, Built) {
    let base = sec.map(|v| v * 16);
    let end = [0, 1, 2].map(|a| r.origin[a] + r.size[a]);
    let total = (GRID * GRID * GRID) as usize;

    let mut ghost: Vec<Option<u16>> = vec![None; total];
    let (sx, sz) = (r.size[0] as usize, r.size[2] as usize);
    let x0 = (base[0] - 1).max(r.origin[0]);
    let x1 = (base[0] + 17).min(end[0]);
    if x0 < x1 {
        for y in (base[1] - 1).max(r.origin[1])..(base[1] + 17).min(end[1]) {
            for z in (base[2] - 1).max(r.origin[2])..(base[2] + 17).min(end[2]) {
                let row = ((y - r.origin[1]) as usize * sz + (z - r.origin[2]) as usize) * sx;
                let first = row + (x0 - r.origin[0]) as usize;
                let mut x = x0;
                r.blocks.each(first..first + (x1 - x0) as usize, |v| {
                    ghost[cell(x - base[0], y - base[1], z - base[2])] =
                        Some(r.palette.get(v as usize).copied().unwrap_or(0));
                    x += 1;
                });
            }
        }
    }

    let world_at = |x: i32, y: i32, z: i32| {
        w.get_block_state(BlockPos::new(base[0] + x, base[1] + y, base[2] + z))
    };
    let mut seen: Vec<u32> = vec![u32::MAX; total];
    let mut occupant = |x: i32, y: i32, z: i32| -> u16 {
        let i = cell(x, y, z);
        if seen[i] == u32::MAX {
            let ws = world_at(x, y, z);
            let id = match (ghost[i], ws) {
                (Some(g), Some(s)) if g != 0 && s.id() != g && replaceable(s) => g,
                (_, Some(s)) => s.id(),
                _ => 0,
            };
            seen[i] = u32::from(id);
        }
        seen[i] as u16
    };

    let mut ghosts = QuadMesh::new();
    let mut wrong = QuadMesh::new();
    let mut missing: HashMap<u16, u32> = HashMap::new();
    let mut wrong_count = 0;
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let Some(g) = ghost[cell(x, y, z)].filter(|&g| g != 0) else {
                    continue;
                };
                let Some(ws) = world_at(x, y, z) else {
                    continue;
                };
                if ws.id() == g {
                    continue;
                }
                let at = [x as f32, y as f32, z as f32];
                if !replaceable(ws) {
                    cube(&mut wrong, at);
                    wrong_count += 1;
                    continue;
                }
                *missing.entry(g).or_default() += 1;
                let Some(model) = models.get(blocks, g) else {
                    continue;
                };
                for part in &model.parts {
                    let Some(choice) = part.choices.first() else {
                        continue;
                    };
                    for quad in &choice.quads {
                        if let Some(d) = quad.cull {
                            let [dx, dy, dz] = DIRS[d as usize];
                            let n = occupant(x + dx, y + dy, z + dz);
                            if models.solid(blocks, n) {
                                continue;
                            }
                        }
                        push_at(&mut ghosts, quad, at);
                    }
                }
            }
        }
    }
    let mesh = |q: QuadMesh| (!q.is_empty()).then(|| q.finish());
    let counted = Built {
        fresh: true,
        missing: missing.into_iter().collect(),
        wrong: wrong_count,
    };
    (mesh(ghosts), mesh(wrong), counted)
}

fn stats(lua: &Lua, id: u64) -> mlua::Result<Option<Table>> {
    let s = STORE.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(r) = s.regions.get(&id) else {
        return Ok(None);
    };
    let mut by_state: HashMap<u16, u32> = HashMap::new();
    let (mut wrong, mut built) = (0u32, 0usize);
    for (_, b) in s.built.iter().filter(|(k, _)| k.0 == id) {
        built += 1;
        wrong += b.wrong;
        for &(state, n) in &b.missing {
            *by_state.entry(state).or_default() += n;
        }
    }
    let missing = lua.create_table()?;
    for (state, n) in by_state {
        let Ok(state) = BlockState::try_from(u32::from(state)) else {
            continue;
        };
        let name = state.as_block_kind().to_str();
        let before = missing.raw_get::<Option<u32>>(name)?.unwrap_or(0);
        missing.raw_set(name, before + n)?;
    }
    let sections: i64 = (0..3)
        .map(|a| i64::from(((r.origin[a] + r.size[a] - 1) >> 4) - (r.origin[a] >> 4) + 1))
        .product();
    let t = lua.create_table()?;
    t.raw_set("missing", missing)?;
    t.raw_set("wrong", wrong)?;
    t.raw_set("built", built)?;
    t.raw_set("inrange", s.wanted.iter().filter(|k| k.0 == id).count())?;
    t.raw_set("sections", sections)?;
    Ok(Some(t))
}

fn cube(out: &mut QuadMesh, at: [f32; 3]) {
    let (lo, hi) = (-WRONG_INFLATE, 1.0 + WRONG_INFLATE);
    let c = |x: f32, y: f32, z: f32| [at[0] + x, at[1] + y, at[2] + z];
    let faces = [
        (
            [c(lo, lo, lo), c(hi, lo, lo), c(hi, lo, hi), c(lo, lo, hi)],
            [0.0, -1.0, 0.0],
        ),
        (
            [c(lo, hi, lo), c(lo, hi, hi), c(hi, hi, hi), c(hi, hi, lo)],
            [0.0, 1.0, 0.0],
        ),
        (
            [c(lo, lo, lo), c(lo, hi, lo), c(hi, hi, lo), c(hi, lo, lo)],
            [0.0, 0.0, -1.0],
        ),
        (
            [c(lo, lo, hi), c(hi, lo, hi), c(hi, hi, hi), c(lo, hi, hi)],
            [0.0, 0.0, 1.0],
        ),
        (
            [c(lo, lo, lo), c(lo, lo, hi), c(lo, hi, hi), c(lo, hi, lo)],
            [-1.0, 0.0, 0.0],
        ),
        (
            [c(hi, lo, lo), c(hi, hi, lo), c(hi, hi, hi), c(hi, lo, hi)],
            [1.0, 0.0, 0.0],
        ),
    ];
    for (corners, normal) in faces {
        out.untextured(corners, normal);
    }
}

pub(super) fn palette_state(v: &Value) -> mlua::Result<Option<BlockState>> {
    let (name, props): (String, Vec<(String, String)>) = match v {
        Value::String(s) => {
            let s = s.to_str()?.to_string();
            match s.split_once('[') {
                Some((name, rest)) => {
                    let props = rest
                        .trim_end_matches(']')
                        .split(',')
                        .filter_map(|p| p.split_once('='))
                        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                        .collect();
                    (name.to_string(), props)
                }
                None => (s, Vec::new()),
            }
        }
        Value::Table(t) => {
            let Some(name) = t.raw_get::<Option<String>>("Name")? else {
                return Ok(None);
            };
            let mut props = Vec::new();
            if let Some(p) = t.raw_get::<Option<Table>>("Properties")? {
                for pair in p.pairs::<String, String>() {
                    props.push(pair?);
                }
            }
            (name, props)
        }
        _ => return Ok(None),
    };
    let bare = name.strip_prefix("minecraft:").unwrap_or(&name);
    let Ok(kind) = BlockKind::from_str(bare) else {
        return Ok(None);
    };
    let mut block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(BlockState::from(kind));
    for (k, v) in &props {
        let _ = block.set_property(k, v);
    }
    Ok(Some(block.as_block_state()))
}

fn entry_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.to_string_lossy(),
        Value::Table(t) => t
            .raw_get::<Option<String>>("Name")
            .ok()
            .flatten()
            .unwrap_or_else(|| "?".into()),
        other => other.type_name().to_string(),
    }
}

fn int3(v: Value, key: &str, what: &str) -> mlua::Result<[i64; 3]> {
    let bad = || {
        Error::runtime(format!(
            "{what}: `{key}` is {{ x, y, z }} with whole numbers"
        ))
    };
    let Value::Table(t) = v else {
        return Err(bad());
    };
    let mut out = [0; 3];
    for (i, (name, slot)) in ["x", "y", "z"].iter().zip(&mut out).enumerate() {
        let n = super::render::number_at(&t, name, i + 1)?.ok_or_else(bad)?;
        if !n.is_finite() || n.fract() != 0.0 {
            return Err(bad());
        }
        *slot = n as i64;
    }
    Ok(out)
}

fn origin_of(v: Value, what: &str) -> mlua::Result<[i32; 3]> {
    let o = int3(v, "origin", what)?;
    if o.iter().any(|c| c.abs() > MAX_COORD) {
        return Err(Error::runtime(format!(
            "{what}: origin is within {MAX_COORD} of 0"
        )));
    }
    Ok(o.map(|c| c as i32))
}

fn alpha_of(spec: &Table, what: &str) -> mlua::Result<Option<u8>> {
    match spec.raw_get::<Option<f64>>("alpha")? {
        None => Ok(None),
        Some(a) if (0.0..=1.0).contains(&a) => Ok(Some((a * 255.0).round() as u8)),
        Some(a) => Err(Error::runtime(format!("{what}: alpha is 0 to 1, not {a}"))),
    }
}

fn range_of(spec: &Table, what: &str) -> mlua::Result<Option<i32>> {
    match spec.raw_get::<Option<f64>>("range")? {
        None => Ok(None),
        Some(r) if r.fract() == 0.0 && (0.0..=MAX_RANGE as f64).contains(&r) => Ok(Some(r as i32)),
        Some(r) => Err(Error::runtime(format!(
            "{what}: range is 0 to {MAX_RANGE} chunks, not {r}"
        ))),
    }
}

fn check_fields(spec: &Table, fields: &[&str], what: &str) -> mlua::Result<()> {
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

const ADD_FIELDS: [&str; 8] = [
    "origin", "size", "blocks", "bits", "spanning", "palette", "alpha", "range",
];
const SET_FIELDS: [&str; 3] = ["origin", "alpha", "range"];

fn add(lua: &Lua, ctx: &Ctx, spec: Table) -> mlua::Result<(Handle, Table)> {
    let what = "torch.ghost.add";
    let (k, _) = ctx.caller(what)?;
    check_fields(&spec, &ADD_FIELDS, what)?;
    let origin = origin_of(spec.raw_get("origin")?, what)?;
    let size = int3(spec.raw_get("size")?, "size", what)?;
    if size.iter().any(|&s| !(1..=MAX_SIDE).contains(&s)) {
        return Err(Error::runtime(format!(
            "{what}: each side of size is 1 to {MAX_SIDE}"
        )));
    }
    let volume = size[0] * size[1] * size[2];
    if volume > MAX_VOLUME {
        return Err(Error::runtime(format!(
            "{what}: more than {MAX_VOLUME} blocks"
        )));
    }
    let bits = match spec.raw_get::<Option<f64>>("bits")? {
        None => None,
        Some(b) if b.fract() == 0.0 && (1.0..=32.0).contains(&b) => Some(b as u32),
        Some(b) => return Err(Error::runtime(format!("{what}: bits is 1 to 32, not {b}"))),
    };
    let spanning = spec.raw_get::<Option<bool>>("spanning")?.unwrap_or(false);
    let Some(ud) = spec.raw_get::<Option<AnyUserData>>("blocks")? else {
        return Err(Error::runtime(format!(
            "{what}: blocks is an array handle from torch.nbt"
        )));
    };
    let blocks = Indices::new(&ud, bits, spanning, volume as usize)
        .map_err(|e| Error::runtime(format!("{what}: {e}")))?;
    let Some(list) = spec.raw_get::<Option<Table>>("palette")? else {
        return Err(Error::runtime(format!(
            "{what}: palette is a list of block states"
        )));
    };
    let n = list.raw_len();
    if n > MAX_PALETTE {
        return Err(Error::runtime(format!(
            "{what}: more than {MAX_PALETTE} palette entries"
        )));
    }
    let mut palette = Vec::with_capacity(n);
    let unknown = lua.create_table()?;
    let mut missed = 0usize;
    for i in 1..=n {
        let v: Value = list.raw_get(i)?;
        match palette_state(&v)? {
            Some(s) => palette.push(s.id()),
            None => {
                palette.push(0);
                let text = entry_text(&v);
                let air = matches!(text.as_str(), "minecraft:air" | "air");
                if !air {
                    missed += 1;
                    if missed <= UNKNOWN_LISTED {
                        unknown.raw_push(text)?;
                    }
                }
            }
        }
    }
    if missed > UNKNOWN_LISTED {
        unknown.raw_set("more", missed - UNKNOWN_LISTED)?;
    }
    let region = Region {
        owner: k,
        origin,
        size: size.map(|s| s as i32),
        blocks,
        palette,
        alpha: alpha_of(&spec, what)?.unwrap_or((DEFAULT_ALPHA * 255.0) as u8),
        range: range_of(&spec, what)?.unwrap_or(DEFAULT_RANGE),
    };
    let mut s = STORE.lock().unwrap_or_else(PoisonError::into_inner);
    if s.regions.values().filter(|r| r.owner == k).count() >= PER_MODULE {
        return Err(Error::runtime(format!(
            "{what}: more than {PER_MODULE} regions in one module"
        )));
    }
    s.next += 1;
    let id = s.next;
    s.regions.insert(id, region);
    s.generation += 1;
    ACTIVE.store(true, Relaxed);
    Ok((Handle(id), unknown))
}

struct Handle(u64);

impl UserData for Handle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set", |_, h, spec: Table| {
            let what = "ghost:set";
            check_fields(&spec, &SET_FIELDS, what)?;
            let origin = match spec.raw_get::<Value>("origin")? {
                Value::Nil => None,
                v => Some(origin_of(v, what)?),
            };
            let (alpha, range) = (alpha_of(&spec, what)?, range_of(&spec, what)?);
            let mut s = STORE.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(r) = s.regions.get_mut(&h.0) else {
                return Ok(false);
            };
            let moved =
                origin.is_some_and(|o| o != r.origin) || alpha.is_some_and(|a| a != r.alpha);
            r.origin = origin.unwrap_or(r.origin);
            r.alpha = alpha.unwrap_or(r.alpha);
            r.range = range.unwrap_or(r.range);
            if moved {
                let mut changes = CHANGES.lock().unwrap_or_else(PoisonError::into_inner);
                s.forget_region(h.0, &mut changes);
            } else {
                s.generation += 1;
            }
            Ok(true)
        });
        methods.add_method("remove", |_, h, ()| {
            let mut s = STORE.lock().unwrap_or_else(PoisonError::into_inner);
            if s.regions.remove(&h.0).is_none() {
                return Ok(false);
            }
            let mut changes = CHANGES.lock().unwrap_or_else(PoisonError::into_inner);
            s.forget_region(h.0, &mut changes);
            ACTIVE.store(!s.regions.is_empty(), Relaxed);
            Ok(true)
        });
        methods.add_method("stats", |lua, h, ()| stats(lua, h.0));
        methods.add_method("exists", |_, h, ()| {
            Ok(STORE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .regions
                .contains_key(&h.0))
        });
    }
}

pub(super) fn table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set(
        "add",
        game_fn(lua, ctx, "torch.ghost.add", |lua, ctx, spec: Table| {
            add(lua, ctx, spec)
        })?,
    )?;
    t.raw_set(
        "clear",
        game_fn(lua, ctx, "torch.ghost.clear", |_, ctx, ()| {
            let (k, _) = ctx.caller("torch.ghost.clear")?;
            drop_owner(k);
            Ok(())
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}
