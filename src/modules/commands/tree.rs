use azalea_protocol::packets::game::c_commands::{
    BrigadierNumber, BrigadierParser, BrigadierString,
};

use super::args::{Arg, ArgKind};
use super::builtin::{Shape, norm};
use super::table::{Owner, Table};
use crate::lang::tr;
use crate::modules::registry::{self, Category, Kind, Mode};
use crate::play::commands::{CommandTree, Node, NodeKind, Provider};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Names {
    English,
    #[cfg_attr(not(feature = "multi_language"), allow(dead_code))]
    Drawn,
    #[cfg_attr(not(feature = "multi_language"), allow(dead_code))]
    Both,
}

impl Names {
    pub(super) fn spell(self, english: &str, drawn: &str) -> Vec<String> {
        match self {
            Names::English => vec![norm(english)],
            Names::Drawn => vec![norm(drawn)],
            Names::Both => {
                let (e, d) = (norm(english), norm(drawn));
                if e == d { vec![e] } else { vec![e, d] }
            }
        }
    }
}

#[cfg(any(feature = "multi_language", test))]
pub fn for_line(typed: &str) -> Names {
    use crate::util::keyboard_layout::is_cyrillic;
    if typed
        .rsplit(' ')
        .next()
        .is_some_and(|word| !word.is_empty())
    {
        return Names::Both;
    }
    let args = typed.split_once(' ').map_or("", |(_, rest)| rest);
    let latin = args.chars().any(|c| c.is_ascii_alphabetic());
    let cyrillic = args.chars().any(is_cyrillic);
    match (latin, cyrillic) {
        (true, false) => Names::English,
        (true, true) => Names::Both,
        _ => Names::Drawn,
    }
}

struct B {
    nodes: Vec<Node>,
    names: Names,
}

impl B {
    fn add(&mut self, kind: NodeKind, executable: bool, children: Vec<usize>) -> usize {
        self.nodes.push(Node {
            kind,
            children,
            redirect: None,
            executable,
            restricted: false,
        });
        self.nodes.len() - 1
    }

    fn lit(&mut self, name: impl Into<String>, executable: bool, children: Vec<usize>) -> usize {
        self.add(NodeKind::Literal(name.into()), executable, children)
    }

    fn arg(
        &mut self,
        name: &str,
        parser: BrigadierParser,
        executable: bool,
        children: Vec<usize>,
    ) -> usize {
        let kind = NodeKind::Argument {
            name: name.to_string(),
            parser,
            provider: Provider::Argument,
        };
        self.add(kind, executable, children)
    }

    fn lits<'a>(
        &mut self,
        names: impl IntoIterator<Item = &'a str>,
        executable: bool,
        children: &[usize],
    ) -> Vec<usize> {
        names
            .into_iter()
            .map(|n| self.lit(n, executable, children.to_vec()))
            .collect()
    }

    fn named(
        &mut self,
        english: &str,
        drawn: &str,
        executable: bool,
        children: &[usize],
    ) -> Vec<usize> {
        let spelt = self.names.spell(english, drawn);
        self.lits(spelt.iter().map(String::as_str), executable, children)
    }
}

fn word() -> BrigadierParser {
    BrigadierParser::String(BrigadierString::SingleWord)
}

fn greedy() -> BrigadierParser {
    BrigadierParser::String(BrigadierString::GreedyPhrase)
}

fn value_nodes(b: &mut B, kind: Kind) -> Vec<usize> {
    match kind {
        Kind::Toggle { .. } => {
            let names = ["on", "off"];
            b.lits(names, true, &[])
        }
        Kind::Slider { .. } => {
            let number = BrigadierParser::Double(BrigadierNumber::new(None, None));
            vec![b.arg(tr!("commands.arg.value"), number, true, vec![])]
        }
        Kind::Range { .. } => vec![b.arg(tr!("commands.arg.range"), greedy(), true, vec![])],
        Kind::Enum { options, keys, .. } => {
            let mut out = Vec::new();
            for (i, o) in options.iter().enumerate() {
                out.extend(b.named(o, registry::option_label(options, keys, i), true, &[]));
            }
            out
        }
        Kind::Text { .. } => vec![b.arg(tr!("commands.arg.text"), greedy(), true, vec![])],
        Kind::Key { .. } => {
            let none = b.lit("none", true, vec![]);
            vec![none, b.arg(tr!("commands.arg.key"), word(), true, vec![])]
        }
        Kind::List { .. } | Kind::Button { .. } => Vec::new(),
    }
}

