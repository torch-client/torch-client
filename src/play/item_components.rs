use std::collections::HashMap;
use std::io::Cursor;
use std::str::FromStr;

use azalea::Client;
use azalea_chat::FormattedText;
use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::components::{self as comp, DataComponentUnion};
use azalea_inventory::{ItemStack, ItemStackData};
use azalea_registry::builtin::DataComponentKind;
use simdnbt::owned::{NbtCompound, NbtList, NbtTag};

use crate::text::{GOLD, GREEN, RED, Span, WHITE};
use crate::util::snbt;

pub enum ItemEdit {
    Get,
    Copy,
    Add(Vec<Entry>),
    Set(Vec<Entry>),
    Remove(DataComponentKind),
    Count(i32),
}

pub struct Entry {
    pub kind: DataComponentKind,
    pub value: Option<NbtTag>,
}

pub const TYPED: [&str; 15] = [
    "custom_name",
    "item_name",
    "lore",
    "custom_data",
    "block_entity_data",
    "entity_data",
    "damage",
    "max_damage",
    "max_stack_size",
    "repair_cost",
    "unbreakable",
    "enchantment_glint_override",
    "enchantments",
    "stored_enchantments",
    "dyed_color",
];

pub const COUNT_RANGE: std::ops::RangeInclusive<i32> = 1..=99;

pub fn key(kind: DataComponentKind) -> String {
    namespaced(kind.to_string())
}

fn namespaced(s: String) -> String {
    if s.contains(':') {
        s
    } else {
        format!("minecraft:{s}")
    }
}

pub fn kind_named(name: &str) -> Option<DataComponentKind> {
    DataComponentKind::from_str(name).ok()
}

pub fn raw_union(
    kind: DataComponentKind,
    bytes: &[u8],
    what: &str,
) -> Result<DataComponentUnion, String> {
    let mut cursor = Cursor::new(bytes);
    let union = DataComponentUnion::azalea_read_as(kind, &mut cursor)
        .map_err(|e| format!("{what} did not parse as {}: {e}", key(kind)))?;
    let left = bytes.len() - cursor.position() as usize;
    if left != 0 {
        let mut union = union;
        unsafe { union.drop_as(kind) };
        return Err(format!("{what} has {left} bytes past the component's end"));
    }
    Ok(union)
}

pub fn checked_enchantments(
    pairs: &[(String, i32)],
    path: &str,
    registries: Option<&RegistryHolder>,
) -> Result<HashMap<azalea_registry::data::Enchantment, i32>, String> {
    use crate::play::inventory_bridge::{enchantment_name, resolve_enchantments};
    if let Some((name, _)) = pairs.iter().find(|(_, lvl)| !(1..=255).contains(lvl)) {
        return Err(format!("{path}.{name}: a level is 1 to 255"));
    }
    let out = resolve_enchantments(registries, pairs);
    if out.len() != pairs.len() {
        let known: Vec<String> = out
            .keys()
            .map(|e| enchantment_name(registries, e))
            .collect();
        let missing: Vec<&str> = pairs
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| !known.iter().any(|k| k == n))
            .collect();
        return Err(format!(
            "{path}: the server has no enchantment {}",
            missing.join(", ")
        ));
    }
    Ok(out)
}

fn id_char(c: char) -> bool {
    matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.' | ':' | '/')
}

fn skip_ws(s: &str, at: &mut usize) {
    while let Some(c) = s[*at..].chars().next().filter(|c| c.is_whitespace()) {
        *at += c.len_utf8();
    }
}

fn read_kind(s: &str, at: &mut usize) -> Result<DataComponentKind, String> {
    let start = *at;
    let len = s[start..]
        .find(|c: char| !id_char(c))
        .unwrap_or(s.len() - start);
    if len == 0 {
        return Err(crate::lang::trf!("play.nbt.expected_name", start));
    }
    *at += len;
    let name = &s[start..*at];
    kind_named(name).ok_or_else(|| crate::lang::trf!("play.nbt.unknown_component", name, start))
}

