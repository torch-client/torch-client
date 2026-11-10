pub mod random;
pub mod table;
pub(super) mod terrain;
#[cfg(test)]
mod tests;
pub mod vein;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering::Relaxed};

use azalea_core::position::ChunkPos;

use super::chat_math::say;
use super::esp::scan::{
    BA, BA_LE, BA_RI, BO, BO_BA, BO_FO, BO_LE, BO_RI, FO, FO_LE, FO_RI, LE, RI, TO, TO_BA, TO_FO,
    TO_LE, TO_RI,
};
use super::esp::{Found, Slot, blocks, table as esp_table};
use super::list::{BitList, Built, Entry};
use super::nearest_first;
use super::registry::{Id, seed_xray as setting};
use super::{Edge, Phase, store};
use crate::renderer::dimension::{self, Dimension};
use crate::session::SharedMutex;
use crate::text::{GREEN, RED, WHITE, YELLOW};
use table::{Kind, Row};
use terrain::{Handle, Region};
use vein::{Class, Terrain, simulate};

const BUDGET: usize = 4;

const CHECK_RADIUS: i32 = 3;

static EDGE: Edge = Edge::new(Id::SeedXray);
static GENERATION: AtomicU32 = AtomicU32::new(0);
static SERVER_HASH: AtomicI64 = AtomicI64::new(0);
static CHUNK_ARRIVED: AtomicBool = AtomicBool::new(false);
static CHECK: AtomicBool = AtomicBool::new(false);

static GROUPS: [&str; 1] = ["Ores"];

fn build_ores() -> Built {
    let entries = Kind::ALL
        .iter()
        .map(|k| Entry {
            group: 0,
            label: Cow::Borrowed(k.label()),
            on: *k != Kind::Coal,
            color: blocks::color_of(k.block()),
        })
        .collect();
    (entries, Vec::new())
}

pub static ORES: BitList = BitList::new(
    "seed_xray",
    "Ores",
    214.0,
    &GROUPS,
    true,
    blocks::PRESETS,
    build_ores,
);

#[derive(Clone, Copy, Default)]
pub struct Tick {
    pub running: bool,
    pub edged: bool,
    pub key: u64,
}

#[derive(Default)]
struct State {
    generation: u32,
    seed: Option<i64>,
    masks: Vec<u32>,
    chunks: HashMap<(i32, i32), Vec<Found>>,
    centre: Option<(i32, i32)>,
    backlog: bool,
    done: u64,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn spawned(server_hash: i64) {
    SERVER_HASH.store(server_hash, Relaxed);
    GENERATION.fetch_add(1, Relaxed);
}

#[cfg_attr(not(feature = "seed_cracker"), allow(dead_code))]
pub(super) fn server_hash() -> i64 {
    SERVER_HASH.load(Relaxed)
}

#[inline]
pub fn chunk_arrived() {
    CHUNK_ARRIVED.store(true, Relaxed);
}

pub fn request_check() {
    CHECK.store(true, Relaxed);
}

fn parse_seed(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.parse().unwrap_or_else(|_| {
        text.encode_utf16()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32)) as i64
    }))
}

#[cfg(feature = "multiversion")]
fn hopped() -> bool {
    crate::util::session_protocol() != crate::protocol::version::NATIVE.protocol
}

#[cfg(not(feature = "multiversion"))]
fn hopped() -> bool {
    false
}

fn rows() -> &'static [Row] {
    if hopped() {
        return &[];
    }
    match dimension::current() {
        Dimension::Overworld => &table::OVERWORLD,
        Dimension::Nether => &table::NETHER,
        Dimension::End => &[],
    }
}

fn report(shared: &SharedMutex, seed: Option<i64>) {
    let Some(seed) = seed else {
        say(shared, YELLOW, "Seed Xray: no seed set");
        return;
    };
    if hopped() {
        say(shared, RED, "Seed Xray: off, this server is not on 26.2");
        return;
    }
    let server = SERVER_HASH.load(Relaxed);
    if server != 0 && server != crate::util::hash::obfuscate_seed(seed) {
        say(
            shared,
            RED,
            "Seed Xray: this seed does not match the server's, expect wrong ores",
        );
    }
}

fn neighbourhood(world: &azalea_world::World, (cx, cz): (i32, i32)) -> Option<[Handle; 9]> {
    let mut handles: [Option<Handle>; 9] = Default::default();
    for (i, slot) in handles.iter_mut().enumerate() {
        let (dx, dz) = (i as i32 % 3 - 1, i as i32 / 3 - 1);
        *slot = Some(world.chunks.get(&ChunkPos::new(cx + dx, cz + dz))?);
    }
    Some(handles.map(|h| h.expect("every slot was filled")))
}

