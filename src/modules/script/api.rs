use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use mlua::{Error, Function, Lua, LuaOptions, StdLib, Table, UserData, Value, Variadic, VmState};

use super::super::commands::{Arg, ArgKind, Decl as CommandDecl};
use super::super::registry::{
    COUNT, Category, Kind, MODULES, Mode, SETTING_COUNT, SettingDef, Show,
};
use super::super::{Value as Setting, store};
use super::{OUTBOX, WANTS, kinds};
use crate::gui::keybinds::Bound;
use crate::platform::time::Instant;

pub(super) const BUDGET: Duration = Duration::from_millis(20);

pub(super) const LOAD_BUDGET: Duration = Duration::from_millis(50);

pub(super) const MEMORY: usize = 64 * 1024 * 1024;

const LOCKED: &str = "torch: locked";

const SEND_QUEUE: usize = 32;
const PRINT_QUEUE: usize = 64;
const SPAM_PER_LINE: u32 = 20;
const SPAM_CEILING: u32 = 160;
const SERVER_TICK: Duration = Duration::from_millis(50);

const COMMAND_QUEUE: usize = 4096;
const COMMANDS_PER_TICK: usize = 8;

pub(super) static OUT: Mutex<Outbox> = Mutex::new(Outbox {
    sends: Vec::new(),
    commands: std::collections::VecDeque::new(),
    prints: Vec::new(),
    pressure: 0,
    at: None,
    actions: Actions::NONE,
    copy: None,
});

pub(super) fn print(text: &str) {
    let mut out = OUT.lock().unwrap_or_else(PoisonError::into_inner);
    out.push_lines(text);
    WANTS.fetch_or(OUTBOX, Relaxed);
}

pub(super) fn drop_sends() {
    let mut out = OUT.lock().unwrap_or_else(PoisonError::into_inner);
    out.sends.clear();
    out.commands.clear();
    out.pressure = 0;
    out.at = None;
    out.actions = Actions::NONE;
    out.copy = None;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Actions {
    pub attack: bool,
    pub attack_id: Option<i32>,
    pub use_item: bool,
    pub respawn: bool,
    pub select: Option<u8>,
    pub look: Option<(f32, f32)>,
    pub swing: Option<bool>,
    pub velocity: Option<Push>,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(super) struct Push {
    pub set: [Option<f64>; 3],
    pub add: [f64; 3],
}

impl Push {
    pub const LIMIT: f64 = 10.0;

    pub fn apply(&self, v: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| {
            (self.set[i].unwrap_or(v[i]) + self.add[i]).clamp(-Self::LIMIT, Self::LIMIT)
        })
    }
}

impl Actions {
    pub const NONE: Actions = Actions {
        attack: false,
        attack_id: None,
        use_item: false,
        respawn: false,
        select: None,
        look: None,
        swing: None,
        velocity: None,
    };
}

pub(super) fn intern(s: String) -> &'static str {
    static SEEN: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
    let seen = seen.get_or_insert_with(HashSet::new);
    if let Some(&s) = seen.get(s.as_str()) {
        return s;
    }
    let s: &'static str = Box::leak(s.into_boxed_str());
    seen.insert(s);
    s
}

pub(super) fn shown(x: f32, decimals: u8) -> f64 {
    let scale = 10f64.powi(decimals as i32);
    (x as f64 * scale).round() / scale
}

pub(super) const EVENTS: [&str; 26] = [
    "enable",
    "disable",
    "tick",
    "chat",
    "chatsend",
    "join",
    "spawn",
    "death",
    "keydown",
    "attack",
    "packet",
    "render2d",
    "uiclick",
    "uiclose",
    "profile",
    "job",
    "keyup",
    "scroll",
    "use",
    "uiprompt",
    "button",
    "filerequest",
    "uikey",
    "chunk",
    "blockchange",
    "leave",
];
pub(super) const ENABLE: usize = 0;
pub(super) const DISABLE: usize = 1;
pub(super) const TICK: usize = 2;
pub(super) const CHAT: usize = 3;
pub(super) const CHATSEND: usize = 4;
pub(super) const JOIN: usize = 5;
pub(super) const SPAWN: usize = 6;
pub(super) const DEATH: usize = 7;
pub(super) const KEYDOWN: usize = 8;
pub(super) const ATTACK: usize = 9;
pub(super) const PACKET: usize = 10;
pub(super) const RENDER2D: usize = 11;
pub(super) const UICLICK: usize = 12;
pub(super) const UICLOSE: usize = 13;
pub(super) const PROFILE: usize = 14;
pub(super) const JOB: usize = 15;
pub(super) const KEYUP: usize = 16;
pub(super) const SCROLL: usize = 17;
pub(super) const USE: usize = 18;
pub(super) const UIPROMPT: usize = 19;
pub(super) const BUTTON: usize = 20;
pub(super) const FILEREQUEST: usize = 21;
pub(super) const UIKEY: usize = 22;
pub(super) const CHUNK: usize = 23;
pub(super) const BLOCKCHANGE: usize = 24;
pub(super) const LEAVE: usize = 25;

pub(super) fn cancelable(ev: usize) -> bool {
    matches!(ev, CHAT | CHATSEND | ATTACK | USE)
}

pub(super) const NONE: usize = usize::MAX;

