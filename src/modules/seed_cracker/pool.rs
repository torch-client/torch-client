use std::cell::RefCell;
use std::rc::Rc;

use seedcrack::{Cracker, Slice, Task};
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use wasm_bindgen_futures::JsFuture;

use crate::client::mesh_worker::{error_text, page_url, to_buffer};
use crate::platform::describe_js as describe;
use crate::{log_info, log_warn};

const SLICE_BUDGET: u64 = 1 << 21;

const DEPTH: usize = 2;

const TAG_TASK: u8 = 0;
const TAG_SLICE: u8 = 1;

const SLICE_LEN: usize = 24;

fn put_slice(w: &mut Vec<u8>, slice: &Slice) {
    for v in [slice.job, slice.range.start, slice.range.end] {
        w.extend_from_slice(&v.to_le_bytes());
    }
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

fn encode_task(task: &Task) -> Vec<u8> {
    let mut w = vec![TAG_TASK];
    w.extend_from_slice(&task.to_bytes());
    w
}

fn encode_slice(slice: &Slice) -> Vec<u8> {
    let mut w = Vec::with_capacity(1 + SLICE_LEN);
    w.push(TAG_SLICE);
    put_slice(&mut w, slice);
    w
}

fn decode_done(b: &[u8]) -> Option<(Slice, Vec<i64>)> {
    let slice = Slice {
        job: u64_at(b, 0)?,
        range: u64_at(b, 8)?..u64_at(b, 16)?,
    };
    let rest = b.get(SLICE_LEN..)?;
    if rest.len() % 8 != 0 {
        return None;
    }
    let found = rest
        .chunks_exact(8)
        .map(|c| i64::from_le_bytes(<[u8; 8]>::try_from(c).unwrap_or([0; 8])))
        .collect();
    Some((slice, found))
}

enum Module {
    Loading,
    Ready(js_sys::WebAssembly::Module),
    Failed,
}

thread_local! {
    static MODULE: RefCell<Option<Module>> = const { RefCell::new(None) };
}

fn load_module() {
    let started = MODULE.with(|m| {
        let mut m = m.borrow_mut();
        if m.is_some() {
            return false;
        }
        *m = Some(Module::Loading);
        true
    });
    if !started {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        let url = page_url("torchSeedWasm", "./seedcrack_wasm.wasm");
        let state = match compile(&url).await {
            Ok(module) => Module::Ready(module),
            Err(e) => {
                log_warn!(
                    "seed_cracker",
                    "the search module {url} did not load ({e}); the search runs a slice per tick"
                );
                Module::Failed
            }
        };
        MODULE.with(|m| *m.borrow_mut() = Some(state));
    });
}

async fn compile(url: &str) -> Result<js_sys::WebAssembly::Module, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|e| describe(&e))?
        .dyn_into()
        .map_err(|_| "not a response".to_string())?;
    if !response.ok() {
        return Err(format!("http {}", response.status()));
    }
    let bytes = JsFuture::from(response.array_buffer().map_err(|e| describe(&e))?)
        .await
        .map_err(|e| describe(&e))?;
    JsFuture::from(js_sys::WebAssembly::compile(&bytes))
        .await
        .map_err(|e| describe(&e))?
        .dyn_into::<js_sys::WebAssembly::Module>()
        .map_err(|_| "not a module".to_string())
}

fn module() -> Result<js_sys::WebAssembly::Module, bool> {
    MODULE.with(|m| match &*m.borrow() {
        Some(Module::Ready(module)) => Ok(module.clone()),
        Some(Module::Loading) => Err(true),
        Some(Module::Failed) | None => Err(false),
    })
}

enum Reply {
    Done(u32, Slice, Vec<i64>),
    Failed(u32, String),
}

type Inbox = Rc<RefCell<Vec<Reply>>>;

struct Member {
    id: u32,
    worker: web_sys::Worker,
    task: Option<u64>,
    flight: Vec<Slice>,
    _message: Closure<dyn FnMut(web_sys::MessageEvent)>,
    _error: Closure<dyn FnMut(web_sys::Event)>,
}

impl Drop for Member {
    fn drop(&mut self) {
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.worker.terminate();
    }
}

pub struct Pool {
    members: Vec<Member>,
    inbox: Inbox,
    started: bool,
}

impl Pool {
    pub fn new() -> Pool {
        load_module();
        Pool {
            members: Vec::new(),
            inbox: Inbox::default(),
            started: false,
        }
    }

