use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::platform::packfs::{self, Kind};
use crate::util::zip;

pub(crate) enum Source {
    Directory {
        root: PathBuf,
    },
    Archive {
        bytes: Arc<Vec<u8>>,
        entries: Vec<zip::Entry>,
        index: HashMap<String, usize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pack {
    pub(crate) name: String,
}

pub(crate) fn scan(dir: &Path) -> Vec<Pack> {
    let Ok(entries) = packfs::entries(dir) else {
        return Vec::new();
    };

    let mut packs: Vec<Pack> = entries
        .into_iter()
        .filter_map(|(name, kind)| {
            if kind == Kind::File && !is_zip(&name) {
                return None;
            }

            if name.starts_with('.') {
                return None;
            }

            Some(Pack { name })
        })
        .collect();

    packs.sort_by_cached_key(|pack| pack.name.to_lowercase());
    packs
}

fn is_zip(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".zip")
}

impl Source {
    pub(crate) fn open(path: &Path, root: &str) -> Result<Source, String> {
        let missing = || format!("the pack has no {root} directory");
        if packfs::is_dir(path) {
            let root = path.join(root);
            if !packfs::is_dir(&root) {
                return Err(missing());
            }
            let root = packfs::canonicalize(&root).map_err(|e| e.to_string())?;
            return Ok(Source::Directory { root });
        }

        Source::from_shared_zip(packfs::shared(path).map_err(|e| e.to_string())?, root)
    }

    #[cfg(resource_packs)]
    pub(crate) fn from_zip(bytes: Vec<u8>, root: &str) -> Result<Source, String> {
        Source::from_shared_zip(Arc::new(bytes), root)
    }

    pub(crate) fn from_shared_zip(bytes: Arc<Vec<u8>>, root: &str) -> Result<Source, String> {
        let missing = || format!("the pack has no {root} directory");
        let entries = zip::entries(&bytes)?;
        let prefix = if root.is_empty() {
            String::new()
        } else {
            archive_root(entries.iter().map(|entry| entry.name.as_str()), root)
                .ok_or_else(missing)?
        };

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
            Source::Directory { root } => packfs::read(confined(root, path)?).ok(),
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
        let mut bytes = self.read(path)?;
        bytes.drain(..bytes.len() - crate::platform::assets::skip_bom(&bytes).len());
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
            Source::Directory { root } => {
                confined(root, path).is_some_and(|at| packfs::is_file(&at))
            }
            Source::Archive { index, .. } => index.contains_key(path),
        }
    }

    #[cfg(resource_packs)]
    pub(crate) fn into_zip(self) -> Option<Vec<u8>> {
        match self {
            Source::Directory { .. } => None,
            Source::Archive { bytes, .. } => {
                Some(Arc::try_unwrap(bytes).unwrap_or_else(|shared| shared.to_vec()))
            }
        }
    }
}

fn archive_root<'a>(names: impl Iterator<Item = &'a str>, root: &str) -> Option<String> {
    let top = format!("{root}/");
    let nested = format!("/{root}/");
    names
        .filter_map(|name| {
            if name.starts_with(&top) {
                return Some(top.clone());
            }
            let at = name.find(&nested)?;
            Some(name[..at + nested.len()].to_owned())
        })
        .min_by_key(String::len)
}

fn walk(at: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(entries) = packfs::entries(at) else {
        return;
    };
    for (name, kind) in entries {
        let path = format!("{prefix}/{name}");
        match kind {
            Kind::Dir => walk(&at.join(&name), &path, out),
            _ => out.push(path),
        }
    }
}

fn resolve(root: &Path, path: &str) -> Option<PathBuf> {
    let mut out = root.to_path_buf();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        let mut components = Path::new(part).components();
        let plain = matches!(components.next(), Some(std::path::Component::Normal(_)))
            && components.next().is_none();
        if !plain || part.contains(['\\', ':']) {
            return None;
        }
        out.push(part);
    }
    Some(out)
}

fn confined(root: &Path, path: &str) -> Option<PathBuf> {
    let real = packfs::canonicalize(resolve(root, path)?).ok()?;
    real.starts_with(root).then_some(real)
}

#[cfg(any(resource_packs, feature = "shader_support"))]
pub(crate) struct Failed(std::sync::Mutex<Vec<String>>);

#[cfg(any(resource_packs, feature = "shader_support"))]
impl Failed {
    pub(crate) const fn new() -> Failed {
        Failed(std::sync::Mutex::new(Vec::new()))
    }

    pub(crate) fn set(&self, names: Vec<String>) {
        if let Ok(mut failed) = self.0.lock() {
            *failed = names;
        }
    }

    pub(crate) fn get(&self) -> Vec<String> {
        self.0
            .lock()
            .map(|failed| failed.clone())
            .unwrap_or_default()
    }
}

#[cfg(any(resource_packs, feature = "shader_support"))]
#[derive(Default)]
pub(crate) struct Dropped {
    pub(crate) copied: bool,
    pub(crate) failed: bool,
    pub(crate) rejected: Vec<String>,
}

