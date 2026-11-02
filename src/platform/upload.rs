use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use js_sys::{Array, Function, Promise, Uint8Array};
use parking_lot::Mutex;
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, DragEvent, File, FileSystemDirectoryEntry, FileSystemEntry, FileSystemFileEntry,
    HtmlInputElement,
};

use super::packfs;

const STAGING: &str = "dropped";
const INPUT_ID: &str = "mc-pack-input";

#[derive(Clone, Copy)]
pub(crate) enum Taker {
    #[cfg_attr(not(any(feature = "shader_support", resource_packs)), allow(dead_code))]
    Packs = 1,
    #[cfg_attr(not(lua_mods), allow(dead_code))]
    Mods = 2,
    #[cfg_attr(not(lua_mods), allow(dead_code))]
    ModFile = 4,
}

static ACCEPTING: AtomicU8 = AtomicU8::new(0);
static INSTALL: Once = Once::new();
static NEXT: AtomicU64 = AtomicU64::new(0);
static ARRIVED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub(crate) fn accept(taker: Taker, on: bool) {
    INSTALL.call_once(install);
    if on {
        ACCEPTING.fetch_or(taker as u8, Ordering::Relaxed);
    } else {
        ACCEPTING.fetch_and(!(taker as u8), Ordering::Relaxed);
    }
}

pub(crate) fn take(taker: Taker) -> Vec<PathBuf> {
    let accepting = ACCEPTING.load(Ordering::Relaxed);
    if accepting != 0 && accepting & taker as u8 == 0 {
        return Vec::new();
    }
    let arrived = std::mem::take(&mut *ARRIVED.lock());
    if accepting != 0 {
        return arrived;
    }
    if !arrived.is_empty() {
        crate::log_info!(
            "packs",
            "{} drop(s) finished reading after the pack screen closed; dropped",
            arrived.len()
        );
    }
    discard(&arrived);
    Vec::new()
}

pub(crate) fn discard(paths: &[PathBuf]) {
    for path in paths {
        if let Some(staged) = path.parent().filter(|dir| dir.starts_with(STAGING)) {
            let _ = packfs::remove_dir_all(staged);
        }
    }
}

pub(crate) fn choose() {
    match input() {
        Some(input) => input.click(),
        None => crate::log_warn!("packs", "could not make the file input"),
    }
}

#[cfg(lua_mods)]
pub(crate) fn choose_mods() {
    let input = hidden_input("mc-mod-input", ".lua,.zip,application/zip", true, |files| {
        wasm_bindgen_futures::spawn_local(async move {
            for file in files {
                stage_file(file).await;
            }
        });
    });
    match input {
        Some(input) => input.click(),
        None => crate::log_warn!("mods", "could not make the file input"),
    }
}

fn install() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let on_dragover = Closure::<dyn FnMut(DragEvent)>::new(|event: DragEvent| {
        event.prevent_default();
        if let Some(transfer) = event.data_transfer() {
            transfer.set_drop_effect(if ACCEPTING.load(Ordering::Relaxed) != 0 {
                "copy"
            } else {
                "none"
            });
        }
    });
    let on_drop = Closure::<dyn FnMut(DragEvent)>::new(|event: DragEvent| {
        event.prevent_default();
        if ACCEPTING.load(Ordering::Relaxed) != 0 {
            dropped(&event);
        } else {
            crate::log_info!(
                "packs",
                "a drop was refused: no pack screen or Mods tab is open"
            );
        }
    });
    let _ =
        window.add_event_listener_with_callback("dragover", on_dragover.as_ref().unchecked_ref());
    let _ = window.add_event_listener_with_callback("drop", on_drop.as_ref().unchecked_ref());
    on_dragover.forget();
    on_drop.forget();
}

fn dropped(event: &DragEvent) {
    let Some(transfer) = event.data_transfer() else {
        return;
    };
    let items = transfer.items();
    let mut entries = Vec::new();
    let mut files = Vec::new();
    for i in 0..items.length() {
        let Some(item) = items.get(i) else { continue };
        if item.kind() != "file" {
            continue;
        }
        match item.webkit_get_as_entry() {
            Ok(Some(entry)) => entries.push(entry),
            _ => files.extend(item.get_as_file().ok().flatten()),
        }
    }
    crate::log_info!(
        "packs",
        "dropped: {} item(s), reading",
        entries.len() + files.len()
    );
    wasm_bindgen_futures::spawn_local(async move {
        for entry in entries {
            stage_entry(entry).await;
        }
        for file in files {
            stage_file(file).await;
        }
    });
}

fn staging() -> PathBuf {
    Path::new(STAGING).join(NEXT.fetch_add(1, Ordering::Relaxed).to_string())
}

async fn stage_entry(entry: FileSystemEntry) {
    let top = staging().join(entry.name());
    let mut stack = vec![(entry, top.clone())];
    while let Some((entry, at)) = stack.pop() {
        if entry.is_directory() {
            match children(entry.unchecked_ref()).await {
                Ok(children) => stack.extend(children.into_iter().map(|child| {
                    let path = at.join(child.name());
                    (child, path)
                })),
                Err(e) => crate::log_warn!("packs", "could not list {}: {e:?}", at.display()),
            }
            continue;
        }
        let read = async {
            let file = file_of(entry.unchecked_ref()).await?;
            let bytes = bytes_of(&file).await?;
            packfs::write(&at, bytes).map_err(|e| JsValue::from_str(&e.to_string()))
        };
        if let Err(e) = read.await {
            crate::log_warn!("packs", "could not read {}: {e:?}", at.display());
        }
    }
    crate::log_info!("packs", "read {}", top.display());
    ARRIVED.lock().push(top);
}

