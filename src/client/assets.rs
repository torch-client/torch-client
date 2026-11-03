use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::util::hash::sha1_hex;
use crate::util::zip;
use crate::{log_info, log_warn};

pub(super) const VERSION_MANIFEST: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

const OBJECT_STORE: &str = "https://resources.download.minecraft.net";

#[cfg(target_arch = "wasm32")]
const OBJECT_PROXY: &str = "https://getstuff.xpncvr.workers.dev/";

const NAMESPACE_DIR: &str = "assets/minecraft/";

const DATA_DIR: &str = "data/minecraft/";

const ASSET_KEY_PREFIX: &str = NAMESPACE_DIR;

pub(super) const NAMESPACE_PREFIX: &str = "minecraft/";

const PANORAMA_DIR: &str = "textures/gui/title/background/";

const UNIFONT_JSON_KEY: &str = "font/include/unifont.json";
const UNIFONT_ZIP_KEY: &str = "font/unifont.zip";

const PACK_ICON_SOURCE: &str = "pack.png";
const PACK_ICON_KEY: &str = "assets/minecraft/textures/gui/pack.png";

const META_KEY: &str = ".meta";

#[cfg(feature = "asset_download")]
const LANGUAGES_KEY: &str = ".languages";

const KEEP_LIST: &str = include_str!("../../tools/asset_keep_list.txt");

pub(crate) const CAN_FETCH_OBJECTS: bool = true;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn root() -> PathBuf {
    match std::env::var_os("MC_ASSET_DIR") {
        Some(p) => PathBuf::from(p),
        None => default_root(),
    }
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "windows"),
    not(target_os = "android")
))]
fn default_root() -> PathBuf {
    std::env::temp_dir().join("torch-client/assets")
}

#[cfg(target_os = "android")]
fn default_root() -> PathBuf {
    bevy::android::ANDROID_APP
        .get()
        .and_then(|app| app.internal_data_path())
        .unwrap_or_else(std::env::temp_dir)
        .join("assets")
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn default_root() -> PathBuf {
    use crate::platform::storage::WINDOWS_FOLDER as FOLDER;

    match std::env::var_os("LOCALAPPDATA").filter(|p| !p.is_empty()) {
        Some(local) => PathBuf::from(local).join(FOLDER).join("assets"),
        None => std::env::temp_dir().join(FOLDER).join("assets"),
    }
}

pub(crate) fn pack_root(version: &str) -> PathBuf {
    #[cfg(target_arch = "wasm32")]
    {
        PathBuf::from("/assets").join(version)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        root().join("versions").join(version)
    }
}

mod store {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) use native::*;
    #[cfg(target_arch = "wasm32")]
    pub(super) use web::*;

    #[cfg(not(target_arch = "wasm32"))]
    mod native {
        use std::path::PathBuf;

        fn version_pack(version: &str) -> PathBuf {
            super::super::root()
                .join("versions")
                .join(format!("{version}.bin"))
        }

        fn index_pack(id: &str) -> PathBuf {
            super::super::root()
                .join("indexes")
                .join(format!("{id}.bin"))
        }

        fn save(path: PathBuf, bytes: &[u8]) -> Result<(), String> {
            super::super::write_whole(&path, bytes)?;
            crate::log_info!("assets", "wrote {} ({} bytes)", path.display(), bytes.len());
            Ok(())
        }

        pub(in crate::client::assets) fn save_version(
            version: &str,
            bytes: &[u8],
        ) -> Result<(), String> {
            save(version_pack(version), bytes)
        }

        pub(in crate::client::assets) fn save_index(id: &str, bytes: &[u8]) -> Result<(), String> {
            save(index_pack(id), bytes)
        }

        pub(in crate::client::assets) fn index_keys(
            id: &str,
        ) -> Option<std::collections::HashSet<String>> {
            let path = index_pack(id);
            let len = std::fs::metadata(&path).ok()?.len();
            let mut file = std::io::BufReader::new(std::fs::File::open(&path).ok()?);
            let (table, blobs_at) = crate::platform::assets::read_table(&mut file).ok()?;
            let blobs: u64 = table.iter().map(|(_, n)| u64::from(*n)).sum();
            (blobs_at + blobs == len).then(|| table.into_iter().map(|(key, _)| key).collect())
        }

        pub(in crate::client::assets) fn load_version(version: &str) -> Option<Vec<u8>> {
            std::fs::read(version_pack(version)).ok()
        }

        pub(in crate::client::assets) fn load_index(id: &str) -> Option<Vec<u8>> {
            std::fs::read(index_pack(id)).ok()
        }
    }

    #[cfg(target_arch = "wasm32")]
    mod web {
        use std::sync::Mutex;

        pub(in crate::client::assets) static INSTALLED: Mutex<Option<Vec<u8>>> = Mutex::new(None);

        pub(in crate::client::assets) fn save_version(
            _version: &str,
            bytes: &[u8],
        ) -> Result<(), String> {
            *INSTALLED.lock().unwrap() = Some(bytes.to_vec());
            crate::log_info!("assets", "archive built, {} bytes", bytes.len());
            Ok(())
        }

        pub(in crate::client::assets) fn index_keys(
            _id: &str,
        ) -> Option<std::collections::HashSet<String>> {
            None
        }

        pub(in crate::client::assets) fn load_version(_version: &str) -> Option<Vec<u8>> {
            INSTALLED.lock().unwrap().clone()
        }

        pub(in crate::client::assets) fn load_index(_id: &str) -> Option<Vec<u8>> {
            None
        }
    }
}

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
pub(crate) fn clear_web_cache() -> Result<(), String> {
    use wasm_bindgen::{JsCast, JsValue};

    let window = web_sys::window().ok_or_else(|| "no window".to_owned())?;
    let f = js_sys::Reflect::get(&window, &JsValue::from_str("clearStoredAssets"))
        .map_err(|_| "the page defines no clearStoredAssets".to_owned())?;
    let f = f
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| "clearStoredAssets is not a function".to_owned())?;
    f.call0(&window)
        .map_err(|_| "clearStoredAssets threw".to_owned())?;
    log_info!("assets", "asked the page to clear the stored archive");
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn take_installed() -> Option<Vec<u8>> {
    store::INSTALLED.lock().unwrap().clone()
}

