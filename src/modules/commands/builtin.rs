use super::args::{Tok, rest};
use super::table::{Owner, Table};
use crate::gui::keybinds::Bound;
use crate::lang::{Key, tr, trf};
use crate::modules::chat_math::{GREEN, RED};
use crate::modules::registry::{self, Category, Kind, Mode, ModuleDef, SettingDef};
use crate::modules::{Value, store};
use crate::text::{AQUA, GRAY, Span, WHITE, YELLOW};

pub struct Builtin {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub usage: Key,
    pub tip: Key,
    pub tip_names: fn() -> Vec<&'static str>,
    pub shape: Shape,
    pub run: fn(&mut Call),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Help,
    Module,
    Set,
    Get,
    Bind,
    Profile,
    Word,
    Rest,
    Category,
    Goto,
    Follow,
    Mine,
    Choice(&'static [&'static str]),
    Friend,
    Nbt,
    #[cfg(feature = "seed_cracker")]
    Cracker,
}

impl Shape {
    pub fn raw_args(self) -> bool {
        self == Shape::Nbt
    }
}

#[derive(Default)]
pub struct Line(pub Vec<Span>);

impl Line {
    fn put(mut self, s: impl Into<String>, rgb: u32) -> Line {
        self.0.push(Span::colored(s, rgb));
        self
    }

    pub fn t(self, s: impl Into<String>) -> Line {
        self.put(s, WHITE)
    }

    fn name(self, s: impl Into<String>) -> Line {
        self.put(s, AQUA)
    }

    fn value(self, s: impl Into<String>) -> Line {
        self.put(s, YELLOW)
    }

    fn dim(self, s: impl Into<String>) -> Line {
        self.put(s, GRAY)
    }

    pub fn error(s: impl Into<String>) -> Line {
        Line::default().put(s, RED)
    }

    fn fill(mut self, template: &str, pieces: &[(&str, u32)]) -> Line {
        const MARK: char = '\u{1}';
        let marks: Vec<String> = (0..pieces.len())
            .map(|i| format!("{MARK}{i}{MARK}"))
            .collect();
        let filled = crate::util::placeholders::substitute(template, &marks);
        for (i, part) in filled.split(MARK).enumerate() {
            if i % 2 == 0 {
                if !part.is_empty() {
                    self = self.t(part);
                }
            } else if let Some(&(text, rgb)) =
                part.parse::<usize>().ok().and_then(|n| pieces.get(n))
            {
                self = self.put(text, rgb);
            }
        }
        self
    }
}

pub struct Call<'a> {
    pub raw: &'a str,
    pub toks: Vec<Tok>,
    pub table: &'a Table,
    pub prefix: &'a str,
    pub out: Vec<Line>,
    pub send: Option<String>,
    pub edit: Option<crate::play::item_components::ItemEdit>,
    pub shared: &'a crate::session::SharedMutex,
}

impl Call<'_> {
    fn line(&mut self, l: Line) {
        self.out.push(l);
    }

    fn ok(&mut self, s: impl Into<String>) {
        self.out.push(Line::default().t(s));
    }

    fn err(&mut self, s: impl Into<String>) {
        self.out.push(Line::error(s));
    }

    fn arg(&self, i: usize) -> Option<&str> {
        self.toks.get(i).map(|t| t.text.as_str())
    }
}

const DEV_BUILTINS: usize = 0;

