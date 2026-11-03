use std::cell::Cell;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, PoisonError};

use azalea::Client;
use mlua::{Function, Lua, Table, Value};

use super::super::registry::{Kind, ModuleIx};
use super::super::{Value as Setting, store};
use super::api::{
    self, Api, CHAT, Class, Ctx, DISABLE, ENABLE, EVENTS, NONE, PACKET, RENDER2D, Switch, TICK,
    cancelable,
};
use super::game::{self, Pending};
use super::{CHAT as WANTS_CHAT, RESCAN, RUNNING, SETTINGS, WANTS, kinds, state};

#[derive(Clone, Copy, Default, PartialEq)]
pub(super) struct Body {
    pub velocity: [f64; 3],
    pub fall_distance: f64,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub bbox: [f64; 6],
    pub crouching: bool,
    pub flying: bool,
}

#[derive(Clone, Copy, Default)]
pub(super) struct World {
    pub epoch: u64,
    pub pos: [f64; 3],
    pub health: f32,
    pub food: u32,
    pub screen_open: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub camera: [f32; 2],
    pub on_ground: bool,
    pub physics: Body,
    pub gamemode: &'static str,
    pub xp: u32,
    pub dead: bool,
    pub op_level: u8,
    pub selected: u8,
}

#[derive(Default)]
struct Shown {
    world: World,
    dimension: String,
    address: Option<String>,
    screen: (f32, f32),
}

const STRIKES: u8 = 3;

#[derive(Clone, Copy)]
enum Label<'a> {
    Event(usize),
    Command(&'a str),
}

impl std::fmt::Display for Label<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Label::Event(ev) => write!(f, "on{}", EVENTS[*ev]),
            Label::Command(name) => write!(f, "command {name}"),
        }
    }
}

pub(super) struct Live {
    ix: ModuleIx,
    mod_id: &'static str,
    api: usize,
    overruns: Cell<u8>,
    m: Table,
    backing: Table,
    values: Table,
    handlers: [Option<Function>; EVENTS.len()],
    commands: Vec<(&'static str, Function)>,
    packets: super::packets::PacketSet,
    running: bool,
    seen: u32,
    ticks: u64,
    settings: Vec<Synced>,
}

struct Synced {
    slot: usize,
    key: &'static str,
    kind: Kind,
    value: Option<Setting>,
    text: String,
    generation: u32,
}

impl Synced {
    fn reset(&self) {
        let s = store();
        match self.kind {
            Kind::Text { default, .. } => s.set_text_at(self.slot, default),
            Kind::List { list } => list.apply_preset(crate::modules::list::Preset::Defaults),
            kind => {
                if let Some(v) = Setting::default_of(kind) {
                    s.set_value(self.slot, v);
                }
            }
        }
    }
}

impl Live {
    pub fn new(ix: ModuleIx, first: usize, d: api::Def) -> Live {
        let settings = (d.settings.iter().enumerate())
            .map(|(i, (key, def))| Synced {
                slot: first + i,
                key: *key,
                kind: def.kind,
                value: Setting::default_of(def.kind),
                text: match def.kind {
                    Kind::Text { default, .. } => default.to_string(),
                    _ => String::new(),
                },
                generation: match def.kind {
                    Kind::List { list } => list.generation(),
                    _ => 0,
                },
            })
            .collect();
        Live {
            ix,
            mod_id: d.mod_id,
            api: d.api,
            overruns: Cell::new(0),
            m: d.m,
            backing: d.backing,
            values: d.values,
            handlers: d.handlers,
            commands: d
                .commands
                .into_iter()
                .map(|(decl, run)| (decl.name, run))
                .collect(),
            packets: d.packets,
            running: false,
            seen: 0,
            ticks: 0,
            settings,
        }
    }
}

pub(super) struct Runtime {
    ctx: Arc<Ctx>,
    apis: Vec<Api>,
    modules: Vec<Live>,
    pub mods: Vec<state::Mod>,
    lists: [Vec<usize>; EVENTS.len()],
    fixed: Vec<Vec<Option<Table>>>,
    shown: Option<Shown>,
    stopping: bool,
    epoch: u64,
    parked: Option<u64>,
    queued: Vec<Pending>,
}

fn is_fixed(ev: usize) -> bool {
    matches!(
        EVENTS[ev],
        "enable" | "disable" | "tick" | "join" | "spawn" | "render2d" | "leave"
    )
}

fn check(r: mlua::Result<()>) {
    if let Err(e) = r {
        crate::log_error!("mods", "could not update a mod's view of the game: {e}");
    }
}

impl Runtime {
    pub fn new(ctx: Arc<Ctx>, apis: Vec<Api>, modules: Vec<Live>) -> mlua::Result<Runtime> {
        let fixed = apis
            .iter()
            .map(|api| {
                (0..EVENTS.len())
                    .map(|ev| {
                        if !is_fixed(ev) {
                            return Ok(None);
                        }
                        let e = api.event_table()?;
                        api.seal(&e, ev, Class::Plain)?;
                        Ok(Some(e))
                    })
                    .collect::<mlua::Result<Vec<_>>>()
            })
            .collect::<mlua::Result<_>>()?;
        ctx.game.lock().unwrap_or_else(PoisonError::into_inner).hud =
            std::iter::repeat_with(Default::default)
                .take(modules.len())
                .collect();
        Ok(Runtime {
            ctx,
            apis,
            modules,
            mods: Vec::new(),
            lists: Default::default(),
            fixed,
            shown: None,
            stopping: false,
            epoch: 0,
            parked: None,
            queued: Vec::new(),
        })
    }

