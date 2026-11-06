use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde_json::Value as Json;

use super::load;
use crate::platform::http::{self, Fetched};
use crate::platform::storage::MODS_ONLINE;

const BASE: &str = "https://torchclient.com/mods-2026";

const MAX_INDEX: u64 = 64 << 10;
const MAX_ZIP: u64 = 16 << 20;
const MAX_MODS: usize = 256;

#[derive(Clone)]
pub enum Phase {
    Idle,
    Fetching { done: usize, total: usize },
    Ready,
    Failed(String),
}

pub struct Remote {
    pub id: String,
    pub name: String,
    pub version: String,
    pub license: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub source: Option<String>,
    pub perms: Vec<&'static str>,
    bytes: Arc<Vec<u8>>,
}

impl Remote {
    fn read(id: &str, bytes: Vec<u8>) -> Result<Remote, String> {
        let bytes = Arc::new(bytes);
        let i = load::inspect_zip(id, bytes.clone())?;
        Ok(Remote {
            id: id.to_string(),
            name: i.name.unwrap_or_else(|| id.to_string()),
            version: i.version.unwrap_or_default(),
            license: i.license,
            description: i.description,
            author: i.author,
            source: i.source,
            perms: i.perms,
            bytes,
        })
    }

    pub fn install(&self) {
        let c = super::install::from_memory(
            &self.id,
            &self.name,
            &self.version,
            &self.perms,
            self.bytes.clone(),
        );
        super::install::install(vec![c]);
    }
}

pub struct Listing {
    pub updated: String,
    pub mods: Vec<Remote>,
    pub refused: Vec<String>,
}

pub struct View {
    pub allowed: bool,
    pub phase: Phase,
    pub listing: Option<Arc<Listing>>,
}

struct State {
    allowed: Option<bool>,
    serial: u64,
    phase: Phase,
    listing: Option<Arc<Listing>>,
}

impl State {
    fn allowed(&mut self) -> bool {
        *self
            .allowed
            .get_or_insert_with(|| MODS_ONLINE.load().is_some_and(|t| t.trim() == "true"))
    }
}

static STATE: Mutex<State> = Mutex::new(State {
    allowed: None,
    serial: 0,
    phase: Phase::Idle,
    listing: None,
});

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn view() -> View {
    let mut s = state();
    View {
        allowed: s.allowed(),
        phase: s.phase.clone(),
        listing: s.listing.clone(),
    }
}

pub fn allow() {
    MODS_ONLINE.store("true");
    state().allowed = Some(true);
    refresh();
}

pub fn forbid() {
    MODS_ONLINE.store("false");
    let mut s = state();
    s.allowed = Some(false);
    s.serial += 1;
    s.phase = Phase::Idle;
    s.listing = None;
}

pub fn refresh() {
    let (serial, known) = {
        let mut s = state();
        if !s.allowed() || matches!(s.phase, Phase::Fetching { .. }) {
            return;
        }
        s.serial += 1;
        s.phase = Phase::Fetching { done: 0, total: 0 };
        (
            s.serial,
            s.listing
                .as_ref()
                .filter(|l| l.refused.is_empty())
                .map(|l| l.updated.clone()),
        )
    };
    #[cfg(target_arch = "wasm32")]
    crate::platform::executor::spawn(run(serial, known));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let spawned = std::thread::Builder::new()
            .name("mods-online".to_string())
            .spawn(move || crate::platform::executor::block_on(run(serial, known)));
        if let Err(e) = spawned {
            finish(
                serial,
                Err(format!("could not start the download thread: {e}")),
            );
        }
    }
}

async fn run(serial: u64, known: Option<String>) {
    let got = fetch(serial, known).await;
    finish(serial, got);
}

fn finish(serial: u64, got: Result<Option<Listing>, String>) {
    let mut s = state();
    if s.serial != serial {
        return;
    }
    match got {
        Ok(listing) => {
            if let Some(l) = listing {
                s.listing = Some(Arc::new(l));
            }
            s.phase = Phase::Ready;
        }
        Err(e) => {
            crate::log_warn!("mods", "could not load online mods: {e}");
            s.phase = Phase::Failed(e);
        }
    }
}

