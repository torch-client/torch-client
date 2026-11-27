#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use azalea_core::position::ChunkPos;
use seedcrack::cracker::{BEDROCK_TARGET, OVERWORLD_BEDROCK_CAP};
use seedcrack::{
    Band, BedrockCell, Event, FinderKind, Mode, Observation, Trust, Version, Yields, scan_chunk,
};

use super::cache::{self, Cache, Drawn};
use super::hud;
use super::search::{Cmd, Out, Search, band_name, bedrock_progress, hint_text, stage_name};
use super::{
    CHUNK_ARRIVED, CrackMode, EDGE, FINDERS, IMPORT, OWNER, Request, SESSION, take_request,
};
use crate::modules::chat_math::{say, show_local_events};
use crate::modules::esp::blocks::id_of;
use crate::modules::registry::{
    commands as prefix_setting, seed_cracker as setting, seed_xray as xray_setting,
};
use crate::modules::seed_xray::terrain::{Handle, Region};
use crate::modules::{Phase, nearest_first, store};
use crate::play::dialog::Click;
use crate::renderer::dimension::{self, Dimension};
use crate::renderer::world_draw::{self, Node};
use crate::session::SharedMutex;
use crate::text::events::{Hover, SpanEvent};
use crate::text::{AQUA, GRAY, GREEN, RED, Span, Style, WHITE, YELLOW};

fn tell(shared: &SharedMutex, rgb: u32, text: impl Into<String>) {
    if rgb == RED || !store().flag(setting::HUD) {
        say(shared, rgb, text);
    }
}

const BUDGET: usize = 4;

const DEFAULT_RADIUS: u32 = 12;
const MAX_RADIUS: i32 = 33;

const FEW: usize = 8;

const FILL: u32 = 0x30;

const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

type BedrockRow = (Band, i32, &'static str, bool);

static NETHER_BEDROCK: [BedrockRow; 4] = [
    (Band::NetherFloor, 4, "bedrock", true),
    (Band::NetherFloor, 1, "netherrack", false),
    (Band::NetherRoof, 123, "bedrock", true),
    (Band::NetherRoof, 126, "netherrack", false),
];

static OVERWORLD_BEDROCK: [BedrockRow; 8] = [
    (Band::OverworldFloor, -63, "bedrock", true),
    (Band::OverworldFloor, -63, "deepslate", false),
    (Band::OverworldFloor, -62, "bedrock", true),
    (Band::OverworldFloor, -62, "deepslate", false),
    (Band::OverworldFloor, -61, "bedrock", true),
    (Band::OverworldFloor, -61, "deepslate", false),
    (Band::OverworldFloor, -60, "bedrock", true),
    (Band::OverworldFloor, -60, "deepslate", false),
];

fn bedrock_rows(dimension: Dimension, (min_y, height): (i32, i32)) -> &'static [BedrockRow] {
    match dimension {
        Dimension::Nether if min_y == 0 => &NETHER_BEDROCK[..],
        Dimension::Overworld if (min_y, height) == (-64, 384) => &OVERWORLD_BEDROCK[..],
        _ => &[],
    }
}

fn band_slot(band: Band) -> usize {
    match band {
        Band::NetherFloor => 0,
        Band::NetherRoof => 1,
        Band::OverworldFloor => 2,
    }
}

fn bedrock_cap(band: Band) -> usize {
    match band {
        Band::NetherFloor | Band::NetherRoof => BEDROCK_TARGET,
        Band::OverworldFloor => OVERWORLD_BEDROCK_CAP,
    }
}

fn settings() -> (Mode, bool) {
    let s = store();
    let mode = match s.choice(setting::MODE) {
        CrackMode::Normal => Mode::Normal,
        CrackMode::Fast => Mode::Fast,
    };
    (mode, s.flag(setting::BEDROCK))
}

const SAVE_AFTER: u32 = 20;

const PARKED_MAX: usize = 2;

struct Parked {
    hash: i64,
    search: Option<Search>,
    found: HashMap<Observation, Drawn>,
    seed: Option<i64>,
    trusted: Option<Trust>,
}