    fn refresh(&mut self) {
        if WANTS.fetch_and(!SETTINGS, Relaxed) & SETTINGS != 0 {
            self.sync_settings();
        }
    }

    fn with_bot<R>(&mut self, bot: &Client, f: impl FnOnce(&mut Self) -> R) -> R {
        let world = bot.world().ok();
        {
            let mut g = self.ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
            g.bot = Some(bot.clone());
            g.world = world;
        }
        let r = f(self);
        let mut g = self.ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
        g.bot = None;
        g.world = None;
        drop(g);
        r
    }

    pub fn tick(
        &mut self,
        w: &World,
        snap: game::Snapshot,
        commands: Vec<(String, String)>,
        bot: &Client,
    ) {
        if let Some(left) = self.parked {
            if w.epoch == left {
                return;
            }
            self.parked = None;
        }
        self.epoch = w.epoch;
        let bits = WANTS.fetch_and(!(RESCAN | SETTINGS), Relaxed);
        if bits & SETTINGS != 0 {
            self.sync_settings();
        }
        check(self.publish(w, &snap));
        let old = std::mem::replace(
            &mut self
                .ctx
                .game
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .snap,
            snap,
        );
        old.recycle();
        self.ctx
            .game
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .placed = false;
        self.with_bot(bot, |rt| {
            if bits & RESCAN != 0 {
                rt.rescan();
            }
            let mut queued = std::mem::take(&mut rt.queued);
            game::take(w.epoch, &mut queued);
            for p in queued.drain(..) {
                let ev = p.event();
                let (id, class) = match &p {
                    Pending::Packet(pk) => (pk.id, pk.class()),
                    _ => (0, Class::Plain),
                };
                let owner = p.owner();
                rt.dispatch_as(
                    ev,
                    class,
                    |lua, e| p.fill(lua, e),
                    |k, live| {
                        owner.is_none_or(|o| o == k) && (ev != PACKET || live.packets.has(id))
                    },
                );
            }
            rt.queued = queued;
            let (arrived, changed) = super::chunk::take();
            for (ev, columns) in [(api::CHUNK, arrived), (api::BLOCKCHANGE, changed)] {
                for [cx, cz] in columns {
                    rt.dispatch(
                        ev,
                        |_, e| {
                            e.raw_set("cx", cx)?;
                            e.raw_set("cz", cz)
                        },
                        |_, _| true,
                    );
                }
            }
            for (name, raw) in &commands {
                rt.command(name, raw);
            }
            rt.dispatch(TICK, |_, _| Ok(()), |_, _| true);
            rt.render2d();
        });
        for live in self.modules.iter_mut().filter(|l| l.running) {
            live.ticks += 1;
            check(live.backing.raw_set("ticks", live.ticks as f64));
        }
        let mut g = self.ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
        game::MOVES.store(std::mem::take(&mut g.moves), Relaxed);
        game::RELEASES.store(std::mem::take(&mut g.releases), Relaxed);
        g.publish_hud();
        drop(g);
        super::render::publish();
        super::ghost::tick(bot);
    }

