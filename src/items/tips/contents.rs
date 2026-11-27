mod firework;
mod lookup;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::str::FromStr;

use azalea_chat::FormattedText;
use azalea_inventory::components::{
    self as comp, AdventureModePredicate, DataComponentTrait, DyeColor, TropicalFishPattern,
};
use azalea_inventory::{ItemStack, ItemStackData};
use azalea_registry::builtin::{EntityKind, ItemKind};
use azalea_registry::{DataRegistry, Holder, HolderSet};

use super::{ContainerRows, Ctx, Line, TipAt};
use crate::gui::tooltip::{styled_hover_name, translate};
use crate::session::SlotStack;
use crate::text::{Span, Style};

const GRAY: u32 = crate::text::GRAY;
const DARK_GRAY: u32 = crate::text::DARK_GRAY;
const RED: u32 = crate::text::RED;
const BLUE: u32 = crate::text::FORMAT_COLORS[9];

pub(super) fn push(at: TipAt, data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    match at {
        TipAt::BeforeBook => before_book(data, ctx, out),
        TipAt::BeforePotion => {
            if let Some(c) = shown::<comp::ChargedProjectiles>(data) {
                projectile_rows(&c.items, ctx, out);
            }
            if let Some(c) = shown::<comp::Fireworks>(data) {
                firework::fireworks_rows(&c, out);
            }
            if let Some(c) = shown::<comp::FireworkExplosion>(data) {
                firework::explosion_rows(&c, out);
            }
        }
        TipAt::BeforeLore => {
            if let Some(c) = shown::<comp::Profile>(data) {
                profile_rows(&c, out);
            }
        }
        TipAt::AfterLore => {
            if let Some(c) = shown::<comp::SulfurCubeContent>(data) {
                let name = hover_name(&c.absorbed_block_item_stack, ctx);
                out.push(translate_with(
                    "entity.minecraft.sulfur_cube.content",
                    &[name],
                    gray_italic(),
                ));
            }
        }
        TipAt::BeforeAdvanced => before_advanced(data, ctx, out),
        TipAt::ItemHover | TipAt::BeforeEnchantments => {}
    }
}

fn before_book(data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    if let Some(pattern) = shown::<TropicalFishPattern>(data) {
        let base = crate::items::component::<comp::TropicalFishBaseColor>(data)
            .map_or(DyeColor::White, |c| c.color);
        let over = crate::items::component::<comp::TropicalFishPatternColor>(data)
            .map_or(DyeColor::White, |c| c.color);
        tropical_fish_rows(*pattern, base, over, out);
    }
    if super::shows_component::<comp::Instrument>(data)
        && let Some(description) = instrument_description(data, ctx)
    {
        out.push(crate::client::chat_text::to_spans_styled(
            &description,
            Style::colored(GRAY),
        ));
    }
    if let Some(id) = shown::<comp::MapId>(data) {
        map_rows(id.id, crate::gui::map_view::meta(id.id), data, ctx, out);
    }
    if let Some(bees) = shown::<comp::Bees>(data) {
        out.push(gray(translate(
            "container.beehive.bees",
            &[bees.occupants.len().to_string(), "3".to_owned()],
        )));
    }
    if shown::<comp::ContainerLoot>(data).is_some() {
        out.push(vec![Span::new(translate(
            "item.container.loot_table.unknown",
            &[],
        ))]);
    }
    if let Some(container) = shown::<comp::Container>(data) {
        let name = |stack: &ItemStack| hover_name(stack, ctx);
        match ctx.containers {
            ContainerRows::Vanilla => container_rows(&container.items, name, out),
            ContainerRows::Compact => compact_container_rows(&container.items, name, out),
            ContainerRows::Hidden => {}
        }
    }
    if let Some(layers) = shown::<comp::BannerPatterns>(data) {
        for layer in layers.patterns.iter().take(6) {
            if let Some(prefix) = lookup::banner_translation_key(&layer.pattern, ctx.registries) {
                let color = crate::util::dye::by_id(layer.color.max(0) as usize).name;
                out.push(gray(translate(&format!("{prefix}.{color}"), &[])));
            }
        }
    }
    if let Some(pot) = shown::<comp::PotDecorations>(data) {
        pot_rows(&pot.items, item_name, out);
    }
}