pub fn parse_components(text: &str) -> Result<Vec<Entry>, String> {
    let s = text.trim_end();
    let mut at = 0;
    skip_ws(s, &mut at);
    if !s[at..].starts_with('[') {
        return Err(crate::lang::tr!("play.nbt.expected_open").into());
    }
    at += 1;
    let mut out: Vec<Entry> = Vec::new();
    loop {
        skip_ws(s, &mut at);
        if s[at..].starts_with(']') {
            break;
        }
        let removal = s[at..].starts_with('!');
        if removal {
            at += 1;
        }
        let kind = read_kind(s, &mut at)?;
        if out.iter().any(|e| e.kind == kind) {
            return Err(crate::lang::trf!("play.nbt.named_twice", key(kind)));
        }
        skip_ws(s, &mut at);
        let value = if removal {
            None
        } else {
            if !s[at..].starts_with('=') {
                return Err(crate::lang::trf!("play.nbt.expected_equals", key(kind), at));
            }
            at += 1;
            skip_ws(s, &mut at);
            let (tag, used) = snbt::parse_prefix(&s[at..]).map_err(|e| {
                let e = snbt::Error {
                    at: e.at + at,
                    message: e.message,
                };
                crate::lang::trf!("play.nbt.malformed", key(kind), e)
            })?;
            at += used;
            skip_ws(s, &mut at);
            Some(tag)
        };
        out.push(Entry { kind, value });
        if s[at..].starts_with(',') {
            at += 1;
            skip_ws(s, &mut at);
            if at >= s.len() {
                return Err(crate::lang::trf!("play.nbt.expected_component", at));
            }
            continue;
        }
        if !s[at..].starts_with(']') {
            return Err(crate::lang::trf!("play.nbt.expected_separator", at));
        }
        break;
    }
    at += 1;
    if at < s.len() {
        return Err(crate::lang::trf!("play.nbt.trailing", at));
    }
    Ok(out)
}

fn unwrapped(tag: &NbtTag) -> &NbtTag {
    match tag {
        NbtTag::Compound(c) if c.len() == 1 => match c.get("") {
            Some(inner) => inner,
            None => tag,
        },
        _ => tag,
    }
}

fn elements(list: &NbtList) -> Vec<NbtTag> {
    fn each<T: Clone>(v: &[T], f: impl Fn(T) -> NbtTag) -> Vec<NbtTag> {
        v.iter().cloned().map(f).collect()
    }
    match list {
        NbtList::Empty => Vec::new(),
        NbtList::Byte(v) => each(v, NbtTag::Byte),
        NbtList::Short(v) => each(v, NbtTag::Short),
        NbtList::Int(v) => each(v, NbtTag::Int),
        NbtList::Long(v) => each(v, NbtTag::Long),
        NbtList::Float(v) => each(v, NbtTag::Float),
        NbtList::Double(v) => each(v, NbtTag::Double),
        NbtList::ByteArray(v) => each(v, NbtTag::ByteArray),
        NbtList::String(v) => each(v, NbtTag::String),
        NbtList::List(v) => each(v, NbtTag::List),
        NbtList::Compound(v) => each(v, NbtTag::Compound),
        NbtList::IntArray(v) => each(v, NbtTag::IntArray),
        NbtList::LongArray(v) => each(v, NbtTag::LongArray),
    }
}

fn text(tag: &NbtTag, path: &str) -> Result<FormattedText, String> {
    use simdnbt::FromNbtTag;
    let tag = unwrapped(tag);
    if let NbtTag::List(list) = tag {
        let mut parts = elements(list).into_iter();
        let Some(first) = parts.next() else {
            return Ok(FormattedText::default());
        };
        let mut out = text(&first, path)?;
        for part in parts {
            out.get_base_mut().siblings.push(text(&part, path)?);
        }
        return Ok(out);
    }
    if !matches!(tag, NbtTag::String(_) | NbtTag::Compound(_)) {
        return Err(format!(
            "{path} is a string, a text compound or a list of them"
        ));
    }
    let mut bytes = Vec::new();
    tag.write(&mut bytes);
    let borrowed = simdnbt::borrow::read_optional_tag(&mut Cursor::new(&bytes[..]))
        .ok()
        .flatten()
        .ok_or_else(|| format!("{path} did not read back as NBT"))?;
    FormattedText::from_nbt_tag(borrowed.as_tag())
        .ok_or_else(|| format!("{path} is not a text component"))
}