    fn render2d(&mut self) {
        let s = store();
        for i in 0..self.lists[RENDER2D].len() {
            let k = self.lists[RENDER2D][i];
            if !s.enabled(self.modules[k].ix) {
                continue;
            }
            {
                let mut g = self.ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
                g.begin_hud(k);
                g.drawing = Some(k);
                g.drawn = (0, 0);
            }
            if let Some(e) = &self.fixed[self.modules[k].api][RENDER2D] {
                self.call(k, RENDER2D, e);
            }
            let mut g = self.ctx.game.lock().unwrap_or_else(PoisonError::into_inner);
            g.drawing = None;
            g.end_hud(k);
        }
    }

    pub fn chat(&mut self, text: &str, bot: &Client) -> bool {
        self.refresh();
        self.with_bot(bot, |rt| {
            rt.dispatch(CHAT, |_, e| e.raw_set("text", text), |_, _| true)
        })
    }

    pub fn chatsend(&mut self, lines: Vec<String>, bot: &Client) -> Vec<String> {
        self.refresh();
        self.with_bot(bot, |rt| {
            lines
                .into_iter()
                .filter(|line| {
                    !rt.dispatch(
                        api::CHATSEND,
                        |_, e| {
                            e.raw_set("text", line.as_str())?;
                            e.raw_set("command", line.starts_with('/'))
                        },
                        |_, _| true,
                    )
                })
                .collect()
        })
    }

    pub fn attack(&mut self, id: i32, kind: &str, bot: &Client) -> bool {
        self.refresh();
        self.with_bot(bot, |rt| {
            rt.dispatch(
                api::ATTACK,
                |_, e| {
                    e.raw_set("id", id)?;
                    e.raw_set("type", kind)
                },
                |_, _| true,
            )
        })
    }

    fn command(&mut self, name: &str, raw: &str) {
        use crate::modules::commands::{ArgValue, bind, resolve_script};
        let Some(sc) = resolve_script(name) else {
            api::print(&format!(
                "Unknown command `{name}`; the mods were reloaded since it was typed"
            ));
            return;
        };
        let Some(k) = self.modules.iter().position(|l| l.ix == sc.module) else {
            return;
        };
        let live = &self.modules[k];
        if !live.running || !store().enabled(live.ix) {
            api::print(&format!(
                "{} is off, so `{name}` did nothing",
                self.ctx.tag(k)
            ));
            return;
        }
        let Some(f) = live
            .commands
            .iter()
            .find(|(n, _)| *n == sc.decl.name)
            .map(|(_, f)| f.clone())
        else {
            return;
        };
        let values = match bind(&sc.decl.args, raw) {
            Ok(v) => v,
            Err(e) => {
                let usage = crate::modules::commands::usage(&sc.decl.args);
                api::print(&format!("{e}. Usage: {name} {usage}"));
                return;
            }
        };
        let lua = &self.apis[live.api].lua;
        let e = (|| -> mlua::Result<Table> {
            let e = lua.create_table()?;
            e.raw_set("name", sc.decl.name)?;
            e.raw_set("raw", raw)?;
            let args = lua.create_table()?;
            for (key, v) in values {
                match v {
                    ArgValue::Str(s) => args.raw_set(key, s)?,
                    ArgValue::Int(i) => args.raw_set(key, i)?,
                    ArgValue::Num(x) => args.raw_set(key, x)?,
                    ArgValue::Bool(b) => args.raw_set(key, b)?,
                }
            }
            e.raw_set("args", args)?;
            Ok(e)
        })();
        match e {
            Ok(e) => self.call_fn(k, &f, Label::Command(sc.decl.name), &e),
            Err(err) => crate::log_error!(
                "mods",
                "could not build the event for command {name}: {err}"
            ),
        }
    }

    pub fn use_item(&mut self, bot: &Client) -> bool {
        self.refresh();
        self.with_bot(bot, |rt| rt.dispatch(api::USE, |_, _| Ok(()), |_, _| true))
    }

    fn dispatch(
        &self,
        ev: usize,
        fill: impl Fn(&Lua, &Table) -> mlua::Result<()>,
        wants: impl Fn(usize, &Live) -> bool,
    ) -> bool {
        self.dispatch_as(ev, Class::Plain, fill, wants)
    }

