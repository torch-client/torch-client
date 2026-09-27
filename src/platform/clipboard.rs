pub fn get() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web::take_pasted()
    }
    #[cfg(target_os = "android")]
    {
        String::new()
    }
    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    {
        native::with(|clip| clip.get_text().unwrap_or_default()).unwrap_or_default()
    }
}

pub fn set(text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        web::copy(text);
    }
    #[cfg(target_os = "android")]
    {
        let _ = text;
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

#[cfg(target_arch = "wasm32")]
mod web {
    use std::sync::Mutex;

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
