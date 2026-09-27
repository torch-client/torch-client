pub(crate) fn show(text: &str, anchor: usize, cursor: usize) {
    #[cfg(target_arch = "wasm32")]
    web::show(text, anchor, cursor);
    #[cfg(target_os = "android")]
    android::want(true);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (text, anchor, cursor);
}

pub(crate) fn hide() {
    #[cfg(target_arch = "wasm32")]
    web::hide();
    #[cfg(target_os = "android")]
    android::want(false);
}

pub(crate) fn has_focus() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::has_focus();
    #[cfg(target_os = "android")]
    return android::wanted();
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    false
}

pub(crate) fn inset_px() -> f32 {
    #[cfg(target_arch = "wasm32")]
    let px = web::inset_px();
    #[cfg(target_os = "android")]
    let px = android::inset_px();
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    let px = 0.0;
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    log_inset(px);
    px
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn log_inset(px: f32) {
    use std::sync::atomic::{AtomicI32, Ordering};
    static LAST: AtomicI32 = AtomicI32::new(-1);
    let now = px as i32;
    if LAST.swap(now, Ordering::Relaxed) != now {
        crate::log_info!("input", "soft keyboard: {now}px of window covered");
    }
}

pub(crate) fn poll() -> Option<(String, usize, usize)> {
    #[cfg(target_arch = "wasm32")]
    return web::poll();
    #[cfg(not(target_arch = "wasm32"))]
    None
}

pub(crate) fn take_enter() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::take_enter();
    #[cfg(not(target_arch = "wasm32"))]
    false
}

pub(crate) fn take_tab() -> Option<bool> {
    #[cfg(target_arch = "wasm32")]
    return web::take_tab();
    #[cfg(not(target_arch = "wasm32"))]
    None
}

pub(crate) fn take_backspace() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::take_backspace();
    #[cfg(not(target_arch = "wasm32"))]
    false
}

#[derive(Default)]
pub(crate) struct Nav {
    pub up: bool,
    pub down: bool,
    pub page_up: bool,
    pub page_down: bool,
}

pub(crate) fn take_escape() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::take_escape();
    #[cfg(not(target_arch = "wasm32"))]
    false
}

pub(crate) fn take_nav() -> Nav {
    #[cfg(target_arch = "wasm32")]
    return web::take_nav();
    #[cfg(not(target_arch = "wasm32"))]
    Nav::default()
}

pub(crate) fn sync(text: &str, cursor: usize) {
    sync_selection(text, cursor, cursor);
}

