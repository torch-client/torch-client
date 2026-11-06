use std::sync::OnceLock;

use crate::platform::storage;
use crate::platform::time::{SystemTime, UNIX_EPOCH};

pub(super) const LATEST_API: &str =
    "https://api.github.com/repos/torch-client/torch-client/releases/latest";
pub(crate) const RELEASES_URL: &str =
    "https://github.com/torch-client/torch-client/releases/latest";
const RECHECK_SECS: u64 = 24 * 60 * 60;

static AVAILABLE: OnceLock<String> = OnceLock::new();

pub(crate) fn available() -> Option<&'static str> {
    AVAILABLE.get().map(String::as_str)
}

pub(crate) fn spawn() {
    let started = std::thread::Builder::new()
        .name("update-check".into())
        .spawn(|| {
            check();
            #[cfg(self_update)]
            super::self_update::remove_leftovers();
        });
    if let Err(e) = started {
        crate::log_warn!("update", "could not start the update check: {e}");
    }
}

fn check() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let latest = match cached(now) {
        Some(tag) => tag,
        None => match fetch() {
            Some(tag) => {
                let doc = serde_json::json!({ "checked_at": now, "latest": tag });
                #[cfg(self_update)]
                let doc = super::self_update::with_skip(doc);
                storage::UPDATE_CHECK.store(&doc.to_string());
                tag
            }
            None => return,
        },
    };
    if is_newer(&latest, env!("CARGO_PKG_VERSION")) {
        crate::log_info!("update", "{latest} is available ({RELEASES_URL})");
        let _ = AVAILABLE.set(latest);
    }
}

fn cached(now: u64) -> Option<String> {
    let doc: serde_json::Value = serde_json::from_str(&storage::UPDATE_CHECK.load()?).ok()?;
    let checked_at = doc.get("checked_at")?.as_u64()?;
    if now.saturating_sub(checked_at) >= RECHECK_SECS {
        return None;
    }
    Some(doc.get("latest")?.as_str()?.to_owned())
}

fn fetch() -> Option<String> {
    let body =
        match crate::platform::executor::block_on(crate::platform::http::get_string(LATEST_API)) {
            Ok(body) => body,
            Err(e) => {
                crate::log_warn!("update", "update check failed: {e}");
                return None;
            }
        };
    let doc: serde_json::Value = serde_json::from_str(&body).ok()?;
    doc.get("tag_name")?.as_str().map(str::to_owned)
}

fn parse(version: &str) -> Option<[u64; 3]> {
    let version = version.trim().trim_start_matches(['v', 'V']);
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some([major, minor, patch])
}

pub(super) fn is_newer(tag: &str, current: &str) -> bool {
    match (parse(tag), parse(current)) {
        (Some(tag), Some(current)) => tag > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_numerically_with_or_without_a_v() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("v1", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("v0.0.9", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }
}
