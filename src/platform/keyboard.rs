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
    {
        android::set_dismissed(false);
        android::want(false);
    }
}

pub(crate) fn arm() {
    #[cfg(all(target_arch = "wasm32", feature = "mobile_ui"))]
    touch::arm();
}

pub(crate) fn claim() {
    arm();
    #[cfg(all(target_arch = "wasm32", feature = "mobile_ui"))]
    touch::unprime();
}

pub(crate) fn disarm() {
    #[cfg(all(target_arch = "wasm32", feature = "mobile_ui"))]
    touch::disarm();
}

pub(crate) fn arm_zone(x: f32, y: f32, w: f32, h: f32) {
    #[cfg(all(target_arch = "wasm32", feature = "mobile_ui"))]
    touch::arm_zone(x, y, w, h);
    #[cfg(not(all(target_arch = "wasm32", feature = "mobile_ui")))]
    let _ = (x, y, w, h);
}

pub(crate) fn take_zone_tap() -> bool {
    #[cfg(all(target_arch = "wasm32", feature = "mobile_ui"))]
    return touch::take_zone_tap();
    #[cfg(not(all(target_arch = "wasm32", feature = "mobile_ui")))]
    false
}

pub(crate) fn settle() {
    #[cfg(all(target_arch = "wasm32", feature = "mobile_ui"))]
    touch::settle();
}

pub(crate) fn dismiss() {
    #[cfg(target_os = "android")]
    {
        if android::pinned() {
            return;
        }
        android::set_dismissed(true);
        android::want(false);
    }
}

#[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
pub(crate) fn pin(on: bool) {
    #[cfg(target_os = "android")]
    {
        android::set_pinned(on);
        if on {
            android::set_dismissed(false);
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = on;
}

#[cfg_attr(not(feature = "mobile_ui"), allow(dead_code))]
pub(crate) fn reassert() {
    #[cfg(target_os = "android")]
    if android::wanted() && !android::dismissed() {
        android::want(true);
    }
}

#[cfg_attr(
    not(target_os = "android"),
    allow(
        dead_code,
        reason = "read only by the Android arm of TextBox::sync_keyboard"
    )
)]
pub(crate) fn dismissed() -> bool {
    #[cfg(target_os = "android")]
    return android::dismissed();
    #[cfg(not(target_os = "android"))]
    false
}

pub(crate) fn note_tap(inside: bool) {
    #[cfg(target_os = "android")]
    android::note_tap(inside);
    #[cfg(not(target_os = "android"))]
    let _ = inside;
}

pub(crate) fn resolve_taps() {
    #[cfg(target_os = "android")]
    if android::take_outside_tap() {
        crate::log_info!(
            "input",
            "tap outside the text field: dismissing the keyboard"
        );
        dismiss();
    }
}

pub(crate) fn has_focus() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::has_focus();
    #[cfg(target_os = "android")]
    return android::wanted();
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    false
}

