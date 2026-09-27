use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub(crate) fn read(path: impl AsRef<Path>) -> Option<Cow<'static, [u8]>> {
    let path = path.as_ref();
    if let Some(key) = archive_key(path) {
        return archive::get(&key).map(Cow::Borrowed);
    }
    #[cfg(packed_assets)]
    {
        None
    }
    #[cfg(not(packed_assets))]
    std::fs::read(path).ok().map(Cow::Owned)
}

pub(crate) fn read_to_string(path: impl AsRef<Path>) -> Option<Cow<'static, str>> {
    let path = path.as_ref();
    if let Some(key) = archive_key(path) {
        return std::str::from_utf8(archive::get(&key)?)
            .ok()
            .map(Cow::Borrowed);
    }
    #[cfg(packed_assets)]
    {
        None
    }
    #[cfg(not(packed_assets))]
    std::fs::read_to_string(path).ok().map(Cow::Owned)
}

pub(crate) fn read_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let dir = dir.as_ref();
    if let Some(prefix) = archive_key(dir) {
        return archive::children(&prefix)
            .map(|name| dir.join(name))
            .collect();
    }
    #[cfg(packed_assets)]
    {
        Vec::new()
    }
    #[cfg(not(packed_assets))]
    {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.path())
            .collect()
    }
}

pub(crate) fn open_image(path: impl AsRef<Path>) -> Result<image::DynamicImage, image::ImageError> {
    let path = path.as_ref();
    let bytes = read(path).ok_or_else(|| {
        image::ImageError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not in the assets", path.display()),
        ))
    })?;
    image::load_from_memory(&bytes)
}

pub(crate) fn walk(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let dir = dir.as_ref();
    if let Some(prefix) = archive_key(dir) {
        return archive::descendants(&prefix)
            .map(|rest| dir.join(rest))
            .collect();
    }
    #[cfg(packed_assets)]
    {
        Vec::new()
    }
    #[cfg(not(packed_assets))]
    {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(next) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&next) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                match entry.file_type() {
                    Ok(t) if t.is_dir() => stack.push(path),
                    Ok(t) if t.is_file() => out.push(path),
                    _ => {}
                }
            }
        }
        out
    }
}

fn archive_key(path: &Path) -> Option<String> {
    if !archive::active() {
        return None;
    }
    key(path)
}

fn key(path: &Path) -> Option<String> {
    let slashes = |p: &Path| p.to_string_lossy().replace('\\', "/");
    if let Ok(relative) = path.strip_prefix(crate::assets_root()) {
        return Some(format!("assets/minecraft/{}", slashes(relative)));
    }
    let relative = path.strip_prefix(crate::datapack_root()).ok()?;
    Some(format!("data/minecraft/{}", slashes(relative)))
}

pub(crate) mod archive {
    use std::collections::{BTreeSet, HashMap};
    use std::sync::OnceLock;

    static LAYERS: OnceLock<Vec<HashMap<String, &'static [u8]>>> = OnceLock::new();

    #[cfg_attr(not(packed_assets), allow(dead_code))]
    pub(crate) fn load(packed: Vec<u8>) -> Result<(), String> {
        install(vec![super::unpack(packed)?])
    }

    #[cfg_attr(
        all(not(packed_assets), not(feature = "asset_download")),
        allow(dead_code)
    )]
    pub(crate) fn install(layers: Vec<HashMap<String, &'static [u8]>>) -> Result<(), String> {
        LAYERS
            .set(layers)
            .map_err(|_| "assets already loaded".to_owned())
    }

    pub(crate) fn active() -> bool {
        LAYERS.get().is_some_and(|l| !l.is_empty())
    }

    pub(crate) fn get(key: &str) -> Option<&'static [u8]> {
        LAYERS
            .get()?
            .iter()
            .find_map(|layer| layer.get(key))
            .copied()
    }

    pub(crate) fn children(prefix: &str) -> impl Iterator<Item = &'static str> {
        descendants(prefix).filter(|rest| !rest.contains('/'))
    }

    pub(crate) fn descendants(prefix: &str) -> impl Iterator<Item = &'static str> {
        let prefix = format!("{}/", prefix.trim_end_matches('/'));
        LAYERS
            .get()
            .into_iter()
            .flatten()
            .flat_map(|files| files.keys())
            .filter_map(|k| k.strip_prefix(&prefix))
            .collect::<BTreeSet<_>>()
            .into_iter()
    }
}

pub(crate) const ARCHIVE_MAGIC: &[u8; 8] = b"MCASSET2";

#[cfg_attr(
    all(not(packed_assets), not(feature = "asset_download")),
    allow(dead_code)
)]
pub(crate) fn unpack(
    packed: Vec<u8>,
) -> Result<std::collections::HashMap<String, &'static [u8]>, String> {
    let len = packed.len();
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<std::ops::Range<usize>, String> {
        let end = at
            .checked_add(n)
            .filter(|e| *e <= len)
            .ok_or_else(|| "the asset archive ends mid-record".to_owned())?;
        let range = at..end;
        at = end;
        Ok(range)
    };
    if packed[take(8)?] != ARCHIVE_MAGIC[..] {
        return Err("this is not an asset archive (bad magic)".to_owned());
    }
    let count = u32::from_le_bytes(packed[take(4)?].try_into().unwrap()) as usize;
    let mut table = Vec::with_capacity(count);
    for _ in 0..count {
        let path_len = u32::from_le_bytes(packed[take(4)?].try_into().unwrap()) as usize;
        let path = String::from_utf8(packed[take(path_len)?].to_vec())
            .map_err(|_| "an asset path is not utf-8".to_owned())?;
        let blob_len = u32::from_le_bytes(packed[take(4)?].try_into().unwrap()) as usize;
        table.push((path, blob_len));
    }
    let mut blobs = Vec::with_capacity(table.len());
    for (_, blob_len) in &table {
        blobs.push(take(*blob_len)?);
    }

    let bytes: &'static [u8] = Box::leak(packed.into_boxed_slice());
    let mut files = std::collections::HashMap::with_capacity(count);
    for ((path, _), blob) in table.into_iter().zip(blobs) {
        files.insert(path, &bytes[blob]);
    }
    Ok(files)
}

#[cfg(target_os = "android")]
pub(crate) mod android {
    use bevy::android::android_activity::AndroidApp;

    const ARCHIVE: &str = "assets.bin";
    const LAUNCH: &str = "launch.txt";
    const ENV: &str = "env.txt";

    fn entry(app: &AndroidApp, name: &str) -> Option<Vec<u8>> {
        let name = std::ffi::CString::new(name).ok()?;
        let mut asset = app.asset_manager().open(&name)?;
        asset.buffer().ok().map(<[u8]>::to_vec)
    }

    pub(crate) fn read_apk_archive(app: &AndroidApp) -> Result<Vec<u8>, String> {
        entry(app, ARCHIVE).ok_or_else(|| format!("{ARCHIVE} is not in the apk"))
    }

    pub(crate) fn launch_target(app: &AndroidApp) -> (Option<String>, Option<String>) {
        let Some(text) = entry(app, LAUNCH).and_then(|b| String::from_utf8(b).ok()) else {
            return (None, None);
        };
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let address = lines.next().map(str::to_owned);
        let username = lines.next().map(str::to_owned);
        (username, address)
    }

    pub(crate) fn apply_env(app: &AndroidApp) {
        let Some(text) = entry(app, ENV).and_then(|b| String::from_utf8(b).ok()) else {
            return;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            if key.is_empty() {
                continue;
            }
            crate::step(&format!("env: {key}={value}"));
            unsafe { std::env::set_var(key, value) };
        }
    }
}
