use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub(crate) fn read(path: impl AsRef<Path>) -> Option<Cow<'static, [u8]>> {
    let path = path.as_ref();
    #[cfg(resource_packs)]
    if let Some(bytes) = pack_key(path).and_then(|key| archive::PACKS.get(&key)) {
        return Some(Cow::Borrowed(bytes));
    }
    read_base(path)
}

fn read_base(path: &Path) -> Option<Cow<'static, [u8]>> {
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
    utf8(read(path)?)
}

fn utf8(bytes: Cow<'static, [u8]>) -> Option<Cow<'static, str>> {
    match bytes {
        Cow::Borrowed(b) => std::str::from_utf8(skip_bom(b)).ok().map(Cow::Borrowed),
        Cow::Owned(mut v) => {
            v.drain(..v.len() - skip_bom(&v).len());
            String::from_utf8(v).ok().map(Cow::Owned)
        }
    }
}

pub(crate) fn skip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

pub(crate) fn read_stack(path: impl AsRef<Path>) -> Vec<Cow<'static, str>> {
    let path = path.as_ref();
    #[cfg_attr(not(resource_packs), allow(unused_mut))]
    let mut out: Vec<Cow<'static, str>> = read_base(path).and_then(utf8).into_iter().collect();
    #[cfg(resource_packs)]
    if let Some(key) = pack_key(path) {
        out.extend(
            archive::PACKS
                .stack(&key)
                .filter_map(|b| utf8(Cow::Borrowed(b))),
        );
    }
    out
}

pub(crate) fn read_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let dir = dir.as_ref();
    #[cfg_attr(not(resource_packs), allow(unused_mut))]
    let mut out = read_dir_base(dir);
    #[cfg(resource_packs)]
    add_pack_files(dir, &mut out, false);
    out
}

