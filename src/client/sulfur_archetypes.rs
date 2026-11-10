#![allow(
    dead_code,
    reason = "kept as data for the archetype logic to come; nothing reads it yet"
)]

use azalea_core::registry_holder::float_provider::FloatProvider;
use azalea_core::registry_holder::{RegistryHolder, nbt_entry_as};
use azalea_registry::identifier::Identifier;
use simdnbt::FromNbtTag;

#[derive(Clone, Debug, simdnbt::Deserialize)]
pub struct SulfurCubeArchetype {
    pub items: ItemSet,
    pub attribute_modifiers: Vec<AttributeEntry>,
    pub buoyant: Option<bool>,
    pub explosion: Option<ExplosionData>,
    pub contact_damage: Option<ContactDamage>,
    pub knockback_modifiers: KnockbackModifiers,
    pub sound_settings: SoundSettings,
}

#[derive(Clone, Debug)]
pub enum ItemSet {
    Tag(Identifier),
    Items(Vec<Identifier>),
}

impl FromNbtTag for ItemSet {
    fn from_nbt_tag(tag: simdnbt::borrow::NbtTag) -> Option<Self> {
        if let Some(id) = tag.string() {
            let id = id.to_str();
            return Some(match id.strip_prefix('#') {
                Some(tag) => ItemSet::Tag(Identifier::new(tag)),
                None => ItemSet::Items(vec![Identifier::new(&*id)]),
            });
        }
        let list = tag.list()?;
        let ids = list.strings().unwrap_or_default();
        Some(ItemSet::Items(
            ids.iter()
                .map(|id| Identifier::new(&*id.to_str()))
                .collect(),
        ))
    }
}

#[derive(Clone, Debug, simdnbt::Deserialize)]
pub struct AttributeEntry {
    pub attribute: Identifier,
    pub id: Identifier,
    pub amount: f64,
    pub operation: azalea_core::attribute_modifier_operation::AttributeModifierOperation,
}

#[derive(Clone, Debug, simdnbt::Deserialize)]
pub struct ExplosionData {
    pub power: i32,
    pub causes_fire: bool,
    pub fuse: i32,
}

#[derive(Clone, Debug, simdnbt::Deserialize)]
pub struct ContactDamage {
    pub damage_type: Identifier,
    pub amount: FloatProvider,
    pub attribute_to_source: bool,
}

#[derive(Clone, Debug, simdnbt::Deserialize)]
pub struct KnockbackModifiers {
    pub horizontal_power: f32,
    pub vertical_power: f32,
}

#[derive(Clone, Debug, simdnbt::Deserialize)]
pub struct SoundSettings {
    pub hit_sound: Option<Identifier>,
    pub push_sound: Option<Identifier>,
    pub push_sound_impulse_threshold: f32,
    pub push_sound_cooldown: f32,
}

pub(crate) fn archetypes(registries: &RegistryHolder) -> Vec<(Identifier, SulfurCubeArchetype)> {
    let Some(registry) = registries
        .extra
        .get(&Identifier::new("sulfur_cube_archetype"))
    else {
        return Vec::new();
    };
    registry
        .map
        .iter()
        .filter_map(
            |(id, entry)| match nbt_entry_as::<SulfurCubeArchetype>(entry) {
                Some(archetype) => Some((id.clone(), archetype)),
                None => {
                    crate::log_warn!("registry", "sulfur_cube_archetype {id} did not parse");
                    None
                }
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use simdnbt::owned::NbtCompound;

    use super::*;

    #[test]
    fn the_hot_archetype_parses() {
        let mut modifier = NbtCompound::new();
        modifier.insert("amount", -1.0f64);
        modifier.insert("attribute", "minecraft:knockback_resistance");
        modifier.insert("id", "minecraft:hot_add_knockback_resistance");
        modifier.insert("operation", "add_value");
        let mut contact = NbtCompound::new();
        contact.insert("amount", 1.0f32);
        contact.insert("attribute_to_source", false);
        contact.insert("damage_type", "minecraft:sulfur_cube_hot");
        let mut knockback = NbtCompound::new();
        knockback.insert("horizontal_power", 0.4125f32);
        knockback.insert("vertical_power", 0.09f32);
        let mut sounds = NbtCompound::new();
        sounds.insert("hit_sound", "minecraft:entity.sulfur_cube.hot.hit");
        sounds.insert("push_sound", "minecraft:entity.sulfur_cube.hot.push");
        sounds.insert("push_sound_cooldown", 0.7f32);
        sounds.insert("push_sound_impulse_threshold", 0.2f32);
        let mut entry = NbtCompound::new();
        entry.insert("attribute_modifiers", vec![modifier]);
        entry.insert("buoyant", true);
        entry.insert("contact_damage", contact);
        entry.insert("items", "#minecraft:sulfur_cube_archetype/hot");
        entry.insert("knockback_modifiers", knockback);
        entry.insert("sound_settings", sounds);

        let hot = nbt_entry_as::<SulfurCubeArchetype>(&entry).expect("parses");
        assert!(matches!(&hot.items, ItemSet::Tag(t) if t.path() == "sulfur_cube_archetype/hot"));
        assert_eq!(hot.attribute_modifiers.len(), 1);
        assert_eq!(hot.attribute_modifiers[0].amount, -1.0);
        assert_eq!(hot.buoyant, Some(true));
        assert!(hot.explosion.is_none());
        let contact = hot.contact_damage.expect("contact damage");
        assert!(matches!(contact.amount, FloatProvider::Constant(a) if a == 1.0));
        assert_eq!(hot.knockback_modifiers.horizontal_power, 0.4125);
        assert_eq!(hot.sound_settings.push_sound_cooldown, 0.7);
        assert!(hot.sound_settings.hit_sound.is_some());
    }
}
