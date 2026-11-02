use azalea_chat::{
    FormattedText,
    style::{Style, TextColor},
    text_component::TextComponent,
};
use azalea_core::{
    attribute_modifier_operation::AttributeModifierOperation,
    checksum::get_checksum,
    position::{BlockPos, GlobalPos},
};
use azalea_inventory::{
    ItemStack,
    components::{
        AdventureModePredicate, AttributeModifier, AttributeModifierDisplay, AttributeModifiers,
        AttributeModifiersEntry, BlockPredicate, CanPlaceOn, ChargedProjectiles, CustomData,
        CustomName, EquipmentSlotGroup, Glider, JukeboxPlayable, LodestoneTracker, Lore, MapColor,
        PotDecorations, Rarity,
    },
};
use azalea_registry::builtin::{Attribute, BlockKind, ItemKind};
use simdnbt::owned::{BaseNbt, Nbt, NbtCompound, NbtList, NbtTag};

#[test]
fn test_custom_name_checksum() {
    let c = CustomName {
        name: FormattedText::from("meow"),
    };
    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 2222287064);
}
#[test]
fn test_custom_name_checksum_2() {
    let c = CustomName {
        name: TextComponent::new("meow")
            .with_style(
                Style::new()
                    .color(Some(TextColor::parse("red").unwrap()))
                    .underlined(true),
            )
            .into(),
    };

    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 187682122);
}
#[test]
fn test_map_color_checksum() {
    let c = MapColor { color: 1 };
    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 1565579036);
}
#[test]
fn test_lore_checksum() {
    let c = Lore {
        lines: vec!["first".into(), "second".into()],
    };
    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 1545409323);
}
#[test]
fn test_rarity_checksum() {
    let c = Rarity::Rare;
    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 2874400570);
}
#[test]
fn test_can_place_on_checksum() {
    let c = CanPlaceOn {
        predicate: AdventureModePredicate {
            predicates: vec![BlockPredicate {
                blocks: Some(vec![BlockKind::GrassBlock].into()),
                properties: None,
                nbt: None,
                components: Default::default(),
            }],
        },
    };

    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 227436005);
}

#[test]
fn test_custom_data_nbt() {
    let c = CustomData {
        nbt: Nbt::Some(BaseNbt::new(
            "",
            NbtCompound::from_values(vec![
                ("meow".into(), "mrrp".into()),
                (
                    "nya".into(),
                    NbtList::Compound(vec![
                        NbtTag::Int(1).into(),
                        NbtTag::Int(2).into(),
                        NbtCompound::new(),
                        NbtCompound::from_values(vec![("data".into(), NbtTag::Byte(1))]),
                    ])
                    .into(),
                ),
            ]),
        )),
    };
    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 1035780974);
}
#[test]
fn test_attribute_modifiers_checksum() {
    let c = AttributeModifiers {
        modifiers: vec![AttributeModifiersEntry {
            kind: Attribute::Scale,
            modifier: AttributeModifier {
                id: "example:grow".into(),
                amount: 4.0,
                operation: AttributeModifierOperation::AddMultipliedBase,
            },
            slot: EquipmentSlotGroup::Hand,
            display: AttributeModifierDisplay::Default,
        }],
    };

    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 2501379836);
}

#[test]
fn test_firework_explosion_checksum() {
    let c = AttributeModifiers {
        modifiers: vec![AttributeModifiersEntry {
            kind: Attribute::Scale,
            modifier: AttributeModifier {
                id: "example:grow".into(),
                amount: 4.0,
                operation: AttributeModifierOperation::AddMultipliedBase,
            },
            slot: EquipmentSlotGroup::Hand,
            display: AttributeModifierDisplay::Default,
        }],
    };

    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 2501379836);
}

#[test]
fn test_charged_projectile_checksum() {
    let c = ChargedProjectiles {
        items: vec![ItemStack::from(ItemKind::MusicDiscCat)],
    };

    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 3435761017);
}

#[test]
fn test_charged_projectile_with_components_checksum() {
    let c = ChargedProjectiles {
        items: vec![
            ItemStack::from(ItemKind::MusicDiscCat)
                .with_component::<JukeboxPlayable>(None)
                .with_component(ChargedProjectiles {
                    items: vec![ItemStack::from(ItemKind::MusicDiscCat)],
                }),
        ],
    };

    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 170375255);
}

#[test]
fn test_lodestone_tracker_checksum() {
    let c = LodestoneTracker {
        target: Some(GlobalPos {
            dimension: "meow".into(),
            pos: BlockPos::new(1, 2, 3),
        }),
        tracked: true,
    };

    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 4138292505);
}

#[test]
fn test_pot_decorations_checksum() {
    let c = PotDecorations {
        items: vec![
            ItemKind::Stick,
            ItemKind::Brick,
            ItemKind::Brick,
            ItemKind::Brick,
        ],
    };

    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 1951715383);
}

