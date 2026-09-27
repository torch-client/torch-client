use serde::Serialize;
#[cfg(feature = "simdnbt")]
use simdnbt::owned::NbtCompound;

use crate::FormattedText;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum HoverEvent {
    ShowText {
        value: Box<FormattedText>,
    },
    ShowItem {
        #[cfg(feature = "simdnbt")]
        item: NbtCompound,
    },
    ShowEntity {
        id: String,
        uuid: Option<String>,
        name: Option<Box<FormattedText>>,
    },
}

#[cfg(feature = "simdnbt")]
impl simdnbt::Serialize for HoverEvent {
    fn to_compound(self) -> NbtCompound {
        let mut compound = NbtCompound::new();
        let mut action = |s: &str| {
            compound.insert("action", s);
        };
        match self {
            HoverEvent::ShowText { value } => {
                action("show_text");
                compound.insert("value", value.to_compound());
            }
            HoverEvent::ShowItem { item } => {
                action("show_item");
                compound.extend(item);
            }
            HoverEvent::ShowEntity { id, uuid, name } => {
                action("show_entity");
                compound.insert("id", id);
                if let Some(uuid) = uuid {
                    compound.insert("uuid", uuid);
                }
                if let Some(name) = name {
                    compound.insert("name", name.to_compound());
                }
            }
        }
        compound
    }
}

#[cfg(feature = "simdnbt")]
impl simdnbt::Deserialize for HoverEvent {
    fn from_compound(
        compound: simdnbt::borrow::NbtCompound,
    ) -> Result<Self, simdnbt::DeserializeError> {
        use simdnbt::{DeserializeError, FromNbtTag};

        use crate::get_in_compound;

        let action = get_in_compound::<String>(&compound, "action")?;
        let component = |key: &str| {
            compound
                .get(key)
                .and_then(FormattedText::from_nbt_tag)
                .map(Box::new)
        };
        Ok(match action.as_str() {
            "show_text" => HoverEvent::ShowText {
                value: component("value").ok_or(DeserializeError::MissingField)?,
            },
            "show_item" => HoverEvent::ShowItem {
                item: compound.to_owned(),
            },
            "show_entity" => HoverEvent::ShowEntity {
                id: get_in_compound(&compound, "id")?,
                uuid: compound.get("uuid").and_then(uuid_string),
                name: component("name"),
            },
            _ => return Err(DeserializeError::MismatchedFieldType(action.to_owned())),
        })
    }
}

#[cfg(feature = "simdnbt")]
fn uuid_string(tag: simdnbt::borrow::NbtTag) -> Option<String> {
    if let Some(string) = tag.string() {
        return Some(string.to_string());
    }
    let ints = tag.int_array()?;
    if ints.len() != 4 {
        return None;
    }
    let most = ((ints[0] as u32 as u64) << 32) | ints[1] as u32 as u64;
    let least = ((ints[2] as u32 as u64) << 32) | ints[3] as u32 as u64;
    Some(format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        most >> 32,
        (most >> 16) & 0xFFFF,
        most & 0xFFFF,
        least >> 48,
        least & 0xFFFF_FFFF_FFFF
    ))
}