const LINKS: [(u32, [i32; 3]); 18] = [
    (FO, [0, 0, 1]),
    (BA, [0, 0, -1]),
    (RI, [1, 0, 0]),
    (LE, [-1, 0, 0]),
    (TO, [0, 1, 0]),
    (BO, [0, -1, 0]),
    (FO_RI, [1, 0, 1]),
    (FO_LE, [-1, 0, 1]),
    (BA_RI, [1, 0, -1]),
    (BA_LE, [-1, 0, -1]),
    (TO_FO, [0, 1, 1]),
    (TO_BA, [0, 1, -1]),
    (TO_RI, [1, 1, 0]),
    (TO_LE, [-1, 1, 0]),
    (BO_FO, [0, -1, 1]),
    (BO_BA, [0, -1, -1]),
    (BO_RI, [1, -1, 0]),
    (BO_LE, [-1, -1, 0]),
];

fn link(found: &mut [Found]) {
    let same: HashSet<(i32, i32, i32, u16)> =
        found.iter().map(|f| (f.x, f.y, f.z, f.slot)).collect();
    for f in found {
        f.neigh = LINKS.iter().fold(0, |mask, (bit, d)| {
            let hit = same.contains(&(f.x + d[0], f.y + d[1], f.z + d[2], f.slot));
            mask | if hit { *bit } else { 0 }
        });
    }
}

fn advance(
    bot: &azalea::Client,
    shared: &SharedMutex,
    st: &mut State,
    seed: i64,
    rows: &[Row],
    range: i32,
) {
    let centre = {
        let s = shared.lock().unwrap();
        s.session.chunk_pos()
    };
    let arrived = CHUNK_ARRIVED.swap(false, Relaxed);
    if !arrived && !st.backlog && st.centre == Some(centre) {
        return;
    }
    st.centre = Some(centre);
    let Ok(world) = bot.world() else { return };

    let (jobs, bounds) = {
        let w = world.read();
        if st.masks.is_empty() {
            st.masks = terrain::biome_masks(rows, &w.registries);
            if st.masks.is_empty() {
                st.backlog = true;
                return;
            }
        }
        st.chunks
            .retain(|&(x, z), _| w.chunks.get(&ChunkPos::new(x, z)).is_some());

        let r = range / 16 + 1;
        let wanted = nearest_first(
            centre,
            (-r..=r)
                .flat_map(|dz| (-r..=r).map(move |dx| (dx, dz)))
                .filter(|&(dx, dz)| dx * dx + dz * dz <= r * r)
                .map(|(dx, dz)| (centre.0 + dx, centre.1 + dz))
                .filter(|at| !st.chunks.contains_key(at)),
        );
        let mut ready = wanted
            .iter()
            .filter_map(|&at| Some((at, neighbourhood(&w, at)?)));
        let jobs: Vec<_> = ready.by_ref().take(BUDGET).collect();
        st.backlog = ready.next().is_some();
        (jobs, (w.chunks.min_y(), w.chunks.height() as i32))
    };

    crate::prof_span!("modules:seed_xray");
    let zoom = crate::util::hash::obfuscate_seed(seed);
    for (at, handles) in &jobs {
        let region = Region::new(*at, handles, bounds, zoom, &st.masks);
        let mut found: Vec<Found> = simulate(seed, rows, at.0, at.1, &region)
            .into_iter()
            .map(|p| Found {
                x: p.x,
                y: p.y,
                z: p.z,
                neigh: 0,
                slot: p.kind as u16,
            })
            .collect();
        drop(region);
        link(&mut found);
        st.chunks.insert(*at, found);
        st.done += 1;
    }
}

fn prune(bot: &azalea::Client, st: &mut State) {
    let Ok(world) = bot.world() else { return };
    let (jobs, bounds): (Vec<_>, _) = {
        let w = world.read();
        let jobs = st
            .chunks
            .keys()
            .filter_map(|&at| Some((at, neighbourhood(&w, at)?)))
            .collect();
        (jobs, (w.chunks.min_y(), w.chunks.height() as i32))
    };
    for (at, handles) in &jobs {
        let region = Region::new(*at, handles, bounds, 0, &[]);
        let Some(found) = st.chunks.get_mut(at) else {
            continue;
        };
        let before = found.len();
        found.retain(|f| region.class(f.x, f.y, f.z).holds_ore());
        if found.len() != before {
            link(found);
        }
    }
}

pub fn append(
    bot: &azalea::Client,
    centre: [i32; 3],
    dirty: bool,
    cap: usize,
    boxes: &mut Vec<Found>,
    slots: &mut Vec<Slot>,
) {
    STATE.with_borrow_mut(|st| {
        if st.chunks.is_empty() {
            return;
        }
        if dirty {
            prune(bot, st);
        }
        let s = store();
        let range = s.num(setting::RANGE).max(0.0) as i32;
        let range2 = (range * range) as i64;
        let (alpha, lines, sides) = esp_table::look(s.choice(setting::SHAPE), s.num(setting::FILL));

        let of_kind = Kind::ALL.map(|k| {
            let row = ORES.find(k.label())?;
            ORES.enabled(row).then(|| {
                let slot = Slot {
                    rgb: ORES.color(row),
                    alpha,
                    lines,
                    sides,
                    range2: range2 as u32,
                };
                slots.iter().position(|s| *s == slot).unwrap_or_else(|| {
                    slots.push(slot);
                    slots.len() - 1
                }) as u16
            })
        });

        for found in st.chunks.values() {
            for f in found {
                let Some(slot) = of_kind[f.slot as usize] else {
                    continue;
                };
                let d = [f.x - centre[0], f.y - centre[1], f.z - centre[2]].map(|v| v as i64);
                if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > range2 {
                    continue;
                }
                if boxes.len() >= cap {
                    return;
                }
                boxes.push(Found { slot, ..*f });
            }
        }
    });
}