fn live(serial: u64) -> bool {
    state().serial == serial
}

async fn fetch(serial: u64, known: Option<String>) -> Result<Option<Listing>, String> {
    let body = get(&format!("{BASE}/index.json"), MAX_INDEX, serial).await?;
    let (updated, ids, mut refused) = parse_index(&body)?;
    if known.as_deref() == Some(updated.as_str()) {
        return Ok(None);
    }
    let total = ids.len();
    let mut mods = Vec::with_capacity(total);
    for (done, id) in ids.iter().enumerate() {
        {
            let mut s = state();
            if s.serial != serial {
                return Err("cancelled".to_string());
            }
            s.phase = Phase::Fetching { done, total };
        }
        let got = get(&format!("{BASE}/{id}.zip"), MAX_ZIP, serial).await;
        match got.and_then(|bytes| Remote::read(id, bytes)) {
            Ok(m) => mods.push(m),
            Err(e) => refused.push(format!("{id}: {e}")),
        }
    }
    Ok(Some(Listing {
        updated,
        mods,
        refused,
    }))
}

async fn get(url: &str, cap: u64, serial: u64) -> Result<Vec<u8>, String> {
    let mut over = false;
    let got = http::get_bytes(url, 0, &mut |read: u64, _: u64| {
        over = read > cap;
        !over && live(serial)
    })
    .await;
    match got {
        Ok(Fetched::Body(bytes)) => Ok(bytes),
        Ok(Fetched::Cancelled) if over => Err(format!("larger than {} KiB", cap >> 10)),
        Ok(Fetched::Cancelled) => Err("cancelled".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn parse_index(body: &[u8]) -> Result<(String, Vec<String>, Vec<String>), String> {
    let json: Json =
        serde_json::from_slice(body).map_err(|e| format!("index.json is not valid JSON: {e}"))?;
    let updated = json
        .get("last_updated")
        .and_then(Json::as_str)
        .ok_or("index.json: `last_updated` is required")?;
    let list = json
        .get("mods")
        .and_then(Json::as_array)
        .ok_or("index.json: `mods` is required")?;
    if list.len() > MAX_MODS {
        return Err(format!(
            "index.json lists {} mods; at most {MAX_MODS}",
            list.len()
        ));
    }
    let mut ids: Vec<String> = Vec::new();
    let mut refused = Vec::new();
    for entry in list {
        match entry.as_str() {
            Some(id) if !load::valid_id(id) => refused.push(format!("`{id}` is not a mod id")),
            Some(id) if ids.iter().any(|known| known.as_str() == id) => {}
            Some(id) => ids.push(id.to_string()),
            None => refused.push("index.json: an entry of `mods` is not a string".to_string()),
        }
    }
    Ok((updated.to_string(), ids, refused))
}

#[cfg(test)]
mod tests {
    use super::parse_index;

    #[test]
    fn index_keeps_valid_ids_once() {
        let (updated, ids, refused) = parse_index(
            br#"{"last_updated": "2026-11-06", "mods": ["worldmap", "litematica", "worldmap"]}"#,
        )
        .unwrap();
        assert_eq!(updated, "2026-11-06");
        assert_eq!(ids, ["worldmap", "litematica"]);
        assert!(refused.is_empty());
    }

    #[test]
    fn index_refuses_an_id_that_leaves_the_folder() {
        let (_, ids, refused) =
            parse_index(br#"{"last_updated": "x", "mods": ["../evil", "good"]}"#).unwrap();
        assert_eq!(ids, ["good"]);
        assert_eq!(refused.len(), 1);
    }

    #[test]
    fn index_needs_both_keys() {
        assert!(parse_index(b"").is_err());
        assert!(parse_index(br#"{"mods": []}"#).is_err());
        assert!(parse_index(br#"{"last_updated": "x"}"#).is_err());
    }
}
