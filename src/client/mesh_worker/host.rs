use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use wasm_bindgen::{JsCast, JsValue, prelude::Closure};

use super::msg::{FromWorker, ToWorker};
use crate::{log_error, log_info, log_warn};

static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

static DIED: AtomicBool = AtomicBool::new(false);

fn deactivate() {
    if ACTIVE.swap(false, Ordering::Relaxed) {
        DIED.store(true, Ordering::Relaxed);
    }
}

pub fn take_died() -> bool {
    DIED.swap(false, Ordering::Relaxed)
}

thread_local! {
    static WORKER: RefCell<Option<Handle>> = const { RefCell::new(None) };
}

struct Handle {
    worker: web_sys::Worker,
    _message: Closure<dyn FnMut(web_sys::MessageEvent)>,
    _error: Closure<dyn FnMut(web_sys::ErrorEvent)>,
}

pub fn spawn(assets: &[u8]) -> bool {
    if active() {
        return true;
    }
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let worker = match web_sys::Worker::new_with_options("./worker.js", &options) {
        Ok(worker) => worker,
        Err(e) => {
            log_warn!(
                "mesh",
                "no mesh worker ({}); meshing and lighting stay in the frame",
                describe(&e)
            );
            return false;
        }
    };

    let message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
            on_message(e)
        });
    let error = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(|e: web_sys::ErrorEvent| {
        deactivate();
        log_error!(
            "mesh",
            "the mesh worker failed ({}); meshing falls back to the frame",
            e.message()
        );
    });
    worker.set_onmessage(Some(message.as_ref().unchecked_ref()));
    worker.set_onerror(Some(error.as_ref().unchecked_ref()));

    let init = js_sys::Object::new();
    let assets = to_buffer(assets);
    let ok = js_sys::Reflect::set(&init, &"module".into(), &wasm_bindgen::module()).is_ok()
        && js_sys::Reflect::set(&init, &"assets".into(), &assets).is_ok()
        && worker
            .post_message_with_transfer(&init, &js_sys::Array::of1(&assets))
            .is_ok();
    if !ok {
        log_warn!("mesh", "the mesh worker could not be handed the module");
        return false;
    }

    WORKER.with(|slot| {
        *slot.borrow_mut() = Some(Handle {
            worker,
            _message: message,
            _error: error,
        });
    });
    ACTIVE.store(true, Ordering::Relaxed);
    log_info!("mesh", "meshing and lighting moved to a worker");
    true
}

pub fn send(msg: &ToWorker) {
    WORKER.with(|slot| {
        let slot = slot.borrow();
        let Some(handle) = slot.as_ref() else { return };
        let buffer = to_buffer(&msg.encode());
        if handle
            .worker
            .post_message_with_transfer(&buffer, &js_sys::Array::of1(&buffer))
            .is_err()
        {
            deactivate();
            log_error!("mesh", "the mesh worker stopped accepting messages");
        }
    });
}

fn to_buffer(bytes: &[u8]) -> js_sys::ArrayBuffer {
    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(bytes);
    array.buffer()
}

fn on_message(event: web_sys::MessageEvent) {
    let data = event.data();
    if let Some(text) = data.as_string() {
        deactivate();
        log_error!("mesh", "the mesh worker could not start: {text}");
        return;
    }
    let Some(buffer) = data.dyn_ref::<js_sys::ArrayBuffer>() else {
        return;
    };
    let bytes = js_sys::Uint8Array::new(buffer).to_vec();
    let Some(msg) = FromWorker::decode(&bytes) else {
        log_warn!(
            "mesh",
            "the mesh worker sent a message this build cannot read"
        );
        return;
    };
    apply(msg);
}

fn apply(msg: FromWorker) {
    match msg {
        FromWorker::Ready => log_info!("mesh", "the mesh worker is ready"),
        FromWorker::Failed(message) => {
            deactivate();
            log_error!("mesh", "the mesh worker gave up: {message}");
        }
        FromWorker::Sections { chunks, edits } => {
            let Some(shared) = crate::SHARED.get() else {
                return;
            };
            crate::diag::add(
                crate::diag::Stat::SectionsMeshed,
                (chunks.len() + edits.len()) as u64,
            );
            let mut s = shared.lock().unwrap();
            let session = &mut s.session;
            supersede(
                &mut session.pending_chunks,
                &mut session.pending_edits,
                &chunks,
                &edits,
            );
            session.pending_chunks.extend(chunks);
            session.pending_edits.extend(edits);
            super::resend_backlog();
        }
        FromWorker::Light(delta) => {
            let map = crate::client::worldsync::light_map();
            let mut map = map.write();
            map.set_lowest_section(delta.lowest);
            for (sec, layers) in delta.sections {
                map.put_section(sec, layers);
            }
            for (col, lit, top) in delta.columns {
                map.set_column(col, lit, top);
            }
            for col in delta.removed {
                map.drop_column(col);
            }
        }
    }
}

fn describe(value: &JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(value, &"message".into())
                .ok()?
                .as_string()
        })
        .unwrap_or_else(|| format!("{value:?}"))
}

fn supersede(
    pending_chunks: &mut Vec<crate::renderer::PendingSection>,
    pending_edits: &mut Vec<crate::renderer::PendingSection>,
    chunks: &[crate::renderer::PendingSection],
    edits: &[crate::renderer::PendingSection],
) {
    if pending_chunks.is_empty() && pending_edits.is_empty() {
        return;
    }
    let fresh: std::collections::HashSet<(i32, i32, i32)> = chunks
        .iter()
        .chain(edits)
        .map(|p| (p.chunk_x, p.chunk_z, p.sec_y))
        .collect();
    let keep =
        |p: &crate::renderer::PendingSection| !fresh.contains(&(p.chunk_x, p.chunk_z, p.sec_y));
    pending_chunks.retain(keep);
    pending_edits.retain(keep);
}