    fn dispatch_as(
        &self,
        ev: usize,
        class: Class,
        fill: impl Fn(&Lua, &Table) -> mlua::Result<()>,
        wants: impl Fn(usize, &Live) -> bool,
    ) -> bool {
        let s = store();
        let cancel = cancelable(ev);
        let mut prevented = false;
        let mut current: Option<(usize, Table, Option<Table>)> = None;
        for &k in &self.lists[ev] {
            let live = &self.modules[k];
            if !s.enabled(live.ix) || !wants(k, live) {
                continue;
            }
            let a = live.api;
            if let Some(e) = &self.fixed[a][ev] {
                self.call(k, ev, e);
                continue;
            }
            if current.as_ref().is_none_or(|(at, ..)| *at != a) {
                if let Some((_, _, Some(backing))) = &current {
                    prevented |= backing.raw_get::<bool>("defaultPrevented").unwrap_or(false);
                }
                let api = &self.apis[a];
                let built = if cancel {
                    api.cancelable_event(EVENTS[ev], prevented)
                        .map(|(e, backing)| {
                            check(fill(&api.lua, &backing));
                            (e, Some(backing))
                        })
                } else {
                    api.event_table().and_then(|e| {
                        check(fill(&api.lua, &e));
                        api.seal(&e, ev, class).map(|()| (e, None))
                    })
                };
                current = match built {
                    Ok((e, backing)) => Some((a, e, backing)),
                    Err(err) => {
                        crate::log_error!(
                            "mods",
                            "could not build a `{}` event: {err}",
                            EVENTS[ev]
                        );
                        None
                    }
                };
            }
            if let Some((_, e, _)) = &current {
                self.call(k, ev, e);
            }
        }
        if let Some((_, _, Some(backing))) = &current {
            prevented |= backing.raw_get::<bool>("defaultPrevented").unwrap_or(false);
        }
        prevented
    }

    fn publish(&mut self, w: &World, snap: &game::Snapshot) -> mlua::Result<()> {
        let screen = game::screen();
        let mut shown = self.shown.take();
        let o = shown.as_ref();
        let all = o.is_none();
        let last = o.map_or(*w, |o| o.world);
        let (b, lb) = (&w.physics, &last.physics);
        let dimension = all || o.is_some_and(|o| o.dimension != snap.dimension);
        let address = all || o.is_some_and(|o| o.address != snap.address);
        let screen_moved = all || o.is_some_and(|o| o.screen != screen);
        for api in &self.apis {
            let p = &api.player;
            for (i, k) in ["x", "y", "z"].into_iter().enumerate() {
                if all || w.pos[i] != last.pos[i] {
                    p.raw_set(k, w.pos[i])?;
                }
            }
            if all || w.health != last.health {
                p.raw_set("health", w.health as f64)?;
            }
            if all || w.food != last.food {
                p.raw_set("food", w.food)?;
            }
            if all || w.yaw != last.yaw {
                p.raw_set("yaw", w.yaw as f64)?;
            }
            if all || w.pitch != last.pitch {
                p.raw_set("pitch", w.pitch as f64)?;
            }
            if all || w.camera != last.camera {
                p.raw_set("camerayaw", w.camera[0] as f64)?;
                p.raw_set("camerapitch", w.camera[1] as f64)?;
            }
            if all || w.on_ground != last.on_ground {
                p.raw_set("onground", w.on_ground)?;
            }
            for (i, k) in ["vx", "vy", "vz"].into_iter().enumerate() {
                if all || b.velocity[i] != lb.velocity[i] {
                    p.raw_set(k, b.velocity[i])?;
                }
            }
            if all || b.fall_distance != lb.fall_distance {
                p.raw_set("falldistance", b.fall_distance)?;
            }
            if all || b.horizontal_collision != lb.horizontal_collision {
                p.raw_set("hcollision", b.horizontal_collision)?;
            }
            if all || b.vertical_collision != lb.vertical_collision {
                p.raw_set("vcollision", b.vertical_collision)?;
            }
            if all || b.crouching != lb.crouching {
                p.raw_set("crouching", b.crouching)?;
            }
            if all || b.flying != lb.flying {
                p.raw_set("flying", b.flying)?;
            }
            for (i, k) in ["minx", "miny", "minz", "maxx", "maxy", "maxz"]
                .into_iter()
                .enumerate()
            {
                if all || b.bbox[i] != lb.bbox[i] {
                    api.bbox.raw_set(k, b.bbox[i])?;
                }
            }
            if dimension {
                p.raw_set("dimension", snap.dimension.as_str())?;
            }
            if all || w.gamemode != last.gamemode {
                p.raw_set("gamemode", w.gamemode)?;
            }
            if all || w.xp != last.xp {
                p.raw_set("xp", w.xp)?;
            }
            if all || w.dead != last.dead {
                p.raw_set("dead", w.dead)?;
            }
            if all || w.op_level != last.op_level {
                p.raw_set("oplevel", w.op_level)?;
            }
            if all || w.screen_open != last.screen_open {
                api.screen.raw_set("open", w.screen_open)?;
            }
            if screen_moved {
                api.screen.raw_set("width", screen.0 as f64)?;
                api.screen.raw_set("height", screen.1 as f64)?;
            }
            if all || w.selected != last.selected {
                api.inventory.raw_set("selected", w.selected + 1)?;
            }
            if address {
                api.server.raw_set("address", snap.address.as_deref())?;
            }
        }
        let s = shown.get_or_insert_with(Shown::default);
        s.world = *w;
        if dimension {
            s.dimension.clone_from(&snap.dimension);
        }
        if address {
            s.address.clone_from(&snap.address);
        }
        s.screen = screen;
        self.shown = shown;
        Ok(())
    }