fn builtin_children(b: &mut B, shape: Shape, table: &Table) -> (bool, Vec<usize>) {
    let modules: Vec<_> = registry::modules().collect();
    match shape {
        Shape::Help => {
            let names: Vec<String> = table.commands.iter().map(|c| c.shown()).collect();
            (true, b.lits(names.iter().map(String::as_str), true, &[]))
        }
        Shape::Module => {
            let mut out = Vec::with_capacity(modules.len());
            for m in &modules {
                out.extend(b.named(m.name, m.label(), true, &[]));
            }
            (false, out)
        }
        Shape::Set | Shape::Get => {
            let set = shape == Shape::Set;
            let mut out = Vec::with_capacity(modules.len());
            for m in &modules {
                let mut settings = Vec::with_capacity(m.settings().len());
                for d in m.settings() {
                    if set {
                        let values = value_nodes(b, d.kind);
                        if !values.is_empty() {
                            settings.extend(b.named(d.name, d.label(), false, &values));
                        }
                    } else if !matches!(d.kind, Kind::Button { .. }) {
                        settings.extend(b.named(d.name, d.label(), true, &[]));
                    }
                }
                out.extend(b.named(m.name, m.label(), !set, &settings));
            }
            (false, out)
        }
        Shape::Bind => {
            let none = b.lit("none", true, vec![]);
            let key = b.arg(tr!("commands.arg.key"), word(), true, vec![]);
            let mut out = Vec::with_capacity(modules.len());
            for m in &modules {
                out.extend(b.named(m.name, m.label(), false, &[none, key]));
            }
            (false, out)
        }
        Shape::Profile => {
            let mut out = Vec::new();
            for mode in Mode::ALL {
                out.extend(b.named(Mode::NAMES[mode as usize], mode.label(), true, &[]));
            }
            (true, out)
        }
        Shape::Word => (
            true,
            vec![b.arg(tr!("commands.arg.text"), greedy(), true, vec![])],
        ),
        Shape::Rest => (
            false,
            vec![b.arg(tr!("commands.arg.text"), greedy(), true, vec![])],
        ),
        Shape::Goto => {
            let z = b.arg("z", word(), true, vec![]);
            let y = b.arg("y", word(), false, vec![z]);
            let x = b.arg("x", word(), false, vec![y]);
            let stop = b.lit("stop", true, vec![]);
            let freecam = b.lit("freecam", true, vec![]);
            (false, vec![stop, freecam, x])
        }
        Shape::Follow => (
            true,
            vec![b.arg(
                tr!("commands.arg.player"),
                BrigadierParser::GameProfile,
                true,
                vec![],
            )],
        ),
        Shape::Choice(words) => (true, b.lits(words.iter().copied(), true, &[])),
        #[cfg(feature = "seed_cracker")]
        Shape::Cracker => {
            let sides = b.lits(["bedrock", "structures"], true, &[]);
            let mut out = b.lits(["status", "clear", "restart", "help"], true, &[]);
            out.push(b.lit("trust", false, sides));
            (true, out)
        }
        Shape::Friend => {
            let player = b.arg(
                tr!("commands.arg.player"),
                BrigadierParser::GameProfile,
                true,
                vec![],
            );
            let mut out = b.lits(["add", "remove"], false, &[player]);
            out.extend(b.lits(["list"], true, &[]));
            (true, out)
        }
        Shape::Mine => {
            use crate::modules::seed_xray::table::Kind;
            let mut names = vec!["log".to_string(), "wood".to_string()];
            names.extend(
                Kind::ALL
                    .into_iter()
                    .map(crate::modules::ore_miner::kind_name),
            );
            names.extend(
                crate::modules::esp::blocks::ids()
                    .iter()
                    .map(|id| id.to_string()),
            );
            names.sort_unstable();
            names.dedup();
            names.retain(|n| n != "stop");
            let whole = BrigadierParser::Integer(BrigadierNumber::new(Some(1), None));
            let count = b.arg(tr!("commands.arg.count"), whole, true, vec![]);
            let mut out = b.lits(names.iter().map(String::as_str), true, &[count]);
            out.push(b.lit("stop", true, vec![]));
            out.push(b.arg(tr!("commands.arg.name"), greedy(), true, vec![]));
            (true, out)
        }
        Shape::Category => {
            let mut out = Vec::new();
            for c in Category::ALL {
                out.extend(b.named(c.name(), c.label(), true, &[]));
            }
            (true, out)
        }
        Shape::Nbt => {
            use azalea_registry::Registry;
            use azalea_registry::builtin::DataComponentKind;
            let names: Vec<String> = (0..)
                .map_while(DataComponentKind::from_u32)
                .map(|k| {
                    let s = crate::play::item_components::key(k);
                    s.strip_prefix("minecraft:")
                        .map_or(s.clone(), str::to_string)
                })
                .collect();
            let component = b.arg(tr!("commands.arg.component"), greedy(), true, vec![]);
            let mut kinds = b.lits(names.iter().map(String::as_str), true, &[]);
            kinds.push(component);
            let remove = b.lit("remove", false, kinds);
            let list = b.arg(tr!("commands.arg.components"), greedy(), true, vec![]);
            let mut out = b.lits(["add", "set"], false, &[list]);
            out.extend(b.lits(["get", "copy"], true, &[]));
            let stack = BrigadierParser::Integer(BrigadierNumber::new(Some(1), Some(99)));
            let count = b.arg(tr!("commands.arg.count"), stack, true, vec![]);
            out.push(b.lit("count", false, vec![count]));
            out.push(remove);
            (false, out)
        }
    }
}

