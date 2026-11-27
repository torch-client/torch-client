use std::sync::Mutex;
use std::sync::atomic::Ordering;

use super::{LANGUAGES_KEY, META_KEY, Stage, call_page, download, pack, store};
use crate::log_info;
use crate::log_warn;

const EXPORT_KEY: &str = ".export";

const LANGUAGE_KEY: &str = ".language";

#[cfg(feature = "audio")]
use crate::audio::store::{DEFS_KEY as SOUND_DEFS_KEY, PREFIX as SOUND_PREFIX};
#[cfg(not(feature = "audio"))]
const SOUND_DEFS_KEY: &str = ".sounds.json";
#[cfg(not(feature = "audio"))]
const SOUND_PREFIX: &str = ".sounds/";

pub(crate) fn file_name() -> String {
    format!("torch-assets-{}.mcasset", crate::ASSET_VERSION)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Part {
    Jar,
    Panorama,
    Unifont,
    Sounds,
    Languages,
    Language,
}

impl Part {
    pub(crate) const ALL: [Part; 6] = [
        Part::Jar,
        Part::Panorama,
        Part::Unifont,
        Part::Sounds,
        Part::Languages,
        Part::Language,
    ];

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn caption(self) -> &'static str {
        match self {
            Part::Jar => crate::lang::tr!("asset_export.part.jar"),
            Part::Panorama => crate::lang::tr!("asset_export.part.panorama"),
            Part::Unifont => crate::lang::tr!("asset_export.part.unifont"),
            Part::Sounds => crate::lang::tr!("asset_export.part.sounds"),
            Part::Languages => crate::lang::tr!("asset_export.part.languages"),
            Part::Language => crate::lang::tr!("asset_export.part.language"),
        }
    }

    pub(crate) fn default_on(self) -> bool {
        !matches!(self, Part::Jar | Part::Language)
    }

    fn word(self) -> &'static str {
        match self {
            Part::Jar => "jar",
            Part::Panorama => "panorama",
            Part::Unifont => "unifont",
            Part::Sounds => "sounds",
            Part::Languages => "languages",
            Part::Language => "language",
        }
    }

    fn bit(self) -> u8 {
        1 << self.index()
    }
}

fn part_of(key: &str) -> Option<Part> {
    if key == META_KEY || key == EXPORT_KEY {
        return None;
    }
    Some(if key == LANGUAGE_KEY {
        Part::Language
    } else if key == LANGUAGES_KEY {
        Part::Languages
    } else if key == SOUND_DEFS_KEY || key.starts_with(SOUND_PREFIX) {
        Part::Sounds
    } else if (0..6).any(|face| key == super::panorama_face(face)) {
        Part::Panorama
    } else if key == format!("{}{}", super::ASSET_KEY_PREFIX, super::UNIFONT_JSON_KEY)
        || key == format!("{}{}", super::ASSET_KEY_PREFIX, super::UNIFONT_ZIP_KEY)
    {
        Part::Unifont
    } else {
        Part::Jar
    })
}

fn words(parts: u8, sep: &str) -> String {
    Part::ALL
        .iter()
        .filter(|p| parts & p.bit() != 0)
        .map(|p| p.word())
        .collect::<Vec<_>>()
        .join(sep)
}

pub(super) fn current_entries() -> Vec<(String, Vec<u8>)> {
    if let Some(built) = store::INSTALLED.lock().unwrap().as_deref()
        && let Ok(entries) = read(built)
    {
        return entries;
    }
    crate::platform::assets::archive::entries()
        .into_iter()
        .map(|(key, bytes)| (key.to_owned(), bytes.to_vec()))
        .collect()
}

pub(crate) struct Offer {
    pub(crate) available: [bool; 6],
    pub(crate) sizes: [u64; 6],
    pub(crate) language: String,
}

pub(crate) fn offer() -> Offer {
    let entries = current_entries();
    let get = |key: &str| {
        entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, b)| b.as_slice())
    };
    let mut sizes = [0u64; 6];
    for (key, bytes) in &entries {
        if let Some(part) = part_of(key) {
            sizes[part.index()] += bytes.len() as u64;
        }
    }
    let stored = crate::platform::storage::LANGUAGE
        .load()
        .unwrap_or_default();
    let language = super::super::language::current();
    let english = language == super::super::language::ENGLISH;
    sizes[Part::Language.index()] = if english { 0 } else { stored.len() as u64 };

    let mut available = [false; 6];
    available[Part::Jar.index()] = sizes[Part::Jar.index()] > 0;
    available[Part::Panorama.index()] = !super::panorama_is_stubbed(&get);
    available[Part::Unifont.index()] = !super::unifont_is_stubbed(&get);
    available[Part::Sounds.index()] = get(SOUND_DEFS_KEY).is_some();
    available[Part::Languages.index()] = get(LANGUAGES_KEY).is_some();
    available[Part::Language.index()] = !english && !stored.is_empty();
    Offer {
        available,
        sizes,
        language,
    }
}

