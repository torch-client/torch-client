use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::util::zip;
use crate::{log_info, log_warn};

const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

const OBJECT_STORE: &str = "https://resources.download.minecraft.net";

#[cfg(target_arch = "wasm32")]
const OBJECT_PROXY: &str = "https://getstuff.xpncvr.workers.dev/";

const NAMESPACE_DIR: &str = "assets/minecraft/";

const DATA_DIR: &str = "data/minecraft/";

const ASSET_KEY_PREFIX: &str = NAMESPACE_DIR;

const NAMESPACE_PREFIX: &str = "minecraft/";

const PANORAMA_DIR: &str = "textures/gui/title/background/";

const UNIFONT_JSON_KEY: &str = "font/include/unifont.json";
const UNIFONT_ZIP_KEY: &str = "font/unifont.zip";

const PACK_ICON_SOURCE: &str = "pack.png";
const PACK_ICON_KEY: &str = "assets/minecraft/textures/gui/pack.png";

const META_KEY: &str = ".meta";

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
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let part = path.with_extension("part");
            std::fs::write(&part, bytes).map_err(|e| e.to_string())?;
            std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
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

        pub(in crate::client::assets) fn has_index(id: &str) -> bool {
            index_pack(id).is_file()
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

        pub(in crate::client::assets) fn has_index(_id: &str) -> bool {
            false
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

    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    {
        let sounds_ok = meta["index"]
            .as_str()
            .is_some_and(crate::audio::store::installed);
        if !sounds_ok {
            log_info!(
                "assets",
                "audio is enabled but no sound set is installed; the download screen will fetch it"
            );
            SOUNDS_ONLY.store(true, Ordering::Relaxed);
            return false;
        }
        SOUNDS_ONLY.store(false, Ordering::Relaxed);
    }

    let mut layers = Vec::with_capacity(2);
    if let Some(id) = meta["index"].as_str() {
        match store::load_index(id)
            .ok_or_else(|| "not on disk".to_owned())
            .and_then(crate::platform::assets::unpack)
        {
            Ok(layer) => {
                log_info!("assets", "objects layer {id}: {} entries", layer.len());
                layers.push(layer);
            }
            Err(e) => log_warn!("assets", "objects layer {id} unreadable: {e}"),
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
    true
}

fn jar_key(name: &str) -> Option<String> {
    let inside = name.starts_with(NAMESPACE_DIR) || name.starts_with(DATA_DIR);
    inside.then(|| name.to_owned())
}

const REPORT_STEP: u64 = 4 * 1024 * 1024;

fn progress_line(done: u64, total: u64) -> String {
    let mb = |n: u64| n as f64 / (1024.0 * 1024.0);
    if total == 0 {
        return format!("{:.1} MB", mb(done));
    }
    format!(
        "{:.1}/{:.1} MB ({}%)",
        mb(done),
        mb(total),
        done * 100 / total.max(1)
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
}

pub(crate) fn download() -> &'static Download {
    static D: OnceLock<Download> = OnceLock::new();
    D.get_or_init(|| Download {
        stage: AtomicU8::new(0),
        done: AtomicU64::new(0),
        total: AtomicU64::new(0),
        cancel: AtomicBool::new(false),
        error: Mutex::new(""),
    })
}

#[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
static SOUNDS_ONLY: AtomicBool = AtomicBool::new(false);

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
            Stage::Asking | Stage::Downloading | Stage::Extracting | Stage::Fetching
        ) {
            return;
        }
        self.cancel.store(false, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
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

    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    async fn run(&self, version: &str) {
        #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
        if SOUNDS_ONLY.load(Ordering::Relaxed) {
            match self.install_sounds_only(version).await {
                Ok(true) => {
                    log_info!("assets", "sound set installed");
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
            return;
        }
        match self.install(version).await {
            Ok(true) => {
                log_info!("assets", "{version} installed");
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

    async fn install(&self, version: &str) -> Result<bool, Failure> {
        log_info!("assets", "asking Mojang about {version}");
        self.set(Stage::Asking);
        let manifest = get_json(VERSION_MANIFEST).await?;
        let meta = get_json(&pick_version(&manifest, version)?).await?;

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

        let wants_panorama = panorama_is_stubbed(&entries);
        let wants_unifont = cfg!(feature = "full_font") && unifont_is_stubbed(&entries);
        let wants_sounds = cfg!(feature = "audio");
        let mut index_id = None;
        if CAN_FETCH_OBJECTS && (wants_panorama || wants_unifont || wants_sounds) {
            let id = meta["assetIndex"]["id"]
                .as_str()
                .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;
            let index_url = meta["assetIndex"]["url"]
                .as_str()
                .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;
            if wants_panorama || wants_unifont {
                if store::has_index(id) {
                    log_info!("assets", "objects layer {id} is already installed");
                } else {
                    log_info!(
                        "assets",
                        "the jar's panorama or unifont table is stubbed; reading index {id}"
                    );
                    let wanted = |key: &str| {
                        (wants_panorama && key.starts_with(PANORAMA_DIR))
                            || (wants_unifont
                                && (key == UNIFONT_JSON_KEY || key == UNIFONT_ZIP_KEY))
                    };
                    let objects = match self.fetch_objects(index_url, wanted).await {
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
            #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
            if !self.install_sounds(index_url, id).await? {
                return Ok(false);
            }
            index_id = (!cfg!(target_arch = "wasm32")).then(|| id.to_owned());
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
        .map_err(|e| Failure::new("The download was interrupted", e))?;

        let bytes = match body {
            crate::platform::http::Fetched::Body(bytes) => bytes,
            crate::platform::http::Fetched::Cancelled => return Ok(None),
        };

        let mut hash = Sha1::new();
        hash.update(&bytes);
        let got = hash.hex();
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
        index_url: &str,
        want: impl Fn(&str) -> bool,
    ) -> Result<Option<Vec<(String, Vec<u8>)>>, Failure> {
        self.set(Stage::Fetching);
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);

        let index = get_json(index_url).await?;
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
                .map_err(|e| Failure::new("Could not reach Mojang", e))?;
            let bytes = match body {
                crate::platform::http::Fetched::Body(bytes) => bytes,
                crate::platform::http::Fetched::Cancelled => return Ok(None),
            };

            let mut sha = Sha1::new();
            sha.update(&bytes);
            let got = sha.hex();
            if got != hash {
                return Err(Failure::new(
                    "An asset arrived damaged",
                    format!("{key}: sha1 {got}, expected {hash}"),
                ));
            }

            done += bytes.len() as u64;
            self.done.store(done, Ordering::Relaxed);
            log_info!("assets", "object {key}, {} bytes", bytes.len());
            out.push((key, bytes));
        }
        Ok(Some(out))
    }

    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    async fn install_sounds_only(&self, version: &str) -> Result<bool, Failure> {
        self.set(Stage::Asking);
        let manifest = get_json(VERSION_MANIFEST).await?;
        let meta = get_json(&pick_version(&manifest, version)?).await?;
        let index_url = meta["assetIndex"]["url"]
            .as_str()
            .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;
        let index_id = meta["assetIndex"]["id"]
            .as_str()
            .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;
        self.install_sounds(index_url, index_id).await
    }

    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    async fn install_sounds(&self, index_url: &str, index_id: &str) -> Result<bool, Failure> {
        use std::sync::atomic::AtomicUsize;

        if crate::audio::store::installed(index_id) {
            log_info!("assets", "the sound set for index {index_id} is installed");
            return Ok(true);
        }

        self.set(Stage::Fetching);
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
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
        self.total.store(total, Ordering::Relaxed);
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
                        if self.cancelled() || failed.lock().is_ok_and(|f| f.is_some()) {
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
                                    Some(Failure::new("A sound could not be fetched", e));
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
                        self.done.store(so_far, Ordering::Relaxed);
                        collected.lock().unwrap().push((key.clone(), bytes));
                    }
                });
            }
        });

        if let Some(failure) = failed.lock().unwrap().take() {
            return Err(failure);
        }
        if self.cancelled() {
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
        let url = object_url(hash);
        let body = crate::platform::http::get_bytes(&url, 0, &mut |_, _| !self.cancelled())
            .await
            .map_err(|e| Failure::new("Mojang's asset store did not answer", e))?;
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
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hash = Sha1::new();
    hash.update(bytes);
    hash.hex()
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

fn panorama_is_stubbed(entries: &[(String, Vec<u8>)]) -> bool {
    (0..6).any(|face| {
        let name = format!("{ASSET_KEY_PREFIX}{PANORAMA_DIR}panorama_{face}.png");
        match entries.iter().find(|(key, _)| *key == name) {
            Some((_, bytes)) => png_size(bytes) != Some((1024, 1024)),
            None => true,
        }
    })
}

fn unifont_is_stubbed(entries: &[(String, Vec<u8>)]) -> bool {
    let name = format!("{ASSET_KEY_PREFIX}{UNIFONT_JSON_KEY}");
    match entries.iter().find(|(key, _)| *key == name) {
        Some((_, bytes)) => {
            let json: serde_json::Value = serde_json::from_slice(bytes).unwrap_or_default();
            json["providers"]
                .as_array()
                .is_none_or(std::vec::Vec::is_empty)
        }
        None => true,
    }
}

struct Failure {
    short: &'static str,
    detail: String,
}

impl Failure {
    fn new(short: &'static str, detail: impl std::fmt::Display) -> Failure {
        Failure {
            short,
            detail: detail.to_string(),
        }
    }

    fn plain(short: &'static str) -> Failure {
        Failure {
            short,
            detail: short.to_string(),
        }
    }
}

async fn get_json(url: &str) -> Result<serde_json::Value, Failure> {
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
    .map_err(|e| Failure::new("Could not reach Mojang", e))?;
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

fn pick_version(manifest: &serde_json::Value, version: &str) -> Result<String, Failure> {
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

struct Sha1 {
    h: [u32; 5],
    block: [u8; 64],
    filled: usize,
    len: u64,
}

impl Sha1 {
    fn new() -> Sha1 {
        Sha1 {
            h: [
                0x6745_2301,
                0xEFCD_AB89,
                0x98BA_DCFE,
                0x1032_5476,
                0xC3D2_E1F0,
            ],
            block: [0; 64],
            filled: 0,
            len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.len = self.len.wrapping_add(data.len() as u64);
        while !data.is_empty() {
            let take = (64 - self.filled).min(data.len());
            self.block[self.filled..self.filled + take].copy_from_slice(&data[..take]);
            self.filled += take;
            data = &data[take..];
            if self.filled == 64 {
                self.compress();
                self.filled = 0;
            }
        }
    }

    fn compress(&mut self) {
        let mut w = [0u32; 80];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let b = &self.block[i * 4..i * 4 + 4];
            *word = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        self.h[0] = self.h[0].wrapping_add(a);
        self.h[1] = self.h[1].wrapping_add(b);
        self.h[2] = self.h[2].wrapping_add(c);
        self.h[3] = self.h[3].wrapping_add(d);
        self.h[4] = self.h[4].wrapping_add(e);
    }

    fn hex(mut self) -> String {
        let bits = self.len.wrapping_mul(8);
        self.update(&[0x80]);
        while self.filled != 56 {
            self.update(&[0]);
        }
        self.block[56..64].copy_from_slice(&bits.to_be_bytes());
        self.compress();
        let mut out = String::with_capacity(40);
        for word in self.h {
            out.push_str(&format!("{word:08x}"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_the_published_vectors() {
        let mut h = Sha1::new();
        h.update(b"abc");
        assert_eq!(h.hex(), "a9993e364706816aba3e25717850c26c9cd0d89d");

        let mut h = Sha1::new();
        h.update(b"");
        assert_eq!(h.hex(), "da39a3ee5e6b4b0d3255bfef95601890afd80709");

        let mut h = Sha1::new();
        for chunk in b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".chunks(7) {
            h.update(chunk);
        }
        assert_eq!(h.hex(), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
    }

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