    pub fn leave(&mut self) {
        if !self.modules.iter().any(|l| l.running) {
            return;
        }
        self.dispatch(api::LEAVE, |_, _| Ok(()), |_, _| true);
        self.stop_all(true);
        self.parked = Some(self.epoch);
        WANTS.fetch_or(RESCAN, Relaxed);
    }

    fn rescan(&mut self) {
        let s = store();
        for k in 0..self.modules.len() {
            let (now, count) = (
                s.enabled(self.modules[k].ix),
                s.toggle_count(self.modules[k].ix),
            );
            let live = &mut self.modules[k];
            let restart = live.running && now && count != live.seen;
            live.seen = count;
            if now == live.running && !restart {
                continue;
            }
            if live.running {
                self.stop(k);
            }
            if now {
                self.start(k);
            }
        }
        self.relist();
    }

    fn start(&mut self, k: usize) {
        let live = &mut self.modules[k];
        live.running = true;
        live.ticks = 0;
        check(live.backing.raw_set("enabled", true));
        check(live.backing.raw_set("ticks", 0));
        if let Some(e) = &self.fixed[live.api][ENABLE] {
            self.call(k, ENABLE, e);
        }
    }

    fn stop(&mut self, k: usize) {
        self.stopped(k);
        if let Some(e) = &self.fixed[self.modules[k].api][DISABLE] {
            self.call(k, DISABLE, e);
        }
        super::render::drop_owner(k);
        super::ghost::drop_owner(k);
    }

    fn stopped(&mut self, k: usize) {
        let live = &mut self.modules[k];
        live.running = false;
        check(live.backing.raw_set("enabled", false));
        self.ctx
            .game
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear_hud(k);
        game::close_ui_of(k);
        game::release_scroll(k);
        super::render::drop_owner(k);
        super::ghost::drop_owner(k);
    }

    fn relist(&mut self) {
        let mut listen = 0u32;
        for ev in 0..EVENTS.len() {
            self.lists[ev] = (0..self.modules.len())
                .filter(|&k| self.modules[k].running && self.modules[k].handlers[ev].is_some())
                .collect();
            if !self.lists[ev].is_empty() {
                listen |= 1 << ev;
            }
        }
        let packets = (self.lists[PACKET].iter())
            .fold(super::packets::PacketSet::default(), |m, &k| {
                m.union(self.modules[k].packets)
            });
        game::LISTEN.store(listen, Relaxed);
        super::packets::store_mask(packets);
        if self.modules.iter().any(|l| l.running) {
            WANTS.fetch_or(RUNNING, Relaxed);
        } else {
            WANTS.fetch_and(!RUNNING, Relaxed);
        }
        if self.lists[CHAT].is_empty() {
            WANTS.fetch_and(!WANTS_CHAT, Relaxed);
        } else {
            WANTS.fetch_or(WANTS_CHAT, Relaxed);
        }
    }

    fn halt(&mut self, k: usize, in_world: bool) {
        if !self.modules[k].running {
            return;
        }
        if in_world {
            self.stop(k);
        } else {
            self.stopped(k);
        }
    }

    pub fn stop_all(&mut self, in_world: bool) {
        self.stopping = true;
        for k in 0..self.modules.len() {
            self.halt(k, in_world);
        }
        self.stopping = false;
        self.relist();
    }