pub(crate) fn window_active(window_focused: bool) -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::page_focused().unwrap_or(window_focused);
    #[cfg(not(target_arch = "wasm32"))]
    window_focused
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

    pub(super) fn field() -> Option<HtmlInputElement> {
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
        let _ = style.set_property("font-size", "16px");
        doc.body()?.append_child(&el).ok()?;

        let on_blur = Closure::<dyn FnMut()>::new(|| {
            #[cfg(feature = "mobile_ui")]
            super::touch::release_zone();
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

    pub(super) fn canvas() -> Option<web_sys::HtmlElement> {
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

    pub(super) fn page_focused() -> Option<bool> {
        web_sys::window()?.document()?.has_focus().ok()
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

#[cfg(all(target_arch = "wasm32", feature = "mobile_ui"))]
mod touch {
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};

    use wasm_bindgen::{JsCast, prelude::Closure};
    use web_sys::{HtmlInputElement, TouchEvent};

    use super::web::{canvas, field, has_focus, hide};

    const ARM_WINDOW_MS: f64 = 250.0;
    const SETTLE_FRAMES: u8 = 3;

    static ARMED_AT: AtomicU64 = AtomicU64::new(0);
    static LISTENERS: AtomicBool = AtomicBool::new(false);
    static PRIMED: AtomicBool = AtomicBool::new(false);
    static PRIMED_AGE: AtomicU8 = AtomicU8::new(0);
    static ZONE_AT: AtomicU64 = AtomicU64::new(0);
    static ZONE: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
    static ZONE_TAP: AtomicBool = AtomicBool::new(false);
    static ZONE_HELD: AtomicBool = AtomicBool::new(false);

    pub(super) fn arm() {
        ARMED_AT.store(js_sys::Date::now().to_bits(), Ordering::Relaxed);
        install();
    }

    pub(super) fn arm_zone(x: f32, y: f32, w: f32, h: f32) {
        ZONE_AT.store(js_sys::Date::now().to_bits(), Ordering::Relaxed);
        for (slot, v) in ZONE.iter().zip([x, y, w, h]) {
            slot.store(v.to_bits(), Ordering::Relaxed);
        }
        install();
    }

    pub(super) fn disarm() {
        ARMED_AT.store(0, Ordering::Relaxed);
        ZONE_AT.store(0, Ordering::Relaxed);
    }

    pub(super) fn take_zone_tap() -> bool {
        ZONE_TAP.swap(false, Ordering::Relaxed)
    }

    pub(super) fn release_zone() {
        ZONE_HELD.store(false, Ordering::Relaxed);
    }

    pub(super) fn unprime() {
        PRIMED.store(false, Ordering::Relaxed);
    }

    fn refocus(el: &HtmlInputElement) {
        let start = el.selection_start().ok().flatten();
        let end = el.selection_end().ok().flatten();
        let _ = el.blur();
        let _ = el.focus();
        if let (Some(start), Some(end)) = (start, end) {
            let _ = el.set_selection_range(start, end);
        }
    }

    fn prime(cycle: bool) {
        let armed = f64::from_bits(ARMED_AT.load(Ordering::Relaxed));
        let held = has_focus();
        if js_sys::Date::now() - armed > ARM_WINDOW_MS || (held && !cycle) {
            return;
        }
        let Some(el) = field() else { return };
        if held {
            refocus(&el);
        } else {
            let _ = el.focus();
            PRIMED_AGE.store(0, Ordering::Relaxed);
            PRIMED.store(true, Ordering::Relaxed);
        }
    }

    fn zone_hit(event: &TouchEvent) -> Option<bool> {
        let at = f64::from_bits(ZONE_AT.load(Ordering::Relaxed));
        if js_sys::Date::now() - at > ARM_WINDOW_MS {
            return None;
        }
        let touch = event.changed_touches().get(0)?;
        let rect = canvas()?.get_bounding_client_rect();
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return None;
        }
        let fx = ((f64::from(touch.client_x()) - rect.left()) / rect.width()) as f32;
        let fy = ((f64::from(touch.client_y()) - rect.top()) / rect.height()) as f32;
        let [x, y, w, h] = [0, 1, 2, 3].map(|i| f32::from_bits(ZONE[i].load(Ordering::Relaxed)));
        let inside = fx >= x && fx <= x + w && fy >= y && fy <= y + h;
        crate::log_info!(
            "input",
            "keyboard zone {}: ({fx:.3}, {fy:.3}) against ({x:.3}, {y:.3}) {w:.3}x{h:.3}, inside: {inside}",
            event.type_(),
        );
        Some(inside)
    }

    fn claim_zone() {
        ZONE_TAP.store(true, Ordering::Relaxed);
        ZONE_HELD.store(true, Ordering::Relaxed);
    }

    fn zone_press(event: &TouchEvent) {
        if zone_hit(event) == Some(true)
            && let Some(el) = field()
        {
            let _ = el.focus();
            claim_zone();
        }
    }

    fn zone_touch(event: &TouchEvent) -> bool {
        let Some(inside) = zone_hit(event) else {
            return false;
        };
        if inside {
            if let Some(el) = field() {
                if has_focus() {
                    refocus(&el);
                } else {
                    let _ = el.focus();
                }
                claim_zone();
            }
            return true;
        }
        if ZONE_HELD.swap(false, Ordering::Relaxed) {
            ARMED_AT.store(0, Ordering::Relaxed);
            hide();
            return true;
        }
        false
    }

    fn install() {
        if LISTENERS.swap(true, Ordering::Relaxed) {
            return;
        }
        let (Some(canvas), Some(_)) = (canvas(), field()) else {
            LISTENERS.store(false, Ordering::Relaxed);
            return;
        };
        let on_press =
            Closure::<dyn FnMut(TouchEvent)>::new(|event: TouchEvent| zone_press(&event));
        let _ = canvas
            .add_event_listener_with_callback("touchstart", on_press.as_ref().unchecked_ref());
        on_press.forget();
        let on_touch = Closure::<dyn FnMut(TouchEvent)>::new(|event: TouchEvent| {
            if !zone_touch(&event) {
                prime(true);
            }
        });
        let _ =
            canvas.add_event_listener_with_callback("touchend", on_touch.as_ref().unchecked_ref());
        on_touch.forget();
        let on_click = Closure::<dyn FnMut()>::new(|| prime(false));
        let _ = canvas.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref());
        on_click.forget();
    }

    pub(super) fn settle() {
        if !PRIMED.load(Ordering::Relaxed) {
            return;
        }
        if PRIMED_AGE.fetch_add(1, Ordering::Relaxed) + 1 >= SETTLE_FRAMES {
            PRIMED.store(false, Ordering::Relaxed);
            hide();
        }
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

    static DISMISSED: AtomicBool = AtomicBool::new(false);
    static PINNED: AtomicBool = AtomicBool::new(false);

    pub(super) fn set_pinned(on: bool) {
        PINNED.store(on, Ordering::Relaxed);
    }

    pub(super) fn pinned() -> bool {
        PINNED.load(Ordering::Relaxed)
    }
    static TAP_OUTSIDE: AtomicBool = AtomicBool::new(false);
    static TAP_INSIDE: AtomicBool = AtomicBool::new(false);

    pub(super) fn set_dismissed(on: bool) {
        DISMISSED.store(on, Ordering::Relaxed);
    }

    pub(super) fn dismissed() -> bool {
        DISMISSED.load(Ordering::Relaxed)
    }

    pub(super) fn note_tap(inside: bool) {
        if inside {
            TAP_INSIDE.store(true, Ordering::Relaxed);
            DISMISSED.store(false, Ordering::Relaxed);
        } else {
            TAP_OUTSIDE.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn take_outside_tap() -> bool {
        let outside = TAP_OUTSIDE.swap(false, Ordering::Relaxed);
        let inside = TAP_INSIDE.swap(false, Ordering::Relaxed);
        outside && !inside
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
