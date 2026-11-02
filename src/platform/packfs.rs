#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    File,
    Dir,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Other,
}

#[cfg(all(resource_packs, not(target_arch = "wasm32")))]
pub(crate) type Stamp = (u64, std::time::SystemTime);
#[cfg(all(resource_packs, target_arch = "wasm32"))]
pub(crate) type Stamp = (u64, u64);

#[cfg(not(target_arch = "wasm32"))]
#[allow(unused_imports)]
pub(crate) use std::fs::{
    canonicalize, copy, create_dir, create_dir_all, read, read_to_string, remove_dir,
    remove_dir_all, remove_file, rename, write,
};

#[cfg(all(lua_mods, not(target_arch = "wasm32")))]
pub(crate) fn append(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(bytes)
}

#[cfg(all(lua_mods, not(target_arch = "wasm32")))]
pub(crate) fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

#[cfg(all(lua_mods, not(target_arch = "wasm32")))]
pub(crate) fn modified(path: &Path) -> Option<crate::platform::time::SystemTime> {
    std::fs::symlink_metadata(path).ok()?.modified().ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn root() -> PathBuf {
    crate::platform::storage::dir()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn shared(path: &Path) -> std::io::Result<Arc<Vec<u8>>> {
    std::fs::read(path).map(Arc::new)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn is_dir(path: &Path) -> bool {
    path.is_dir()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn is_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn kind(path: &Path) -> Option<Kind> {
    std::fs::symlink_metadata(path)
        .ok()
        .map(|meta| kind_of(meta.file_type()))
}

#[cfg(not(target_arch = "wasm32"))]
fn kind_of(file_type: std::fs::FileType) -> Kind {
    if file_type.is_file() {
        Kind::File
    } else if file_type.is_dir() {
        Kind::Dir
    } else {
        Kind::Other
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn entries(dir: &Path) -> std::io::Result<Vec<(String, Kind)>> {
    Ok(std::fs::read_dir(dir)?
        .flatten()
        .filter_map(|entry| {
            let kind = kind_of(entry.file_type().ok()?);
            Some((entry.file_name().into_string().ok()?, kind))
        })
        .collect())
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(explore_modrinth), allow(dead_code))]
pub(crate) fn len(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|meta| meta.len())
}

#[cfg(all(resource_packs, not(target_arch = "wasm32")))]
pub(crate) fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    meta.is_file().then_some(())?;
    Some((meta.len(), meta.modified().ok()?))
}

#[cfg(target_arch = "wasm32")]
pub(crate) use web::*;

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
mod web {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::{Error, ErrorKind, Result};
    use std::ops::Bound;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::Kind;

    type File = (u64, Arc<Vec<u8>>);

    static FILES: Mutex<BTreeMap<PathBuf, File>> = Mutex::new(BTreeMap::new());
    static DIRS: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn not_found(path: &Path) -> Error {
        Error::new(
            ErrorKind::NotFound,
            format!("{} is not in memory", path.display()),
        )
    }

    fn under<'a>(
        files: &'a BTreeMap<PathBuf, File>,
        dir: &'a Path,
    ) -> impl Iterator<Item = (&'a PathBuf, &'a File)> {
        files
            .range::<Path, _>((Bound::Excluded(dir), Bound::Unbounded))
            .take_while(move |(path, _)| path.starts_with(dir))
    }

    fn dirs_under<'a>(
        dirs: &'a BTreeSet<PathBuf>,
        dir: &'a Path,
    ) -> impl Iterator<Item = &'a PathBuf> {
        dirs.range::<Path, _>((Bound::Excluded(dir), Bound::Unbounded))
            .take_while(move |path| path.starts_with(dir))
    }

    fn moved(path: &Path, from: &Path, to: &Path) -> PathBuf {
        match path.strip_prefix(from) {
            Ok(rest) if rest.as_os_str().is_empty() => to.to_path_buf(),
            Ok(rest) => to.join(rest),
            Err(_) => path.to_path_buf(),
        }
    }

    pub(crate) fn root() -> PathBuf {
        PathBuf::new()
    }

    pub(crate) fn shared(path: &Path) -> Result<Arc<Vec<u8>>> {
        FILES
            .lock()
            .get(path)
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| not_found(path))
    }

    pub(crate) fn read(path: impl AsRef<Path>) -> Result<Vec<u8>> {
        shared(path.as_ref()).map(|bytes| bytes.to_vec())
    }

    pub(crate) fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
        String::from_utf8(read(path)?).map_err(|e| Error::new(ErrorKind::InvalidData, e))
    }

    pub(crate) fn write(path: impl AsRef<Path>, bytes: impl Into<Vec<u8>>) -> Result<()> {
        let path = path.as_ref();
        if is_dir(path) {
            return Err(Error::new(
                ErrorKind::IsADirectory,
                format!("{} is a directory", path.display()),
            ));
        }
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        FILES
            .lock()
            .insert(path.to_path_buf(), (serial, Arc::new(bytes.into())));
        Ok(())
    }

    pub(crate) fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<u64> {
        let bytes = shared(from.as_ref())?;
        let len = bytes.len() as u64;
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        FILES
            .lock()
            .insert(to.as_ref().to_path_buf(), (serial, bytes));
        Ok(len)
    }

    pub(crate) fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
        let (from, to) = (from.as_ref(), to.as_ref());
        let mut files = FILES.lock();
        if let Some(file) = files.remove(from) {
            files.insert(to.to_path_buf(), file);
            return Ok(());
        }
        let mut dirs = DIRS.lock();
        let paths: Vec<PathBuf> = under(&files, from).map(|(path, _)| path.clone()).collect();
        let mut made: Vec<PathBuf> = dirs_under(&dirs, from).cloned().collect();
        if dirs.remove(from) {
            made.push(from.to_path_buf());
        }
        if paths.is_empty() && made.is_empty() {
            return Err(not_found(from));
        }
        for path in paths {
            if let Some(file) = files.remove(&path) {
                files.insert(moved(&path, from, to), file);
            }
        }
        for path in made {
            dirs.remove(&path);
            dirs.insert(moved(&path, from, to));
        }
        Ok(())
    }

    pub(crate) fn remove_file(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        FILES
            .lock()
            .remove(path)
            .map(drop)
            .ok_or_else(|| not_found(path))
    }

    pub(crate) fn remove_dir(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let files = FILES.lock();
        let mut dirs = DIRS.lock();
        if files.contains_key(path) {
            return Err(Error::new(
                ErrorKind::NotADirectory,
                format!("{} is a file", path.display()),
            ));
        }
        if under(&files, path).next().is_some() || dirs_under(&dirs, path).next().is_some() {
            return Err(Error::new(
                ErrorKind::DirectoryNotEmpty,
                format!("{} is not empty", path.display()),
            ));
        }
        if dirs.remove(path) {
            Ok(())
        } else {
            Err(not_found(path))
        }
    }

    pub(crate) fn remove_dir_all(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        FILES.lock().retain(|at, _| !at.starts_with(path));
        DIRS.lock().retain(|at| !at.starts_with(path));
        Ok(())
    }

    pub(crate) fn create_dir_all(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(file) = path.ancestors().find(|at| is_file(at)) {
            return Err(Error::new(
                ErrorKind::NotADirectory,
                format!("{} is a file", file.display()),
            ));
        }
        let mut dirs = DIRS.lock();
        for at in path.ancestors().filter(|at| !at.as_os_str().is_empty()) {
            dirs.insert(at.to_path_buf());
        }
        Ok(())
    }

    pub(crate) fn create_dir(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if exists(path) {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!("{} already exists", path.display()),
            ));
        }
        DIRS.lock().insert(path.to_path_buf());
        Ok(())
    }

    #[cfg(lua_mods)]
    pub(crate) fn append(path: &Path, bytes: &[u8]) -> Result<()> {
        let mut whole = match shared(path) {
            Ok(old) => old.to_vec(),
            Err(_) => Vec::new(),
        };
        whole.extend_from_slice(bytes);
        write(path, whole)
    }

    #[cfg(lua_mods)]
    pub(crate) fn is_link(_: &Path) -> bool {
        false
    }

    #[cfg(lua_mods)]
    pub(crate) fn modified(_: &Path) -> Option<crate::platform::time::SystemTime> {
        None
    }

    pub(crate) fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();
        if exists(path) {
            Ok(path.to_path_buf())
        } else {
            Err(not_found(path))
        }
    }

    pub(crate) fn is_file(path: &Path) -> bool {
        FILES.lock().contains_key(path)
    }

    pub(crate) fn is_dir(path: &Path) -> bool {
        if under(&FILES.lock(), path).next().is_some() {
            return true;
        }
        let dirs = DIRS.lock();
        dirs.contains(path) || dirs_under(&dirs, path).next().is_some()
    }

    pub(crate) fn exists(path: &Path) -> bool {
        is_file(path) || is_dir(path)
    }

    pub(crate) fn kind(path: &Path) -> Option<Kind> {
        if is_file(path) {
            Some(Kind::File)
        } else if is_dir(path) {
            Some(Kind::Dir)
        } else {
            None
        }
    }

    pub(crate) fn entries(dir: &Path) -> Result<Vec<(String, Kind)>> {
        let files = FILES.lock();
        let mut out: Vec<(String, Kind)> = Vec::new();
        for (path, _) in under(&files, dir) {
            let Ok(rest) = path.strip_prefix(dir) else {
                continue;
            };
            let mut parts = rest.components();
            let Some(first) = parts.next() else { continue };
            let name = first.as_os_str().to_string_lossy().into_owned();
            let kind = if parts.next().is_some() {
                Kind::Dir
            } else {
                Kind::File
            };
            if out.last().is_none_or(|(last, _)| *last != name) {
                out.push((name, kind));
            }
        }
        let dirs = DIRS.lock();
        for path in dirs_under(&dirs, dir) {
            let Ok(rest) = path.strip_prefix(dir) else {
                continue;
            };
            let Some(first) = rest.components().next() else {
                continue;
            };
            let name = first.as_os_str().to_string_lossy().into_owned();
            if !out.iter().any(|(seen, _)| *seen == name) {
                out.push((name, Kind::Dir));
            }
        }
        Ok(out)
    }

    pub(crate) fn len(path: &Path) -> Option<u64> {
        FILES.lock().get(path).map(|(_, bytes)| bytes.len() as u64)
    }

    #[cfg(resource_packs)]
    pub(crate) fn stamp(path: &Path) -> Option<super::Stamp> {
        FILES
            .lock()
            .get(path)
            .map(|(serial, bytes)| (bytes.len() as u64, *serial))
    }
}