async fn stage_file(file: File) {
    let at = staging().join(file.name());
    match bytes_of(&file).await {
        Ok(bytes) => {
            if let Err(e) = packfs::write(&at, bytes) {
                crate::log_warn!("packs", "could not keep {}: {e}", at.display());
            }
        }
        Err(e) => crate::log_warn!("packs", "could not read {}: {e:?}", file.name()),
    }
    crate::log_info!("packs", "read {}", at.display());
    ARRIVED.lock().push(at);
}

fn called_back(start: impl FnOnce(&Function, &Function)) -> JsFuture {
    let mut start = Some(start);
    let promise = Promise::new(&mut |resolve, reject| {
        if let Some(start) = start.take() {
            start(&resolve, &reject);
        }
    });
    JsFuture::from(promise)
}

async fn file_of(entry: &FileSystemFileEntry) -> Result<File, JsValue> {
    called_back(|resolve, reject| entry.file_with_callback_and_callback(resolve, reject))
        .await?
        .dyn_into()
}

async fn children(dir: &FileSystemDirectoryEntry) -> Result<Vec<FileSystemEntry>, JsValue> {
    let reader = dir.create_reader();
    let mut out = Vec::new();
    loop {
        let batch = called_back(|resolve, reject| {
            if let Err(e) = reader.read_entries_with_callback_and_callback(resolve, reject) {
                let _ = reject.call1(&JsValue::NULL, &e);
            }
        })
        .await?;
        let batch: Array = batch.unchecked_into();
        if batch.length() == 0 {
            return Ok(out);
        }
        out.extend(
            batch
                .iter()
                .map(|entry| entry.unchecked_into::<FileSystemEntry>()),
        );
    }
}

async fn bytes_of(blob: &Blob) -> Result<Vec<u8>, JsValue> {
    let buffer = JsFuture::from(blob.array_buffer()).await?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

fn input() -> Option<HtmlInputElement> {
    hidden_input(INPUT_ID, ".zip,application/zip", true, |files| {
        wasm_bindgen_futures::spawn_local(async move {
            for file in files {
                stage_file(file).await;
            }
        });
    })
}

#[cfg(any(eagler_skins, lua_mods))]
type Chosen = Mutex<Option<(String, Vec<u8>)>>;

#[cfg(any(eagler_skins, lua_mods))]
fn read_one(files: Vec<File>, slot: &'static Chosen, tag: &'static str) {
    let Some(file) = files.into_iter().next() else {
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        match bytes_of(&file).await {
            Ok(bytes) => *slot.lock() = Some((file.name(), bytes)),
            Err(e) => crate::log_warn!(tag, "could not read {}: {e:?}", file.name()),
        }
    });
}

#[cfg(eagler_skins)]
static PNG: Chosen = Mutex::new(None);

#[cfg(eagler_skins)]
pub(crate) fn choose_png() {
    let input = hidden_input("mc-skin-input", ".png,image/png", false, |files| {
        read_one(files, &PNG, "skins")
    });
    if let Some(input) = input {
        input.click();
    }
}

#[cfg(eagler_skins)]
pub(crate) fn take_png() -> Option<(String, Vec<u8>)> {
    PNG.lock().take()
}

#[cfg(lua_mods)]
static MOD_FILE: Chosen = Mutex::new(None);

#[cfg(lua_mods)]
pub(crate) fn choose_mod_file(accept: &[String]) {
    match hidden_input("mc-modfile-input", "", false, |files| {
        read_one(files, &MOD_FILE, "mods")
    }) {
        Some(input) => {
            input.set_accept(&accept.join(","));
            input.click();
        }
        None => crate::log_warn!("mods", "could not make the file input"),
    }
}

#[cfg(lua_mods)]
pub(crate) fn take_mod_file() -> Option<(String, Vec<u8>)> {
    MOD_FILE.lock().take()
}

fn hidden_input(
    id: &'static str,
    accept: &str,
    multiple: bool,
    chosen: fn(Vec<File>),
) -> Option<HtmlInputElement> {
    let doc = web_sys::window()?.document()?;
    if let Some(el) = doc.get_element_by_id(id) {
        return el.dyn_into().ok();
    }
    let el: HtmlInputElement = doc.create_element("input").ok()?.dyn_into().ok()?;
    el.set_id(id);
    el.set_type("file");
    el.set_multiple(multiple);
    el.set_accept(accept);
    let _ = el.style().set_property("display", "none");
    doc.body()?.append_child(&el).ok()?;

    let on_change = Closure::<dyn FnMut()>::new(move || {
        let Some(input) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
        else {
            return;
        };
        let files: Vec<File> = input
            .files()
            .map(|list| (0..list.length()).filter_map(|i| list.get(i)).collect())
            .unwrap_or_default();
        input.set_value("");
        chosen(files);
    });
    el.set_onchange(Some(on_change.as_ref().unchecked_ref()));
    on_change.forget();
    Some(el)
}