pub(crate) fn sync_selection(text: &str, anchor: usize, cursor: usize) {
    #[cfg(target_arch = "wasm32")]
    web::sync_selection(text, anchor, cursor);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (text, anchor, cursor);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn install_devtools_passthrough() {
    web::install_devtools_passthrough();
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::sync::atomic::{AtomicBool, Ordering};

    use wasm_bindgen::{JsCast, prelude::Closure};
    use web_sys::{HtmlInputElement, KeyboardEvent};

    const ID: &str = "mc-text-input";

    static ENTER: AtomicBool = AtomicBool::new(false);
    static BACKSPACE: AtomicBool = AtomicBool::new(false);
    static TAB: AtomicBool = AtomicBool::new(false);
    static TAB_SHIFT: AtomicBool = AtomicBool::new(false);
    static ESCAPE: AtomicBool = AtomicBool::new(false);
    static UP: AtomicBool = AtomicBool::new(false);
    static DOWN: AtomicBool = AtomicBool::new(false);
    static PAGE_UP: AtomicBool = AtomicBool::new(false);
    static PAGE_DOWN: AtomicBool = AtomicBool::new(false);
    static F12: AtomicBool = AtomicBool::new(false);

    pub(super) fn install_devtools_passthrough() {
        let Some(window) = web_sys::window() else {
            return;
        };
        let on_keydown = Closure::<dyn FnMut(KeyboardEvent)>::new(|event: KeyboardEvent| {
            if event.key() == "F12" {
                F12.store(true, Ordering::Relaxed);
                event.stop_propagation();
            }
        });
        let _ = window.add_event_listener_with_callback_and_bool(
            "keydown",
            on_keydown.as_ref().unchecked_ref(),
            true,
        );
        on_keydown.forget();
    }

    fn field() -> Option<HtmlInputElement> {
        let doc = web_sys::window()?.document()?;
        if let Some(el) = doc.get_element_by_id(ID) {
            return el.dyn_into().ok();
        }
        let el: HtmlInputElement = doc.create_element("input").ok()?.dyn_into().ok()?;
        el.set_id(ID);
        let _ = el.set_attribute("autocomplete", "off");
        let _ = el.set_attribute("autocapitalize", "off");
        let _ = el.set_attribute("autocorrect", "off");
        let _ = el.set_attribute("spellcheck", "false");
        let style = el.style();
        let _ = style.set_property("position", "fixed");
        let _ = style.set_property("top", "-1000px");
        let _ = style.set_property("left", "0");
        let _ = style.set_property("opacity", "0");
        doc.body()?.append_child(&el).ok()?;

        let on_blur = Closure::<dyn FnMut()>::new(|| {
            if let Some(canvas) = canvas() {
                let _ = canvas.focus();
            }
        });
        let _ = el.add_event_listener_with_callback("blur", on_blur.as_ref().unchecked_ref());
        on_blur.forget();

        let on_keydown =
            Closure::<dyn FnMut(KeyboardEvent)>::new(|event: KeyboardEvent| {
                match event.key().as_str() {
                    "Enter" => ENTER.store(true, Ordering::Relaxed),
                    "Backspace" => {
                        BACKSPACE.store(true, Ordering::Relaxed);
                        event.prevent_default();
                    }
                    "Escape" => {
                        ESCAPE.store(true, Ordering::Relaxed);
                        event.prevent_default();
                    }
                    "ArrowUp" => {
                        UP.store(true, Ordering::Relaxed);
                        event.prevent_default();
                    }
                    "ArrowDown" => {
                        DOWN.store(true, Ordering::Relaxed);
                        event.prevent_default();
                    }
                    "PageUp" => {
                        PAGE_UP.store(true, Ordering::Relaxed);
                        event.prevent_default();
                    }
                    "PageDown" => {
                        PAGE_DOWN.store(true, Ordering::Relaxed);
                        event.prevent_default();
                    }
                    "Tab" => {
                        TAB_SHIFT.store(event.shift_key(), Ordering::Relaxed);
                        TAB.store(true, Ordering::Relaxed);
                        event.prevent_default();
                    }
                    _ => {}
                }
            });
        let _ = el.add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref());
        on_keydown.forget();

        Some(el)
    }

    fn canvas() -> Option<web_sys::HtmlElement> {
        web_sys::window()?
            .document()?
            .get_element_by_id("canvas")?
            .dyn_into()
            .ok()
    }

    pub(super) fn show(text: &str, anchor: usize, cursor: usize) {
        sync_selection(text, anchor, cursor);
        if let Some(el) = field() {
            let _ = el.focus();
            sync_selection(text, anchor, cursor);
        }
    }

    pub(super) fn hide() {
        if let Some(el) = field() {
            let _ = el.blur();
        }
    }

    pub(super) fn sync_selection(text: &str, anchor: usize, cursor: usize) {
        let Some(el) = field() else { return };
        if el.value() != text {
            el.set_value(text);
        }
        let units = |chars: usize| -> u32 {
            text.chars()
                .take(chars)
                .map(|c| c.len_utf16() as u32)
                .sum::<u32>()
        };
        let (a, c) = (units(anchor), units(cursor));
        let dir = if c < a { "backward" } else { "forward" };
        let _ = el.set_selection_range_with_direction(a.min(c), a.max(c), dir);
    }

    pub(super) fn take_enter() -> bool {
        let _ = field();
        ENTER.swap(false, Ordering::Relaxed)
    }

    pub(super) fn take_tab() -> Option<bool> {
        let _ = field();
        TAB.swap(false, Ordering::Relaxed)
            .then(|| TAB_SHIFT.load(Ordering::Relaxed))
    }

    pub(super) fn take_escape() -> bool {
        let _ = field();
        ESCAPE.swap(false, Ordering::Relaxed)
    }

    pub(super) fn take_nav() -> super::Nav {
        let _ = field();
        super::Nav {
            up: UP.swap(false, Ordering::Relaxed),
            down: DOWN.swap(false, Ordering::Relaxed),
            page_up: PAGE_UP.swap(false, Ordering::Relaxed),
            page_down: PAGE_DOWN.swap(false, Ordering::Relaxed),
        }
    }

    pub(super) fn take_backspace() -> bool {
        let _ = field();
        BACKSPACE.swap(false, Ordering::Relaxed)
    }

    pub(super) fn inset_px() -> f32 {
        let Some(win) = web_sys::window() else {
            return 0.0;
        };
        let Some(vv) = win.visual_viewport() else {
            return 0.0;
        };
        let Some(inner) = win.inner_height().ok().and_then(|v| v.as_f64()) else {
            return 0.0;
        };
        let covered = (inner - (vv.height() + vv.offset_top())).max(0.0);
        (covered * win.device_pixel_ratio()) as f32
    }

    pub(super) fn has_focus() -> bool {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return false;
        };
        let Some(el) = doc.get_element_by_id(ID) else {
            return false;
        };
        doc.active_element().is_some_and(|active| active == el)
    }

    pub(super) fn poll() -> Option<(String, usize, usize)> {
        let el = field()?;
        let value = el.value();
        let to_char = |units: u32| -> usize {
            let mut seen = 0u32;
            for (i, c) in value.chars().enumerate() {
                if seen >= units {
                    return i;
                }
                seen += c.len_utf16() as u32;
            }
            value.chars().count()
        };
        let start = el.selection_start().ok().flatten().map(to_char);
        let end = el.selection_end().ok().flatten().map(to_char);
        let backward = el
            .selection_direction()
            .ok()
            .flatten()
            .is_some_and(|d| d == "backward");
        let (cursor, anchor) = match (start, end) {
            (Some(s), Some(e)) if backward => (s, e),
            (Some(s), Some(e)) => (e, s),
            (s, e) => {
                let pos = e.or(s).unwrap_or(0);
                (pos, pos)
            }
        };
        Some((value, cursor, anchor))
    }
}

