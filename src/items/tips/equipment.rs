use std::str::FromStr;

use azalea_chat::FormattedText;
use azalea_core::attribute_modifier_operation::AttributeModifierOperation;
use azalea_core::registry_holder::RegistryDeserializesTo;
use azalea_core::registry_holder::RegistryHolder;
use azalea_core::registry_holder::value::{AttributeEffect, LevelBasedValue};
use azalea_inventory::ItemStackData;
use azalea_inventory::components::{
    AttributeModifierDisplay, AttributeModifiers, DyedColor, Enchantments, EquipmentSlotGroup,
    IntangibleProjectile, JukeboxPlayable, Trim,
};
use azalea_registry::builtin::{Attribute, EnchantmentEffectComponentKind};
use azalea_registry::identifier::Identifier;
use azalea_registry::{DataRegistry, Holder};
use serde_json::Value;
use simdnbt::owned::NbtCompound;

use super::{Ctx, Line, TipAt, shows_component};
use crate::client::chat_text;
use crate::gui::tooltip::{format_modifier, translate};
use crate::items::component;
use crate::text::{Span, Style};

const DARK_GREEN: u32 = crate::text::FORMAT_COLORS[2];
const GRAY: u32 = crate::text::GRAY;
const BLUE: u32 = crate::text::FORMAT_COLORS[9];
const RED: u32 = crate::text::RED;

pub(super) fn push(at: TipAt, data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    match at {
        TipAt::ItemHover => hover_text(data, out),
        TipAt::BeforeEnchantments => {
            jukebox_line(data, ctx, out);
            trim_lines(data, ctx, out);
        }
        TipAt::BeforeLore => dyed_line(data, ctx, out),
        TipAt::AfterLore => {
            attribute_lines(data, ctx, out);
            if shows_component::<IntangibleProjectile>(data)
                && component::<IntangibleProjectile>(data).is_some()
            {
                out.push(colored(translate("item.intangible", &[]), GRAY));
            }
        }
        _ => {}
    }
}

fn colored(text: String, color: u32) -> Line {
    vec![Span::colored(text, color)]
}

fn item_id(data: &ItemStackData) -> &'static str {
    let id = data.kind.to_str();
    id.strip_prefix("minecraft:").unwrap_or(id)
}

fn hover_text(data: &ItemStackData, out: &mut Vec<Line>) {
    let id = item_id(data);
    if let Some((applies_to, ingredients)) = smithing_template_keys(id) {
        smithing_template_lines(applies_to, ingredients, out);
    } else if id == "disc_fragment_5" {
        out.push(colored(
            translate("item.minecraft.disc_fragment_5.desc", &[]),
            GRAY,
        ));
    }
}

fn smithing_template_keys(id: &str) -> Option<(&'static str, &'static str)> {
    if id == "netherite_upgrade_smithing_template" {
        Some((
            "item.minecraft.smithing_template.netherite_upgrade.applies_to",
            "item.minecraft.smithing_template.netherite_upgrade.ingredients",
        ))
    } else if id.ends_with("_armor_trim_smithing_template") {
        Some((
            "item.minecraft.smithing_template.armor_trim.applies_to",
            "item.minecraft.smithing_template.armor_trim.ingredients",
        ))
    } else {
        None
    }
}

fn smithing_template_lines(applies_to: &str, ingredients: &str, out: &mut Vec<Line>) {
    out.push(colored(
        translate("item.minecraft.smithing_template", &[]),
        GRAY,
    ));
    out.push(Vec::new());
    out.push(colored(
        translate("item.minecraft.smithing_template.applies_to", &[]),
        GRAY,
    ));
    out.push(colored(format!(" {}", translate(applies_to, &[])), BLUE));
    out.push(colored(
        translate("item.minecraft.smithing_template.ingredients", &[]),
        GRAY,
    ));
    out.push(colored(format!(" {}", translate(ingredients, &[])), BLUE));
}