pub(super) struct Ctx {
    pub loading: AtomicBool,
    pub current: AtomicUsize,
    pub labels: OnceLock<Vec<String>>,
    pub mod_ids: OnceLock<Vec<&'static str>>,
    pub perms: Mutex<HashSet<(&'static str, usize)>>,
    pub switches: Mutex<Vec<(usize, Switch)>>,
    warned: Mutex<HashSet<usize>>,
    epoch: Instant,
    deadline: AtomicU64,
    top: AtomicBool,
    pub timed_out: AtomicBool,
    tripped: AtomicBool,
    polls: AtomicU32,
    pub game: Mutex<super::game::Game>,
    pub load: Mutex<Loading>,
    pub jobs: AtomicU64,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Switch {
    On,
    Off,
    Flip,
}

pub(super) struct Outbox {
    sends: Vec<String>,
    commands: std::collections::VecDeque<(String, bool)>,
    pub prints: Vec<String>,
    pressure: u32,
    at: Option<Instant>,
    pub actions: Actions,
    pub copy: Option<String>,
}

impl Outbox {
    pub fn act(&mut self, f: impl FnOnce(&mut Actions)) {
        f(&mut self.actions);
        WANTS.fetch_or(OUTBOX, Relaxed);
    }

    pub fn copy(&mut self, text: String) {
        self.copy = Some(text);
        WANTS.fetch_or(OUTBOX, Relaxed);
    }

    fn push_lines(&mut self, text: &str) {
        self.prints.extend(
            text.split('\n')
                .map(|l| l.trim_end_matches('\r').to_string()),
        );
    }

    pub fn waiting(&self) -> bool {
        !self.sends.is_empty() || !self.commands.is_empty()
    }

    pub fn command(&mut self, line: String, op: bool) -> bool {
        if self.commands.len() >= COMMAND_QUEUE {
            return false;
        }
        self.commands.push_back((line, op));
        WANTS.fetch_or(OUTBOX, Relaxed);
        true
    }

    pub fn commands_now(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        while out.len() < COMMANDS_PER_TICK
            && let Some((_, op)) = self.commands.front()
        {
            if !*op {
                if self.pressure + SPAM_PER_LINE > SPAM_CEILING {
                    break;
                }
                self.pressure += SPAM_PER_LINE;
            }
            out.extend(self.commands.pop_front().map(|(line, _)| line));
        }
        out
    }

    pub fn sendable(&mut self, now: Instant) -> Vec<String> {
        let at = *self.at.get_or_insert(now);
        let ticks =
            (now.saturating_duration_since(at).as_millis() / SERVER_TICK.as_millis()) as u32;
        if ticks > 0 {
            self.pressure = self.pressure.saturating_sub(ticks);
            self.at = Some(at + SERVER_TICK * ticks);
        }
        let mut n = 0;
        while n < self.sends.len() && self.pressure + SPAM_PER_LINE <= SPAM_CEILING {
            self.pressure += SPAM_PER_LINE;
            n += 1;
        }
        self.sends.drain(..n).collect()
    }
}

#[derive(Default)]
pub(super) struct Loading {
    pub mod_id: &'static str,
    pub defs: Vec<Def>,
}

pub(super) struct Def {
    pub mod_id: &'static str,
    pub api: usize,
    pub name: &'static str,
    pub desc: &'static str,
    pub category: Category,
    pub mode: Mode,
    pub settings: Vec<(&'static str, SettingDef)>,
    pub m: Table,
    pub backing: Table,
    pub values: Table,
    pub handlers: [Option<Function>; EVENTS.len()],
    pub packets: super::packets::PacketSet,
    pub panels: Vec<PanelSpec>,
    pub commands: Vec<(CommandDecl, Function)>,
}

const COMMAND_FIELDS: [&str; 5] = ["name", "aliases", "tip", "args", "run"];
const COMMAND_ARG_FIELDS: [&str; 4] = ["name", "type", "optional", "suggest"];

const COMMANDS_PER_MODULE: usize = 32;

fn command_name(what: &str, field: &str, s: &str) -> mlua::Result<&'static str> {
    let ok = (1..=32).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        && !s.starts_with('-');
    if !ok {
        return Err(Error::runtime(format!(
            "{what}: {field} `{s}` is not 1 to 32 of a-z, 0-9, _ and -, starting with a letter, digit or _"
        )));
    }
    Ok(intern(s.to_string()))
}

fn words(what: &str, field: &str, v: Option<&Value>) -> mlua::Result<Vec<String>> {
    let t = match v {
        None => return Ok(Vec::new()),
        Some(Value::Table(t)) => t,
        Some(_) => {
            return Err(Error::runtime(format!(
                "{what}: `{field}` is a list of strings"
            )));
        }
    };
    let list = t
        .sequence_values::<String>()
        .collect::<mlua::Result<Vec<_>>>()
        .map_err(|_| Error::runtime(format!("{what}: `{field}` is a list of strings")))?;
    for w in &list {
        if !(1..=32).contains(&w.chars().count()) || w.chars().any(char::is_whitespace) {
            return Err(Error::runtime(format!(
                "{what}: {field} entry `{w}` is not one word of 1 to 32 characters"
            )));
        }
    }
    Ok(list)
}

fn command_decl(what: &'static str, spec: &Value) -> mlua::Result<(CommandDecl, Function)> {
    let f = Fields::read(spec, what, &COMMAND_FIELDS)?;
    let name = f
        .str("name")?
        .ok_or_else(|| Error::runtime(format!("{what}: `name` is required")))?;
    let name = command_name(what, "name", &name)?;
    let mut aliases = Vec::new();
    for a in words(what, "aliases", f.map.get("aliases"))? {
        let a = command_name(what, "alias", &a)?;
        if a != name && !aliases.contains(&a) {
            aliases.push(a);
        }
    }
    let tip = intern(f.str("tip")?.unwrap_or_default());
    let run = match f.map.get("run") {
        Some(Value::Function(run)) => run.clone(),
        None => {
            return Err(Error::runtime(format!(
                "{what}: `run` is required: run = function(m, e) ... end"
            )));
        }
        Some(_) => return Err(f.wrong("run", "function")),
    };
    let mut args: Vec<Arg> = Vec::new();
    match f.map.get("args") {
        None => {}
        Some(Value::Table(t)) => {
            for v in t.sequence_values::<Value>() {
                let a = Fields::read(&v?, "m:command args entry", &COMMAND_ARG_FIELDS)?;
                let arg_name = a
                    .str("name")?
                    .ok_or_else(|| Error::runtime(format!("{what}: every arg has a `name`")))?;
                if !(1..=32).contains(&arg_name.chars().count())
                    || arg_name.chars().any(char::is_whitespace)
                {
                    return Err(Error::runtime(format!(
                        "{what}: arg name `{arg_name}` is not one word of 1 to 32 characters"
                    )));
                }
                let ty = a.str("type")?.unwrap_or_else(|| "word".to_string());
                let Some(kind) = ArgKind::from_name(&ty) else {
                    return Err(Error::runtime(format!(
                        "{what}: arg `{arg_name}` has unknown type `{ty}`{}; one of {}",
                        suggest(&ty, &ArgKind::NAMES),
                        ArgKind::NAMES.join(", ")
                    )));
                };
                let optional = a.bool("optional")?.unwrap_or(false);
                if let Some(last) = args.last() {
                    if last.kind == ArgKind::String {
                        return Err(Error::runtime(format!(
                            "{what}: arg `{}` is a string, which takes the rest of the line, so it must be last",
                            last.name
                        )));
                    }
                    if last.optional && !optional {
                        return Err(Error::runtime(format!(
                            "{what}: arg `{arg_name}` is required but follows the optional `{}`",
                            last.name
                        )));
                    }
                }
                let offered = words(what, "suggest", a.map.get("suggest"))?
                    .into_iter()
                    .map(intern)
                    .collect();
                args.push(Arg {
                    name: intern(arg_name),
                    kind,
                    optional,
                    suggest: offered,
                });
            }
        }
        Some(_) => return Err(f.wrong("args", "list of { name = ..., type = ... }")),
    }
    Ok((
        CommandDecl {
            name,
            aliases,
            tip,
            args,
        },
        run,
    ))
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PanelSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub w: f32,
    pub h: f32,
    pub anchor: crate::gui::hud_layout::Anchor,
    pub x: f32,
    pub y: f32,
}

const PANEL_FIELDS: [&str; 6] = ["label", "w", "h", "anchor", "x", "y"];

fn panels_of(what: &str, v: Option<&Value>) -> mlua::Result<Vec<PanelSpec>> {
    use crate::gui::hud_layout::Anchor;
    let t = match v {
        None => return Ok(Vec::new()),
        Some(Value::Table(t)) => t,
        Some(_) => {
            return Err(Error::runtime(format!(
                "{what}: `panels` is a table of name = {{ w = ..., h = ... }}"
            )));
        }
    };
    let mut out = Vec::new();
    for pair in t.pairs::<Value, Value>() {
        let (k, v) = pair?;
        let Value::String(k) = k else {
            return Err(Error::runtime(format!(
                "{what}: every key of `panels` is a name"
            )));
        };
        let name = k.to_string_lossy();
        if !(1..=32).contains(&name.chars().count()) {
            return Err(Error::runtime(format!(
                "{what}: panel name `{name}` is not 1 to 32 characters"
            )));
        }
        let at = format!("{what}: panels.{name}");
        let Value::Table(p) = v else {
            return Err(Error::runtime(format!(
                "{at} is a table {{ w = ..., h = ..., label = ..., anchor = ... }}"
            )));
        };
        for key in p.pairs::<Value, Value>() {
            let (key, _) = key?;
            let key = match key {
                Value::String(s) => s.to_string_lossy(),
                _ => return Err(Error::runtime(format!("{at}: every key is a name"))),
            };
            if !PANEL_FIELDS.contains(&key.as_str()) {
                return Err(Error::runtime(format!(
                    "{at}: `{key}` is not one of {}{}",
                    PANEL_FIELDS.join(", "),
                    suggest(&key, &PANEL_FIELDS)
                )));
            }
        }
        let num = |k: &str, default: Option<f64>, lo: f64, hi: f64| -> mlua::Result<f32> {
            let v = match p.raw_get::<Value>(k)? {
                Value::Nil => {
                    default.ok_or_else(|| Error::runtime(format!("{at}: `{k}` is required")))?
                }
                Value::Integer(i) => i as f64,
                Value::Number(n) => n,
                other => {
                    return Err(Error::runtime(format!(
                        "{at}: `{k}` is a number, not a {}",
                        other.type_name()
                    )));
                }
            };
            if !(lo..=hi).contains(&v) {
                return Err(Error::runtime(format!(
                    "{at}: `{k}` ({v}) is outside {lo}..{hi}"
                )));
            }
            Ok(v as f32)
        };
        let label = match p.raw_get::<Option<String>>("label")? {
            Some(l) if (1..=32).contains(&l.chars().count()) => l,
            Some(l) => {
                return Err(Error::runtime(format!(
                    "{at}: label `{l}` is not 1 to 32 characters"
                )));
            }
            None => name.clone(),
        };
        let anchor = match p.raw_get::<Option<String>>("anchor")? {
            None => Anchor::TopLeft,
            Some(a) => Anchor::parse(&a).ok_or_else(|| {
                Error::runtime(format!(
                    "{at}: anchor `{a}` is not one of {}",
                    Anchor::names().join(", ")
                ))
            })?,
        };
        out.push(PanelSpec {
            name: intern(name),
            label: intern(label),
            w: num("w", None, 1.0, 1024.0)?,
            h: num("h", None, 1.0, 1024.0)?,
            anchor,
            x: num("x", Some(0.0), -4096.0, 4096.0)?,
            y: num("y", Some(0.0), -4096.0, 4096.0)?,
        });
    }
    out.sort_by(|a, b| a.name.cmp(b.name));
    Ok(out)
}

impl Ctx {
    pub fn new() -> Ctx {
        Ctx {
            loading: AtomicBool::new(true),
            current: AtomicUsize::new(NONE),
            labels: OnceLock::new(),
            mod_ids: OnceLock::new(),
            perms: Mutex::new(HashSet::new()),
            switches: Mutex::new(Vec::new()),
            warned: Mutex::new(HashSet::new()),
            epoch: Instant::now(),
            deadline: AtomicU64::new(0),
            top: AtomicBool::new(false),
            timed_out: AtomicBool::new(false),
            tripped: AtomicBool::new(false),
            polls: AtomicU32::new(0),
            game: Mutex::new(super::game::Game::default()),
            load: Mutex::new(Loading::default()),
            jobs: AtomicU64::new(0),
        }
    }

    pub fn bot(&self, what: &str) -> mlua::Result<azalea::Client> {
        (self
            .game
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .bot
            .clone())
        .ok_or_else(|| no_world_here(what))
    }

    pub fn world(&self, what: &str) -> mlua::Result<Option<super::game::WorldLock>> {
        let game = self.game.lock().unwrap_or_else(PoisonError::into_inner);
        if game.bot.is_none() {
            return Err(no_world_here(what));
        }
        Ok(game.world.clone())
    }

    pub fn arm(&self) {
        self.timed_out.store(false, Relaxed);
        self.tripped.store(false, Relaxed);
        self.top.store(false, Relaxed);
        let at = (self.epoch.elapsed() + BUDGET).as_nanos() as u64;
        self.deadline.store(at.max(1), Relaxed);
    }

    pub fn disarm(&self) {
        self.deadline.store(0, Relaxed);
        self.tripped.store(false, Relaxed);
    }

    fn now(&self) -> u64 {
        self.epoch.elapsed().as_nanos() as u64
    }

    pub fn top_level<R>(&self, f: impl FnOnce() -> R) -> R {
        let outer = self.deadline.load(Relaxed);
        let own = self.now() + LOAD_BUDGET.as_nanos() as u64;
        let at_run_time = outer != 0 && !self.loading.load(Relaxed);
        let was_top = self.top.swap(!at_run_time || own < outer, Relaxed);
        let left = outer.saturating_sub(self.now());
        self.deadline
            .store(if at_run_time { own.min(outer) } else { own }, Relaxed);
        let r = f();
        self.top.store(was_top, Relaxed);
        if at_run_time {
            self.deadline.store(outer, Relaxed);
            if self.now() < outer {
                self.tripped.store(false, Relaxed);
                self.timed_out.store(false, Relaxed);
            } else {
                self.tripped.store(true, Relaxed);
            }
        } else {
            self.tripped.store(false, Relaxed);
            self.deadline.store(
                if outer == 0 {
                    0
                } else {
                    (self.now() + left).max(1)
                },
                Relaxed,
            );
        }
        r
    }

    pub fn tag(&self, k: usize) -> String {
        if self.loading.load(Relaxed) {
            let load = self.load.lock().unwrap_or_else(PoisonError::into_inner);
            return format!("[mods/{}]", load.mod_id);
        }
        match self.labels.get().and_then(|l| l.get(k)) {
            Some(label) => format!("[mods/{label}]"),
            None => "[mods]".to_string(),
        }
    }

    pub(super) fn at_load(&self, what: &str) -> mlua::Result<()> {
        if self.loading.load(Relaxed) {
            return Err(too_early(what));
        }
        Ok(())
    }

    pub(super) fn caller(&self, what: &str) -> mlua::Result<(usize, &'static str)> {
        let k = self.current.load(Relaxed);
        let id = (k != NONE)
            .then(|| self.mod_ids.get().and_then(|ids| ids.get(k).copied()))
            .flatten();
        id.map(|id| (k, id))
            .ok_or_else(|| Error::runtime(format!("{what} works inside a module's handlers")))
    }
}

fn no_world_here(what: &str) -> Error {
    Error::runtime(format!(
        "{what} can't read the world here: it works in handlers the game calls during play, \
         not in an ondisable run by Reload, Reset data or leaving a world"
    ))
}

fn too_early(what: &str) -> Error {
    Error::runtime(format!(
        "{what} can't be used while the mod loads: a file's top level may only call \
         require, torch.module and torch.setting.*"
    ))
}

pub(super) struct Api {
    pub lua: Lua,
    pub ctx: Arc<Ctx>,
    pub torch: Table,
    pub globals: Table,
    pub player: Table,
    pub screen: Table,
    pub server: Table,
    pub inventory: Table,
    pub bbox: Table,
    player_mt: Table,
    screen_mt: Table,
    pub xpcall: Function,
    pub msgh: Function,
    prevent: Function,
    classes: Vec<Table>,
    packet_debug: Table,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Class {
    Plain,
    PacketDebug,
}

impl Api {
    pub fn new(ctx: Arc<Ctx>) -> mlua::Result<Api> {
        let lua = Lua::new_with(
            StdLib::MATH
                | StdLib::STRING
                | StdLib::TABLE
                | StdLib::UTF8
                | StdLib::COROUTINE
                | StdLib::BIT
                | StdLib::BUFFER,
            LuaOptions::default(),
        )?;

        let globals = lua.globals();
        for name in [
            "os",
            "debug",
            "load",
            "loadstring",
            "dofile",
            "loadfile",
            "getfenv",
            "setfenv",
            "require",
        ] {
            globals.raw_set(name, Value::Nil)?;
        }
        let log = log_fn(&lua, &ctx)?;
        globals.raw_set("print", &log)?;
        let rawset: Function = globals.raw_get("rawset")?;
        globals.raw_set(
            "rawset",
            lua.create_function(move |_, (t, k, v): (Table, Value, Value)| {
                let guarded = t
                    .metatable()
                    .and_then(|mt| mt.raw_get::<Option<String>>("__metatable").ok().flatten())
                    .is_some_and(|s| s == LOCKED);
                if guarded {
                    return Err(Error::runtime(
                        "rawset can't be used on torch's tables (m, m.settings, events); assign normally",
                    ));
                }
                rawset.call::<Table>((t, k, v))
            })?,
        )?;
        let xpcall: Function = globals.raw_get("xpcall")?;
        lua.sandbox(true)?;
        let globals = lua.globals();

        let clock = ctx.clone();
        lua.set_interrupt(move |_| {
            let message = |clock: &Ctx| {
                Error::runtime(if clock.top.load(Relaxed) {
                    format!(
                        "a file's top level ran for more than {} ms",
                        LOAD_BUDGET.as_millis()
                    )
                } else {
                    format!("ran for more than {} ms in one call", BUDGET.as_millis())
                })
            };
            if clock.tripped.load(Relaxed) {
                return Err(message(&clock));
            }
            let deadline = clock.deadline.load(Relaxed);
            if deadline == 0 {
                return Ok(VmState::Continue);
            }
            let polls = clock.polls.load(Relaxed);
            clock.polls.store(polls.wrapping_add(1), Relaxed);
            if polls % 64 != 0 {
                return Ok(VmState::Continue);
            }
            if (clock.epoch.elapsed().as_nanos() as u64) < deadline {
                return Ok(VmState::Continue);
            }
            clock.tripped.store(true, Relaxed);
            clock.timed_out.store(true, Relaxed);
            Err(message(&clock))
        });
        if let Err(e) = lua.set_memory_limit(MEMORY) {
            crate::log_warn!("mods", "no memory limit on the Lua state: {e}");
        }

        let msgh =
            lua.create_function(|lua, err: Value| lua.traceback(Some(describe(&err).as_str()), 1))?;

        let prevent = {
            let ctx = ctx.clone();
            lua.create_function(move |_, e: Table| {
                let mt = (e.metatable())
                    .ok_or_else(|| Error::runtime("preventDefault is called on an event: e:preventDefault()"))?;
                if e.get::<bool>("cancelable")? {
                    mt.raw_get::<Table>("__index")?.raw_set("defaultPrevented", true)?;
                    return Ok(());
                }
                let k = ctx.current.load(Relaxed);
                if ctx.warned.lock().unwrap_or_else(PoisonError::into_inner).insert(k) {
                    let ty: String = e.get::<Option<String>>("type")?.unwrap_or_default();
                    crate::log_warn!(
                        "mods",
                        "{} e:preventDefault() on a `{ty}` event, which cannot be cancelled; ignored",
                        ctx.tag(k)
                    );
                }
                Ok(())
            })?
        };

        let (player, player_mt) = guarded(&lua, "torch.player")?;
        let (screen, screen_mt) = guarded(&lua, "torch.screen")?;

        let bbox = lua.create_table()?;
        player.raw_set("box", &bbox)?;
        player_velocity(&lua, &ctx, &player)?;

        let torch = lua.create_table()?;
        torch.raw_set("module", module_fn(&lua, &ctx)?)?;
        torch.raw_set("setting", setting_table(&lua)?)?;
        torch.raw_set("player", view(&lua, &player_mt)?)?;
        torch.raw_set("screen", view(&lua, &screen_mt)?)?;
        torch.raw_set("chat", chat_table(&lua, &ctx)?)?;
        torch.raw_set("log", &log)?;
        let game = super::game::tables(&lua, &ctx)?;
        torch.raw_set("server", &game.server)?;
        torch.raw_set("world", &game.world)?;
        torch.raw_set("inventory", &game.inventory)?;
        torch.raw_set("game", &game.game)?;
        torch.raw_set("input", &game.input)?;
        torch.raw_set("hud", &game.hud)?;
        torch.raw_set("util", &game.util)?;
        torch.raw_set("clipboard", &game.clipboard)?;
        torch.raw_set("ui", &game.ui)?;
        torch.raw_set("profile", &game.profile)?;
        torch.raw_set("fs", super::fs::table(&lua, &ctx)?)?;
        torch.raw_set("json", super::json::table(&lua)?)?;
        torch.raw_set("render", super::render::table(&lua, &ctx)?)?;
        torch.raw_set("nbt", super::nbt::table(&lua, &ctx)?)?;
        torch.raw_set("net", super::packets::table(&lua, &ctx)?)?;
        torch.raw_set("ghost", super::ghost::table(&lua, &ctx)?)?;
        let (blocks, items) = super::blocks::tables(&lua, &ctx)?;
        torch.raw_set("blocks", blocks)?;
        torch.raw_set("items", items)?;
        torch.raw_set("image", super::image::table(&lua, &ctx)?)?;
        torch.raw_set("time", super::game::time_table(&lua)?)?;
        torch.set_readonly(true);

        let class = |index: Value| -> mlua::Result<Table> {
            let mt = lua.create_table_with_capacity(0, 2)?;
            mt.raw_set("__index", index)?;
            mt.raw_set("__metatable", LOCKED)?;
            Ok(mt)
        };
        let mut classes = Vec::with_capacity(EVENTS.len());
        for ty in EVENTS {
            let index = lua.create_table_with_capacity(0, 4)?;
            index.raw_set("type", ty)?;
            index.raw_set("cancelable", false)?;
            index.raw_set("defaultPrevented", false)?;
            index.raw_set("preventDefault", &prevent)?;
            classes.push(class(Value::Table(index))?);
        }
        let packet_base: Table = classes[PACKET].raw_get("__index")?;
        let packet_debug = class(Value::Function(super::packets::debug_index(
            &lua,
            packet_base,
        )?))?;

        Ok(Api {
            lua,
            ctx,
            torch,
            globals,
            player,
            screen,
            server: game.server_backing,
            inventory: game.inventory_backing,
            bbox,
            player_mt,
            screen_mt,
            xpcall,
            msgh,
            prevent,
            classes,
            packet_debug,
        })
    }

    pub fn finish_loading(&self) -> mlua::Result<()> {
        self.ctx.loading.store(false, Relaxed);
        self.player_mt.raw_set("__index", &self.player)?;
        self.screen_mt.raw_set("__index", &self.screen)?;
        Ok(())
    }

    pub fn cancelable_event(&self, ty: &str, prevented: bool) -> mlua::Result<(Table, Table)> {
        let backing = self.lua.create_table()?;
        backing.raw_set("type", ty)?;
        backing.raw_set("cancelable", true)?;
        backing.raw_set("defaultPrevented", prevented)?;
        backing.raw_set("preventDefault", &self.prevent)?;
        let e = proxy(&self.lua, &backing)?;
        e.set_readonly(true);
        Ok((e, backing))
    }

    pub fn event_table(&self) -> mlua::Result<Table> {
        self.lua.create_table_with_capacity(0, EVENT_FIELDS)
    }

    pub fn seal(&self, e: &Table, ev: usize, class: Class) -> mlua::Result<()> {
        let mt = match class {
            Class::Plain => &self.classes[ev],
            Class::PacketDebug => &self.packet_debug,
        };
        e.set_metatable(Some(mt.clone()))?;
        e.set_readonly(true);
        Ok(())
    }
}

const EVENT_FIELDS: usize = 4;

pub(super) fn describe(err: &Value) -> String {
    fn root(e: &Error) -> &Error {
        match e {
            Error::CallbackError { cause, .. } | Error::WithContext { cause, .. } => root(cause),
            e => e,
        }
    }
    match err {
        Value::Error(e) => root(e).to_string(),
        Value::String(s) => s.to_string_lossy(),
        other => other
            .to_string()
            .unwrap_or_else(|_| other.type_name().to_string()),
    }
}

fn proxy(lua: &Lua, index: &Table) -> mlua::Result<Table> {
    let mt = lua.create_table()?;
    mt.raw_set("__index", index)?;
    mt.raw_set("__metatable", LOCKED)?;
    let t = lua.create_table()?;
    t.set_metatable(Some(mt))?;
    Ok(t)
}

pub(super) fn view_of(lua: &Lua, backing: &Table) -> mlua::Result<Table> {
    let t = proxy(lua, backing)?;
    t.set_readonly(true);
    Ok(t)
}

fn view(lua: &Lua, mt: &Table) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set_metatable(Some(mt.clone()))?;
    t.set_readonly(true);
    Ok(t)
}

fn player_velocity(lua: &Lua, ctx: &Arc<Ctx>, player: &Table) -> mlua::Result<()> {
    fn axis(v: Option<f64>, what: &str) -> mlua::Result<Option<f64>> {
        match v {
            Some(x) if !x.is_finite() => Err(Error::runtime(format!(
                "{what} takes finite numbers, not {x}"
            ))),
            v => Ok(v),
        }
    }
    let c = ctx.clone();
    player.raw_set(
        "setvelocity",
        lua.create_function(
            move |_, (x, y, z): (Option<f64>, Option<f64>, Option<f64>)| {
                let what = "torch.player.setvelocity";
                c.at_load(what)?;
                let set = [axis(x, what)?, axis(y, what)?, axis(z, what)?];
                OUT.lock().unwrap_or_else(PoisonError::into_inner).act(|a| {
                    let p = a.velocity.get_or_insert_with(Push::default);
                    for i in 0..3 {
                        if let Some(v) = set[i] {
                            p.set[i] = Some(v);
                            p.add[i] = 0.0;
                        }
                    }
                });
                Ok(())
            },
        )?,
    )?;
    let c = ctx.clone();
    player.raw_set(
        "addvelocity",
        lua.create_function(
            move |_, (x, y, z): (Option<f64>, Option<f64>, Option<f64>)| {
                let what = "torch.player.addvelocity";
                c.at_load(what)?;
                let add = [axis(x, what)?, axis(y, what)?, axis(z, what)?];
                OUT.lock().unwrap_or_else(PoisonError::into_inner).act(|a| {
                    let p = a.velocity.get_or_insert_with(Push::default);
                    for i in 0..3 {
                        p.add[i] += add[i].unwrap_or(0.0);
                    }
                });
                Ok(())
            },
        )?,
    )?;
    Ok(())
}

fn guarded(lua: &Lua, what: &'static str) -> mlua::Result<(Table, Table)> {
    let backing = lua.create_table()?;
    let mt = lua.create_table()?;
    mt.raw_set(
        "__index",
        lua.create_function(move |_, _: Variadic<Value>| -> mlua::Result<Value> {
            Err(too_early(what))
        })?,
    )?;
    mt.raw_set("__metatable", LOCKED)?;
    Ok((backing, mt))
}

fn log_fn(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Function> {
    let ctx = ctx.clone();
    lua.create_function(move |_, args: Variadic<Value>| {
        let mut line = String::new();
        for (i, v) in args.iter().enumerate() {
            if i > 0 {
                line.push('\t');
            }
            line.push_str(&v.to_string()?);
        }
        crate::log_info!(&ctx.tag(ctx.current.load(Relaxed)), "{line}");
        Ok(())
    })
}

fn chat_table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let chat = lua.create_table()?;
    let c = ctx.clone();
    chat.raw_set(
        "send",
        lua.create_function(move |_, text: String| {
            c.at_load("torch.chat.send")?;
            let mut out = OUT.lock().unwrap_or_else(PoisonError::into_inner);
            if out.sends.len() >= SEND_QUEUE {
                return Ok(false);
            }
            out.sends.push(text);
            WANTS.fetch_or(OUTBOX, Relaxed);
            Ok(true)
        })?,
    )?;
    let c = ctx.clone();
    chat.raw_set(
        "print",
        lua.create_function(move |_, text: String| {
            c.at_load("torch.chat.print")?;
            let mut out = OUT.lock().unwrap_or_else(PoisonError::into_inner);
            if out.prints.len() >= PRINT_QUEUE {
                return Ok(false);
            }
            out.push_lines(&text);
            WANTS.fetch_or(OUTBOX, Relaxed);
            Ok(true)
        })?,
    )?;
    chat.set_readonly(true);
    Ok(chat)
}

#[derive(Clone)]
struct Spec {
    label: Option<String>,
    tip: String,
    kind: SpecKind,
    visible: Option<kinds::Visible>,
    seq: u64,
}

pub(super) fn next_seq() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    SEQ.fetch_add(1, Relaxed)
}

#[derive(Clone)]
enum SpecKind {
    Toggle(bool),
    Slider {
        min: f32,
        max: f32,
        value: f32,
        decimals: u8,
    },
    Text {
        max_len: usize,
        default: String,
        hint: String,
    },
    Key(Bound),
    Button(String),
    Choice {
        options: Vec<String>,
        index: u8,
    },
    Blocks {
        title: String,
        default: Vec<String>,
    },
}

const BUTTON_TEXT: usize = 16;

pub(super) fn bound(name: &str) -> Result<Bound, String> {
    let unbound = Bound::Unbound.serialized_name();
    if name.is_empty() || name == unbound {
        return Ok(Bound::Unbound);
    }
    match Bound::from_serialized_name(name) {
        Bound::Unbound => Err(format!(
            "`{name}` is not a key name; they look like key.keyboard.j, key.keyboard.left.shift or key.mouse.left"
        )),
        b => Ok(b),
    }
}

impl UserData for Spec {}

fn setting_table(lua: &Lua) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set(
        "toggle",
        lua.create_function(|lua, arg: Value| {
            let f = Fields::read(
                &arg,
                "torch.setting.toggle",
                &["default", "label", "tip", "visible"],
            )?;
            let kind = SpecKind::Toggle(f.bool("default")?.unwrap_or(false));
            lua.create_userdata(f.spec(kind)?)
        })?,
    )?;
    t.raw_set(
        "slider",
        lua.create_function(|lua, arg: Value| {
            let what = "torch.setting.slider";
            let f = Fields::read(
                &arg,
                what,
                &[
                    "min", "max", "default", "decimals", "label", "tip", "visible",
                ],
            )?;
            let need = |k: &str| {
                f.num(k)?
                    .ok_or_else(|| Error::runtime(format!("{what}: `{k}` is required")))
            };
            let (min, max) = (need("min")?, need("max")?);
            if min > max {
                return Err(Error::runtime(format!(
                    "{what}: min ({min}) is greater than max ({max})"
                )));
            }
            let value = f.num("default")?.unwrap_or(min);
            if !(min..=max).contains(&value) {
                return Err(Error::runtime(format!(
                    "{what}: default ({value}) is outside {min}..{max}"
                )));
            }
            let decimals = match f.num("decimals")? {
                None => 0,
                Some(d) if d.fract() == 0.0 && (0.0..=6.0).contains(&d) => d as u8,
                Some(d) => {
                    return Err(Error::runtime(format!(
                        "{what}: decimals ({d}) is not a whole number from 0 to 6"
                    )));
                }
            };
            let kind = SpecKind::Slider {
                min: min as f32,
                max: max as f32,
                value: value as f32,
                decimals,
            };
            lua.create_userdata(f.spec(kind)?)
        })?,
    )?;
    t.raw_set(
        "text",
        lua.create_function(|lua, arg: Value| {
            let what = "torch.setting.text";
            let f = Fields::read(
                &arg,
                what,
                &[
                    "default",
                    "maxlength",
                    "placeholder",
                    "label",
                    "tip",
                    "visible",
                ],
            )?;
            let max_len = match f.num("maxlength")? {
                None => 64,
                Some(n) if n.fract() == 0.0 && n >= 1.0 => n as usize,
                Some(n) => {
                    return Err(Error::runtime(format!(
                        "{what}: maxlength ({n}) is not a whole number of at least 1"
                    )));
                }
            };
            let default = f.str("default")?.unwrap_or_default();
            if default.chars().count() > max_len {
                return Err(Error::runtime(format!(
                    "{what}: default is longer than maxlength ({max_len})"
                )));
            }
            let hint = f.str("placeholder")?.unwrap_or_default();
            lua.create_userdata(f.spec(SpecKind::Text {
                max_len,
                default,
                hint,
            })?)
        })?,
    )?;
    t.raw_set(
        "key",
        lua.create_function(|lua, arg: Value| {
            let what = "torch.setting.key";
            let f = Fields::read(&arg, what, &["default", "label", "tip", "visible"])?;
            let default = match f.str("default")? {
                None => Bound::Unbound,
                Some(name) => bound(&name).map_err(|e| Error::runtime(format!("{what}: {e}")))?,
            };
            lua.create_userdata(f.spec(SpecKind::Key(default))?)
        })?,
    )?;
    t.raw_set(
        "button",
        lua.create_function(|lua, arg: Value| {
            let what = "torch.setting.button";
            let f = Fields::read(&arg, what, &["text", "label", "tip", "visible"])?;
            let text = f.str("text")?.unwrap_or_else(|| "Open".to_string());
            let chars = text.chars().count();
            if !(1..=BUTTON_TEXT).contains(&chars) {
                return Err(Error::runtime(format!(
                    "{what}: text `{text}` is {chars} characters; 1 to {BUTTON_TEXT}"
                )));
            }
            lua.create_userdata(f.spec(SpecKind::Button(text))?)
        })?,
    )?;
    t.raw_set(
        "choice",
        lua.create_function(|lua, arg: Value| {
            let what = "torch.setting.choice";
            let f = Fields::read(
                &arg,
                what,
                &["options", "default", "label", "tip", "visible"],
            )?;
            let options = kinds::options(what, f.map.get("options"))?;
            let index = match f.str("default")? {
                None => 0,
                Some(d) => options.iter().position(|o| *o == d).ok_or_else(|| {
                    Error::runtime(format!(
                        "{what}: default `{d}` is not one of the options ({})",
                        options.join(", ")
                    ))
                })?,
            };
            lua.create_userdata(f.spec(SpecKind::Choice {
                options,
                index: index as u8,
            })?)
        })?,
    )?;
    t.raw_set(
        "blocks",
        lua.create_function(|lua, arg: Value| {
            let what = "torch.setting.blocks";
            let f = Fields::read(&arg, what, &["default", "title", "label", "tip", "visible"])?;
            let default = match f.map.get("default") {
                None => Vec::new(),
                Some(Value::Table(t)) => kinds::block_ids(&format!("{what}: default"), t)?,
                Some(_) => return Err(f.wrong("default", "list of block ids")),
            };
            let title = f.str("title")?.unwrap_or_else(|| "Blocks".to_string());
            let chars = title.chars().count();
            if !(1..=32).contains(&chars) {
                return Err(Error::runtime(format!(
                    "{what}: title `{title}` is {chars} characters; 1 to 32"
                )));
            }
            lua.create_userdata(f.spec(SpecKind::Blocks { title, default })?)
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}

struct Fields {
    what: &'static str,
    map: HashMap<String, Value>,
}

impl Fields {
    fn read(arg: &Value, what: &'static str, allowed: &[&str]) -> mlua::Result<Fields> {
        let Value::Table(t) = arg else {
            return Err(Error::runtime(format!(
                "{what} takes a table: {what} {{ ... }}"
            )));
        };
        let mut map = HashMap::new();
        let pairs: Vec<(Value, Value)> = t.pairs::<Value, Value>().collect::<mlua::Result<_>>()?;
        for (k, v) in pairs {
            let Value::String(k) = k else {
                return Err(Error::runtime(format!("{what}: every key is a field name")));
            };
            let k = k.to_string_lossy();
            if !allowed.contains(&k.as_str()) {
                return Err(Error::runtime(format!(
                    "{what}: unknown field `{k}`{}",
                    suggest(&k, allowed)
                )));
            }
            map.insert(k, v);
        }
        Ok(Fields { what, map })
    }

    fn wrong(&self, k: &str, ty: &str) -> Error {
        Error::runtime(format!("{}: `{k}` is a {ty}", self.what))
    }

    fn bool(&self, k: &str) -> mlua::Result<Option<bool>> {
        match self.map.get(k) {
            None => Ok(None),
            Some(Value::Boolean(b)) => Ok(Some(*b)),
            Some(_) => Err(self.wrong(k, "boolean")),
        }
    }

    fn num(&self, k: &str) -> mlua::Result<Option<f64>> {
        match self.map.get(k) {
            None => Ok(None),
            Some(Value::Integer(i)) => Ok(Some(*i as f64)),
            Some(Value::Number(n)) if n.is_finite() => Ok(Some(*n)),
            Some(_) => Err(self.wrong(k, "finite number")),
        }
    }

    fn str(&self, k: &str) -> mlua::Result<Option<String>> {
        match self.map.get(k) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.to_string_lossy())),
            Some(_) => Err(self.wrong(k, "string")),
        }
    }

