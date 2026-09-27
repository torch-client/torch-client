use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::util::zip;

pub(crate) enum Source {
    Directory {
        root: PathBuf,
    },
    Archive {
        bytes: Vec<u8>,
        entries: Vec<zip::Entry>,
        index: HashMap<String, usize>,
    },
}

impl Source {
    pub(crate) fn open(path: &Path) -> Result<Source, String> {
        if path.is_dir() {
            let root = path.join("shaders");
            if !root.is_dir() {
                return Err("the pack has no shaders directory".into());
            }
            return Ok(Source::Directory { root });
        }

        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let entries = zip::entries(&bytes)?;
        let prefix = archive_root(entries.iter().map(|entry| entry.name.as_str()))
            .ok_or("the pack has no shaders directory")?;

        let index = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !entry.name.ends_with('/'))
            .filter_map(|(at, entry)| {
                let rest = entry.name.strip_prefix(&prefix)?;
                Some((format!("/{rest}"), at))
            })
            .collect();

        Ok(Source::Archive {
            bytes,
            entries,
            index,
        })
    }

    pub(crate) fn read(&self, path: &str) -> Option<Vec<u8>> {
        match self {
            Source::Directory { root } => {
                let at = resolve(root, path)?;
                std::fs::read(at).ok()
            }
            Source::Archive {
                bytes,
                entries,
                index,
            } => {
                let entry = entries.get(*index.get(path)?)?;
                zip::read(bytes, entry).ok()
            }
        }
    }

    pub(crate) fn read_text(&self, path: &str) -> Option<String> {
        let bytes = self.read(path)?;
        Some(match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
        })
    }

    pub(crate) fn files(&self) -> Vec<String> {
        match self {
            Source::Directory { root } => {
                let mut out = Vec::new();
                walk(root, "", &mut out);
                out
            }
            Source::Archive { index, .. } => index.keys().cloned().collect(),
        }
    }

    pub(crate) fn contains(&self, path: &str) -> bool {
        match self {
            Source::Directory { root } => resolve(root, path).is_some_and(|at| at.is_file()),
            Source::Archive { index, .. } => index.contains_key(path),
        }
    }
}

fn archive_root<'a>(names: impl Iterator<Item = &'a str>) -> Option<String> {
    names
        .filter_map(|name| {
            if name.starts_with("shaders/") {
                return Some("shaders/".to_owned());
            }
            let at = name.find("/shaders/")?;
            Some(name[..at + "/shaders/".len()].to_owned())
        })
        .min_by_key(String::len)
}

fn walk(at: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let path = format!("{prefix}/{name}");
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => walk(&entry.path(), &path, out),
            Ok(_) => out.push(path),
            Err(_) => {}
        }
    }
}

fn resolve(root: &Path, path: &str) -> Option<PathBuf> {
    let mut out = root.to_path_buf();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains('\\') {
            return None;
        }
        out.push(part);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("torch-client-tests").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_directory_pack_reads_by_pack_absolute_path() {
        let pack = fixture("dir-pack");
        std::fs::create_dir_all(pack.join("shaders/lib")).unwrap();
        std::fs::write(pack.join("shaders/lib/common.glsl"), "#define A 1").unwrap();

        let source = Source::open(&pack).unwrap();
        assert_eq!(
            source.read_text("/lib/common.glsl").as_deref(),
            Some("#define A 1")
        );
        assert!(source.contains("/lib/common.glsl"));
        assert!(!source.contains("/lib/missing.glsl"));
    }

    #[test]
    fn a_directory_without_shaders_is_not_a_pack() {
        let pack = fixture("no-shaders");
        std::fs::create_dir_all(pack.join("lib")).unwrap();
        assert!(Source::open(&pack).is_err());
    }

    #[test]
    fn a_path_cannot_escape_the_pack() {
        let root = Path::new("/packs/x/shaders");
        assert!(resolve(root, "/../../.ssh/id_rsa").is_none());
        assert!(resolve(root, "/lib/../etc/passwd").is_none());
        assert_eq!(
            resolve(root, "/lib/common.glsl"),
            Some(PathBuf::from("/packs/x/shaders/lib/common.glsl"))
        );
    }

    #[test]
    fn the_shaders_root_is_found_under_a_wrapper() {
        let root = |names: &[&str]| archive_root(names.iter().copied());
        assert_eq!(
            root(&["Solas/shaders/lib/a.glsl"]).as_deref(),
            Some("Solas/shaders/")
        );
        assert_eq!(
            root(&["shaders/lib/a.glsl", "docs/shaders/b.md"]).as_deref(),
            Some("shaders/")
        );
        assert_eq!(root(&["readme.txt"]), None);
    }
}
