use super::args::{Tok, rest};
use super::table::{Owner, Table};
use crate::gui::keybinds::Bound;
use crate::modules::chat_math::{GREEN, RED};
use crate::modules::registry::{self, Category, Kind, Mode, ModuleDef, SettingDef};
use crate::modules::{Value, store};
use crate::text::Span;

pub struct Builtin {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub usage: &'static str,
    pub tip: &'static str,
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
}

const WHITE: u32 = 0xFFFFFF;
const GRAY: u32 = 0xAAAAAA;
const YELLOW: u32 = 0xFFFF55;
const AQUA: u32 = 0x55FFFF;

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

    fn state(self, on: bool) -> Line {
        self.put(on_off(on), if on { GREEN } else { RED })
    }

    pub fn error(s: impl Into<String>) -> Line {
        Line::default().put(s, RED)
    }
}

pub struct Call<'a> {
    pub raw: &'a str,
    pub toks: Vec<Tok>,
    pub table: &'a Table,
    pub prefix: &'a str,
    pub out: Vec<Line>,
    pub send: Option<String>,
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

pub static BUILTINS: [Builtin; 11] = [
    Builtin {
        name: "help",
        aliases: &[],
        usage: "[command]",
        tip: "Lists every command, or shows how to use one",
        shape: Shape::Help,
        run: help,
    },
    Builtin {
        name: "toggle",
        aliases: &["t"],
        usage: "<module>",
        tip: "Switches a module on or off",
        shape: Shape::Module,
        run: toggle,
    },
    Builtin {
        name: "set",
        aliases: &[],
        usage: "<module> <setting> <value>",
        tip: "Changes one of a module's settings",
        shape: Shape::Set,
        run: set,
    },
    Builtin {
        name: "get",
        aliases: &[],
        usage: "<module> [setting]",
        tip: "Shows a module's settings, or one of them",
        shape: Shape::Get,
        run: get,
    },
    Builtin {
        name: "bind",
        aliases: &[],
        usage: "<module> <key|none>",
        tip: "Binds a module's switch to a key or mouse button",
        shape: Shape::Bind,
        run: bind,
    },
    Builtin {
        name: "profile",
        aliases: &[],
        usage: "[legit|normal|blatant]",
        tip: "Shows or changes which modules may run",
        shape: Shape::Profile,
        run: profile,
    },
    Builtin {
        name: "prefix",
        aliases: &[],
        usage: "[text]",
        tip: "Shows or changes what a command line starts with",
        shape: Shape::Word,
        run: prefix,
    },
    Builtin {
        name: "say",
        aliases: &[],
        usage: "<text...>",
        tip: "Sends the text as chat, for a line that has to start with the prefix",
        shape: Shape::Rest,
        run: say,
    },
    Builtin {
        name: "modules",
        aliases: &[],
        usage: "[category]",
        tip: "Lists the modules, the ones switched on marked",
        shape: Shape::Category,
        run: modules,
    },
    Builtin {
        name: "goto",
        aliases: &[],
        usage: "<x> <y> <z>|freecam|stop",
        tip: "Walks to a block, or to where the freecam is, drawing the route; ~ is relative to your feet",
        shape: Shape::Goto,
        run: goto,
    },
    Builtin {
        name: "follow",
        aliases: &[],
        usage: "[player]",
        tip: "Walks after a player in sight until they leave it; on its own, stops",
        shape: Shape::Follow,
        run: follow,
    },
];

pub fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn find_module(name: &str) -> Result<&'static ModuleDef, String> {
    let want = norm(name);
    registry::modules()
        .find(|m| norm(m.name) == want)
        .ok_or_else(|| format!("No module named `{name}`"))
}

fn find_setting(m: &ModuleDef, name: &str) -> Result<(usize, &'static SettingDef), String> {
    let want = norm(name);
    m.slots()
        .zip(m.settings())
        .find(|(_, d)| norm(d.name) == want)
        .ok_or_else(|| {
            let names: Vec<String> = m.settings().iter().map(|d| norm(d.name)).collect();
            if names.is_empty() {
                format!("{} has no settings", m.name)
            } else {
                format!(
                    "{} has no setting `{name}`; it has {}",
                    m.name,
                    names.join(", ")
                )
            }
        })
}

