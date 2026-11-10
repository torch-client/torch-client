use azalea_protocol::packets::game::c_commands::{
    BrigadierNumber, BrigadierParser, BrigadierString,
};

use super::args::{Arg, ArgKind};
use super::builtin::{Shape, norm};
use super::table::{Owner, Table};
use crate::modules::registry::{self, Category, Kind, Mode};
use crate::play::commands::{CommandTree, Node, NodeKind, Provider};

struct B {
    nodes: Vec<Node>,
}

impl B {
    fn add(&mut self, kind: NodeKind, executable: bool, children: Vec<usize>) -> usize {
        self.nodes.push(Node {
            kind,
            children,
            redirect: None,
            executable,
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
        Kind::Slider { .. } => vec![b.arg(
            "value",
            BrigadierParser::Double(BrigadierNumber::new(None, None)),
            true,
            vec![],
        )],
        Kind::Range { .. } => vec![b.arg("low..high", greedy(), true, vec![])],
        Kind::Enum { options, .. } => {
            let names: Vec<String> = options.iter().map(|o| norm(o)).collect();
            b.lits(names.iter().map(String::as_str), true, &[])
        }
        Kind::Text { .. } => vec![b.arg("text", greedy(), true, vec![])],
        Kind::Key { .. } => {
            let none = b.lit("none", true, vec![]);
            vec![none, b.arg("key", word(), true, vec![])]
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
            let names: Vec<String> = modules.iter().map(|m| norm(m.name)).collect();
            (false, b.lits(names.iter().map(String::as_str), true, &[]))
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
                            settings.push(b.lit(norm(d.name), false, values));
                        }
                    } else if !matches!(d.kind, Kind::Button { .. }) {
                        settings.push(b.lit(norm(d.name), true, vec![]));
                    }
                }
                out.push(b.lit(norm(m.name), !set, settings));
            }
            (false, out)
        }
        Shape::Bind => {
            let none = b.lit("none", true, vec![]);
            let key = b.arg("key", word(), true, vec![]);
            let names: Vec<String> = modules.iter().map(|m| norm(m.name)).collect();
            (
                false,
                b.lits(names.iter().map(String::as_str), false, &[none, key]),
            )
        }
        Shape::Profile => {
            let names: Vec<String> = Mode::NAMES.iter().map(|n| n.to_lowercase()).collect();
            (true, b.lits(names.iter().map(String::as_str), true, &[]))
        }
        Shape::Word => (true, vec![b.arg("text", greedy(), true, vec![])]),
        Shape::Rest => (false, vec![b.arg("text", greedy(), true, vec![])]),
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
            vec![b.arg("player", BrigadierParser::GameProfile, true, vec![])],
        ),
        Shape::Choice(words) => (true, b.lits(words.iter().copied(), true, &[])),
        Shape::Category => {
            let names: Vec<String> = Category::ALL.iter().map(|c| norm(c.name())).collect();
            (true, b.lits(names.iter().map(String::as_str), true, &[]))
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

pub fn build(table: &Table) -> CommandTree {
    let mut b = B { nodes: Vec::new() };
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
