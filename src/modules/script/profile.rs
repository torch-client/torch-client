use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Condvar, OnceLock};
use std::time::Duration;

use mlua::{Lua, Table};

use super::api::Ctx;
use super::game::{self, parse_uuid};
use super::perm;

const SPACING: Duration = Duration::from_millis(500);

const BACKOFF: Duration = Duration::from_secs(30);

const QUEUE_CAP: usize = 64;

#[derive(Clone, Debug)]
pub(super) enum Answer {
    Found(String),
    Missing,
    Offline,
    Failed(String),
}

impl Answer {
    pub fn fill(&self, e: &Table) -> mlua::Result<()> {
        e.raw_set("ok", !matches!(self, Answer::Failed(_)))?;
        e.raw_set("found", matches!(self, Answer::Found(_)))?;
        match self {
            Answer::Found(name) => e.raw_set("name", name.as_str()),
            Answer::Missing => Ok(()),
            Answer::Offline => e.raw_set("offline", true),
            Answer::Failed(why) => e.raw_set("error", why.as_str()),
        }
    }
}

struct Request {
    uuid: u128,
    owner: usize,
    generation: u64,
}

static QUEUE: Mutex<VecDeque<Request>> = Mutex::new(VecDeque::new());
#[cfg(not(target_arch = "wasm32"))]
static WAKE: Condvar = Condvar::new();

static GENERATION: AtomicU64 = AtomicU64::new(0);

static CACHE: Mutex<Option<HashMap<u128, Answer>>> = Mutex::new(None);

pub(super) fn forget_requests() {
    QUEUE.lock().unwrap_or_else(PoisonError::into_inner).clear();
    GENERATION.fetch_add(1, Relaxed);
}

fn cached(uuid: u128) -> Option<Answer> {
    if uuid::Uuid::from_u128(uuid).get_version_num() == 3 {
        return Some(Answer::Offline);
    }
    CACHE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .and_then(|c| c.get(&uuid).cloned())
}

fn ask(uuid: u128, owner: usize) -> bool {
    let mut q = QUEUE.lock().unwrap_or_else(PoisonError::into_inner);
    if q.iter().any(|r| r.uuid == uuid && r.owner == owner) {
        return true;
    }
    if q.len() >= QUEUE_CAP {
        return false;
    }
    q.push_back(Request {
        uuid,
        owner,
        generation: GENERATION.load(Relaxed),
    });
    drop(q);
    start();
    true
}

#[cfg(not(target_arch = "wasm32"))]
fn start() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        if let Err(e) = std::thread::Builder::new()
            .name("lua-lookup".into())
            .spawn(worker)
        {
            crate::log_error!("mods", "could not start the profile lookup thread: {e}");
        }
    });
    WAKE.notify_one();
}

#[cfg(target_arch = "wasm32")]
fn start() {
    static DRAINING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DRAINING.swap(true, Relaxed) {
        return;
    }
    crate::platform::executor::spawn(async {
        loop {
            let Some(req) = QUEUE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop_front()
            else {
                DRAINING.store(false, Relaxed);
                return;
            };
            let answer = match cached(req.uuid) {
                Some(a) => a,
                None => {
                    let (answer, wait) =
                        answer_of(crate::platform::http::get_status(&url(req.uuid)).await);
                    remember(req.uuid, &answer);
                    crate::platform::time::sleep(wait).await;
                    answer
                }
            };
            deliver(req, answer);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn worker() {
    loop {
        let req = {
            let mut q = QUEUE.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if let Some(r) = q.pop_front() {
                    break r;
                }
                q = WAKE.wait(q).unwrap_or_else(PoisonError::into_inner);
            }
        };
        let answer = match cached(req.uuid) {
            Some(a) => a,
            None => {
                let (answer, wait) =
                    answer_of(crate::platform::http::get_status_blocking(&url(req.uuid)));
                remember(req.uuid, &answer);
                std::thread::sleep(wait);
                answer
            }
        };
        deliver(req, answer);
    }
}

fn remember(uuid: u128, answer: &Answer) {
    if !matches!(answer, Answer::Failed(_)) {
        CACHE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(HashMap::new)
            .insert(uuid, answer.clone());
    }
}

fn deliver(req: Request, answer: Answer) {
    if req.generation == GENERATION.load(Relaxed) {
        game::push(game::Pending::Profile {
            owner: req.owner,
            uuid: req.uuid,
            answer,
        });
    }
}

fn url(uuid: u128) -> String {
    format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{}",
        uuid::Uuid::from_u128(uuid).simple()
    )
}

fn answer_of(response: Result<(u16, Vec<u8>), String>) -> (Answer, Duration) {
    match response {
        Ok((200, body)) => {
            let name = serde_json::from_slice::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string));
            match name {
                Some(name) => (Answer::Found(name), SPACING),
                None => (
                    Answer::Failed("Mojang's answer had no name".into()),
                    SPACING,
                ),
            }
        }
        Ok((204 | 404, _)) => (Answer::Missing, SPACING),
        Ok((429, _)) => (
            Answer::Failed("rate limited by Mojang; try again later".into()),
            BACKOFF,
        ),
        Ok((status, _)) => (
            Answer::Failed(format!("Mojang answered HTTP {status}")),
            SPACING,
        ),
        Err(e) => (Answer::Failed(e), SPACING),
    }
}

pub(super) fn table(lua: &Lua, ctx: &Arc<Ctx>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let c = ctx.clone();
    t.raw_set(
        "lookup",
        lua.create_function(move |_, id: String| {
            let what = "torch.profile.lookup";
            c.at_load(what)?;
            let (k, mod_id) = c.caller(what)?;
            if !perm::declared(&c, perm::LOOKUP, mod_id) {
                return Err(perm::undeclared(what, perm::LOOKUP));
            }
            let uuid = parse_uuid(&id, what)?;
            Ok(perm::granted(perm::LOOKUP, mod_id) && ask(uuid, k))
        })?,
    )?;
    t.raw_set(
        "cached",
        lua.create_function(|lua, id: String| {
            let uuid = parse_uuid(&id, "torch.profile.cached")?;
            Ok(match cached(uuid) {
                Some(Answer::Found(name)) => mlua::Value::String(lua.create_string(&name)?),
                Some(Answer::Missing | Answer::Offline) => mlua::Value::Boolean(false),
                _ => mlua::Value::Nil,
            })
        })?,
    )?;
    let c = ctx.clone();
    t.raw_set(
        "allowed",
        lua.create_function(move |_, ()| {
            let (_, mod_id) = c.caller("torch.profile.allowed")?;
            Ok(perm::declared(&c, perm::LOOKUP, mod_id) && perm::granted(perm::LOOKUP, mod_id))
        })?,
    )?;
    t.set_readonly(true);
    Ok(t)
}