fn shown(slot: usize, def: &SettingDef) -> Option<String> {
    let s = store();
    Some(match (def.kind, s.value(slot)) {
        (Kind::Text { .. }, _) => format!("\"{}\"", s.text_at(slot)),
        (Kind::List { .. }, _) => "a list; edit it in the click GUI".to_string(),
        (Kind::Button { .. }, _) => return None,
        (_, Some(Value::Bool(b))) => on_off(b).to_string(),
        (Kind::Slider { decimals, .. }, Some(Value::Num(x))) => {
            format!("{:.*}", decimals as usize, x)
        }
        (Kind::Range { decimals, .. }, Some(Value::Range(lo, hi))) => {
            let d = decimals as usize;
            format!("{:.*}..{:.*}", d, lo, d, hi)
        }
        (Kind::Enum { options, .. }, Some(Value::Choice(i))) => {
            options.get(i as usize).copied().unwrap_or("?").to_string()
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
        .ok_or_else(|| format!("`{w}` is not a number"))
}

fn state_line(m: &ModuleDef) -> Line {
    let s = store();
    let armed = s.armed(m.id);
    let mut line = Line::default().name(m.name).t(" is ").state(armed);
    if armed && !s.allowed(m.id) {
        let mode = Mode::NAMES[m.mode as usize];
        let now = Mode::NAMES[s.profile() as usize];
        line = line.dim(format!(
            ", but held back: it needs the {mode} profile and {now} is in force"
        ));
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
        let head = Line::default()
            .t("Commands (")
            .value(format!("{p}help <command>"))
            .t(" for one):");
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
    let Some(cmd) = table.find(&name) else {
        c.err(format!("No command `{name}`"));
        return;
    };
    c.line(usage_line(&p, &cmd.shown(), &cmd.usage()));
    if !cmd.tip().is_empty() {
        c.ok(cmd.tip());
    }
    let others: Vec<String> = cmd
        .names
        .iter()
        .skip(1)
        .map(|n| format!("{p}{n}"))
        .collect();
    if !others.is_empty() {
        c.line(Line::default().t("Also: ").value(others.join(", ")));
    }
    if let Some(q) = &cmd.qualified {
        c.line(
            Line::default()
                .t("Always reachable as ")
                .value(format!("{p}{q}")),
        );
    }
}

fn toggle(c: &mut Call) {
    let Some(name) = c.arg(0) else {
        c.err("Which module? Usage: toggle <module>");
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
        c.err("Usage: set <module> <setting> <value>");
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
        return c.err(format!(
            "Usage: set {} {} <value>",
            norm(m.name),
            norm(def.name)
        ));
    }
    let s = store();
    let word = c.arg(2).unwrap_or_default().to_string();
    let result: Result<(), String> = match def.kind {
        Kind::Toggle { .. } => bool_word(&word)
            .map(|b| s.set_value(slot, Value::Bool(b)))
            .ok_or_else(|| format!("{} takes on or off, not `{word}`", def.name)),
        Kind::Slider { min, max, .. } => {
            number(&word).map(|x| s.set_value(slot, Value::Num(x.clamp(min, max))))
        }
        Kind::Range { min, max, .. } => {
            let pair = match word.split_once("..") {
                Some((a, b)) => Some((a.to_string(), b.to_string())),
                None => c.arg(3).map(|b| (word.clone(), b.to_string())),
            };
            match pair {
                None => Err(format!("{} takes a range: low..high", def.name)),
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
        Kind::Enum { options, .. } => match options.iter().position(|o| norm(o) == norm(&value)) {
            Some(i) => {
                s.set_value(slot, Value::Choice(i as u8));
                Ok(())
            }
            None => Err(format!(
                "{} is one of {}",
                def.name,
                options
                    .iter()
                    .map(|o| norm(o))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        },
        Kind::Text { max_len, .. } => {
            if value.chars().count() > max_len {
                Err(format!("{} is at most {max_len} characters", def.name))
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
                .ok_or_else(|| format!("`{word}` is not a key name")),
        },
        Kind::List { .. } => Err(format!("{} is a list; edit it in the click GUI", def.name)),
        Kind::Button { .. } => Err(format!(
            "{} is a button; press it in the click GUI",
            def.name
        )),
    };
    match result {
        Ok(()) => {
            let now = shown(slot, def).unwrap_or_default();
            c.line(setting_line(m.name, def.name, now));
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
        c.err("Usage: get <module> [setting]");
        return;
    };
    let m = match find_module(module) {
        Ok(m) => m,
        Err(e) => return c.err(e),
    };
    if let Some(setting) = c.arg(1) {
        match find_setting(m, setting) {
            Ok((slot, def)) => {
                let now = shown(slot, def).unwrap_or_else(|| "a button".to_string());
                c.line(setting_line(m.name, def.name, now));
            }
            Err(e) => c.err(e),
        }
        return;
    }
    let bind = store().bind(m.id);
    c.line(state_line(m).t(", bound to ").value(bind.display_name()));
    for (slot, def) in m.slots().zip(m.settings()) {
        if let Some(now) = shown(slot, def) {
            c.line(Line::default().t("  ").name(def.name).t(" = ").value(now));
        }
    }
}

fn bind(c: &mut Call) {
    let (Some(module), Some(key)) = (c.arg(0), c.arg(1)) else {
        c.err("Usage: bind <module> <key|none>");
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
            None => {
                return c.err(format!(
                    "`{key}` is not a key name; try r, f5, leftshift or mouse.4"
                ));
            }
        },
    };
    store().set_bind(m.id, b);
    c.line(
        Line::default()
            .name(m.name)
            .t(" is bound to ")
            .value(b.display_name()),
    );
}

fn profile_line(name: &str) -> Line {
    Line::default()
        .t("The ")
        .value(name)
        .t(" profile is in force")
}

fn profile(c: &mut Call) {
    let s = store();
    let Some(name) = c.arg(0) else {
        c.line(profile_line(Mode::NAMES[s.profile() as usize]));
        return;
    };
    match Mode::NAMES
        .iter()
        .position(|n| n.eq_ignore_ascii_case(name))
    {
        Some(i) => {
            s.set_profile(Mode::ALL[i]);
            c.line(profile_line(Mode::NAMES[i]));
        }
        None => c.err(format!(
            "`{name}` is not a profile; legit, normal or blatant"
        )),
    }
}

fn prefix(c: &mut Call) {
    let Some(new) = c.arg(0).map(str::to_string) else {
        let p = c.prefix.to_string();
        c.line(Line::default().t("The prefix is ").value(p));
        return;
    };
    if let Err(e) = super::check_prefix(&new) {
        return c.err(e);
    }
    store().set_text_at(registry::commands::PREFIX.0 as usize, &new);
    c.line(Line::default().t("The prefix is now ").value(new));
}

fn say(c: &mut Call) {
    let text = c.raw.trim().to_string();
    if text.is_empty() {
        c.err("Usage: say <text...>");
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
            None => c.err("The freecam is off; turn it on and fly to the spot first"),
        }
        return;
    }
    if c.toks.len() != 3 {
        let usage = format!("Usage: {}goto <x> <y> <z> | freecam | stop", c.prefix);
        c.err(usage);
        return;
    }
    let mut at = [Coord::Rel(0); 3];
    for (i, slot) in at.iter_mut().enumerate() {
        let text = c.arg(i).unwrap_or_default().to_string();
        match Coord::parse(&text) {
            Some(v) => *slot = v,
            None => {
                c.err(format!(
                    "`{text}` is not a coordinate; use a whole number, ~ or ~n"
                ));
                return;
            }
        }
    }
    request(Request::Goto(at));
}

fn follow(c: &mut Call) {
    use crate::modules::pathfinder::{Request, request};
    if c.toks.len() > 1 {
        let usage = format!("Usage: {}follow [player]", c.prefix);
        return c.err(usage);
    }
    request(Request::Follow(c.arg(0).map(str::to_string)));
}

fn modules(c: &mut Call) {
    let only = c.arg(0).map(norm);
    let s = store();
    let mut any = false;
    for cat in Category::ALL {
        if only.as_deref().is_some_and(|o| o != norm(cat.name())) {
            continue;
        }
        any = true;
        let mut line = Line::default().name(cat.name()).t(":");
        for (i, m) in cat.modules().enumerate() {
            line = line.t(if i == 0 { " " } else { ", " });
            line = if s.armed(m.id) {
                line.put(m.name, GREEN)
            } else {
                line.t(m.name)
            };
        }
        c.line(line);
    }
    if !any {
        c.err("Categories are combat, render, world, movement and misc");
    }
}
