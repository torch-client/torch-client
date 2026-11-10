#[cfg(explore_modrinth)]
pub(crate) mod modrinth;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use parking_lot::Mutex;
use serde_json::Value;

use crate::platform::assets::archive::{self, OwnedLayer};
use crate::platform::packfs::{self, Stamp};
use crate::platform::storage;
use crate::text::Span;
use crate::util::pack::{Pack, Source};

const ROOT: &str = "";

const FORMAT: Format = (88, 0);

type Format = (u32, u32);

pub(crate) fn dir() -> PathBuf {
    packfs::root().join("resourcepacks")
}

pub(crate) fn is_pack_dir(path: &std::path::Path) -> bool {
    packfs::is_file(&path.join("pack.mcmeta"))
}

pub(crate) fn list() -> Vec<Pack> {
    crate::util::pack::scan(&dir())
}

#[cfg(explore_modrinth)]
pub(crate) fn size(name: &str) -> u64 {
    fn walk(path: &std::path::Path, kind: packfs::Kind) -> u64 {
        match kind {
            packfs::Kind::Dir => packfs::entries(path)
                .into_iter()
                .flatten()
                .map(|(name, kind)| walk(&path.join(name), kind))
                .sum(),
            packfs::Kind::File => packfs::len(path).unwrap_or(0),
            packfs::Kind::Other => 0,
        }
    }
    let path = dir().join(name);
    packfs::kind(&path).map_or(0, |kind| walk(&path, kind))
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
    match Source::open(&dir().join(&pack.name), ROOT) {
        Ok(source) => info_of(&source),
        Err(e) => Info {
            description: crate::text::parse_formatted(&e),
            compat: Compat::Unknown,
            icon: None,
        },
    }
}

fn info_of(source: &Source) -> Info {
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

pub(crate) fn icon_pixels(png: &[u8]) -> Option<Vec<u8>> {
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

static INFLATED: LazyLock<Mutex<HashMap<String, (Stamp, Arc<OwnedLayer>)>>> =
    LazyLock::new(Default::default);

pub(crate) static FAILED: crate::util::pack::Failed = crate::util::pack::Failed::new();

pub(crate) fn mount() {
    let started = crate::platform::time::Instant::now();
    let names = selected();
    let stamps: Vec<Option<Stamp>> = names
        .iter()
        .map(|name| packfs::stamp(&dir().join(name)))
        .collect();
    let mut inflated = INFLATED.lock();
    inflated.retain(|name, _| names.contains(name));

    let mut layers: Vec<Option<Arc<OwnedLayer>>> = names
        .iter()
        .zip(&stamps)
        .map(|(name, stamp)| {
            let (held, layer) = inflated.get(name)?;
            (Some(held) == stamp.as_ref()).then(|| layer.clone())
        })
        .collect();
    let missing: Vec<usize> = (0..names.len()).filter(|&i| layers[i].is_none()).collect();
    #[cfg(target_arch = "wasm32")]
    let opened: Vec<Result<OwnedLayer, String>> = missing
        .iter()
        .map(|&i| layer(&dir().join(&names[i])))
        .collect();
    #[cfg(not(target_arch = "wasm32"))]
    let opened: Vec<Result<OwnedLayer, String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = missing
            .iter()
            .map(|&i| {
                let path = dir().join(&names[i]);
                scope.spawn(move || layer(&path))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err("reading it panicked".to_owned()))
            })
            .collect()
    });
    let mut failed = Vec::new();
    for (i, result) in missing.into_iter().zip(opened) {
        let name = &names[i];
        match result {
            Ok(layer) => {
                crate::log_info!("resourcepacks", "{name}: {} files", layer.files.len());
                let layer = Arc::new(layer);
                if let Some(stamp) = stamps[i] {
                    inflated.insert(name.clone(), (stamp, layer.clone()));
                }
                layers[i] = Some(layer);
            }
            Err(e) => {
                inflated.remove(name);
                crate::log_warn!("resourcepacks", "{name} is not mounted: {e}");
                failed.push(name.clone());
            }
        }
    }
    FAILED.set(failed);
    drop(inflated);

    let layers: Vec<Arc<OwnedLayer>> = layers.into_iter().flatten().collect();
    let count = layers.len();
    archive::set_selected(layers);
    if count > 0 {
        crate::log_info!(
            "resourcepacks",
            "mounted {count} pack(s) in {:.0} ms",
            started.elapsed().as_secs_f32() * 1000.0
        );
    }
}