fn int(tag: &NbtTag, path: &str) -> Result<i32, String> {
    let n: Option<i64> = match unwrapped(tag) {
        NbtTag::Byte(v) => Some((*v).into()),
        NbtTag::Short(v) => Some((*v).into()),
        NbtTag::Int(v) => Some((*v).into()),
        NbtTag::Long(v) => Some(*v),
        NbtTag::Float(v) if v.fract() == 0.0 => Some(*v as i64),
        NbtTag::Double(v) if v.fract() == 0.0 => Some(*v as i64),
        _ => None,
    };
    n.and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| format!("{path} is a whole number"))
}

fn int_in(tag: &NbtTag, path: &str, range: std::ops::RangeInclusive<i32>) -> Result<i32, String> {
    let n = int(tag, path)?;
    if range.contains(&n) {
        Ok(n)
    } else if *range.end() == i32::MAX {
        Err(format!("{path} is at least {}", range.start()))
    } else {
        Err(format!("{path} is {} to {}", range.start(), range.end()))
    }
}

fn boolean(tag: &NbtTag, path: &str) -> Result<bool, String> {
    match unwrapped(tag) {
        NbtTag::Byte(v) => Ok(*v != 0),
        NbtTag::Short(v) => Ok(*v != 0),
        NbtTag::Int(v) => Ok(*v != 0),
        NbtTag::Long(v) => Ok(*v != 0),
        _ => Err(format!("{path} is true or false")),
    }
}

fn compound<'a>(tag: &'a NbtTag, path: &str) -> Result<&'a NbtCompound, String> {
    match unwrapped(tag) {
        NbtTag::Compound(c) => Ok(c),
        _ => Err(format!("{path} is a compound {{...}}")),
    }
}

fn typed_data<K: FromStr>(tag: &NbtTag, path: &str, of: &str) -> Result<(K, NbtCompound), String> {
    let mut data = compound(tag, path)?.clone();
    let id = match data.remove("id") {
        Some(NbtTag::String(s)) => s.to_str().into_owned(),
        _ => return Err(format!("{path} needs an id, the {of} type, as a string")),
    };
    let kind = K::from_str(&id).map_err(|_| format!("{path}.id: {id} is not a {of} type"))?;
    Ok((kind, data))
}

fn enchantments(
    tag: &NbtTag,
    path: &str,
    registries: Option<&RegistryHolder>,
) -> Result<HashMap<azalea_registry::data::Enchantment, i32>, String> {
    let mut pairs = Vec::new();
    for (name, lvl) in compound(tag, path)?.iter() {
        let name = name.to_str();
        let lvl = int(lvl, &format!("{path}.{name}"))?;
        pairs.push((
            name.strip_prefix("minecraft:")
                .unwrap_or(&*name)
                .to_string(),
            lvl,
        ));
    }
    checked_enchantments(&pairs, path, registries)
}