#[cfg(any(resource_packs, feature = "shader_support"))]
pub(crate) fn copy_in(paths: &[PathBuf], target: &Path, is_pack_dir: fn(&Path) -> bool) -> Dropped {
    let mut out = Dropped::default();
    if let Err(e) = packfs::create_dir_all(target) {
        crate::log_warn!("packs", "could not create {}: {e}", target.display());
        out.failed = true;
        return out;
    }
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_pack = match packfs::kind(path) {
            Some(Kind::Dir) => is_pack_dir(path),
            Some(Kind::File) => name.ends_with(".zip"),
            _ => false,
        };
        if !is_pack {
            crate::log_warn!("packs", "{} does not seem like a pack", path.display());
            out.rejected.push(name);
            continue;
        }
        out.copied = true;
        if let Err(e) = copy_tree(path, &target.join(&name)) {
            crate::log_warn!(
                "packs",
                "could not copy {} into {}: {e}",
                path.display(),
                target.display()
            );
            out.failed = true;
        }
    }
    out
}

#[cfg(any(resource_packs, feature = "shader_support", lua_mods))]
pub(crate) fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if packfs::exists(to) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} already exists", to.display()),
        ));
    }
    if packfs::kind(from) != Some(Kind::Dir) {
        return packfs::copy(from, to).map(drop);
    }
    packfs::create_dir(to)?;
    let mut first = None;
    for (name, _) in packfs::entries(from)? {
        if let Err(e) = copy_tree(&from.join(&name), &to.join(&name)) {
            first.get_or_insert(e);
        }
    }
    first.map_or(Ok(()), Err)
}

#[cfg(test)]
pub(crate) fn fixture(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("torch-client-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_pack_reads_by_pack_absolute_path() {
        let pack = fixture("dir-pack");
        std::fs::create_dir_all(pack.join("shaders/lib")).unwrap();
        std::fs::write(pack.join("shaders/lib/common.glsl"), "#define A 1").unwrap();

        let source = Source::open(&pack, "shaders").unwrap();
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
        assert!(Source::open(&pack, "shaders").is_err());
    }

    #[test]
    fn a_path_cannot_escape_the_pack() {
        let root = Path::new("/packs/x/shaders");
        assert!(resolve(root, "/../../.ssh/id_rsa").is_none());
        assert!(resolve(root, "/lib/../etc/passwd").is_none());
        assert!(resolve(root, "/C:/Windows/win.ini").is_none());
        assert!(resolve(root, "/lib\\..\\x").is_none());
        assert_eq!(
            resolve(root, "/lib/common.glsl"),
            Some(PathBuf::from("/packs/x/shaders/lib/common.glsl"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_pack_is_not_followed() {
        let pack = fixture("link-pack");
        std::fs::create_dir_all(pack.join("shaders")).unwrap();
        let outside = fixture("link-target").join("secret.txt");
        std::fs::write(&outside, "secret").unwrap();
        std::os::unix::fs::symlink(&outside, pack.join("shaders/leak.glsl")).unwrap();
        std::fs::write(pack.join("shaders/own.glsl"), "own").unwrap();

        let source = Source::open(&pack, "shaders").unwrap();
        assert_eq!(source.read_text("/leak.glsl"), None);
        assert!(!source.contains("/leak.glsl"));
        assert_eq!(source.read_text("/own.glsl").as_deref(), Some("own"));
    }

    #[test]
    fn the_shaders_root_is_found_under_a_wrapper() {
        let root = |names: &[&str]| archive_root(names.iter().copied(), "shaders");
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

    fn touch(dir: &std::path::Path, name: &str) {
        std::fs::write(dir.join(name), b"").unwrap();
    }

    #[test]
    fn a_missing_directory_is_an_empty_list() {
        let dir = fixture("missing").join("not-created");
        assert_eq!(scan(&dir), Vec::new());
    }

    #[test]
    fn zips_and_directories_are_both_packs() {
        let dir = fixture("both");
        touch(&dir, "Solas Shader V3.7b.zip");
        std::fs::create_dir(dir.join("BSL")).unwrap();

        let packs = scan(&dir);
        assert_eq!(
            packs,
            vec![
                Pack { name: "BSL".into() },
                Pack {
                    name: "Solas Shader V3.7b.zip".into()
                },
            ]
        );
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_not_a_pack() {
        let dir = fixture("stray-files");
        touch(&dir, "Solas Shader V3.7b.zip.txt");
        touch(&dir, "notes.md");
        touch(&dir, "zip");
        assert_eq!(scan(&dir), Vec::new());
    }

    #[test]
    fn the_extension_is_matched_whatever_its_case() {
        let dir = fixture("case");
        touch(&dir, "Loud.ZIP");
        assert_eq!(scan(&dir).len(), 1);
    }

    #[test]
    fn a_name_in_another_script_is_not_cut_mid_character() {
        assert!(!is_zip("テクスチャ"));
        assert!(is_zip("パック.zip"));
        assert!(!is_zip(".zip"));
    }

    #[test]
    fn an_options_document_is_not_a_pack() {
        let dir = fixture("options-doc");
        touch(&dir, "Solas Shader V3.7b.zip");
        touch(&dir, "Solas Shader V3.7b.zip.txt");
        assert_eq!(scan(&dir).len(), 1);
    }

    #[test]
    fn dotfiles_are_skipped() {
        let dir = fixture("dotfiles");
        touch(&dir, ".DS_Store");
        std::fs::create_dir(dir.join(".git")).unwrap();
        assert_eq!(scan(&dir), Vec::new());
    }

    #[test]
    fn the_list_sorts_without_regard_to_case() {
        let dir = fixture("sorting");
        for name in ["zebra.zip", "Apple.zip", "banana.zip"] {
            touch(&dir, name);
        }
        let names: Vec<String> = scan(&dir).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Apple.zip", "banana.zip", "zebra.zip"]);
    }
}