pub(crate) fn mount() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        MISSING.store(0, Ordering::Relaxed);
        finish_pending_clear();
    }

    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("MINECRAFT_ASSETS").is_some() {
        return false;
    }

    let version = crate::ASSET_VERSION;
    let Some(bytes) = store::load_version(version) else {
        return false;
    };
    let jar_layer = match crate::platform::assets::unpack(bytes) {
        Ok(layer) => layer,
        Err(e) => {
            log_warn!("assets", "the {version} archive is not readable: {e}");
            return false;
        }
    };

    let meta: serde_json::Value = jar_layer
        .get(META_KEY)
        .and_then(|b| serde_json::from_slice(b).ok())
        .unwrap_or(serde_json::Value::Null);
    let recorded = meta["version"].as_str().unwrap_or("");
    if recorded != version {
        log_warn!(
            "assets",
            "the archive holds {recorded:?}, not {version}; ignoring it"
        );
        return false;
    }

    #[cfg(feature = "asset_download")]
    match jar_layer.get(LANGUAGES_KEY) {
        Some(table) => super::language::adopt(table),
        None => log_info!(
            "assets",
            "the {version} archive lists no languages; English only"
        ),
    }

    let index = meta["index"].as_str();

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "audio")]
        if crate::platform::storage::SOUNDS_SKIPPED
            .load()
            .is_some_and(|s| !s.trim().is_empty())
        {
            SOUNDS_SKIPPED.store(true, Ordering::Relaxed);
        }
        let wanted = wanted_objects(|key| jar_layer.get(key).copied());
        WANTED.store(wanted, Ordering::Relaxed);
        *MOUNTED_INDEX.lock().unwrap() = index.map(str::to_owned);
        let missing = (wanted | wanted_sounds()) & !present(index);
        MISSING.store(missing, Ordering::Relaxed);

        if missing & piece::SOUNDS != 0 {
            log_info!(
                "assets",
                "audio is enabled but no sound set is installed; the download screen will fetch it"
            );
            return false;
        }
        if missing & piece::OBJECTS != 0 {
            log_warn!(
                "assets",
                "the objects layer lacks pieces this build wants ({missing:#b}); \
                 Manage Storage can fetch them"
            );
        }
    }

    let mut layers = Vec::with_capacity(2);
    if let Some(id) = index {
        match store::load_index(id)
            .ok_or_else(|| "not on disk".to_owned())
            .and_then(crate::platform::assets::unpack)
        {
            Ok(layer) => {
                log_info!("assets", "objects layer {id}: {} entries", layer.len());
                layers.push(layer);
            }
            Err(e) => {
                log_warn!("assets", "objects layer {id} unreadable: {e}");
                #[cfg(not(target_arch = "wasm32"))]
                MISSING.fetch_or(WANTED.load(Ordering::Relaxed), Ordering::Relaxed);
            }
        }
        #[cfg(feature = "audio")]
        crate::audio::init(id);
    }
    log_info!("assets", "jar layer {version}: {} entries", jar_layer.len());
    layers.push(jar_layer);

    if let Err(e) = crate::platform::assets::archive::install(layers) {
        log_warn!("assets", "could not mount the assets: {e}");
        return false;
    }
    #[cfg(not(target_arch = "wasm32"))]
    RESTART_PENDING.store(false, Ordering::Relaxed);
    true
}

mod piece {
    pub(super) const PANORAMA: u8 = 1;
    pub(super) const UNIFONT: u8 = 2;
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) const SOUNDS: u8 = 4;
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) const OBJECTS: u8 = PANORAMA | UNIFONT;
}

fn wanted_objects<'a>(get: impl Fn(&str) -> Option<&'a [u8]>) -> u8 {
    if !CAN_FETCH_OBJECTS {
        return 0;
    }
    let mut wanted = 0;
    if panorama_is_stubbed(&get) {
        wanted |= piece::PANORAMA;
    }
    if cfg!(feature = "full_font") && unifont_is_stubbed(&get) {
        wanted |= piece::UNIFONT;
    }
    wanted
}

#[cfg(not(target_arch = "wasm32"))]
fn wanted_sounds() -> u8 {
    #[cfg(feature = "audio")]
    if !SOUNDS_SKIPPED.load(Ordering::Relaxed) {
        return piece::SOUNDS;
    }
    0
}

#[cfg(not(target_arch = "wasm32"))]
fn present(index: Option<&str>) -> u8 {
    let Some(id) = index else {
        return 0;
    };
    #[cfg_attr(not(feature = "audio"), allow(unused_mut))]
    let mut held = store::index_keys(id).map_or(0, |keys| objects_hold(|key| keys.contains(key)));
    #[cfg(feature = "audio")]
    if crate::audio::store::installed(id) {
        held |= piece::SOUNDS;
    }
    held
}

