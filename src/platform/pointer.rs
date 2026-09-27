pub(crate) fn lock() {
    #[cfg(target_arch = "wasm32")]
    web::lock();
}

pub(crate) fn unlock() {
    #[cfg(target_arch = "wasm32")]
    web::unlock();
}

pub(crate) fn take_claimed() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::take_claimed();
    #[cfg(not(target_arch = "wasm32"))]
    false
}

pub(crate) fn is_locked() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::is_locked();
    #[cfg(not(target_arch = "wasm32"))]
    true
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::sync::atomic::{AtomicBool, Ordering};

    use js_sys::Reflect;
    use wasm_bindgen::{JsCast, JsValue, prelude::Closure};

    static WANT: AtomicBool = AtomicBool::new(false);
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static CLAIMED: AtomicBool = AtomicBool::new(false);

    thread_local! {
        static SWALLOW: js_sys::Function = js_sys::Function::new_no_args("");
    }

    fn document() -> Option<web_sys::Document> {
        web_sys::window()?.document()
    }

    fn canvas() -> Option<web_sys::Element> {
        document()?.get_element_by_id("canvas")
    }

    fn request(canvas: &web_sys::Element) {
        let Ok(method) = Reflect::get(canvas, &JsValue::from_str("requestPointerLock")) else {
            return;
        };
        let Ok(method) = method.dyn_into::<js_sys::Function>() else {
            return;
        };
        let Ok(promise) = method.call0(canvas) else {
            return;
        };
        let Ok(catch) = Reflect::get(&promise, &JsValue::from_str("catch")) else {
            return;
        };
        let Ok(catch) = catch.dyn_into::<js_sys::Function>() else {
            return;
        };
        SWALLOW.with(|f| {
            let _ = catch.call1(&promise, f);
        });
    }

    fn install() {
        if INSTALLED.swap(true, Ordering::Relaxed) {
            return;
        }
        let Some(el) = canvas() else {
            INSTALLED.store(false, Ordering::Relaxed);
            return;
        };
        for event in ["pointerdown", "pointerup"] {
            let claims = event == "pointerdown";
            let handler = Closure::<dyn FnMut()>::new(move || {
                if !WANT.load(Ordering::Relaxed) || is_locked() {
                    return;
                }
                if let Some(canvas) = canvas() {
                    request(&canvas);
                    if claims {
                        CLAIMED.store(true, Ordering::Relaxed);
                    }
                }
            });
            let _ = el.add_event_listener_with_callback(event, handler.as_ref().unchecked_ref());
            handler.forget();
        }
    }

    fn activation_live() -> bool {
        let Some(nav) = web_sys::window().map(|w| w.navigator()) else {
            return false;
        };
        let Ok(activation) = Reflect::get(&nav, &JsValue::from_str("userActivation")) else {
            return false;
        };
        if !activation.is_object() {
            return false;
        }
        let Ok(active) = Reflect::get(&activation, &JsValue::from_str("isActive")) else {
            return false;
        };
        active.as_bool().unwrap_or(false)
    }

    pub(super) fn lock() {
        install();
        WANT.store(true, Ordering::Relaxed);
        if activation_live()
            && let Some(canvas) = canvas()
        {
            request(&canvas);
        }
    }

    pub(super) fn unlock() {
        WANT.store(false, Ordering::Relaxed);
        CLAIMED.store(false, Ordering::Relaxed);
        if let Some(doc) = document() {
            doc.exit_pointer_lock();
        }
    }

    pub(super) fn take_claimed() -> bool {
        CLAIMED.swap(false, Ordering::Relaxed)
    }

    pub(super) fn is_locked() -> bool {
        document().is_some_and(|d| d.pointer_lock_element().is_some())
    }
}