fn check(bot: &azalea::Client, shared: &SharedMutex, st: &State) {
    let Some(centre) = st.centre else { return };
    let Ok(world) = bot.world() else { return };
    let predicted: HashMap<[i32; 3], Kind> = st
        .chunks
        .values()
        .flatten()
        .map(|f| ([f.x, f.y, f.z], Kind::ALL[f.slot as usize]))
        .collect();

    let (jobs, (min_y, height)): (Vec<_>, _) = {
        let w = world.read();
        let mut jobs = Vec::new();
        for dz in -CHECK_RADIUS..=CHECK_RADIUS {
            for dx in -CHECK_RADIUS..=CHECK_RADIUS {
                let at = (centre.0 + dx, centre.1 + dz);
                let simulated = (-1..=1)
                    .all(|a| (-1..=1).all(|b| st.chunks.contains_key(&(at.0 + a, at.1 + b))));
                if simulated && let Some(handles) = neighbourhood(&w, at) {
                    jobs.push((at, handles));
                }
            }
        }
        (jobs, (w.chunks.min_y(), w.chunks.height() as i32))
    };

    let mut matched = [0u32; Kind::ALL.len()];
    let mut guessed = [0u32; Kind::ALL.len()];
    let mut real = [0u32; Kind::ALL.len()];
    for (at, handles) in &jobs {
        let region = Region::new(*at, handles, (min_y, height), 0, &[]);
        for y in min_y..min_y + height {
            for x in at.0 * 16..at.0 * 16 + 16 {
                for z in at.1 * 16..at.1 * 16 + 16 {
                    if let Class::Ore(kind) = region.class(x, y, z) {
                        real[kind as usize] += 1;
                    }
                }
            }
        }
        for (p, &kind) in &predicted {
            if (p[0] >> 4, p[2] >> 4) == *at {
                guessed[kind as usize] += 1;
                matched[kind as usize] +=
                    (region.class(p[0], p[1], p[2]) == Class::Ore(kind)) as u32;
            }
        }
    }

    if jobs.is_empty() {
        say(
            shared,
            YELLOW,
            "Seed check: nothing simulated around you yet",
        );
        return;
    }
    say(
        shared,
        GREEN,
        format!(
            "Seed check over {} chunks: matched/predicted, real",
            jobs.len()
        ),
    );
    for kind in Kind::ALL {
        let k = kind as usize;
        if guessed[k] != 0 || real[k] != 0 {
            let line = format!(
                "{}: {}/{}, {}",
                kind.label(),
                matched[k],
                guessed[k],
                real[k]
            );
            crate::log_info!("seed_xray", "{line}");
            say(shared, WHITE, line);
        }
    }
}

pub fn tick(bot: &azalea::Client, shared: &SharedMutex) -> Tick {
    let s = store();
    let phase = EDGE.poll(s);
    match phase {
        Phase::Off => return Tick::default(),
        Phase::Stopped => {
            STATE.with_borrow_mut(|st| *st = State::default());
            return Tick {
                running: false,
                edged: true,
                key: 0,
            };
        }
        Phase::Started | Phase::Running => {}
    }

    let seed = parse_seed(&s.text(setting::SEED));
    let generation = GENERATION.load(Relaxed);
    let range = s.num(setting::RANGE).max(0.0) as i32;
    let rows = rows();

    let done = STATE.with_borrow_mut(|st| {
        if phase == Phase::Started || st.seed != seed || st.generation != generation {
            let done = st.done;
            *st = State {
                seed,
                generation,
                backlog: true,
                done: done + 1,
                ..State::default()
            };
            report(shared, seed);
        }
        if let Some(seed) = seed
            && !rows.is_empty()
        {
            advance(bot, shared, st, seed, rows, range);
        }
        if CHECK.swap(false, Relaxed) {
            check(bot, shared, st);
        }
        st.done
    });

    let key = [
        done,
        ORES.generation() as u64,
        range as u64,
        s.choice(setting::SHAPE) as u64,
        s.num(setting::FILL).to_bits() as u64,
    ]
    .into_iter()
    .fold(0xcbf2_9ce4_8422_2325_u64, |h, v| {
        (h ^ v).wrapping_mul(0x100_0000_01b3)
    });

    Tick {
        running: true,
        edged: phase == Phase::Started,
        key,
    }
}
