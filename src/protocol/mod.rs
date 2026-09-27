#![allow(
    dead_code,
    reason = "the tables carry lookups no hop written yet reads"
)]

pub(crate) mod hop;
pub(crate) mod packets;
pub(crate) mod remap;
pub(crate) mod version;
pub(crate) mod wire;

pub(crate) use version::joinable;

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, Ordering};

static SESSION: AtomicI32 = AtomicI32::new(0);

static FORCED: AtomicI32 = AtomicI32::new(0);

pub(crate) fn forced() -> Option<i32> {
    match FORCED.load(Ordering::Relaxed) {
        0 => None,
        protocol => Some(protocol),
    }
}

pub(crate) fn set_forced(protocol: Option<i32>) {
    let protocol = protocol.filter(|p| joinable(*p)).unwrap_or(0);
    FORCED.store(protocol, Ordering::Relaxed);
}

pub(crate) fn preselected() -> i32 {
    forced()
        .or_else(from_env)
        .unwrap_or(version::NATIVE.protocol)
}

static SEEN: OnceLock<Mutex<HashMap<String, i32>>> = OnceLock::new();

fn seen() -> &'static Mutex<HashMap<String, i32>> {
    SEEN.get_or_init(Default::default)
}

fn key(address: &str) -> String {
    address.trim().to_ascii_lowercase()
}

pub(crate) fn remember(address: &str, protocol: i32) {
    seen().lock().unwrap().insert(key(address), protocol);
}

pub(crate) async fn detect(address: &str) {
    if let Some(forced) = forced().or_else(from_env) {
        set_session(Some(forced));
        return;
    }
    if let Some(known) = seen().lock().unwrap().get(&key(address)).copied() {
        set_session(Some(known));
        return;
    }
    let protocol = crate::gui::ping::status_once(address, false)
        .await
        .ok()
        .map(|status| status.protocol);
    if let Some(protocol) = protocol {
        remember(address, protocol);
    }
    set_session(protocol);
}

fn from_env() -> Option<i32> {
    static CACHED: OnceLock<Option<i32>> = OnceLock::new();
    *CACHED.get_or_init(parse_env)
}

fn parse_env() -> Option<i32> {
    let raw = raw_override()?;
    let Ok(protocol) = raw.trim().parse::<i32>() else {
        crate::log_warn!("net", "MC_PROTOCOL={raw} is not a number; ignoring it");
        return None;
    };
    if !joinable(protocol) {
        crate::log_warn!(
            "net",
            "MC_PROTOCOL={protocol} is not a version this client can translate; ignoring it"
        );
        return None;
    }
    Some(protocol)
}

#[cfg(not(target_arch = "wasm32"))]
fn raw_override() -> Option<String> {
    std::env::var("MC_PROTOCOL").ok()
}

#[cfg(target_arch = "wasm32")]
fn raw_override() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search
        .trim_start_matches('?')
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == "MC_PROTOCOL")
        .map(|(_, value)| value.to_owned())
}

pub(crate) fn set_session(protocol: Option<i32>) {
    let protocol = protocol
        .filter(|p| joinable(*p))
        .unwrap_or(version::NATIVE.protocol);
    SESSION.store(protocol, Ordering::Relaxed);
    azalea::join::HANDSHAKE_PROTOCOL.store(protocol, Ordering::Relaxed);
    if protocol != version::NATIVE.protocol {
        let name = version::ProtocolVersion::from_protocol(protocol)
            .map(|v| v.name)
            .unwrap_or("?");
        crate::log_info!("net", "translating this connection as {name} ({protocol})");
    }
}

pub(crate) fn session() -> i32 {
    match SESSION.load(Ordering::Relaxed) {
        0 => version::NATIVE.protocol,
        protocol => protocol,
    }
}

pub(crate) fn entity_metadata_is_native() -> bool {
    session() == version::NATIVE.protocol
}

pub(crate) fn translator() -> Option<Box<dyn azalea::connection::PacketTranslator>> {
    (session() == 774).then(|| Box::new(hop::Translator::v774()) as Box<_>)
}
