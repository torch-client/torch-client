use super::{folder, install, load};
use crate::platform::packfs;

pub const SOURCE_MAX: usize = 262_144;

pub const TEMPLATE: &str = "local torch = require(\"torch\")\n\
\n\
local m = torch.module { name = \"My mod\" }\n\
\n\
function m:onenable()\n\
    torch.chat.print(\"My mod is on\")\n\
end\n\
\n\
function m:ondisable()\n\
    torch.chat.print(\"My mod is off\")\n\
end\n";

pub fn mod_id(name: &str) -> Result<String, String> {
    let name = name.trim();
    let id = name.strip_suffix(".lua").unwrap_or(name);
    match id {
        _ if load::valid_id(id) => Ok(id.to_string()),
        "" => Err("Name the mod".to_string()),
        _ => Err(format!("Not a mod id: {}", load::ID_RULE)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    pub line: Option<usize>,
    pub message: String,
}

pub fn syntax(src: &str) -> Result<(), SyntaxError> {
    match mlua::Compiler::new().compile(src) {
        Ok(_) => Ok(()),
        Err(mlua::Error::SyntaxError { message, .. }) => {
            let parsed = message
                .split_once(": ")
                .and_then(|(n, rest)| Some((n.parse().ok()?, rest.to_string())));
            Err(match parsed {
                Some((n, rest)) => SyntaxError {
                    line: Some(n),
                    message: rest,
                },
                None => SyntaxError {
                    line: None,
                    message,
                },
            })
        }
        Err(e) => Err(SyntaxError {
            line: None,
            message: e.to_string(),
        }),
    }
}

pub fn open(file: &str) -> Result<String, String> {
    let bytes =
        packfs::read(folder().join(file)).map_err(|e| format!("Could not read {file}: {e}"))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| format!("{file} is not UTF-8 text, so it cannot be edited here"))?;
    let n = text.chars().count();
    if n > SOURCE_MAX {
        return Err(format!(
            "{file} is {n} characters; the editor takes up to {SOURCE_MAX}"
        ));
    }
    Ok(text)
}

pub fn save(original: Option<&str>, id: &str, src: &str) -> Result<(), String> {
    let dir = folder();
    let file = format!("{id}.lua");
    let same = install::same_id(&dir, id);
    let renamed_from = match original {
        Some(old) if old == file => match same.iter().all(|n| n == &file) {
            true => None,
            false => {
                return Err(format!(
                    "The mods folder now holds {}; reload and edit again",
                    same.join(", ")
                ));
            }
        },
        _ if !same.is_empty() => {
            return Err(format!("{} already uses the name `{id}`", same.join(", ")));
        }
        None => None,
        Some(old) => Some(old.to_string()),
    };

    let tmp = dir.join(format!(".{file}.tmp"));
    packfs::create_dir_all(&dir)
        .and_then(|()| packfs::write(&tmp, src.as_bytes()))
        .and_then(|()| packfs::rename(&tmp, dir.join(&file)))
        .map_err(|e| format!("Could not save {file}: {e}"))?;
    let mut problem = None;
    if let Some(old) = &renamed_from
        && let Err(e) = packfs::remove_file(dir.join(old))
        && e.kind() != std::io::ErrorKind::NotFound
    {
        problem = Some(format!("Saved as {file}, but could not remove {old}: {e}"));
    }
    crate::log_info!("mods", "saved {file} from the editor");

    let id = id.to_string();
    let new = original.is_none();
    super::on_worker(move || {
        let mut ids = super::disabled();
        let changed = match renamed_from {
            _ if new => ids.remove(&id),
            Some(old) => ids.remove(old.strip_suffix(".lua").unwrap_or(&old)) && ids.insert(id),
            None => false,
        };
        if changed {
            super::store_disabled(&ids);
        }
        let world = super::in_world();
        super::load(world, world);
    });
    problem.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_id_agrees_with_the_load_rule() {
        for name in [
            "autogg",
            "a1",
            "low-health_2",
            "a",
            "1ab",
            "AutoGG",
            "auto.gg",
            "",
            "ab cd",
            "-ab",
            "zz",
        ] {
            assert_eq!(mod_id(name).is_ok(), load::valid_id(name), "{name}");
        }
        assert!(mod_id(&"a".repeat(65)).is_err());
        assert_eq!(mod_id(" autogg.lua ").as_deref(), Ok("autogg"));
        assert_eq!(mod_id("").unwrap_err(), "Name the mod");
    }

    #[test]
    fn a_syntax_error_names_its_line() {
        assert!(syntax(TEMPLATE).is_ok());
        let e = syntax("local x = 1\nfunction f(\n").unwrap_err();
        assert!(e.line.is_some(), "{e:?}");
    }
}