    pub fn pump(&mut self, cracker: &mut Cracker) -> Option<bool> {
        if !self.started {
            match module() {
                Ok(module) => {
                    self.started = true;
                    self.start(&module);
                }
                Err(true) => return None,
                Err(false) => self.started = true,
            }
        }
        if self.members.is_empty() {
            return None;
        }

        let replies = std::mem::take(&mut *self.inbox.borrow_mut());
        for reply in replies {
            match reply {
                Reply::Done(id, slice, found) => {
                    let ours = self.members.iter_mut().find(|m| m.id == id).and_then(|m| {
                        m.flight
                            .iter()
                            .position(|s| *s == slice)
                            .map(|i| m.flight.remove(i))
                    });
                    if let Some(slice) = ours {
                        cracker.complete(slice, &found);
                    }
                }
                Reply::Failed(id, why) => self.drop_member(id, &why, cracker),
            }
        }

        let mut dead = Vec::new();
        'fill: for m in &mut self.members {
            while m.flight.len() < DEPTH {
                let Some(slice) = cracker.next_slice(SLICE_BUDGET) else {
                    break 'fill;
                };
                let task = if m.task == Some(slice.job) {
                    None
                } else {
                    match cracker.task().filter(|t| t.id() == slice.job) {
                        Some(task) => Some(task),
                        None => {
                            cracker.requeue(slice);
                            break 'fill;
                        }
                    }
                };
                let posted = task
                    .as_ref()
                    .is_none_or(|t| post(&m.worker, &encode_task(t)))
                    && post(&m.worker, &encode_slice(&slice));
                if task.is_some() {
                    m.task = Some(slice.job);
                }
                m.flight.push(slice);
                if !posted {
                    dead.push(m.id);
                    break;
                }
            }
        }
        for id in dead {
            self.drop_member(id, "it stopped accepting messages", cracker);
        }

        Some(self.members.is_empty() || self.members.iter().any(|m| !m.flight.is_empty()))
    }

    fn start(&mut self, module: &js_sys::WebAssembly::Module) {
        let want = size();
        let url = page_url("torchSeedWorker", "./seed_worker.js");
        self.members = (0..want as u32)
            .filter_map(|id| spawn(id, &url, module, &self.inbox))
            .collect();
        if self.members.is_empty() {
            log_warn!(
                "seed_cracker",
                "no seed worker started; the search runs a slice per tick"
            );
        } else {
            log_info!(
                "seed_cracker",
                "the search runs in {} of {want} web workers",
                self.members.len()
            );
        }
    }

    fn drop_member(&mut self, id: u32, why: &str, cracker: &mut Cracker) {
        let Some(i) = self.members.iter().position(|m| m.id == id) else {
            return;
        };
        let mut m = self.members.remove(i);
        for slice in std::mem::take(&mut m.flight) {
            cracker.requeue(slice);
        }
        drop(m);
        log_warn!(
            "seed_cracker",
            "a seed worker failed ({why}); {} left",
            self.members.len()
        );
        if self.members.is_empty() {
            log_warn!(
                "seed_cracker",
                "no seed worker left; the search runs a slice per tick"
            );
        }
    }
}

fn size() -> usize {
    let cores = web_sys::window().map_or(0.0, |w| w.navigator().hardware_concurrency());
    (cores as usize).saturating_sub(2).max(1)
}

fn post(worker: &web_sys::Worker, bytes: &[u8]) -> bool {
    let buffer = to_buffer(bytes);
    worker
        .post_message_with_transfer(&buffer, &js_sys::Array::of1(&buffer))
        .is_ok()
}

fn spawn(
    id: u32,
    url: &str,
    module: &js_sys::WebAssembly::Module,
    inbox: &Inbox,
) -> Option<Member> {
    let worker = match web_sys::Worker::new(url) {
        Ok(worker) => worker,
        Err(e) => {
            log_warn!("seed_cracker", "no seed worker ({})", describe(&e));
            return None;
        }
    };
    let to = inbox.clone();
    let message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            to.borrow_mut().push(read_reply(id, &e.data()));
        });
    let to = inbox.clone();
    let error = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
        to.borrow_mut().push(Reply::Failed(id, error_text(&e)));
    });
    worker.set_onmessage(Some(message.as_ref().unchecked_ref()));
    worker.set_onerror(Some(error.as_ref().unchecked_ref()));
    let member = Member {
        id,
        worker,
        task: None,
        flight: Vec::new(),
        _message: message,
        _error: error,
    };

    let init = js_sys::Object::new();
    let ok = js_sys::Reflect::set(&init, &"module".into(), module).is_ok()
        && member.worker.post_message(&init).is_ok();
    if !ok {
        log_warn!(
            "seed_cracker",
            "a seed worker could not be handed the module"
        );
        return None;
    }
    Some(member)
}

fn read_reply(id: u32, data: &JsValue) -> Reply {
    if let Some(text) = data.as_string() {
        return Reply::Failed(id, text);
    }
    let Some(buffer) = data.dyn_ref::<js_sys::ArrayBuffer>() else {
        return Reply::Failed(id, "a reply that is not a buffer".to_string());
    };
    let bytes = js_sys::Uint8Array::new(buffer).to_vec();
    match decode_done(&bytes) {
        Some((slice, found)) => Reply::Done(id, slice, found),
        None => Reply::Failed(id, "a reply this build cannot read".to_string()),
    }
}