#[test]
fn test_glider_checksum() {
    let c = Glider;
    assert_eq!(get_checksum(&c, &Default::default()).unwrap().0, 3312760008);
}

#[test]
fn test_container_is_optional_templates() {
    use std::io::Cursor;

    use azalea_buf::AzBuf;
    use azalea_inventory::components::Container;
    use azalea_registry::Registry;

    let c = Container {
        items: vec![ItemStack::Empty, ItemStack::from((ItemKind::Stone, 5))],
    };
    let mut bytes = Vec::new();
    c.azalea_write(&mut bytes).unwrap();
    let stone = ItemKind::Stone.to_u32() as u8;
    assert_eq!(bytes, [2, 0, 1, stone, 5, 0, 0]);
    assert_eq!(Container::azalea_read(&mut Cursor::new(&bytes[..])).unwrap(), c);
}

#[test]
fn test_equippable_reads_slot_ids_and_inline_sounds() {
    use std::io::Cursor;

    use azalea_buf::{AzBuf, AzBufVar};
    use azalea_core::sound::CustomSound;
    use azalea_inventory::components::{EquipmentSlot, Equippable};
    use azalea_registry::{Holder, Registry, builtin::SoundEvent};

    let mut bytes = Vec::new();
    1u32.azalea_write_var(&mut bytes).unwrap();
    0u32.azalea_write_var(&mut bytes).unwrap();
    "custom:clank".to_owned().azalea_write(&mut bytes).unwrap();
    bytes.push(0);
    bytes.push(0);
    bytes.push(0);
    bytes.push(0);
    bytes.extend_from_slice(&[1, 1, 1, 0, 0]);
    (SoundEvent::ItemShearsSnip.to_u32() + 1).azalea_write_var(&mut bytes).unwrap();

    let mut cursor = Cursor::new(&bytes[..]);
    let e = Equippable::azalea_read(&mut cursor).unwrap();
    assert_eq!(cursor.position() as usize, bytes.len());
    assert_eq!(e.slot, EquipmentSlot::Feet);
    assert_eq!(
        e.equip_sound,
        Holder::Direct(CustomSound {
            sound_id: "custom:clank".into(),
            range: None,
        })
    );
    assert_eq!(e.shearing_sound, Holder::Reference(SoundEvent::ItemShearsSnip));

    let mut back = Vec::new();
    e.azalea_write(&mut back).unwrap();
    assert_eq!(back, bytes);

    let mut offhand = bytes.clone();
    offhand[0] = 5;
    assert_eq!(
        Equippable::azalea_read(&mut Cursor::new(&offhand[..])).unwrap().slot,
        EquipmentSlot::Offhand
    );
}

#[test]
fn test_trim_is_two_holders() {
    use std::io::Cursor;

    use azalea_buf::{AzBuf, AzBufVar};
    use azalea_inventory::components::Trim;
    use azalea_registry::{
        DataRegistry, Holder, Registry,
        data::{TrimMaterial, TrimPattern},
    };

    let material = TrimMaterial::new_raw(1);
    let pattern = TrimPattern::new_raw(2);
    let mut bytes = Vec::new();
    (material.to_u32() + 1).azalea_write_var(&mut bytes).unwrap();
    (pattern.to_u32() + 1).azalea_write_var(&mut bytes).unwrap();

    let mut cursor = Cursor::new(&bytes[..]);
    let t = Trim::azalea_read(&mut cursor).unwrap();
    assert_eq!(cursor.position() as usize, bytes.len());
    assert_eq!(t.material, Holder::Reference(material));
    assert_eq!(t.pattern, Holder::Reference(pattern));
}

#[test]
fn test_item_use_animation_matches_vanilla_order() {
    use std::io::Cursor;

    use azalea_buf::AzBuf;
    use azalea_inventory::components::ItemUseAnimation;

    let read = |id: u8| ItemUseAnimation::azalea_read(&mut Cursor::new(&[id][..])).unwrap();
    assert_eq!(read(3), ItemUseAnimation::BlockKind);
    assert_eq!(read(5), ItemUseAnimation::Trident);
    assert_eq!(read(10), ItemUseAnimation::Bundle);
    assert_eq!(read(11), ItemUseAnimation::Spear);
}

#[test]
fn test_intangible_projectile_is_an_empty_compound() {
    use std::io::Cursor;

    use azalea_buf::AzBuf;
    use azalea_inventory::components::IntangibleProjectile;

    let bytes = [0x0a, 0x00];
    let mut cursor = Cursor::new(&bytes[..]);
    let c = IntangibleProjectile::azalea_read(&mut cursor).unwrap();
    assert_eq!(cursor.position(), 2);
    let mut back = Vec::new();
    c.azalea_write(&mut back).unwrap();
    assert_eq!(back, bytes);
}
