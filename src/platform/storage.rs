pub(crate) struct Slot(pub &'static str);

macro_rules! slots {
    ($($(#[doc = $doc:literal])* $(#[cfg($cfg:meta)])? $name:ident = $file:literal;)*) => {
        $(
            $(#[doc = $doc])*
            $(#[cfg($cfg)])?
            pub(crate) const $name: Slot = Slot($file);
        )*

        #[cfg(not(target_arch = "wasm32"))]
        pub(crate) const ALL: &[Slot] = &[$($(#[cfg($cfg)])? $name,)*];
    };
}

slots! {
    SERVERS = "servers.json";
    OPTIONS = "options.json";
    KEYBINDS = "keybinds.json";
    MODULES = "modules.json";
    #[cfg(any(feature = "eagler", target_arch = "wasm32"))]
    PROXY = "proxy.txt";
    PROFILE = "profile.json";
    ACCOUNTS = "accounts.json";
    COMMAND_HISTORY = "command_history.txt";
    #[cfg(eagler_skins)]
    SKINS = "skins.json";
    #[cfg(feature = "hud_editor")]
    HUD = "hud.json";
    #[cfg(feature = "tutorial")]
    TUTORIAL = "tutorial.txt";
    #[cfg(resource_packs)]
    RESOURCE_PACKS = "resourcepacks.json";
    #[cfg(explore_modrinth)]
    MODRINTH_PACKS = "modrinth_packs.json";
    #[cfg(feature = "shader_support")]
    SHADER_PACK = "shaderpack.txt";
    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    SOUNDS_SKIPPED = "sounds_skipped.txt";
    #[cfg(all(feature = "update_checker", not(target_arch = "wasm32")))]
    UPDATE_CHECK = "update_check.json";
    #[cfg(lua_mods)]
    MODS_DISABLED = "mods_disabled.json";
    #[cfg(lua_mods)]
    MODS_LOOKUP = "mods_lookup.json";
    #[cfg(lua_mods)]
    MODS_FILES = "mods_files.json";
    #[cfg(lua_mods)]
    MODS_PACKETS = "mods_packets.json";
    #[cfg(lua_mods)]
    MODS_LARGE_FILES = "mods_large_files.json";
    #[cfg(lua_mods)]
    MODS_ONLINE = "mods_online.json";
    #[cfg(feature = "asset_download")]
    LANGUAGE = "language.txt";
    #[cfg(feature = "asset_download")]
    LANGUAGE_LIST = "language_list.txt";
    #[cfg(feature = "seed_cracker")]
    SEED_CRACKER_CACHE = ".seedcracker_cache";
}

impl Slot {
    pub(crate) fn load(&self) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            if self.in_memory() {
                return web::MEMORY.lock().get(self.0).cloned();
            }
            return web::get(&self.web_key());
        }
        #[cfg(not(target_arch = "wasm32"))]
        std::fs::read_to_string(self.path()).ok()
    }

    pub(crate) fn store(&self, value: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            if self.in_memory() {
                web::MEMORY.lock().insert(self.0, value.to_owned());
                return;
            }
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

    #[cfg(lua_mods)]
    pub(crate) fn remove(&self) -> std::io::Result<()> {
        #[cfg(target_arch = "wasm32")]
        {
            if self.in_memory() {
                web::MEMORY.lock().remove(self.0);
            } else {
                web::remove(&self.web_key());
            }
            return Ok(());
        }
        #[cfg(not(target_arch = "wasm32"))]
        match std::fs::remove_file(self.path()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
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

    #[cfg(target_arch = "wasm32")]
    fn in_memory(&self) -> bool {
        #[cfg(resource_packs)]
        if self.0 == RESOURCE_PACKS.0 {
            return true;
        }
        #[cfg(explore_modrinth)]
        if self.0 == MODRINTH_PACKS.0 {
            return true;
        }
        #[cfg(feature = "shader_support")]
        if self.0 == SHADER_PACK.0 {
            return true;
        }
        false
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
    #[cfg(target_os = "linux")]
    if let Some(base) = xdg_config_home().filter(|p| p.is_dir()) {
        return base.join("torch-client");
    }
    std::path::PathBuf::from("/tmp/torch-client")
}

#[cfg(target_os = "linux")]
fn xdg_config_home() -> Option<std::path::PathBuf> {
    xdg_base("XDG_CONFIG_HOME", ".config")
}

#[cfg(target_os = "linux")]
pub(crate) fn xdg_cache_home() -> Option<std::path::PathBuf> {
    xdg_base("XDG_CACHE_HOME", ".cache")
}

#[cfg(target_os = "linux")]
fn xdg_base(var: &str, under_home: &str) -> Option<std::path::PathBuf> {
    let absolute = |p: std::path::PathBuf| Some(p).filter(|p| p.is_absolute());
    std::env::var_os(var)
        .map(std::path::PathBuf::from)
        .and_then(absolute)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| std::path::PathBuf::from(h).join(under_home))
                .and_then(absolute)
        })
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
    pub(super) static MEMORY: parking_lot::Mutex<std::collections::BTreeMap<&'static str, String>> =
        parking_lot::Mutex::new(std::collections::BTreeMap::new());

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

    #[cfg(lua_mods)]
    pub(super) fn remove(key: &str) {
        if let Some(storage) = storage() {
            let _ = storage.remove_item(key);
        }
    }

    #[cfg(feature = "asset_download")]
    pub(super) fn all(prefix: &str) -> Vec<(String, String)> {
        let Some(storage) = storage() else {
            return Vec::new();
        };
        let len = storage.length().unwrap_or(0);
        (0..len)
            .filter_map(|i| storage.key(i).ok().flatten())
            .filter_map(|key| {
                let name = key.strip_prefix(prefix)?.to_owned();
                let value = storage.get_item(&key).ok().flatten()?;
                Some((name, value))
            })
            .collect()
    }
}

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
pub(crate) fn export() -> Vec<(String, String)> {
    let prefix = Slot("").web_key();
    let mut docs: Vec<_> = web::all(&prefix)
        .into_iter()
        .filter(|(name, _)| !name.starts_with("mod-data/"))
        .collect();
    docs.sort_by(|a, b| a.0.cmp(&b.0));
    docs
}
