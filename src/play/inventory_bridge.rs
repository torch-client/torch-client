use crate::client::chat_text;
use crate::items::potions;
use crate::session::{
    Book, BundleView, ContainerKind, CreativeAnswer, FurnaceKind, HorseMenu, InvAction, SlotStack,
    StackExtra,
};
use azalea::inventory::ContainerClickEvent;
use azalea::prelude::*;
use azalea_inventory::{ItemStack, Menu};

pub(crate) fn container_kind(menu: &Menu) -> ContainerKind {
    match menu {
        Menu::Generic9x1 { .. } => ContainerKind::Chest(1),
        Menu::Generic9x2 { .. } => ContainerKind::Chest(2),
        Menu::Generic9x3 { .. } => ContainerKind::Chest(3),
        Menu::Generic9x4 { .. } => ContainerKind::Chest(4),
        Menu::Generic9x5 { .. } => ContainerKind::Chest(5),
        Menu::Generic9x6 { .. } => ContainerKind::Chest(6),
        Menu::Crafting { .. } => ContainerKind::Crafting,
        Menu::Furnace { .. } => ContainerKind::Furnace(FurnaceKind::Furnace),
        Menu::BlastFurnace { .. } => ContainerKind::Furnace(FurnaceKind::BlastFurnace),
        Menu::Smoker { .. } => ContainerKind::Furnace(FurnaceKind::Smoker),
        Menu::Hopper { .. } => ContainerKind::Hopper,
        Menu::ShulkerBox { .. } => ContainerKind::ShulkerBox,
        Menu::Generic3x3 { .. } => ContainerKind::Dispenser,
        Menu::Grindstone { .. } => ContainerKind::Grindstone,
        Menu::Enchantment { .. } => ContainerKind::Enchantment,
        Menu::Loom { .. } => ContainerKind::Loom,
        Menu::Stonecutter { .. } => ContainerKind::Stonecutter,
        Menu::CartographyTable { .. } => ContainerKind::Cartography,
        Menu::Smithing { .. } => ContainerKind::Smithing,
        Menu::Beacon { .. } => ContainerKind::Beacon,
        Menu::BrewingStand { .. } => ContainerKind::BrewingStand,
        Menu::Merchant { .. } => ContainerKind::Merchant,
        Menu::Lectern { .. } => ContainerKind::Lectern,
        Menu::Anvil { .. } => ContainerKind::Anvil,
        Menu::Crafter3x3 { .. } => ContainerKind::Crafter,
        Menu::Player(_) => ContainerKind::None,
    }
}

