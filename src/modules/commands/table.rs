#![cfg_attr(not(lua_mods), allow(dead_code))]

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, LazyLock, PoisonError, RwLock};

use super::args::Arg;
use super::builtin::{BUILTINS, Builtin};
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

    pub fn tip(&self) -> &'static str {
        match &self.owner {
            Owner::Builtin(b) => b.tip,
            Owner::Script(s) => s.decl.tip,
        }
    }

    pub fn usage(&self) -> String {
        match &self.owner {
            Owner::Builtin(b) => b.usage.to_string(),
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

struct Current {
    table: Arc<Table>,
    tree: Arc<CommandTree>,
}

static CURRENT: LazyLock<RwLock<Current>> = LazyLock::new(|| {
    let (table, _) = Table::with_scripts(Vec::new());
    let tree = super::tree::build(&table);
    RwLock::new(Current {
        table: Arc::new(table),
        tree: Arc::new(tree),
    })
});

static GENERATION: AtomicU32 = AtomicU32::new(0);

pub fn table() -> Arc<Table> {
    CURRENT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .table
        .clone()
}

pub fn tree() -> Arc<CommandTree> {
    CURRENT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .tree
        .clone()
}

pub fn generation() -> u32 {
    GENERATION.load(Relaxed)
}

#[cfg_attr(not(lua_mods), allow(dead_code))]
pub fn install_scripts(scripts: Vec<ScriptCommand>) -> Vec<String> {
    let (table, clashes) = Table::with_scripts(scripts);
    let tree = super::tree::build(&table);
    *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = Current {
        table: Arc::new(table),
        tree: Arc::new(tree),
    };
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
