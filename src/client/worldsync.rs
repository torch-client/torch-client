use crate::util::global;
use crate::{log_debug, log_error, log_warn};
use azalea::Client;
use azalea_core::position::BlockPos as AzBlockPos;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use crate::client::tracking::{
    current_world, meshed_chunks, received_chunks, reset_chunk_tracking, reset_maps,
    set_current_world,
};
use crate::client::worker::JobScope;
use crate::session::SharedMutex;
use crate::{CHUNK_Q, SHARED, blockentities, lighting};

pub(crate) fn install_block_change_hook() {
    crate::play::interaction::install_local_break_hook();
    let hook_installed =
        azalea_world::block_change::set_block_change_hook(Box::new(|pos, old, new| {
            crate::blockentities::feed::on_block_change(pos, new);
            hidden::on_write(pos, new);
            if crate::modules::esp::any_enabled() {
                crate::modules::esp::mark_dirty();
            }
            crate::modules::auto_mine::block_changed(pos, old, new);
            crate::modules::builder::block_changed(pos.x, pos.y, pos.z);
            #[cfg(lua_mods)]
            {
                crate::modules::script::ghost::block_changed(pos.x, pos.y, pos.z);
                crate::modules::script::block_changed(pos.x, pos.z);
            }
            #[cfg(feature = "render_lod")]
            crate::client::lod::note_column(pos.x >> 4, pos.z >> 4);
            let (Some(world), Some(q)) = (current_world().lock().unwrap().clone(), CHUNK_Q.get())
            else {
                return;
            };
            if !crate::client::mesh_worker::forward_block_change(pos, new) {
                {
                    let meshed = meshed_chunks().lock().unwrap();
                    for (cx, cz, scope) in remesh_jobs(pos) {
                        if meshed.contains(&(cx, cz)) {
                            q.push(world.clone(), cx, cz, scope);
                        }
                    }
                }
                send_light(lighting::LightJob::Block { pos });
            }

            if !old.is_air()
                && new.is_air()
                && let Some(shared) = SHARED.get()
                && let Ok(mut s) = shared.try_lock()
                && crate::play::interaction::take_local_break(pos)
            {
                crate::session::push_particle_emit(
                    &mut s.session.particle_emits,
                    crate::session::ParticleEmit::Event(crate::session::LevelEventEmit {
                        id: 2001,
                        pos: [pos.x, pos.y, pos.z],
                        data: old.id() as u32,
                        global: false,
                    }),
                );
            }
        }));
    if hook_installed.is_err() {
        log_error!(
            "blocks",
            "the block-change hook could not be installed because something else already owns \
             it; no block placed or broken will ever be redrawn this run."
        );
    }
}

global! {
pub(crate) fn light_map() -> Arc<parking_lot::RwLock<lighting::LightMap>>
}

#[cfg(any(feature = "audio", lua_mods))]
pub(crate) fn light_at(x: i32, y: i32, z: i32) -> Option<(u8, u8)> {
    let map = light_map().read();
    map.is_lit(lighting::SectionPos::of_block(x, y, z).column())
        .then(|| map.at(x, y, z))
}

static LIGHT_THREAD: OnceLock<lighting::LightThread> = OnceLock::new();

pub(crate) fn start_light_thread() {
    let _ = LIGHT_THREAD.get_or_init(|| {
        lighting::level::spawn(light_map().clone(), |lit_in, dirty, to_mesh| {
            let Some(q) = CHUNK_Q.get() else {
                crate::diag::add(crate::diag::Stat::LightJobsDropped, to_mesh.len() as u64);
                return;
            };
            log_debug!("light", "queueing {} column meshes", to_mesh.len());
            let mut jobs = Vec::new();
            let mut whole = HashSet::new();
            {
                let received = received_chunks().lock().unwrap();
                let mut meshed = meshed_chunks().lock().unwrap();
                let current = current_world().lock().unwrap().clone();
                let Some(current) = current.filter(|w| Arc::ptr_eq(w, lit_in)) else {
                    log_debug!(
                        "light",
                        "a light pass for a level already left queued nothing"
                    );
                    return;
                };
                for col in to_mesh.iter().map(|(_, col)| col) {
                    if !received.contains(&(col.x, col.z))
                        || !surrounded(|c| received.contains(&c), col.x, col.z)
                    {
                        continue;
                    }
                    if whole.insert((col.x, col.z)) {
                        meshed.insert((col.x, col.z));
                        jobs.push((current.clone(), col.x, col.z, JobScope::Column));
                    }
                }
                for sec in dirty {
                    if whole.contains(&(sec.x, sec.z)) {
                        continue;
                    }
                    if meshed.contains(&(sec.x, sec.z)) {
                        jobs.push((current.clone(), sec.x, sec.z, JobScope::Section(sec.y)));
                    }
                }
            }
            crate::diag::add(crate::diag::Stat::MeshJobs, jobs.len() as u64);
            for (world, cx, cz, scope) in jobs {
                match scope {
                    JobScope::Section(sy) => q.push_relight(world, cx, cz, sy),
                    JobScope::Column => q.push(world, cx, cz, scope),
                }
            }
        })
    });
}

pub(crate) fn send_light(job: lighting::LightJob) {
    match LIGHT_THREAD.get() {
        Some(t) => t.send(job),
        None => crate::diag::bump(crate::diag::Stat::LightJobsDropped),
    }
}

pub(crate) fn remesh_jobs(pos: AzBlockPos) -> Vec<(i32, i32, JobScope)> {
    let (cx, cz, sy) = (pos.x >> 4, pos.z >> 4, pos.y >> 4);
    let mut v = vec![(cx, cz, JobScope::Section(sy))];
    match pos.y.rem_euclid(16) {
        0 => v.push((cx, cz, JobScope::Section(sy - 1))),
        15 => v.push((cx, cz, JobScope::Section(sy + 1))),
        _ => {}
    }
    if pos.x.rem_euclid(16) == 0 {
        v.push((cx - 1, cz, JobScope::Section(sy)));
    }
    if pos.x.rem_euclid(16) == 15 {
        v.push((cx + 1, cz, JobScope::Section(sy)));
    }
    if pos.z.rem_euclid(16) == 0 {
        v.push((cx, cz - 1, JobScope::Section(sy)));
    }
    if pos.z.rem_euclid(16) == 15 {
        v.push((cx, cz + 1, JobScope::Section(sy)));
    }
    v
}