struct State {
    session: u32,
    version: Option<Version>,
    search: Option<Search>,
    hash: i64,
    dimension: Dimension,
    scanned: HashSet<(i32, i32)>,
    centre: Option<(i32, i32)>,
    backlog: bool,
    biomes: Vec<String>,
    found: HashMap<Observation, Drawn>,
    ignored: HashSet<Observation>,
    redraw: bool,
    shown: bool,
    target: Option<i64>,
    foreign: bool,
    seed: Option<i64>,
    unsaved: Option<u32>,
    rejoined: bool,
    parked: Vec<Parked>,
    warned_hidden: bool,
    bedrock: [HashSet<BedrockCell>; 3],
    applied: Option<(Mode, bool)>,
    trusted: Option<Trust>,
}

#[cfg(not(target_arch = "wasm32"))]
static STATE: std::sync::Mutex<Option<State>> = std::sync::Mutex::new(None);

#[cfg(not(target_arch = "wasm32"))]
fn with_state<R>(f: impl FnOnce(&mut Option<State>) -> R) -> R {
    f(&mut STATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner))
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn with_state<R>(f: impl FnOnce(&mut Option<State>) -> R) -> R {
    STATE.with_borrow_mut(f)
}

pub(super) fn flush() {
    let save = |slot: &mut Option<State>| {
        if let Some(st) = slot.as_mut()
            && st.unsaved.is_some()
        {
            st.save();
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(mut slot) = STATE.try_lock() {
        save(&mut *slot);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = STATE.try_with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut() {
            save(&mut *slot);
        }
    });
}

pub(super) fn finder_on(kind: FinderKind) -> bool {
    FINDERS
        .find(kind.name())
        .is_some_and(|i| FINDERS.enabled(i))
}

pub(super) fn to_seedcrack(dimension: Dimension) -> seedcrack::Dimension {
    match dimension {
        Dimension::Overworld => seedcrack::Dimension::Overworld,
        Dimension::Nether => seedcrack::Dimension::Nether,
        Dimension::End => seedcrack::Dimension::End,
    }
}

fn biome_names(registries: &azalea_core::registry_holder::RegistryHolder) -> Vec<String> {
    crate::util::biome_color::map_biomes(registries, str::to_owned)
}

fn chunk_of(observation: &Observation, scanned: (i32, i32)) -> (i32, i32) {
    match observation {
        Observation::Structure(s) => (s.chunk_x, s.chunk_z),
        Observation::Decorator(d) => d.chunk(),
        _ => scanned,
    }
}

fn listed<T: std::fmt::Display>(values: &[T]) -> String {
    if values.is_empty() || values.len() > FEW {
        return String::new();
    }
    let values: Vec<String> = values.iter().map(T::to_string).collect();
    format!(": {}", values.join(", "))
}

struct View<'a> {
    region: Region<'a>,
    biomes: &'a [String],
    bounds: (i32, i32),
}

impl seedcrack::World for View<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> &str {
        self.region.state(x, y, z).map_or("", id_of)
    }

    fn biome(&self, x: i32, y: i32, z: i32) -> &str {
        self.region
            .quart_biome([x >> 2, y >> 2, z >> 2])
            .and_then(|id| self.biomes.get(id as usize))
            .map_or("", String::as_str)
    }

    fn min_y(&self) -> i32 {
        self.bounds.0
    }

    fn max_y(&self) -> i32 {
        self.bounds.0 + self.bounds.1
    }
}

impl State {
    fn new(shared: &SharedMutex, session: u32, version: Option<Version>) -> State {
        let (mode, bedrock) = settings();
        let search = match version {
            Some(version) => {
                let search = Search::start(version, mode, bedrock);
                if search.is_none() {
                    tell(shared, RED, crate::lang::tr!("seed_cracker.no_thread"));
                    hud::off();
                }
                search
            }
            None => {
                let protocol = crate::util::session_protocol();
                tell(
                    shared,
                    RED,
                    crate::lang::trf!("seed_cracker.no_tables", protocol),
                );
                hud::off();
                None
            }
        };
        State {
            session,
            version,
            search,
            hash: 0,
            dimension: dimension::current(),
            scanned: HashSet::new(),
            centre: None,
            backlog: true,
            biomes: Vec::new(),
            found: HashMap::new(),
            ignored: HashSet::new(),
            redraw: true,
            shown: false,
            target: None,
            foreign: false,
            seed: None,
            unsaved: None,
            rejoined: false,
            parked: Vec::new(),
            warned_hidden: false,
            bedrock: Default::default(),
            applied: Some((mode, bedrock)),
            trusted: None,
        }
    }