fn set_one(
    stack: ItemStack,
    kind: DataComponentKind,
    tag: &NbtTag,
    registries: Option<&RegistryHolder>,
) -> Result<ItemStack, String> {
    let k = key(kind);
    let bare = k.strip_prefix("minecraft:").unwrap_or(&k);
    let path = bare;
    Ok(match bare {
        "custom_name" => stack.with_component(comp::CustomName {
            name: text(tag, path)?,
        }),
        "item_name" => stack.with_component(comp::ItemName {
            name: text(tag, path)?,
        }),
        "lore" => {
            let NbtTag::List(list) = unwrapped(tag) else {
                return Err(format!("{path} is a list of lines"));
            };
            let items = elements(list);
            if items.len() > 256 {
                return Err(format!("{path}: at most 256 lines"));
            }
            let lines = items
                .iter()
                .enumerate()
                .map(|(i, l)| text(l, &format!("{path}[{i}]")))
                .collect::<Result<_, _>>()?;
            stack.with_component(comp::Lore { lines })
        }
        "custom_data" => {
            let nbt = compound(tag, path)?.clone();
            stack.with_component(comp::CustomData {
                nbt: simdnbt::owned::Nbt::new("".into(), nbt),
            })
        }
        "block_entity_data" => {
            let (kind, data) =
                typed_data::<azalea_registry::builtin::BlockEntityKind>(tag, path, "block entity")?;
            stack.with_component(comp::BlockEntityData { kind, data })
        }
        "entity_data" => {
            let (kind, data) =
                typed_data::<azalea_registry::builtin::EntityKind>(tag, path, "entity")?;
            stack.with_component(comp::EntityData { kind, data })
        }
        "damage" => stack.with_component(comp::Damage {
            amount: int_in(tag, path, 0..=i32::MAX)?,
        }),
        "max_damage" => stack.with_component(comp::MaxDamage {
            amount: int_in(tag, path, 1..=i32::MAX)?,
        }),
        "max_stack_size" => stack.with_component(comp::MaxStackSize {
            count: int_in(tag, path, 1..=99)?,
        }),
        "repair_cost" => stack.with_component(comp::RepairCost {
            cost: int_in(tag, path, 0..=i32::MAX)? as u32,
        }),
        "unbreakable" => stack.with_component(comp::Unbreakable),
        "enchantment_glint_override" => stack.with_component(comp::EnchantmentGlintOverride {
            show_glint: boolean(tag, path)?,
        }),
        "enchantments" => stack.with_component(comp::Enchantments {
            levels: enchantments(tag, path, registries)?,
        }),
        "stored_enchantments" => stack.with_component(comp::StoredEnchantments {
            enchantments: enchantments(tag, path, registries)?,
        }),
        "dyed_color" => stack.with_component(comp::DyedColor {
            rgb: int(tag, path)? & 0xFF_FFFF,
        }),
        _ => match raw_union(kind, &[], path) {
            Ok(union) => {
                let mut stack = stack;
                if let Some(data) = stack.as_present_mut() {
                    unsafe {
                        data.component_patch
                            .unchecked_insert_component(kind, Some(union))
                    };
                }
                stack
            }
            Err(_) => {
                return Err(format!(
                    "{k} cannot be written here; the writable ones are {}, any unit component, \
                     and !name removes any",
                    TYPED.join(", ")
                ));
            }
        },
    })
}

fn has_default(item: azalea_registry::builtin::ItemKind, kind: DataComponentKind) -> Option<bool> {
    use DataComponentKind as K;
    use azalea_inventory::default_components::get_default_component as d;
    Some(match kind {
        K::CustomName => d::<comp::CustomName>(item).is_some(),
        K::ItemName => d::<comp::ItemName>(item).is_some(),
        K::Lore => d::<comp::Lore>(item).is_some(),
        K::CustomData => d::<comp::CustomData>(item).is_some(),
        K::BlockEntityData => d::<comp::BlockEntityData>(item).is_some(),
        K::EntityData => d::<comp::EntityData>(item).is_some(),
        K::Damage => d::<comp::Damage>(item).is_some(),
        K::MaxDamage => d::<comp::MaxDamage>(item).is_some(),
        K::MaxStackSize => d::<comp::MaxStackSize>(item).is_some(),
        K::RepairCost => d::<comp::RepairCost>(item).is_some(),
        K::Unbreakable => d::<comp::Unbreakable>(item).is_some(),
        K::EnchantmentGlintOverride => d::<comp::EnchantmentGlintOverride>(item).is_some(),
        K::Enchantments => d::<comp::Enchantments>(item).is_some(),
        K::StoredEnchantments => d::<comp::StoredEnchantments>(item).is_some(),
        K::DyedColor => d::<comp::DyedColor>(item).is_some(),
        _ => return None,
    })
}

