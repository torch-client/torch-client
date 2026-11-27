use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock, PoisonError};

use azalea::block::BlockState;
use azalea::block::fluid_state::FluidState;
use azalea::local_player::WorldHolder;
use azalea_core::position::{BlockPos, ChunkPos};
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_world::chunk::Chunk;
use azalea_world::palette::Palette;

use super::registry::{Id, new_chunks as setting};
use super::{Edge, Phase, store};
use crate::renderer::world_draw::{self, Node};
use crate::session::SharedMutex;

static EDGE: Edge = Edge::new(Id::NewChunks);

const OWNER: &str = "new_chunks";

const NEW_ARGB: u32 = 0x30_FF_40_40;
const OLD_ARGB: u32 = 0x30_40_FF_60;

const RECENT_CAP: usize = 4096;

const SECTION_BLOCKS: usize = 4096;

const FROM: [(i32, i32, i32); 5] = [(1, 0, 0), (0, 0, -1), (-1, 0, 0), (0, 0, 1), (0, 1, 0)];

struct State {
    session: u32,
    fresh: HashSet<(i32, i32)>,
    new: HashSet<(i32, i32)>,
    old: HashSet<(i32, i32)>,
    recent: HashMap<BlockPos, BlockState>,
    older: HashMap<BlockPos, BlockState>,
}

impl State {
    fn queued(&self, pos: &BlockPos) -> Option<BlockState> {
        self.recent
            .get(pos)
            .or_else(|| self.older.get(pos))
            .copied()
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static DIRTY: AtomicBool = AtomicBool::new(false);

fn state(slot: &mut Option<State>) -> &mut State {
    let session = super::basefinder::session();
    if slot.as_ref().is_none_or(|st| st.session != session) {
        *slot = Some(State {
            session,
            fresh: HashSet::new(),
            new: HashSet::new(),
            old: HashSet::new(),
            recent: HashMap::new(),
            older: HashMap::new(),
        });
        DIRTY.store(true, Relaxed);
    }
    slot.as_mut().unwrap()
}

fn flowing_ids() -> &'static [bool] {
    static TABLE: OnceLock<Box<[bool]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=BlockState::MAX_STATE)
            .map(|id| BlockState::try_from(id).is_ok_and(|st| is_flowing(&FluidState::from(st))))
            .collect()
    })
}

fn is_flowing(f: &FluidState) -> bool {
    !f.is_empty() && !f.is_source()
}

fn flowing_state(st: BlockState) -> bool {
    flowing_ids()
        .get(st.id() as usize)
        .copied()
        .unwrap_or(false)
}

fn holds_flowing(chunk: &Chunk) -> bool {
    chunk.sections.iter().any(|section| {
        let states = &section.states;
        match &states.palette {
            Palette::SingleValue(v) => flowing_state(*v),
            Palette::Linear(v) | Palette::Hashmap(v) => v.iter().any(|&st| flowing_state(st)),
            Palette::Global => (0..SECTION_BLOCKS).any(|i| flowing_state(states.get_at_index(i))),
        }
    })
}

pub fn packet(world: &WorldHolder, packet: &ClientboundGamePacket) {
    if !matches!(
        packet,
        ClientboundGamePacket::LevelChunkWithLight(_)
            | ClientboundGamePacket::BlockUpdate(_)
            | ClientboundGamePacket::SectionBlocksUpdate(_)
    ) || !store().enabled(Id::NewChunks)
    {
        return;
    }
    match packet {
        ClientboundGamePacket::LevelChunkWithLight(p) => {
            column_arrived(world, ChunkPos::new(p.x, p.z))
        }
        ClientboundGamePacket::BlockUpdate(p) => {
            updates(world, || std::iter::once((p.pos, p.block_state)))
        }
        ClientboundGamePacket::SectionBlocksUpdate(p) => updates(world, || {
            p.states.iter().map(|s| (p.section_pos + s.pos, s.state))
        }),
        _ => {}
    }
}

