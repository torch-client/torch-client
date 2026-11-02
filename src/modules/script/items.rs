use std::io::Cursor;
use std::str::FromStr;

use azalea::Client;
use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::components::{self as comp, DataComponentUnion};
use azalea_inventory::{ItemStack, ItemStackData};
use azalea_registry::builtin::DataComponentKind;
use mlua::{Error, Lua, Table, Value};

#[derive(Clone, Copy)]
pub(super) enum Slot {
    Inventory(u8),
    Offhand,
}

impl Slot {
    pub fn parse(v: &Value, what: &str) -> mlua::Result<Slot> {
        match v {
            Value::String(s) if s.to_string_lossy() == "offhand" => Ok(Slot::Offhand),
            Value::Integer(n) if (1..=36).contains(n) => Ok(Slot::Inventory(*n as u8)),
            Value::Number(n) if n.fract() == 0.0 && (1.0..=36.0).contains(n) => {
                Ok(Slot::Inventory(*n as u8))
            }
            _ => Err(Error::runtime(format!(
                "{what}: a slot is 1 to 36 (1 to 9 the hotbar) or \"offhand\""
            ))),
        }
    }

    pub fn player_menu_index(self) -> u16 {
        match self {
            Slot::Inventory(n @ 1..=9) => 36 + n as u16 - 1,
            Slot::Inventory(n) => n as u16 - 1,
            Slot::Offhand => 45,
        }
    }
}

fn live(bot: &Client, slot: Slot) -> Option<ItemStack> {
    let inv = bot
        .component::<azalea::entity::inventory::Inventory>()
        .ok()?;
    match slot {
        Slot::Offhand => inv.inventory_menu.slot(45).cloned(),
        Slot::Inventory(n) => {
            let menu = inv.menu();
            let start = *menu.player_slots_range().start();
            let i = if n <= 9 {
                start + 27 + n as usize - 1
            } else {
                start + n as usize - 10
            };
            menu.slot(i).cloned()
        }
    }
}

pub(super) fn with_registries<R>(bot: &Client, f: impl FnOnce(Option<&RegistryHolder>) -> R) -> R {
    match bot.world() {
        Ok(w) => f(Some(&w.read().registries)),
        Err(_) => f(None),
    }
}

fn key(kind: DataComponentKind) -> String {
    let s = kind.to_string();
    if s.contains(':') {
        s
    } else {
        format!("minecraft:{s}")
    }
}

fn encode(data: &ItemStackData, kind: DataComponentKind) -> Vec<u8> {
    let mut out = Vec::new();
    if let Some(c) = data.component_patch.get_kind(kind) {
        let _ = c.encode(&mut out);
    }
    out
}

fn enchantment_map(
    lua: &Lua,
    registries: Option<&RegistryHolder>,
    levels: &std::collections::HashMap<azalea_registry::data::Enchantment, i32>,
) -> mlua::Result<Value> {
    let t = lua.create_table()?;
    for (e, lvl) in levels {
        let name = crate::play::inventory_bridge::enchantment_name(registries, e);
        let name = if name.starts_with('#') {
            name
        } else {
            format!("minecraft:{name}")
        };
        t.raw_set(name, *lvl)?;
    }
    Ok(Value::Table(t))
}

pub(super) fn read(lua: &Lua, bot: &Client, slot: Slot, raw: bool) -> mlua::Result<Value> {
    let Some(ItemStack::Present(data)) = live(bot, slot) else {
        return Ok(Value::Nil);
    };
    with_registries(bot, |registries| {
        let out = lua.create_table()?;
        for (kind, value) in data.component_patch.serializable() {
            let k = key(kind);
            let Some(value) = value else {
                out.raw_set(k, false)?;
                continue;
            };
            if raw {
                out.raw_set(k, lua.create_string(encode(&data, kind))?)?;
                continue;
            }
            let v = match kind {
                DataComponentKind::Enchantments => {
                    match data.component_patch.get::<comp::Enchantments>() {
                        Some(c) => enchantment_map(lua, registries, &c.levels)?,
                        None => Value::Nil,
                    }
                }
                DataComponentKind::StoredEnchantments => {
                    match data.component_patch.get::<comp::StoredEnchantments>() {
                        Some(c) => enchantment_map(lua, registries, &c.enchantments)?,
                        None => Value::Nil,
                    }
                }
                _ => match serde_json::to_value(&value) {
                    Ok(serde_json::Value::Null) => Value::Boolean(true),
                    Ok(json) => super::json::plain_to_lua(lua, &json)?,
                    Err(_) => {
                        let t = lua.create_table()?;
                        t.raw_set("raw", lua.create_string(encode(&data, kind))?)?;
                        Value::Table(t)
                    }
                },
            };
            out.raw_set(k, v)?;
        }
        Ok(Value::Table(out))
    })
}