pub static BUILTINS: [Builtin; 17 + cfg!(feature = "seed_cracker") as usize + DEV_BUILTINS] = [
    Builtin {
        name: "help",
        aliases: &[],
        usage: Key::of("commands.help.usage"),
        tip: Key::of("commands.help.tip"),
        tip_names: no_names,
        shape: Shape::Help,
        run: help,
    },
    Builtin {
        name: "toggle",
        aliases: &["t"],
        usage: Key::of("commands.toggle.usage"),
        tip: Key::of("commands.toggle.tip"),
        tip_names: no_names,
        shape: Shape::Module,
        run: toggle,
    },
    Builtin {
        name: "set",
        aliases: &[],
        usage: Key::of("commands.set.usage"),
        tip: Key::of("commands.set.tip"),
        tip_names: no_names,
        shape: Shape::Set,
        run: set,
    },
    Builtin {
        name: "get",
        aliases: &[],
        usage: Key::of("commands.get.usage"),
        tip: Key::of("commands.get.tip"),
        tip_names: no_names,
        shape: Shape::Get,
        run: get,
    },
    Builtin {
        name: "bind",
        aliases: &[],
        usage: Key::of("commands.bind.usage"),
        tip: Key::of("commands.bind.tip"),
        tip_names: no_names,
        shape: Shape::Bind,
        run: bind,
    },
    Builtin {
        name: "profile",
        aliases: &[],
        usage: Key::of("commands.profile.usage"),
        tip: Key::of("commands.profile.tip"),
        tip_names: no_names,
        shape: Shape::Profile,
        run: profile,
    },
    Builtin {
        name: "prefix",
        aliases: &[],
        usage: Key::of("commands.prefix.usage"),
        tip: Key::of("commands.prefix.tip"),
        tip_names: no_names,
        shape: Shape::Word,
        run: prefix,
    },
    Builtin {
        name: "seedcheck",
        aliases: &[],
        usage: Key::NONE,
        tip: Key::of("commands.seedcheck.tip"),
        tip_names: seed_xray_name,
        shape: Shape::Word,
        run: seedcheck,
    },
    #[cfg(feature = "seed_cracker")]
    Builtin {
        name: "cracker",
        aliases: &[],
        usage: Key::of("commands.cracker.usage"),
        tip: Key::of("commands.cracker.tip"),
        tip_names: seed_cracker_name,
        shape: Shape::Cracker,
        run: cracker,
    },
    Builtin {
        name: "say",
        aliases: &[],
        usage: Key::of("commands.say.usage"),
        tip: Key::of("commands.say.tip"),
        tip_names: no_names,
        shape: Shape::Rest,
        run: say,
    },
    Builtin {
        name: "modules",
        aliases: &[],
        usage: Key::of("commands.modules.usage"),
        tip: Key::of("commands.modules.tip"),
        tip_names: no_names,
        shape: Shape::Category,
        run: modules,
    },
    Builtin {
        name: "goto",
        aliases: &[],
        usage: Key::of("commands.goto.usage"),
        tip: Key::of("commands.goto.tip"),
        tip_names: no_names,
        shape: Shape::Goto,
        run: goto,
    },
    Builtin {
        name: "mine",
        aliases: &[],
        usage: Key::of("commands.mine.usage"),
        tip: Key::of("commands.mine.tip"),
        tip_names: ore_miner_and_ores,
        shape: Shape::Mine,
        run: mine,
    },
    Builtin {
        name: "stop",
        aliases: &[],
        usage: Key::NONE,
        tip: Key::of("commands.stop.tip"),
        tip_names: ore_miner_name,
        shape: Shape::Choice(&[]),
        run: stop,
    },
    Builtin {
        name: "follow",
        aliases: &[],
        usage: Key::of("commands.follow.usage"),
        tip: Key::of("commands.follow.tip"),
        tip_names: no_names,
        shape: Shape::Follow,
        run: follow,
    },
    Builtin {
        name: "friend",
        aliases: &[],
        usage: Key::of("commands.friend.usage"),
        tip: Key::of("commands.friend.tip"),
        tip_names: no_names,
        shape: Shape::Friend,
        run: friend,
    },
    Builtin {
        name: "nbt",
        aliases: &[],
        usage: Key::of("commands.nbt.usage"),
        tip: Key::of("commands.nbt.tip"),
        tip_names: no_names,
        shape: Shape::Nbt,
        run: nbt,
    },
    Builtin {
        name: "server",
        aliases: &["serverinfo"],
        usage: Key::NONE,
        tip: Key::of("commands.server.tip"),
        tip_names: no_names,
        shape: Shape::Choice(&[]),
        run: server,
    },
];

fn no_names() -> Vec<&'static str> {
    Vec::new()
}

fn seed_xray_name() -> Vec<&'static str> {
    vec![registry::module(registry::Id::SeedXray).label()]
}

#[cfg(feature = "seed_cracker")]
fn seed_cracker_name() -> Vec<&'static str> {
    vec![registry::module(registry::Id::SeedCracker).label()]
}

fn ore_miner_name() -> Vec<&'static str> {
    vec![registry::module(registry::Id::OreMiner).label()]
}

fn ore_miner_and_ores() -> Vec<&'static str> {
    const ORES: Key = Key::of("modules.ore_miner.ORES.name");
    vec![registry::module(registry::Id::OreMiner).label(), ORES.get()]
}

