use azalea_core::direction::Direction;
use simdnbt::owned::{Nbt, NbtCompound, NbtTag};

use crate::blocks::facing;
use crate::items::potions::{self, PotionContents};
use crate::renderer::anim::HeldItem;

#[derive(Clone, Debug, PartialEq)]
pub struct Container<const N: usize> {
    pub items: [HeldItem; N],
}

impl<const N: usize> Default for Container<N> {
    fn default() -> Self {
        Container {
            items: std::array::from_fn(|_| HeldItem::default()),
        }
    }
}

impl<const N: usize> Container<N> {
    fn load(tag: &NbtCompound) -> Self {
        let mut out = Self::default();
        let entries = tag.list("Items").and_then(|list| list.compounds());
        for entry in entries.unwrap_or_default() {
            let slot = entry.byte("Slot").unwrap_or(-1) as u8 as usize;
            if let Some(held) = out.items.get_mut(slot) {
                *held = stack(entry);
            }
        }
        out
    }
}

pub type Campfire = Container<4>;

pub fn campfire(tag: &Nbt) -> Campfire {
    Container::load(tag)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shelf {
    pub slots: Container<3>,
    pub align_to_bottom: bool,
}

pub fn shelf(tag: &Nbt) -> Shelf {
    Shelf {
        slots: Container::load(tag),
        align_to_bottom: tag.byte("align_items_to_bottom").unwrap_or(0) != 0,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Brushable {
    pub item: HeldItem,
    pub hit_direction: Option<Direction>,
}

pub fn brushable(tag: &Nbt) -> Brushable {
    Brushable {
        item: tag.compound("item").map(stack).unwrap_or_default(),
        hit_direction: tag
            .byte("hit_direction")
            .map(|value| facing::from_3d_data_value(value.into())),
    }
}

pub fn stack(tag: &NbtCompound) -> HeldItem {
    let Some(id) = tag.string("id") else {
        return HeldItem::default();
    };
    let id = id.to_str();
    let id = id.strip_prefix("minecraft:").unwrap_or(&id);
    let Some(def) = crate::generated_items::item(id).filter(|def| def.id != "air") else {
        return HeldItem::default();
    };
    let count = tag
        .int("count")
        .or_else(|| tag.byte("count").map(i32::from))
        .unwrap_or(1);
    let components = tag.compound("components");
    let projectiles = components
        .and_then(|c| c.list("minecraft:charged_projectiles"))
        .and_then(|list| list.compounds());
    HeldItem {
        id: def.id,
        count: count.clamp(1, u8::MAX as i32) as u8,
        tint: potions::is_potion_item(def.id).then(|| potion_color(components)),
        map_id: components.and_then(|c| c.int("minecraft:map_id")),
        charged: projectiles.is_some_and(|p| !p.is_empty()),
        rocket: projectiles.is_some_and(|p| {
            p.iter().any(|s| {
                s.string("id").is_some_and(|id| {
                    id.to_str().trim_start_matches("minecraft:") == "firework_rocket"
                })
            })
        }),
        glint: crate::items::glint::codec_foil(def.id, components),
        trim: components
            .and_then(|c| c.compound("minecraft:trim"))
            .and_then(|trim| trim.string("material"))
            .map_or(0, |m| {
                crate::items::dispatch::trim_index(&bare(&m.to_str()))
            }),
        ..HeldItem::default()
    }
}

fn potion_color(components: Option<&NbtCompound>) -> u32 {
    let contents = match components.and_then(|c| c.get("minecraft:potion_contents")) {
        Some(NbtTag::String(potion)) => PotionContents {
            potion: Some(bare(&potion.to_str())),
            ..PotionContents::default()
        },
        Some(NbtTag::Compound(contents)) => PotionContents {
            potion: contents.string("potion").map(|p| bare(&p.to_str())),
            custom_color: contents.int("custom_color").map(|c| c as u32),
            ..PotionContents::default()
        },
        _ => PotionContents::default(),
    };
    contents.color()
}

fn bare(id: &str) -> String {
    id.strip_prefix("minecraft:").unwrap_or(id).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockentities::sim::test_nbt::{compound, item, root, string};

    fn slotted(slot: i8, id: &str) -> NbtCompound {
        let mut values = item(id, 1);
        values.push(("Slot", NbtTag::Byte(slot)));
        compound(values)
    }

    fn items(entries: Vec<NbtCompound>) -> NbtTag {
        NbtTag::List(simdnbt::owned::NbtList::Compound(entries))
    }

    #[test]
    fn a_stack_resolves_to_a_static_id_and_count() {
        let held = stack(&compound(item("minecraft:beef", 3)));
        assert_eq!(held.id, "beef");
        assert_eq!(held.count, 3);
        assert!(stack(&compound(item("minecraft:not_an_item", 1))).is_empty());
        assert!(stack(&compound(vec![])).is_empty());
    }

    #[test]
    fn a_stack_without_a_count_holds_one() {
        let held = stack(&compound(vec![("id", string("stick"))]));
        assert_eq!((held.id, held.count), ("stick", 1));
    }

    #[test]
    fn a_filled_map_keeps_its_map_id() {
        let mut values = item("minecraft:filled_map", 1);
        values.push((
            "components",
            NbtTag::Compound(compound(vec![("minecraft:map_id", NbtTag::Int(7))])),
        ));
        assert_eq!(stack(&compound(values)).map_id, Some(7));
    }

    #[test]
    fn campfire_slots_follow_the_slot_byte() {
        let tag = root(vec![(
            "Items",
            items(vec![
                slotted(2, "minecraft:beef"),
                slotted(9, "minecraft:stick"),
            ]),
        )]);
        let fire = campfire(&tag);
        assert_eq!(fire.items[2].id, "beef");
        assert!(
            fire.items
                .iter()
                .enumerate()
                .all(|(i, h)| i == 2 || h.is_empty())
        );
    }

    #[test]
    fn shelf_reads_its_alignment() {
        let tag = root(vec![
            ("Items", items(vec![slotted(0, "minecraft:stick")])),
            ("align_items_to_bottom", NbtTag::Byte(1)),
        ]);
        let shelf = shelf(&tag);
        assert!(shelf.align_to_bottom);
        assert_eq!(shelf.slots.items[0].id, "stick");
    }

    #[test]
    fn brushable_reads_the_item_and_the_face() {
        let tag = root(vec![
            (
                "item",
                NbtTag::Compound(compound(item("minecraft:emerald", 1))),
            ),
            ("hit_direction", NbtTag::Byte(5)),
        ]);
        let brushable = brushable(&tag);
        assert_eq!(brushable.item.id, "emerald");
        assert_eq!(brushable.hit_direction, Some(Direction::East));
        assert_eq!(super::brushable(&root(vec![])), Brushable::default());
    }
}
