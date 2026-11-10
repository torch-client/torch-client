#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn toggle() {
    #[cfg(target_arch = "wasm32")]
    web::toggle();
}

#[cfg(target_arch = "wasm32")]
pub fn available() -> bool {
    web::available()
}

#[cfg(target_os = "android")]
pub fn android_init() {
    android::keep_screen_on();
    android::immersive();
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn capture_escape(take: bool) {
    #[cfg(target_arch = "wasm32")]
    web::capture_escape(take);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = take;
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn is_fullscreen() -> bool {
    #[cfg(target_arch = "wasm32")]
    return web::is_fullscreen();
    #[cfg(target_os = "android")]
    return true;
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    false
}

#[cfg(target_os = "android")]
mod android {
    use bevy::android::android_activity::WindowManagerFlags;
    use jni::{JValue, jni_sig, jni_str};

    pub(super) fn keep_screen_on() {
        if let Some(app) = bevy::android::ANDROID_APP.get() {
            app.set_window_flags(
                WindowManagerFlags::KEEP_SCREEN_ON,
                WindowManagerFlags::empty(),
            );
        }
    }

    pub(super) fn immersive() {
        let result = crate::platform::java::with_activity(|env, activity| {
            let class =
                crate::platform::java::app_class(env, activity, "com.torchclient.game.Immersive")?;
            env.call_static_method(
                &class,
                jni_str!("install"),
                jni_sig!("(Landroid/app/Activity;)V"),
                &[JValue::Object(activity)],
            )?;
            Ok(())
        });
        match result {
            None => crate::log_warn!("window", "immersive mode: no AndroidApp yet"),
            Some(Err(e)) => crate::log_warn!("window", "immersive mode failed: {e}"),
            Some(Ok(())) => {}
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::{JsCast, JsValue};

    fn document() -> Option<web_sys::Document> {
        web_sys::window()?.document()
    }

    pub(super) fn available() -> bool {
        static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *AVAILABLE.get_or_init(|| {
            document().is_some_and(|d| {
                js_sys::Reflect::get(&d, &JsValue::from_str("fullscreenEnabled"))
                    .is_ok_and(|enabled| enabled.is_truthy())
            })
        })
    }

    pub(super) fn toggle() {
        let Some(doc) = document() else { return };
        if doc.fullscreen_element().is_some() {
            let _ = doc.exit_fullscreen();
            return;
        }
        let Some(root) = doc.document_element() else {
            return;
        };
        let _ = root.request_fullscreen();
    }

    pub(super) fn capture_escape(take: bool) {
        let Some(nav) = web_sys::window().map(|w| w.navigator()) else {
            return;
        };
        let Ok(keyboard) = js_sys::Reflect::get(&nav, &JsValue::from_str("keyboard")) else {
            return;
        };
        if !keyboard.is_object() {
            return;
        }
        let name = if take { "lock" } else { "unlock" };
        let Ok(method) = js_sys::Reflect::get(&keyboard, &JsValue::from_str(name)) else {
            return;
        };
        let Ok(method) = method.dyn_into::<js_sys::Function>() else {
            return;
        };
        let result = if take {
            method.call1(&keyboard, &js_sys::Array::of1(&JsValue::from_str("Escape")))
        } else {
            method.call0(&keyboard)
        };
        let _ = result;
    }

    pub(super) fn is_fullscreen() -> bool {
        document().is_some_and(|d| d.fullscreen_element().is_some())
    }
}