fn registry_description(
    registries: &RegistryHolder,
    registry: &'static str,
    protocol_id: u32,
) -> Option<FormattedText> {
    let (name, entry) = <NbtCompound as RegistryDeserializesTo>::get_for_registry(
        registries,
        registry,
        protocol_id,
    )?;
    entry_description(registry, name, entry)
}

fn named_description(
    registries: Option<&RegistryHolder>,
    registry: &'static str,
    key: &str,
) -> Option<FormattedText> {
    let name = Identifier::new(key);
    let entry = registries
        .and_then(|r| r.extra.get(&Identifier::new(registry)))
        .and_then(|r| r.map.get(&name));
    match entry {
        Some(entry) => entry_description(registry, &name, entry),
        None => datapack_description(registry, &name),
    }
}

fn entry_description(
    registry: &str,
    name: &Identifier,
    entry: &NbtCompound,
) -> Option<FormattedText> {
    if entry.is_empty() {
        return datapack_description(registry, name);
    }
    chat_text::from_nbt_tag(entry.get("description")?)
}

fn datapack_description(registry: &str, name: &Identifier) -> Option<FormattedText> {
    if name.namespace() != "minecraft" {
        return None;
    }
    let json = crate::util::datapack::entry(registry, name.path())?;
    serde_json::from_value(json.get("description")?.clone()).ok()
}

const DISC_SONGS: [(&str, &str); 22] = [
    ("music_disc_13", "13"),
    ("music_disc_cat", "cat"),
    ("music_disc_blocks", "blocks"),
    ("music_disc_bounce", "bounce"),
    ("music_disc_chirp", "chirp"),
    ("music_disc_creator", "creator"),
    ("music_disc_creator_music_box", "creator_music_box"),
    ("music_disc_far", "far"),
    ("music_disc_lava_chicken", "lava_chicken"),
    ("music_disc_mall", "mall"),
    ("music_disc_mellohi", "mellohi"),
    ("music_disc_stal", "stal"),
    ("music_disc_strad", "strad"),
    ("music_disc_ward", "ward"),
    ("music_disc_11", "11"),
    ("music_disc_wait", "wait"),
    ("music_disc_otherside", "otherside"),
    ("music_disc_relic", "relic"),
    ("music_disc_5", "5"),
    ("music_disc_pigstep", "pigstep"),
    ("music_disc_precipice", "precipice"),
    ("music_disc_tears", "tears"),
];

fn jukebox_line(data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    if !shows_component::<JukeboxPlayable>(data) {
        return;
    }
    let Some(playable) = component::<JukeboxPlayable>(data) else {
        return;
    };
    let description = if data.component_patch.get::<JukeboxPlayable>().is_some() {
        match &playable.value {
            Holder::Direct(song) => Some(song.description.clone()),
            Holder::Reference(song) => ctx
                .registries
                .and_then(|r| registry_description(r, "jukebox_song", song.protocol_id())),
        }
    } else {
        let id = item_id(data);
        DISC_SONGS
            .iter()
            .find(|(item, _)| *item == id)
            .and_then(|(_, song)| named_description(ctx.registries, "jukebox_song", song))
    };
    if let Some(description) = description {
        out.push(chat_text::to_spans_styled(
            &description,
            Style::colored(GRAY),
        ));
    }
}

fn trim_lines(data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    if !shows_component::<Trim>(data) {
        return;
    }
    let Some(trim) = component::<Trim>(data) else {
        return;
    };
    let material = match &trim.material {
        Holder::Direct(material) => Some(material.description.clone()),
        Holder::Reference(material) => ctx
            .registries
            .and_then(|r| registry_description(r, "trim_material", material.protocol_id())),
    };
    let pattern = match &trim.pattern {
        Holder::Direct(pattern) => Some(pattern.description.clone()),
        Holder::Reference(pattern) => ctx
            .registries
            .and_then(|r| registry_description(r, "trim_pattern", pattern.protocol_id())),
    };
    let (Some(material), Some(mut pattern)) = (material, pattern) else {
        return;
    };
    pattern
        .get_base_mut()
        .style
        .apply(&material.get_base().style);

    out.push(colored(
        translate("item.minecraft.smithing_template.upgrade", &[]),
        GRAY,
    ));
    for text in [&pattern, &material] {
        let mut line = vec![Span::new(" ")];
        line.extend(chat_text::to_spans(text));
        out.push(line);
    }
}