#[cfg(target_os = "android")]
pub(crate) mod android {
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};

    use bevy::android::android_activity::AndroidApp;
    use jni::{Env, JValue, JavaVM, errors::Result, jni_sig, jni_str, objects::JObject};

    static WANTED: AtomicBool = AtomicBool::new(false);

    pub(super) fn want(on: bool) {
        WANTED.store(on, Ordering::Relaxed);
        crate::log_info!(
            "input",
            "soft keyboard: {}",
            if on { "show" } else { "hide" }
        );
        let Some(app) = bevy::android::ANDROID_APP.get() else {
            crate::log_warn!("input", "soft keyboard: no AndroidApp yet");
            return;
        };
        if on {
            app.show_soft_input(false);
        } else {
            app.hide_soft_input(false);
        }
    }

    pub(super) fn wanted() -> bool {
        WANTED.load(Ordering::Relaxed)
    }

    pub(super) fn inset_px() -> f32 {
        const POLL_MS: u64 = 50;
        static CACHED: AtomicI32 = AtomicI32::new(0);
        static NEXT_READ_MS: AtomicU64 = AtomicU64::new(0);

        let now = crate::platform::time::epoch().elapsed().as_millis() as u64;
        if now >= NEXT_READ_MS.load(Ordering::Relaxed) {
            NEXT_READ_MS.store(now + POLL_MS, Ordering::Relaxed);
            CACHED.store(read_ime_inset().unwrap_or(0), Ordering::Relaxed);
        }
        CACHED.load(Ordering::Relaxed) as f32
    }

    fn read_ime_inset() -> Option<i32> {
        let app = bevy::android::ANDROID_APP.get()?;
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
        vm.attach_current_thread(|env| {
            let bottom = ime_bottom(env, app);
            if bottom.is_err() {
                env.exception_clear();
            }
            bottom
        })
        .ok()
    }

    fn ime_bottom(env: &mut Env<'_>, app: &AndroidApp) -> Result<i32> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
        let window = env
            .call_method(
                &activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            )?
            .l()?;
        let decor = env
            .call_method(
                &window,
                jni_str!("getDecorView"),
                jni_sig!("()Landroid/view/View;"),
                &[],
            )?
            .l()?;
        let insets = env
            .call_method(
                &decor,
                jni_str!("getRootWindowInsets"),
                jni_sig!("()Landroid/view/WindowInsets;"),
                &[],
            )?
            .l()?;
        let kind = env
            .call_static_method(
                jni_str!("android/view/WindowInsets$Type"),
                jni_str!("ime"),
                jni_sig!("()I"),
                &[],
            )?
            .i()?;
        let ime = env
            .call_method(
                &insets,
                jni_str!("getInsets"),
                jni_sig!("(I)Landroid/graphics/Insets;"),
                &[JValue::Int(kind)],
            )?
            .l()?;
        env.get_field(&ime, jni_str!("bottom"), jni_sig!("I"))?.i()
    }
}
