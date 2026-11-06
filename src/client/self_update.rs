use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use serde_json::Value;

use super::update_checker::{LATEST_API, available, is_newer};
use crate::platform::http::{self, Fetched};
use crate::platform::{executor, storage};
use crate::util::{hash, zip};

const ASSET: &str = match (cfg!(feature = "click_gui"), cfg!(target_os = "windows")) {
    (false, false) => "torch-client-linux-x86_64.zip",
    (true, false) => "torch-client-clickgui-linux-x86_64.zip",
    (false, true) => "torch-client-windows-x86_64.exe.zip",
    (true, true) => "torch-client-clickgui-windows-x86_64.exe.zip",
};

const EXE_MAGIC: &[u8] = if cfg!(target_os = "windows") {
    b"MZ"
} else {
    b"\x7fELF"
};

const SKIP_KEY: &str = "skip_until";

const STAGED_SUFFIX: &str = ".new";
const RETIRED_SUFFIX: &str = ".old";

const REMOVE_ATTEMPTS: u32 = 10;
const REMOVE_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Stage {
    Idle,
    Asking,
    Downloading,
    Installing,
    Done,
    Failed(String),
}

static STAGE: Mutex<Stage> = Mutex::new(Stage::Idle);
static DOWNLOADED: AtomicU64 = AtomicU64::new(0);
static TOTAL: AtomicU64 = AtomicU64::new(0);
static CANCEL: AtomicBool = AtomicBool::new(false);
static OFFERED: AtomicBool = AtomicBool::new(false);
static SKIP_UNTIL: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(load_skip()));

fn load_skip() -> Option<String> {
    let doc: Value = serde_json::from_str(&storage::UPDATE_CHECK.load()?).ok()?;
    doc.get(SKIP_KEY)?.as_str().map(str::to_owned)
}

fn save_skip(skip_until: Option<&str>) {
    let mut doc = storage::UPDATE_CHECK
        .load()
        .and_then(|text| serde_json::from_str::<serde_json::Map<String, Value>>(&text).ok())
        .unwrap_or_default();
    match skip_until {
        Some(tag) => {
            doc.insert(SKIP_KEY.to_owned(), tag.into());
        }
        None => {
            doc.remove(SKIP_KEY);
        }
    }
    storage::UPDATE_CHECK.store(&Value::Object(doc).to_string());
    *SKIP_UNTIL.lock().unwrap() = skip_until.map(str::to_owned);
}

pub(super) fn with_skip(mut doc: Value) -> Value {
    if let Some(tag) = SKIP_UNTIL.lock().unwrap().clone() {
        doc[SKIP_KEY] = Value::String(tag);
    }
    doc
}

fn set_stage(stage: Stage) {
    *STAGE.lock().unwrap() = stage;
}

pub(crate) fn stage() -> Stage {
    STAGE.lock().unwrap().clone()
}

pub(crate) fn progress() -> (u64, u64) {
    (
        DOWNLOADED.load(Ordering::Relaxed),
        TOTAL.load(Ordering::Relaxed),
    )
}

pub(crate) fn take_offer() -> bool {
    if OFFERED.load(Ordering::Relaxed) {
        return false;
    }
    let Some(tag) = available() else {
        return false;
    };
    if !wanted(tag, SKIP_UNTIL.lock().unwrap().as_deref()) {
        return false;
    }
    !OFFERED.swap(true, Ordering::Relaxed)
}

fn wanted(tag: &str, skip_until: Option<&str>) -> bool {
    skip_until.is_none_or(|skipped| is_newer(tag, skipped))
}

pub(crate) fn skip(tag: &str) {
    save_skip(Some(tag));
}

pub(crate) fn skipping() -> bool {
    SKIP_UNTIL.lock().unwrap().is_some()
}

pub(crate) fn enable() {
    save_skip(None);
    OFFERED.store(false, Ordering::Relaxed);
}

pub(crate) fn start() {
    {
        let mut stage = STAGE.lock().unwrap();
        if !matches!(*stage, Stage::Idle | Stage::Failed(_)) {
            return;
        }
        *stage = Stage::Asking;
    }
    CANCEL.store(false, Ordering::Relaxed);
    DOWNLOADED.store(0, Ordering::Relaxed);
    TOTAL.store(0, Ordering::Relaxed);
    let started = std::thread::Builder::new()
        .name("self-update".into())
        .spawn(|| {
            set_stage(match install() {
                Ok(true) => Stage::Done,
                Ok(false) => Stage::Idle,
                Err(e) => {
                    crate::log_warn!("update", "self update failed: {e}");
                    Stage::Failed(e)
                }
            });
        });
    if let Err(e) = started {
        set_stage(Stage::Failed(format!("Could not start the download: {e}")));
    }
}

pub(crate) fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}

pub(crate) fn dismiss() {
    set_stage(Stage::Idle);
}

pub(super) fn remove_leftovers() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let old = sibling(&exe, RETIRED_SUFFIX);
    for _ in 0..REMOVE_ATTEMPTS {
        match std::fs::remove_file(&old) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => std::thread::sleep(REMOVE_WAIT),
            _ => return,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ReleaseAsset {
    tag: String,
    url: String,
    size: u64,
    sha256: String,
}

fn release_asset(doc: &Value, name: &str) -> Result<ReleaseAsset, String> {
    let tag = doc
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or("GitHub named no release")?;
    let asset = doc
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a.get("name").and_then(Value::as_str) == Some(name))
        })
        .ok_or_else(|| format!("Release {tag} has no {name}"))?;
    let url = asset
        .get("browser_download_url")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Release {tag} gives no link to {name}"))?;
    let sha256 = asset
        .get("digest")
        .and_then(Value::as_str)
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .ok_or_else(|| format!("Release {tag} lists no SHA-256 for {name}"))?;
    Ok(ReleaseAsset {
        tag: tag.to_owned(),
        url: url.to_owned(),
        size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
        sha256: sha256.to_owned(),
    })
}