pub(crate) mod hidden {
    use super::*;
    use std::sync::atomic::AtomicBool;

    use azalea_registry::builtin::BlockKind;
    use bevy::platform::collections::{HashMap as FastMap, HashSet as FastSet};

    use crate::blockentities::sim::piston::{Delta, MoveState};

    type Cell = [i32; 3];
    pub(crate) type Section = (i32, i32, i32);

    static ACTIVE: AtomicBool = AtomicBool::new(false);

    #[derive(Clone, Copy)]
    struct Owner {
        seq: u64,
        hidden: bool,
    }

    struct Move {
        seq: u64,
        start: u32,
        gate: [Option<Section>; 2],
        shown: Option<(u32, Section)>,
    }

    type CellMap = FastMap<Section, Arc<FastMap<Cell, Owner>>>;

    #[derive(Default)]
    struct State {
        version: u32,
        cells: Arc<CellMap>,
        moves: FastMap<Cell, Move>,
        sections: FastMap<Section, u32>,
        gated: Vec<Cell>,
        ending: Vec<Cell>,
        #[cfg(target_arch = "wasm32")]
        log: Vec<(Cell, u8)>,
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) const SHOWN: u8 = 0;
    #[cfg(target_arch = "wasm32")]
    pub(crate) const HIDDEN: u8 = 1;
    #[cfg(target_arch = "wasm32")]
    pub(crate) const RELEASED: u8 = 2;

    global!(fn state() -> parking_lot::Mutex<State>);

    impl State {
        fn stored_at(&self, sec: &Section) -> u32 {
            self.sections.get(sec).copied().unwrap_or(0)
        }

        fn is_shown(&self, m: &Move) -> bool {
            m.shown
                .is_some_and(|(version, sec)| version == 0 || self.stored_at(&sec) >= version)
        }

        fn owner(&self, cell: &Cell) -> Option<Owner> {
            self.cells.get(&section_of(cell))?.get(cell).copied()
        }