fn remove_one(data: &mut ItemStackData, kind: DataComponentKind) {
    let patched = data.component_patch.has_kind(kind);
    let default = has_default(data.kind, kind).unwrap_or(!patched);
    if default {
        unsafe { data.component_patch.unchecked_insert_component(kind, None) };
    } else {
        data.component_patch.clear_kind(kind);
    }
}

fn apply(
    mut stack: ItemStack,
    entries: &[Entry],
    registries: Option<&RegistryHolder>,
) -> Result<ItemStack, String> {
    for e in entries {
        match &e.value {
            Some(tag) => stack = set_one(stack, e.kind, tag, registries)?,
            None => {
                if let Some(data) = stack.as_present_mut() {
                    remove_one(data, e.kind);
                }
            }
        }
    }
    Ok(stack)
}

fn json_tag(v: &serde_json::Value) -> NbtTag {
    use serde_json::Value as J;
    match v {
        J::Null => NbtTag::Compound(NbtCompound::new()),
        J::Bool(b) => NbtTag::Byte(i8::from(*b)),
        J::Number(n) => match n.as_i64() {
            Some(i) => i32::try_from(i).map_or(NbtTag::Long(i), NbtTag::Int),
            None => NbtTag::Double(n.as_f64().unwrap_or(0.0)),
        },
        J::String(s) => NbtTag::String(s.as_str().into()),
        J::Array(items) => NbtTag::List(list_of(items.iter().map(json_tag).collect())),
        J::Object(map) => {
            let mut c = NbtCompound::new();
            for (k, v) in map {
                c.insert(k.as_str(), json_tag(v));
            }
            NbtTag::Compound(c)
        }
    }
}

fn list_of(items: Vec<NbtTag>) -> NbtList {
    let Some(first) = items.first() else {
        return NbtList::Empty;
    };
    let id = first.id();
    if items.iter().any(|t| t.id() != id) {
        return NbtList::Compound(
            items
                .into_iter()
                .map(|t| {
                    let mut c = NbtCompound::new();
                    c.insert("", t);
                    c
                })
                .collect(),
        );
    }
    macro_rules! all {
        ($variant:ident, $into:ident) => {
            NbtList::$variant(items.into_iter().filter_map(|t| t.$into()).collect())
        };
    }
    match first {
        NbtTag::Byte(_) => all!(Byte, into_byte),
        NbtTag::Short(_) => all!(Short, into_short),
        NbtTag::Int(_) => all!(Int, into_int),
        NbtTag::Long(_) => all!(Long, into_long),
        NbtTag::Float(_) => all!(Float, into_float),
        NbtTag::Double(_) => all!(Double, into_double),
        NbtTag::ByteArray(_) => all!(ByteArray, into_byte_array),
        NbtTag::String(_) => all!(String, into_string),
        NbtTag::List(_) => all!(List, into_list),
        NbtTag::Compound(_) => all!(Compound, into_compound),
        NbtTag::IntArray(_) => all!(IntArray, into_int_array),
        NbtTag::LongArray(_) => all!(LongArray, into_long_array),
    }
}

fn text_tag(t: &FormattedText) -> NbtTag {
    use simdnbt::Serialize;
    NbtTag::Compound(t.clone().to_compound())
}

fn enchantment_tag(
    levels: &HashMap<azalea_registry::data::Enchantment, i32>,
    registries: Option<&RegistryHolder>,
) -> NbtTag {
    let mut c = NbtCompound::new();
    for (e, lvl) in levels {
        let name = crate::play::inventory_bridge::enchantment_name(registries, e);
        let name = if name.starts_with('#') || name.contains(':') {
            name
        } else {
            format!("minecraft:{name}")
        };
        c.insert(name.as_str(), NbtTag::Int(*lvl));
    }
    NbtTag::Compound(c)
}

fn typed_data_tag(id: String, data: &NbtCompound) -> NbtTag {
    let mut c = data.clone();
    c.insert("id", NbtTag::String(id.as_str().into()));
    NbtTag::Compound(c)
}

