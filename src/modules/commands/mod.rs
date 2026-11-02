#[cfg(feature = "click_gui")]
mod args;
#[cfg(feature = "click_gui")]
mod builtin;
#[cfg(feature = "click_gui")]
mod table;
#[cfg(all(test, feature = "click_gui"))]
mod tests;
#[cfg(feature = "click_gui")]
mod tree;

#[cfg(lua_mods)]
pub use args::{Arg, ArgKind, ArgValue, bind, usage};
#[cfg(lua_mods)]
pub use builtin::norm;
#[cfg(lua_mods)]
pub use table::{Decl, ScriptCommand, install_scripts, resolve_script};

use crate::session::SharedMutex;

#[cfg(not(feature = "click_gui"))]
#[inline(always)]
pub fn intercept(_: &SharedMutex, line: String) -> Option<String> {
    Some(line)
}

#[cfg(not(feature = "click_gui"))]
#[inline(always)]
pub fn is_client_line(_: &str) -> bool {
    false
}

#[cfg(not(feature = "click_gui"))]
#[inline(always)]
pub fn open_key() -> Option<(bevy::prelude::KeyCode, String)> {
    None
}

#[cfg(not(feature = "click_gui"))]
#[inline(always)]
pub fn completion() -> Option<(char, std::sync::Arc<crate::play::commands::CommandTree>)> {
    None
}

#[cfg(not(feature = "click_gui"))]
#[inline(always)]
pub fn generation() -> u64 {
    0
}

#[cfg(feature = "click_gui")]
pub use on::*;

#[cfg(feature = "click_gui")]
mod on {
    use std::sync::Arc;

    use super::builtin::{Call, Line, norm};
    use super::table::{self, Owner};
    use super::{SharedMutex, args};
    use crate::modules::chat_math::show_local_spans;
    use crate::modules::registry::{self, Id};
    use crate::modules::store;
    use crate::play::commands::CommandTree;

    const DEFAULT_PREFIX: &str = ".";

    pub fn check_prefix(p: &str) -> Result<(), String> {
        let Some(first) = p.chars().next() else {
            return Err("The prefix cannot be empty".to_string());
        };
        if first == '/' {
            return Err("The prefix cannot start with /, which is the server's".to_string());
        }
        if first.is_alphanumeric() {
            return Err(
                "The prefix cannot start with a letter or digit, or ordinary chat would be taken"
                    .to_string(),
            );
        }
        if p.chars().any(char::is_whitespace) {
            return Err("The prefix cannot hold a space".to_string());
        }
        if p.chars().count() > 4 {
            return Err("The prefix is at most 4 characters".to_string());
        }
        Ok(())
    }

    pub fn prefix() -> String {
        let slot = registry::commands::PREFIX.0 as usize;
        store().with_text(slot, |p| {
            if check_prefix(p).is_ok() {
                p.to_string()
            } else {
                DEFAULT_PREFIX.to_string()
            }
        })
    }

    fn split<'a>(line: &'a str, prefix: &str) -> Option<(String, &'a str)> {
        let body = line.strip_prefix(prefix)?;
        let first = body.chars().next()?;
        if !(first.is_alphanumeric() || first == '_') {
            return None;
        }
        let end = body.find(char::is_whitespace).unwrap_or(body.len());
        Some((body[..end].to_lowercase(), body[end..].trim_start()))
    }

    fn show(shared: &SharedMutex, lines: Vec<Line>) {
        if lines.is_empty() {
            return;
        }
        let mut s = shared.lock().unwrap();
        for line in lines {
            show_local_spans(&mut s.session, line.0);
        }
    }

    fn plain(line: &str) -> bool {
        line.chars()
            .next()
            .is_none_or(|c| c == '/' || c.is_alphanumeric() || c.is_whitespace())
    }

    pub fn intercept(shared: &SharedMutex, line: String) -> Option<String> {
        if plain(&line) {
            return Some(line);
        }
        let s = store();
        if !s.enabled(Id::Commands) {
            return Some(line);
        }
        let prefix = prefix();
        let Some((name, raw)) = split(&line, &prefix) else {
            return Some(line);
        };
        let table = table::table();
        let Some(cmd) = table.find(&name) else {
            show(
                shared,
                vec![Line::error(format!(
                    "Unknown command {prefix}{name}; {prefix}help lists them"
                ))],
            );
            return None;
        };
        match &cmd.owner {
            Owner::Builtin(b) => {
                let toks = match args::tokens(raw) {
                    Ok(t) => t,
                    Err(e) => {
                        show(shared, vec![Line::error(e)]);
                        return None;
                    }
                };
                let mut call = Call {
                    raw,
                    toks,
                    table: &table,
                    prefix: &prefix,
                    out: Vec::new(),
                    send: None,
                };
                (b.run)(&mut call);
                let Call { out, send, .. } = call;
                show(shared, out);
                send
            }
            Owner::Script(sc) => {
                if !s.enabled(sc.module) {
                    let m = registry::module(sc.module);
                    let why = format!(
                        "{} is off; {prefix}toggle {} turns it on",
                        m.name,
                        norm(m.name)
                    );
                    show(shared, vec![Line::error(why)]);
                    return None;
                }
                #[cfg(lua_mods)]
                crate::modules::script::command_typed(name, raw.to_string());
                None
            }
        }
    }

    pub fn open_key() -> Option<(bevy::prelude::KeyCode, String)> {
        use bevy::prelude::KeyCode;
        if !store().enabled(Id::Commands) {
            return None;
        }
        let p = prefix();
        let key = match p.chars().next()? {
            '.' => KeyCode::Period,
            ',' => KeyCode::Comma,
            '-' => KeyCode::Minus,
            '=' => KeyCode::Equal,
            ';' => KeyCode::Semicolon,
            '\'' => KeyCode::Quote,
            '`' => KeyCode::Backquote,
            '[' => KeyCode::BracketLeft,
            ']' => KeyCode::BracketRight,
            '\\' => KeyCode::Backslash,
            _ => return None,
        };
        Some((key, p))
    }

    pub fn is_client_line(line: &str) -> bool {
        !plain(line) && store().enabled(Id::Commands) && split(line, &prefix()).is_some()
    }

    pub fn completion() -> Option<(char, Arc<CommandTree>)> {
        if !store().enabled(Id::Commands) {
            return None;
        }
        let p = prefix();
        let mut chars = p.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some((c, table::tree())),
            _ => None,
        }
    }

    pub fn generation() -> u64 {
        let on = store().enabled(Id::Commands);
        let slot = registry::commands::PREFIX.0 as usize;
        let first = |p: &str| {
            if check_prefix(p).is_ok() {
                p.chars().next()
            } else {
                DEFAULT_PREFIX.chars().next()
            }
        };
        let lead = if on {
            store().with_text(slot, first).map_or(0, u64::from)
        } else {
            0
        };
        (table::generation() as u64) << 33 | lead << 1 | on as u64
    }
}