fn before_advanced(data: &ItemStackData, ctx: &Ctx, out: &mut Vec<Line>) {
    if let Some(state) = shown::<comp::BlockState>(data)
        && let Some(level) = state
            .properties
            .get("honey_level")
            .and_then(|v| honey_level(v.as_str()))
    {
        out.push(gray(translate(
            "container.beehive.honey",
            &[level.to_string(), "5".to_owned()],
        )));
    }
    if let Some(entity) = shown::<comp::EntityData>(data)
        && ctx.peaceful
        && !allowed_in_peaceful(entity.kind)
    {
        out.push(vec![Span::colored(
            translate("item.spawn_egg.peaceful", &[]),
            RED,
        )]);
    }
    if matches!(data.kind, ItemKind::Spawner | ItemKind::TrialSpawner)
        && super::shows_component::<comp::BlockEntityData>(data)
    {
        let block_entity = crate::items::component::<comp::BlockEntityData>(data);
        spawner_rows(block_entity.as_deref().map(|b| &b.data), out);
    }
    if let Some(c) = shown::<comp::CanBreak>(data) {
        predicate_rows("item.canBreak", &c.predicate, out);
    }
    if let Some(c) = shown::<comp::CanPlaceOn>(data) {
        predicate_rows("item.canPlace", &c.predicate, out);
    }
}

fn shown<T: DataComponentTrait>(data: &ItemStackData) -> Option<Cow<'_, T>> {
    if super::shows_component::<T>(data) {
        crate::items::component::<T>(data)
    } else {
        None
    }
}

fn gray(text: String) -> Line {
    vec![Span::colored(text, GRAY)]
}

fn gray_italic() -> Style {
    Style {
        italic: true,
        ..Style::colored(GRAY)
    }
}

fn tint(spans: impl IntoIterator<Item = Span>, style: Style) -> impl Iterator<Item = Span> {
    spans.into_iter().map(move |mut span| {
        if span.style.is_default() {
            span.style.color = style.color;
        }
        span.style.italic |= style.italic;
        span
    })
}

const MARK: char = '\u{E000}';

fn translate_with(key: &str, args: &[Line], style: Style) -> Line {
    let marks: Vec<String> = (0..args.len())
        .map(|i| format!("{MARK}{i}{MARK}"))
        .collect();
    splice(&translate(key, &marks), args, style)
}

fn splice(text: &str, args: &[Line], style: Style) -> Line {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(MARK) {
        let after = &rest[start + MARK.len_utf8()..];
        let Some(end) = after.find(MARK) else { break };
        let Ok(index) = after[..end].parse::<usize>() else {
            break;
        };
        push_text(&mut out, &rest[..start], style);
        if let Some(arg) = args.get(index) {
            out.extend(tint(arg.iter().cloned(), style));
        }
        rest = &after[end + MARK.len_utf8()..];
    }
    push_text(&mut out, rest, style);
    out
}

fn push_text(out: &mut Line, text: &str, style: Style) {
    if !text.is_empty() {
        out.push(Span {
            text: text.to_owned(),
            style,
        });
    }
}

fn hover_name(stack: &ItemStack, ctx: &Ctx) -> Line {
    let slot = crate::play::inventory_bridge::slot_stack(stack, ctx.registries, false);
    match &slot.extra.custom_name {
        Some(spans) => spans.clone(),
        None => styled_hover_name(&slot)
            .into_iter()
            .map(|span| Span::new(span.text))
            .collect(),
    }
}

fn item_name(kind: ItemKind) -> String {
    let slot = SlotStack {
        item: kind.to_str().trim_start_matches("minecraft:"),
        ..SlotStack::default()
    };
    styled_hover_name(&slot)
        .into_iter()
        .map(|span| span.text)
        .collect()
}

fn display_name(stack: &ItemStack, ctx: &Ctx) -> Line {
    let slot = crate::play::inventory_bridge::slot_stack(stack, ctx.registries, false);
    let rarity = crate::gui::tooltip::rarity(&slot, stack.as_present()).color();
    translate_with(
        "chat.square_brackets",
        &[styled_hover_name(&slot)],
        Style::colored(rarity),
    )
}