pub fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn pick<T: Copy, R: Iterator<Item = T>, E: IntoIterator<Item = &'static str>>(
    rows: impl Fn() -> R,
    want: &str,
    english: impl Fn(T) -> E,
    drawn: impl Fn(T) -> &'static str,
) -> Option<T> {
    rows()
        .find(|&r| english(r).into_iter().any(|n| norm(n) == want))
        .or_else(|| rows().find(|&r| norm(drawn(r)) == want))
}

pub(super) fn rescued<T>(typed: &str, find: impl Fn(&str) -> Option<T>) -> Option<T> {
    let found = find(typed);
    #[cfg(feature = "multi_language")]
    let found = found.or_else(|| remapped(typed, &find));
    found
}

#[cfg(any(feature = "multi_language", test))]
#[cfg_attr(not(feature = "multi_language"), allow(dead_code))]
pub(super) fn remapped<T>(typed: &str, find: impl Fn(&str) -> Option<T>) -> Option<T> {
    let keys = crate::util::keyboard_layout::to_qwerty(typed);
    if keys == typed { None } else { find(&keys) }
}

fn find_module(name: &str) -> Result<&'static ModuleDef, String> {
    rescued(name, |typed| {
        let aliases =
            |m: &'static ModuleDef| std::iter::once(m.name).chain(m.aliases().iter().copied());
        pick(registry::modules, &norm(typed), aliases, |m| m.label())
    })
    .ok_or_else(|| trf!("commands.no_module", name))
}

fn find_setting(m: &'static ModuleDef, name: &str) -> Result<(usize, &'static SettingDef), String> {
    rescued(name, |typed| {
        let rows = || m.slots().zip(m.settings());
        pick(rows, &norm(typed), |(_, d)| [d.name], |(_, d)| d.label())
    })
    .ok_or_else(|| {
        let names: Vec<String> = m.settings().iter().map(|d| norm(d.label())).collect();
        if names.is_empty() {
            trf!("commands.no_settings", m.label())
        } else {
            trf!("commands.no_setting", m.label(), name, names.join(", "))
        }
    })
}

fn find_option(
    options: &'static [&'static str],
    keys: &'static [Key],
    value: &str,
) -> Option<usize> {
    rescued(value, |typed| {
        pick(
            || 0..options.len(),
            &norm(typed),
            |i| [options[i]],
            |i| registry::option_label(options, keys, i),
        )
    })
}

fn find_category(name: &str) -> Option<Category> {
    rescued(name, |typed| {
        pick(
            || Category::ALL.into_iter(),
            &norm(typed),
            |c| [c.name()],
            |c| c.label(),
        )
    })
}

fn find_mode(name: &str) -> Option<Mode> {
    rescued(name, |typed| {
        pick(
            || Mode::ALL.into_iter(),
            &norm(typed),
            |m| [Mode::NAMES[m as usize]],
            |m| m.label(),
        )
    })
}

fn shown(slot: usize, def: &SettingDef) -> Option<String> {
    let s = store();
    Some(match (def.kind, s.value(slot)) {
        (Kind::Text { .. }, _) => format!("\"{}\"", s.text_at(slot)),
        (Kind::List { .. }, _) => tr!("commands.value.list").to_string(),
        (Kind::Button { .. }, _) => return None,
        (_, Some(Value::Bool(b))) => on_off(b).to_string(),
        (Kind::Slider { decimals, .. }, Some(Value::Num(x))) => {
            format!("{:.*}", decimals as usize, x)
        }
        (Kind::Range { decimals, .. }, Some(Value::Range(lo, hi))) => {
            let d = decimals as usize;
            format!("{:.*}..{:.*}", d, lo, d, hi)
        }
        (Kind::Enum { options, keys, .. }, Some(Value::Choice(i))) => {
            match registry::option_label(options, keys, i as usize) {
                "" => "?".to_string(),
                drawn => drawn.to_string(),
            }
        }
        (_, Some(Value::Key(b))) => b.display_name().to_string(),
        _ => "?".to_string(),
    })
}

