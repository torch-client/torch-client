use serde_json::{Map, Value as Json};

use super::super::Store;
use super::super::persist;
use super::super::registry::{Kind, ModuleIx};
use super::MOD_DATA;
use super::api::intern;
use crate::platform::storage::Slot;

pub(super) struct Mod {
    pub id: &'static str,
    pub doc: Slot,
    pub modules: Vec<Module>,
}

pub(super) struct Module {
    pub ix: ModuleIx,
    pub name: &'static str,
    pub settings: Vec<(usize, &'static str, Kind)>,
}

impl Module {
    fn keyed(&self) -> impl Iterator<Item = (usize, &'static str, Kind)> + '_ {
        self.settings.iter().copied()
    }
}

pub(super) fn doc(id: &str) -> Slot {
    Slot(intern(format!("{MOD_DATA}/{id}.json")))
}

pub(super) fn delete(id: &str) {
    let doc = doc(id);
    if let Err(e) = doc.remove() {
        crate::log_warn!("mods", "could not delete {}: {e}", doc.0);
    }
}

pub(super) fn load(s: &Store, mods: &[Mod]) {
    for m in mods {
        let Some(text) = m.doc.load() else {
            continue;
        };
        let Ok(Json::Object(root)) = serde_json::from_str::<Json>(&text) else {
            crate::log_warn!(
                "mods",
                "{} is not a JSON object; {} starts from its defaults",
                m.doc.0,
                m.id
            );
            continue;
        };
        for module in &m.modules {
            if let Some(saved) = root.get(module.name).and_then(Json::as_object) {
                persist::load_module(s, module.ix, saved, module.keyed());
            }
        }
    }
}

pub(super) fn save(s: &Store, mods: &[Mod]) {
    for m in mods {
        let mut root = Map::new();
        for module in &m.modules {
            root.insert(
                module.name.to_string(),
                persist::module_json(s, module.ix, module.keyed()),
            );
        }
        m.doc.store(&Json::Object(root).to_string());
    }
}