fn pattern_name(pattern: TropicalFishPattern) -> &'static str {
    match pattern {
        TropicalFishPattern::Kob => "kob",
        TropicalFishPattern::Sunstreak => "sunstreak",
        TropicalFishPattern::Snooper => "snooper",
        TropicalFishPattern::Dasher => "dasher",
        TropicalFishPattern::Brinely => "brinely",
        TropicalFishPattern::Spotty => "spotty",
        TropicalFishPattern::Flopper => "flopper",
        TropicalFishPattern::Stripey => "stripey",
        TropicalFishPattern::Glitter => "glitter",
        TropicalFishPattern::Blockfish => "blockfish",
        TropicalFishPattern::Betty => "betty",
        TropicalFishPattern::Clayfish => "clayfish",
    }
}

const COMMON_VARIANTS: [(TropicalFishPattern, DyeColor, DyeColor); 22] = {
    use DyeColor as D;
    use TropicalFishPattern as P;
    [
        (P::Stripey, D::Orange, D::Gray),
        (P::Flopper, D::Gray, D::Gray),
        (P::Flopper, D::Gray, D::Blue),
        (P::Clayfish, D::White, D::Gray),
        (P::Sunstreak, D::Blue, D::Gray),
        (P::Kob, D::Orange, D::White),
        (P::Spotty, D::Pink, D::LightBlue),
        (P::Blockfish, D::Purple, D::Yellow),
        (P::Clayfish, D::White, D::Red),
        (P::Spotty, D::White, D::Yellow),
        (P::Glitter, D::White, D::Gray),
        (P::Clayfish, D::White, D::Orange),
        (P::Dasher, D::Cyan, D::Pink),
        (P::Brinely, D::Lime, D::LightBlue),
        (P::Betty, D::Red, D::White),
        (P::Snooper, D::Gray, D::Red),
        (P::Blockfish, D::Red, D::White),
        (P::Flopper, D::White, D::Yellow),
        (P::Kob, D::Red, D::White),
        (P::Sunstreak, D::Gray, D::White),
        (P::Dasher, D::Cyan, D::Yellow),
        (P::Flopper, D::Yellow, D::Yellow),
    ]
};

fn tropical_fish_rows(
    pattern: TropicalFishPattern,
    base: DyeColor,
    over: DyeColor,
    out: &mut Vec<Line>,
) {
    let style = gray_italic();
    let row = |text: String| vec![Span { text, style }];
    if let Some(index) = COMMON_VARIANTS
        .iter()
        .position(|v| *v == (pattern, base, over))
    {
        out.push(row(translate(
            &format!("entity.minecraft.tropical_fish.predefined.{index}"),
            &[],
        )));
        return;
    }
    out.push(row(translate(
        &format!(
            "entity.minecraft.tropical_fish.type.{}",
            pattern_name(pattern)
        ),
        &[],
    )));
    let color = |c: DyeColor| {
        translate(
            &format!(
                "color.minecraft.{}",
                crate::util::dye::by_id(c as usize).name
            ),
            &[],
        )
    };
    let mut text = color(base);
    if base != over {
        text.push_str(", ");
        text.push_str(&color(over));
    }
    out.push(row(text));
}

fn instrument_description(data: &ItemStackData, ctx: &Ctx) -> Option<FormattedText> {
    if let Some(component) = data.component_patch.get::<comp::Instrument>() {
        return match &component.value {
            Holder::Direct(inline) => Some(inline.description.clone()),
            Holder::Reference(instrument) => {
                let (name, entry) = lookup::registry_entry(
                    ctx.registries?,
                    azalea_registry::data::Instrument::NAME,
                    instrument.protocol_id(),
                )?;
                lookup::instrument_description(name, entry)
            }
        };
    }
    crate::items::component::<comp::Instrument>(data)?;
    if data.kind != ItemKind::GoatHorn {
        return None;
    }
    let name = azalea::Identifier::new("ponder_goat_horn");
    let entry = ctx.registries.and_then(|r| {
        lookup::registry_entry_by_key(r, azalea_registry::data::Instrument::NAME, &name)
    });
    lookup::instrument_description(&name, entry)
}

const MAP_MAX_SCALE: u8 = 4;