fn dyed_line(data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    if !shows_component::<DyedColor>(data) {
        return;
    }
    let Some(dyed) = component::<DyedColor>(data) else {
        return;
    };
    if ctx.advanced {
        let hex = format!("#{:06X}", dyed.rgb);
        out.push(colored(translate("item.color", &[hex]), GRAY));
    } else {
        out.push(vec![Span {
            text: translate("item.dyed", &[]),
            style: Style {
                italic: true,
                ..Style::colored(GRAY)
            },
        }]);
    }
}

const SLOT_GROUPS: [(EquipmentSlotGroup, &str); 11] = [
    (EquipmentSlotGroup::Any, "any"),
    (EquipmentSlotGroup::Mainhand, "mainhand"),
    (EquipmentSlotGroup::Offhand, "offhand"),
    (EquipmentSlotGroup::Hand, "hand"),
    (EquipmentSlotGroup::Feet, "feet"),
    (EquipmentSlotGroup::Legs, "legs"),
    (EquipmentSlotGroup::Chest, "chest"),
    (EquipmentSlotGroup::Head, "head"),
    (EquipmentSlotGroup::Armor, "armor"),
    (EquipmentSlotGroup::Body, "body"),
    (EquipmentSlotGroup::Saddle, "saddle"),
];

const PLAYER_ATTACK_DAMAGE_BASE: f64 = 1.0;

fn operation_id(operation: AttributeModifierOperation) -> u8 {
    match operation {
        AttributeModifierOperation::AddValue => 0,
        AttributeModifierOperation::AddMultipliedBase => 1,
        AttributeModifierOperation::AddMultipliedTotal => 2,
    }
}

fn attribute_color(attribute: Attribute, increase: bool) -> u32 {
    match attribute {
        Attribute::BurningTime | Attribute::FallDamageMultiplier => {
            if increase {
                RED
            } else {
                BLUE
            }
        }
        Attribute::Gravity
        | Attribute::Scale
        | Attribute::WaypointTransmitRange
        | Attribute::WaypointReceiveRange => GRAY,
        _ => {
            if increase {
                BLUE
            } else {
                RED
            }
        }
    }
}

fn attribute_name(attribute: Attribute) -> String {
    let id = attribute.to_str();
    let path = id.strip_prefix("minecraft:").unwrap_or(id);
    translate(&format!("attribute.name.{path}"), &[])
}

fn player_base(id: &Identifier, ctx: &Ctx) -> Option<f64> {
    let attack_speed_base = ctx.attack_speed_base?;
    if id.namespace() != "minecraft" {
        return None;
    }
    match id.path() {
        "base_attack_damage" => Some(PLAYER_ATTACK_DAMAGE_BASE),
        "base_attack_speed" => Some(attack_speed_base),
        _ => None,
    }
}

fn default_row(
    attribute: Attribute,
    id: Option<&Identifier>,
    amount: f64,
    operation: AttributeModifierOperation,
    ctx: &Ctx,
) -> Option<Line> {
    let base = id.and_then(|id| player_base(id, ctx));
    let amount = amount + base.unwrap_or(0.0);
    let display = match operation {
        AttributeModifierOperation::AddValue if attribute == Attribute::KnockbackResistance => {
            amount * 10.0
        }
        AttributeModifierOperation::AddValue => amount,
        _ => amount * 100.0,
    };
    let op = operation_id(operation);
    if base.is_some() {
        let text = translate(
            &format!("attribute.modifier.equals.{op}"),
            &[format_modifier(display), attribute_name(attribute)],
        );
        Some(colored(format!(" {text}"), DARK_GREEN))
    } else if amount > 0.0 {
        let text = translate(
            &format!("attribute.modifier.plus.{op}"),
            &[format_modifier(display), attribute_name(attribute)],
        );
        Some(colored(text, attribute_color(attribute, true)))
    } else if amount < 0.0 {
        let text = translate(
            &format!("attribute.modifier.take.{op}"),
            &[format_modifier(-display), attribute_name(attribute)],
        );
        Some(colored(text, attribute_color(attribute, false)))
    } else {
        None
    }
}

