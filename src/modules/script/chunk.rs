use std::collections::HashSet;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use mlua::{Lua, Table, Value};

use super::api::{BLOCKCHANGE, CHUNK, Ctx};
use super::game::{self, game_fn};

const QUEUE_CAP: usize = 8192;

pub(super) fn clear() {
    let mut q = QUEUES.lock().unwrap_or_else(PoisonError::into_inner);
    *q = Queues::default();
}

pub(super) fn add_world(lua: &Lua, ctx: &Arc<Ctx>, world: &Table) -> mlua::Result<()> {
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
