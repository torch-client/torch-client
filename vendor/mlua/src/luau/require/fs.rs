use std::collections::VecDeque;
use std::io::Result as IoResult;
use std::path::{Component, Path, PathBuf};
use std::result::Result as StdResult;
use std::{env, fs};

use crate::error::Result;
use crate::function::Function;
use crate::state::Lua;

use super::{NavigateError, Require};

#[derive(Default, Debug)]
pub struct TextRequirer {
    abs_path: PathBuf,
    rel_path: PathBuf,
    resolved_path: Option<PathBuf>,
}

impl TextRequirer {
    const CHUNK_PREFIX: &str = "@";

    const FILE_EXTENSIONS: &[&str] = &["luau", "lua"];

    const LUAURC_CONFIG_FILENAME: &str = ".luaurc";

    const LUAU_CONFIG_FILENAME: &str = ".config.luau";

    pub fn new() -> Self {
        Self::default()
    }

    fn normalize_chunk_name(chunk_name: &str) -> &str {
        if let Some((path, line)) = chunk_name.rsplit_once(':') {
            if line.parse::<u32>().is_ok() {
                return path;
            }
        }
        chunk_name
    }

    fn normalize_path(path: &Path) -> PathBuf {
        let mut components = VecDeque::new();

        for comp in path.components() {
            match comp {
                Component::Prefix(..) | Component::RootDir => {
                    components.push_back(comp);
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if matches!(components.back(), None | Some(Component::ParentDir)) {
                        components.push_back(Component::ParentDir);
                    } else if matches!(components.back(), Some(Component::Normal(..))) {
                        components.pop_back();
                    }
                }
                Component::Normal(..) => components.push_back(comp),
            }
        }

        if matches!(components.front(), None | Some(Component::Normal(..))) {
            components.push_front(Component::CurDir);
        }

        components.into_iter().collect()
    }

    fn resolve_module(path: &Path) -> StdResult<Option<PathBuf>, NavigateError> {
        let mut found_path = None;

        if path.components().next_back() != Some(Component::Normal("init".as_ref())) {
            let current_ext = (path.extension().and_then(|s| s.to_str()))
                .map(|s| format!("{s}."))
                .unwrap_or_default();
            for ext in Self::FILE_EXTENSIONS {
                let candidate = path.with_extension(format!("{current_ext}{ext}"));
                if candidate.is_file() && found_path.replace(candidate).is_some() {
                    return Err(NavigateError::Ambiguous);
                }
            }
        }
        if path.is_dir() {
            for component in Self::FILE_EXTENSIONS.iter().map(|ext| format!("init.{ext}")) {
                let candidate = path.join(component);
                if candidate.is_file() && found_path.replace(candidate).is_some() {
                    return Err(NavigateError::Ambiguous);
                }
            }

            if found_path.is_none() {
                return Ok(None);
            }
        }

        Ok(Some(found_path.ok_or(NavigateError::NotFound)?))
    }
}

impl Require for TextRequirer {
    fn is_require_allowed(&self, chunk_name: &str) -> bool {
        chunk_name.starts_with(Self::CHUNK_PREFIX)
    }

    fn reset(&mut self, chunk_name: &str) -> StdResult<(), NavigateError> {
        if !chunk_name.starts_with(Self::CHUNK_PREFIX) {
            return Err(NavigateError::NotFound);
        }
        let chunk_name = Self::normalize_chunk_name(&chunk_name[1..]);
        let chunk_path = Self::normalize_path(chunk_name.as_ref());

        if chunk_path.extension() == Some("rs".as_ref()) {
            let chunk_filename = chunk_path.file_name().unwrap();
            let cwd = env::current_dir().map_err(|_| NavigateError::NotFound)?;
            self.abs_path = Self::normalize_path(&cwd.join(chunk_filename));
            self.rel_path = ([Component::CurDir, Component::Normal(chunk_filename)].into_iter()).collect();
            self.resolved_path = None;

            return Ok(());
        }

        if chunk_path.is_absolute() {
            let resolved_path = Self::resolve_module(&chunk_path)?;
            self.abs_path = chunk_path.clone();
            self.rel_path = chunk_path;
            self.resolved_path = resolved_path;
        } else {
            let cwd = env::current_dir().map_err(|_| NavigateError::NotFound)?;
            let abs_path = Self::normalize_path(&cwd.join(&chunk_path));
            let resolved_path = Self::resolve_module(&abs_path)?;
            self.abs_path = abs_path;
            self.rel_path = chunk_path;
            self.resolved_path = resolved_path;
        }

        Ok(())
    }