struct EnchantedModifiers {
    slots: Vec<EquipmentSlotGroup>,
    effects: Vec<(Attribute, f64, AttributeModifierOperation)>,
}

fn enchanted_modifiers(
    data: &ItemStackData,
    registries: Option<&RegistryHolder>,
) -> Vec<EnchantedModifiers> {
    let (Some(registries), Some(enchantments)) = (registries, component::<Enchantments>(data))
    else {
        return Vec::new();
    };
    let mut levels: Vec<(u32, i32)> = enchantments
        .levels
        .iter()
        .map(|(enchantment, level)| (enchantment.protocol_id(), *level))
        .collect();
    levels.sort_unstable();

    let mut out = Vec::new();
    for (id, level) in levels {
        let Some((name, parsed)) = registries.enchantment.map.get_index(id as usize) else {
            continue;
        };
        if name.namespace() != "minecraft" {
            continue;
        }
        let Some(json) = crate::util::datapack::entry("enchantment", name.path()) else {
            continue;
        };
        let slots: Vec<EquipmentSlotGroup> = json
            .get("slots")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|slot| {
                let slot = slot.as_str()?;
                SLOT_GROUPS
                    .iter()
                    .find(|(_, n)| *n == slot)
                    .map(|(g, _)| *g)
            })
            .collect();
        let effects = match parsed.get::<AttributeEffect>() {
            Some(effects) => effects
                .iter()
                .map(|e| (e.attribute, e.amount.calculate(level) as f64, e.operation))
                .collect(),
            None if is_placeholder(parsed, &json) => json_attribute_effects(&json, level),
            None => Vec::new(),
        };
        if !slots.is_empty() && !effects.is_empty() {
            out.push(EnchantedModifiers { slots, effects });
        }
    }
    out
}

fn is_placeholder(
    parsed: &azalea_core::registry_holder::enchantment::EnchantmentData,
    json: &Value,
) -> bool {
    let Some(effects) = json.get("effects").and_then(Value::as_object) else {
        return false;
    };
    !effects.is_empty()
        && effects.keys().all(|key| {
            EnchantmentEffectComponentKind::from_str(key)
                .map_or(true, |kind| parsed.get_kind(kind).is_none())
        })
}

fn json_attribute_effects(
    json: &Value,
    level: i32,
) -> Vec<(Attribute, f64, AttributeModifierOperation)> {
    let list = json
        .get("effects")
        .and_then(Value::as_object)
        .and_then(|effects| {
            effects
                .iter()
                .find(|(key, _)| key.strip_prefix("minecraft:").unwrap_or(key) == "attributes")
        })
        .and_then(|(_, list)| list.as_array());
    list.into_iter()
        .flatten()
        .filter_map(|effect| {
            let attribute = Attribute::from_str(effect.get("attribute")?.as_str()?).ok()?;
            let amount = level_value(effect.get("amount")?)?.calculate(level) as f64;
            let operation =
                AttributeModifierOperation::from_str(effect.get("operation")?.as_str()?).ok()?;
            Some((attribute, amount, operation))
        })
        .collect()
}