fn column_arrived(world: &WorldHolder, at: ChunkPos) {
    let key = (at.x, at.z);
    let Some(chunk) = world.shared.read().chunks.get(&at) else {
        return;
    };
    let old = holds_flowing(&chunk.read());

    let mut slot = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    let st = state(&mut slot);
    st.recent
        .retain(|p, _| (p.x.div_euclid(16), p.z.div_euclid(16)) != key);
    st.older
        .retain(|p, _| (p.x.div_euclid(16), p.z.div_euclid(16)) != key);
    if st.new.contains(&key) {
        return;
    }
    if old {
        st.fresh.remove(&key);
        if st.old.insert(key) {
            DIRTY.store(true, Relaxed);
        }
    } else {
        st.fresh.insert(key);
    }
}

fn updates<I: Iterator<Item = (BlockPos, BlockState)>>(world: &WorldHolder, list: impl Fn() -> I) {
    let mut slot = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    let st = state(&mut slot);
    if st.recent.len() > RECENT_CAP {
        st.recent.clear();
    }
    let mut any_flowing = false;
    for (pos, state) in list() {
        let fluid = FluidState::from(state);
        any_flowing |= is_flowing(&fluid);
        if !fluid.is_empty() || st.recent.contains_key(&pos) || st.older.contains_key(&pos) {
            st.recent.insert(pos, state);
        }
    }
    if !any_flowing {
        return;
    }
    let w = world.shared.read();
    let mut added = false;
    for (pos, state) in list() {
        let fluid = FluidState::from(state);
        if !is_flowing(&fluid) {
            continue;
        }
        let key = (pos.x.div_euclid(16), pos.z.div_euclid(16));
        if !st.fresh.contains(&key) {
            continue;
        }
        let fed = FROM.iter().any(|&(dx, dy, dz)| {
            let n = BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz);
            let Some(ns) = st.queued(&n).or_else(|| w.get_block_state(n)) else {
                return false;
            };
            let nf = FluidState::from(ns);
            nf.is_source() && nf.kind == fluid.kind
        });
        if fed {
            st.fresh.remove(&key);
            st.new.insert(key);
            added = true;
        }
    }
    if added {
        DIRTY.store(true, Relaxed);
    }
}

#[derive(PartialEq)]
struct Drawn {
    key: (bool, i32, i32),
    centre: (i32, i32),
}

pub fn tick(shared: &SharedMutex) {
    static LAST: Mutex<Option<Drawn>> = Mutex::new(None);
    match EDGE.poll(store()) {
        Phase::Off => return,
        Phase::Stopped => {
            world_draw::clear(OWNER);
            *LAST.lock().unwrap_or_else(PoisonError::into_inner) = None;
            return;
        }
        Phase::Started | Phase::Running => {}
    }
    let centre = shared.lock().unwrap().session.chunk_pos();
    let s = store();
    let key = (
        s.flag(setting::SHOW_OLD),
        s.num(setting::HEIGHT) as i32,
        s.num(setting::RANGE) as i32,
    );

    let mut slot = STATE.lock().unwrap_or_else(PoisonError::into_inner);
    let st = state(&mut slot);
    st.older = std::mem::take(&mut st.recent);

    let drawn = Drawn { key, centre };
    let mut last = LAST.lock().unwrap_or_else(PoisonError::into_inner);
    if !DIRTY.swap(false, Relaxed) && last.as_ref() == Some(&drawn) {
        return;
    }
    *last = Some(drawn);

    let (show_old, y, range) = key;
    let in_range =
        |&(cx, cz): &(i32, i32)| (cx - centre.0).abs() <= range && (cz - centre.1).abs() <= range;
    let plane = |(cx, cz): (i32, i32), argb: u32| {
        let (x0, z0) = (f64::from(cx * 16), f64::from(cz * 16));
        let y = f64::from(y);
        Node::boxed([x0, y, z0], [x0 + 16.0, y + 0.05, z0 + 16.0], argb, true)
    };
    let mut nodes: Vec<Node> = st
        .new
        .iter()
        .filter(|c| in_range(c))
        .map(|&c| plane(c, NEW_ARGB))
        .collect();
    if show_old {
        nodes.extend(
            st.old
                .iter()
                .filter(|c| in_range(c))
                .map(|&c| plane(c, OLD_ARGB)),
        );
    }
    world_draw::set(OWNER, nodes);
}