fn arg_chain(b: &mut B, args: &[Arg]) -> (bool, Vec<usize>) {
    let mut next: Vec<usize> = Vec::new();
    for (i, a) in args.iter().enumerate().rev() {
        let done = args.get(i + 1).is_none_or(|n| n.optional);
        let parser = match a.kind {
            ArgKind::Word => BrigadierParser::String(BrigadierString::QuotablePhrase),
            ArgKind::String => greedy(),
            ArgKind::Int => BrigadierParser::Integer(BrigadierNumber::new(None, None)),
            ArgKind::Number => BrigadierParser::Double(BrigadierNumber::new(None, None)),
            ArgKind::Bool => BrigadierParser::Bool,
        };
        let mut level = b.lits(a.suggest.iter().copied(), done, &next);
        level.push(b.arg(a.name, parser, done, next));
        next = level;
    }
    (args.first().is_none_or(|a| a.optional), next)
}

pub fn build(table: &Table, names: Names) -> CommandTree {
    let mut b = B {
        nodes: Vec::new(),
        names,
    };
    let root = b.add(NodeKind::Root, false, Vec::new());
    let mut top = Vec::new();
    for c in &table.commands {
        let (exec, children) = match &c.owner {
            Owner::Builtin(builtin) => builtin_children(&mut b, builtin.shape, table),
            Owner::Script(s) => arg_chain(&mut b, &s.decl.args),
        };
        for name in &c.names {
            top.push(b.lit(*name, exec, children.clone()));
        }
        if let Some(q) = &c.qualified {
            top.push(b.lit(q.as_str(), exec, children.clone()));
        }
    }
    b.nodes[root].children = top;
    CommandTree {
        nodes: b.nodes,
        root,
    }
}