        fn set_owner(&mut self, cell: Cell, owner: Option<Owner>) {
            let cells = Arc::make_mut(&mut self.cells);
            let sec = section_of(&cell);
            match owner {
                Some(owner) => {
                    Arc::make_mut(cells.entry(sec).or_default()).insert(cell, owner);
                }
                None => {
                    let Some(section) = cells.get_mut(&sec) else {
                        return;
                    };
                    let section = Arc::make_mut(section);
                    section.remove(&cell);
                    if section.is_empty() {
                        cells.remove(&sec);
                    }
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        fn note(&mut self, cell: Cell, what: u8) {
            if crate::client::mesh_worker::active() {
                self.log.push((cell, what));
            }
        }

        fn end(&mut self, pos: Cell, version: u32, sec: Section) {
            if let Some(m) = self.moves.get_mut(&pos) {
                if m.shown.is_none() {
                    self.ending.push(pos);
                }
                m.shown = Some((version, sec));
                if version != 0 {
                    self.sections.entry(sec).or_insert(0);
                }
            }
        }
    }

    fn section_of(cell: &Cell) -> Section {
        (cell[0] >> 4, cell[1] >> 4, cell[2] >> 4)
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn is_active() -> bool {
        ACTIVE.load(Ordering::Acquire)
    }

    pub(crate) struct Cells {
        pub version: u32,
        cells: Arc<CellMap>,
    }

    impl Cells {
        pub fn within(&self, min: Cell, max: Cell) -> FastSet<Cell> {
            let mut out = FastSet::default();
            if self.cells.is_empty() {
                return out;
            }
            let inside = |cell: &Cell| (0..3).all(|i| (min[i]..=max[i]).contains(&cell[i]));
            let (lo, hi) = (section_of(&min), section_of(&max));
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    for z in lo.2..=hi.2 {
                        let Some(section) = self.cells.get(&(x, y, z)) else {
                            continue;
                        };
                        out.extend(
                            section
                                .iter()
                                .filter(|(cell, owner)| owner.hidden && inside(*cell))
                                .map(|(cell, _)| *cell),
                        );
                    }
                }
            }
            out
        }
    }

    pub(crate) fn snapshot() -> Option<Cells> {
        if !ACTIVE.load(Ordering::Acquire) {
            return None;
        }
        let state = state().lock();
        Some(Cells {
            version: state.version,
            cells: state.cells.clone(),
        })
    }

    pub(crate) fn stored(sec: Section, version: u32) {
        if version == 0 || !ACTIVE.load(Ordering::Acquire) {
            return;
        }
        if let Some(at) = state().lock().sections.get_mut(&sec) {
            *at = (*at).max(version);
        }
    }

    pub(crate) fn withheld() -> FastSet<Cell> {
        let mut out = FastSet::default();
        if !ACTIVE.load(Ordering::Acquire) {
            return out;
        }
        let state = state().lock();
        for pos in &state.gated {
            if let Some(m) = state.moves.get(pos)
                && m.gate
                    .iter()
                    .flatten()
                    .any(|sec| state.stored_at(sec) < m.start)
            {
                out.insert(*pos);
            }
        }
        for pos in &state.ending {
            if let Some(m) = state.moves.get(pos)
                && state.is_shown(m)
            {
                out.insert(*pos);
            }
        }
        out
    }

    pub(crate) fn on_write(pos: AzBlockPos, new: azalea::block::BlockState) {
        if !ACTIVE.load(Ordering::Acquire) {
            return;
        }
        let ends = BlockKind::from(new) != BlockKind::MovingPiston;
        let cell = [pos.x, pos.y, pos.z];
        let mut guard = state().lock();
        let state = &mut *guard;
        let owner = state.owner(&cell).filter(|owner| owner.hidden);
        let ended = ends && state.moves.contains_key(&cell);
        if owner.is_none() && !ended {
            return;
        }
        state.version += 1;
        if let Some(owner) = owner {
            state.set_owner(
                cell,
                Some(Owner {
                    hidden: false,
                    ..owner
                }),
            );
            #[cfg(target_arch = "wasm32")]
            state.note(cell, RELEASED);
        }
        if ended {
            let version = state.version;
            state.end(cell, version, section_of(&cell));
        }
    }

    pub(crate) fn sync(delta: &Delta) -> Vec<Cell> {
        let was_active = ACTIVE.load(Ordering::Acquire);
        if !was_active && delta.is_empty() {
            return Vec::new();
        }
        let meshed: FastSet<(i32, i32)> = {
            let columns: FastSet<(i32, i32)> = {
                let state = state().lock();
                delta
                    .cells
                    .iter()
                    .map(|(c, _)| c)
                    .chain(&state.ending)
                    .map(|c| (c[0] >> 4, c[2] >> 4))
                    .collect()
            };
            let meshed = meshed_chunks().lock().unwrap();
            columns
                .into_iter()
                .filter(|col| meshed.contains(col))
                .collect()
        };
        let mut changed: Vec<Cell> = Vec::new();
        let mut landed: Vec<Cell> = Vec::new();
        let active;
        {
            let mut guard = state().lock();
            let state = &mut *guard;
            state.gated.clear();

            let mut added: FastSet<Cell> = FastSet::default();
            let mut again: FastSet<Cell> = FastSet::default();
            for &(cell, want) in &delta.cells {
                match (want, state.owner(&cell)) {
                    (Some(seq), None) => {
                        state.set_owner(cell, Some(Owner { seq, hidden: true }));
                        added.insert(cell);
                    }
                    (Some(seq), Some(owner)) if seq > owner.seq => {
                        state.set_owner(cell, Some(Owner { seq, hidden: true }));
                        if !owner.hidden {
                            added.insert(cell);
                        }
                    }
                    (Some(_), Some(_)) | (None, None) => {}
                    (None, Some(owner)) => {
                        state.set_owner(cell, None);
                        if owner.hidden {
                            again.insert(cell);
                        }
                    }
                }
            }
            if !added.is_empty() || !again.is_empty() {
                state.version += 1;
            }
            let version = state.version;
            #[cfg(target_arch = "wasm32")]
            for (cells, what) in [(&added, HIDDEN), (&again, SHOWN)] {
                for cell in cells {
                    state.note(*cell, what);
                }
            }

            for &(pos, change) in &delta.moves {
                match change {
                    MoveState::Live { seq, cells } => {
                        if state.moves.get(&pos).is_some_and(|m| m.seq == seq) {
                            continue;
                        }
                        let cells = || cells.iter().flatten();
                        let mut gate = [None; 2];
                        if cells().all(|c| added.contains(c)) {
                            for (slot, c) in gate.iter_mut().zip(cells()) {
                                let sec = section_of(c);
                                *slot = meshed.contains(&(sec.0, sec.2)).then_some(sec);
                            }
                        }
                        for sec in gate.iter().flatten() {
                            state.sections.entry(*sec).or_insert(0);
                        }
                        if gate.iter().any(Option::is_some) {
                            state.gated.push(pos);
                        }
                        let m = Move {
                            seq,
                            start: version,
                            gate,
                            shown: None,
                        };
                        state.moves.insert(pos, m);
                    }
                    MoveState::Left { seq } => {
                        if state
                            .moves
                            .get(&pos)
                            .is_some_and(|m| m.seq == seq && m.shown.is_none())
                        {
                            let version = if again.contains(&pos) { version } else { 0 };
                            state.end(pos, version, section_of(&pos));
                        }
                    }
                    MoveState::Gone => {
                        state.moves.remove(&pos);
                    }
                }
            }

            let mut ending = std::mem::take(&mut state.ending);
            ending.retain(|pos| {
                let Some(m) = state.moves.get(pos).filter(|m| m.shown.is_some()) else {
                    return false;
                };
                if state.is_shown(m) || !meshed.contains(&(pos[0] >> 4, pos[2] >> 4)) {
                    landed.push(*pos);
                }
                true
            });
            state.ending = ending;

            if state.sections.len() > 64 && state.sections.len() > 4 * state.moves.len() {
                let used: FastSet<Section> = state
                    .moves
                    .values()
                    .flat_map(|m| {
                        m.gate
                            .into_iter()
                            .flatten()
                            .chain(m.shown.map(|(_, sec)| sec))
                    })
                    .collect();
                state.sections.retain(|sec, _| used.contains(sec));
            }
            active = !state.moves.is_empty() || !state.cells.is_empty();
            if !active {
                state.sections.clear();
            }
            ACTIVE.store(active, Ordering::Release);
            changed.extend(added);
            changed.extend(again);
        }
        if !changed.is_empty() || active != was_active {
            remesh(&changed);
        }
        landed
    }

    fn remesh(changed: &[Cell]) {
        if crate::client::mesh_worker::forward_hidden() {
            return;
        }
        let (Some(world), Some(q)) = (current_world().lock().unwrap().clone(), CHUNK_Q.get())
        else {
            return;
        };
        let mut jobs: HashSet<(i32, i32, JobScope)> = changed
            .iter()
            .flat_map(|c| remesh_jobs(AzBlockPos::new(c[0], c[1], c[2])))
            .collect();
        {
            let meshed = meshed_chunks().lock().unwrap();
            jobs.retain(|(cx, cz, _)| meshed.contains(&(*cx, *cz)));
        }
        crate::diag::add(crate::diag::Stat::MeshJobs, jobs.len() as u64);
        for (cx, cz, scope) in jobs {
            q.push(world.clone(), cx, cz, scope);
        }
    }

    pub(crate) fn reset() {
        {
            let mut state = state().lock();
            *state = State {
                version: state.version + 1,
                ..State::default()
            };
            if !ACTIVE.swap(false, Ordering::AcqRel) {
                return;
            }
        }
        crate::client::mesh_worker::forward_hidden();
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn export() -> (u32, bool, Vec<(Cell, u8)>) {
        let mut state = state().lock();
        (
            state.version,
            ACTIVE.load(Ordering::Acquire),
            std::mem::take(&mut state.log),
        )
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn install(version: u32, active: bool, cells: &[(Cell, u8)]) {
        let mut state = state().lock();
        state.version = version;
        if !active {
            state.cells = Arc::default();
        }
        for &(cell, what) in cells.iter().filter(|_| active) {
            let owner = (what == HIDDEN).then_some(Owner {
                seq: 0,
                hidden: true,
            });
            state.set_owner(cell, owner);
        }
        ACTIVE.store(active, Ordering::Release);
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_copy_is_drawn_between_the_mesh_without_and_the_mesh_with_its_block() {
            reset();
            let (origin, dest) = ([1, 64, 1], [2, 64, 1]);
            let sec = section_of(&origin);
            meshed_chunks().lock().unwrap().insert((0, 0));
            let hidden = |cells: &Cells| cells.within([0; 3], [15, 79, 15]);
            let drawn = |pos| !withheld().contains(&pos);
            let start = |seq| Delta {
                cells: vec![(origin, Some(seq))],
                moves: vec![(
                    dest,
                    MoveState::Live {
                        seq,
                        cells: [None, Some(origin)],
                    },
                )],
            };
            assert!(sync(&start(7)).is_empty());
            let cells = snapshot().unwrap();
            assert!(hidden(&cells).contains(&origin));
            assert!(!drawn(dest));
            stored(sec, cells.version);
            assert!(drawn(dest));

            let stone = azalea::block::BlockState::from(BlockKind::Stone);
            on_write(AzBlockPos::new(2, 64, 1), stone);
            assert!(drawn(dest));
            stored(sec, snapshot().unwrap().version);
            assert!(!drawn(dest));
            let left = Delta {
                cells: Vec::new(),
                moves: vec![(dest, MoveState::Left { seq: 7 })],
            };
            assert_eq!(sync(&left), vec![dest]);

            on_write(AzBlockPos::new(1, 64, 1), stone);
            assert!(hidden(&snapshot().unwrap()).is_empty());
            sync(&start(7));
            assert!(hidden(&snapshot().unwrap()).is_empty());
            sync(&start(8));
            assert!(hidden(&snapshot().unwrap()).contains(&origin));
            assert!(!drawn(dest));

            let equal = Delta {
                cells: vec![(origin, None), (dest, None)],
                moves: vec![(dest, MoveState::Left { seq: 8 })],
            };
            assert_eq!(sync(&equal), vec![dest]);
            let gone = Delta {
                cells: Vec::new(),
                moves: vec![(dest, MoveState::Gone)],
            };
            sync(&gone);
            assert!(snapshot().is_none());
            meshed_chunks().lock().unwrap().remove(&(0, 0));
        }
    }
}

static CHUNK_GENERATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn chunk_generation() -> u32 {
    CHUNK_GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn bump_chunk_generation() {
    CHUNK_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
    (1, 1),
];

fn surrounded(received: impl Fn((i32, i32)) -> bool, x: i32, z: i32) -> bool {
    NEIGHBOURS.iter().all(|(dx, dz)| received((x + dx, z + dz)))
}

fn ready_after_arrival(
    received: impl Fn((i32, i32)) -> bool,
    meshed: impl Fn((i32, i32)) -> bool,
    cx: i32,
    cz: i32,
) -> Vec<lighting::ColumnPos> {
    std::iter::once((0, 0))
        .chain(NEIGHBOURS)
        .map(|(dx, dz)| (cx + dx, cz + dz))
        .filter(|&t| !meshed(t) && received(t) && surrounded(&received, t.0, t.1))
        .map(|(x, z)| lighting::ColumnPos { x, z })
        .collect()
}

global! {
fn resent_waiting() -> crate::session::SideMutex<HashSet<(i32, i32)>>
}

static RESENT_WAITING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn forget_resent(cx: i32, cz: i32) {
    if RESENT_WAITING.load(Ordering::Relaxed) {
        let mut waiting = resent_waiting().lock().unwrap();
        waiting.remove(&(cx, cz));
        RESENT_WAITING.store(!waiting.is_empty(), Ordering::Relaxed);
    }
}

fn clear_resent() {
    if RESENT_WAITING.load(Ordering::Relaxed) {
        resent_waiting().lock().unwrap().clear();
        RESENT_WAITING.store(false, Ordering::Relaxed);
    }
}

pub(crate) const MESH_FALLBACK_AFTER: std::time::Duration = std::time::Duration::from_secs(5);

const FALLBACK_SCAN_EVERY: u32 = 10;

global! {
fn arrivals() -> crate::session::SideMutex<HashMap<(i32, i32), crate::platform::time::Instant>>
}

fn note_arrival(cx: i32, cz: i32) {
    arrivals()
        .lock()
        .unwrap()
        .entry((cx, cz))
        .or_insert_with(crate::platform::time::Instant::now);
}

fn forget_arrival(cx: i32, cz: i32) {
    arrivals().lock().unwrap().remove(&(cx, cz));
}

fn clear_arrivals() {
    arrivals().lock().unwrap().clear();
}

fn skipped_over(received: impl Fn((i32, i32)) -> bool, x: i32, z: i32, reach: i32) -> bool {
    let toward = |dx: i32, dz: i32| (1..=reach).any(|d| received((x + dx * d, z + dz * d)));
    (toward(-1, 0) && toward(1, 0)) || (toward(0, -1) && toward(0, 1))
}

pub(crate) fn mesh_waiting_columns(bot: &Client) {
    static TICKS: AtomicU32 = AtomicU32::new(0);
    if TICKS.fetch_add(1, Ordering::Relaxed) % FALLBACK_SCAN_EVERY != 0 {
        return;
    }
    let server = azalea_world::retain::server_radius();
    if server == 0 {
        return;
    }
    let requested = bot
        .component::<azalea::ClientInformation>()
        .map(|info| info.view_distance as u32)
        .unwrap_or(server);
    let radius = requested.clamp(2, server.max(2));
    let (Ok(pos), Ok(world)) = (bot.position(), bot.world()) else {
        return;
    };
    let (px, pz) = ((pos.x.floor() as i32) >> 4, (pos.z.floor() as i32) >> 4);
    let overdue: Vec<(i32, i32)> = arrivals()
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, at)| at.elapsed() >= MESH_FALLBACK_AFTER)
        .map(|(col, _)| *col)
        .collect();
    if overdue.is_empty() {
        return;
    }
    let mut settled = Vec::new();
    let mut candidates = Vec::new();
    {
        let received = received_chunks().lock().unwrap();
        let meshed = meshed_chunks().lock().unwrap();
        for col in overdue {
            if !received.contains(&col) || meshed.contains(&col) {
                settled.push(col);
                continue;
            }
            let missing: Vec<(i32, i32)> = NEIGHBOURS
                .iter()
                .map(|(dx, dz)| (col.0 + dx, col.1 + dz))
                .filter(|n| !received.contains(n))
                .collect();
            if missing.is_empty() {
                continue;
            }
            let late = missing.iter().all(|&(nx, nz)| {
                crate::util::chunk_tracking::is_in_view_distance(px, pz, radius, nx, nz)
                    && skipped_over(|c| received.contains(&c), nx, nz, radius as i32)
            });
            if late {
                candidates.push((col, missing));
            }
        }
    }
    {
        let mut arrivals = arrivals().lock().unwrap();
        for col in &settled {
            arrivals.remove(col);
        }
    }
    let to_worker = crate::client::mesh_worker::active();
    if !to_worker && crate::renderer::lighting_enabled() {
        let map = light_map().read();
        candidates.retain(|((x, z), _)| map.is_lit(lighting::ColumnPos { x: *x, z: *z }));
    }
    if candidates.is_empty() {
        return;
    }
    let mut taken = Vec::new();
    {
        let received = received_chunks().lock().unwrap();
        let mut meshed = meshed_chunks().lock().unwrap();
        let mut waiting = resent_waiting().lock().unwrap();
        for (col, missing) in candidates {
            if !received.contains(&col) || !meshed.insert(col) {
                continue;
            }
            waiting.insert(col);
            taken.push((col, missing));
        }
        RESENT_WAITING.store(!waiting.is_empty(), Ordering::Relaxed);
    }
    {
        let mut arrivals = arrivals().lock().unwrap();
        for (col, _) in &taken {
            arrivals.remove(col);
        }
    }
    let before = crate::diag::get(crate::diag::Stat::MeshedWithoutNeighbours);
    crate::diag::add(
        crate::diag::Stat::MeshedWithoutNeighbours,
        taken.len() as u64,
    );
    for (n, ((cx, cz), missing)) in taken.into_iter().enumerate() {
        if before + (n as u64) < 16 {
            log_warn!(
                "mesh",
                "column {cx},{cz} waited {}s for its neighbours {missing:?}, which the server \
                 streamed past and have not arrived; meshed with that side as air, rebuilt when \
                 they come",
                MESH_FALLBACK_AFTER.as_secs()
            );
        }
        let col = lighting::ColumnPos { x: cx, z: cz };
        if to_worker {
            crate::client::mesh_worker::forward_chunk(&world, cx, cz, &[col], None);
        } else if let Some(q) = CHUNK_Q.get() {
            crate::diag::bump(crate::diag::Stat::MeshJobs);
            q.push(world.clone(), cx, cz, JobScope::Column);
        }
    }
}

pub(crate) fn chunk_decode_failed(
    pos: azalea_core::position::ChunkPos,
    failure: &azalea_world::chunk::ChunkDecodeFailure,
) {
    let biome_update = failure.part == azalea_world::chunk::BIOME_UPDATE;
    if !biome_update {
        crate::diag::bump(crate::diag::Stat::ChunkDecodeFailed);
    }
    {
        let mut logged = decode_logged().lock().unwrap();
        let now = crate::platform::time::Instant::now();
        if logged
            .get(&(pos.x, pos.z))
            .is_some_and(|at| now.duration_since(*at) < DECODE_LOG_EVERY)
        {
            return;
        }
        if logged.len() >= 4096 {
            logged.clear();
        }
        logged.insert((pos.x, pos.z), now);
    }
    log_warn!(
        "chunks",
        "column {},{}: section {} of {} would not decode ({}: {} bits per entry sent, read at {}, \
         {} palette{}): {}. {}",
        pos.x,
        pos.z,
        failure.section,
        failure.sections,
        failure.part,
        failure
            .sent_bits
            .map_or("no".to_string(), |b| b.to_string()),
        failure.read_bits.map_or("?".to_string(), |b| b.to_string()),
        failure
            .palette
            .as_ref()
            .map_or("unknown".to_string(), |k| format!("{k:?}").to_lowercase()),
        if failure.read_as_sent {
            ", second reading at the widths sent"
        } else {
            ""
        },
        failure.error,
        if biome_update {
            "The column keeps the biomes it had.".to_string()
        } else {
            format!(
                "Kept the {} sections below it; the rest of the column is air.",
                failure.section
            )
        }
    );
}

const DECODE_LOG_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

global! {
fn decode_logged() -> crate::session::SideMutex<HashMap<(i32, i32), crate::platform::time::Instant>>
}

pub(crate) enum Arrival<'a> {
    Packet(&'a azalea_protocol::packets::game::c_level_chunk_with_light::ClientboundLevelChunkWithLight),
    Replay,
}

pub(crate) fn note_chunk_received(
    world: &Arc<parking_lot::RwLock<azalea_world::World>>,
    cx: i32,
    cz: i32,
    arrival: Arrival<'_>,
) {
    crate::diag::on_chunk_received();
    crate::modules::seed_xray::chunk_arrived();
    crate::modules::basefinder::chunk_arrived();
    #[cfg(feature = "seed_cracker")]
    crate::modules::seed_cracker::chunk_arrived();
    bump_chunk_generation();
    #[cfg(lua_mods)]
    {
        crate::modules::script::ghost::chunk_changed(cx, cz);
        crate::modules::script::chunk_arrived(cx, cz);
    }
    log_debug!("chunks", "received column {cx},{cz}");
    #[cfg(feature = "render_lod")]
    crate::client::lod::note_column(cx, cz);
    note_arrival(cx, cz);
    let mut received = received_chunks().lock().unwrap();
    let resent = !received.insert((cx, cz));
    let mut meshed = meshed_chunks().lock().unwrap();
    #[cfg(feature = "render_lod")]
    if crate::client::lod::cache::reclaim(cx, cz) {
        meshed.remove(&(cx, cz));
    }
    let resend = resent && meshed.contains(&(cx, cz));
    let resend_ready = resend && surrounded(|c| received.contains(&c), cx, cz);
    let mut waiting = (resend || RESENT_WAITING.load(Ordering::Relaxed))
        .then(|| resent_waiting().lock().unwrap());
    if resend && let Some(waiting) = waiting.as_mut() {
        if resend_ready {
            waiting.remove(&(cx, cz));
        } else {
            waiting.insert((cx, cz));
        }
    }
    let mut ready = ready_after_arrival(
        |c| received.contains(&c),
        |c| meshed.contains(&c) && !waiting.as_ref().is_some_and(|w| w.contains(&c)),
        cx,
        cz,
    );
    if resend_ready {
        ready.push(lighting::ColumnPos { x: cx, z: cz });
    }
    if let Some(waiting) = waiting.as_mut() {
        for col in &ready {
            waiting.remove(&(col.x, col.z));
        }
        RESENT_WAITING.store(!waiting.is_empty(), Ordering::Relaxed);
    }
    drop(waiting);
    let to_worker = crate::client::mesh_worker::active();
    if to_worker {
        for col in &ready {
            meshed.insert((col.x, col.z));
        }
    }
    drop(meshed);
    drop(received);
    let packet = match arrival {
        Arrival::Packet(p) => Some(p),
        Arrival::Replay => None,
    };
    if to_worker {
        crate::client::mesh_worker::forward_chunk(world, cx, cz, &ready, packet);
        return;
    }
    send_light(lighting::LightJob::Chunk {
        world: world.clone(),
        col: lighting::ColumnPos { x: cx, z: cz },
        mesh: ready,
        light: packet.map(|p| crate::client::packets::server_light(&p.light_data)),
    });
}

pub(crate) fn note_biomes_changed(
    world: &azalea::local_player::WorldHolder,
    packet: &azalea_protocol::packets::game::c_chunks_biomes::ClientboundChunksBiomes,
) {
    let loaded: Vec<_> = {
        let partial = world.partial.read();
        packet
            .chunk_biome_data
            .iter()
            .filter(|data| partial.chunks.limited_get(&data.pos).is_some())
            .collect()
    };
    if loaded.is_empty() {
        return;
    }
    bump_chunk_generation();
    #[cfg(feature = "render_lod")]
    for data in &loaded {
        crate::client::lod::note_column(data.pos.x, data.pos.z);
    }
    if crate::client::mesh_worker::forward_biomes(&world.shared, &loaded) {
        return;
    }
    let Some(q) = CHUNK_Q.get() else { return };
    let mut cols = Vec::new();
    {
        let meshed = meshed_chunks().lock().unwrap();
        let mut seen = HashSet::new();
        for (x, z) in biome_remesh(loaded.iter().map(|data| (data.pos.x, data.pos.z))) {
            if meshed.contains(&(x, z)) && seen.insert((x, z)) {
                cols.push((x, z));
            }
        }
    }
    crate::diag::add(crate::diag::Stat::MeshJobs, cols.len() as u64);
    for (cx, cz) in cols {
        q.push(world.shared.clone(), cx, cz, JobScope::Column);
    }
}

pub(crate) fn biome_remesh(
    changed: impl Iterator<Item = (i32, i32)>,
) -> impl Iterator<Item = (i32, i32)> {
    changed.flat_map(|(x, z)| {
        std::iter::once((0, 0))
            .chain(NEIGHBOURS)
            .map(move |(dx, dz)| (x + dx, z + dz))
    })
}

pub(crate) fn send_server_light(cx: i32, cz: i32, light: lighting::ServerLight) {
    send_light(lighting::LightJob::Light {
        col: lighting::ColumnPos { x: cx, z: cz },
        light,
    });
}

pub(crate) fn reload_all_chunks(bot: &Client, shared: &SharedMutex) {
    let mut cols: Vec<(i32, i32)> = received_chunks().lock().unwrap().iter().copied().collect();
    sort_nearest_first(bot, &mut cols);
    if let Some(q) = CHUNK_Q.get() {
        q.next_level();
    }
    reset_chunk_tracking();
    clear_resent();
    clear_arrivals();
    if let Some(q) = CHUNK_Q.get() {
        q.reset();
    }
    {
        let mut s = shared.lock().unwrap();
        s.session.clear_chunks = true;
        s.session.pending_chunks.clear();
        s.session.pending_edits.clear();
        s.session.meshed_columns.cleared();
        s.session.unloaded_chunks.clear();
    }
    if !crate::client::mesh_worker::forward_reset_light() {
        send_light(lighting::LightJob::Reset);
    }
    let Ok(world) = bot.world() else { return };
    for (cx, cz) in cols {
        note_chunk_received(&world, cx, cz, Arrival::Replay);
    }
}

pub(crate) fn remesh_all_chunks(bot: &Client) {
    if crate::client::mesh_worker::active() {
        #[cfg(resource_packs)]
        crate::client::mesh_worker::forward_packs();
        return;
    }
    let (Ok(world), Some(q)) = (bot.world(), CHUNK_Q.get()) else {
        return;
    };
    let mut cols: Vec<(i32, i32)> = meshed_chunks().lock().unwrap().iter().copied().collect();
    sort_nearest_first(bot, &mut cols);
    for (cx, cz) in cols {
        q.push(world.clone(), cx, cz, JobScope::Column);
    }
}

fn sort_nearest_first(bot: &Client, cols: &mut [(i32, i32)]) {
    let Ok(pos) = bot.position() else { return };
    let (px, pz) = ((pos.x.floor() as i32) >> 4, (pos.z.floor() as i32) >> 4);
    cols.sort_unstable_by_key(|&(x, z)| (x - px).pow(2) + (z - pz).pow(2));
}

const COMPACT_INTERVAL: u32 = 256;
static FORGOTTEN_SINCE_COMPACT: AtomicU32 = AtomicU32::new(0);

pub(crate) fn forget_chunk(
    world: &azalea::local_player::WorldHolder,
    shared: &SharedMutex,
    cx: i32,
    cz: i32,
) {
    blockentities::feed::on_forget_chunk(cx, cz);
    bump_chunk_generation();
    received_chunks().lock().unwrap().remove(&(cx, cz));
    meshed_chunks().lock().unwrap().remove(&(cx, cz));
    forget_resent(cx, cz);
    forget_arrival(cx, cz);
    if let Some(q) = CHUNK_Q.get() {
        q.forget_chunk(cx, cz);
    }
    shared
        .lock()
        .unwrap()
        .session
        .unloaded_chunks
        .push((cx, cz));
    if !crate::client::mesh_worker::forward_unload(cx, cz) {
        send_light(lighting::LightJob::Unload {
            col: lighting::ColumnPos { x: cx, z: cz },
        });
    }

    if FORGOTTEN_SINCE_COMPACT.fetch_add(1, Ordering::Relaxed) + 1 >= COMPACT_INTERVAL {
        FORGOTTEN_SINCE_COMPACT.store(0, Ordering::Relaxed);
        world.shared.write().chunks.compact_dead();
    }
}

const MESH_RETRIES: u8 = 2;

pub(crate) struct LostJob {
    pub cx: i32,
    pub cz: i32,
    pub seq: u64,
    pub epoch: u32,
    pub attempt: u8,
}

enum LostOutcome {
    Requeue,
    GaveUp,
    NoBlocks,
}

pub(crate) fn mesh_job_lost(
    q: &crate::client::worker::JobQueue,
    world: &Arc<parking_lot::RwLock<azalea_world::World>>,
    job: LostJob,
) {
    let LostJob {
        cx,
        cz,
        seq,
        epoch,
        attempt,
    } = job;
    if column_kept_by_lod(cx, cz) {
        return;
    }
    let present = world
        .read()
        .chunks
        .get(&azalea_core::position::ChunkPos::new(cx, cz))
        .is_some();
    let outcome = {
        let received = received_chunks().lock().unwrap();
        let meshed = meshed_chunks().lock().unwrap();
        if !meshed.contains(&(cx, cz)) || !q.owns_any(cx, cz, seq, epoch) {
            log_debug!(
                "mesh",
                "a job for {cx},{cz} built nothing; a newer one owns the column"
            );
            return;
        }
        let ready = received.contains(&(cx, cz)) && surrounded(|c| received.contains(&c), cx, cz);
        if present && ready && attempt < MESH_RETRIES {
            LostOutcome::Requeue
        } else {
            let mut waiting = resent_waiting().lock().unwrap();
            waiting.insert((cx, cz));
            RESENT_WAITING.store(true, Ordering::Relaxed);
            if present {
                LostOutcome::GaveUp
            } else {
                LostOutcome::NoBlocks
            }
        }
    };
    match outcome {
        LostOutcome::Requeue => {
            crate::diag::bump(crate::diag::Stat::MeshRequeued);
            log_warn!(
                "mesh",
                "column {cx},{cz} ended its mesh job with nothing built; queued again ({} of \
                 {MESH_RETRIES})",
                attempt + 1
            );
            q.push_retry(world.clone(), cx, cz, attempt + 1);
        }
        LostOutcome::GaveUp => {
            crate::diag::bump(crate::diag::Stat::MeshGaveUp);
            log_error!(
                "mesh",
                "column {cx},{cz} ended its mesh job with nothing built {} times; left unbuilt \
                 until it next arrives or a neighbour does",
                attempt + 1
            );
        }
        LostOutcome::NoBlocks => {
            log_debug!(
                "mesh",
                "column {cx},{cz} has no blocks in azalea's storage; left for the next arrival \
                 beside it"
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn mesh_worker_lost(cx: i32, cz: i32) {
    let Some(world) = current_world().lock().unwrap().clone() else {
        return;
    };
    if column_kept_by_lod(cx, cz) {
        return;
    }
    let present = world
        .read()
        .chunks
        .get(&azalea_core::position::ChunkPos::new(cx, cz))
        .is_some();
    let (outcome, attempt) = {
        let received = received_chunks().lock().unwrap();
        let mut meshed = meshed_chunks().lock().unwrap();
        if !meshed.contains(&(cx, cz)) {
            return;
        }
        if !present {
            meshed.remove(&(cx, cz));
            (LostOutcome::NoBlocks, 0)
        } else {
            let attempt = crate::client::mesh_worker::note_retry(cx, cz);
            let ready =
                received.contains(&(cx, cz)) && surrounded(|c| received.contains(&c), cx, cz);
            if ready && attempt <= MESH_RETRIES {
                (LostOutcome::Requeue, attempt)
            } else {
                meshed.remove(&(cx, cz));
                (LostOutcome::GaveUp, attempt)
            }
        }
    };
    match outcome {
        LostOutcome::Requeue => {
            crate::diag::bump(crate::diag::Stat::MeshRequeued);
            log_warn!(
                "mesh",
                "the mesh worker had no blocks for column {cx},{cz}; asked again ({attempt} of \
                 {MESH_RETRIES})"
            );
            let col = lighting::ColumnPos { x: cx, z: cz };
            crate::client::mesh_worker::forward_chunk(&world, cx, cz, &[col], None);
        }
        LostOutcome::GaveUp => {
            crate::diag::bump(crate::diag::Stat::MeshGaveUp);
            log_error!(
                "mesh",
                "the mesh worker has no blocks for column {cx},{cz}; left unbuilt until the \
                 server sends it again"
            );
        }
        LostOutcome::NoBlocks => {
            log_debug!(
                "mesh",
                "column {cx},{cz} has no blocks in azalea's storage; unmarked"
            );
        }
    }
}

#[cfg(feature = "render_lod")]
pub(crate) fn column_kept_by_lod(cx: i32, cz: i32) -> bool {
    crate::client::lod::cache::is_cached(cx, cz)
}

#[cfg(not(feature = "render_lod"))]
pub(crate) fn column_kept_by_lod(_cx: i32, _cz: i32) -> bool {
    false
}

pub(crate) fn drop_maps(shared: &SharedMutex) {
    reset_maps();
    let mut s = shared.lock().unwrap();
    s.session.map_updates.clear();
    s.session.clear_maps = true;
}

pub(crate) fn reset_level(world: &azalea::local_player::WorldHolder, shared: &SharedMutex) {
    set_current_world(&world.shared);
    if let Some(q) = CHUNK_Q.get() {
        q.next_level();
    }
    reset_chunk_tracking();
    clear_resent();
    bump_chunk_generation();
    crate::renderer::environment::reset_weather();
    crate::client::envprobe::reset();
    if !crate::client::mesh_worker::forward_reset_level() {
        send_light(lighting::LightJob::Reset);
    }
    crate::client::viewwindow::reset_positioned();
    clear_arrivals();
    if let Some(q) = CHUNK_Q.get() {
        q.reset();
    }
    blockentities::feed::reset();
    #[cfg(feature = "render_lod")]
    crate::client::lod::reset();
    let mut s = shared.lock().unwrap();
    s.session.block_entities = Default::default();
    s.session.block_entities_version = s.session.block_entities_version.wrapping_add(1);
    s.session.clear_chunks = true;
    s.session.pending_chunks.clear();
    s.session.pending_edits.clear();
    s.session.meshed_columns.cleared();
    s.session.unloaded_chunks.clear();
}

pub(crate) fn set_dimension(
    world: &azalea::local_player::WorldHolder,
    common: &azalea_protocol::packets::common::CommonPlayerSpawnInfo,
) {
    use crate::renderer::dimension::Dimension;

    let type_path = {
        let shared = world.shared.read();
        common
            .dimension_type(&shared.registries)
            .map(|(id, _)| id.path().to_owned())
    };
    let path = type_path.unwrap_or_else(|| common.dimension.path().to_owned());
    crate::renderer::dimension::set_current(Dimension::from_path(&path));
    crate::renderer::dimension::set_level(&common.dimension);
    crate::renderer::dimension::set_sea_level(common.sea_level);
    crate::modules::seed_xray::spawned(common.seed);
    crate::modules::sound_esp::spawned(&common.dimension.to_string());
    crate::modules::basefinder::spawned(&common.dimension.to_string());
}

#[cfg(test)]
mod readiness_tests {
    use super::*;
    use std::collections::HashSet;

    fn ready(
        received: &HashSet<(i32, i32)>,
        meshed: &HashSet<(i32, i32)>,
        at: (i32, i32),
    ) -> Vec<(i32, i32)> {
        let mut v: Vec<_> = ready_after_arrival(
            |c| received.contains(&c),
            |c| meshed.contains(&c),
            at.0,
            at.1,
        )
        .into_iter()
        .map(|c| (c.x, c.z))
        .collect();
        v.sort();
        v
    }

    fn block_without(missing: &[(i32, i32)]) -> HashSet<(i32, i32)> {
        let mut s = HashSet::new();
        for x in -1..=1 {
            for z in -1..=1 {
                if !missing.contains(&(x, z)) {
                    s.insert((x, z));
                }
            }
        }
        s
    }

    #[test]
    fn the_edge_of_the_stream_is_not_skipped_over() {
        let mut received = HashSet::new();
        for x in -4..=4 {
            for z in -4..=4 {
                received.insert((x, z));
            }
        }
        let has = |c: (i32, i32)| received.contains(&c);
        for k in -5..=5 {
            for edge in [(5, k), (-5, k), (k, 5), (k, -5)] {
                assert!(!skipped_over(has, edge.0, edge.1, 10), "{edge:?}");
            }
        }
        received.remove(&(1, 2));
        received.remove(&(4, 0));
        let has = |c: (i32, i32)| received.contains(&c);
        assert!(skipped_over(has, 1, 2, 10));
        assert!(skipped_over(has, 4, 0, 10));
    }

    #[test]
    fn a_lost_block_is_skipped_over_and_a_lost_corner_is_not() {
        let mut received = HashSet::new();
        for x in -4..=4 {
            for z in -4..=4 {
                received.insert((x, z));
            }
        }
        let block = [(0, 0), (1, 0), (0, 1), (1, 1)];
        for lost in block {
            received.remove(&lost);
        }
        received.remove(&(4, 4));
        let has = |c: (i32, i32)| received.contains(&c);
        for lost in block {
            assert!(skipped_over(has, lost.0, lost.1, 10), "{lost:?}");
            assert!(!skipped_over(has, lost.0, lost.1, 1), "{lost:?}");
        }
        assert!(!skipped_over(has, 4, 4, 10));
    }

    #[test]
    fn a_missing_corner_holds_the_column_back() {
        let received = block_without(&[(1, 1)]);
        let meshed = HashSet::new();
        assert!(ready(&received, &meshed, (0, 0)).is_empty());
    }

    #[test]
    fn the_last_corner_completes_its_diagonal() {
        let received = block_without(&[]);
        let meshed = HashSet::new();
        assert_eq!(ready(&received, &meshed, (1, 1)), vec![(0, 0)]);
    }

    #[test]
    fn meshed_and_absent_columns_are_skipped() {
        let received = block_without(&[]);
        let meshed: HashSet<_> = [(0, 0)].into_iter().collect();
        assert!(ready(&received, &meshed, (1, 1)).is_empty());

        let received = block_without(&[(0, 0)]);
        assert!(ready(&received, &HashSet::new(), (1, 1)).is_empty());
    }

    #[test]
    fn a_square_meshes_its_interior() {
        let mut received = HashSet::new();
        let mut meshed = HashSet::new();
        for x in -2..=2 {
            for z in -2..=2 {
                received.insert((x, z));
                for c in ready(&received, &meshed, (x, z)) {
                    meshed.insert(c);
                }
            }
        }
        assert_eq!(meshed, block_without(&[]));
    }
}

#[cfg(test)]
mod remesh_tests {
    use super::*;
    use std::collections::HashSet;

    fn jobs(x: i32, y: i32, z: i32) -> Vec<(i32, i32, JobScope)> {
        remesh_jobs(AzBlockPos::new(x, y, z))
    }

    #[test]
    fn interior_block_touches_one_section() {
        assert_eq!(jobs(8, 72, 8), vec![(0, 0, JobScope::Section(4))]);
    }

    #[test]
    fn section_boundary_pulls_in_the_neighbour_section() {
        assert_eq!(
            jobs(8, 64, 8),
            vec![(0, 0, JobScope::Section(4)), (0, 0, JobScope::Section(3))]
        );
        assert_eq!(
            jobs(8, 79, 8),
            vec![(0, 0, JobScope::Section(4)), (0, 0, JobScope::Section(5))]
        );
    }

    #[test]
    fn chunk_edges_pull_in_the_neighbour_chunk() {
        assert_eq!(
            jobs(-16, 72, 8),
            vec![(-1, 0, JobScope::Section(4)), (-2, 0, JobScope::Section(4))]
        );
        assert_eq!(
            jobs(-1, 72, 8),
            vec![(-1, 0, JobScope::Section(4)), (0, 0, JobScope::Section(4))]
        );
    }

    #[test]
    fn corner_of_a_section_hits_every_neighbour() {
        let got: HashSet<_> = jobs(0, 64, 0).into_iter().collect();
        let want: HashSet<_> = [
            (0, 0, JobScope::Section(4)),
            (0, 0, JobScope::Section(3)),
            (-1, 0, JobScope::Section(4)),
            (0, -1, JobScope::Section(4)),
        ]
        .into_iter()
        .collect();
        assert_eq!(got, want);
    }
}