fn map_rows(
    id: i32,
    map: Option<crate::gui::map_view::MapMeta>,
    data: &ItemStackData,
    ctx: &Ctx,
    out: &mut Vec<Line>,
) {
    let Some(map) = map else {
        out.push(gray(translate("filled_map.unknown", &[])));
        return;
    };
    let post = crate::items::component::<comp::MapPostProcessing>(data).map(|p| *p);
    if crate::items::component::<comp::CustomName>(data).is_none() && post.is_none() {
        out.push(gray(translate("filled_map.id", &[id.to_string()])));
    }
    if map.locked || post == Some(comp::MapPostProcessing::Lock) {
        out.push(gray(translate("filled_map.locked", &[])));
    }
    if ctx.advanced {
        let add = u8::from(post == Some(comp::MapPostProcessing::Scale));
        let scale = map.scale.saturating_add(add).min(MAP_MAX_SCALE);
        out.push(gray(translate(
            "filled_map.scale",
            &[(1u32 << scale).to_string()],
        )));
        out.push(gray(translate(
            "filled_map.level",
            &[scale.to_string(), MAP_MAX_SCALE.to_string()],
        )));
    }
}

fn container_rows(items: &[ItemStack], name: impl Fn(&ItemStack) -> Line, out: &mut Vec<Line>) {
    let mut lines = 0usize;
    let mut held = 0usize;
    for stack in items.iter().filter(|s| s.is_present()) {
        held += 1;
        if lines <= 4 {
            lines += 1;
            let count = vec![Span::new(stack.count().to_string())];
            out.push(translate_with(
                "item.container.item_count",
                &[name(stack), count],
                Style::default(),
            ));
        }
    }
    if held > lines {
        out.push(vec![Span {
            text: translate("item.container.more_items", &[(held - lines).to_string()]),
            style: Style {
                italic: true,
                ..Style::default()
            },
        }]);
    }
}

fn compact_container_rows(
    items: &[ItemStack],
    name: impl Fn(&ItemStack) -> Line,
    out: &mut Vec<Line>,
) {
    let mut merged: Vec<(ItemKind, Line, u64)> = Vec::new();
    for stack in items {
        let ItemStack::Present(data) = stack else {
            continue;
        };
        let line = name(stack);
        let count = data.count.max(0) as u64;
        match merged
            .iter_mut()
            .find(|(kind, l, _)| *kind == data.kind && *l == line)
        {
            Some(entry) => entry.2 += count,
            None => merged.push((data.kind, line, count)),
        }
    }
    merged.sort_by(|a, b| b.2.cmp(&a.2));
    for (_, line, count) in merged.iter().take(5) {
        out.push(translate_with(
            "item.container.item_count",
            &[line.clone(), vec![Span::new(count.to_string())]],
            Style::default(),
        ));
    }
    if merged.len() > 5 {
        out.push(vec![Span {
            text: translate(
                "item.container.more_items",
                &[(merged.len() - 5).to_string()],
            ),
            style: Style {
                italic: true,
                ..Style::default()
            },
        }]);
    }
}

fn pot_rows(items: &[ItemKind], name: impl Fn(ItemKind) -> String, out: &mut Vec<Line>) {
    let side = |i: usize| items.get(i).copied().filter(|k| *k != ItemKind::Brick);
    if (0..4).all(|i| side(i).is_none()) {
        return;
    }
    out.push(Vec::new());
    for i in [3, 1, 2, 0] {
        out.push(gray(name(side(i).unwrap_or(ItemKind::Brick))));
    }
}

const ALL_AT: [TipAt; 7] = [
    TipAt::ItemHover,
    TipAt::BeforeBook,
    TipAt::BeforePotion,
    TipAt::BeforeEnchantments,
    TipAt::BeforeLore,
    TipAt::AfterLore,
    TipAt::BeforeAdvanced,
];

