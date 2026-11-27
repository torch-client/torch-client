use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, OnceLock, PoisonError};

use azalea::block::properties::{KelpAge, Lit, SugarCaneAge, TwistingVinesAge, WeepingVinesAge};
use azalea::block::{BlockState, Property};
use azalea_core::position::ChunkPos;
use azalea_registry::builtin::BlockKind;
use azalea_registry::tags::blocks as tags;

use super::esp::{self, Found, table::Merged};
use super::registry::{Id, basefinder as setting};
use super::{Edge, Phase, store};
use crate::renderer::dimension::{self, Dimension};
use crate::renderer::world_draw::{self, Node};
use crate::session::SharedMutex;

static EDGE: Edge = Edge::new(Id::Basefinder);

const OWNER: &str = "basefinder";

const BUDGET: usize = 8;

const EVIDENCE_PER_CHUNK: usize = 64;
const EVIDENCE_DRAWN: usize = 4000;

const CHI2_P01: f64 = 11.345;
const CHI2_P001: f64 = 16.266;

const NEIGHBOURS: [(i32, i32); 9] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (0, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum Group {
    Small,
    Medium,
    Large,
    Cluster,
    KelpGrown,
    Broken,
    CaneAged,
    PlacedHigh,
    PlacedMid,
    PlacedLow,
    EnderChest,
}

const GROUPS: [Group; 11] = [
    Group::Small,
    Group::Medium,
    Group::Large,
    Group::Cluster,
    Group::KelpGrown,
    Group::Broken,
    Group::CaneAged,
    Group::PlacedHigh,
    Group::PlacedMid,
    Group::PlacedLow,
    Group::EnderChest,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Signal {
    Amethyst,
    Kelp,
    Broken,
    Cane,
    Placed,
}

impl Group {
    fn signal(self) -> Signal {
        match self {
            Group::Small | Group::Medium | Group::Large | Group::Cluster => Signal::Amethyst,
            Group::KelpGrown => Signal::Kelp,
            Group::Broken => Signal::Broken,
            Group::CaneAged => Signal::Cane,
            Group::PlacedHigh | Group::PlacedMid | Group::PlacedLow | Group::EnderChest => {
                Signal::Placed
            }
        }
    }

    fn priority(self) -> u8 {
        match self {
            Group::PlacedHigh => 0,
            Group::PlacedMid | Group::EnderChest => 1,
            Group::Broken => 2,
            Group::PlacedLow => 3,
            Group::KelpGrown => 4,
            Group::Cluster => 5,
            Group::CaneAged => 6,
            Group::Large | Group::Medium | Group::Small => 7,
        }
    }
}

impl Signal {
    fn on(self) -> bool {
        let s = store();
        s.flag(match self {
            Signal::Amethyst => setting::AMETHYST,
            Signal::Kelp => setting::KELP,
            Signal::Broken => setting::BROKEN,
            Signal::Cane => setting::CANE,
            Signal::Placed => setting::PLACED,
        })
    }

    fn rgb(self) -> u32 {
        match self {
            Signal::Amethyst => 0xB8_6B_FF,
            Signal::Kelp => 0x3F_BF_5F,
            Signal::Broken => 0xE0_A0_30,
            Signal::Cane => 0x9F_E0_7F,
            Signal::Placed => 0xFF_50_50,
        }
    }
}

fn age<P: Property>(value: P) -> u8 {
    value.to_static_str().parse().unwrap_or(0)
}

fn group_of(state: BlockState) -> Option<Group> {
    let kind = BlockKind::from(state);
    Some(match kind {
        BlockKind::SmallAmethystBud => Group::Small,
        BlockKind::MediumAmethystBud => Group::Medium,
        BlockKind::LargeAmethystBud => Group::Large,
        BlockKind::AmethystCluster => Group::Cluster,
        BlockKind::Kelp => match age(state.property::<KelpAge>()?) {
            24.. => Group::KelpGrown,
            ..20 => Group::Broken,
            _ => return None,
        },
        BlockKind::WeepingVines => {
            (age(state.property::<WeepingVinesAge>()?) < 17).then_some(Group::Broken)?
        }
        BlockKind::TwistingVines => {
            (age(state.property::<TwistingVinesAge>()?) < 17).then_some(Group::Broken)?
        }
        BlockKind::SugarCane => {
            (age(state.property::<SugarCaneAge>()?) > 0).then_some(Group::CaneAged)?
        }
        BlockKind::Beacon | BlockKind::NetheriteBlock | BlockKind::RespawnAnchor => {
            Group::PlacedHigh
        }
        k if tags::SHULKER_BOXES.contains(&k) => Group::PlacedHigh,
        BlockKind::EnchantingTable
        | BlockKind::Anvil
        | BlockKind::ChippedAnvil
        | BlockKind::Crafter
        | BlockKind::EmeraldBlock
        | BlockKind::NetherPortal => Group::PlacedMid,
        BlockKind::Furnace | BlockKind::BlastFurnace | BlockKind::Smoker => {
            (state.property::<Lit>() == Some(true)).then_some(Group::PlacedLow)?
        }
        BlockKind::Observer => Group::PlacedLow,
        BlockKind::EnderChest => Group::EnderChest,
        _ => return None,
    })
}

fn table() -> &'static Merged {
    static TABLE: OnceLock<Merged> = OnceLock::new();
    TABLE
        .get_or_init(|| Merged::of_groups(GROUPS.len() as u16, |st| group_of(st).map(|g| g as u16)))
}

#[derive(Clone, Copy)]
struct Evidence {
    x: i32,
    y: i32,
    z: i32,
    group: Group,
}

#[derive(Default)]
struct Column {
    buds: [u32; 4],
    kelp: u32,
    broken: u32,
    cane: u32,
    placed: [u32; 3],
    evidence: Vec<Evidence>,
    score: u32,
}

impl Column {
    fn of(found: &[Found], in_end: bool) -> Option<Column> {
        let mut c = Column::default();
        for f in found {
            let Some(&group) = GROUPS.get(f.slot as usize) else {
                continue;
            };
            match group {
                Group::Small | Group::Medium | Group::Large | Group::Cluster => {
                    c.buds[group as usize] += 1
                }
                Group::KelpGrown => c.kelp += 1,
                Group::Broken => c.broken += 1,
                Group::CaneAged => c.cane += 1,
                Group::PlacedHigh => c.placed[0] += 1,
                Group::PlacedMid => c.placed[1] += 1,
                Group::PlacedLow => c.placed[2] += 1,
                Group::EnderChest if in_end => continue,
                Group::EnderChest => c.placed[1] += 1,
            }
            c.evidence.push(Evidence {
                x: f.x,
                y: f.y,
                z: f.z,
                group,
            });
        }
        if c.evidence.is_empty() {
            return None;
        }
        c.evidence.sort_by_key(|e| e.group.priority());
        c.evidence.truncate(EVIDENCE_PER_CHUNK);
        Some(c)
    }
}

fn amethyst_points(buds: [u32; 4], min: u32) -> u32 {
    let n: u32 = buds.iter().sum();
    if n == 0 || n < min {
        return 0;
    }
    let e = n as f64 / 4.0;
    let chi2: f64 = buds.iter().map(|&o| (o as f64 - e).powi(2) / e).sum();
    if chi2 >= CHI2_P001 {
        40
    } else if chi2 >= CHI2_P01 {
        20
    } else {
        0
    }
}

fn settings_key() -> u64 {
    let s = store();
    let flags = [
        setting::AMETHYST,
        setting::KELP,
        setting::BROKEN,
        setting::CANE,
        setting::PLACED,
        setting::EVIDENCE,
    ]
    .iter()
    .enumerate()
    .fold(0u64, |k, (i, &h)| k | (s.flag(h) as u64) << i);
    flags
        | (s.num(setting::MIN_BUDS) as u64 & 0xFFFF) << 8
        | (s.num(setting::MIN_SCORE) as u64 & 0xFFFF) << 24
        | (s.num(setting::RANGE) as u64 & 0xFFFF) << 40
}

struct State {
    session: u32,
    scanned: HashSet<(i32, i32)>,
    columns: HashMap<(i32, i32), Column>,
    backlog: bool,
    key: u64,
    drawn_at: Option<(i32, i32)>,
    redraw: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

static SESSION: AtomicU32 = AtomicU32::new(0);
static ARRIVED: AtomicBool = AtomicBool::new(false);
static LEVEL: Mutex<String> = Mutex::new(String::new());

pub(super) fn session() -> u32 {
    SESSION.load(Relaxed)
}

#[inline]
pub fn chunk_arrived() {
    ARRIVED.store(true, Relaxed);
}

pub fn logged_in() {
    SESSION.fetch_add(1, Relaxed);
}

pub fn disconnected() {
    SESSION.fetch_add(1, Relaxed);
}

pub fn spawned(level: &str) {
    let mut last = LEVEL.lock().unwrap_or_else(PoisonError::into_inner);
    if *last != level {
        *last = level.to_owned();
        SESSION.fetch_add(1, Relaxed);
    }
}

pub fn tick(bot: &azalea::Client, shared: &SharedMutex) {
    let phase = EDGE.poll(store());
    let session = SESSION.load(Relaxed);
    match phase {
        Phase::Off => return,
        Phase::Stopped => {
            world_draw::clear(OWNER);
            STATE.with_borrow_mut(|slot| {
                if let Some(st) = slot {
                    st.redraw = true;
                }
            });
            return;
        }
        Phase::Started | Phase::Running => {}
    }
    STATE.with_borrow_mut(|slot| {
        if slot.as_ref().is_none_or(|st| st.session != session) {
            world_draw::clear(OWNER);
            ARRIVED.store(true, Relaxed);
            *slot = Some(State {
                session,
                scanned: HashSet::new(),
                columns: HashMap::new(),
                backlog: true,
                key: u64::MAX,
                drawn_at: None,
                redraw: true,
            });
        }
        if let Some(st) = slot {
            if phase == Phase::Started {
                st.redraw = true;
                st.backlog = true;
            }
            st.tick(bot, shared);
        }
    });
}

impl State {
    fn tick(&mut self, bot: &azalea::Client, shared: &SharedMutex) {
        let centre = shared.lock().unwrap().session.chunk_pos();
        if ARRIVED.swap(false, Relaxed) || self.backlog {
            self.scan(bot, centre);
        }
        let key = settings_key();
        if key != self.key {
            self.key = key;
            let all: Vec<(i32, i32)> = self.columns.keys().copied().collect();
            for at in all {
                self.rescore(at);
            }
            self.redraw = true;
        }
        if self.redraw || self.drawn_at != Some(centre) {
            self.draw(centre);
        }
    }

    fn scan(&mut self, bot: &azalea::Client, centre: (i32, i32)) {
        let Ok(world) = bot.world() else { return };
        let (jobs, min_y): (Vec<esp::Handle>, i32) = {
            let w = world.read();
            let mut fresh: HashMap<(i32, i32), esp::Handle> = HashMap::new();
            for at in w.chunks.chunks().iter() {
                let key = (at.x, at.z);
                if self.scanned.contains(&key) {
                    continue;
                }
                if let Some(c) = w.chunks.get(at) {
                    fresh.insert(key, (**at, c));
                }
            }
            let wanted = super::nearest_first(centre, fresh.keys().copied());
            self.backlog = wanted.len() > BUDGET;
            let jobs = wanted
                .into_iter()
                .take(BUDGET)
                .filter_map(|at| fresh.remove(&at))
                .collect();
            (jobs, w.chunks.min_y())
        };
        if jobs.is_empty() {
            return;
        }

        crate::prof_span!("modules:basefinder");
        let table = table();
        let in_end = dimension::current() == Dimension::End;
        let mut found = Vec::new();
        let mut touched = Vec::new();
        for job in &jobs {
            let at = (job.0.x, job.0.z);
            self.scanned.insert(at);
            esp::sweep_column(job, table, min_y, &mut found);
            if let Some(column) = Column::of(&found, in_end) {
                self.columns.insert(at, column);
                touched.push(at);
            }
        }
        for at in touched {
            for (dx, dz) in NEIGHBOURS {
                self.rescore((at.0 + dx, at.1 + dz));
            }
        }
    }

    fn rescore(&mut self, at: (i32, i32)) {
        let Some(c) = self.columns.get(&at) else {
            return;
        };
        let s = store();
        let mut score = 0u32;
        if Signal::Amethyst.on() && c.buds.iter().any(|&n| n > 0) {
            let mut buds = [0u32; 4];
            for (dx, dz) in NEIGHBOURS {
                if let Some(n) = self.columns.get(&(at.0 + dx, at.1 + dz)) {
                    for (sum, own) in buds.iter_mut().zip(n.buds) {
                        *sum += own;
                    }
                }
            }
            score += amethyst_points(buds, s.num(setting::MIN_BUDS) as u32);
        }
        if Signal::Kelp.on() {
            score += (c.kelp * 6).min(30);
        }
        if Signal::Broken.on() {
            score += (c.broken * 10).min(40);
        }
        if Signal::Cane.on() {
            score += (c.cane * 2).min(40);
        }
        if Signal::Placed.on() {
            score += c.placed[0] * 40 + c.placed[1] * 20 + c.placed[2] * 10;
        }
        let score = score.min(100);
        if let Some(c) = self.columns.get_mut(&at)
            && c.score != score
        {
            c.score = score;
            self.redraw = true;
        }
    }

    fn draw(&mut self, centre: (i32, i32)) {
        self.redraw = false;
        self.drawn_at = Some(centre);
        let s = store();
        let min = (s.num(setting::MIN_SCORE) as u32).max(1);
        let range = s.num(setting::RANGE) as i32;
        let evidence = s.flag(setting::EVIDENCE);
        let on: [bool; 5] = [
            Signal::Amethyst,
            Signal::Kelp,
            Signal::Broken,
            Signal::Cane,
            Signal::Placed,
        ]
        .map(Signal::on);
        let signal_on = |g: Group| on[g.signal() as usize];

        let dist2 = |(cx, cz): (i32, i32)| {
            let (dx, dz) = (
                i64::from(cx) - i64::from(centre.0),
                i64::from(cz) - i64::from(centre.1),
            );
            dx * dx + dz * dz
        };
        let range2 = i64::from(range) * i64::from(range);
        let mut passing: Vec<((i32, i32), &Column)> = self
            .columns
            .iter()
            .filter(|&(&at, c)| c.score >= min && dist2(at) <= range2)
            .map(|(&at, c)| (at, c))
            .collect();
        passing.sort_unstable_by_key(|&(at, _)| dist2(at));

        let mut nodes = Vec::new();
        let mut drawn = 0usize;
        for ((cx, cz), c) in passing {
            let shown = || c.evidence.iter().filter(|e| signal_on(e.group));
            let (y0, y1) = shown()
                .fold(None, |r: Option<(i32, i32)>, e| {
                    Some(r.map_or((e.y, e.y), |(a, b)| (a.min(e.y), b.max(e.y))))
                })
                .unwrap_or((c.evidence[0].y, c.evidence[0].y));
            let (x0, z0) = (f64::from(cx * 16), f64::from(cz * 16));
            let rgb = match c.score {
                70.. => 0xFF_30_30,
                40.. => 0xFF_90_20,
                _ => 0xFF_E0_40,
            };
            nodes.push(Node::boxed(
                [x0, f64::from(y0), z0],
                [x0 + 16.0, f64::from(y1 + 1), z0 + 16.0],
                rgb,
                true,
            ));
            if !evidence {
                continue;
            }
            for e in shown() {
                if drawn >= EVIDENCE_DRAWN {
                    break;
                }
                drawn += 1;
                let p = [f64::from(e.x), f64::from(e.y), f64::from(e.z)];
                nodes.push(Node::boxed(
                    p.map(|v| v + 0.1),
                    p.map(|v| v + 0.9),
                    e.group.signal().rgb(),
                    true,
                ));
            }
        }
        world_draw::set(OWNER, nodes);
    }
}