    fn forget_bedrock(&mut self) {
        self.bedrock = Default::default();
        self.ignored.clear();
    }

    fn export_bedrock(&self, shared: &SharedMutex) {
        let mut cells: Vec<&BedrockCell> = self.bedrock[band_slot(Band::OverworldFloor)]
            .iter()
            .collect();
        if cells.is_empty() {
            tell(shared, RED, crate::lang::tr!("seed_cracker.export.none"));
            return;
        }
        cells.sort();
        let rows: Vec<_> = cells
            .iter()
            .map(|c| serde_json::json!({ "x": c.x(), "y": c.y(), "z": c.z(), "bedrock": c.bedrock() }))
            .collect();
        let doc =
            serde_json::json!({ "target_hash": self.hash, "k": 0, "cells": rows }).to_string();
        crate::platform::clipboard::set(&doc);
        say(
            shared,
            GREEN,
            crate::lang::trf!("seed_cracker.export.copied", cells.len()),
        );
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = crate::platform::storage::dir().join("seed_cracker_bedrock.json");
            match std::fs::write(&path, &doc) {
                Ok(()) => say(
                    shared,
                    GRAY,
                    crate::lang::trf!("seed_cracker.export.saved", path.display()),
                ),
                Err(e) => tell(
                    shared,
                    RED,
                    crate::lang::trf!("seed_cracker.export.failed", path.display(), e),
                ),
            }
        }
    }

    fn rejoin(&mut self, session: u32) {
        self.session = session;
        self.scanned.clear();
        self.centre = None;
        self.backlog = true;
        self.biomes.clear();
        self.hash = 0;
        self.dimension = dimension::current();
        self.redraw = true;
        self.rejoined = true;
        self.warned_hidden = false;
    }

    fn restore(&mut self, shared: &SharedMutex, hash: i64) {
        self.target = Some(hash);
        self.forget_bedrock();
        if let Some(c) = cache::load(hash) {
            let n = self.take_in(c.finds);
            if n > 0 {
                say(shared, GRAY, crate::lang::trn!("seed_cracker.restored", n));
            }
            if let Some(seed) = c.seed
                && self.seed.is_none()
            {
                self.world_seeds(shared, &[seed]);
            }
        }
        self.save();
    }

    fn switch_world(&mut self, shared: &SharedMutex, hash: i64) {
        self.save();
        let server = crate::modules::seed_xray::server_hash();
        let mut search = self.search.take();
        if let Some(search) = search.as_mut() {
            search.pause(true);
        }
        if let Some(old) = self.target
            && (!self.found.is_empty() || self.seed.is_some())
        {
            self.parked.retain(|p| p.hash != old);
            self.parked.push(Parked {
                hash: old,
                search,
                found: std::mem::take(&mut self.found),
                seed: self.seed.take(),
                trusted: self.trusted,
            });
        }
        self.trusted = None;
        if let Some(i) = self.parked.iter().position(|p| p.hash == hash) {
            let back = self.parked.remove(i);
            self.search = back.search;
            self.found = back.found;
            self.seed = back.seed;
            self.trusted = back.trusted;
            if let Some(search) = self.search.as_mut() {
                search.pause(false);
            }
            self.applied = None;
        } else {
            let (mode, bedrock) = settings();
            self.search = self.version.and_then(|v| Search::start(v, mode, bedrock));
            self.applied = Some((mode, bedrock));
            if self.search.is_none() {
                tell(shared, RED, crate::lang::tr!("seed_cracker.no_thread"));
            }
        }
        while self.parked.len() > PARKED_MAX {
            self.parked.remove(0);
        }
        if hash != server && hash != 0 {
            self.send(Cmd::Add {
                observation: Observation::HashedSeed(hash),
                sighting: None,
            });
        }
        self.scanned.clear();
        self.backlog = true;
        self.hash = 0;
        self.foreign = hash != server;
        self.redraw = true;
        self.restore(shared, hash);
    }

    fn send(&mut self, cmd: Cmd) {
        if let Some(search) = self.search.as_mut() {
            search.send(cmd);
        }
    }

    fn apply_settings(&mut self) {
        let wanted = settings();
        if self.applied == Some(wanted) {
            return;
        }
        let (mode, bedrock) = wanted;
        self.send(Cmd::SetMode(mode));
        self.send(Cmd::SetBedrock(bedrock));
        if let Some((_, was)) = self.applied
            && was != bedrock
        {
            self.forget_bedrock();
            if bedrock {
                self.scanned.clear();
                self.backlog = true;
            }
        }
        self.applied = Some(wanted);
    }

    fn answered(&mut self, shared: &SharedMutex, trust: Option<Trust>) {
        let prefix = store().text(prefix_setting::PREFIX);
        match trust {
            Some(Trust::Bedrock) => {
                self.found
                    .retain(|o, _| !matches!(o, Observation::Structure(_)));
                self.redraw = true;
                self.changed();
                say(
                    shared,
                    YELLOW,
                    crate::lang::trf!("seed_cracker.trusted.bedrock", prefix),
                );
            }
            Some(Trust::Structures) => {
                self.forget_bedrock();
                say(
                    shared,
                    YELLOW,
                    crate::lang::trf!("seed_cracker.trusted.structures", prefix),
                );
            }
            None => say(shared, GRAY, crate::lang::tr!("seed_cracker.trusted.none")),
        }
        if trust.is_some() {
            self.trusted = trust;
        }
    }

    fn take_in(&mut self, finds: Vec<(Observation, Drawn)>) -> usize {
        let Some(search) = self.search.as_mut() else {
            return 0;
        };
        let mut new = 0;
        for (observation, drawn) in finds {
            if self.found.contains_key(&observation) {
                continue;
            }
            search.send(Cmd::Add {
                observation,
                sighting: None,
            });
            self.found.insert(observation, drawn);
            new += 1;
        }
        if new > 0 {
            self.redraw = true;
            self.changed();
        }
        new
    }

    fn changed(&mut self) {
        self.unsaved.get_or_insert(0);
    }

    fn save(&mut self) {
        let Some(hash) = self.target else { return };
        self.unsaved = None;
        let finds = self.found.iter().map(|(o, d)| (*o, d.clone())).collect();
        cache::save(&Cache {
            hash,
            seed: self.seed,
            finds,
        });
    }

    fn import(&mut self, shared: &SharedMutex) {
        let field = store().text(setting::FRIEND);
        let share = match Cache::parse(&field) {
            Ok(share) => share,
            Err(field_err) => match Cache::parse(&crate::platform::clipboard::get()) {
                Ok(share) => share,
                Err(_) if field.trim().is_empty() => {
                    tell(shared, RED, crate::lang::tr!("seed_cracker.import.empty"));
                    return;
                }
                Err(_) => {
                    tell(
                        shared,
                        RED,
                        crate::lang::trf!("seed_cracker.import.not_share", field_err),
                    );
                    return;
                }
            },
        };
        store().set_text_at(setting::FRIEND.0 as usize, "");
        let total = share.finds.len();

        if self.target == Some(share.hash) {
            let n = self.take_in(share.finds);
            let text = crate::lang::trn!("seed_cracker.import.merged", total, n);
            say(shared, AQUA, text);
        } else {
            self.switch_world(shared, share.hash);
            let n = self.take_in(share.finds);
            let prefix = store().text(prefix_setting::PREFIX);
            let text = if self.foreign {
                crate::lang::trn!("seed_cracker.import.foreign", total, share.hash, n, prefix)
            } else {
                crate::lang::trn!("seed_cracker.import.back", total, n)
            };
            say(shared, YELLOW, text);
        }
        if let Some(seed) = share.seed
            && self.seed.is_none()
        {
            self.world_seeds(shared, &[seed]);
        }
    }

    fn tick(&mut self, bot: &azalea::Client, shared: &SharedMutex) {
        let request = take_request();
        if self.search.is_none() {
            if IMPORT.swap(false, Relaxed) || request.is_some() {
                tell(shared, RED, crate::lang::tr!("seed_cracker.unsupported"));
            }
            return;
        }

        let hash = crate::modules::seed_xray::server_hash();
        let rejoined = std::mem::take(&mut self.rejoined);
        if hash == 0 && !self.warned_hidden {
            self.warned_hidden = true;
            warn_hidden(shared);
        }
        if self.target.is_none() {
            self.restore(shared, hash);
        } else if rejoined && (self.foreign || (hash != 0 && self.target != Some(hash))) {
            self.switch_world(shared, hash);
        }
        if !self.foreign && hash != 0 && hash != self.hash {
            self.hash = hash;
            self.send(Cmd::Add {
                observation: Observation::HashedSeed(hash),
                sighting: None,
            });
        }
        self.apply_settings();
        match request {
            Some(Request::Status) => self.send(Cmd::Status),
            Some(Request::Clear) => {
                self.send(Cmd::Clear);
                self.hash = 0;
                self.found.clear();
                self.seed = None;
                self.trusted = None;
                self.forget_bedrock();
                self.save();
                if self.foreign {
                    self.switch_world(shared, hash);
                }
                self.scanned.clear();
                self.backlog = true;
                self.redraw = true;
                tell(shared, YELLOW, crate::lang::tr!("seed_cracker.cleared"));
            }
            Some(Request::Restart) => {
                self.send(Cmd::Restart);
                tell(shared, YELLOW, crate::lang::tr!("seed_cracker.restarted"));
            }
            Some(Request::TrustBedrock) => self.send(Cmd::Resolve(Trust::Bedrock)),
            Some(Request::TrustStructures) => self.send(Cmd::Resolve(Trust::Structures)),
            Some(Request::ExportBedrock) => self.export_bedrock(shared),
            None => {}
        }
        if IMPORT.swap(false, Relaxed) {
            self.import(shared);
        }

        let sightings = if self.foreign {
            Vec::new()
        } else {
            self.scan(bot, shared)
        };
        let Some(search) = self.search.as_mut() else {
            return;
        };
        for cmd in sightings {
            search.send(cmd);
        }
        for out in search.poll() {
            self.report(shared, out);
        }
        self.draw();
        if let Some(waited) = self.unsaved.as_mut() {
            *waited += 1;
            if *waited >= SAVE_AFTER {
                self.save();
            }
        }
    }

    fn scan(&mut self, bot: &azalea::Client, shared: &SharedMutex) -> Vec<Cmd> {
        let dimension = dimension::current();
        if dimension != self.dimension {
            self.dimension = dimension;
            self.scanned.clear();
            self.backlog = true;
            self.redraw = true;
        }
        let target = to_seedcrack(dimension);
        let structures = self.trusted != Some(Trust::Bedrock);
        let kinds: Vec<FinderKind> = FinderKind::ALL
            .into_iter()
            .filter(|&k| k.dimension() == target && finder_on(k))
            .filter(|&k| structures || !matches!(k.yields(), Yields::Structure(_)))
            .collect();
        let bedrock = self.applied.is_some_and(|(_, on)| on)
            && self.trusted != Some(Trust::Structures)
            && Band::ALL.into_iter().any(|b| {
                let here = match b {
                    Band::NetherFloor | Band::NetherRoof => Dimension::Nether,
                    Band::OverworldFloor => Dimension::Overworld,
                };
                here == dimension && self.bedrock[band_slot(b)].len() < bedrock_cap(b)
            });
        if kinds.is_empty() && !bedrock {
            return Vec::new();
        }

        let (centre, radius) = {
            let s = shared.lock().unwrap();
            let radius = s.render_distance_sent.unwrap_or(DEFAULT_RADIUS) as i32 + 1;
            (s.session.chunk_pos(), radius.clamp(2, MAX_RADIUS))
        };
        let arrived = CHUNK_ARRIVED.swap(false, Relaxed);
        if !arrived && !self.backlog && self.centre == Some(centre) {
            return Vec::new();
        }
        self.centre = Some(centre);
        let Ok(world) = bot.world() else {
            return Vec::new();
        };

        let (jobs, bounds) = {
            let w = world.read();
            if self.biomes.is_empty() {
                self.biomes = biome_names(&w.registries);
            }
            let mut loaded: HashMap<(i32, i32), Handle> = HashMap::new();
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let at = (centre.0 + dx, centre.1 + dz);
                    if let Some(chunk) = w.chunks.get(&ChunkPos::new(at.0, at.1)) {
                        loaded.insert(at, chunk);
                    }
                }
            }
            self.scanned.retain(|at| loaded.contains_key(at));
            let wanted = nearest_first(
                centre,
                loaded.keys().copied().filter(|&at| {
                    !self.scanned.contains(&at)
                        && NEIGHBOURS
                            .iter()
                            .all(|&(dx, dz)| loaded.contains_key(&(at.0 + dx, at.1 + dz)))
                }),
            );
            self.backlog = wanted.len() > BUDGET;
            let jobs: Vec<((i32, i32), [Handle; 9])> = wanted
                .iter()
                .take(BUDGET)
                .map(|&at| {
                    let handles: [Handle; 9] = std::array::from_fn(|i| {
                        let (dx, dz) = (i as i32 % 3 - 1, i as i32 / 3 - 1);
                        loaded[&(at.0 + dx, at.1 + dz)].clone()
                    });
                    (at, handles)
                })
                .collect();
            (jobs, (w.chunks.min_y(), w.chunks.height() as i32))
        };

        crate::prof_span!("modules:seed_cracker");
        let rows: &[BedrockRow] = if bedrock {
            bedrock_rows(dimension, bounds)
        } else {
            &[]
        };
        let mut sightings = Vec::new();
        for (at, handles) in &jobs {
            let view = View {
                region: Region::new(*at, handles, bounds, 0, &[]),
                biomes: &self.biomes,
                bounds,
            };
            for &kind in &kinds {
                for found in scan_chunk(&view, target, at.0, at.1, |k| k == kind) {
                    let observation = found.observation;
                    if self.ignored.contains(&observation) {
                        continue;
                    }
                    let drawn = Drawn {
                        dimension,
                        boxes: found.boxes,
                        color: found.color,
                    };
                    if self.found.insert(observation, drawn).is_none() {
                        self.unsaved.get_or_insert(0);
                    }
                    self.redraw = true;
                    sightings.push(Cmd::Add {
                        observation,
                        sighting: Some((super::finder_label(kind), chunk_of(&observation, *at))),
                    });
                }
            }
            let (x0, z0) = (at.0 * 16, at.1 * 16);
            for &(band, y, id, on) in rows {
                let held = &mut self.bedrock[band_slot(band)];
                for z in z0..z0 + 16 {
                    for x in x0..x0 + 16 {
                        if held.len() >= bedrock_cap(band) {
                            break;
                        }
                        if view.region.state(x, y, z).map(id_of) != Some(id) {
                            continue;
                        }
                        if let Some(cell) = BedrockCell::new(band, x, y, z, on)
                            && held.insert(cell)
                        {
                            sightings.push(Cmd::Add {
                                observation: Observation::Bedrock(cell),
                                sighting: None,
                            });
                        }
                    }
                }
            }
            drop(view);
            self.scanned.insert(*at);
        }
        sightings
    }

    fn report(&mut self, shared: &SharedMutex, out: Out) {
        match out {
            Out::Added {
                label,
                chunk,
                structure_bits,
                decorator_bits,
            } => {
                let (structure_bits, decorator_bits) = (
                    format!("{structure_bits:.1}"),
                    (decorator_bits > 0.0).then(|| format!("{decorator_bits:.1}")),
                );
                let label = label.to_lowercase();
                let text = match decorator_bits {
                    Some(dec) => crate::lang::trf!(
                        "seed_cracker.added.both",
                        label,
                        chunk.0,
                        chunk.1,
                        structure_bits,
                        dec
                    ),
                    None => crate::lang::trf!(
                        "seed_cracker.added",
                        label,
                        chunk.0,
                        chunk.1,
                        structure_bits
                    ),
                };
                tell(shared, WHITE, text);
            }
            Out::Status(lines) => {
                for line in lines {
                    say(shared, WHITE, line);
                }
            }
            Out::Event(event) => self.event(shared, event),
            Out::Hint { hint, cells } => {
                if let Some(text) = hint_text(hint, cells) {
                    tell(shared, GRAY, crate::lang::trf!("seed_cracker.chat", text));
                }
            }
            Out::BandFull { band, cells } => {
                let text = bedrock_progress(band, cells);
                tell(shared, GRAY, crate::lang::trf!("seed_cracker.chat", text));
            }
            Out::Resolved(trust) => self.answered(shared, trust),
        }
    }

    fn event(&mut self, shared: &SharedMutex, event: Event) {
        let prefix = store().text(prefix_setting::PREFIX);
        match event {
            Event::PillarSeeds(seeds) => {
                let text =
                    crate::lang::trn!("seed_cracker.pillar_seeds", seeds.len(), listed(&seeds));
                tell(shared, AQUA, text)
            }
            Event::PillarsInconsistent => tell(
                shared,
                RED,
                crate::lang::tr!("seed_cracker.pillars_inconsistent"),
            ),
            Event::Started(stage) => tell(
                shared,
                GRAY,
                crate::lang::trf!("seed_cracker.started", stage_name(stage)),
            ),
            Event::LiftedLowParts(n) => {
                let text = crate::lang::trn!("seed_cracker.lifted", n);
                tell(shared, GRAY, text)
            }
            Event::StructureSeeds(seeds) => {
                let text =
                    crate::lang::trn!("seed_cracker.structure_seeds", seeds.len(), listed(&seeds));
                tell(shared, AQUA, text)
            }
            Event::NoStructureSeed => tell(
                shared,
                RED,
                crate::lang::trf!("seed_cracker.no_structure_seed", prefix),
            ),
            Event::WorldSeeds(seeds) => self.world_seeds(shared, &seeds),
            Event::HashMismatch => {
                tell(shared, RED, crate::lang::tr!("seed_cracker.hash_mismatch"))
            }
            Event::NoWorldSeed => tell(
                shared,
                RED,
                crate::lang::trf!("seed_cracker.no_world_seed", prefix),
            ),
            Event::BandSeeds(band, n) => {
                let text = crate::lang::trn!("seed_cracker.band_seeds", n, band_name(band));
                tell(shared, AQUA, text)
            }
            Event::BedrockOverflow(band) => tell(
                shared,
                RED,
                crate::lang::trf!("seed_cracker.bedrock_overflow", band_name(band), prefix),
            ),
            Event::BedrockInconsistent => tell(
                shared,
                RED,
                crate::lang::trf!("seed_cracker.bedrock_inconsistent", prefix),
            ),
            Event::BedrockConflict => conflict_prompt(shared, &prefix),
            Event::SightingIgnored(s) => {
                let observation = Observation::Structure(s);
                if self.ignored.insert(observation) {
                    if self.found.remove(&observation).is_some() {
                        self.unsaved.get_or_insert(0);
                        self.redraw = true;
                    }
                    say(
                        shared,
                        YELLOW,
                        crate::lang::trf!("seed_cracker.sighting_ignored", s.chunk_x, s.chunk_z),
                    );
                }
            }
        }
    }

    fn world_seeds(&mut self, shared: &SharedMutex, seeds: &[i64]) {
        let [seed] = seeds else {
            tell(
                shared,
                AQUA,
                crate::lang::trf!("seed_cracker.world_seeds", seeds.len(), listed(seeds)),
            );
            return;
        };
        if self.seed != Some(*seed) {
            self.seed = Some(*seed);
            self.changed();
        }
        let text = seed.to_string();
        let events: Vec<SpanEvent> = vec![SpanEvent {
            click: Some(Click::CopyToClipboard(text.clone())),
            hover: Some(Hover::Lines(vec![vec![Span::colored(
                crate::lang::tr!("seed_cracker.click_to_copy"),
                GRAY,
            )]])),
            insertion: Some(text.clone()),
        }];
        let mut style = Style::colored(GREEN);
        style.underline = true;
        style.event = 1;
        let spans = vec![
            Span::colored(crate::lang::tr!("seed_cracker.world_seed"), WHITE),
            Span {
                text: text.clone(),
                style,
            },
        ];
        {
            let mut s = shared.lock().unwrap();
            show_local_events(&mut s.session, spans, Some(Arc::from(events)));
        }
        crate::log_info!("seed_cracker", "world seed {text}");
        let s = store();
        if s.flag(setting::SEND) {
            s.set_text_at(xray_setting::SEED.0 as usize, &text);
            tell(shared, GRAY, crate::lang::tr!("seed_cracker.sent_to_xray"));
        }
    }

    fn draw(&mut self) {
        let show = store().flag(setting::SHOW_BOXES) && !self.foreign;
        if !self.redraw && show == self.shown {
            return;
        }
        self.redraw = false;
        self.shown = show;
        if !show {
            world_draw::clear(OWNER);
            return;
        }
        let nodes: Vec<Node> = self
            .found
            .values()
            .filter(|d| d.dimension == self.dimension)
            .flat_map(|d| {
                let argb = FILL << 24 | (d.color & 0xFF_FF_FF);
                d.boxes.iter().map(move |c| {
                    Node::boxed(c.min.map(f64::from), c.max.map(f64::from), argb, true)
                })
            })
            .collect();
        world_draw::set(OWNER, nodes);
    }
}