    pub fn reset(&mut self, mod_id: &str, in_world: bool) {
        let s = store();
        self.stopping = true;
        for k in 0..self.modules.len() {
            if self.modules[k].mod_id != mod_id {
                continue;
            }
            self.halt(k, in_world);
            let live = &mut self.modules[k];
            s.set_enabled(live.ix, false);
            s.set_bind(live.ix, crate::gui::keybinds::Bound::Unbound);
            for set in &live.settings {
                set.reset();
            }
            live.seen = s.toggle_count(live.ix);
        }
        self.stopping = false;
        self.relist();
    }

    fn sync_settings(&mut self) {
        let s = store();
        for live in &mut self.modules {
            for set in &mut live.settings {
                if let Kind::Text { .. } = set.kind {
                    let moved = s.with_text(set.slot, |text| {
                        let moved = text != set.text;
                        if moved {
                            set.text.clear();
                            set.text.push_str(text);
                        }
                        moved
                    });
                    if moved {
                        check(live.values.raw_set(set.key, set.text.as_str()));
                    }
                    continue;
                }
                if let Kind::List { list } = set.kind {
                    let now = list.generation();
                    if now != set.generation {
                        set.generation = now;
                        let lua = &self.apis[live.api].lua;
                        check(
                            kinds::block_set(lua, list)
                                .and_then(|t| live.values.raw_set(set.key, t)),
                        );
                    }
                    continue;
                }
                let now = s.value(set.slot);
                if now == set.value {
                    continue;
                }
                set.value = now;
                let r = match (now, set.kind) {
                    (Some(Setting::Bool(b)), _) => live.values.raw_set(set.key, b),
                    (Some(Setting::Num(x)), Kind::Slider { decimals, .. }) => {
                        live.values.raw_set(set.key, api::shown(x, decimals))
                    }
                    (Some(Setting::Key(b)), _) => live.values.raw_set(set.key, b.serialized_name()),
                    (Some(Setting::Choice(c)), Kind::Enum { options, .. }) => live
                        .values
                        .raw_set(set.key, options.get(c as usize).copied()),
                    _ => Ok(()),
                };
                check(r);
            }
        }
    }

    fn call(&self, k: usize, ev: usize, e: &Table) {
        if let Some(f) = &self.modules[k].handlers[ev] {
            self.call_fn(k, f, Label::Event(ev), e);
        }
    }

    fn call_fn(&self, k: usize, f: &Function, label: Label<'_>, e: &Table) {
        let live = &self.modules[k];
        let disable = matches!(label, Label::Event(DISABLE));
        let lua = &self.apis[live.api];
        let ctx = &lua.ctx;
        ctx.current.store(k, Relaxed);
        ctx.arm();
        let result = lua.xpcall.call::<(bool, Value)>((f, &lua.msgh, &live.m, e));
        ctx.disarm();
        ctx.current.store(NONE, Relaxed);

        let error = match result {
            Ok((true, _)) => None,
            Ok((false, err)) => Some(api::describe(&err)),
            Err(err) => Some(err.to_string()),
        };
        let s = store();
        match error {
            None => live.overruns.set(0),
            Some(error)
                if ctx.timed_out.load(Relaxed) && !disable && live.overruns.get() + 1 < STRIKES =>
            {
                let n = live.overruns.get() + 1;
                live.overruns.set(n);
                let first = error.lines().next().unwrap_or_default();
                crate::log_warn!(
                    "mods",
                    "{} {label} stopped ({n} of {STRIKES} in a row before it is switched off): {first}",
                    ctx.tag(k)
                );
            }
            Some(error) => {
                live.overruns.set(0);
                let tag = ctx.tag(k);
                let what = if disable {
                    "reported only"
                } else {
                    "switched off"
                };
                crate::log_error!("mods", "{tag} {label} failed; {what}: {error}");
                api::print(&format!("{tag} {label} failed; {what}:\n{error}"));
                if !disable {
                    s.set_enabled(live.ix, false);
                    let first = error.lines().next().unwrap_or_default();
                    super::note_error(live.mod_id, format!("{tag} {label}: {first}"));
                }
            }
        }
        let asked =
            std::mem::take(&mut *ctx.switches.lock().unwrap_or_else(PoisonError::into_inner));
        if !self.stopping {
            for (d, switch) in asked {
                let ix = self.modules[d].ix;
                let on = match switch {
                    Switch::On => true,
                    Switch::Off => false,
                    Switch::Flip => !s.armed(ix),
                };
                s.set_enabled(ix, on);
            }
        }
    }
}
