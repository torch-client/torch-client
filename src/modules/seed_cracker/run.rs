use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use azalea_core::position::ChunkPos;
use seedcrack::{Event, FinderKind, Observation, Version, scan_chunk};

use super::cache::{self, Cache, Drawn};
use super::hud;
use super::search::{Cmd, Out, Search, stage_name};
use super::{CHUNK_ARRIVED, EDGE, FINDERS, IMPORT, OWNER, Request, SESSION, take_request};
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

const SAVE_AFTER: u32 = 20;

struct State {
    session: u32,
    search: Option<Search>,
    hash: i64,
    dimension: Dimension,
    scanned: HashSet<(i32, i32)>,
    centre: Option<(i32, i32)>,
    backlog: bool,
    biomes: Vec<String>,
    found: HashMap<Observation, Drawn>,
    redraw: bool,
    shown: bool,
    target: Option<i64>,
    foreign: bool,
    seed: Option<i64>,
    unsaved: Option<u32>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
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

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
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
    fn new(shared: &SharedMutex, session: u32) -> State {
        let protocol = crate::util::session_protocol();
        let search = match Version::from_protocol(protocol) {
            Some(version) => {
                let search = Search::start(version, session);
                if search.is_none() {
                    tell(
                        shared,
                        RED,
                        "Seed Cracker: could not start the search thread",
                    );
                    hud::off();
                }
                search
            }
            None => {
                tell(
                    shared,
                    RED,
                    format!("Seed Cracker: off, it has no tables for protocol {protocol}"),
                );
                hud::off();
                None
            }
        };
        State {
            session,
            search,
            hash: 0,
            dimension: dimension::current(),
            scanned: HashSet::new(),
            centre: None,
            backlog: true,
            biomes: Vec::new(),
            found: HashMap::new(),
            redraw: true,
            shown: false,
            target: None,
            foreign: false,
            seed: None,
            unsaved: None,
        }
    }