pub(crate) fn export(on: [bool; 6]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut parts = 0u8;
    for (key, bytes) in current_entries() {
        match part_of(&key) {
            Some(part) if on[part.index()] => {
                parts |= part.bit();
                out.push((key, bytes));
            }
            Some(_) => {}
            None if key == META_KEY => out.push((key, bytes)),
            None => {}
        }
    }
    if on[Part::Language.index()]
        && let Some(stored) = crate::platform::storage::LANGUAGE
            .load()
            .filter(|s| !s.is_empty())
    {
        parts |= Part::Language.bit();
        out.push((LANGUAGE_KEY.to_owned(), stored.into_bytes()));
    }
    if !out.iter().any(|(k, _)| k == META_KEY) {
        out.push((
            META_KEY.to_owned(),
            super::meta_blob(crate::ASSET_VERSION, None),
        ));
    }
    out.push((EXPORT_KEY.to_owned(), words(parts, "\n").into_bytes()));
    log_info!("assets", "exporting {}", words(parts, ", "));
    pack(out)
}

#[derive(Clone, Debug)]
pub(crate) enum Refused {
    NotOurs,
    Version(String),
    Empty,
}

impl Refused {
    pub(crate) fn short(&self) -> String {
        match self {
            Refused::NotOurs => crate::lang::tr!("install.refused.not_ours").to_owned(),
            Refused::Version(v) => {
                let v: String = v
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
                    .collect();
                if v.is_empty() {
                    crate::lang::trf!("install.refused.not_for", crate::ASSET_VERSION)
                } else {
                    crate::lang::trf!("install.refused.made_for", v, crate::ASSET_VERSION)
                }
            }
            Refused::Empty => crate::lang::tr!("install.refused.empty").to_owned(),
        }
    }

    pub(crate) fn long(&self) -> String {
        match self {
            Refused::NotOurs => crate::lang::tr!("asset_export.refused.not_ours").to_owned(),
            Refused::Version(v) => {
                crate::lang::trf!("asset_export.refused.version", v, crate::ASSET_VERSION)
            }
            Refused::Empty => crate::lang::tr!("asset_export.refused.empty").to_owned(),
        }
    }
}