fn install() -> Result<bool, String> {
    let body = executor::block_on(http::get_string(LATEST_API))
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;
    let doc: Value =
        serde_json::from_str(&body).map_err(|e| format!("Could not read GitHub's answer: {e}"))?;
    let release = release_asset(&doc, ASSET)?;
    if !is_newer(&release.tag, env!("CARGO_PKG_VERSION")) {
        return Err(format!("{} is not newer than this client", release.tag));
    }

    set_stage(Stage::Downloading);
    let mut report = |done: u64, total: u64| {
        DOWNLOADED.store(done, Ordering::Relaxed);
        TOTAL.store(total, Ordering::Relaxed);
        !CANCEL.load(Ordering::Relaxed)
    };
    let fetched = executor::block_on(http::get_bytes(&release.url, release.size, &mut report))
        .map_err(|e| format!("The download failed: {e}"))?;
    let archive = match fetched {
        Fetched::Body(bytes) => bytes,
        Fetched::Cancelled => return Ok(false),
    };

    set_stage(Stage::Installing);
    if !hash::sha256_hex(&archive).eq_ignore_ascii_case(&release.sha256) {
        return Err(format!("{ASSET} does not match the SHA-256 GitHub lists"));
    }
    let executable = unpack(&archive)?;
    drop(archive);
    let exe = swap(&executable)?;
    crate::log_info!("update", "installed {} at {}", release.tag, exe.display());

    std::process::Command::new(&exe)
        .args(std::env::args_os().skip(1))
        .spawn()
        .map_err(|e| format!("{} is installed. Start it again by hand: {e}", release.tag))?;
    Ok(true)
}

fn unpack(archive: &[u8]) -> Result<Vec<u8>, String> {
    let entries = zip::entries(archive).map_err(|e| format!("{ASSET} is unreadable: {e}"))?;
    let mut files = entries.iter().filter(|entry| !entry.name.ends_with('/'));
    let (Some(entry), None) = (files.next(), files.next()) else {
        return Err(format!("{ASSET} does not hold exactly one file"));
    };
    let bytes = zip::read(archive, entry).map_err(|e| format!("{ASSET} is unreadable: {e}"))?;
    if !bytes.starts_with(EXE_MAGIC) {
        return Err(format!(
            "{ASSET} does not hold an executable for this system"
        ));
    }
    Ok(bytes)
}

fn sibling(exe: &Path, suffix: &str) -> PathBuf {
    let mut path = exe.as_os_str().to_owned();
    path.push(suffix);
    PathBuf::from(path)
}

fn swap(executable: &[u8]) -> Result<PathBuf, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("Could not find the running client: {e}"))?;
    let staged = sibling(&exe, STAGED_SUFFIX);
    let swapped = std::fs::write(&staged, executable)
        .and_then(|()| {
            let permissions = std::fs::metadata(&exe)?.permissions();
            std::fs::set_permissions(&staged, permissions)
        })
        .and_then(|()| move_into_place(&staged, &exe));
    if let Err(e) = swapped {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("Could not replace {}: {e}", exe.display()));
    }
    Ok(exe)
}

fn move_into_place(staged: &Path, exe: &Path) -> std::io::Result<()> {
    if !cfg!(target_os = "windows") {
        return std::fs::rename(staged, exe);
    }
    let old = sibling(exe, RETIRED_SUFFIX);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old)?;
    if let Err(e) = std::fs::rename(staged, exe) {
        let _ = std::fs::rename(&old, exe);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "6df728db737047fb0830b7b13915f0a6d4777fe91731fd4fde5d5c2e9047686b";

    fn release(asset: Value) -> Value {
        serde_json::json!({ "tag_name": "v0.1.4", "assets": [asset] })
    }

    #[test]
    fn finds_the_named_asset_and_its_digest() {
        let doc = release(serde_json::json!({
            "name": "a.zip",
            "size": 7,
            "digest": format!("sha256:{DIGEST}"),
            "browser_download_url": "https://example.com/a.zip",
        }));
        assert_eq!(
            release_asset(&doc, "a.zip"),
            Ok(ReleaseAsset {
                tag: "v0.1.4".to_owned(),
                url: "https://example.com/a.zip".to_owned(),
                size: 7,
                sha256: DIGEST.to_owned(),
            })
        );
        assert!(release_asset(&doc, "b.zip").is_err());
    }

    #[test]
    fn refuses_an_asset_with_no_sha256() {
        let doc = release(serde_json::json!({
            "name": "a.zip",
            "browser_download_url": "https://example.com/a.zip",
        }));
        assert!(release_asset(&doc, "a.zip").is_err());
    }

    #[test]
    fn a_skip_holds_until_a_newer_release() {
        assert!(wanted("v0.1.6", None));
        assert!(!wanted("v0.1.6", Some("v0.1.6")));
        assert!(!wanted("v0.1.5", Some("v0.1.6")));
        assert!(wanted("v0.1.7", Some("v0.1.6")));
    }

    #[test]
    fn a_suffix_keeps_the_whole_name() {
        let path = sibling(Path::new("dir/client-1.2.exe"), STAGED_SUFFIX);
        assert_eq!(path, Path::new("dir/client-1.2.exe.new"));
    }
}