pub(crate) fn slot_stack(
    stack: &ItemStack,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
    advanced_tooltips: bool,
) -> SlotStack {
    use azalea_buf::AzBuf;
    use azalea_inventory::components as comp;

    let ItemStack::Present(data) = stack else {
        return SlotStack::default();
    };

    let get = |f: fn(&azalea_inventory::ItemStackData) -> Option<i32>| f(data).unwrap_or(0);
    let damage = get(|d| d.get_component::<comp::Damage>().map(|c| c.amount)) as u16;
    let max_damage = get(|d| d.get_component::<comp::MaxDamage>().map(|c| c.amount)) as u16;

    let custom_name = data
        .get_component::<comp::CustomName>()
        .map(|c| chat_text::to_spans(&c.name));
    let lore = data
        .get_component::<comp::Lore>()
        .map(|c| c.lines.iter().map(chat_text::to_spans).collect())
        .unwrap_or_default();
    let name_for = |e: &azalea_registry::data::Enchantment| enchantment_name(registries, e);
    let mut enchantments: Vec<(String, i32)> = data
        .get_component::<comp::StoredEnchantments>()
        .map(|c| {
            c.enchantments
                .iter()
                .map(|(e, lvl)| (name_for(e), *lvl))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    enchantments.extend(
        data.get_component::<comp::Enchantments>()
            .map(|c| {
                c.levels
                    .iter()
                    .map(|(e, lvl)| (name_for(e), *lvl))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    );
    enchantments.sort();

    let nbt_bytes = if advanced_tooltips {
        let mut buf = Vec::new();
        let _ = stack.azalea_write(&mut buf);
        buf.len()
    } else {
        0
    };

    let item = data.kind.to_str().trim_start_matches("minecraft:");
    let (has_lodestone, lodestone) = crate::items::dispatch::lodestone_of(data);
    let cooldown = crate::client::cooldowns::stack_percent(data);

    SlotStack {
        item,
        count: data.count.clamp(0, 255) as u8,
        cooldown,
        damage,
        max_damage,
        extra: StackExtra {
            custom_name,
            lore,
            enchantments,
            potion: potion_contents(data),
            stew_effects: data
                .get_component::<comp::SuspiciousStewEffects>()
                .map(|c| {
                    c.effects
                        .iter()
                        .map(|e| {
                            let id = e.effect.to_str().trim_start_matches("minecraft:");
                            (id.to_string(), e.duration)
                        })
                        .collect()
                }),
            ominous_amplifier: data
                .get_component::<comp::OminousBottleAmplifier>()
                .map(|c| c.amplifier),
            banner_patterns: banner_patterns(data, registries),
            banner_layers: banner_layers(data, registries),
            book: book(data).map(Box::new),
        }
        .into(),
        unbreakable: data.get_component::<comp::Unbreakable>().is_some(),
        glint: crate::items::glint::has_foil(data),
        charged: data
            .get_component::<comp::ChargedProjectiles>()
            .is_some_and(|c| !c.items.is_empty()),
        rocket: charged_rocket(data),
        trim: crate::items::dispatch::trim_of(data),
        has_lodestone,
        lodestone,
        death_protection: death_protection(data),
        kinetic: crate::items::kinetic::kinetic(data),
        swing_ticks: crate::items::kinetic::swing_ticks(data),
        map_id: data.get_component::<comp::MapId>().map(|m| m.id),
        component_count: data.component_patch.iter().count() as u16,
        nbt_bytes,
        bundle: bundle_view(data, registries),
        raw: (data.component_patch.iter().next().is_some()).then(|| {
            let mut data = data.clone();
            data.count = 1;
            std::sync::Arc::new(ItemStack::Present(data))
        }),
    }
}

fn bundle_view(
    data: &azalea_inventory::ItemStackData,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
) -> Option<std::sync::Arc<BundleView>> {
    use azalea_inventory::bundle::{Fraction, NO_SELECTED_ITEM, contents_weight, is_bundle};
    use azalea_inventory::components::{BundleContents, DataComponentTrait, TooltipDisplay};

    if !is_bundle(data.kind) {
        return None;
    }
    let contents = data.get_unless_removed::<BundleContents>();
    let shows = contents.is_some()
        && data
            .get_unless_removed::<TooltipDisplay>()
            .is_none_or(|display| {
                !display.hide_tooltip && !display.hidden_components.contains(&BundleContents::KIND)
            });
    let (items, selected, weight) = match &contents {
        Some(contents) => (
            contents
                .items
                .iter()
                .map(|item| slot_stack(item, registries, false))
                .collect(),
            contents.selected_item,
            contents_weight(contents),
        ),
        None => (Vec::new(), NO_SELECTED_ITEM, Some(Fraction::ZERO)),
    };
    Some(std::sync::Arc::new(BundleView {
        items,
        selected,
        weight,
        shows,
    }))
}

pub(crate) fn slot_stack_nbt(item: &simdnbt::owned::NbtCompound) -> Option<SlotStack> {
    use crate::items::model::strip_namespace;
    use simdnbt::owned::{NbtCompound, NbtList, NbtTag};

    fn component<'a>(components: Option<&'a NbtCompound>, key: &str) -> Option<&'a NbtTag> {
        let components = components?;
        components
            .get(&format!("minecraft:{key}"))
            .or_else(|| components.get(key))
    }

    let raw = item.string("id")?.to_str();
    let id = crate::generated_items::item(strip_namespace(&raw))?.id;
    let components = item.compound("components");
    let spans = |tag: Option<&NbtTag>| -> Option<Vec<crate::text::Span>> {
        let text = chat_text::from_nbt_tag(tag?)?;
        Some(chat_text::to_spans(&text))
    };
    let int = |key: &str| component(components, key).and_then(NbtTag::int);

    let mut enchantments: Vec<(String, i32)> = ["enchantments", "stored_enchantments"]
        .iter()
        .filter_map(|key| component(components, key)?.compound())
        .flat_map(|map| {
            map.iter()
                .filter_map(|(name, level)| {
                    Some((strip_namespace(&name.to_string()).to_string(), level.int()?))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    enchantments.sort();

    Some(SlotStack {
        item: id,
        count: item.int("count").unwrap_or(1).clamp(1, 99) as u8,
        damage: int("damage").unwrap_or(0).clamp(0, u16::MAX as i32) as u16,
        max_damage: int("max_damage").unwrap_or(0).clamp(0, u16::MAX as i32) as u16,
        extra: StackExtra {
            custom_name: spans(component(components, "custom_name")),
            lore: match component(components, "lore") {
                Some(NbtTag::List(NbtList::String(lines))) => lines
                    .iter()
                    .filter_map(|line| spans(Some(&NbtTag::String(line.clone()))))
                    .collect(),
                Some(NbtTag::List(NbtList::Compound(lines))) => lines
                    .iter()
                    .filter_map(|line| spans(Some(&NbtTag::Compound(line.clone()))))
                    .collect(),
                _ => Vec::new(),
            },
            enchantments,
            ..StackExtra::default()
        }
        .into(),
        glint: crate::items::glint::codec_foil(id, components),
        unbreakable: component(components, "unbreakable").is_some(),
        component_count: components.map(|c| c.len()).unwrap_or(0) as u16,
        trim: component(components, "trim")
            .and_then(NbtTag::compound)
            .and_then(|trim| trim.string("material"))
            .map_or(0, |m| {
                crate::items::dispatch::trim_index(strip_namespace(&m.to_str()))
            }),
        ..SlotStack::default()
    })
}

fn banner_patterns(
    data: &azalea_inventory::ItemStackData,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
) -> Option<Box<[Box<str>]>> {
    use azalea_core::data_registry::DataRegistryWithKey;
    use azalea_inventory::components as comp;

    let component = data.get_component::<comp::ProvidesBannerPatterns>()?;
    let azalea_registry::HolderSet::Direct { contents } = &component.key else {
        return None;
    };
    let registries = registries?;
    let out: Vec<Box<str>> = contents
        .iter()
        .filter_map(|k| {
            let key = k.key_owned(registries)?;
            Some(Box::from(
                azalea_registry::DataRegistryKey::into_ident(key).path(),
            ))
        })
        .collect();
    (!out.is_empty()).then(|| out.into_boxed_slice())
}

pub(crate) fn banner_layers(
    data: &azalea_inventory::ItemStackData,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
) -> Option<Box<[crate::blockentities::banner::BannerLayer]>> {
    use azalea_inventory::components as comp;

    use crate::blockentities::banner::{BannerLayer, MAX_PATTERNS, asset_for};

    let mut out: Vec<BannerLayer> = Vec::new();
    if let Some(base) = data.get_component::<comp::BaseColor>() {
        out.push(BannerLayer {
            asset: Box::from("base"),
            color: base.color as u8,
        });
    }
    if let Some(component) = data.get_component::<comp::BannerPatterns>() {
        use azalea_registry::{DataRegistry, Holder};

        out.extend(
            component
                .patterns
                .iter()
                .take(MAX_PATTERNS)
                .filter_map(|layer| {
                    let asset = match &layer.pattern {
                        Holder::Reference(kind) => {
                            let id = registries?.protocol_id_to_identifier(
                                azalea::Identifier::from("banner_pattern"),
                                kind.protocol_id(),
                            )?;
                            asset_for(id.path())
                        }
                        Holder::Direct(pattern) => Box::from(pattern.asset_id.path()),
                    };
                    Some(BannerLayer {
                        asset,
                        color: layer.color.clamp(0, 15) as u8,
                    })
                }),
        );
    }
    (!out.is_empty()).then(|| out.into_boxed_slice())
}

fn book(data: &azalea_inventory::ItemStackData) -> Option<Book> {
    use azalea_inventory::components as comp;

    if let Some(written) = data.get_component::<comp::WrittenBookContent>() {
        return Some(Book {
            pages: written
                .pages
                .iter()
                .map(|p| chat_text::to_spans(&p.raw))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            title: written.title.raw.clone(),
            author: written.author.clone(),
            generation: written.generation.clamp(0, MAX_GENERATION as i32) as u8,
            signed: true,
        });
    }
    let writable = data.get_component::<comp::WritableBookContent>()?;
    Some(Book {
        pages: writable
            .pages
            .iter()
            .map(|p| {
                vec![crate::text::Span {
                    text: p.raw.clone(),
                    style: Default::default(),
                }]
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        title: String::new(),
        author: String::new(),
        generation: 0,
        signed: false,
    })
}

const MAX_GENERATION: u8 = 3;

pub(crate) fn death_protection(data: &azalea_inventory::ItemStackData) -> bool {
    crate::items::component::<azalea_inventory::components::DeathProtection>(data).is_some()
}

pub(crate) fn charged_rocket(data: &azalea_inventory::ItemStackData) -> bool {
    use azalea_registry::builtin::ItemKind;
    data.get_component::<azalea_inventory::components::ChargedProjectiles>()
        .is_some_and(|c| c.items.iter().any(|i| i.kind() == ItemKind::FireworkRocket))
}

pub(crate) fn potion_contents(
    data: &azalea_inventory::ItemStackData,
) -> Option<potions::PotionContents> {
    use azalea_inventory::components as comp;

    let bare = |id: &str| id.trim_start_matches("minecraft:").to_string();
    if !potions::is_potion_item(&bare(data.kind.to_str())) {
        return None;
    }
    let c = data.get_component::<comp::PotionContents>()?;
    Some(potions::PotionContents {
        potion: c.potion.as_ref().map(|p| bare(p.to_str())),
        custom_color: c.custom_color.map(|v| v as u32 & 0xFF_FFFF),
        custom_effects: c
            .custom_effects
            .iter()
            .map(|e| potions::PotionEffectInstance {
                effect: bare(e.id.to_str()),
                details: e.details.clone(),
            })
            .collect(),
        custom_name: c.custom_name.clone(),
        duration_scale: data
            .component_patch
            .get::<comp::PotionDurationScale>()
            .map(|c| c.value),
    })
}

pub(crate) fn enchantment_name(
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
    e: &azalea_registry::data::Enchantment,
) -> String {
    use azalea_core::data_registry::DataRegistryWithKey;
    use azalea_registry::DataRegistry;
    registries
        .and_then(|r| e.key_owned(r))
        .map(|k| {
            azalea_registry::DataRegistryKey::into_ident(k)
                .path()
                .to_string()
        })
        .unwrap_or_else(|| format!("#{}", e.protocol_id()))
}

pub(crate) fn resolve_enchantments(
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
    enchantments: &[(String, i32)],
) -> std::collections::HashMap<azalea_registry::data::Enchantment, i32> {
    use azalea_registry::DataRegistry;
    use azalea_registry::identifier::Identifier;

    let Some(registries) = registries else {
        return std::collections::HashMap::new();
    };
    enchantments
        .iter()
        .filter_map(|(name, level)| {
            let id = registries
                .enchantment
                .map
                .get_index_of(&Identifier::new(name.clone()))?;
            Some((
                azalea_registry::data::Enchantment::new_raw(id as u32),
                *level,
            ))
        })
        .collect()
}

fn creative_stack(
    stack: &SlotStack,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
) -> ItemStack {
    use azalea_registry::builtin::ItemKind;
    use std::str::FromStr;

    if stack.is_empty() {
        return ItemStack::Empty;
    }
    let Ok(kind) = ItemKind::from_str(stack.item) else {
        return ItemStack::Empty;
    };
    if let Some(raw) = &stack.raw
        && let ItemStack::Present(data) = raw.as_ref()
        && data.kind == kind
    {
        let mut data = data.clone();
        data.count = stack.count as i32;
        let mut out = ItemStack::Present(data);
        azalea_inventory::bundle::toggle_selected_item(
            &mut out,
            azalea_inventory::bundle::NO_SELECTED_ITEM,
        );
        return out;
    }
    let mut out = ItemStack::new(kind, stack.count as i32);
    if !stack.extra.enchantments.is_empty() {
        let levels = resolve_enchantments(registries, &stack.extra.enchantments);
        if !levels.is_empty() {
            use azalea_inventory::components as comp;
            out = if kind == ItemKind::EnchantedBook {
                out.with_component(comp::StoredEnchantments {
                    enchantments: levels,
                })
            } else {
                out.with_component(comp::Enchantments { levels })
            };
        }
    }
    if let Some(contents) = &stack.extra.potion {
        use azalea_inventory::components as comp;
        use azalea_registry::builtin::{MobEffect, Potion};

        let potion = contents
            .potion
            .as_deref()
            .and_then(|id| Potion::from_str(id).ok());
        let custom_effects = contents
            .custom_effects
            .iter()
            .filter_map(|e| {
                Some(comp::MobEffectInstance {
                    id: MobEffect::from_str(&e.effect).ok()?,
                    details: e.details.clone(),
                })
            })
            .collect::<Vec<_>>();
        if potion.is_some()
            || contents.custom_color.is_some()
            || !custom_effects.is_empty()
            || contents.custom_name.is_some()
        {
            out = out.with_component(comp::PotionContents {
                potion,
                custom_color: contents.custom_color.map(|v| v as i32),
                custom_effects,
                custom_name: contents.custom_name.clone(),
            });
        }
        if let Some(value) = contents.duration_scale {
            out = out.with_component(comp::PotionDurationScale { value });
        }
    }
    if let Some(effects) = stack.extra.stew_effects.as_ref().filter(|e| !e.is_empty()) {
        use azalea_inventory::components as comp;
        use azalea_registry::builtin::MobEffect;

        let effects = effects
            .iter()
            .filter_map(|(id, duration)| {
                Some(comp::SuspiciousStewEffect {
                    effect: MobEffect::from_str(id).ok()?,
                    duration: *duration,
                })
            })
            .collect();
        out = out.with_component(comp::SuspiciousStewEffects { effects });
    }
    if let Some(amplifier) = stack.extra.ominous_amplifier.filter(|a| *a != 0) {
        out =
            out.with_component(azalea_inventory::components::OminousBottleAmplifier { amplifier });
    }
    out
}

pub(crate) fn set_creative_slot(bot: &Client, slot: u16, stack: ItemStack) {
    use azalea_protocol::packets::game::s_set_creative_mode_slot::ServerboundSetCreativeModeSlot;

    bot.write_packet(ServerboundSetCreativeModeSlot {
        slot_num: slot,
        item_stack: stack.clone().into(),
    });
    let mut ecs = bot.ecs.write();
    if let Some(mut inv) = ecs.get_mut::<azalea::entity::inventory::Inventory>(bot.entity)
        && let Some(dst) = inv.inventory_menu.slot_mut(slot as usize)
    {
        *dst = stack;
    }
}

#[derive(Default)]
pub(crate) struct AppliedInv {
    pub creative_cursor: Option<CreativeAnswer>,
    pub horse_selected: Vec<(usize, i32)>,
}

fn next_selection(selected: i32, size: usize, step: i8) -> Option<i32> {
    use azalea_inventory::bundle::{NO_SELECTED_ITEM, number_of_items_to_show};

    if step == 0 {
        return Some(NO_SELECTED_ITEM);
    }
    let shown = number_of_items_to_show(size) as i32;
    if shown == 0 {
        return None;
    }
    let next = crate::renderer::input::next_scroll_selection(f64::from(step), selected, shown);
    (next != selected).then_some(next)
}

fn select_in_menu(bot: &Client, slot: usize, step: i8) -> Option<i32> {
    use azalea_inventory::bundle::{is_bundle, toggle_selected_item};
    use azalea_inventory::components::BundleContents;

    let mut ecs = bot.ecs.write();
    let mut inventory = ecs.get_mut::<azalea::entity::inventory::Inventory>(bot.entity)?;
    let stack = inventory.menu_mut().slot_mut(slot)?;
    let item = stack.as_present().filter(|item| is_bundle(item.kind))?;
    let contents = item.get_unless_removed::<BundleContents>();
    let (selected, size) = contents.map_or((-1, 0), |c| (c.selected_item, c.items.len()));
    let next = next_selection(selected, size, step)?;
    toggle_selected_item(stack, next);
    Some(next)
}

fn select_in_horse(
    horse: &mut HorseMenu,
    slot: usize,
    step: i8,
    applied: &mut AppliedInv,
) -> Option<i32> {
    let bundle = horse.slots.get_mut(slot)?.bundle.as_mut()?;
    let next = next_selection(bundle.selected, bundle.items.len(), step)?;
    let selected = azalea_inventory::bundle::toggled(bundle.selected, next, bundle.items.len());
    std::sync::Arc::make_mut(bundle).selected = selected;
    applied.horse_selected.push((slot, selected));
    Some(next)
}

fn creative_bundle_click(bot: &Client, slot: u16, right: bool, cursor: ItemStack) -> ItemStack {
    use azalea_inventory::operations::PickupClick;
    use azalea_protocol::packets::game::s_set_creative_mode_slot::ServerboundSetCreativeModeSlot;

    let operation = if right {
        PickupClick::Right { slot: Some(slot) }
    } else {
        PickupClick::Left { slot: Some(slot) }
    }
    .into();
    let (changed, cursor) = {
        let mut ecs = bot.ecs.write();
        let Some(mut inventory) = ecs.get_mut::<azalea::entity::inventory::Inventory>(bot.entity)
        else {
            return cursor;
        };
        let old = inventory.inventory_menu.slots();
        let held = std::mem::replace(&mut inventory.carried, cursor);
        inventory.simulate_click(&operation, &azalea::entity::PlayerAbilities::default());
        let new = inventory.inventory_menu.slots();
        let changed: Vec<(usize, ItemStack)> = new
            .into_iter()
            .enumerate()
            .filter(|(i, stack)| old[*i] != *stack)
            .collect();
        (changed, std::mem::replace(&mut inventory.carried, held))
    };
    for (i, stack) in changed.into_iter().filter(|(i, _)| *i > 0) {
        bot.write_packet(ServerboundSetCreativeModeSlot {
            slot_num: i as u16,
            item_stack: stack.into(),
        });
    }
    #[cfg(feature = "audio")]
    play_bundle_sound(bot);
    cursor
}

#[cfg(feature = "audio")]
fn play_bundle_sound(bot: &Client) {
    let sound = bot
        .component::<azalea::entity::inventory::Inventory>()
        .ok()
        .and_then(|i| i.bundle_sound);
    if let Some(sound) = sound {
        crate::play::player_sounds::bundle_click(bot, sound);
    }
}

#[cfg(feature = "audio")]
fn horse_bundle_sound(
    bot: &Client,
    horse: &HorseMenu,
    operation: &azalea_inventory::operations::ClickOperation,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
) {
    use azalea::entity::inventory::Inventory;
    use azalea_inventory::operations::{ClickOperation, PickupClick};

    const STORAGE: u16 = 9;
    const REFUSES: u16 = 0;
    let (slot, right) = match operation {
        ClickOperation::Pickup(PickupClick::Left { slot: Some(slot) }) => (*slot, false),
        ClickOperation::Pickup(PickupClick::Right { slot: Some(slot) }) => (*slot, true),
        _ => return,
    };
    let Some(clicked) = horse.slots.get(usize::from(slot)) else {
        return;
    };
    let carried = bot
        .component::<Inventory>()
        .map(|i| i.carried.clone())
        .unwrap_or_default();
    let gear = slot < 2;
    let target = if gear && !gear_takes_next_out(&carried, slot) {
        REFUSES
    } else {
        STORAGE
    };
    let mut scratch = Inventory {
        carried,
        ..Inventory::default()
    };
    if let Some(stack) = scratch.inventory_menu.slot_mut(usize::from(target)) {
        *stack = creative_stack(clicked, registries);
    }
    let click = if right {
        PickupClick::Right { slot: Some(target) }
    } else {
        PickupClick::Left { slot: Some(target) }
    };
    scratch.simulate_click(&click.into(), &azalea::entity::PlayerAbilities::default());
    if let Some(sound) = scratch.bundle_sound {
        crate::play::player_sounds::bundle_click(bot, sound);
    }
}

#[cfg(feature = "audio")]
fn gear_takes_next_out(carried: &ItemStack, slot: u16) -> bool {
    use azalea_inventory::components::{BundleContents, EquipmentSlot, Equippable};

    let gear = if slot == 0 {
        EquipmentSlot::Saddle
    } else {
        EquipmentSlot::Body
    };
    let Some(contents) = carried.get_component::<BundleContents>() else {
        return false;
    };
    let next = usize::try_from(contents.selected_item).unwrap_or(0);
    let item = contents.items.get(next).and_then(ItemStack::as_present);
    item.and_then(|item| item.get_unless_removed::<Equippable>())
        .is_some_and(|e| e.slot == gear)
}

pub(crate) fn apply_inv_actions(
    bot: &Client,
    actions: Vec<InvAction>,
    mut horse: Option<HorseMenu>,
) -> AppliedInv {
    use azalea_protocol::packets::game::s_bundle_item_selected::ServerboundBundleItemSelected;
    use azalea_protocol::packets::game::s_container_button_click::ServerboundContainerButtonClick;
    use azalea_protocol::packets::game::s_container_close::ServerboundContainerClose;
    use azalea_protocol::packets::game::s_container_slot_state_changed::ServerboundContainerSlotStateChanged;
    use azalea_protocol::packets::game::s_rename_item::ServerboundRenameItem;
    use azalea_protocol::packets::game::s_select_trade::ServerboundSelectTrade;
    use azalea_protocol::packets::game::s_set_beacon::ServerboundSetBeacon;
    use azalea_protocol::packets::game::s_set_creative_mode_slot::ServerboundSetCreativeModeSlot;

    let world = bot.world().ok();
    let world_guard = world.as_ref().map(|w| w.read());
    let registries = world_guard.as_ref().map(|g| &g.registries);
    let mut applied = AppliedInv::default();

    for action in actions {
        match action {
            InvAction::Click(operation) => {
                if let Some(horse) = horse.as_ref() {
                    #[cfg(feature = "audio")]
                    horse_bundle_sound(bot, horse, &operation, registries);
                    send_horse_click(bot, horse, &operation, registries);
                    continue;
                }
                let window_id = bot
                    .component::<azalea::entity::inventory::Inventory>()
                    .map(|i| i.id)
                    .unwrap_or(0);
                bot.ecs.write().trigger(ContainerClickEvent {
                    entity: bot.entity,
                    window_id,
                    operation,
                });
                #[cfg(feature = "audio")]
                play_bundle_sound(bot);
            }
            InvAction::CreativeBundleClick {
                slot,
                right,
                cursor,
            } => {
                let cursor = creative_stack(&cursor, registries);
                let cursor = creative_bundle_click(bot, slot, right, cursor);
                let slots = bot
                    .component::<azalea::entity::inventory::Inventory>()
                    .map(|i| i.inventory_menu.slots())
                    .unwrap_or_default();
                applied.creative_cursor = Some(CreativeAnswer {
                    cursor: slot_stack(&cursor, registries, false),
                    slots: slots
                        .iter()
                        .map(|s| slot_stack(s, registries, false))
                        .collect(),
                });
            }
            InvAction::BundleSelect {
                slot,
                wire_slot,
                step,
            } => {
                let slot = usize::from(slot);
                let next = match horse.as_mut() {
                    Some(horse) => select_in_horse(horse, slot, step, &mut applied),
                    None => select_in_menu(bot, slot, step),
                };
                if let Some(next) = next {
                    bot.write_packet(ServerboundBundleItemSelected {
                        slot_id: wire_slot,
                        selected_item_index: next as u32,
                    });
                }
            }
            InvAction::CreativeSet { slot, stack } => {
                set_creative_slot(bot, slot, creative_stack(&stack, registries));
            }
            InvAction::CreativeDrop { stack } => {
                let stack = creative_stack(&stack, registries);
                if stack.is_empty() {
                    continue;
                }
                bot.write_packet(ServerboundSetCreativeModeSlot {
                    slot_num: u16::MAX,
                    item_stack: stack.into(),
                });
            }
            InvAction::ButtonClick(button_id) => {
                let window_id = bot
                    .component::<azalea::entity::inventory::Inventory>()
                    .map(|i| i.id)
                    .unwrap_or(0);
                bot.write_packet(ServerboundContainerButtonClick {
                    container_id: window_id,
                    button_id: button_id as u32,
                });
            }
            InvAction::SelectTrade(item) => {
                bot.write_packet(ServerboundSelectTrade { item });
            }
            InvAction::SetBeacon { primary, secondary } => {
                bot.write_packet(ServerboundSetBeacon { primary, secondary });
            }
            InvAction::RenameItem(name) => {
                bot.write_packet(ServerboundRenameItem { name });
            }
            InvAction::SlotStateChanged { slot, enabled } => {
                let window_id = bot
                    .component::<azalea::entity::inventory::Inventory>()
                    .map(|i| i.id)
                    .unwrap_or(0);
                if let Some(mut inv) = bot
                    .ecs
                    .write()
                    .get_mut::<azalea::entity::inventory::Inventory>(bot.entity)
                {
                    inv.set_crafter_slot_disabled(slot as usize, !enabled);
                }
                bot.write_packet(ServerboundContainerSlotStateChanged {
                    slot_id: slot as u32,
                    container_id: window_id,
                    new_state: enabled,
                });
            }
            InvAction::Close => {
                if let Some(horse) = horse.as_ref() {
                    bot.write_packet(ServerboundContainerClose {
                        container_id: horse.container_id,
                    });
                    continue;
                }
                let id = bot
                    .component::<azalea::entity::inventory::Inventory>()
                    .map(|i| i.id)
                    .unwrap_or(0);
                bot.ecs
                    .write()
                    .trigger(azalea::inventory::CloseContainerEvent {
                        entity: bot.entity,
                        id,
                    });
            }
        }
    }
    applied
}

fn send_horse_click(
    bot: &Client,
    horse: &HorseMenu,
    operation: &azalea_inventory::operations::ClickOperation,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
) {
    use azalea_protocol::packets::game::s_container_click::{
        HashedStack, ServerboundContainerClick,
    };

    let Some(registries) = registries else { return };
    let carried = bot
        .component::<azalea::entity::inventory::Inventory>()
        .map(|i| i.carried.clone())
        .unwrap_or_default();
    let carried_item = HashedStack::from_item_stack(&carried, registries);

    bot.write_packet(ServerboundContainerClick {
        container_id: horse.container_id,
        state_id: horse.state_id,
        slot_num: operation.slot_num().map(|n| n as i16).unwrap_or(-999),
        button_num: operation.button_num(),
        click_type: operation.click_type(),
        changed_slots: Default::default(),
        carried_item,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use azalea_buf::AzBuf;
    use azalea_inventory::UntrustedItemStack;
    use azalea_registry::builtin::DataComponentKind;

    fn water_bottle() -> SlotStack {
        SlotStack {
            item: "potion",
            count: 1,
            extra: StackExtra {
                potion: Some(potions::PotionContents {
                    potion: Some("water".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_creative_stack_length_delimits_its_components() {
        let stack = UntrustedItemStack(creative_stack(&water_bottle(), None));
        let mut bytes = Vec::new();
        stack.azalea_write(&mut bytes).unwrap();

        let mut kind = Vec::new();
        DataComponentKind::PotionContents
            .azalea_write(&mut kind)
            .unwrap();
        let head_len = bytes.len() - kind.len() - 6;
        assert_eq!(&bytes[head_len..head_len + kind.len()], &kind[..]);

        let payload = &bytes[head_len + kind.len()..];
        assert_eq!(payload, &[5, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn an_untrusted_stack_round_trips() {
        for stack in [
            water_bottle(),
            SlotStack {
                extra: StackExtra {
                    potion: Some(potions::PotionContents {
                        potion: Some("strong_healing".into()),
                        custom_color: Some(0x123456),
                        custom_effects: vec![potions::PotionEffectInstance::new("poison", 200, 1)],
                        custom_name: None,
                        duration_scale: None,
                    }),
                    ..Default::default()
                }
                .into(),
                ..water_bottle()
            },
            SlotStack {
                item: "diamond_sword",
                count: 1,
                ..Default::default()
            },
        ] {
            let built = UntrustedItemStack(creative_stack(&stack, None));
            let mut bytes = Vec::new();
            built.azalea_write(&mut bytes).unwrap();
            let read =
                UntrustedItemStack::azalea_read(&mut std::io::Cursor::new(&bytes[..])).unwrap();
            assert_eq!(read, built, "{}", stack.item);
        }
    }

    #[test]
    fn the_next_selection_wraps_over_the_shown_items() {
        assert_eq!(next_selection(-1, 3, 1), Some(2));
        assert_eq!(next_selection(-1, 3, -1), Some(0));
        assert_eq!(next_selection(0, 3, 1), Some(2));
        assert_eq!(next_selection(2, 3, -1), Some(0));
        assert_eq!(next_selection(-1, 0, 1), None);
        assert_eq!(next_selection(-1, 0, 0), Some(-1));
        assert_eq!(next_selection(0, 1, 1), None);
    }

    #[test]
    fn death_protection_reads_the_patch_before_the_default() {
        use azalea_inventory::ItemStack;
        use azalea_inventory::components::DeathProtection;
        use azalea_registry::builtin::ItemKind;

        let protects = |stack: ItemStack| death_protection(stack.as_present().unwrap());
        assert!(protects(ItemStack::new(ItemKind::TotemOfUndying, 1)));
        assert!(!protects(
            ItemStack::new(ItemKind::TotemOfUndying, 1).with_component::<DeathProtection>(None)
        ));
        assert!(!protects(ItemStack::new(ItemKind::Stick, 1)));
        assert!(protects(ItemStack::new(ItemKind::Stick, 1).with_component(
            DeathProtection {
                death_effects: Vec::new()
            }
        )));
    }
}