    fn jump_to_alias(&mut self, path: &str) -> StdResult<(), NavigateError> {
        let path = Self::normalize_path(path.as_ref());
        let resolved_path = Self::resolve_module(&path)?;

        self.abs_path = path.clone();
        self.rel_path = path;
        self.resolved_path = resolved_path;

        Ok(())
    }

    fn to_parent(&mut self) -> StdResult<(), NavigateError> {
        let mut abs_path = self.abs_path.clone();
        if !abs_path.pop() {
            return Err(NavigateError::NotFound);
        }
        let mut rel_parent = self.rel_path.clone();
        rel_parent.pop();
        let resolved_path = Self::resolve_module(&abs_path)?;

        self.abs_path = abs_path;
        self.rel_path = Self::normalize_path(&rel_parent);
        self.resolved_path = resolved_path;

        Ok(())
    }

    fn to_child(&mut self, name: &str) -> StdResult<(), NavigateError> {
        let abs_path = self.abs_path.join(name);
        let rel_path = self.rel_path.join(name);
        let resolved_path = Self::resolve_module(&abs_path)?;

        self.abs_path = abs_path;
        self.rel_path = rel_path;
        self.resolved_path = resolved_path;

        Ok(())
    }

    fn has_module(&self) -> bool {
        (self.resolved_path.as_deref())
            .map(Path::is_file)
            .unwrap_or(false)
    }

    fn cache_key(&self) -> String {
        self.resolved_path.as_deref().unwrap().display().to_string()
    }

    fn has_config(&self) -> bool {
        self.abs_path.is_dir() && self.abs_path.join(Self::LUAURC_CONFIG_FILENAME).is_file()
            || self.abs_path.is_dir() && self.abs_path.join(Self::LUAU_CONFIG_FILENAME).is_file()
    }

    fn config(&self) -> IoResult<Vec<u8>> {
        if self.abs_path.join(Self::LUAURC_CONFIG_FILENAME).is_file() {
            return fs::read(self.abs_path.join(Self::LUAURC_CONFIG_FILENAME));
        }
        fs::read(self.abs_path.join(Self::LUAU_CONFIG_FILENAME))
    }

    fn loader(&self, lua: &Lua) -> Result<Function> {
        let name = format!("@{}", self.rel_path.display());
        lua.load(self.resolved_path.as_deref().unwrap())
            .set_name(name)
            .into_function()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::TextRequirer;

    #[test]
    fn test_path_normalize() {
        for (input, expected) in [
            ("", "./"),
            (".", "./"),
            ("a/relative/path", "./a/relative/path"),
            ("./remove/extraneous/symbols/", "./remove/extraneous/symbols"),
            ("./remove/extraneous//symbols", "./remove/extraneous/symbols"),
            ("./remove/extraneous/symbols/.", "./remove/extraneous/symbols"),
            ("./remove/extraneous/./symbols", "./remove/extraneous/symbols"),
            ("../remove/extraneous/symbols/", "../remove/extraneous/symbols"),
            ("../remove/extraneous//symbols", "../remove/extraneous/symbols"),
            ("../remove/extraneous/symbols/.", "../remove/extraneous/symbols"),
            ("../remove/extraneous/./symbols", "../remove/extraneous/symbols"),
            ("/remove/extraneous/symbols/", "/remove/extraneous/symbols"),
            ("/remove/extraneous//symbols", "/remove/extraneous/symbols"),
            ("/remove/extraneous/symbols/.", "/remove/extraneous/symbols"),
            ("/remove/extraneous/./symbols", "/remove/extraneous/symbols"),
            ("./remove/me/..", "./remove"),
            ("./remove/me/../", "./remove"),
            ("../remove/me/..", "../remove"),
            ("../remove/me/../", "../remove"),
            ("/remove/me/..", "/remove"),
            ("/remove/me/../", "/remove"),
            ("./..", "../"),
            ("./../", "../"),
            ("../..", "../../"),
            ("../../", "../../"),
            ("/../", "/"),
        ] {
            let path = TextRequirer::normalize_path(input.as_ref());
            assert_eq!(
                &path,
                expected.as_ref() as &Path,
                "wrong normalization for {input}"
            );
        }
    }
}