fn level_value(value: &Value) -> Option<LevelBasedValue> {
    if let Some(constant) = value.as_f64() {
        return Some(LevelBasedValue::Constant(constant as f32));
    }
    let float = |key: &str| value.get(key)?.as_f64().map(|v| v as f32);
    let nested = |key: &str| level_value(value.get(key)?).map(Box::new);
    Some(match crate::util::datapack::bare_id(value.get("type")?)? {
        "linear" => LevelBasedValue::Linear {
            base: float("base")?,
            per_level_above_first: float("per_level_above_first")?,
        },
        "levels_squared" => LevelBasedValue::LevelsSquared {
            added: float("added")?,
        },
        "clamped" => LevelBasedValue::Clamped {
            value: nested("value")?,
            min: float("min")?,
            max: float("max")?,
        },
        "fraction" => LevelBasedValue::Fraction {
            numerator: nested("numerator")?,
            denominator: nested("denominator")?,
        },
        "exponent" => LevelBasedValue::Exponent {
            base: nested("base")?,
            power: nested("power")?,
        },
        "lookup" => LevelBasedValue::Lookup {
            values: value
                .get("values")?
                .as_array()?
                .iter()
                .map(|v| v.as_f64().map(|v| v as f32))
                .collect::<Option<Vec<f32>>>()?,
            fallback: nested("fallback")?,
        },
        _ => return None,
    })
}

