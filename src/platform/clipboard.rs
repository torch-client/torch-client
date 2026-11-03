pub fn get() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web::take_pasted()
    }
    #[cfg(target_os = "android")]
    {
        android::get().unwrap_or_default()
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    {
        native::with(|clip| clip.get_text().unwrap_or_default()).unwrap_or_default()
    }
}

pub fn offer(text: Option<String>) {
    #[cfg(target_arch = "wasm32")]
    web::offer(text);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = text;
}

pub fn set(text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        web::copy(text);
    }
    #[cfg(target_os = "android")]
    {
        android::set(text);
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    {
        native::with(|clip| {
            let _ = clip.set_text(text);
        });
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod native {
    use std::sync::{Mutex, OnceLock};

    static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();

    pub(super) fn with<T>(f: impl FnOnce(&mut arboard::Clipboard) -> T) -> Option<T> {
        let cell = CLIPBOARD.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok()));
        let mut guard = match cell.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.as_mut().map(f)
    }
}

#[cfg(target_os = "android")]
mod android {
    use jni::{
        Env, JValue, JavaVM,
        errors::Result,
        jni_sig, jni_str,
        objects::{JObject, JString},
    };

    fn with<T>(f: impl FnOnce(&mut Env<'_>, &JObject<'_>) -> Result<T>) -> Option<T> {
        let app = bevy::android::ANDROID_APP.get()?;
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
        vm.attach_current_thread(|env| {
            let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
            let result = f(env, &activity);
            if result.is_err() {
                env.exception_clear();
            }
            result
        })
        .ok()
    }

    fn manager<'local>(env: &mut Env<'local>, activity: &JObject<'_>) -> Result<JObject<'local>> {
        let name: JObject = env.new_string("clipboard")?.into();
        env.call_method(
            activity,
            jni_str!("getSystemService"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
            &[JValue::Object(&name)],
        )?
        .l()
    }

    pub(super) fn set(text: &str) {
        with(|env, activity| {
            let manager = manager(env, activity)?;
            let label: JObject = env.new_string("Torch Client")?.into();
            let text: JObject = env.new_string(text)?.into();
            let clip = env
                .call_static_method(
                    jni_str!("android/content/ClipData"),
                    jni_str!("newPlainText"),
                    jni_sig!(
                        "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;"
                    ),
                    &[JValue::Object(&label), JValue::Object(&text)],
                )?
                .l()?;
            env.call_method(
                &manager,
                jni_str!("setPrimaryClip"),
                jni_sig!("(Landroid/content/ClipData;)V"),
                &[JValue::Object(&clip)],
            )?;
            Ok(())
        });
    }

    pub(super) fn get() -> Option<String> {
        with(|env, activity| {
            let manager = manager(env, activity)?;
            let clip = env
                .call_method(
                    &manager,
                    jni_str!("getPrimaryClip"),
                    jni_sig!("()Landroid/content/ClipData;"),
                    &[],
                )?
                .l()?;
            let item = env
                .call_method(
                    &clip,
                    jni_str!("getItemAt"),
                    jni_sig!("(I)Landroid/content/ClipData$Item;"),
                    &[JValue::Int(0)],
                )?
                .l()?;
            let chars = env
                .call_method(
                    &item,
                    jni_str!("coerceToText"),
                    jni_sig!("(Landroid/content/Context;)Ljava/lang/CharSequence;"),
                    &[JValue::Object(activity)],
                )?
                .l()?;
            let string = env
                .call_method(
                    &chars,
                    jni_str!("toString"),
                    jni_sig!("()Ljava/lang/String;"),
                    &[],
                )?
                .l()?;
            env.cast_local::<JString>(string)?.try_to_string(env)
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::Closure;
    use web_sys::{ClipboardEvent, HtmlDocument, KeyboardEvent};

    static OFFERED: Mutex<Option<String>> = Mutex::new(None);

    pub(super) fn offer(text: Option<String>) {
        if text.is_some() {
            install();
        }
        if let Ok(mut slot) = OFFERED.lock() {
            *slot = text;
        }
    }

    fn offered() -> Option<String> {
        OFFERED.lock().ok().and_then(|slot| slot.clone())
    }

    fn install() {
        static INSTALLED: AtomicBool = AtomicBool::new(false);
        if INSTALLED.swap(true, Ordering::Relaxed) {
            return;
        }
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };

        let on_copy = Closure::<dyn FnMut(ClipboardEvent)>::new(|event: ClipboardEvent| {
            let Some(text) = offered() else {
                return;
            };
            let Some(data) = event.clipboard_data() else {
                return;
            };
            if data.set_data("text/plain", &text).is_ok() {
                event.prevent_default();
            }
        });
        let _ = doc.add_event_listener_with_callback("copy", on_copy.as_ref().unchecked_ref());
        on_copy.forget();

        let on_keydown = Closure::<dyn FnMut(KeyboardEvent)>::new(|event: KeyboardEvent| {
            let chord = (event.ctrl_key() || event.meta_key())
                && !event.shift_key()
                && !event.alt_key()
                && event.code() == "KeyC";
            if !chord {
                return;
            }
            let Some(text) = offered() else {
                return;
            };
            let html: Option<HtmlDocument> = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.dyn_into().ok());
            let copied = html.is_some_and(|d| d.exec_command("copy").unwrap_or(false));
            if !copied {
                copy(&text);
            }
            event.prevent_default();
            event.stop_propagation();
        });
        let _ = doc.add_event_listener_with_callback_and_bool(
            "keydown",
            on_keydown.as_ref().unchecked_ref(),
            true,
        );
        on_keydown.forget();
    }

    static PASTED: Mutex<String> = Mutex::new(String::new());

    #[wasm_bindgen::prelude::wasm_bindgen]
    pub fn on_paste(text: String) {
        if let Ok(mut slot) = PASTED.lock() {
            *slot = text;
        }
    }

    pub(super) fn take_pasted() -> String {
        PASTED
            .lock()
            .map(|mut slot| std::mem::take(&mut *slot))
            .unwrap_or_default()
    }

    pub(super) fn copy(text: &str) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let _ = window.navigator().clipboard().write_text(text);
    }
}
