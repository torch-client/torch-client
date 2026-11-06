use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait};

static WANTED: Mutex<Option<&'static str>> = Mutex::new(None);

static CHANGED: AtomicBool = AtomicBool::new(false);

static FELL_BACK: AtomicBool = AtomicBool::new(false);

static LISTED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

static PRESENT: AtomicBool = AtomicBool::new(false);

static SCANNING: AtomicBool = AtomicBool::new(false);

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd"
))]
const ALSA_LISTED: [&str; 4] = ["sysdefault", "pipewire", "pulse", "jack"];

const ALSA_CARD_PREFIX: &str = "sysdefault:CARD=";

pub fn set_device(name: Option<&'static str>) {
    let mut wanted = WANTED.lock().unwrap_or_else(PoisonError::into_inner);
    if *wanted == name {
        return;
    }
    *wanted = name;
    drop(wanted);
    request_open();
}

pub fn fell_back() -> bool {
    FELL_BACK.load(Ordering::Relaxed)
}

pub fn scan() {
    if SCANNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("audio-devices".to_owned())
        .spawn(|| {
            let wanted = *WANTED.lock().unwrap_or_else(PoisonError::into_inner);
            match list(wanted) {
                Ok((found, present)) => {
                    reconcile(wanted, present);
                    *LISTED.lock().unwrap_or_else(PoisonError::into_inner) = found;
                }
                Err(e) => crate::log_warn!("audio", "cannot list output devices: {e}"),
            }
            SCANNING.store(false, Ordering::Release);
        });
    if let Err(e) = spawned {
        crate::log_warn!("audio", "cannot start the device scan: {e}");
        SCANNING.store(false, Ordering::Release);
    }
}

pub fn next(current: Option<&'static str>) -> Option<&'static str> {
    let listed = LISTED.lock().unwrap_or_else(PoisonError::into_inner);
    let at = match current {
        None => 0,
        Some(name) => listed.iter().position(|&device| device == name)? + 1,
    };
    listed.get(at).copied()
}

pub fn label(name: &'static str) -> &'static str {
    name.strip_prefix(ALSA_CARD_PREFIX).unwrap_or(name)
}

pub(super) fn changed() -> bool {
    CHANGED.load(Ordering::Acquire)
}

pub(super) fn open() -> Option<(rodio::OutputStream, rodio::OutputStreamHandle)> {
    CHANGED.store(false, Ordering::Release);
    let wanted = *WANTED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(name) = wanted {
        match open_named(name) {
            Ok(pair) => {
                FELL_BACK.store(false, Ordering::Relaxed);
                crate::log_info!("audio", "output device: {name}");
                return Some(pair);
            }
            Err(e) => crate::log_warn!(
                "audio",
                "output device {name}: {e}; using the system default"
            ),
        }
    }
    FELL_BACK.store(wanted.is_some(), Ordering::Relaxed);
    match rodio::OutputStream::try_default() {
        Ok(pair) => Some(pair),
        Err(e) => {
            crate::log_warn!("audio", "no output device: {e}");
            None
        }
    }
}

fn request_open() {
    CHANGED.store(true, Ordering::Release);
    super::engine::wake();
}

fn open_named(name: &str) -> Result<(rodio::OutputStream, rodio::OutputStreamHandle), String> {
    let device = cpal::default_host()
        .output_devices()
        .map_err(|e| e.to_string())?
        .find(|device| device.name().is_ok_and(|n| n == name))
        .ok_or_else(|| "not found".to_owned())?;
    rodio::OutputStream::try_from_device(&device).map_err(|e| e.to_string())
}

fn list(wanted: Option<&str>) -> Result<(Vec<&'static str>, bool), String> {
    let mut found: Vec<&'static str> = Vec::new();
    let mut present = false;
    for name in output_names()? {
        present |= wanted == Some(name.as_str());
        if !listable(&name) {
            continue;
        }
        let name = crate::util::intern(name);
        if !found.contains(&name) {
            found.push(name);
        }
    }
    Ok((found, present))
}

fn reconcile(scanned: Option<&'static str>, present: bool) {
    let was_present = PRESENT.swap(present, Ordering::Relaxed);
    let Some(scanned) = scanned else {
        return;
    };
    if *WANTED.lock().unwrap_or_else(PoisonError::into_inner) != Some(scanned) {
        return;
    }
    let fell_back = FELL_BACK.load(Ordering::Relaxed);
    let returned = present && !was_present && fell_back;
    let gone = !present && !fell_back;
    if returned || gone {
        request_open();
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd"
))]
fn output_names() -> Result<Vec<String>, String> {
    let hints = alsa::device_name::HintIter::new_str(None, "pcm").map_err(|e| e.to_string())?;
    Ok(hints
        .filter(|hint| hint.direction != Some(alsa::Direction::Capture))
        .filter_map(|hint| hint.name)
        .filter(|name| name != "null")
        .collect())
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd"
)))]
fn output_names() -> Result<Vec<String>, String> {
    let devices = cpal::default_host()
        .output_devices()
        .map_err(|e| e.to_string())?;
    Ok(devices.filter_map(|device| device.name().ok()).collect())
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd"
))]
fn listable(name: &str) -> bool {
    let plugin = name.split_once(':').map_or(name, |(plugin, _)| plugin);
    ALSA_LISTED.contains(&plugin)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd"
)))]
fn listable(_name: &str) -> bool {
    true
}