const TYPED: [&str; 15] = [
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

fn text(v: &Value, path: &str) -> Result<azalea_chat::FormattedText, String> {
    match v {
        Value::String(s) => Ok(azalea_chat::FormattedText::from(s.to_string_lossy())),
        Value::Table(_) => {
            let json = super::json::plain_to_json(v, path).map_err(|e| e.to_string())?;
            serde_json::from_value(json).map_err(|e| format!("{path} is not a text component: {e}"))
        }
        _ => Err(format!("{path} is a string or a text component table")),
    }
}

fn int(v: &Value, path: &str) -> Result<i32, String> {
    let n = match v {
        Value::Integer(n) => *n as f64,
        Value::Number(n) => *n,
        _ => f64::NAN,
    };
    if n.fract() == 0.0 && n >= f64::from(i32::MIN) && n <= f64::from(i32::MAX) {
        Ok(n as i32)
    } else {
        Err(format!("{path} is a whole number"))
    }
}

fn typed_nbt<K: FromStr>(
    v: &Value,
    path: &str,
    of: &str,
) -> Result<(K, simdnbt::owned::NbtCompound), String> {
    let Value::Table(t) = v else {
        return Err(format!("{path} is a table with an `id`"));
    };
    let id: Option<String> = t
        .raw_get("id")
        .map_err(|_| format!("{path}.id is a string"))?;
    let id = id.ok_or_else(|| format!("{path} needs an `id`, the {of} type"))?;
    let kind = K::from_str(&id).map_err(|_| format!("{path}.id: `{id}` is not a {of} type"))?;
    let mut nbt = super::nbt::lua_to_compound(t, path)?;
    nbt.remove("id");
    Ok((kind, nbt))
}

fn enchantments(
    v: &Value,
    path: &str,
    registries: Option<&RegistryHolder>,
) -> Result<std::collections::HashMap<azalea_registry::data::Enchantment, i32>, String> {
    let Value::Table(t) = v else {
        return Err(format!("{path} is a table of name = level"));
    };
    let mut pairs = Vec::new();
    for pair in t.pairs::<String, Value>() {
        let (name, lvl) = pair.map_err(|_| format!("{path} is a table of name = level"))?;
        let lvl = int(&lvl, &format!("{path}.{name}"))?;
        if !(1..=255).contains(&lvl) {
            return Err(format!("{path}.{name}: a level is 1 to 255"));
        }
        let bare = name.strip_prefix("minecraft:").unwrap_or(&name).to_string();
        pairs.push((bare, lvl));
    }
    let out = crate::play::inventory_bridge::resolve_enchantments(registries, &pairs);
    if out.len() != pairs.len() {
        let known: Vec<String> = out
            .keys()
            .map(|e| crate::play::inventory_bridge::enchantment_name(registries, e))
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

fn typed(
    stack: ItemStack,
    name: &str,
    v: &Value,
    path: &str,
    registries: Option<&RegistryHolder>,
) -> Result<ItemStack, String> {
    Ok(match name {
        "custom_name" => stack.with_component(comp::CustomName {
            name: text(v, path)?,
        }),
        "item_name" => stack.with_component(comp::ItemName {
            name: text(v, path)?,
        }),
        "lore" => {
            let Value::Table(t) = v else {
                return Err(format!("{path} is a list of lines"));
            };
            let mut lines = Vec::new();
            for i in 1..=t.raw_len() {
                let line: Value = t.raw_get(i).map_err(|e| e.to_string())?;
                lines.push(text(&line, &format!("{path}[{i}]"))?);
            }
            if lines.len() > 256 {
                return Err(format!("{path}: at most 256 lines"));
            }
            stack.with_component(comp::Lore { lines })
        }
        "custom_data" => {
            let Value::Table(t) = v else {
                return Err(format!("{path} is a table"));
            };
            let nbt = super::nbt::lua_to_compound(t, path)?;
            stack.with_component(comp::CustomData {
                nbt: simdnbt::owned::Nbt::new("".into(), nbt),
            })
        }
        "block_entity_data" => {
            let (kind, data) =
                typed_nbt::<azalea_registry::builtin::BlockEntityKind>(v, path, "block entity")?;
            stack.with_component(comp::BlockEntityData { kind, data })
        }
        "entity_data" => {
            let (kind, data) =
                typed_nbt::<azalea_registry::builtin::EntityKind>(v, path, "entity")?;
            stack.with_component(comp::EntityData { kind, data })
        }
        "damage" => stack.with_component(comp::Damage {
            amount: int(v, path)?.max(0),
        }),
        "max_damage" => stack.with_component(comp::MaxDamage {
            amount: int(v, path)?.max(1),
        }),
        "max_stack_size" => stack.with_component(comp::MaxStackSize {
            count: int(v, path)?.clamp(1, 99),
        }),
        "repair_cost" => stack.with_component(comp::RepairCost {
            cost: int(v, path)?.max(0) as u32,
        }),
        "unbreakable" => match v {
            Value::Boolean(true) => stack.with_component(comp::Unbreakable),
            _ => return Err(format!("{path} is true (false removes it)")),
        },
        "enchantment_glint_override" => match v {
            Value::Boolean(b) => {
                stack.with_component(comp::EnchantmentGlintOverride { show_glint: *b })
            }
            _ => return Err(format!("{path} is a boolean")),
        },
        "enchantments" => stack.with_component(comp::Enchantments {
            levels: enchantments(v, path, registries)?,
        }),
        "stored_enchantments" => stack.with_component(comp::StoredEnchantments {
            enchantments: enchantments(v, path, registries)?,
        }),
        "dyed_color" => stack.with_component(comp::DyedColor {
            rgb: int(v, path)? & 0xFF_FFFF,
        }),
        _ => unreachable!("only names in TYPED reach here"),
    })
}

fn raw_union(
    kind: DataComponentKind,
    bytes: &[u8],
    path: &str,
) -> Result<DataComponentUnion, String> {
    let mut cursor = Cursor::new(bytes);
    let union = DataComponentUnion::azalea_read_as(kind, &mut cursor)
        .map_err(|e| format!("{path}.raw did not parse as {}: {e}", key(kind)))?;
    let left = bytes.len() - cursor.position() as usize;
    if left != 0 {
        let mut union = union;
        unsafe { union.drop_as(kind) };
        return Err(format!(
            "{path}.raw has {left} bytes past the component's end"
        ));
    }
    Ok(union)
}

pub(super) fn apply(
    mut stack: ItemStack,
    spec: &Table,
    what: &str,
    registries: Option<&RegistryHolder>,
) -> mlua::Result<ItemStack> {
    let fail = |e: String| Error::runtime(format!("{what}: {e}"));
    for pair in spec.pairs::<String, Value>() {
        let (name, v) =
            pair.map_err(|_| fail("components is a table keyed by component name".into()))?;
        let Ok(kind) = DataComponentKind::from_str(&name) else {
            return Err(fail(format!("`{name}` is not a data component")));
        };
        let bare = name.strip_prefix("minecraft:").unwrap_or(&name);
        let path = format!("components.{bare}");
        let raw = match &v {
            Value::Table(t) => match t.raw_get::<Value>("raw")? {
                Value::String(s) => Some(s.as_bytes().to_vec()),
                Value::Nil => None,
                _ => return Err(fail(format!("{path}.raw is a string of bytes"))),
            },
            _ => None,
        };
        let Some(data) = stack.as_present_mut() else {
            return Ok(stack);
        };
        if let Some(bytes) = raw {
            let union = raw_union(kind, &bytes, &path).map_err(fail)?;
            unsafe {
                data.component_patch
                    .unchecked_insert_component(kind, Some(union))
            };
            continue;
        }
        match &v {
            Value::Boolean(false) => unsafe {
                data.component_patch.unchecked_insert_component(kind, None)
            },
            _ if TYPED.contains(&bare) => {
                stack = typed(stack, bare, &v, &path, registries).map_err(fail)?
            }
            Value::Boolean(true) => match raw_union(kind, &[], &path) {
                Ok(union) => unsafe {
                    data.component_patch
                        .unchecked_insert_component(kind, Some(union))
                },
                Err(_) => {
                    return Err(fail(format!(
                        "{path} needs a value; give it as {{ raw = ... }}"
                    )));
                }
            },
            _ => {
                return Err(fail(format!(
                    "{path}: written as a value only for {}; any component takes {{ raw = <bytes> }}, \
                     which torch.inventory.components(slot, {{ raw = true }}) reads",
                    TYPED.join(", ")
                )));
            }
        }
    }
    Ok(stack)
}