fn attribute_lines(data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    if !shows_component::<AttributeModifiers>(data) {
        return;
    }
    let modifiers = component::<AttributeModifiers>(data);
    let enchanted = enchanted_modifiers(data, ctx.registries);
    for (group, name) in SLOT_GROUPS {
        let mut shown = false;
        let mut rows: Vec<Line> = Vec::new();
        let entries = modifiers
            .iter()
            .flat_map(|m| m.modifiers.iter())
            .filter(|entry| entry.slot == group);
        for entry in entries {
            match &entry.display {
                AttributeModifierDisplay::Hidden => {}
                AttributeModifierDisplay::Default => {
                    shown = true;
                    let modifier = &entry.modifier;
                    rows.extend(default_row(
                        entry.kind,
                        Some(&modifier.id),
                        modifier.amount,
                        modifier.operation,
                        ctx,
                    ));
                }
                AttributeModifierDisplay::Override { text } => {
                    shown = true;
                    rows.push(chat_text::to_spans(text));
                }
            }
        }
        for enchantment in enchanted.iter().filter(|e| e.slots.contains(&group)) {
            for &(attribute, amount, operation) in &enchantment.effects {
                shown = true;
                rows.extend(default_row(attribute, None, amount, operation, ctx));
            }
        }
        if shown {
            out.push(Vec::new());
            out.push(colored(
                translate(&format!("item.modifiers.{name}"), &[]),
                GRAY,
            ));
            out.extend(rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use azalea_registry::builtin::ItemKind;

    use super::*;

    fn ctx() -> Ctx<'static> {
        Ctx {
            registries: None,
            advanced: false,
            attack_speed_base: Some(4.0),
            peaceful: false,
            containers: super::super::ContainerRows::Vanilla,
        }
    }

    fn text(line: &Line) -> String {
        line.iter().map(|s| s.text.as_str()).collect()
    }

    fn has_lang() -> bool {
        translate("item.modifiers.mainhand", &[]) != "item.modifiers.mainhand"
    }

    #[test]
    fn modifier_format_matches_decimal_format() {
        assert_eq!(format_modifier(6.0), "6");
        assert_eq!(format_modifier(100.0), "100");
        assert_eq!(format_modifier(0.0), "0");
        assert_eq!(format_modifier(4.0 - 2.4000000953674316), "1.6");
        assert_eq!(format_modifier(0.33333334f32 as f64), "0.33");
        assert_eq!(format_modifier(15.000000596046448), "15");
        assert_eq!(format_modifier(0.125), "0.12");
        assert_eq!(format_modifier(0.375), "0.38");
        assert_eq!(format_modifier(-1.5), "-1.5");
    }

    #[test]
    fn iron_sword_shows_base_attack_rows() {
        let data = ItemStackData::new(ItemKind::IronSword, 1);
        let mut out = Vec::new();
        push(TipAt::AfterLore, &data, &ctx(), &mut out);
        assert_eq!(out.len(), 4, "{out:?}");
        assert!(out[0].is_empty());
        assert_eq!(out[1][0].style.color, GRAY);
        assert_eq!(out[2][0].style.color, DARK_GREEN);
        assert_eq!(out[3][0].style.color, DARK_GREEN);
        assert!(text(&out[2]).starts_with(' '));
        if !has_lang() {
            return;
        }
        assert_eq!(text(&out[1]), "When in Main Hand:");
        assert_eq!(text(&out[2]), " 6 Attack Damage");
        assert_eq!(text(&out[3]), " 1.6 Attack Speed");
    }

    #[test]
    fn hidden_and_zero_modifiers() {
        let data = ItemStackData::new(ItemKind::CarvedPumpkin, 1);
        let mut out = Vec::new();
        push(TipAt::AfterLore, &data, &ctx(), &mut out);
        assert!(out.is_empty(), "{out:?}");

        let data = ItemStackData::new(ItemKind::ChainmailBoots, 1);
        let mut out = Vec::new();
        push(TipAt::AfterLore, &data, &ctx(), &mut out);
        assert_eq!(out.len(), 3, "{out:?}");
        assert_eq!(out[2][0].style.color, BLUE);
        if has_lang() {
            assert_eq!(text(&out[1]), "When on Feet:");
            assert_eq!(text(&out[2]), "+1 Armor");
        }
    }

    #[test]
    fn negative_sentiment_colours() {
        assert_eq!(attribute_color(Attribute::BurningTime, false), BLUE);
        assert_eq!(attribute_color(Attribute::BurningTime, true), RED);
        assert_eq!(attribute_color(Attribute::Scale, true), GRAY);
        assert_eq!(attribute_color(Attribute::Armor, false), RED);
    }

    #[test]
    fn json_level_values() {
        let linear: Value = serde_json::json!({
            "type": "minecraft:linear", "base": -0.15, "per_level_above_first": -0.15
        });
        let at_4 = level_value(&linear).unwrap().calculate(4) as f64;
        assert_eq!(format_modifier(-at_4), "0.6");
        let fraction: Value = serde_json::json!({
            "type": "minecraft:fraction",
            "numerator": {"type": "minecraft:linear", "base": 1.0, "per_level_above_first": 1.0},
            "denominator": {"type": "minecraft:linear", "base": 2.0, "per_level_above_first": 1.0}
        });
        assert_eq!(level_value(&fraction).unwrap().calculate(3), 0.75);
        assert_eq!(
            level_value(&serde_json::json!(2.5)).unwrap().calculate(7),
            2.5
        );
    }

    #[test]
    fn smithing_template_rows() {
        let data = ItemStackData::new(ItemKind::SentryArmorTrimSmithingTemplate, 1);
        let mut out = Vec::new();
        push(TipAt::ItemHover, &data, &ctx(), &mut out);
        assert_eq!(out.len(), 6, "{out:?}");
        assert!(out[1].is_empty());
        assert_eq!(out[0][0].style.color, GRAY);
        assert_eq!(out[3][0].style.color, BLUE);
        assert_eq!(out[5][0].style.color, BLUE);
        assert!(text(&out[3]).starts_with(' '));

        let data = ItemStackData::new(ItemKind::NetheriteUpgradeSmithingTemplate, 1);
        let mut netherite = Vec::new();
        push(TipAt::ItemHover, &data, &ctx(), &mut netherite);
        assert_eq!(netherite.len(), 6);
        if !has_lang() {
            return;
        }
        assert_eq!(text(&out[0]), "Smithing Template");
        assert_eq!(text(&out[2]), "Applies to:");
        assert_eq!(text(&out[3]), " Armor");
        assert_eq!(text(&netherite[3]), " Diamond Equipment");
        assert_eq!(text(&netherite[5]), " Netherite Ingot");
    }

    #[test]
    fn plain_item_adds_nothing() {
        let data = ItemStackData::new(ItemKind::Stone, 1);
        let mut out = Vec::new();
        for at in [
            TipAt::ItemHover,
            TipAt::BeforeEnchantments,
            TipAt::BeforeLore,
            TipAt::AfterLore,
        ] {
            push(at, &data, &ctx(), &mut out);
        }
        assert!(out.is_empty(), "{out:?}");
    }
}