fn on_off(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

fn bool_word(w: &str) -> Option<bool> {
    match w.to_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Some(true),
        "off" | "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn number(w: &str) -> Result<f32, String> {
    w.parse::<f32>()
        .ok()
        .filter(|x| x.is_finite())
        .ok_or_else(|| trf!("commands.not_number", w))
}

fn state_line(m: &ModuleDef) -> Line {
    let s = store();
    let armed = s.armed(m.id);
    let state = (on_off(armed), if armed { GREEN } else { RED });
    let mut line = Line::default().fill(tr!("commands.state.is"), &[(m.label(), AQUA), state]);
    if armed && !s.allowed(m.id) {
        let mode = m.mode.label();
        let now = s.profile().label();
        line = line.dim(trf!("commands.state.held_back", mode, now));
    }
    line
}

fn usage_line(p: &str, shown: &str, usage: &str) -> Line {
    let line = Line::default().value(format!("{p}{shown}"));
    if usage.is_empty() {
        line
    } else {
        line.dim(format!(" {usage}"))
    }
}

fn help(c: &mut Call) {
    let p = c.prefix.to_string();
    let table = c.table;
    let Some(name) = c.arg(0).map(str::to_lowercase) else {
        let arg = format!("{p}help {}", tr!("commands.help.head_arg"));
        let head = Line::default().fill(tr!("commands.help.head"), &[(arg.as_str(), YELLOW)]);
        c.line(head);
        for cmd in &table.commands {
            let mut line = usage_line(&p, &cmd.shown(), &cmd.usage()).t(format!(": {}", cmd.tip()));
            if let Owner::Script(s) = &cmd.owner {
                line = line.dim(format!(" [{}]", s.mod_id));
            }
            c.line(line);
        }
        return;
    };
    let Some(cmd) = rescued(&name, |n| table.find(n)) else {
        c.err(trf!("commands.help.unknown", name));
        return;
    };
    c.line(usage_line(&p, &cmd.shown(), &cmd.usage()));
    let tip = cmd.tip();
    if !tip.is_empty() {
        c.ok(tip);
    }
    let others: Vec<String> = cmd
        .names
        .iter()
        .skip(1)
        .map(|n| format!("{p}{n}"))
        .collect();
    if !others.is_empty() {
        let others = others.join(", ");
        c.line(Line::default().fill(tr!("commands.help.also"), &[(others.as_str(), YELLOW)]));
    }
    if let Some(q) = &cmd.qualified {
        let q = format!("{p}{q}");
        c.line(Line::default().fill(tr!("commands.help.qualified"), &[(q.as_str(), YELLOW)]));
    }
}

fn toggle(c: &mut Call) {
    let Some(name) = c.arg(0) else {
        c.err(tr!("commands.toggle.which"));
        return;
    };
    match find_module(name) {
        Ok(m) => {
            store().toggle(m.id);
            c.line(state_line(m));
        }
        Err(e) => c.err(e),
    }
}

fn set(c: &mut Call) {
    let (Some(module), Some(setting)) = (c.arg(0), c.arg(1)) else {
        c.err(tr!("commands.set.usage_line"));
        return;
    };
    let m = match find_module(module) {
        Ok(m) => m,
        Err(e) => return c.err(e),
    };
    let (slot, def) = match find_setting(m, setting) {
        Ok(s) => s,
        Err(e) => return c.err(e),
    };
    let value = rest(c.raw, &c.toks, 2);
    if value.is_empty() && !matches!(def.kind, Kind::Text { .. }) {
        return c.err(trf!(
            "commands.set.usage_value",
            norm(m.label()),
            norm(def.label())
        ));
    }
    let label = def.label();
    let s = store();
    let word = c.arg(2).unwrap_or_default().to_string();
    let result: Result<(), String> = match def.kind {
        Kind::Toggle { .. } => bool_word(&word)
            .map(|b| s.set_value(slot, Value::Bool(b)))
            .ok_or_else(|| trf!("commands.set.toggle", label, word)),
        Kind::Slider { min, max, .. } => {
            let max = crate::modules::registry::typed_max(slot as usize, max);
            number(&word).map(|x| s.set_value(slot, Value::Num(x.clamp(min, max))))
        }
        Kind::Range { min, max, .. } => {
            let pair = match word.split_once("..") {
                Some((a, b)) => Some((a.to_string(), b.to_string())),
                None => c.arg(3).map(|b| (word.clone(), b.to_string())),
            };
            match pair {
                None => Err(trf!("commands.set.range", label)),
                Some((a, b)) => {
                    number(&a)
                        .and_then(|a| number(&b).map(|b| (a, b)))
                        .map(|(a, b)| {
                            let (lo, hi) = (a.min(b).clamp(min, max), a.max(b).clamp(min, max));
                            s.set_value(slot, Value::Range(lo, hi));
                        })
                }
            }
        }
        Kind::Enum { options, keys, .. } => match find_option(options, keys, &value) {
            Some(i) => {
                s.set_value(slot, Value::Choice(i as u8));
                Ok(())
            }
            None => {
                let drawn =
                    (0..options.len()).map(|i| norm(registry::option_label(options, keys, i)));
                let names = drawn.collect::<Vec<_>>().join(", ");
                Err(trf!("commands.set.enum", label, names))
            }
        },
        Kind::Text { max_len, .. } => {
            if value.chars().count() > max_len {
                Err(trf!("commands.set.text_long", label, max_len))
            } else {
                s.set_text_at(slot, &value);
                Ok(())
            }
        }
        Kind::Key { .. } => match word.to_lowercase().as_str() {
            "none" | "unbound" => {
                s.set_value(slot, Value::Key(Bound::Unbound));
                Ok(())
            }
            _ => Bound::from_typed_name(&word)
                .map(|b| s.set_value(slot, Value::Key(b)))
                .ok_or_else(|| trf!("commands.set.not_key", word)),
        },
        Kind::List { .. } => Err(trf!("commands.set.list", label)),
        Kind::Button { .. } => Err(trf!("commands.set.button", label)),
    };
    match result {
        Ok(()) => {
            let now = shown(slot, def).unwrap_or_default();
            c.line(setting_line(m.label(), label, now));
        }
        Err(e) => c.err(e),
    }
}

fn setting_line(module: &str, setting: &str, value: String) -> Line {
    Line::default()
        .name(module)
        .t(" ")
        .name(setting)
        .t(" = ")
        .value(value)
}

fn get(c: &mut Call) {
    let Some(module) = c.arg(0) else {
        c.err(tr!("commands.get.usage_line"));
        return;
    };
    let m = match find_module(module) {
        Ok(m) => m,
        Err(e) => return c.err(e),
    };
    if let Some(setting) = c.arg(1) {
        match find_setting(m, setting) {
            Ok((slot, def)) => {
                let now =
                    shown(slot, def).unwrap_or_else(|| tr!("commands.value.button").to_string());
                c.line(setting_line(m.label(), def.label(), now));
            }
            Err(e) => c.err(e),
        }
        return;
    }
    let bind = store().bind(m.id);
    c.line(state_line(m).fill(tr!("commands.get.bound"), &[(bind.display_name(), YELLOW)]));
    for (slot, def) in m.slots().zip(m.settings()) {
        if let Some(now) = shown(slot, def) {
            c.line(
                Line::default()
                    .t("  ")
                    .name(def.label())
                    .t(" = ")
                    .value(now),
            );
        }
    }
}

fn bind(c: &mut Call) {
    let (Some(module), Some(key)) = (c.arg(0), c.arg(1)) else {
        c.err(tr!("commands.bind.usage_line"));
        return;
    };
    let m = match find_module(module) {
        Ok(m) => m,
        Err(e) => return c.err(e),
    };
    let b = match key.to_lowercase().as_str() {
        "none" | "unbound" => Bound::Unbound,
        _ => match Bound::from_typed_name(key) {
            Some(b) => b,
            None => return c.err(trf!("commands.bind.not_key", key)),
        },
    };
    store().set_bind(m.id, b);
    let pieces = [(m.label(), AQUA), (b.display_name(), YELLOW)];
    c.line(Line::default().fill(tr!("commands.bind.done"), &pieces));
}

fn profile_line(name: &str) -> Line {
    Line::default().fill(tr!("commands.profile.in_force"), &[(name, YELLOW)])
}

fn profile(c: &mut Call) {
    let s = store();
    let Some(name) = c.arg(0) else {
        c.line(profile_line(s.profile().label()));
        return;
    };
    match find_mode(name) {
        Some(mode) => {
            s.set_profile(mode);
            c.line(profile_line(mode.label()));
        }
        None => c.err(trf!("commands.profile.unknown", name)),
    }
}

fn prefix(c: &mut Call) {
    let Some(new) = c.arg(0).map(str::to_string) else {
        let p = c.prefix.to_string();
        c.line(Line::default().fill(tr!("commands.prefix.is"), &[(p.as_str(), YELLOW)]));
        return;
    };
    if let Err(e) = super::check_prefix(&new) {
        return c.err(e);
    }
    store().set_text_at(registry::commands::PREFIX.0 as usize, &new);
    c.line(Line::default().fill(tr!("commands.prefix.now"), &[(new.as_str(), YELLOW)]));
}

fn seedcheck(c: &mut Call) {
    if store().enabled(registry::Id::SeedXray) {
        crate::modules::seed_xray::request_check();
    } else {
        c.err(trf!(
            "commands.module_off",
            registry::module(registry::Id::SeedXray).label()
        ));
    }
}

#[cfg(feature = "seed_cracker")]
fn cracker(c: &mut Call) {
    use crate::modules::seed_cracker::Request;
    let word = c.arg(0).unwrap_or("status").to_string();
    if word == "help" {
        crate::modules::seed_cracker::open_help();
        return;
    }
    let side = c.arg(1).map(str::to_string);
    let request = match (word.as_str(), side.as_deref()) {
        ("status", _) => Request::Status,
        ("clear", _) => Request::Clear,
        ("restart", _) => Request::Restart,
        ("trust", Some("bedrock")) => Request::TrustBedrock,
        ("trust", Some("structures")) => Request::TrustStructures,
        ("export", Some("bedrock")) => Request::ExportBedrock,
        _ => {
            let typed = c.raw.trim().to_string();
            c.err(trf!("commands.cracker.unknown", typed));
            return;
        }
    };
    if store().enabled(registry::Id::SeedCracker) {
        crate::modules::seed_cracker::request(request);
    } else {
        c.err(trf!(
            "commands.module_off",
            registry::module(registry::Id::SeedCracker).label()
        ));
    }
}

fn say(c: &mut Call) {
    let text = c.raw.trim().to_string();
    if text.is_empty() {
        c.err(tr!("commands.say.usage_line"));
        return;
    }
    c.send = Some(text);
}

fn goto(c: &mut Call) {
    use crate::modules::pathfinder::{Coord, Request, request};
    if c.arg(0) == Some("stop") && c.toks.len() == 1 {
        request(Request::Stop);
        return;
    }
    if c.arg(0) == Some("freecam") && c.toks.len() == 1 {
        match crate::modules::freecam::eye() {
            Some(eye) => request(Request::Goto(eye.map(|v| Coord::Abs(v.floor() as i32)))),
            None => c.err(tr!("commands.goto.freecam_off")),
        }
        return;
    }
    if c.toks.len() != 3 {
        let usage = trf!("commands.goto.usage_line", c.prefix);
        c.err(usage);
        return;
    }
    let mut at = [Coord::Rel(0); 3];
    for (i, slot) in at.iter_mut().enumerate() {
        let text = c.arg(i).unwrap_or_default().to_string();
        match Coord::parse(&text) {
            Some(v) => *slot = v,
            None => {
                c.err(trf!("commands.goto.not_coord", text));
                return;
            }
        }
    }
    request(Request::Goto(at));
}

fn mine(c: &mut Call) {
    use crate::modules::ore_miner::{self, kind_name};
    use crate::modules::seed_xray::table::Kind;
    let stop = c.arg(0).is_some_and(is_stop);
    if c.toks.len() > 2 || (stop && c.toks.len() > 1) {
        let usage = trf!("commands.mine.usage_line", c.prefix);
        return c.err(usage);
    }
    let count = match c.arg(1) {
        None => None,
        Some(t) => match mine_count(t) {
            Some(n) => Some(n),
            None => return c.err(trf!("commands.mine.not_count", t)),
        },
    };
    match c.arg(0) {
        None => ore_miner::start(None, None),
        Some(_) if stop => store().set_enabled(registry::Id::OreMiner, false),
        Some(w) => match ore_miner::named(w) {
            Some(want) => ore_miner::start(Some(want), count),
            None => {
                let names: Vec<String> = Kind::ALL.into_iter().map(kind_name).collect();
                return c.err(trf!("commands.mine.not_block", w, names.join(", ")));
            }
        },
    }
    c.line(state_line(registry::module(registry::Id::OreMiner)));
}

fn stop(c: &mut Call) {
    if !c.toks.is_empty() {
        return c.err(trf!("commands.stop.usage_line", c.prefix));
    }
    store().set_enabled(registry::Id::OreMiner, false);
    c.line(state_line(registry::module(registry::Id::OreMiner)));
}

pub(super) fn mine_count(t: &str) -> Option<u32> {
    t.parse::<i32>().ok().filter(|n| *n >= 1).map(|n| n as u32)
}

fn is_stop(w: &str) -> bool {
    let bare = w
        .get(..10)
        .filter(|p| p.eq_ignore_ascii_case("minecraft:"))
        .map_or(w, |_| &w[10..]);
    bare.eq_ignore_ascii_case("stop")
}

fn follow(c: &mut Call) {
    use crate::modules::pathfinder::{Request, request};
    if c.toks.len() > 1 {
        let usage = trf!("commands.follow.usage_line", c.prefix);
        return c.err(usage);
    }
    request(Request::Follow(c.arg(0).map(str::to_string)));
}

fn friend(c: &mut Call) {
    use crate::modules::friends;
    let usage = trf!("commands.friend.usage_line", c.prefix);
    match (
        c.arg(0).map(str::to_ascii_lowercase).as_deref(),
        c.arg(1),
        c.toks.len(),
    ) {
        (None | Some("list"), None, _) => {
            let names = friends::list();
            if names.is_empty() {
                let hint = trf!("commands.friend.add_hint", c.prefix);
                return c.line(Line::default().t(tr!("commands.friend.none")).dim(hint));
            }
            let mut line = Line::default().t(trf!("commands.friend.list", names.len()));
            for (i, n) in names.into_iter().enumerate() {
                if i > 0 {
                    line = line.dim(", ");
                }
                line = line.name(n);
            }
            c.line(line);
        }
        (Some("add"), Some(name), 2) => {
            if !friends::valid_name(name) {
                let msg = trf!("commands.friend.invalid", name);
                return c.err(msg);
            }
            let name = name.to_string();
            let pieces = [(name.as_str(), AQUA)];
            if friends::add(&name) {
                c.line(Line::default().fill(tr!("commands.friend.added"), &pieces));
            } else {
                c.line(Line::default().fill(tr!("commands.friend.already"), &pieces));
            }
        }
        (Some("remove"), Some(name), 2) => match friends::remove(name) {
            Some(gone) => {
                c.line(
                    Line::default().fill(tr!("commands.friend.removed"), &[(gone.as_str(), AQUA)]),
                );
            }
            None => {
                let msg = trf!("commands.friend.not_friend", name);
                c.err(msg);
            }
        },
        _ => c.err(usage),
    }
}

fn modules(c: &mut Call) {
    let only = match c.arg(0) {
        None => None,
        Some(name) => match find_category(name) {
            Some(cat) => Some(cat),
            None => {
                c.err(tr!("commands.modules.categories"));
                return;
            }
        },
    };
    let s = store();
    for cat in Category::ALL {
        if only.is_some_and(|o| o != cat) {
            continue;
        }
        let mut line = Line::default().name(cat.label()).t(":");
        for (i, m) in cat.modules().enumerate() {
            line = line.t(if i == 0 { " " } else { ", " });
            line = if s.armed(m.id) {
                line.put(m.label(), GREEN)
            } else {
                line.t(m.label())
            };
        }
        c.line(line);
    }
}

fn nbt(c: &mut Call) {
    use crate::play::item_components::{COUNT_RANGE, ItemEdit, kind_named, parse_components};
    let usage = trf!("commands.nbt.usage_line", c.prefix);
    let raw = c.raw.trim();
    let (sub, rest) = raw
        .split_once(char::is_whitespace)
        .map_or((raw, ""), |(s, r)| (s, r.trim()));
    let edit = match (sub.to_lowercase().as_str(), rest.is_empty()) {
        ("get", true) => ItemEdit::Get,
        ("copy", true) => ItemEdit::Copy,
        ("add", false) => match parse_components(rest) {
            Ok(list) => ItemEdit::Add(list),
            Err(e) => return c.err(e),
        },
        ("set", false) => match parse_components(rest) {
            Ok(list) => ItemEdit::Set(list),
            Err(e) => return c.err(e),
        },
        ("remove", false) => match kind_named(rest) {
            Some(kind) => ItemEdit::Remove(kind),
            None => return c.err(trf!("commands.nbt.unknown_component", rest)),
        },
        ("count", false) => match rest.parse::<i32>() {
            Ok(n) if COUNT_RANGE.contains(&n) => ItemEdit::Count(n),
            _ => {
                return c.err(trf!(
                    "commands.nbt.count_range",
                    COUNT_RANGE.start(),
                    COUNT_RANGE.end()
                ));
            }
        },
        _ => return c.err(usage),
    };
    c.edit = Some(edit);
}

fn server(c: &mut Call) {
    let shared = c.shared;
    let (address, info, tick_rate, frozen, hardcore, op_level, players, links) = {
        let s = shared.lock().unwrap();
        if !s.in_world {
            return c.err(tr!("commands.server.offline"));
        }
        let ss = &s.session;
        (
            ss.current_address.clone(),
            ss.server_info.clone(),
            ss.tick_rate.0,
            ss.world_run.frozen,
            ss.hardcore,
            ss.op_level,
            ss.tab_list.rows.len(),
            ss.server_links.len(),
        )
    };
    let unsent = tr!("commands.server.unsent");
    let yes_no = |b: Option<bool>| match b {
        Some(true) => tr!("commands.server.yes").to_string(),
        Some(false) => tr!("commands.server.no").to_string(),
        None => unsent.to_string(),
    };
    let row = |label: &str, value: String| Line::default().t(format!("{label}: ")).value(value);

    c.line(Line::default().name(tr!("commands.server.title")));
    c.line(row(
        tr!("commands.server.address"),
        address.unwrap_or_else(|| unsent.to_string()),
    ));
    let protocol = crate::util::session_protocol();
    let version = match crate::util::session_version_name() {
        Some(name) => format!("{name} ({protocol})"),
        None => protocol.to_string(),
    };
    c.line(row(tr!("commands.server.version"), version));
    let mut brand = Line::default().t(format!("{}: ", tr!("commands.server.brand")));
    match &info.brand {
        Some(b) => brand.0.extend(crate::text::parse_formatted_from(
            b,
            crate::text::Style::colored(YELLOW),
        )),
        None => brand = brand.value(unsent),
    }
    c.line(brand);
    let max = info
        .max_players
        .map_or_else(|| unsent.to_string(), |m| m.to_string());
    c.line(row(
        tr!("commands.server.players"),
        format!("{players} / {max}"),
    ));
    let chunks = |d: Option<u32>| d.map_or_else(|| unsent.to_string(), |d| d.to_string());
    c.line(row(
        tr!("commands.server.view_distance"),
        chunks(info.view_distance),
    ));
    c.line(row(
        tr!("commands.server.simulation_distance"),
        chunks(info.simulation_distance),
    ));
    let difficulty = match info.difficulty_locked {
        None => unsent.to_string(),
        Some(locked) => {
            let key = match crate::client::tracking::DIFFICULTY
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                0 => "options.difficulty.peaceful",
                1 => "options.difficulty.easy",
                2 => "options.difficulty.normal",
                _ => "options.difficulty.hard",
            };
            let mut d = crate::gui::tooltip::translate(key, &[]);
            if hardcore {
                d = format!(
                    "{d}, {}",
                    crate::gui::tooltip::translate("selectWorld.gameMode.hardcore", &[])
                );
            }
            if locked {
                d = format!("{d}, {}", tr!("commands.server.locked"));
            }
            d
        }
    };
    c.line(row(tr!("commands.server.difficulty"), difficulty));
    let mut rate = format!("{tick_rate}");
    if frozen {
        rate = format!("{rate}, {}", tr!("commands.server.frozen"));
    }
    c.line(row(tr!("commands.server.tick_rate"), rate));
    c.line(row(
        tr!("commands.server.online_mode"),
        yes_no(info.online_mode),
    ));
    c.line(row(
        tr!("commands.server.secure_chat"),
        yes_no(info.enforces_secure_chat),
    ));
    c.line(row(
        tr!("commands.server.reduced_debug"),
        yes_no(info.reduced_debug_info),
    ));
    c.line(row(tr!("commands.server.op_level"), op_level.to_string()));
    let mut dimension = info.dimension.unwrap_or_else(|| unsent.to_string());
    if info.flat == Some(true) {
        dimension = format!("{dimension}, {}", tr!("commands.server.flat"));
    }
    c.line(row(tr!("commands.server.dimension"), dimension));
    let seed = info
        .hashed_seed
        .map_or_else(|| unsent.to_string(), |h| h.to_string());
    c.line(row(tr!("commands.server.seed_hash"), seed));
    c.line(row(tr!("commands.server.links"), links.to_string()));
}