fn read(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Refused> {
    let (table, mut at) =
        crate::platform::assets::read_table(&mut &bytes[..]).map_err(|_| Refused::NotOurs)?;
    let blobs: u64 = table.iter().map(|(_, n)| u64::from(*n)).sum();
    if at + blobs != bytes.len() as u64 {
        return Err(Refused::NotOurs);
    }
    Ok(table
        .into_iter()
        .map(|(key, n)| {
            let start = at as usize;
            at += u64::from(n);
            (key, bytes[start..at as usize].to_vec())
        })
        .collect())
}

fn validate(bytes: &[u8]) -> Result<(Vec<(String, Vec<u8>)>, u8), Refused> {
    let mut entries = read(bytes)?;
    entries.retain(|(k, _)| k != EXPORT_KEY);
    let meta: serde_json::Value = entries
        .iter()
        .find(|(k, _)| k == META_KEY)
        .and_then(|(_, b)| serde_json::from_slice(b).ok())
        .ok_or(Refused::NotOurs)?;
    let version = meta["version"].as_str().unwrap_or("");
    if version != crate::ASSET_VERSION {
        return Err(Refused::Version(version.to_owned()));
    }
    let parts = entries
        .iter()
        .filter_map(|(k, _)| part_of(k))
        .fold(0u8, |acc, p| acc | p.bit());
    if parts == 0 {
        return Err(Refused::Empty);
    }
    Ok((entries, parts))
}

fn apply_language(entries: &mut Vec<(String, Vec<u8>)>) {
    let Some(at) = entries.iter().position(|(k, _)| k == LANGUAGE_KEY) else {
        return;
    };
    let (_, bytes) = entries.swap_remove(at);
    match String::from_utf8(bytes)
        .ok()
        .and_then(|s| super::super::language::import_stored(&s))
    {
        Some(code) => log_info!("assets", "imported language {code}"),
        None => log_warn!("assets", "the imported language could not be applied"),
    }
}

fn keep(entries: Vec<(String, Vec<u8>)>) {
    let _ = store::save_version(crate::ASSET_VERSION, &pack(entries));
    if let Err(e) = call_page("keepStoredAssets") {
        log_warn!(
            "assets",
            "{e}; the imported assets last until the tab closes"
        );
    }
}

static KEPT: Mutex<Option<(Vec<(String, Vec<u8>)>, u8)>> = Mutex::new(None);

pub(crate) enum Prepared {
    Installed,
    Kept(String),
}

pub(crate) fn import_for_install(bytes: &[u8]) -> Result<Prepared, Refused> {
    let (mut entries, parts) = validate(bytes)?;
    apply_language(&mut entries);
    if parts & Part::Jar.bit() != 0 {
        log_info!("assets", "imported a whole set: {}", words(parts, ", "));
        *KEPT.lock().unwrap() = None;
        keep(entries);
        download().set(Stage::Done);
        return Ok(Prepared::Installed);
    }
    let kept = parts & !Part::Language.bit();
    log_info!("assets", "keeping {} for the install", words(kept, ", "));
    *KEPT.lock().unwrap() = Some((entries, kept));
    Ok(Prepared::Kept(words(kept, " ")))
}

pub(crate) fn kept_sounds() -> bool {
    KEPT.lock()
        .unwrap()
        .as_ref()
        .is_some_and(|(_, parts)| parts & Part::Sounds.bit() != 0)
}

pub(super) fn kept() -> super::Kept {
    let Some((entries, parts)) = KEPT.lock().unwrap().clone() else {
        return super::Kept::default();
    };
    let mut objects = 0;
    if parts & Part::Panorama.bit() != 0 {
        objects |= super::piece::PANORAMA;
    }
    if parts & Part::Unifont.bit() != 0 {
        objects |= super::piece::UNIFONT;
    }
    super::Kept {
        entries,
        objects,
        languages: parts & Part::Languages.bit() != 0,
    }
}

pub(super) fn forget_kept() {
    *KEPT.lock().unwrap() = None;
}

pub(crate) fn import_into_running(bytes: &[u8]) -> Result<String, Refused> {
    let (mut file, parts) = validate(bytes)?;
    apply_language(&mut file);

    let mut entries = current_entries();
    if parts & Part::Sounds.bit() != 0 {
        entries.retain(|(k, _)| part_of(k) != Some(Part::Sounds));
    }
    let mut sounds = Vec::new();
    for (key, bytes) in file {
        if key == META_KEY {
            continue;
        }
        if part_of(&key) == Some(Part::Sounds) {
            sounds.push((key.clone(), bytes.clone()));
        }
        if part_of(&key) == Some(Part::Languages) {
            super::super::language::adopt(&bytes);
        }
        match entries.iter().position(|(k, _)| *k == key) {
            Some(at) => entries[at].1 = bytes,
            None => entries.push((key, bytes)),
        }
    }
    if !entries.iter().any(|(k, _)| k == META_KEY) {
        entries.push((
            META_KEY.to_owned(),
            super::meta_blob(crate::ASSET_VERSION, None),
        ));
    }
    keep(entries);

    #[cfg_attr(not(feature = "audio"), allow(unused_mut))]
    let mut reload = parts & (Part::Jar.bit() | Part::Panorama.bit() | Part::Unifont.bit()) != 0;
    #[cfg(feature = "audio")]
    if !sounds.is_empty() {
        if crate::audio::installed() {
            reload = true;
        } else if let Some(set) = crate::audio::store::Sounds::from_entries(sounds) {
            crate::audio::adopt(set);
        }
    }
    #[cfg(not(feature = "audio"))]
    drop(sounds);
    if reload {
        super::RESTART_PENDING.store(true, Ordering::Relaxed);
    }
    super::MISSING.store(0, Ordering::Relaxed);

    let key = if reload {
        const { crate::lang::Key::of("asset_export.imported_reload") }
    } else {
        const { crate::lang::Key::of("asset_export.imported") }
    };
    let args = [words(parts, ", ")];
    log_info!(
        "assets",
        "{}",
        crate::util::placeholders::substitute(key.english(), &args)
    );
    Ok(crate::util::placeholders::substitute(key.get(), &args))
}