fn projectile_rows(items: &[ItemStack], ctx: &Ctx, out: &mut Vec<Line>) {
    let mut runs: Vec<(&ItemStack, usize)> = Vec::new();
    for stack in items {
        if let Some(last) = runs.last_mut()
            && last.0 == stack
        {
            last.1 += 1;
        } else {
            runs.push((stack, 1));
        }
    }
    let nested = Ctx {
        registries: ctx.registries,
        advanced: false,
        attack_speed_base: None,
        peaceful: ctx.peaceful,
        containers: ctx.containers,
    };
    for (stack, count) in runs {
        let name = display_name(stack, ctx);
        out.push(if count == 1 {
            translate_with(
                "item.minecraft.crossbow.projectile.single",
                &[name],
                Style::default(),
            )
        } else {
            let count = vec![Span::new(count.to_string())];
            translate_with(
                "item.minecraft.crossbow.projectile.multiple",
                &[count, name],
                Style::default(),
            )
        });
        let ItemStack::Present(projectile) = stack else {
            continue;
        };
        let mut rows = Vec::new();
        for at in ALL_AT {
            super::lines(at, projectile, &nested, &mut rows);
        }
        for row in rows {
            let mut line = vec![Span::colored("  ", GRAY)];
            line.extend(tint(row, Style::colored(GRAY)));
            out.push(line);
        }
    }
}

fn profile_rows(profile: &comp::Profile, out: &mut Vec<Line>) {
    if let comp::PartialOrFullProfile::Partial(partial) = &*profile.unpack
        && partial.properties.map.is_empty()
        && partial.name.is_some() != partial.id.is_some()
    {
        out.push(gray(translate("component.profile.dynamic", &[])));
    }
}

fn honey_level(value: &str) -> Option<i32> {
    value.parse::<i32>().ok().filter(|v| (0..=5).contains(v))
}

const NOT_IN_PEACEFUL: [&str; 38] = [
    "blaze",
    "bogged",
    "breeze",
    "cave_spider",
    "creaking",
    "creeper",
    "drowned",
    "elder_guardian",
    "enderman",
    "endermite",
    "evoker",
    "ghast",
    "giant",
    "guardian",
    "hoglin",
    "husk",
    "illusioner",
    "magma_cube",
    "parched",
    "phantom",
    "piglin_brute",
    "pillager",
    "ravager",
    "silverfish",
    "skeleton",
    "slime",
    "spider",
    "stray",
    "vex",
    "vindicator",
    "warden",
    "witch",
    "wither",
    "wither_skeleton",
    "zoglin",
    "zombie",
    "zombie_villager",
    "zombified_piglin",
];

fn allowed_in_peaceful(kind: EntityKind) -> bool {
    let id = kind.to_str().trim_start_matches("minecraft:");
    !NOT_IN_PEACEFUL.contains(&id)
}

fn spawner_rows(data: Option<&simdnbt::owned::NbtCompound>, out: &mut Vec<Line>) {
    let kind = data
        .and_then(|d| d.compound("SpawnData")?.compound("entity")?.string("id"))
        .and_then(|id| EntityKind::from_str(&id.to_str()).ok());
    match kind {
        Some(kind) => {
            let key = format!("entity.{}", kind.to_str().replace(':', "."));
            out.push(gray(translate(&key, &[])));
        }
        None => {
            out.push(Vec::new());
            out.push(gray(translate("block.minecraft.spawner.desc1", &[])));
            out.push(vec![
                Span::new(" "),
                Span::colored(translate("block.minecraft.spawner.desc2", &[]), BLUE),
            ]);
        }
    }
}

fn predicate_rows(header: &str, predicate: &AdventureModePredicate, out: &mut Vec<Line>) {
    out.push(Vec::new());
    out.push(gray(translate(header, &[])));
    if predicate.predicates.iter().any(|p| p.blocks.is_none()) {
        out.push(gray(translate("item.canUse.unknown", &[])));
        return;
    }
    let mut blocks: Vec<String> = Vec::new();
    for set in predicate
        .predicates
        .iter()
        .filter_map(|p| p.blocks.as_ref())
    {
        match set {
            HolderSet::Direct { contents } => {
                blocks.extend(contents.iter().map(|b| b.to_str().to_owned()));
            }
            HolderSet::Named { key, .. } => blocks.extend(lookup::block_tag(key).iter().cloned()),
        }
    }
    let mut seen: Vec<&str> = Vec::with_capacity(blocks.len());
    for block in &blocks {
        if seen.contains(&block.as_str()) {
            continue;
        }
        seen.push(block);
        let id = azalea::Identifier::new(block.as_str());
        let key = format!("block.{}.{}", id.namespace(), id.path().replace('/', "."));
        out.push(vec![Span::colored(translate(&key, &[]), DARK_GRAY)]);
    }
}