    fn restore(&mut self, shared: &SharedMutex, hash: i64) {
        self.target = Some(hash);
        match cache::load() {
            Some(c) if c.hash == hash => {
                let n = self.take_in(c.finds);
                if n > 0 {
                    say(
                        shared,
                        GRAY,
                        format!(
                            "Seed Cracker: {n} find{} restored from the cache",
                            plural(n)
                        ),
                    );
                }
                if let Some(seed) = c.seed {
                    self.world_seeds(shared, &[seed]);
                }
            }
            Some(_) => say(
                shared,
                GRAY,
                "Seed Cracker: another hashed seed, so the cache starts again",
            ),
            None => {}
        }
        self.save();
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
                    tell(
                        shared,
                        RED,
                        "Seed Cracker: paste a friend's share into Friend's share first",
                    );
                    return;
                }
                Err(_) => {
                    tell(
                        shared,
                        RED,
                        format!("Seed Cracker: that is not a share: {field_err}"),
                    );
                    return;
                }
            },
        };
        store().set_text_at(setting::FRIEND.0 as usize, "");
        let total = share.finds.len();

        if self.target == Some(share.hash) {
            let n = self.take_in(share.finds);
            say(
                shared,
                AQUA,
                format!(
                    "Seed Cracker: {n} of the friend's {total} find{} were new",
                    plural(total)
                ),
            );
        } else {
            let server = crate::modules::seed_xray::server_hash();
            if let Some(search) = self.search.as_mut() {
                search.send(Cmd::Clear);
                if share.hash != server && share.hash != 0 {
                    search.send(Cmd::Add {
                        observation: Observation::HashedSeed(share.hash),
                        sighting: None,
                    });
                }
            }
            self.found.clear();
            self.seed = None;
            self.scanned.clear();
            self.backlog = true;
            self.hash = 0;
            self.target = Some(share.hash);
            self.foreign = share.hash != server;
            self.redraw = true;
            self.take_in(share.finds);
            self.changed();
            let prefix = store().text(prefix_setting::PREFIX);
            if self.foreign {
                say(
                    shared,
                    YELLOW,
                    format!(
                        "Seed Cracker: replaced with the friend's {total} find{} for another world (hashed seed {}); \
                         this world is not scanned until {prefix}cracker clear or the next join",
                        plural(total),
                        share.hash
                    ),
                );
            } else {
                say(
                    shared,
                    YELLOW,
                    format!(
                        "Seed Cracker: replaced with the friend's {total} find{}",
                        plural(total)
                    ),
                );
            }
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
                tell(
                    shared,
                    RED,
                    "Seed Cracker: off, this server's version is not supported",
                );
            }
            return;
        }

        let hash = crate::modules::seed_xray::server_hash();
        if self.target.is_none() {
            self.restore(shared, hash);
        }
        let Some(search) = self.search.as_mut() else {
            return;
        };
        if !self.foreign && hash != 0 && hash != self.hash {
            self.hash = hash;
            search.send(Cmd::Add {
                observation: Observation::HashedSeed(hash),
                sighting: None,
            });
        }
        match request {
            Some(Request::Status) => search.send(Cmd::Status),
            Some(Request::Clear) => {
                search.send(Cmd::Clear);
                self.hash = 0;
                self.found.clear();
                self.seed = None;
                self.foreign = false;
                self.target = Some(hash);
                self.changed();
                self.scanned.clear();
                self.redraw = true;
                tell(
                    shared,
                    YELLOW,
                    "Seed Cracker: cleared; loaded chunks are scanned again",
                );
            }
            Some(Request::Restart) => {
                search.send(Cmd::Restart);
                tell(shared, YELLOW, "Seed Cracker: searching again");
            }
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
        let kinds: Vec<FinderKind> = FinderKind::ALL
            .into_iter()
            .filter(|&k| k.dimension() == target && finder_on(k))
            .collect();
        if kinds.is_empty() {
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
                        sighting: Some((kind.name(), chunk_of(&observation, *at))),
                    });
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
                let mut bits = format!("{structure_bits:.1} structure bits");
                if decorator_bits > 0.0 {
                    bits += &format!(", {decorator_bits:.1} decoration bits");
                }
                tell(
                    shared,
                    WHITE,
                    format!(
                        "Seed Cracker: {} at chunk {}, {} ({bits})",
                        label.to_lowercase(),
                        chunk.0,
                        chunk.1
                    ),
                );
            }
            Out::Status(lines) => {
                for line in lines {
                    say(shared, WHITE, line);
                }
            }
            Out::Event(event) => self.event(shared, event),
        }
    }

    fn event(&mut self, shared: &SharedMutex, event: Event) {
        let prefix = store().text(prefix_setting::PREFIX);
        match event {
            Event::PillarSeeds(seeds) => tell(
                shared,
                AQUA,
                format!(
                    "Seed Cracker: {} pillar seed{}{}",
                    seeds.len(),
                    plural(seeds.len()),
                    listed(&seeds)
                ),
            ),
            Event::PillarsInconsistent => tell(
                shared,
                RED,
                "Seed Cracker: the End's spikes fit no pillar seed, so they are ignored; the overworld can still crack it",
            ),
            Event::Started(stage) => {
                tell(shared, GRAY, format!("Seed Cracker: {}", stage_name(stage)))
            }
            Event::LiftedLowParts(n) => tell(
                shared,
                GRAY,
                format!(
                    "Seed Cracker: {n} low part{} left after the lift filter",
                    plural(n)
                ),
            ),
            Event::StructureSeeds(seeds) => tell(
                shared,
                AQUA,
                format!(
                    "Seed Cracker: {} structure seed{}{}",
                    seeds.len(),
                    plural(seeds.len()),
                    listed(&seeds)
                ),
            ),
            Event::NoStructureSeed => tell(
                shared,
                RED,
                format!("Seed Cracker: no result: a sighting is wrong, run {prefix}cracker clear"),
            ),
            Event::WorldSeeds(seeds) => self.world_seeds(shared, &seeds),
            Event::HashMismatch => tell(
                shared,
                RED,
                "Seed Cracker: no world seed matches the server's hashed seed; dungeons, desert wells and end \
                 gateways can pick it instead",
            ),
            Event::NoWorldSeed => tell(
                shared,
                RED,
                format!(
                    "Seed Cracker: no world seed fits the decorations: a sighting is wrong, run {prefix}cracker clear"
                ),
            ),
        }
    }

    fn world_seeds(&mut self, shared: &SharedMutex, seeds: &[i64]) {
        let [seed] = seeds else {
            tell(
                shared,
                AQUA,
                format!(
                    "Seed Cracker: {} world seed{}{}",
                    seeds.len(),
                    plural(seeds.len()),
                    listed(seeds)
                ),
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
                "Click to copy",
                GRAY,
            )]])),
            insertion: Some(text.clone()),
        }];
        let mut style = Style::colored(GREEN);
        style.underline = true;
        style.event = 1;
        let spans = vec![
            Span::colored("Seed Cracker: world seed ", WHITE),
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
            tell(
                shared,
                GRAY,
                "Seed Cracker: sent to Seed Xray's Seed setting",
            );
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
                tell(shared, RED, "Seed Cracker: turn it on to import a share");
            }
            return;
        }
        Phase::Stopped => {
            STATE.with_borrow_mut(|st| *st = None);
            hud::reset();
            world_draw::clear(OWNER);
            return;
        }
        Phase::Started => STATE.with_borrow_mut(|st| *st = None),
        Phase::Running => {}
    }
    let session = SESSION.load(Relaxed);
    STATE.with_borrow_mut(|slot| {
        if slot.as_ref().is_none_or(|st| st.session != session) {
            world_draw::clear(OWNER);
            hud::reset();
            *slot = Some(State::new(shared, session));
        }
        if let Some(st) = slot {
            st.tick(bot, shared);
        }
    });
}
