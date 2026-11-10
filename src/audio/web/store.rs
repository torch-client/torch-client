use std::borrow::Cow;
use std::collections::HashMap;

use crate::platform::assets::archive;

pub(crate) const DEFS_KEY: &str = ".sounds.json";

pub(crate) const PREFIX: &str = ".sounds/";

pub(crate) fn archive_key(object_key: &str) -> String {
    format!("{PREFIX}{object_key}")
}

pub(crate) struct Sounds {
    pub(crate) defs: String,
    files: HashMap<String, &'static [u8]>,
}

impl Sounds {
    pub(crate) fn from_archive() -> Option<Sounds> {
        let defs = std::str::from_utf8(archive::get(DEFS_KEY)?)
            .ok()?
            .to_owned();
        let files = archive::descendants(PREFIX)
            .filter_map(|key| Some((key.to_owned(), archive::get(&archive_key(key))?)))
            .collect();
        Some(Sounds { defs, files })
    }

    pub(crate) fn from_entries(entries: Vec<(String, Vec<u8>)>) -> Option<Sounds> {
        let mut defs = None;
        let mut files = HashMap::with_capacity(entries.len());
        for (key, bytes) in entries {
            if key == DEFS_KEY {
                defs = String::from_utf8(bytes).ok();
            } else if let Some(object_key) = key.strip_prefix(PREFIX) {
                files.insert(object_key.to_owned(), &*Box::leak(bytes.into_boxed_slice()));
            }
        }
        Some(Sounds { defs: defs?, files })
    }

    pub(crate) fn read(&self, key: &str) -> Option<Cow<'static, [u8]>> {
        if let Some(bytes) = super::super::pack_override(key) {
            return Some(Cow::Owned(bytes.to_vec()));
        }
        self.files.get(key).map(|bytes| Cow::Borrowed(*bytes))
    }

    #[cfg(resource_packs)]
    pub(crate) fn has(&self, key: &str) -> bool {
        self.files.contains_key(key) || archive::pack_has(&super::super::defs::layer_key(key))
    }

    pub(crate) fn len(&self) -> usize {
        self.files.len()
    }
}
