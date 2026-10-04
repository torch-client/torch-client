use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

use crate::platform::assets::archive::{self, Layer};
use crate::platform::storage;
use crate::text::Span;
use crate::util::pack::{Pack, Source};

const ROOT: &str = "";

const FORMAT: Format = (84, 0);

type Format = (u32, u32);

pub(crate) fn dir() -> PathBuf {
    storage::dir().join("resourcepacks")
}

pub(crate) fn list() -> Vec<Pack> {
    crate::util::pack::scan(&dir())
}

pub(crate) fn selected() -> Vec<String> {
    storage::RESOURCE_PACKS
        .load()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(crate) fn save_selected(names: &[String]) {
    match serde_json::to_string(names) {
        Ok(text) => storage::RESOURCE_PACKS.store(&text),
        Err(e) => crate::log_warn!("resourcepacks", "could not encode the selection: {e}"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Compat {
    Compatible,
    TooOld,
    TooNew,
    Unknown,
}

impl Compat {
    fn of(range: Option<(Format, Format)>) -> Compat {
        match range {
            None => Compat::Unknown,
            Some((min, _)) if FORMAT < min => Compat::TooNew,
            Some((_, max)) if FORMAT > max => Compat::TooOld,
            Some(_) => Compat::Compatible,
        }
    }
}

pub(crate) struct Info {
    pub(crate) description: Vec<Span>,
    pub(crate) compat: Compat,
    pub(crate) icon: Option<Vec<u8>>,
}

pub(crate) fn info(pack: &Pack) -> Info {
    let source = match Source::open(&dir().join(&pack.name), ROOT) {
        Ok(source) => source,
        Err(e) => {
            return Info {
                description: crate::text::parse_formatted(&e),
                compat: Compat::Unknown,
                icon: None,
            };
        }
    };
    let meta = source
        .read_text("/pack.mcmeta")
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let pack_section = meta.as_ref().map(|m| &m["pack"]);
    let description = pack_section
        .and_then(|p| {
            serde_json::from_value::<azalea_chat::FormattedText>(p["description"].clone()).ok()
        })
        .map(|text| crate::client::chat_text::to_spans(&text))
        .unwrap_or_default();
    Info {
        description,
        compat: Compat::of(pack_section.and_then(pack_range)),
        icon: source
            .read("/pack.png")
            .and_then(|bytes| icon_pixels(&bytes)),
    }
}

fn icon_pixels(png: &[u8]) -> Option<Vec<u8>> {
    let side = crate::gui::atlas::SERVER_ICON_PX;
    let image = image::load_from_memory(png).ok()?.to_rgba8();
    let image = image::imageops::resize(&image, side, side, image::imageops::FilterType::Triangle);
    Some(image.into_raw())
}

fn pack_range(section: &Value) -> Option<(Format, Format)> {
    explicit_range(section).unwrap_or_else(|| {
        old_range(&section["supported_formats"]).or_else(|| {
            section["pack_format"]
                .as_u64()
                .map(|n| ((n as u32, 0), (n as u32, 0)))
        })
    })
}

fn overlay_range(entry: &Value) -> Option<(Format, Format)> {
    explicit_range(entry).unwrap_or_else(|| old_range(&entry["formats"]))
}

fn explicit_range(v: &Value) -> Option<Option<(Format, Format)>> {
    let bound = |field: &Value, default_minor: u32| -> Option<Format> {
        match field {
            Value::Number(n) => Some((n.as_u64()? as u32, default_minor)),
            Value::Array(parts) => {
                let major = parts.first()?.as_u64()? as u32;
                let minor = parts
                    .get(1)
                    .and_then(Value::as_u64)
                    .map_or(default_minor, |m| m as u32);
                Some((major, minor))
            }
            _ => None,
        }
    };
    match (
        bound(&v["min_format"], 0),
        bound(&v["max_format"], u32::MAX),
    ) {
        (None, None) => None,
        (Some(min), Some(max)) => Some(Some((min, max))),
        _ => Some(None),
    }
}

fn old_range(v: &Value) -> Option<(Format, Format)> {
    let (min, max) = match v {
        Value::Number(n) => (n.as_u64()?, n.as_u64()?),
        Value::Array(parts) => (parts.first()?.as_u64()?, parts.get(1)?.as_u64()?),
        Value::Object(_) => (v["min_inclusive"].as_u64()?, v["max_inclusive"].as_u64()?),
        _ => return None,
    };
    Some(((min as u32, 0), (max as u32, u32::MAX)))
}

fn overlays(meta: &Value) -> Vec<String> {
    let Some(entries) = meta["overlays"]["entries"].as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|entry| {
            overlay_range(entry).is_some_and(|(min, max)| (min..=max).contains(&FORMAT))
        })
        .filter_map(|entry| entry["directory"].as_str())
        .filter(|dir| {
            !dir.is_empty()
                && dir
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        })
        .map(str::to_owned)
        .collect()
}

pub(crate) fn mount() {
    let started = crate::platform::time::Instant::now();
    let mut layers = Vec::new();
    for name in selected() {
        match layer(&dir().join(&name)) {
            Ok(layer) => {
                crate::log_info!("resourcepacks", "{name}: {} files", layer.len());
                layers.push(layer);
            }
            Err(e) => crate::log_warn!("resourcepacks", "{name} is not mounted: {e}"),
        }
    }
    if layers.is_empty() {
        return;
    }
    let count = layers.len();
    if let Err(e) = archive::PACKS.install(layers) {
        crate::log_warn!("resourcepacks", "{e}");
        return;
    }
    crate::log_info!(
        "resourcepacks",
        "mounted {count} pack(s) in {:.0} ms",
        started.elapsed().as_secs_f32() * 1000.0
    );
}

fn layer(path: &std::path::Path) -> Result<Layer, String> {
    let source = Source::open(path, ROOT)?;
    let meta = source
        .read_text("/pack.mcmeta")
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .ok_or("it has no readable pack.mcmeta")?;

    let mut prefixes = vec![String::from("/")];
    prefixes.extend(overlays(&meta).into_iter().map(|dir| format!("/{dir}/")));
    let files = source.files();
    let mut chosen: std::collections::HashMap<String, &str> = std::collections::HashMap::new();
    for prefix in &prefixes {
        for file in &files {
            if let Some(key) = file
                .strip_prefix(prefix.as_str())
                .filter(|key| key.starts_with("assets/"))
            {
                chosen.insert(key.to_owned(), file);
            }
        }
    }

    let mut bytes = Vec::new();
    let mut ranges = Vec::with_capacity(chosen.len());
    for (key, file) in chosen {
        let Some(data) = source.read(file) else {
            continue;
        };
        ranges.push((key, bytes.len()..bytes.len() + data.len()));
        bytes.extend_from_slice(&data);
    }
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    Ok(ranges
        .into_iter()
        .map(|(key, range)| (key, &bytes[range]))
        .collect())
}

static RESTART: AtomicBool = AtomicBool::new(false);

pub(crate) fn request_restart() {
    RESTART.store(true, Ordering::Relaxed);
}

pub(crate) fn restart_if_requested() {
    if !RESTART.load(Ordering::Relaxed) {
        return;
    }
    let spawned = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .spawn()
    });
    if let Err(e) = spawned {
        crate::log_warn!(
            "resourcepacks",
            "could not relaunch the client: {e}; start it again to use the new packs"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(json: &str) -> Option<(Format, Format)> {
        pack_range(&serde_json::from_str(json).unwrap())
    }

    #[test]
    fn the_new_fields_take_a_major_or_a_major_and_minor() {
        assert_eq!(
            range(r#"{"min_format": 84, "max_format": 84}"#),
            Some(((84, 0), (84, u32::MAX)))
        );
        assert_eq!(
            range(r#"{"min_format": [80, 1], "max_format": [84, 0]}"#),
            Some(((80, 1), (84, 0)))
        );
    }

    #[test]
    fn half_of_the_new_pair_is_no_range() {
        assert_eq!(range(r#"{"min_format": 84, "pack_format": 34}"#), None);
    }

    #[test]
    fn the_old_fields_still_read() {
        assert_eq!(range(r#"{"pack_format": 34}"#), Some(((34, 0), (34, 0))));
        assert_eq!(
            range(r#"{"supported_formats": [18, 34]}"#),
            Some(((18, 0), (34, u32::MAX)))
        );
        assert_eq!(
            range(r#"{"supported_formats": {"min_inclusive": 18, "max_inclusive": 34}}"#),
            Some(((18, 0), (34, u32::MAX)))
        );
    }

    #[test]
    fn compatibility_is_against_this_version() {
        assert_eq!(Compat::of(range(r#"{"pack_format": 34}"#)), Compat::TooOld);
        assert_eq!(
            Compat::of(range(r#"{"min_format": 90, "max_format": 99}"#)),
            Compat::TooNew
        );
        assert_eq!(
            Compat::of(range(r#"{"min_format": 70, "max_format": 99}"#)),
            Compat::Compatible
        );
        assert_eq!(Compat::of(range(r#"{}"#)), Compat::Unknown);
    }

    #[test]
    fn only_overlays_for_this_version_with_plain_names_apply() {
        let meta: Value = serde_json::from_str(
            r#"{"overlays": {"entries": [
                {"directory": "old", "formats": [18, 34]},
                {"directory": "now", "min_format": 80, "max_format": 90},
                {"directory": "../escape", "min_format": 80, "max_format": 90}
            ]}}"#,
        )
        .unwrap();
        assert_eq!(overlays(&meta), ["now"]);
    }

    #[test]
    fn an_overlay_shadows_the_pack() {
        let pack = crate::util::pack::fixture("resourcepack-overlay");
        let file = |rel: &str, body: &str| {
            let at = pack.join(rel);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, body).unwrap();
        };
        let meta = r#"{"pack": {"description": "", "min_format": 84, "max_format": 84},
            "overlays": {"entries": [{"directory": "ov", "min_format": 84, "max_format": 84}]}}"#;
        file("pack.mcmeta", &format!("\u{feff}{meta}"));
        file("assets/minecraft/a.txt", "base");
        file("assets/minecraft/b.txt", "base");
        file("ov/assets/minecraft/a.txt", "overlay");
        file("readme.txt", "not an asset");

        let layer = layer(&pack).unwrap();
        assert_eq!(
            layer.get("assets/minecraft/a.txt").copied(),
            Some(&b"overlay"[..])
        );
        assert_eq!(
            layer.get("assets/minecraft/b.txt").copied(),
            Some(&b"base"[..])
        );
        assert_eq!(layer.len(), 2);
    }
}