fn component_tag(
    data: &ItemStackData,
    kind: DataComponentKind,
    json: impl FnOnce() -> serde_json::Result<serde_json::Value>,
    registries: Option<&RegistryHolder>,
) -> NbtTag {
    let p = &data.component_patch;
    let found = match kind {
        DataComponentKind::CustomName => p.get::<comp::CustomName>().map(|c| text_tag(&c.name)),
        DataComponentKind::ItemName => p.get::<comp::ItemName>().map(|c| text_tag(&c.name)),
        DataComponentKind::Lore => p
            .get::<comp::Lore>()
            .map(|c| NbtTag::List(list_of(c.lines.iter().map(text_tag).collect()))),
        DataComponentKind::CustomData => p.get::<comp::CustomData>().map(|c| match &c.nbt {
            simdnbt::owned::Nbt::Some(base) => NbtTag::Compound(base.clone().as_compound()),
            simdnbt::owned::Nbt::None => NbtTag::Compound(NbtCompound::new()),
        }),
        DataComponentKind::BlockEntityData => p
            .get::<comp::BlockEntityData>()
            .map(|c| typed_data_tag(namespaced(c.kind.to_string()), &c.data)),
        DataComponentKind::EntityData => p
            .get::<comp::EntityData>()
            .map(|c| typed_data_tag(namespaced(c.kind.to_string()), &c.data)),
        DataComponentKind::Enchantments => p
            .get::<comp::Enchantments>()
            .map(|c| enchantment_tag(&c.levels, registries)),
        DataComponentKind::StoredEnchantments => p
            .get::<comp::StoredEnchantments>()
            .map(|c| enchantment_tag(&c.enchantments, registries)),
        _ => None,
    };
    found.unwrap_or_else(|| match json() {
        Ok(json) => json_tag(&json),
        Err(e) => NbtTag::String(format!("<unreadable: {e}>").as_str().into()),
    })
}

pub fn stack_tag(data: &ItemStackData, registries: Option<&RegistryHolder>) -> NbtCompound {
    let mut out = NbtCompound::new();
    out.insert(
        "id",
        NbtTag::String(namespaced(data.kind.to_string()).as_str().into()),
    );
    out.insert("count", NbtTag::Int(data.count));
    let mut components = NbtCompound::new();
    for (kind, value) in data.component_patch.serializable() {
        match value {
            None => components.insert(
                format!("!{}", key(kind)).as_str(),
                NbtTag::Compound(NbtCompound::new()),
            ),
            Some(v) => components.insert(
                key(kind).as_str(),
                component_tag(data, kind, || serde_json::to_value(&v), registries),
            ),
        }
    }
    if !components.is_empty() {
        out.insert("components", NbtTag::Compound(components));
    }
    out
}

fn error(s: impl Into<String>) -> Vec<Span> {
    vec![Span::colored(s, RED)]
}

fn lead() -> Span {
    Span::colored("NBT ", GOLD)
}