fn read_dir_base(dir: &Path) -> Vec<PathBuf> {
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

pub(crate) fn open_gui_image(
    path: impl AsRef<Path>,
) -> Result<image::RgbaImage, image::ImageError> {
    let path = path.as_ref();
    #[cfg_attr(not(resource_packs), allow(unused_mut))]
    let mut img = open_image(path)?.to_rgba8();
    #[cfg(resource_packs)]
    if let Some((w, h)) = read_base(path).and_then(|bytes| {
        image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?
            .into_dimensions()
            .ok()
    }) && (img.width() > w || img.height() > h)
    {
        img = image::imageops::resize(&img, w, h, image::imageops::FilterType::Nearest);
    }
    Ok(img)
}

pub(crate) fn walk(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let dir = dir.as_ref();
    #[cfg_attr(not(resource_packs), allow(unused_mut))]
    let mut out = walk_base(dir);
    #[cfg(resource_packs)]
    add_pack_files(dir, &mut out, true);
    out
}

fn walk_base(dir: &Path) -> Vec<PathBuf> {
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

#[cfg(resource_packs)]
fn pack_key(path: &Path) -> Option<String> {
    if !archive::PACKS.active() {
        return None;
    }
    key(path)
}

#[cfg(resource_packs)]
fn add_pack_files(dir: &Path, out: &mut Vec<PathBuf>, deep: bool) {
    let Some(prefix) = pack_key(dir) else { return };
    let listed: std::collections::HashSet<PathBuf> = out.iter().cloned().collect();
    let names: Vec<&'static str> = if deep {
        archive::PACKS.descendants(&prefix).collect()
    } else {
        archive::PACKS.children(&prefix).collect()
    };
    out.extend(
        names
            .into_iter()
            .map(|name| dir.join(name))
            .filter(|p| !listed.contains(p)),
    );
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

    pub(crate) type Layer = HashMap<String, &'static [u8]>;

    pub(crate) struct Stack(OnceLock<Vec<Layer>>);

    impl Stack {
        const fn new() -> Stack {
            Stack(OnceLock::new())
        }

        pub(crate) fn install(&self, layers: Vec<Layer>) -> Result<(), String> {
            self.0
                .set(layers)
                .map_err(|_| "assets already loaded".to_owned())
        }

        pub(crate) fn active(&self) -> bool {
            self.0.get().is_some_and(|l| !l.is_empty())
        }

        pub(crate) fn get(&self, key: &str) -> Option<&'static [u8]> {
            self.0
                .get()?
                .iter()
                .find_map(|layer| layer.get(key))
                .copied()
        }

        #[cfg_attr(not(resource_packs), allow(dead_code))]
        pub(crate) fn stack(&self, key: &str) -> impl Iterator<Item = &'static [u8]> {
            self.0
                .get()
                .into_iter()
                .flat_map(|layers| layers.iter().rev())
                .filter_map(move |layer| layer.get(key).copied())
        }

        pub(crate) fn children(&'static self, prefix: &str) -> impl Iterator<Item = &'static str> {
            self.descendants(prefix).filter(|rest| !rest.contains('/'))
        }

        pub(crate) fn descendants(
            &'static self,
            prefix: &str,
        ) -> impl Iterator<Item = &'static str> {
            let prefix = format!("{}/", prefix.trim_end_matches('/'));
            self.0
                .get()
                .into_iter()
                .flatten()
                .flat_map(|files| files.keys())
                .filter_map(|k| k.strip_prefix(&prefix))
                .collect::<BTreeSet<_>>()
                .into_iter()
        }
    }

    static BASE: Stack = Stack::new();

    #[cfg(resource_packs)]
    pub(crate) static PACKS: Stack = Stack::new();

    #[cfg_attr(not(packed_assets), allow(dead_code))]
    pub(crate) fn load(packed: Vec<u8>) -> Result<(), String> {
        install(vec![super::unpack(packed)?])
    }

    #[cfg_attr(
        all(not(packed_assets), not(feature = "asset_download")),
        allow(dead_code)
    )]
    pub(crate) fn install(layers: Vec<Layer>) -> Result<(), String> {
        BASE.install(layers)
    }

    pub(crate) fn active() -> bool {
        BASE.active()
    }

    pub(crate) fn get(key: &str) -> Option<&'static [u8]> {
        BASE.get(key)
    }

    pub(crate) fn children(prefix: &str) -> impl Iterator<Item = &'static str> {
        BASE.children(prefix)
    }

    pub(crate) fn descendants(prefix: &str) -> impl Iterator<Item = &'static str> {
        BASE.descendants(prefix)
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
    let (table, table_end) = read_table(&mut &packed[..])?;
    let mut at = table_end as usize;
    let mut blobs = Vec::with_capacity(table.len());
    for (_, blob_len) in &table {
        let end = at
            .checked_add(*blob_len as usize)
            .filter(|e| *e <= packed.len())
            .ok_or_else(|| "the asset archive ends mid-record".to_owned())?;
        blobs.push(at..end);
        at = end;
    }
    let count = table.len();

    let bytes: &'static [u8] = Box::leak(packed.into_boxed_slice());
    let mut files = std::collections::HashMap::with_capacity(count);
    for ((path, _), blob) in table.into_iter().zip(blobs) {
        files.insert(path, &bytes[blob]);
    }
    Ok(files)
}

#[cfg_attr(
    all(
        not(packed_assets),
        not(feature = "asset_download"),
        not(feature = "audio")
    ),
    allow(dead_code)
)]
pub(crate) fn read_table(
    from: &mut impl std::io::Read,
) -> Result<(Vec<(String, u32)>, u64), String> {
    use std::io::Read;

    const SHORT: &str = "the asset archive ends mid-record";
    fn u32_le(from: &mut impl Read) -> Result<u32, String> {
        let mut n = [0u8; 4];
        from.read_exact(&mut n).map_err(|_| SHORT.to_owned())?;
        Ok(u32::from_le_bytes(n))
    }

    let mut magic = [0u8; 8];
    from.read_exact(&mut magic).map_err(|_| SHORT.to_owned())?;
    if magic != *ARCHIVE_MAGIC {
        return Err("this is not an asset archive (bad magic)".to_owned());
    }
    let count = u32_le(from)? as usize;
    let mut table = Vec::with_capacity(count.min(1 << 16));
    let mut consumed = 12u64;
    for _ in 0..count {
        let key_len = u32_le(from)?;
        let mut key = Vec::new();
        from.by_ref()
            .take(u64::from(key_len))
            .read_to_end(&mut key)
            .map_err(|_| SHORT.to_owned())?;
        if key.len() != key_len as usize {
            return Err(SHORT.to_owned());
        }
        let key = String::from_utf8(key).map_err(|_| "an asset path is not utf-8".to_owned())?;
        let blob_len = u32_le(from)?;
        consumed += 8 + u64::from(key_len);
        table.push((key, blob_len));
    }
    Ok((table, consumed))
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