fn layer(path: &std::path::Path) -> Result<OwnedLayer, String> {
    owned(&Source::open(path, ROOT)?)
}

pub(crate) struct ServerPack {
    pub(crate) info: Info,
    pub(crate) zip: Vec<u8>,
    pub(crate) file_name: String,
}

pub(crate) fn server_pack(
    zip: Vec<u8>,
    url: &str,
    sha1: &str,
) -> Result<(OwnedLayer, ServerPack), String> {
    let source = Source::from_zip(zip, ROOT)?;
    let layer = owned(&source)?;
    let info = info_of(&source);
    let zip = source.into_zip().unwrap_or_default();
    Ok((
        layer,
        ServerPack {
            info,
            zip,
            file_name: server_file_name(url, sha1),
        },
    ))
}

fn server_file_name(url: &str, sha1: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let last = path.rsplit('/').next().unwrap_or_default();
    let ext = last
        .len()
        .checked_sub(4)
        .filter(|&at| last.is_char_boundary(at) && last[at..].eq_ignore_ascii_case(".zip"));
    let stem: String = last[..ext.unwrap_or(last.len())]
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(48)
        .collect();
    let stem = stem.trim_matches('.');
    let short = &sha1[..sha1.len().min(8)];
    if stem.is_empty() {
        format!("server-{short}.zip")
    } else {
        format!("{stem}-{short}.zip")
    }
}

pub(crate) fn install(name: &str, bytes: &[u8]) -> Result<(), String> {
    let dir = dir();
    let part = dir.join(format!("{name}.part"));
    packfs::create_dir_all(&dir)
        .and_then(|()| packfs::write(&part, bytes))
        .and_then(|()| packfs::rename(&part, dir.join(name)))
        .map_err(|e| e.to_string())
}

fn owned(source: &Source) -> Result<OwnedLayer, String> {
    let (bytes, files) = inflate(source)?;
    Ok(OwnedLayer {
        bytes: bytes.into_boxed_slice(),
        files: files.into_iter().collect(),
    })
}

fn inflate(source: &Source) -> Result<(Vec<u8>, Vec<(String, std::ops::Range<usize>)>), String> {
    let meta = source
        .read_text("/pack.mcmeta")
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .ok_or("it has no readable pack.mcmeta")?;

    let mut prefixes = vec![String::from("/")];
    prefixes.extend(overlays(&meta).into_iter().map(|dir| format!("/{dir}/")));
    let files = source.files();
    let mut chosen: HashMap<String, &str> = HashMap::new();
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
    Ok((bytes, ranges))
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
    fn a_saved_server_pack_is_named_from_its_url_and_hash() {
        let sha1 = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            server_file_name("https://example.com/p/Cool%20Pack.ZIP?t=1", sha1),
            "Cool20Pack-01234567.zip"
        );
        assert_eq!(
            server_file_name("https://example.com/download/", sha1),
            "server-01234567.zip"
        );
        assert_eq!(
            server_file_name("https://example.com/../..", sha1),
            "server-01234567.zip"
        );
    }

    #[test]
    fn an_overlay_shadows_the_pack() {
        let pack = crate::util::pack::fixture("resourcepack-overlay");
        let file = |rel: &str, body: &str| {
            let at = pack.join(rel);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, body).unwrap();
        };
        let meta = r#"{"pack": {"description": "", "min_format": 88, "max_format": 88},
            "overlays": {"entries": [{"directory": "ov", "min_format": 88, "max_format": 88}]}}"#;
        file("pack.mcmeta", &format!("\u{feff}{meta}"));
        file("assets/minecraft/a.txt", "base");
        file("assets/minecraft/b.txt", "base");
        file("ov/assets/minecraft/a.txt", "overlay");
        file("readme.txt", "not an asset");

        let layer = layer(&pack).unwrap();
        assert_eq!(layer.get("assets/minecraft/a.txt"), Some(&b"overlay"[..]));
        assert_eq!(layer.get("assets/minecraft/b.txt"), Some(&b"base"[..]));
        assert_eq!(layer.files.len(), 2);
    }
}