fn objects_hold(has: impl Fn(&str) -> bool) -> u8 {
    let mut held = 0;
    if (0..6).all(|face| has(&panorama_face(face))) {
        held |= piece::PANORAMA;
    }
    if has(&format!("{ASSET_KEY_PREFIX}{UNIFONT_JSON_KEY}"))
        && has(&format!("{ASSET_KEY_PREFIX}{UNIFONT_ZIP_KEY}"))
    {
        held |= piece::UNIFONT;
    }
    held
}

fn object_wanted(want: u8, key: &str) -> bool {
    (want & piece::PANORAMA != 0 && key.starts_with(PANORAMA_DIR))
        || (want & piece::UNIFONT != 0 && (key == UNIFONT_JSON_KEY || key == UNIFONT_ZIP_KEY))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_whole(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;

    let parent = path.parent();
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let part = path.with_extension("part");
    let written = std::fs::File::create(&part)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&part, path));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&part);
        return Err(e.to_string());
    }
    #[cfg(unix)]
    if let Some(dir) = parent.and_then(|p| std::fs::File::open(p).ok()) {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn clear_native_cache() -> Result<(), String> {
    let result = delete_sets();
    if result.is_err() {
        let marker = root().join(CLEAR_MARKER);
        if let Err(e) = std::fs::write(&marker, b"") {
            log_warn!("assets", "could not leave {}: {e}", marker.display());
        }
    }
    result
}

#[cfg(not(target_arch = "wasm32"))]
const CLEAR_MARKER: &str = ".clear-pending";

#[cfg(not(target_arch = "wasm32"))]
fn finish_pending_clear() {
    let marker = root().join(CLEAR_MARKER);
    if !marker.is_file() {
        return;
    }
    log_info!("assets", "finishing the clear the last session started");
    match delete_sets() {
        Ok(()) => {
            let _ = std::fs::remove_file(&marker);
        }
        Err(e) => log_warn!("assets", "the clear is still not finished: {e}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn delete_sets() -> Result<(), String> {
    let mut removed = 0usize;
    let mut first_failure = None;
    for dir in ["versions", "indexes", "sounds"] {
        let Ok(entries) = std::fs::read_dir(root().join(dir)) else {
            continue;
        };
        for path in entries.flatten().map(|entry| entry.path()) {
            let ours = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e, "bin" | "json" | "part"));
            if !ours || !path.is_file() {
                continue;
            }
            match std::fs::remove_file(&path) {
                Ok(()) => removed += 1,
                Err(e) => {
                    log_warn!("assets", "could not delete {}: {e}", path.display());
                    first_failure.get_or_insert_with(|| format!("{}: {e}", path.display()));
                }
            }
        }
    }
    log_info!(
        "assets",
        "cleared {removed} files under {}",
        root().display()
    );
    first_failure.map_or(Ok(()), Err)
}

fn jar_key(name: &str) -> Option<String> {
    let inside = name.starts_with(NAMESPACE_DIR) || name.starts_with(DATA_DIR);
    inside.then(|| name.to_owned())
}

const REPORT_STEP: u64 = 4 * 1024 * 1024;

fn progress_line(done: u64, total: u64) -> String {
    use crate::util::bytes::Bytes;
    if total == 0 {
        return Bytes(done).to_string();
    }
    format!(
        "{} of {} ({}%)",
        Bytes(done),
        Bytes(total),
        done * 100 / total
    )
}

fn object_url(hash: &str) -> String {
    let direct = format!("{OBJECT_STORE}/{}/{hash}", &hash[..2]);
    #[cfg(target_arch = "wasm32")]
    {
        format!("{OBJECT_PROXY}{direct}")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        direct
    }
}

fn kept(key: &str) -> bool {
    KEEP_LIST
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .any(|rule| match rule.strip_suffix('/') {
            Some(dir) => key.starts_with(dir) && key.as_bytes().get(dir.len()) == Some(&b'/'),
            None => key == rule,
        })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Stage {
    Idle,
    Asking,
    Downloading,
    Extracting,
    Fetching,
    Sounds,
    Done,
    Cancelled,
    Failed,
}

impl Stage {
    fn code(self) -> u8 {
        match self {
            Stage::Idle => 0,
            Stage::Asking => 1,
            Stage::Downloading => 2,
            Stage::Extracting => 3,
            Stage::Fetching => 4,
            Stage::Done => 5,
            Stage::Cancelled => 6,
            Stage::Failed => 7,
            Stage::Sounds => 8,
        }
    }

    fn from_code(c: u8) -> Stage {
        match c {
            1 => Stage::Asking,
            2 => Stage::Downloading,
            3 => Stage::Extracting,
            4 => Stage::Fetching,
            5 => Stage::Done,
            6 => Stage::Cancelled,
            7 => Stage::Failed,
            8 => Stage::Sounds,
            _ => Stage::Idle,
        }
    }
}

pub(crate) struct Download {
    stage: AtomicU8,
    done: AtomicU64,
    total: AtomicU64,
    cancel: AtomicBool,
    error: Mutex<&'static str>,
    sound_done: AtomicU64,
    sound_total: AtomicU64,
    sound_active: AtomicBool,
    sound_skip: AtomicBool,
}

pub(crate) fn download() -> &'static Download {
    static D: OnceLock<Download> = OnceLock::new();
    D.get_or_init(|| Download {
        stage: AtomicU8::new(0),
        done: AtomicU64::new(0),
        total: AtomicU64::new(0),
        cancel: AtomicBool::new(false),
        error: Mutex::new(""),
        sound_done: AtomicU64::new(0),
        sound_total: AtomicU64::new(0),
        sound_active: AtomicBool::new(false),
        sound_skip: AtomicBool::new(false),
    })
}

#[cfg(not(target_arch = "wasm32"))]
static MISSING: AtomicU8 = AtomicU8::new(0);

#[cfg(not(target_arch = "wasm32"))]
static WANTED: AtomicU8 = AtomicU8::new(0);

#[cfg(not(target_arch = "wasm32"))]
static MOUNTED_INDEX: Mutex<Option<String>> = Mutex::new(None);

#[cfg(not(target_arch = "wasm32"))]
static RESTART_PENDING: AtomicBool = AtomicBool::new(false);

#[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
static SOUNDS_FETCHED: AtomicBool = AtomicBool::new(false);

#[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
static SOUNDS_SKIPPED: AtomicBool = AtomicBool::new(false);

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn check_missing() -> bool {
    #[cfg(feature = "audio")]
    {
        SOUNDS_SKIPPED.store(false, Ordering::Relaxed);
        crate::platform::storage::SOUNDS_SKIPPED.store("");
    }
    if !crate::platform::assets::archive::active() {
        return false;
    }
    let index = MOUNTED_INDEX.lock().unwrap().clone();
    let missing = (WANTED.load(Ordering::Relaxed) | wanted_sounds()) & !present(index.as_deref());
    MISSING.store(missing, Ordering::Relaxed);
    if missing != 0 {
        log_info!(
            "assets",
            "pieces missing ({missing:#b}); offering them again"
        );
        download().rewind();
    }
    missing != 0
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn check_missing() -> bool {
    false
}

pub(crate) fn adopt_sounds() {
    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    if SOUNDS_FETCHED.swap(false, Ordering::Relaxed)
        && let Some(id) = MOUNTED_INDEX.lock().unwrap().clone()
    {
        crate::audio::init(&id);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn needs_restart() -> bool {
    RESTART_PENDING.load(Ordering::Relaxed)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn needs_restart() -> bool {
    false
}

impl Download {
    pub(crate) fn stage(&self) -> Stage {
        Stage::from_code(self.stage.load(Ordering::Relaxed))
    }

    pub(crate) fn progress(&self) -> (u64, u64) {
        (
            self.done.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn error(&self) -> &'static str {
        *self.error.lock().unwrap()
    }

    pub(crate) fn sound_progress(&self) -> (u64, u64) {
        (
            self.sound_done.load(Ordering::Relaxed),
            self.sound_total.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn sounds_running(&self) -> bool {
        cfg!(all(feature = "audio", not(target_arch = "wasm32")))
            && self.sound_active.load(Ordering::Relaxed)
            && !self.sound_skip.load(Ordering::Relaxed)
    }

    pub(crate) fn skip_sounds(&self) {
        self.sound_skip.store(true, Ordering::Relaxed);
    }

    fn set(&self, stage: Stage) {
        self.stage.store(stage.code(), Ordering::Relaxed);
    }

    fn fail(&self, short: &'static str, detail: impl std::fmt::Display) {
        log_warn!("assets", "download failed: {detail}");
        *self.error.lock().unwrap() = short;
        self.set(Stage::Failed);
    }

    pub(crate) fn report_failure(&self, short: &'static str, detail: impl std::fmt::Display) {
        self.fail(short, detail);
    }

    pub(crate) fn start(&'static self, version: String) {
        if matches!(
            self.stage(),
            Stage::Asking
                | Stage::Downloading
                | Stage::Extracting
                | Stage::Fetching
                | Stage::Sounds
        ) {
            return;
        }
        self.cancel.store(false, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        self.sound_done.store(0, Ordering::Relaxed);
        self.sound_total.store(0, Ordering::Relaxed);
        self.sound_skip.store(false, Ordering::Relaxed);
        *self.error.lock().unwrap() = "";
        self.set(Stage::Asking);

        #[cfg(target_arch = "wasm32")]
        crate::platform::executor::spawn(async move { self.run(&version).await });

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::Builder::new()
            .name("asset-download".into())
            .spawn(move || crate::platform::executor::block_on(self.run(&version)))
            .ok();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn rewind(&self) {
        if matches!(self.stage(), Stage::Done | Stage::Cancelled | Stage::Failed) {
            self.set(Stage::Idle);
        }
    }

    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    async fn run(&'static self, version: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let missing = MISSING.load(Ordering::Relaxed);
            if missing != 0 {
                let result = self.repair(version, missing).await;
                self.settle(result, "the missing pieces");
                return;
            }
        }
        let result = self.install(version).await;
        self.settle(result, version);
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn repair(&'static self, version: &str, missing: u8) -> Result<bool, Failure> {
        let mounted = MOUNTED_INDEX.lock().unwrap().clone();
        self.set(Stage::Asking);
        let manifest = get_json(VERSION_MANIFEST).await?;
        let meta = get_json(&pick_version(&manifest, version)?).await?;
        let index_url = meta["assetIndex"]["url"]
            .as_str()
            .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;
        let index_id = meta["assetIndex"]["id"]
            .as_str()
            .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;

        if mounted.as_deref() != Some(index_id) {
            log_warn!(
                "assets",
                "the installed set names index {mounted:?} and Mojang names {index_id}; \
                 installing the whole set"
            );
            RESTART_PENDING.store(true, Ordering::Relaxed);
            return self.install(version).await;
        }

        if missing & piece::OBJECTS != 0 {
            let want = WANTED.load(Ordering::Relaxed);
            let index = get_json(index_url).await?;
            let Some(objects) = self
                .fetch_objects(&index, |key| object_wanted(want, key))
                .await?
            else {
                return Ok(false);
            };
            store::save_index(index_id, &pack(objects))
                .map_err(|e| Failure::new("Cannot write to the assets folder", e))?;
            RESTART_PENDING.store(true, Ordering::Relaxed);
        }

        #[cfg(feature = "audio")]
        if missing & piece::SOUNDS != 0 {
            self.set(Stage::Sounds);
            let fetched = self.install_sounds(index_url, index_id).await;
            if !self.settle_sounds(fetched)? {
                return Ok(false);
            }
            if crate::audio::store::installed(index_id) {
                SOUNDS_FETCHED.store(true, Ordering::Relaxed);
            }
        }
        Ok(true)
    }

    fn settle(&self, result: Result<bool, Failure>, what: &str) {
        match result {
            Ok(true) => {
                log_info!("assets", "{what} installed");
                self.set(Stage::Done);
            }
            Ok(false) => {
                log_info!("assets", "download cancelled");
                self.done.store(0, Ordering::Relaxed);
                self.total.store(0, Ordering::Relaxed);
                self.set(Stage::Cancelled);
            }
            Err(e) => self.fail(e.short, e.detail),
        }
    }

    async fn install(&'static self, version: &str) -> Result<bool, Failure> {
        log_info!("assets", "asking Mojang about {version}");
        self.set(Stage::Asking);
        let manifest = get_json(VERSION_MANIFEST).await?;
        let meta = get_json(&pick_version(&manifest, version)?).await?;

        #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
        let sounds = SoundTask::spawn(self, &meta)?;

        let client = &meta["downloads"]["client"];
        let url = client["url"]
            .as_str()
            .ok_or_else(|| Failure::plain("Mojang sent no download link"))?;
        let sha1 = client["sha1"].as_str().unwrap_or("");
        let size = client["size"].as_u64().unwrap_or(0);

        let Some(jar) = self.fetch_jar(url, sha1, size).await? else {
            return Ok(false);
        };

        let Some(mut entries) = self.collect(&jar)? else {
            return Ok(false);
        };
        drop(jar);

        let wants_objects = wanted_objects(|key| {
            entries
                .iter()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, bytes)| bytes.as_slice())
        });
        let wants_sounds = cfg!(feature = "audio");
        let wants_languages = cfg!(feature = "asset_download");
        let mut index_id = None;
        if CAN_FETCH_OBJECTS && (wants_objects != 0 || wants_sounds || wants_languages) {
            let id = meta["assetIndex"]["id"]
                .as_str()
                .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;
            let index_url = meta["assetIndex"]["url"]
                .as_str()
                .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;
            let index = get_json(index_url).await?;

            #[cfg(feature = "asset_download")]
            match super::language::table_from_index(&index, &|| self.cancelled()).await {
                Ok(Some(table)) => entries.push((LANGUAGES_KEY.to_owned(), table)),
                Ok(None) => return Ok(false),
                Err(e) => log_warn!("assets", "no language list: {}", e.detail),
            }

            if wants_objects != 0 {
                let held =
                    store::index_keys(id).map_or(0, |keys| objects_hold(|key| keys.contains(key)));
                if wants_objects & !held == 0 {
                    log_info!("assets", "objects layer {id} is already installed");
                } else {
                    let wanted = |key: &str| object_wanted(wants_objects, key);
                    let objects = match self.fetch_objects(&index, wanted).await {
                        Ok(Some(objects)) => objects,
                        Ok(None) => return Ok(false),
                        #[cfg(target_arch = "wasm32")]
                        Err(e) => {
                            log_warn!("assets", "no objects layer: {}", e.detail);
                            Vec::new()
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        Err(e) => return Err(e),
                    };
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let packed = pack(objects);
                        store::save_index(id, &packed)
                            .map_err(|e| Failure::new("Cannot write to the assets folder", e))?;
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let count = objects.len();
                        for (key, bytes) in objects {
                            match entries.iter().position(|(k, _)| *k == key) {
                                Some(at) => entries[at].1 = bytes,
                                None => entries.push((key, bytes)),
                            }
                        }
                        log_info!("assets", "folded {count} objects into the archive");
                    }
                }
            }
            index_id = (!cfg!(target_arch = "wasm32")).then(|| id.to_owned());
        }

        #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
        if !sounds.finish()? {
            return Ok(false);
        }

        entries.push((META_KEY.to_owned(), meta_blob(version, index_id.as_deref())));
        let packed = pack(entries);
        store::save_version(version, &packed)
            .map_err(|e| Failure::new("Cannot write to the assets folder", e))?;
        Ok(true)
    }

    async fn fetch_jar(
        &self,
        url: &str,
        sha1: &str,
        size: u64,
    ) -> Result<Option<Vec<u8>>, Failure> {
        log_info!("assets", "downloading {size} bytes from {url}");
        self.total.store(size, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        self.set(Stage::Downloading);

        let mut reported = 0u64;
        let mut progress = |done: u64, total: u64| {
            if done < reported {
                reported = 0;
            }
            self.done.store(done, Ordering::Relaxed);
            if total > 0 {
                self.total.store(total, Ordering::Relaxed);
            }
            if done >= reported + REPORT_STEP {
                reported = done;
                log_info!("assets", "jar: {}", progress_line(done, total.max(size)));
            }
            !self.cancelled()
        };
        let body = match crate::platform::http::get_bytes(url, size, &mut progress).await {
            Ok(body) => Ok(body),
            Err(e) => match fallback_url(url) {
                Some(proxied) => {
                    log_warn!("assets", "jar: {e}; retrying through the proxy");
                    crate::platform::http::get_bytes(&proxied, size, &mut progress).await
                }
                None => Err(e),
            },
        }
        .map_err(|e| Failure::http("The download was interrupted", e))?;

        let bytes = match body {
            crate::platform::http::Fetched::Body(bytes) => bytes,
            crate::platform::http::Fetched::Cancelled => return Ok(None),
        };

        let got = sha1_hex(&bytes);
        if !sha1.is_empty() && got != sha1 {
            return Err(Failure::new(
                "The download arrived damaged",
                format!("sha1 {got}, expected {sha1}"),
            ));
        }
        Ok(Some(bytes))
    }

    fn collect(&self, jar: &[u8]) -> Result<Option<Vec<(String, Vec<u8>)>>, Failure> {
        let entries =
            zip::entries(jar).map_err(|e| Failure::new("The jar is not a readable archive", e))?;

        let count = entries.len();
        self.total.store(count as u64, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        self.set(Stage::Extracting);
        log_info!("assets", "reading {count} entries");

        let mut out = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            if self.cancelled() {
                return Ok(None);
            }
            self.done.store(i as u64, Ordering::Relaxed);

            let Some(key) = jar_key(&entry.name) else {
                continue;
            };
            if key.is_empty() || key.ends_with('/') || !kept(&key) {
                continue;
            }
            let bytes = zip::read(jar, entry)
                .map_err(|e| Failure::new("The jar is not a readable archive", e))?;
            out.push((key, bytes));
        }

        if let Some(entry) = entries.iter().find(|e| e.name == PACK_ICON_SOURCE) {
            let bytes = zip::read(jar, entry)
                .map_err(|e| Failure::new("The jar is not a readable archive", e))?;
            out.push((PACK_ICON_KEY.to_owned(), bytes));
        }

        self.done.store(count as u64, Ordering::Relaxed);
        let bytes: usize = out.iter().map(|(_, b)| b.len()).sum();
        log_info!("assets", "kept {} entries, {bytes} bytes", out.len());
        Ok(Some(out))
    }

    async fn fetch_objects(
        &self,
        index: &serde_json::Value,
        want: impl Fn(&str) -> bool,
    ) -> Result<Option<Vec<(String, Vec<u8>)>>, Failure> {
        self.set(Stage::Fetching);
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);

        let objects = index["objects"]
            .as_object()
            .ok_or_else(|| Failure::plain("Mojang sent something unreadable"))?;
        let wanted: Vec<(String, String, u64)> = objects
            .iter()
            .filter_map(|(name, info)| {
                let key = name.strip_prefix(NAMESPACE_PREFIX)?;
                if !want(key) {
                    return None;
                }
                let key = format!("{ASSET_KEY_PREFIX}{key}");
                let hash = info["hash"].as_str().filter(|h| h.len() >= 2)?;
                Some((key, hash.to_owned(), info["size"].as_u64().unwrap_or(0)))
            })
            .collect();
        let total: u64 = wanted.iter().map(|(_, _, size)| size).sum();
        self.total.store(total, Ordering::Relaxed);
        log_info!("assets", "fetching {} objects, {total} bytes", wanted.len());

        let mut out = Vec::with_capacity(wanted.len());
        let mut done = 0u64;
        for (key, hash, _) in wanted {
            if self.cancelled() {
                return Ok(None);
            }
            let url = object_url(&hash);
            let body = crate::platform::http::get_bytes(&url, 0, &mut |_, _| !self.cancelled())
                .await
                .map_err(|e| Failure::http("Could not reach Mojang", e))?;
            let bytes = match body {
                crate::platform::http::Fetched::Body(bytes) => bytes,
                crate::platform::http::Fetched::Cancelled => return Ok(None),
            };

            let got = sha1_hex(&bytes);
            if got != hash {
                return Err(Failure::new(
                    "An asset arrived damaged",
                    format!("{key}: sha1 {got}, expected {hash}"),
                ));
            }

            done += bytes.len() as u64;
            self.done.store(done, Ordering::Relaxed);
            out.push((key, bytes));
        }
        Ok(Some(out))
    }

    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    fn settle_sounds(&self, result: Result<bool, Failure>) -> Result<bool, Failure> {
        match result {
            Ok(false) if !self.cancelled() => {
                log_info!(
                    "assets",
                    "sounds skipped; the client will be silent until asked again"
                );
                SOUNDS_SKIPPED.store(true, Ordering::Relaxed);
                crate::platform::storage::SOUNDS_SKIPPED.store("1");
                Ok(true)
            }
            other => other,
        }
    }

    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    async fn install_sounds(&self, index_url: &str, index_id: &str) -> Result<bool, Failure> {
        if crate::audio::store::installed(index_id) {
            log_info!("assets", "the sound set for index {index_id} is installed");
            return Ok(true);
        }
        self.sound_active.store(true, Ordering::Relaxed);
        let result = self.fetch_sounds(index_url, index_id).await;
        self.sound_active.store(false, Ordering::Relaxed);
        result
    }

    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    async fn fetch_sounds(&self, index_url: &str, index_id: &str) -> Result<bool, Failure> {
        use std::sync::atomic::AtomicUsize;

        self.sound_done.store(0, Ordering::Relaxed);
        self.sound_total.store(0, Ordering::Relaxed);
        let index = get_json(index_url).await?;
        let objects = index["objects"]
            .as_object()
            .ok_or_else(|| Failure::plain("Mojang sent something unreadable"))?;
        let object = |name: &str| -> Option<(String, u64)> {
            let info = objects.get(name)?;
            let hash = info["hash"].as_str().filter(|h| h.len() >= 2)?.to_owned();
            Some((hash, info["size"].as_u64().unwrap_or(0)))
        };

        let (defs_hash, _) = object("minecraft/sounds.json")
            .ok_or_else(|| Failure::plain("The asset index lists no sounds.json"))?;
        let Some(defs_bytes) = self.fetch_object(&defs_hash).await? else {
            return Ok(false);
        };
        let defs_text = String::from_utf8(defs_bytes)
            .map_err(|e| Failure::new("sounds.json is not text", e))?;
        let defs = crate::audio::defs::Defs::parse(&defs_text)
            .map_err(|e| Failure::new("sounds.json will not parse", e))?;

        let wanted: Vec<(String, String, u64)> = defs
            .object_keys()
            .into_iter()
            .filter_map(|key| {
                let (hash, size) = object(&format!("{NAMESPACE_PREFIX}{key}"))?;
                Some((key, hash, size))
            })
            .collect();
        let total: u64 = wanted.iter().map(|(_, _, size)| size).sum();
        self.sound_total.store(total, Ordering::Relaxed);
        log_info!(
            "assets",
            "{} sound events, {} files, {total} bytes",
            defs.len(),
            wanted.len()
        );

        let workers = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .clamp(2, 8);
        let next = AtomicUsize::new(0);
        let done = AtomicU64::new(0);
        let collected: Mutex<Vec<(String, Vec<u8>)>> = Mutex::new(Vec::with_capacity(wanted.len()));
        let failed: Mutex<Option<Failure>> = Mutex::new(None);

        std::thread::scope(|scope| {
            for _ in 0..workers.min(wanted.len().max(1)) {
                scope.spawn(|| {
                    loop {
                        if self.cancelled()
                            || self.sound_skip.load(Ordering::Relaxed)
                            || failed.lock().is_ok_and(|f| f.is_some())
                        {
                            return;
                        }
                        let at = next.fetch_add(1, Ordering::Relaxed);
                        let Some((key, hash, _)) = wanted.get(at) else {
                            return;
                        };
                        let url = object_url(hash);
                        let bytes = match crate::platform::http::get_bytes_blocking(&url) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                *failed.lock().unwrap() =
                                    Some(Failure::http("A sound could not be fetched", e));
                                return;
                            }
                        };
                        let got = sha1_hex(&bytes);
                        if got != *hash {
                            *failed.lock().unwrap() = Some(Failure::new(
                                "A sound arrived damaged",
                                format!("{key}: sha1 {got}, expected {hash}"),
                            ));
                            return;
                        }
                        let so_far = done.fetch_add(bytes.len() as u64, Ordering::Relaxed)
                            + bytes.len() as u64;
                        self.sound_done.store(so_far, Ordering::Relaxed);
                        collected.lock().unwrap().push((key.clone(), bytes));
                    }
                });
            }
        });

        if let Some(failure) = failed.lock().unwrap().take() {
            return Err(failure);
        }
        if self.cancelled() || self.sound_skip.load(Ordering::Relaxed) {
            return Ok(false);
        }

        let entries = collected.into_inner().unwrap();
        log_info!("assets", "packing {} sounds", entries.len());
        let packed = pack(entries);
        crate::audio::store::save(index_id, &packed, &defs_text)
            .map_err(|e| Failure::new("Cannot write to the assets folder", e))?;
        Ok(true)
    }

    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    async fn fetch_object(&self, hash: &str) -> Result<Option<Vec<u8>>, Failure> {
        get_object(hash, &|| self.cancelled()).await
    }
}

pub(super) async fn get_object(
    hash: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<u8>>, Failure> {
    let url = object_url(hash);
    let body = crate::platform::http::get_bytes(&url, 0, &mut |_, _| !cancelled())
        .await
        .map_err(|e| Failure::http("Mojang's asset store did not answer", e))?;
    let bytes = match body {
        crate::platform::http::Fetched::Body(bytes) => bytes,
        crate::platform::http::Fetched::Cancelled => return Ok(None),
    };
    let got = sha1_hex(&bytes);
    if got != hash {
        return Err(Failure::new(
            "An asset arrived damaged",
            format!("sha1 {got}, expected {hash}"),
        ));
    }
    Ok(Some(bytes))
}

#[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
struct SoundTask {
    dl: &'static Download,
    handle: Option<std::thread::JoinHandle<Result<bool, Failure>>>,
}

#[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
impl SoundTask {
    fn spawn(dl: &'static Download, meta: &serde_json::Value) -> Result<SoundTask, Failure> {
        let index = &meta["assetIndex"];
        let (Some(url), Some(id)) = (index["url"].as_str(), index["id"].as_str()) else {
            return Err(Failure::plain("Mojang sent no asset index"));
        };
        let (url, id) = (url.to_owned(), id.to_owned());
        let handle = std::thread::Builder::new()
            .name("sound-download".into())
            .spawn(move || crate::platform::executor::block_on(dl.install_sounds(&url, &id)))
            .map_err(|e| Failure::new("A sound could not be fetched", e))?;
        Ok(SoundTask {
            dl,
            handle: Some(handle),
        })
    }

    fn finish(mut self) -> Result<bool, Failure> {
        let handle = self.handle.take().expect("finished once");
        if !handle.is_finished() {
            self.dl.set(Stage::Sounds);
        }
        let result = handle
            .join()
            .unwrap_or_else(|_| Err(Failure::plain("A sound could not be fetched")));
        self.dl.settle_sounds(result)
    }
}

#[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
impl Drop for SoundTask {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.dl.skip_sounds();
            let _ = handle.join();
        }
    }
}

fn pack(mut entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let before = entries.len();
    entries.dedup_by(|a, b| a.0 == b.0);
    debug_assert_eq!(before, entries.len(), "pack() got a duplicate archive key");

    let table: usize = entries.iter().map(|(k, _)| 8 + k.len()).sum();
    let blobs: usize = entries.iter().map(|(_, v)| v.len()).sum();
    let mut out = Vec::with_capacity(12 + table + blobs);
    out.extend_from_slice(crate::platform::assets::ARCHIVE_MAGIC);
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (key, blob) in &entries {
        out.extend_from_slice(&(key.len() as u32).to_le_bytes());
        out.extend_from_slice(key.as_bytes());
        out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
    }
    for (_, blob) in &entries {
        out.extend_from_slice(blob);
    }
    out
}

fn meta_blob(version: &str, index: Option<&str>) -> Vec<u8> {
    serde_json::json!({ "version": version, "index": index })
        .to_string()
        .into_bytes()
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let at = |i: usize| u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    Some((at(16), at(20)))
}

fn panorama_is_stubbed<'a>(get: impl Fn(&str) -> Option<&'a [u8]>) -> bool {
    (0..6).any(|face| {
        get(&panorama_face(face)).is_none_or(|bytes| png_size(bytes) != Some((1024, 1024)))
    })
}

fn panorama_face(face: u32) -> String {
    format!("{ASSET_KEY_PREFIX}{PANORAMA_DIR}panorama_{face}.png")
}

fn unifont_is_stubbed<'a>(get: impl Fn(&str) -> Option<&'a [u8]>) -> bool {
    match get(&format!("{ASSET_KEY_PREFIX}{UNIFONT_JSON_KEY}")) {
        Some(bytes) => {
            let json: serde_json::Value = serde_json::from_slice(bytes).unwrap_or_default();
            json["providers"]
                .as_array()
                .is_none_or(std::vec::Vec::is_empty)
        }
        None => true,
    }
}

pub(crate) const NETWORK_FAILURE: &str = "Network connection problem";

pub(super) struct Failure {
    pub(super) short: &'static str,
    pub(super) detail: String,
}

impl Failure {
    pub(super) fn new(short: &'static str, detail: impl std::fmt::Display) -> Failure {
        Failure {
            short,
            detail: detail.to_string(),
        }
    }

    fn http(short: &'static str, e: crate::platform::http::HttpError) -> Failure {
        if e.network {
            Failure::new(NETWORK_FAILURE, e)
        } else {
            Failure::new(short, e)
        }
    }

    pub(super) fn plain(short: &'static str) -> Failure {
        Failure {
            short,
            detail: short.to_string(),
        }
    }
}

pub(super) async fn get_json(url: &str) -> Result<serde_json::Value, Failure> {
    let text = match crate::platform::http::get_string(url).await {
        Ok(text) => Ok(text),
        Err(e) => match fallback_url(url) {
            Some(proxied) => {
                log_warn!("assets", "{url}: {e}; retrying through the proxy");
                crate::platform::http::get_string(&proxied).await
            }
            None => Err(e),
        },
    }
    .map_err(|e| Failure::http("Could not reach Mojang", e))?;
    serde_json::from_str(&text).map_err(|e| Failure::new("Mojang sent something unreadable", e))
}

#[cfg(target_arch = "wasm32")]
fn fallback_url(url: &str) -> Option<String> {
    Some(format!("{OBJECT_PROXY}{url}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn fallback_url(_: &str) -> Option<String> {
    None
}

pub(super) fn pick_version(manifest: &serde_json::Value, version: &str) -> Result<String, Failure> {
    manifest["versions"]
        .as_array()
        .ok_or_else(|| Failure::plain("Mojang sent something unreadable"))?
        .iter()
        .find(|v| v["id"].as_str() == Some(version))
        .and_then(|v| v["url"].as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            Failure::new(
                "Mojang has no version to install",
                format!("the manifest lists no {version}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_the_jar_path_verbatim() {
        for name in [
            "assets/minecraft/blockstates/stone.json",
            "assets/minecraft/textures/block/dirt.png",
            "data/minecraft/timeline/day.json",
            "data/minecraft/worldgen/biome/plains.json",
        ] {
            assert_eq!(jar_key(name).as_deref(), Some(name));
        }
    }

    #[test]
    fn nothing_else_in_the_jar_has_a_key() {
        for name in [
            "net/minecraft/client/Minecraft.class",
            "META-INF/MANIFEST.MF",
            "assets/realms/lang/en_us.json",
            "assets/.mcassetsroot",
            "data/realms/whatever.json",
            "version.json",
        ] {
            assert_eq!(jar_key(name), None, "{name}");
        }
    }

    #[test]
    fn a_rule_selects_out_of_one_namespace_only() {
        assert!(kept("data/minecraft/dimension_type/overworld.json"));
        assert!(!kept("assets/minecraft/dimension_type/overworld.json"));
        assert!(kept("assets/minecraft/blockstates/stone.json"));
        assert!(!kept("data/minecraft/blockstates/stone.json"));
        assert!(!kept("blockstates/stone.json"));
    }

    #[test]
    fn a_directory_rule_stops_at_a_slash() {
        assert!(kept("assets/minecraft/textures/block/stone.png"));
        assert!(!kept("assets/minecraft/textures_extra/block/stone.png"));
        assert!(!kept("data/minecraft/timeline_extra/day.json"));
        assert!(!kept("assets/minecraft/post_effect/blur.json"));
    }

    #[test]
    fn only_the_one_language_file_is_kept() {
        assert!(kept("assets/minecraft/lang/en_us.json"));
        assert!(!kept("assets/minecraft/lang/fr_fr.json"));
        assert!(!kept("assets/minecraft/lang/deprecated.json"));
    }
}