    fn spec(&self, kind: SpecKind) -> mlua::Result<Spec> {
        Ok(Spec {
            label: self.str("label")?,
            tip: self.str("tip")?.unwrap_or_default(),
            kind,
            visible: self
                .map
                .get("visible")
                .map(|v| kinds::visible(self.what, v))
                .transpose()?,
            seq: next_seq(),
        })
    }
}

const MODULE_FIELDS: [&str; 7] = [
    "name",
    "category",
    "description",
    "mode",
    "settings",
    "packets",
    "panels",
];

const M_PROPS: [&str; 8] = [
    "name", "enabled", "ticks", "settings", "enable", "disable", "toggle", "command",
];

fn module_fn(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Function> {
    let ctx = ctx.clone();
    lua.create_function(move |lua, arg: Value| {
        if !ctx.loading.load(Relaxed) {
            return Err(Error::runtime(
                "torch.module can only be called while the mod loads, from a file's top level",
            ));
        }
        let what = "torch.module";
        let f = Fields::read(&arg, what, &MODULE_FIELDS)?;

        let name = f.str("name")?.ok_or_else(|| Error::runtime(format!("{what}: `name` is required")))?;
        let chars = name.chars().count();
        if !(1..=32).contains(&chars) {
            return Err(Error::runtime(format!("{what}: name `{name}` is {chars} characters; 1 to 32")));
        }
        let category = match f.str("category")?.as_deref() {
            None | Some("misc") => Category::Misc,
            Some("combat") => Category::Combat,
            Some("render") => Category::Render,
            Some("world") => Category::World,
            Some("movement") => Category::Movement,
            Some(other) => {
                return Err(Error::runtime(format!(
                    "{what}: unknown category `{other}`; one of combat, render, world, movement, misc"
                )));
            }
        };
        let mode = match f.str("mode")?.as_deref() {
            None | Some("blatant") => Mode::Blatant,
            Some("legit") => Mode::Legit,
            Some("normal") => Mode::Normal,
            Some(other) => {
                return Err(Error::runtime(format!("{what}: unknown mode `{other}`; one of legit, normal, blatant")));
            }
        };
        let desc = f.str("description")?.unwrap_or_default();
        let packets = match f.map.get("packets") {
            None => super::packets::PacketSet::default(),
            Some(Value::Table(t)) => {
                let names = t.sequence_values::<String>().collect::<mlua::Result<Vec<_>>>().map_err(|_| {
                    Error::runtime(format!("{what}: `packets` is a list of names, {{ \"SetHealth\", ... }}"))
                })?;
                super::packets::packet_set(&names).map_err(|e| Error::runtime(format!("{what}: {e}")))?
            }
            Some(_) => return Err(f.wrong("packets", "list of packet names")),
        };
        let panels = panels_of(what, f.map.get("panels"))?;

        let mut settings: Vec<(String, Spec)> = Vec::new();
        match f.map.get("settings") {
            None => {}
            Some(Value::Table(t)) => {
                let pairs: Vec<(Value, Value)> = t.pairs::<Value, Value>().collect::<mlua::Result<_>>()?;
                for (k, v) in pairs {
                    let Value::String(k) = k else {
                        return Err(Error::runtime(format!("{what}: every key of `settings` is a name")));
                    };
                    let k = k.to_string_lossy();
                    let spec = match &v {
                        Value::UserData(ud) => ud.borrow::<Spec>().ok().map(|s| (*s).clone()),
                        _ => None,
                    };
                    let Some(spec) = spec else {
                        return Err(Error::runtime(format!(
                            "{what}: settings.{k} is not made with torch.setting.toggle, slider, text, key, button, choice or blocks"
                        )));
                    };
                    settings.push((k, spec));
                }
            }
            Some(_) => return Err(f.wrong("settings", "table")),
        }
        settings.sort_by(|a, b| a.1.seq.cmp(&b.1.seq).then_with(|| a.0.cmp(&b.0)));

        let (k, taken, slots, mod_id) = {
            let load = ctx.load.lock().unwrap_or_else(PoisonError::into_inner);
            let lower = name.to_lowercase();
            let taken = MODULES.iter().map(|m| m.name).chain(load.defs.iter().map(|d| d.name))
                .find(|n| n.to_lowercase() == lower);
            let slots: usize = load.defs.iter().map(|d| d.settings.len()).sum();
            (load.defs.len(), taken, slots, load.mod_id)
        };
        if let Some(other) = taken {
            return Err(Error::runtime(format!(
                "{what}: a module named `{other}` already exists; names are unique across the client and every mod, ignoring case"
            )));
        }
        if COUNT + k + 1 > u16::MAX as usize || SETTING_COUNT + slots + settings.len() > u16::MAX as usize {
            return Err(Error::runtime(format!("{what}: too many modules or settings across every mod")));
        }

        let values = lua.create_table()?;
        let mut defs = Vec::with_capacity(settings.len());
        let mut visibles = Vec::with_capacity(settings.len());
        for (key, spec) in settings {
            visibles.push(spec.visible);
            let kind = match spec.kind {
                SpecKind::Toggle(on) => {
                    values.raw_set(key.as_str(), on)?;
                    Kind::Toggle { on }
                }
                SpecKind::Slider { min, max, value, decimals } => {
                    values.raw_set(key.as_str(), value as f64)?;
                    Kind::Slider { min, max, value, decimals }
                }
                SpecKind::Text { max_len, default, hint } => {
                    values.raw_set(key.as_str(), default.as_str())?;
                    Kind::Text { max_len, default: intern(default), expand: false, hint: intern(hint) }
                }
                SpecKind::Key(default) => {
                    values.raw_set(key.as_str(), default.serialized_name())?;
                    Kind::Key { default }
                }
                SpecKind::Button(text) => Kind::Button { text: intern(text), key: intern(key.clone()) },
                SpecKind::Choice { options, index } => {
                    values.raw_set(key.as_str(), options[index as usize].as_str())?;
                    Kind::Enum { options: kinds::intern_options(options), index }
                }
                SpecKind::Blocks { title, default } => {
                    let list = kinds::block_list(mod_id, &key, intern(title), &default);
                    values.raw_set(key.as_str(), kinds::block_set(lua, list)?)?;
                    Kind::List { list }
                }
            };
            let label = intern(spec.label.unwrap_or_else(|| key.clone()));
            let def = SettingDef { name: label, tip: intern(spec.tip), kind, show: Show::Always };
            defs.push((intern(key), def));
        }
        let keys: Vec<(&str, Kind)> = defs.iter().map(|(k, d)| (*k, d.kind)).collect();
        for (i, v) in visibles.iter().enumerate() {
            if let Some(v) = v {
                let at = format!("{what}: settings.{}", keys[i].0);
                defs[i].1.show = kinds::resolve(&at, v, i, &keys)?;
            }
        }
        let name = intern(name);

        let first = SETTING_COUNT + slots;
        let writable: Vec<(&'static str, usize, Kind)> =
            defs.iter().enumerate().map(|(i, (key, def))| (*key, first + i, def.kind)).collect();
        let settings_view = proxy(lua, &values)?;
        if let Some(mt) = settings_view.metatable() {
            mt.raw_set("__newindex", settings_setter(lua, &ctx, name, writable, &values)?)?;
        }
        let backing = lua.create_table()?;
        backing.raw_set("name", name)?;
        backing.raw_set("enabled", false)?;
        backing.raw_set("ticks", 0)?;
        backing.raw_set("settings", &settings_view)?;
        for (method, switch) in [("enable", Switch::On), ("disable", Switch::Off), ("toggle", Switch::Flip)] {
            let c = ctx.clone();
            backing.raw_set(
                method,
                lua.create_function(move |_, _: Variadic<Value>| {
                    if c.loading.load(Relaxed) {
                        return Err(Error::runtime(format!("m:{method}() can't be called while the mod loads")));
                    }
                    c.switches.lock().unwrap_or_else(PoisonError::into_inner).push((k, switch));
                    Ok(())
                })?,
            )?;
        }
        {
            let c = ctx.clone();
            backing.raw_set(
                "command",
                lua.create_function(move |_, (_m, spec): (Value, Value)| {
                    if !c.loading.load(Relaxed) {
                        return Err(Error::runtime(format!(
                            "{name}:command: commands are declared while the mod loads, not after"
                        )));
                    }
                    let (decl, run) = command_decl("m:command", &spec)?;
                    let mut load = c.load.lock().unwrap_or_else(PoisonError::into_inner);
                    let Some(def) = load.defs.get_mut(k) else {
                        return Err(Error::runtime(format!("{name}:command: the module is gone")));
                    };
                    if def.commands.len() >= COMMANDS_PER_MODULE {
                        return Err(Error::runtime(format!(
                            "{name}:command: at most {COMMANDS_PER_MODULE} commands per module"
                        )));
                    }
                    if def.commands.iter().any(|(d, _)| d.name == decl.name) {
                        return Err(Error::runtime(format!("{name}:command: `{}` is declared twice", decl.name)));
                    }
                    def.commands.push((decl, run));
                    Ok(())
                })?,
            )?;
        }

        let m = lua.create_table()?;
        let mt = lua.create_table()?;
        mt.raw_set("__index", &backing)?;
        mt.raw_set("__newindex", setter(lua, &ctx, k, name, &backing)?)?;
        mt.raw_set("__metatable", LOCKED)?;
        m.set_metatable(Some(mt))?;

        let mut load = ctx.load.lock().unwrap_or_else(PoisonError::into_inner);
        let mod_id = load.mod_id;
        load.defs.push(Def {
            mod_id,
            api: 0,
            name,
            desc: intern(desc),
            category,
            mode,
            settings: defs,
            m: m.clone(),
            backing,
            values,
            handlers: Default::default(),
            packets,
            panels,
            commands: Vec::new(),
        });
        Ok(m)
    })
}

fn settings_setter(
    lua: &Lua,
    ctx: &Arc<Ctx>,
    name: &'static str,
    writable: Vec<(&'static str, usize, Kind)>,
    values: &Table,
) -> mlua::Result<Function> {
    let ctx = ctx.clone();
    let values = values.clone();
    lua.create_function(move |lua, (_t, key, value): (Value, Value, Value)| {
        let Value::String(key) = key else {
            return Err(Error::runtime(format!(
                "{name}.settings: settings are named by strings"
            )));
        };
        let key = key.to_string_lossy();
        let Some(&(k, slot, kind)) = writable.iter().find(|(k, ..)| *k == key) else {
            let keys: Vec<&str> = writable.iter().map(|(k, ..)| *k).collect();
            return Err(Error::runtime(format!(
                "{name}.settings.{key}: no such setting{}",
                suggest(&key, &keys)
            )));
        };
        if ctx.loading.load(Relaxed) {
            return Err(Error::runtime(format!(
                "{name}.settings.{k} can be set once the mod has loaded, not while it loads"
            )));
        }
        let s = store();
        let number = match &value {
            Value::Integer(i) => Some(*i as f32),
            Value::Number(n) => Some(*n as f32),
            _ => None,
        };
        match (kind, &value, number) {
            (Kind::Toggle { .. }, Value::Boolean(b), _) => {
                s.set_value(slot, Setting::Bool(*b));
                values.raw_set(k, *b)?;
            }
            (Kind::Slider { .. }, _, Some(x)) if !x.is_finite() => {
                return Err(Error::runtime(format!(
                    "{name}.settings.{k} takes a finite number, not {x}"
                )));
            }
            (Kind::Slider { decimals, .. }, _, Some(x)) => {
                s.set_value(slot, Setting::Num(x));
                values.raw_set(k, shown(x, decimals))?;
            }
            (Kind::Key { .. }, Value::String(t), _) => {
                let b = bound(&t.to_string_lossy())
                    .map_err(|e| Error::runtime(format!("{name}.settings.{k}: {e}")))?;
                s.set_value(slot, Setting::Key(b));
                values.raw_set(k, b.serialized_name())?;
            }
            (Kind::Text { max_len, .. }, Value::String(t), _) => {
                let t = t.to_string_lossy();
                let len = t.chars().count();
                if len > max_len {
                    return Err(Error::runtime(format!(
                        "{name}.settings.{k} is at most {max_len} characters; this is {len}"
                    )));
                }
                s.set_text_at(slot, &t);
                values.raw_set(k, t)?;
            }
            (Kind::Button { .. }, ..) => {
                return Err(Error::runtime(format!(
                    "{name}.settings.{k} is a button, which holds no value; a press is onbutton"
                )));
            }
            (Kind::Enum { options, .. }, Value::String(t), _) => {
                let t = t.to_string_lossy();
                let Some(c) = options.iter().position(|o| *o == t) else {
                    return Err(Error::runtime(format!(
                        "{name}.settings.{k}: `{t}` is not one of {}",
                        options.join(", ")
                    )));
                };
                s.set_value(slot, Setting::Choice(c as u8));
                values.raw_set(k, options[c])?;
            }
            (Kind::List { list }, Value::Table(t), _) => {
                let ids = kinds::block_ids(&format!("{name}.settings.{k}"), t)?;
                list.set_only(|label| ids.binary_search_by(|d| d.as_str().cmp(label)).is_ok());
                values.raw_set(k, kinds::block_set(lua, list)?)?;
            }
            (kind, ..) => {
                let wants = match kind {
                    Kind::Toggle { .. } => "a boolean",
                    Kind::Slider { .. } => "a number",
                    Kind::Key { .. } => "a key name string",
                    Kind::List { .. } => "a list of block ids",
                    Kind::Enum { .. } => "one of its options",
                    _ => "a string",
                };
                return Err(Error::runtime(format!(
                    "{name}.settings.{k} takes {wants}, not a {}",
                    value.type_name()
                )));
            }
        }
        Ok(())
    })
}

fn setter(
    lua: &Lua,
    ctx: &Arc<Ctx>,
    k: usize,
    name: &'static str,
    backing: &Table,
) -> mlua::Result<Function> {
    let ctx = ctx.clone();
    let backing = backing.clone();
    lua.create_function(move |_, (_m, key, value): (Value, Value, Value)| {
        let Value::String(key) = key else {
            return Err(Error::runtime(format!(
                "{name}: a module's fields are named by strings"
            )));
        };
        let key = key.to_string_lossy();
        let Some(ev) = key
            .strip_prefix("on")
            .and_then(|t| EVENTS.iter().position(|e| *e == t))
        else {
            if M_PROPS.contains(&key.as_str()) {
                return Err(Error::runtime(format!("{name}.{key} is read-only")));
            }
            if key.starts_with("on") {
                let handlers: Vec<String> = EVENTS.iter().map(|e| format!("on{e}")).collect();
                let handlers: Vec<&str> = handlers.iter().map(String::as_str).collect();
                let near = suggest(&key, &handlers);
                if !near.is_empty() {
                    return Err(Error::runtime(format!(
                        "{name}.{key}: not an event handler{near}"
                    )));
                }
            }
            backing.raw_set(key.as_str(), value)?;
            return Ok(());
        };
        if !ctx.loading.load(Relaxed) {
            return Err(Error::runtime(format!(
                "{name}.{key}: handlers are set while the mod loads, not after"
            )));
        }
        let handler = match &value {
            Value::Function(f) => Some(f.clone()),
            Value::Nil => None,
            _ => return Err(Error::runtime(format!("{name}.{key} must be a function"))),
        };
        backing.raw_set(key.as_str(), value)?;
        if let Some(def) = ctx
            .load
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .defs
            .get_mut(k)
        {
            def.handlers[ev] = handler;
        }
        Ok(())
    })
}

pub(super) fn suggest(word: &str, options: &[&str]) -> String {
    options
        .iter()
        .map(|o| (distance(word, o), o))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map_or_else(String::new, |(_, o)| format!(" (did you mean `{o}`?)"))
}

pub(super) fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = (prev + (ca != *cb) as usize).min(row[j] + 1).min(here + 1);
            prev = here;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_indices_name_their_events() {
        for (ix, name) in [
            (ENABLE, "enable"),
            (DISABLE, "disable"),
            (TICK, "tick"),
            (CHAT, "chat"),
            (CHATSEND, "chatsend"),
            (JOIN, "join"),
            (SPAWN, "spawn"),
            (DEATH, "death"),
            (KEYDOWN, "keydown"),
            (ATTACK, "attack"),
            (PACKET, "packet"),
            (RENDER2D, "render2d"),
            (UICLICK, "uiclick"),
            (UICLOSE, "uiclose"),
            (PROFILE, "profile"),
            (JOB, "job"),
            (KEYUP, "keyup"),
            (SCROLL, "scroll"),
            (USE, "use"),
            (UIPROMPT, "uiprompt"),
            (BUTTON, "button"),
            (FILEREQUEST, "filerequest"),
            (UIKEY, "uikey"),
            (CHUNK, "chunk"),
            (BLOCKCHANGE, "blockchange"),
            (LEAVE, "leave"),
        ] {
            assert_eq!(EVENTS[ix], name);
        }
        assert!(EVENTS.len() <= 32, "game::LISTEN has one bit per event");
    }

    #[test]
    fn a_near_miss_is_suggested() {
        assert_eq!(suggest("nmae", &MODULE_FIELDS), " (did you mean `name`?)");
        let handlers: Vec<String> = EVENTS.iter().map(|e| format!("on{e}")).collect();
        let handlers: Vec<&str> = handlers.iter().map(String::as_str).collect();
        assert_eq!(suggest("ontik", &handlers), " (did you mean `ontick`?)");
        assert_eq!(
            suggest("online", &handlers),
            "",
            "a field of the mod's own, not a typo"
        );
        assert_eq!(suggest("zzzzzz", &MODULE_FIELDS), "");
    }

    #[test]
    fn events_keep_their_shape() {
        let api = Api::new(Arc::new(Ctx::new())).unwrap();
        let (e, backing) = api.cancelable_event(EVENTS[CHAT], false).unwrap();
        backing.raw_set("text", "hi").unwrap();
        let plain = api.event_table().unwrap();
        plain.raw_set("cx", 3).unwrap();
        api.seal(&plain, CHUNK, Class::Plain).unwrap();
        let check: Function = (api.lua)
            .load(
                r#"
                local e, plain = ...
                assert(e.cancelable and not e.defaultPrevented and e.text == "hi" and e.type == "chat")
                e:preventDefault()
                assert(e.defaultPrevented)
                assert(not pcall(rawset, e, "text", "no"), "rawset reached an event")
                assert(plain.type == "chunk" and plain.cx == 3 and not plain.cancelable)
                local n = 0
                for k in pairs(plain) do n += 1 end
                assert(n == 1, "pairs lists the fields only")
                assert(not pcall(rawset, plain, "cx", 4), "rawset reached an event")
                return true
                "#,
            )
            .into_function()
            .unwrap();
        assert!(check.call::<bool>((&e, &plain)).unwrap());
        assert!(backing.raw_get::<bool>("defaultPrevented").unwrap());
    }
}