pub fn run(bot: &Client, edit: ItemEdit, registries: Option<&RegistryHolder>) -> Vec<Vec<Span>> {
    let Ok(inv) = bot.component::<azalea::entity::inventory::Inventory>() else {
        return vec![error(crate::lang::tr!("play.nbt.no_inventory"))];
    };
    let selected = inv.selected_hotbar_slot;
    let held = inv.held_item().clone();
    drop(inv);

    if matches!(edit, ItemEdit::Get | ItemEdit::Copy) {
        let tag = match held.as_present() {
            Some(data) => stack_tag(data, registries),
            None => NbtCompound::new(),
        };
        let mut spans = vec![lead()];
        spans.extend(snbt::compound_to_spans(&tag));
        if matches!(edit, ItemEdit::Copy) {
            crate::platform::clipboard::set(&snbt::compound_to_snbt(&tag));
            spans.push(Span::colored(crate::lang::tr!("play.nbt.copied"), WHITE));
        }
        return vec![spans];
    }

    if !bot
        .component::<azalea::entity::PlayerAbilities>()
        .is_ok_and(|a| a.instant_break)
    {
        return vec![error(crate::lang::tr!("play.nbt.creative_only"))];
    }
    if held.is_empty() {
        return vec![error(crate::lang::tr!("play.nbt.hold_item"))];
    }

    let mut notes = Vec::new();
    let edited = match edit {
        ItemEdit::Get | ItemEdit::Copy => unreachable!("answered above"),
        ItemEdit::Add(entries) => apply(held, &entries, registries),
        ItemEdit::Set(entries) => {
            let mut stack = held;
            if let Some(data) = stack.as_present_mut() {
                let kinds: Vec<DataComponentKind> =
                    data.component_patch.iter().map(|(k, _)| k).collect();
                for k in kinds {
                    data.component_patch.clear_kind(k);
                }
            }
            apply(stack, &entries, registries)
        }
        ItemEdit::Remove(kind) => {
            let mut stack = held;
            if let Some(data) = stack.as_present_mut() {
                remove_one(data, kind);
            }
            Ok(stack)
        }
        ItemEdit::Count(n) => {
            let mut stack = held;
            if let Some(data) = stack.as_present_mut() {
                data.count = n;
            }
            notes.push(vec![Span::colored(
                crate::lang::trf!("play.nbt.count_set", n),
                WHITE,
            )]);
            Ok(stack)
        }
    };
    let stack = match edited {
        Ok(s) => s,
        Err(e) => return vec![error(e)],
    };
    if let Some(data) = stack.as_present() {
        let max = azalea_inventory::bundle::max_stack_size(data);
        if data.count > max {
            notes.push(vec![Span::colored(
                crate::lang::trf!("play.nbt.over_max", data.count, max),
                RED,
            )]);
        }
    }
    crate::play::inventory_bridge::set_creative_slot(bot, 36 + u16::from(selected), stack);
    if notes.is_empty() {
        notes.push(vec![Span::colored(
            crate::lang::tr!("play.nbt.updated"),
            GREEN,
        )]);
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_list_reads_assignments_and_removals() {
        let list =
            parse_components(" [ custom_name = 'Red' , !lore,minecraft:damage=3 ] ").unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].kind, DataComponentKind::CustomName);
        assert!(matches!(&list[0].value, Some(NbtTag::String(s)) if s.to_str() == "Red"));
        assert!(list[1].kind == DataComponentKind::Lore && list[1].value.is_none());
        assert!(matches!(list[2].value, Some(NbtTag::Int(3))));
        assert!(parse_components("[]").unwrap().is_empty());
    }

    #[test]
    fn component_list_refuses_what_item_parser_refuses() {
        assert!(parse_components("custom_name='x'").is_err());
        assert!(parse_components("[damage=1,damage=2]").is_err());
        assert!(parse_components("[damage=1,]").is_err());
        assert!(parse_components("[damage 1]").is_err());
        assert!(parse_components("[not_a_component=1]").is_err());
        assert!(parse_components("[damage=1] x").is_err());
    }

    #[test]
    fn mixed_lists_wrap_and_unwrap() {
        let list = list_of(vec![NbtTag::Int(1), NbtTag::String("a".into())]);
        let NbtList::Compound(items) = &list else {
            panic!("not wrapped: {list:?}")
        };
        assert!(matches!(
            unwrapped(&NbtTag::Compound(items[0].clone())),
            NbtTag::Int(1)
        ));
        assert!(matches!(
            list_of(vec![NbtTag::Int(1), NbtTag::Int(2)]),
            NbtList::Int(_)
        ));
    }

    #[test]
    fn ints_follow_codec_int() {
        assert_eq!(int(&NbtTag::Byte(5), "x"), Ok(5));
        assert_eq!(int(&NbtTag::Double(2.0), "x"), Ok(2));
        assert!(int(&NbtTag::Double(2.5), "x").is_err());
        assert!(int(&NbtTag::Long(i64::MAX), "x").is_err());
        assert!(int_in(&NbtTag::Int(100), "x", 1..=99).is_err());
    }
}
