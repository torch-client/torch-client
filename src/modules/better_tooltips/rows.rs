use azalea_inventory::ItemStackData;
use azalea_inventory::components::{self as comp, AxolotlVariantKind, SalmonSize};
use azalea_inventory::item::consume_effect::ConsumeEffect;

use super::Opts;
use crate::gui::keybinds::Bound;
use crate::gui::tooltip::{format_modifier, potion_lines};
use crate::items::potions::PotionEffectInstance;
use crate::session::SlotStack;
use crate::text::Span;

type Line = Vec<Span>;

const GRAY: u32 = crate::text::GRAY;
const YELLOW: u32 = crate::text::YELLOW;

pub(crate) fn top_rows(
    stack: &SlotStack,
    data: &ItemStackData,
    opts: &Opts,
    creative: bool,
    tick_rate: f32,
) -> Vec<Line> {
    let mut out = Vec::new();
    if opts.previewing && opts.buckets {
        bucket_rows(stack.item, data, &mut out);
    }
    if opts.effects {
        effect_rows(stack, data, creative, tick_rate, &mut out);
    }
    out
}

pub(crate) fn effect_rows(
    stack: &SlotStack,
    data: &ItemStackData,
    creative: bool,
    tick_rate: f32,
    out: &mut Vec<Line>,
) {
    if stack.item == "suspicious_stew" {
        if !creative
            && let Some(stew) = &stack.extra.stew_effects
            && !stew.is_empty()
        {
            let effects: Vec<_> = stew
                .iter()
                .map(|(id, duration)| PotionEffectInstance::new(id.as_str(), *duration, 0))
                .collect();
            out.extend(potion_lines(&effects, 1.0, tick_rate));
        }
        return;
    }
    let Some(consumable) = crate::items::component::<comp::Consumable>(data) else {
        return;
    };
    let mut effects = Vec::new();
    let mut chances = Vec::new();
    for effect in &consumable.on_consume_effects {
        let ConsumeEffect::ApplyEffects {
            effects: list,
            probability,
        } = effect
        else {
            continue;
        };
        for instance in list {
            let id = instance.id.to_str();
            effects.push(PotionEffectInstance {
                effect: id.strip_prefix("minecraft:").unwrap_or(id).to_owned(),
                details: instance.details.clone(),
            });
            chances.push(*probability);
        }
    }
    if effects.is_empty() {
        return;
    }
    let mut rows = potion_lines(&effects, 1.0, tick_rate);
    for (row, chance) in rows.iter_mut().zip(chances) {
        if let Some(text) = chance_text(chance) {
            let style = row.first().map(|s| s.style).unwrap_or_default();
            row.push(Span { text, style });
        }
    }
    out.extend(rows);
}

fn chance_text(probability: f32) -> Option<String> {
    (probability < 1.0).then(|| format!(" ({}%)", format_modifier(f64::from(probability) * 100.0)))
}

const MOB_BUCKETS: [&str; 6] = [
    "axolotl_bucket",
    "cod_bucket",
    "pufferfish_bucket",
    "salmon_bucket",
    "tadpole_bucket",
    "tropical_fish_bucket",
];

pub(crate) fn is_mob_bucket(item: &str) -> bool {
    MOB_BUCKETS.contains(&item)
}

pub(crate) fn bucket_rows(item: &str, data: &ItemStackData, out: &mut Vec<Line>) {
    if !is_mob_bucket(item) {
        return;
    }
    let gray = |text: String| vec![Span::colored(text, GRAY)];
    if let Some(variant) = crate::items::component::<comp::AxolotlVariant>(data) {
        out.push(gray(crate::lang::trf!(
            "better_tooltips.bucket.variant",
            axolotl_name(variant.variant)
        )));
    }
    if let Some(size) = crate::items::component::<SalmonSize>(data) {
        out.push(gray(crate::lang::trf!(
            "better_tooltips.bucket.size",
            salmon_size_name(*size)
        )));
    }
    if let Some(bucket) = crate::items::component::<comp::BucketEntityData>(data) {
        let tag = &bucket.entity;
        if item == "axolotl_bucket" && tag.int("Age").is_some_and(|age| age < 0) {
            out.push(gray(
                crate::lang::tr!("better_tooltips.bucket.baby").to_owned(),
            ));
        }
        if let Some(health) = tag.float("Health") {
            out.push(gray(crate::lang::trf!(
                "better_tooltips.bucket.health",
                format_modifier(f64::from(health))
            )));
        }
    }
}

fn axolotl_name(variant: AxolotlVariantKind) -> &'static str {
    match variant {
        AxolotlVariantKind::Lucy => crate::lang::tr!("better_tooltips.axolotl.lucy"),
        AxolotlVariantKind::Wild => crate::lang::tr!("better_tooltips.axolotl.wild"),
        AxolotlVariantKind::Gold => crate::lang::tr!("better_tooltips.axolotl.gold"),
        AxolotlVariantKind::Cyan => crate::lang::tr!("better_tooltips.axolotl.cyan"),
        AxolotlVariantKind::Blue => crate::lang::tr!("better_tooltips.axolotl.blue"),
    }
}

fn salmon_size_name(size: SalmonSize) -> &'static str {
    match size {
        SalmonSize::Small => crate::lang::tr!("better_tooltips.salmon.small"),
        SalmonSize::Medium => crate::lang::tr!("better_tooltips.salmon.medium"),
        SalmonSize::Large => crate::lang::tr!("better_tooltips.salmon.large"),
    }
}

pub(crate) fn hint_row(key: Bound) -> Line {
    let template = crate::lang::tr!("better_tooltips.hold_to_preview");
    let (before, after) = template.split_once("%s").unwrap_or((template, ""));
    vec![
        Span::new(before),
        Span::colored(key.display_name(), YELLOW),
        Span::new(after),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_chance_under_one_is_printed() {
        assert_eq!(chance_text(1.0), None);
        assert_eq!(chance_text(0.3).as_deref(), Some(" (30%)"));
        assert_eq!(chance_text(0.8).as_deref(), Some(" (80%)"));
        assert_eq!(chance_text(0.125).as_deref(), Some(" (12.5%)"));
        assert_eq!(chance_text(0.0).as_deref(), Some(" (0%)"));
        assert_eq!(chance_text(f32::NAN), None);
    }

    #[test]
    fn food_effects_carry_their_chance() {
        use azalea_registry::builtin::ItemKind;
        let row_text = |kind: ItemKind, item: &'static str| {
            let data = ItemStackData::new(kind, 1);
            let stack = SlotStack {
                item,
                count: 1,
                ..Default::default()
            };
            let mut out = Vec::new();
            effect_rows(&stack, &data, false, 20.0, &mut out);
            out.iter()
                .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
        };
        let apple = row_text(ItemKind::GoldenApple, "golden_apple");
        assert!(apple.len() >= 2, "{apple:?}");
        assert!(
            !apple[0].contains('%') && !apple[1].contains('%'),
            "{apple:?}"
        );
        let flesh = row_text(ItemKind::RottenFlesh, "rotten_flesh");
        assert_eq!(flesh.len(), 1, "{flesh:?}");
        assert!(flesh[0].ends_with(" (80%)"), "{flesh:?}");
    }

    #[test]
    fn hint_names_the_key() {
        let row = hint_row(Bound::Unbound);
        assert_eq!(row.len(), 3);
        assert_eq!(row[1].style.color, YELLOW);
        assert_eq!(row[0].text, "Hold ");
    }
}
