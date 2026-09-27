use std::{collections::HashMap, sync::LazyLock};

use compact_str::CompactString;

static STORAGE: LazyLock<Box<[(CompactString, CompactString)]>> = LazyLock::new(|| {
    let json =
        serde_json::from_str::<HashMap<CompactString, CompactString>>(include_str!("en_us.json"))
            .unwrap();
    let mut json = json.into_iter().collect::<Vec<_>>();

    json.sort_by(|a, b| a.0.cmp(&b.0));

    json.into_boxed_slice()
});

pub fn get(key: &str) -> Option<&str> {
    let key = CompactString::from(key);
    let storage = &*STORAGE;
    let index = storage.binary_search_by(|(k, _)| k.cmp(&key));
    index.ok().map(|i| storage[i].1.as_str())
}
