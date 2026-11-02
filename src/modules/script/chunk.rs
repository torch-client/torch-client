use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError};

use azalea_core::heightmap_kind::HeightmapKind;
use azalea_core::position::{ChunkPos, ChunkSectionBiomePos, ChunkSectionBlockPos};
use azalea_registry::DataRegistry;
use azalea_world::Section;
use azalea_world::heightmap::Heightmap;
use mlua::{Error, Lua, Table, UserData, UserDataFields, UserDataMethods, Value};

use super::api::{BLOCKCHANGE, CHUNK, Ctx};
use super::game::{self, game_fn};

const PER_MODULE: usize = 64;
const MAX_COLUMN: i64 = 4096;
const QUEUE_CAP: usize = 8192;
const ID_BYTES: usize = 2;
const SECTION: i32 = 16;
const BIOME_CELL: i32 = 4;

struct Snapshot {
    min_y: i32,
    height: u32,
    sections: Box<[Section]>,
    surface: Option<Heightmap>,
    computed: OnceLock<Box<[i32]>>,
    biomes: Arc<Vec<String>>,
}

impl Snapshot {
    fn state(&self, x: u8, y: i32, z: u8) -> u16 {
        let i = y - self.min_y;
        let Some(section) = usize::try_from(i / SECTION)
            .ok()
            .filter(|_| i >= 0)
            .and_then(|s| self.sections.get(s))
        else {
            return 0;
        };
        if section.block_count == 0 {
            return 0;
        }
        section
            .get_block_state(ChunkSectionBlockPos {
                x,
                y: (i % SECTION) as u8,
                z,
            })
            .id()
    }

    fn height(&self, x: u8, z: u8) -> i32 {
        if let Some(h) = &self.surface {
            return h.get_first_available(x, z);
        }
        let all = self.computed.get_or_init(|| {
            let top = self.min_y + self.height as i32;
            (0..256u16)
                .map(|i| {
                    let (x, z) = ((i % 16) as u8, (i / 16) as u8);
                    (self.min_y..top)
                        .rev()
                        .find(|&y| self.state(x, y, z) != 0)
                        .map_or(self.min_y, |y| y + 1)
                })
                .collect()
        });
        all[usize::from(z) * 16 + usize::from(x)]
    }
}

static STORE: Mutex<BTreeMap<u64, (usize, Arc<Snapshot>)>> = Mutex::new(BTreeMap::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

pub(super) fn clear() {
    STORE.lock().unwrap_or_else(PoisonError::into_inner).clear();
    let mut q = QUEUES.lock().unwrap_or_else(PoisonError::into_inner);
    *q = Queues::default();
}

struct Handle {
    key: u64,
    cx: i32,
    cz: i32,
}

impl Drop for Handle {
    fn drop(&mut self) {
        STORE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
    }
}

fn snapshot(key: u64, what: &str) -> mlua::Result<Arc<Snapshot>> {
    STORE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .map(|(_, s)| s.clone())
        .ok_or_else(|| {
            Error::runtime(format!(
                "{what}: this chunk handle was freed (h:release, a reload or leaving the world)"
            ))
        })
}

fn local(v: f64, what: &str) -> mlua::Result<u8> {
    if v.is_finite() && v.fract() == 0.0 && (0.0..16.0).contains(&v) {
        Ok(v as u8)
    } else {
        Err(Error::runtime(format!(
            "{what}: x and z are 0 to 15 within the chunk, not {v}"
        )))
    }
}

fn whole(v: f64, what: &str) -> mlua::Result<i64> {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e9 {
        Ok(v as i64)
    } else {
        Err(Error::runtime(format!(
            "{what} takes whole numbers, not {v}"
        )))
    }
}

impl UserData for Handle {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("cx", |_, h| Ok(h.cx));
        fields.add_field_method_get("cz", |_, h| Ok(h.cz));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "column",
            |lua, h, (x, z, ylo, yhi, buf): (f64, f64, f64, f64, Option<mlua::Buffer>)| {
                let what = "h:column";
                let (x, z) = (local(x, what)?, local(z, what)?);
                let (ylo, yhi) = (whole(ylo, what)?, whole(yhi, what)?);
                let count = yhi - ylo + 1;
                if !(1..=MAX_COLUMN).contains(&count) {
                    return Err(Error::runtime(format!(
                        "{what}: ylo to yhi is 1 to {MAX_COLUMN} blocks, not {ylo} to {yhi}"
                    )));
                }
                let s = snapshot(h.key, what)?;
                let mut bytes = Vec::with_capacity(count as usize * ID_BYTES);
                for y in ylo..=yhi {
                    bytes.extend_from_slice(&s.state(x, y as i32, z).to_le_bytes());
                }
                let buf = match buf.filter(|b| b.len() >= bytes.len()) {
                    Some(b) => b,
                    None => lua.create_buffer_with_capacity(bytes.len())?,
                };
                buf.write_bytes(0, &bytes);
                Ok((buf, count))
            },
        );
        methods.add_method("height", |_, h, (x, z): (f64, f64)| {
            let what = "h:height";
            let (x, z) = (local(x, what)?, local(z, what)?);
            Ok(snapshot(h.key, what)?.height(x, z))
        });
        methods.add_method("biome", |_, h, (x, y, z): (f64, f64, f64)| {
            let what = "h:biome";
            let (x, z) = (local(x, what)?, local(z, what)?);
            let y = whole(y, what)? as i32;
            let s = snapshot(h.key, what)?;
            let i = y - s.min_y;
            let Some(section) = usize::try_from(i / SECTION)
                .ok()
                .filter(|_| i >= 0)
                .and_then(|n| s.sections.get(n))
            else {
                return Ok(None);
            };
            let pos = ChunkSectionBiomePos {
                x: x / BIOME_CELL as u8,
                y: ((i % SECTION) / BIOME_CELL) as u8,
                z: z / BIOME_CELL as u8,
            };
            let id = section.get_biome(pos).protocol_id() as usize;
            Ok(s.biomes
                .get(id)
                .map(|n| format!("minecraft:{}", n.strip_prefix("minecraft:").unwrap_or(n))))
        });
        methods.add_method("miny", |_, h, ()| Ok(snapshot(h.key, "h:miny")?.min_y));
        methods.add_method("maxy", |_, h, ()| {
            let s = snapshot(h.key, "h:maxy")?;
            Ok(s.min_y + s.height as i32 - 1)
        });
        methods.add_method("release", |_, h, ()| {
            Ok(STORE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&h.key)
                .is_some())
        });
    }
}

