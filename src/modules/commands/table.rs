#![cfg_attr(not(lua_mods), allow(dead_code))]

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, PoisonError, RwLock};

use super::args::Arg;
use super::builtin::{BUILTINS, Builtin};
use super::tree::Names;
use crate::modules::registry::ModuleIx;
use crate::play::commands::CommandTree;

#[derive(Clone, Debug)]
pub struct Decl {
    pub name: &'static str,
    pub aliases: Vec<&'static str>,
    pub tip: &'static str,
    pub args: Vec<Arg>,
}

#[derive(Clone, Debug)]
pub struct ScriptCommand {
    pub mod_id: &'static str,
    pub module: ModuleIx,
    pub decl: Decl,
}

pub enum Owner {
    Builtin(&'static Builtin),
    Script(ScriptCommand),
}

pub struct Command {
    pub owner: Owner,
    pub names: Vec<&'static str>,
    pub qualified: Option<String>,
}

impl Command {
    pub fn name(&self) -> &'static str {
        match &self.owner {
            Owner::Builtin(b) => b.name,
            Owner::Script(s) => s.decl.name,
        }
    }

    pub fn tip(&self) -> String {
        match &self.owner {
            Owner::Builtin(b) => {
                let names: Vec<String> = (b.tip_names)().into_iter().map(str::to_string).collect();
                if names.is_empty() {
                    b.tip.get().to_string()
                } else {
                    crate::util::placeholders::substitute(b.tip.get(), &names)
                }
            }
            Owner::Script(s) => s.decl.tip.to_string(),
        }
    }

    pub fn usage(&self) -> String {
        match &self.owner {
            Owner::Builtin(b) => b.usage.get().to_string(),
            Owner::Script(s) => super::args::usage(&s.decl.args),
        }
    }

    pub fn shown(&self) -> String {
        match (self.names.first(), &self.qualified) {
            (Some(n), _) => n.to_string(),
            (None, Some(q)) => q.clone(),
            (None, None) => self.name().to_string(),
        }
    }
}

pub struct Table {
    pub commands: Vec<Command>,
}

impl Table {
    pub(super) fn with_scripts(scripts: Vec<ScriptCommand>) -> (Table, Vec<String>) {
        let mut commands: Vec<Command> = BUILTINS
            .iter()
            .map(|b| Command {
                owner: Owner::Builtin(b),
                names: std::iter::once(b.name)
                    .chain(b.aliases.iter().copied())
                    .collect(),
                qualified: None,
            })
            .collect();
        let mut clashes = Vec::new();
        for s in scripts {
            let mut names = Vec::new();
            for n in std::iter::once(s.decl.name).chain(s.decl.aliases.iter().copied()) {
                let holder = commands.iter().find(|c| c.names.contains(&n));
                match holder {
                    Some(c) => clashes.push(format!(
                        "[mods] {}: command name `{n}` is taken by {}; type it as {}:{}",
                        s.mod_id,
                        match &c.owner {
                            Owner::Builtin(_) => "a built-in command".to_string(),
                            Owner::Script(o) => format!("the mod {}", o.mod_id),
                        },
                        s.mod_id,
                        s.decl.name
                    )),
                    None if !names.contains(&n) => names.push(n),
                    None => {}
                }
            }
            let qualified = Some(format!("{}:{}", s.mod_id, s.decl.name));
            commands.push(Command {
                owner: Owner::Script(s),
                names,
                qualified,
            });
        }
        (Table { commands }, clashes)
    }

    pub fn find(&self, name: &str) -> Option<&Command> {
        self.commands
            .iter()
            .find(|c| c.names.iter().any(|n| *n == name))
            .or_else(|| {
                self.commands
                    .iter()
                    .find(|c| c.qualified.as_deref() == Some(name))
            })
    }
}

struct Trees {
    english: Arc<CommandTree>,
    drawn: Arc<CommandTree>,
    both: Arc<CommandTree>,
}

impl Trees {
    fn build(table: &Table) -> Trees {
        let english = Arc::new(super::tree::build(table, Names::English));
        #[cfg(feature = "multi_language")]
        if crate::lang::active() != crate::lang::Lang::EnUs {
            let drawn = Arc::new(super::tree::build(table, Names::Drawn));
            let both = Arc::new(super::tree::build(table, Names::Both));
            return Trees {
                english,
                drawn,
                both,
            };
        }
        Trees {
            drawn: english.clone(),
            both: english.clone(),
            english,
        }
    }

    fn get(&self, names: Names) -> Arc<CommandTree> {
        match names {
            Names::English => self.english.clone(),
            Names::Drawn => self.drawn.clone(),
            Names::Both => self.both.clone(),
        }
    }
}

struct Current {
    table: Arc<Table>,
    trees: Trees,
    epoch: u32,
}

impl Current {
    fn new(table: Table) -> Current {
        let epoch = crate::lang::epoch();
        let trees = Trees::build(&table);
        Current {
            table: Arc::new(table),
            trees,
            epoch,
        }
    }
}

static CURRENT: LazyLock<RwLock<Current>> =
    LazyLock::new(|| RwLock::new(Current::new(Table::with_scripts(Vec::new()).0)));

static GENERATION: AtomicU32 = AtomicU32::new(0);

pub fn table() -> Arc<Table> {
    CURRENT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .table
        .clone()
}

pub fn tree(names: Names) -> Arc<CommandTree> {
    let epoch = crate::lang::epoch();
    {
        let current = CURRENT.read().unwrap_or_else(PoisonError::into_inner);
        if current.epoch == epoch {
            return current.trees.get(names);
        }
    }
    let mut current = CURRENT.write().unwrap_or_else(PoisonError::into_inner);
    if current.epoch != epoch {
        current.trees = Trees::build(&current.table);
        current.epoch = epoch;
    }
    current.trees.get(names)
}

pub fn generation() -> u32 {
    GENERATION.load(Relaxed).wrapping_add(crate::lang::epoch())
}

#[cfg_attr(not(lua_mods), allow(dead_code))]
pub fn install_scripts(scripts: Vec<ScriptCommand>) -> Vec<String> {
    let (table, clashes) = Table::with_scripts(scripts);
    let current = Current::new(table);
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = current;
    GENERATION.fetch_add(1, Relaxed);
    clashes
}

#[cfg_attr(not(lua_mods), allow(dead_code))]
pub fn resolve_script(name: &str) -> Option<ScriptCommand> {
    match &table().find(name)?.owner {
        Owner::Script(s) => Some(s.clone()),
        Owner::Builtin(_) => None,
    }
}
