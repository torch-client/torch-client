use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use azalea::Identifier;
use azalea_chat::FormattedText;
use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::components::BannerPatternData;
use azalea_registry::data::BannerPatternKind;
use azalea_registry::{DataRegistry, Holder};
use serde_json::Value;
use simdnbt::owned::NbtCompound;

pub(super) fn registry_entry<'a>(
    registries: &'a RegistryHolder,
    registry: &str,
    protocol_id: u32,
) -> Option<(&'a Identifier, Option<&'a NbtCompound>)> {
    let (name, data) = registries
        .extra
        .get(&Identifier::new(registry))?
        .map
        .get_index(protocol_id as usize)?;
    Some((name, (!data.is_empty()).then_some(data)))
}

pub(super) fn registry_entry_by_key<'a>(
    registries: &'a RegistryHolder,
    registry: &str,
    key: &Identifier,
) -> Option<&'a NbtCompound> {
    let data = registries
        .extra
        .get(&Identifier::new(registry))?
        .map
        .get(key)?;
    (!data.is_empty()).then_some(data)
}

fn datapack(registry: &str) -> HashMap<String, Value> {
    crate::util::datapack::entries(registry)
        .into_iter()
        .collect()
}

static INSTRUMENTS: LazyLock<HashMap<String, Value>> = LazyLock::new(|| datapack("instrument"));
static BANNER_PATTERNS: LazyLock<HashMap<String, Value>> =
    LazyLock::new(|| datapack("banner_pattern"));

fn file<'a>(table: &'a HashMap<String, Value>, name: &Identifier) -> Option<&'a Value> {
    (name.namespace() == "minecraft")
        .then(|| table.get(name.path()))
        .flatten()
}

pub(super) fn instrument_description(
    name: &Identifier,
    entry: Option<&NbtCompound>,
) -> Option<FormattedText> {
    match entry {
        Some(entry) => crate::client::chat_text::from_nbt_tag(entry.get("description")?),
        None => {
            let json = file(&INSTRUMENTS, name)?.get("description")?;
            serde_json::from_value::<FormattedText>(json.clone()).ok()
        }
    }
}

pub(super) fn banner_translation_key(
    pattern: &Holder<BannerPatternKind, BannerPatternData>,
    registries: Option<&RegistryHolder>,
) -> Option<String> {
    match pattern {
        Holder::Direct(inline) => Some(inline.translation_key.clone()),
        Holder::Reference(kind) => {
            let (name, entry) =
                registry_entry(registries?, BannerPatternKind::NAME, kind.protocol_id())?;
            match entry {
                Some(entry) => Some(entry.string("translation_key")?.to_str().into_owned()),
                None => Some(
                    file(&BANNER_PATTERNS, name)?
                        .get("translation_key")?
                        .as_str()?
                        .to_owned(),
                ),
            }
        }
    }
}

static BLOCK_TAGS: LazyLock<Mutex<HashMap<String, Arc<[String]>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn block_tag(tag: &Identifier) -> Arc<[String]> {
    let key = format!("{}:{}", tag.namespace(), tag.path());
    if let Ok(cache) = BLOCK_TAGS.lock()
        && let Some(blocks) = cache.get(&key)
    {
        return blocks.clone();
    }
    let mut blocks = Vec::new();
    expand_block_tag(tag, &mut blocks, 0);
    let blocks: Arc<[String]> = blocks.into();
    if let Ok(mut cache) = BLOCK_TAGS.lock() {
        cache.insert(key, blocks.clone());
    }
    blocks
}

const MAX_TAG_DEPTH: usize = 16;

fn expand_block_tag(tag: &Identifier, out: &mut Vec<String>, depth: usize) {
    if depth > MAX_TAG_DEPTH || tag.namespace() != "minecraft" {
        return;
    }
    let Some(json) = crate::util::datapack::entry("tags/block", tag.path()) else {
        return;
    };
    let Some(values) = json.get("values").and_then(Value::as_array) else {
        return;
    };
    for value in values {
        let Some(id) = value
            .as_str()
            .or_else(|| value.get("id").and_then(Value::as_str))
        else {
            continue;
        };
        match id.strip_prefix('#') {
            Some(nested) => expand_block_tag(&Identifier::new(nested), out, depth + 1),
            None => {
                let id = Identifier::new(id);
                out.push(format!("{}:{}", id.namespace(), id.path()));
            }
        }
    }
}