fn biome_names(registries: &azalea_core::registry_holder::RegistryHolder) -> Vec<String> {
    let registry = azalea::Identifier::from("worldgen/biome");
    let mut out = Vec::new();
    while let Some(ident) = registries.protocol_id_to_identifier(registry.clone(), out.len() as u32)
    {
        out.push(ident.to_string());
    }
    out
}

pub(super) fn add_world(lua: &Lua, ctx: &Arc<Ctx>, world: &Table) -> mlua::Result<()> {
    world.raw_set(
        "chunk",
        game_fn(
            lua,
            ctx,
            "torch.world.chunk",
            |lua, ctx, (cx, cz): (f64, f64)| {
                let what = "torch.world.chunk";
                let (cx, cz) = (whole(cx, what)? as i32, whole(cz, what)? as i32);
                let (k, _) = ctx.caller(what)?;
                let held = STORE
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .values()
                    .filter(|(o, _)| *o == k)
                    .count();
                if held >= PER_MODULE {
                    return Ok((
                        Value::Nil,
                        Some(format!(
                            "{PER_MODULE} chunk handles are already held; release some"
                        )),
                    ));
                }
                let bot = ctx.bot(what)?;
                let Ok(world) = bot.world() else {
                    return Ok((Value::Nil, None));
                };
                let (found, min_y, height, biomes) = {
                    let w = world.read();
                    (
                        w.chunks.get(&ChunkPos::new(cx, cz)),
                        w.chunks.min_y(),
                        w.chunks.height(),
                        biome_names(&w.registries),
                    )
                };
                let Some(chunk) = found else {
                    return Ok((Value::Nil, None));
                };
                let (sections, surface) = {
                    let c = chunk.read();
                    (
                        c.sections.clone(),
                        c.heightmaps.get(&HeightmapKind::WorldSurface).cloned(),
                    )
                };
                let snap = Snapshot {
                    min_y,
                    height,
                    sections,
                    surface,
                    computed: OnceLock::new(),
                    biomes: Arc::new(biomes),
                };
                let key = NEXT.fetch_add(1, Relaxed);
                STORE
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(key, (k, Arc::new(snap)));
                Ok((
                    Value::UserData(lua.create_userdata(Handle { key, cx, cz })?),
                    None,
                ))
            },
        )?,
    )?;
    world.raw_set(
        "info",
        game_fn(lua, ctx, "torch.world.info", |lua, ctx, ()| {
            let what = "torch.world.info";
            let bot = ctx.bot(what)?;
            let dimension = bot
                .component::<azalea::world::WorldName>()
                .map_or_else(|_| String::new(), |n| n.to_string());
            let Ok(world) = bot.world() else {
                return Ok(Value::Nil);
            };
            let (min_y, height) = {
                let w = world.read();
                (w.chunks.min_y(), w.chunks.height())
            };
            let t = lua.create_table()?;
            t.raw_set("miny", min_y)?;
            t.raw_set("height", height)?;
            t.raw_set("dimension", dimension)?;
            t.raw_set(
                "hasceiling",
                crate::renderer::dimension::current().has_ceiling(),
            )?;
            Ok(Value::Table(t))
        })?,
    )?;
    Ok(())
}

#[derive(Default)]
struct Queues {
    arrived: Vec<[i32; 2]>,
    arrived_seen: HashSet<[i32; 2]>,
    changed: Vec<[i32; 2]>,
    changed_seen: HashSet<[i32; 2]>,
}

static QUEUES: LazyLock<Mutex<Queues>> = LazyLock::new(Mutex::default);

fn note(list: &mut Vec<[i32; 2]>, seen: &mut HashSet<[i32; 2]>, at: [i32; 2]) {
    if list.len() < QUEUE_CAP && seen.insert(at) {
        list.push(at);
    }
}

pub fn arrived(cx: i32, cz: i32) {
    if game::listens(CHUNK) {
        let q = &mut *QUEUES.lock().unwrap_or_else(PoisonError::into_inner);
        note(&mut q.arrived, &mut q.arrived_seen, [cx, cz]);
    }
}

pub fn changed(x: i32, z: i32) {
    if game::listens(BLOCKCHANGE) {
        let q = &mut *QUEUES.lock().unwrap_or_else(PoisonError::into_inner);
        note(&mut q.changed, &mut q.changed_seen, [x >> 4, z >> 4]);
    }
}

pub(super) fn take() -> (Vec<[i32; 2]>, Vec<[i32; 2]>) {
    let q = &mut *QUEUES.lock().unwrap_or_else(PoisonError::into_inner);
    q.arrived_seen.clear();
    q.changed_seen.clear();
    (
        std::mem::take(&mut q.arrived),
        std::mem::take(&mut q.changed),
    )
}
