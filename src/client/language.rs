use std::sync::Arc;

use parking_lot::Mutex;

use super::assets::{self, Failure};
use crate::platform::storage;
use crate::{log_info, log_warn};

pub(crate) const ENGLISH: &str = "en_us";

#[derive(Clone, Debug)]
pub(crate) struct Language {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) region: String,
    hash: String,
}

impl Language {
    pub(crate) fn label(&self) -> String {
        format!("{} ({})", self.name, self.region)
    }

    fn english() -> Language {
        Language {
            code: ENGLISH.to_owned(),
            name: "English".to_owned(),
            region: "US".to_owned(),
            hash: String::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Fetch {
    Idle,
    Fetching(String),
    Failed(String, &'static str),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ListFetch {
    Idle,
    Fetching,
    Failed(&'static str),
}

static LIST: Mutex<Option<Arc<Vec<Language>>>> = Mutex::new(None);
static FETCH: Mutex<Fetch> = Mutex::new(Fetch::Idle);
static LIST_FETCH: Mutex<ListFetch> = Mutex::new(ListFetch::Idle);

static APPLIED: Mutex<Option<Applied>> = Mutex::new(None);

#[derive(Clone)]
struct Applied {
    code: String,
    text: Arc<str>,
}

impl Applied {
    fn english() -> Applied {
        Applied {
            code: ENGLISH.to_owned(),
            text: Arc::from(""),
        }
    }
}

fn applied() -> Applied {
    APPLIED.lock().get_or_insert_with(read_stored).clone()
}

fn read_stored() -> Applied {
    let stored = storage::LANGUAGE.load().unwrap_or_default();
    match stored.split_once('\n') {
        Some((code, text))
            if !code.trim().is_empty() && code.trim() != ENGLISH && !text.is_empty() =>
        {
            Applied {
                code: code.trim().to_owned(),
                text: Arc::from(text),
            }
        }
        _ => Applied::english(),
    }
}

pub(crate) fn current() -> String {
    applied().code
}

pub(crate) fn strings() -> Option<(String, Arc<str>)> {
    let applied = applied();
    (applied.code != ENGLISH).then_some((applied.code, applied.text))
}

fn commit(applied: Applied) -> bool {
    let stored = if applied.code == ENGLISH {
        String::new()
    } else {
        format!("{}\n{}", applied.code, applied.text)
    };
    log_info!("language", "applied {}", applied.code);
    *APPLIED.lock() = Some(applied);
    storage::LANGUAGE.store(&stored);
    crate::gui::tooltip::reset_lang();
    storage::LANGUAGE.load().unwrap_or_default() == stored
}

pub(crate) fn apply(lang: &Language) {
    if lang.code == ENGLISH {
        *FETCH.lock() = Fetch::Idle;
        commit(Applied::english());
        return;
    }
    {
        let mut fetch = FETCH.lock();
        if matches!(*fetch, Fetch::Fetching(_)) {
            return;
        }
        *fetch = Fetch::Fetching(lang.code.clone());
    }
    let (code, hash) = (lang.code.clone(), lang.hash.clone());
    log_info!("language", "fetching {code}");
    spawn(move || async move {
        match fetch_file(&hash).await {
            Ok(text) => {
                let saved = commit(Applied {
                    code: code.clone(),
                    text: Arc::from(text),
                });
                *FETCH.lock() = if saved {
                    Fetch::Idle
                } else {
                    log_warn!("language", "{code} is in use but could not be saved");
                    Fetch::Failed(code, "Could not save it for next time")
                };
            }
            Err(e) => {
                log_warn!("language", "{code}: {}", e.detail);
                *FETCH.lock() = Fetch::Failed(code, e.short);
            }
        }
    });
}

pub(crate) fn fetch() -> Fetch {
    FETCH.lock().clone()
}

async fn fetch_file(hash: &str) -> Result<String, Failure> {
    let bytes = assets::get_object(hash, &|| false)
        .await?
        .ok_or_else(|| Failure::plain("The download was interrupted"))?;
    let text =
        String::from_utf8(bytes).map_err(|e| Failure::new("The language file is not text", e))?;
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Object(_)) => Ok(text),
        Ok(_) => Err(Failure::plain("The language file is not readable")),
        Err(e) => Err(Failure::new("The language file is not readable", e)),
    }
}

pub(crate) fn list() -> Option<Arc<Vec<Language>>> {
    LIST.lock().clone()
}

pub(crate) fn list_fetch() -> ListFetch {
    *LIST_FETCH.lock()
}

pub(crate) fn request_list() -> bool {
    if LIST.lock().is_some() {
        return false;
    }
    if let Some(table) = assets::archived_languages() {
        adopt(table);
    } else if let Some(stored) = storage::LANGUAGE_LIST.load()
        && let Some(table) = stored_table(&stored, crate::ASSET_VERSION)
    {
        adopt(table.as_bytes());
    }
    if LIST.lock().is_some() {
        return false;
    }
    {
        let mut fetch = LIST_FETCH.lock();
        if *fetch == ListFetch::Fetching {
            return true;
        }
        *fetch = ListFetch::Fetching;
    }
    log_info!("language", "fetching the list");
    spawn(|| async {
        let state = match fetch_list().await {
            Ok(table) => {
                adopt(table.as_bytes());
                storage::LANGUAGE_LIST.store(&format!("{}\n{table}", crate::ASSET_VERSION));
                ListFetch::Idle
            }
            Err(e) => {
                log_warn!("language", "no language list: {}", e.detail);
                ListFetch::Failed(e.short)
            }
        };
        *LIST_FETCH.lock() = state;
    });
    true
}

fn stored_table<'a>(stored: &'a str, version: &str) -> Option<&'a str> {
    stored
        .split_once('\n')
        .filter(|(v, _)| *v == version)
        .map(|(_, table)| table)
}

async fn fetch_list() -> Result<String, Failure> {
    let manifest = assets::get_json(assets::VERSION_MANIFEST).await?;
    let meta = assets::get_json(&assets::pick_version(&manifest, crate::ASSET_VERSION)?).await?;
    let index_url = meta["assetIndex"]["url"]
        .as_str()
        .ok_or_else(|| Failure::plain("Mojang sent no asset index"))?;
    let index = assets::get_json(index_url).await?;
    let table = table_from_index(&index, &|| false)
        .await?
        .ok_or_else(|| Failure::plain("The download was interrupted"))?;
    String::from_utf8(table).map_err(|e| Failure::new("Mojang sent something unreadable", e))
}

pub(crate) fn retry() {
    let mut fetch = FETCH.lock();
    if matches!(*fetch, Fetch::Failed(..)) {
        *fetch = Fetch::Idle;
    }
}

pub(crate) fn english_only() -> Arc<Vec<Language>> {
    Arc::new(vec![Language::english()])
}

pub(crate) fn adopt(bytes: &[u8]) {
    let languages = parse_table(bytes);
    match &languages {
        Some(languages) => log_info!("language", "{} languages listed", languages.len()),
        None => log_warn!("language", "the installed language list is unreadable"),
    }
    *LIST.lock() = languages.map(Arc::new);
}

pub(super) async fn table_from_index(
    index: &serde_json::Value,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<u8>>, Failure> {
    let objects = index["objects"]
        .as_object()
        .ok_or_else(|| Failure::plain("Mojang sent something unreadable"))?;
    let hash_of = |name: &str| {
        objects
            .get(name)
            .and_then(|o| o["hash"].as_str())
            .filter(|h| h.len() >= 2)
            .map(str::to_owned)
    };

    let meta_hash = hash_of("pack.mcmeta")
        .ok_or_else(|| Failure::plain("The asset index lists no languages"))?;
    let Some(bytes) = assets::get_object(&meta_hash, cancelled).await? else {
        return Ok(None);
    };
    let mcmeta: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| Failure::new("Mojang sent something unreadable", e))?;
    let table = mcmeta["language"]
        .as_object()
        .ok_or_else(|| Failure::plain("The asset index lists no languages"))?;

    let mut out = Vec::with_capacity(table.len());
    for (code, info) in table {
        let hash = if code == ENGLISH {
            String::new()
        } else {
            match hash_of(&format!("{}lang/{code}.json", assets::NAMESPACE_PREFIX)) {
                Some(hash) => hash,
                None => {
                    log_warn!("language", "{code} is listed but has no file in the index");
                    continue;
                }
            }
        };
        out.push(Language {
            code: code.clone(),
            name: info["name"].as_str().unwrap_or(code).to_owned(),
            region: info["region"].as_str().unwrap_or("").to_owned(),
            hash,
        });
    }
    if !out.iter().any(|l| l.code == ENGLISH) {
        out.push(Language::english());
    }
    out.sort_by(|a, b| a.code.cmp(&b.code));
    log_info!("language", "{} languages for the archive", out.len());
    Ok(Some(write_table(&out)))
}

fn write_table(languages: &[Language]) -> Vec<u8> {
    serde_json::Value::Array(
        languages
            .iter()
            .map(|l| serde_json::json!({ "code": l.code, "name": l.name, "region": l.region, "hash": l.hash }))
            .collect(),
    )
    .to_string()
    .into_bytes()
}

fn parse_table(bytes: &[u8]) -> Option<Vec<Language>> {
    let doc: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let languages: Vec<Language> = doc
        .as_array()?
        .iter()
        .filter_map(|l| {
            Some(Language {
                code: l["code"].as_str()?.to_owned(),
                name: l["name"].as_str()?.to_owned(),
                region: l["region"].as_str()?.to_owned(),
                hash: l["hash"].as_str()?.to_owned(),
            })
        })
        .collect();
    (!languages.is_empty()).then_some(languages)
}

fn spawn<F, Fut>(make: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    #[cfg(target_arch = "wasm32")]
    crate::platform::executor::spawn(make());

    #[cfg(not(target_arch = "wasm32"))]
    {
        let spawned = std::thread::Builder::new()
            .name("language".into())
            .spawn(move || crate::platform::executor::block_on(make()));
        if let Err(e) = spawned {
            log_warn!("language", "could not start the fetch: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_reads_name_then_region() {
        assert_eq!(Language::english().label(), "English (US)");
    }

    #[test]
    fn the_archive_table_round_trips() {
        let de = Language {
            code: "de_de".to_owned(),
            name: "Deutsch".to_owned(),
            region: "Deutschland".to_owned(),
            hash: "abcd".to_owned(),
        };
        let back = parse_table(&write_table(&[Language::english(), de])).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].label(), "Deutsch (Deutschland)");
        assert_eq!(back[1].hash, "abcd");
        assert!(parse_table(b"{}").is_none());
    }

    #[test]
    fn a_stored_list_needs_its_own_version() {
        assert_eq!(stored_table("26.1.1\n[]", "26.1.1"), Some("[]"));
        assert_eq!(stored_table("26.1.0\n[]", "26.1.1"), None);
        assert_eq!(stored_table("[]", "26.1.1"), None);
    }
}
