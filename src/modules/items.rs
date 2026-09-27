use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use azalea_registry::Registry;
use azalea_registry::builtin::ItemKind;

use super::list::{BitList, Built, Entry};

static GROUPS: [&str; 1] = ["Items"];

fn build() -> Built {
    let mut entries = Vec::with_capacity(1500);
    for id in 0u32.. {
        if !ItemKind::is_valid_id(id) {
            break;
        }
        let Some(kind) = <ItemKind as Registry>::from_u32(id) else {
            break;
        };
        if kind == ItemKind::Air {
            continue;
        }
        entries.push(Entry {
            group: 0,
            label: Cow::Owned(kind.to_str().trim_start_matches("minecraft:").to_string()),
            on: false,
            color: 0,
        });
    }
    (entries, Vec::new())
}

pub static LIST: BitList = BitList::new("items", "Items", 214.0, &GROUPS, false, &[], build);

static BY_NAME: OnceLock<HashMap<String, u16>> = OnceLock::new();

fn by_name() -> &'static HashMap<String, u16> {
    BY_NAME.get_or_init(|| {
        (0..LIST.count())
            .map(|i| (LIST.label(i).to_string(), i as u16))
            .collect()
    })
}

pub fn item_selected(item: &str) -> bool {
    match by_name().get(item) {
        Some(&i) => LIST.enabled(i as usize),
        None => false,
    }
}
