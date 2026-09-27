pub(crate) struct Slot(pub &'static str);

pub(crate) const SERVERS: Slot = Slot("servers.json");
pub(crate) const OPTIONS: Slot = Slot("options.json");
pub(crate) const KEYBINDS: Slot = Slot("keybinds.json");
pub(crate) const PROFILE: Slot = Slot("profile.json");
pub(crate) const MODULES: Slot = Slot("modules.json");
pub(crate) const ACCOUNTS: Slot = Slot("accounts.json");
pub(crate) const COMMAND_HISTORY: Slot = Slot("command_history.txt");

#[cfg(not(target_arch = "wasm32"))]
pub(crate) const ALL: &[Slot] = &[
    SERVERS,
    OPTIONS,
    KEYBINDS,
    MODULES,
    PROFILE,
    ACCOUNTS,
    COMMAND_HISTORY,
];

impl Slot {
    pub(crate) fn load(&self) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            return web::get(&self.web_key());
        }
        #[cfg(not(target_arch = "wasm32"))]
        std::fs::read_to_string(self.path()).ok()
    }

    pub(crate) fn store(&self, value: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            web::set(&self.web_key(), value);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Err(e) = self.store_native(value) {
            crate::log_warn!(
                "storage",
                "could not save {}: {e}. The change is only in memory and will be lost when this \
                 client exits.",
                self.0
            );
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn store_native(&self, value: &str) -> std::io::Result<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, value)?;
        match std::fs::rename(&temp, &path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = std::fs::remove_file(&temp);
                Err(e)
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn path(&self) -> std::path::PathBuf {
        dir().join(self.0)
    }

    #[cfg(target_arch = "wasm32")]
    fn web_key(&self) -> String {
        format!("torch-client/{}", self.0)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn dir() -> std::path::PathBuf {
    match std::env::var_os("MC_CLIENT_CONFIG_DIR") {
        Some(p) => std::path::PathBuf::from(p),
        None => default_dir(),
    }
}

#[cfg(target_os = "windows")]
pub(crate) const WINDOWS_FOLDER: &str = "torch-client";

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "windows")))]
fn default_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("/tmp/torch-client")
}

#[cfg(target_os = "windows")]
fn default_dir() -> std::path::PathBuf {
    match std::env::var_os("LOCALAPPDATA").filter(|p| !p.is_empty()) {
        Some(local) => std::path::PathBuf::from(local)
            .join(WINDOWS_FOLDER)
            .join("config"),
        None => std::env::temp_dir().join(WINDOWS_FOLDER).join("config"),
    }
}

#[cfg(target_os = "android")]
fn default_dir() -> std::path::PathBuf {
    bevy::android::ANDROID_APP
        .get()
        .and_then(|app| app.internal_data_path())
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(target_arch = "wasm32")]
mod web {
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub(super) fn get(key: &str) -> Option<String> {
        storage()?.get_item(key).ok()?
    }

    pub(super) fn set(key: &str, value: &str) {
        if let Some(storage) = storage() {
            let _ = storage.set_item(key, value);
        }
    }
}
