use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use mlua::Error;

use super::api::Ctx;
use crate::platform::storage::{MODS_FILES, MODS_LARGE_FILES, MODS_LOOKUP, MODS_PACKETS, Slot};

pub(super) struct Perm {
    pub name: &'static str,
    pub slot: Slot,
    pub on_install: bool,
    pub allow: &'static str,
    pub block: &'static str,
}

pub(super) const LOOKUP: usize = 0;
pub(super) const FILES: usize = 1;
pub(super) const PACKETS: usize = 2;
pub(super) const LARGE_FILES: usize = 3;
const COUNT: usize = 4;

pub(super) static PERMS: [Perm; COUNT] = [
    Perm {
        name: "profile_lookup",
        slot: MODS_LOOKUP,
        on_install: false,
        allow: "Allow lookups",
        block: "Block lookups",
    },
    Perm {
        name: "files",
        slot: MODS_FILES,
        on_install: true,
        allow: "Allow files",
        block: "Block files",
    },
    Perm {
        name: "packets",
        slot: MODS_PACKETS,
        on_install: false,
        allow: "Allow packets",
        block: "Block packets",
    },
    Perm {
        name: "large_files",
        slot: MODS_LARGE_FILES,
        on_install: false,
        allow: "Allow large files",
        block: "Block large files",
    },
];

pub(super) fn large_files(id: &str) -> bool {
    !cfg!(target_arch = "wasm32")
        && granted(LARGE_FILES, id)
        && super::lists_permission(id, PERMS[LARGE_FILES].name)
}

pub(super) fn names() -> Vec<&'static str> {
    PERMS.iter().map(|p| p.name).collect()
}

pub(super) fn index(name: &str) -> Option<usize> {
    PERMS.iter().position(|p| p.name == name)
}

pub fn notice(perm: &str, id: &str) -> String {
    match index(perm) {
        Some(LOOKUP) => {
            "Sends the uuids it looks up to sessionserver.mojang.com for the player's name."
                .to_string()
        }
        Some(FILES) => format!(
            "Reads and writes files in mod-data/{id}/ only, up to {} MiB.",
            super::fs::QUOTA >> 20
        ),
        Some(PACKETS) => "Sends any game packet it builds to the server as you.".to_string(),
        Some(LARGE_FILES) if cfg!(target_arch = "wasm32") => format!(
            "Not available in the browser: mod-data/{id}/ stays at {} MiB here.",
            super::fs::QUOTA >> 20
        ),
        Some(LARGE_FILES) => format!(
            "Raises mod-data/{id}/ from {} MiB to {} MiB of your disk. Reads stay at most {} MiB a file.",
            super::fs::QUOTA >> 20,
            super::fs::LARGE_QUOTA >> 20,
            super::fs::QUOTA >> 20
        ),
        _ => String::new(),
    }
}

pub(super) fn id_set(slot: &Slot) -> BTreeSet<String> {
    slot.load()
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .map(|ids| ids.into_iter().collect())
        .unwrap_or_default()
}

pub(super) fn store_id_set(slot: &Slot, ids: &BTreeSet<String>) {
    let list: Vec<&String> = ids.iter().collect();
    slot.store(&serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()));
}

static GRANTED: Mutex<[Option<BTreeSet<String>>; COUNT]> = Mutex::new([const { None }; COUNT]);

fn with<R>(perm: usize, f: impl FnOnce(&mut BTreeSet<String>) -> R) -> R {
    let mut all = GRANTED.lock().unwrap_or_else(PoisonError::into_inner);
    f(all[perm].get_or_insert_with(|| id_set(&PERMS[perm].slot)))
}

pub(super) fn granted(perm: usize, id: &str) -> bool {
    with(perm, |set| set.contains(id))
}

pub(super) fn set_granted(perm: usize, id: &str, on: bool) {
    if on && perm == LARGE_FILES && cfg!(target_arch = "wasm32") {
        crate::log_info!(
            "mods",
            "large_files refused for {id}: not available in the browser"
        );
        return;
    }
    with(perm, |set| {
        let changed = if on {
            set.insert(id.to_string())
        } else {
            set.remove(id)
        };
        if changed {
            store_id_set(&PERMS[perm].slot, set);
            crate::log_info!(
                "mods",
                "{} {} for {id}",
                PERMS[perm].name,
                if on { "granted" } else { "revoked" }
            );
        }
    });
}

pub(super) fn declared(ctx: &Ctx, perm: usize, id: &str) -> bool {
    ctx.perms
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains(&(id, perm))
}

pub(super) fn undeclared(what: &str, perm: usize) -> Error {
    Error::runtime(format!(
        "{what}: add \"permissions\": [\"{}\"] to mod.json first (a single-file mod cannot)",
        PERMS[perm].name
    ))
}