fn warn_hidden(shared: &SharedMutex) {
    use crate::gui::toast::{SystemId, ToastEvent};
    say(shared, YELLOW, crate::lang::tr!("seed_cracker.hidden_hash"));
    crate::session::queue_toasts(
        shared,
        [ToastEvent::System {
            id: SystemId::HiddenHashedSeed,
            title: vec![Span::colored(
                crate::lang::tr!("seed_cracker.hidden_hash.toast_title"),
                YELLOW,
            )],
            message: vec![Span::colored(
                crate::lang::tr!("seed_cracker.hidden_hash.toast_message"),
                WHITE,
            )],
            multiline: true,
        }],
    );
}

fn conflict_prompt(shared: &SharedMutex, prefix: &str) {
    say(shared, RED, crate::lang::tr!("seed_cracker.conflict"));
    let choices = [
        (
            crate::lang::tr!("seed_cracker.conflict.bedrock"),
            "cracker trust bedrock",
        ),
        (
            crate::lang::tr!("seed_cracker.conflict.structures"),
            "cracker trust structures",
        ),
        (
            crate::lang::tr!("seed_cracker.conflict.clear"),
            "cracker clear",
        ),
    ];
    let mut spans = Vec::new();
    let mut events: Vec<SpanEvent> = Vec::new();
    for (i, (label, command)) in choices.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::colored(" ", WHITE));
        }
        let line = format!("{prefix}{command}");
        let hover = crate::lang::trf!("seed_cracker.conflict.hover", line);
        let mut style = Style::colored(YELLOW);
        style.underline = true;
        style.event = crate::text::events::push(
            &mut events,
            SpanEvent {
                click: Some(Click::SuggestCommand(line)),
                hover: Some(Hover::Lines(vec![vec![Span::colored(hover, GRAY)]])),
                insertion: None,
            },
        );
        spans.push(Span {
            text: label.to_string(),
            style,
        });
    }
    let mut s = shared.lock().unwrap();
    show_local_events(&mut s.session, spans, Some(Arc::from(events)));
}

impl Drop for State {
    fn drop(&mut self) {
        if self.unsaved.is_some() {
            self.save();
        }
    }
}

pub fn tick(bot: &azalea::Client, shared: &SharedMutex) {
    if super::HELP.swap(false, Relaxed) {
        super::help::show(shared);
    }
    match EDGE.poll(store()) {
        Phase::Off => {
            if IMPORT.swap(false, Relaxed) {
                tell(shared, RED, crate::lang::tr!("seed_cracker.import.off"));
            }
            return;
        }
        Phase::Stopped => {
            with_state(|st| *st = None);
            hud::reset();
            world_draw::clear(OWNER);
            return;
        }
        Phase::Started => with_state(|st| *st = None),
        Phase::Running => {}
    }
    let session = SESSION.load(Relaxed);
    let version = Version::from_protocol(crate::util::session_protocol());
    with_state(|slot| {
        if slot.as_ref().is_none_or(|st| {
            st.version != version || (st.search.is_none() && st.session != session)
        }) {
            world_draw::clear(OWNER);
            hud::reset();
            *slot = Some(State::new(shared, session, version));
        }
        let Some(st) = slot.as_mut() else { return };
        if st.session != session {
            st.rejoin(session);
        }
        st.tick(bot, shared);
    });
}
