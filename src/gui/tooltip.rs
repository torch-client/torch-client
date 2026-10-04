use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::generated_items::item;
use crate::gui::painter::Painter;
use crate::items::potions;
use crate::play::mob_effects;
use crate::session::SlotStack;
use crate::text::{Span, Style};

const MOUSE_OFFSET: f32 = 12.0;
const PADDING: f32 = 3.0;
const MARGIN: f32 = 9.0;
const ROW_HEIGHT: f32 = 10.0;
const TITLE_GAP: f32 = 2.0;

const GRAY: u32 = 0xAAAAAA;
const DARK_GRAY: u32 = 0x555555;
const BLUE: u32 = 0x5555FF;
const RED: u32 = 0xFF5555;
const DARK_PURPLE: u32 = 0xAA00AA;
const WHITE: u32 = 0xFFFFFF;

static LANG: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let mut out = HashMap::new();
    for text in crate::platform::assets::read_stack(crate::assets_root().join("lang/en_us.json")) {
        let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&text) else {
            continue;
        };
        out.extend(
            map.into_iter()
                .filter_map(|(k, v)| Some((k, v.as_str()?.to_owned()))),
        );
    }
    out
});

fn tag_values(name: &str) -> Vec<String> {
    crate::util::datapack::entry("tags/enchantment", name)
        .and_then(|v| {
            let values = v.get("values")?.as_array()?;
            Some(
                values
                    .iter()
                    .filter_map(crate::util::datapack::bare_id)
                    .map(str::to_owned)
                    .collect(),
            )
        })
        .unwrap_or_default()
}

static CURSES: LazyLock<HashSet<String>> =
    LazyLock::new(|| tag_values("curse").into_iter().collect());

static TOOLTIP_ORDER: LazyLock<Vec<String>> = LazyLock::new(|| tag_values("tooltip_order"));

static MAX_LEVELS: LazyLock<HashMap<String, i32>> = LazyLock::new(|| {
    crate::util::datapack::entries("enchantment")
        .into_iter()
        .map(|(id, v)| {
            let level = v.get("max_level").and_then(|l| l.as_i64()).unwrap_or(1) as i32;
            (id, level)
        })
        .collect()
});

pub(crate) fn all_enchantments() -> Vec<(&'static str, i32)> {
    MAX_LEVELS
        .iter()
        .map(|(id, lvl)| (id.as_str(), *lvl))
        .collect()
}

pub(crate) fn translate_once(
    key: &'static str,
    cell: &'static std::sync::OnceLock<String>,
) -> &'static str {
    cell.get_or_init(|| translate(key, &[]))
}

macro_rules! label {
    ($key:literal) => {{
        static CELL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        $crate::gui::tooltip::translate_once($key, &CELL)
    }};
}
pub(crate) use label;

pub(crate) fn translate(key: &str, args: &[String]) -> String {
    let template: &str = LANG.get(key).map_or(key, String::as_str);
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let mut next = args.iter();
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let bytes = rest.as_bytes();
        let digits = bytes[1..].iter().take_while(|b| b.is_ascii_digit()).count();
        match (digits, bytes.get(1 + digits)) {
            (1.., Some(b'$')) if bytes.get(2 + digits) == Some(&b's') => {
                let n: usize = rest[1..1 + digits].parse().unwrap_or(0);
                match n.checked_sub(1).and_then(|i| args.get(i)) {
                    Some(a) => out.push_str(a),
                    None => out.push_str(&rest[..3 + digits]),
                }
                rest = &rest[3 + digits..];
            }
            (0, Some(b'%')) => {
                out.push('%');
                rest = &rest[2..];
            }
            (0, Some(b's')) => {
                match next.next() {
                    Some(a) => out.push_str(a),
                    None => out.push_str("%s"),
                }
                rest = &rest[2..];
            }
            _ => {
                out.push('%');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn roman(n: i32) -> String {
    if n <= 0 || n > 3999 {
        return n.to_string();
    }
    const TABLE: [(i32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut left = n;
    let mut out = String::new();
    for (value, sym) in TABLE {
        while left >= value {
            out.push_str(sym);
            left -= value;
        }
    }
    out
}

fn format_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.1} MB", b / MB)
    }
}

fn inherit(spans: &[Span], color: u32, italic: bool) -> Vec<Span> {
    spans
        .iter()
        .map(|s| {
            let mut style = s.style;
            if style.is_default() {
                style.color = color;
            }
            style.italic |= italic;
            Span {
                text: s.text.clone(),
                style,
            }
        })
        .collect()
}

fn plain(text: String, color: u32) -> Vec<Span> {
    vec![Span {
        text,
        style: Style::colored(color),
    }]
}

fn ordered_enchantments(list: &[(String, i32)]) -> Vec<(String, i32)> {
    let order = &*TOOLTIP_ORDER;
    let mut out: Vec<(String, i32)> = Vec::with_capacity(list.len());
    for id in order {
        if let Some(e) = list.iter().find(|(n, _)| n == id) {
            out.push(e.clone());
        }
    }
    for e in list {
        if !order.contains(&e.0) {
            out.push(e.clone());
        }
    }
    out
}

fn enchantment_line(id: &str, level: i32) -> Vec<Span> {
    let name = translate(&format!("enchantment.minecraft.{id}"), &[]);
    let max = MAX_LEVELS.get(id).copied().unwrap_or(1);
    let text = if level != 1 || max != 1 {
        format!("{name} {}", roman(level))
    } else {
        name
    };
    let color = if CURSES.contains(id) { RED } else { GRAY };
    plain(text, color)
}

fn potion_name(stack: &SlotStack) -> Option<String> {
    let contents = stack.potion.as_ref()?;
    if !potions::is_potion_item(stack.item) {
        return None;
    }
    Some(translate(&contents.name_key(stack.item), &[]))
}

fn potion_lines(contents: &potions::PotionContents) -> Vec<Vec<Span>> {
    let mut out: Vec<Vec<Span>> = Vec::new();
    let effects = contents.all_effects();

    if effects.is_empty() {
        out.push(plain(translate("effect.none", &[]), GRAY));
        return out;
    }

    let mut modifiers: Vec<(String, f64, u8)> = Vec::new();
    for effect in &effects {
        if let Some(m) = mob_effects::attribute_modifier(&effect.effect, effect.amplifier) {
            modifiers.push(m);
        }
        let name = translate(&format!("effect.minecraft.{}", effect.effect), &[]);
        let mut text = if effect.amplifier > 0 {
            translate(
                "potion.withAmplifier",
                &[
                    name,
                    translate(&format!("potion.potency.{}", effect.amplifier), &[]),
                ],
            )
        } else {
            name
        };
        if effect.duration < 0 || effect.duration > 20 {
            text = translate(
                "potion.withDuration",
                &[text, mob_effects::format_duration(effect.duration)],
            );
        }
        let color = match mob_effects::category(&effect.effect) {
            mob_effects::Category::Harmful => RED,
            _ => BLUE,
        };
        out.push(plain(text, color));
    }

    if modifiers.is_empty() {
        return out;
    }
    out.push(Vec::new());
    out.push(plain(translate("potion.whenDrank", &[]), DARK_PURPLE));
    for (attribute, amount, operation) in modifiers {
        let display = if operation == 0 {
            amount
        } else {
            amount * 100.0
        };
        let (key, color) = if amount > 0.0 {
            (format!("attribute.modifier.plus.{operation}"), BLUE)
        } else if amount < 0.0 {
            (format!("attribute.modifier.take.{operation}"), RED)
        } else {
            continue;
        };
        let text = translate(
            &key,
            &[format_modifier(display.abs()), translate(&attribute, &[])],
        );
        out.push(plain(text, color));
    }
    out
}

fn book_lines(book: &crate::session::Book) -> impl Iterator<Item = Vec<Span>> {
    let author = (!book.author.trim().is_empty())
        .then(|| plain(translate("book.byAuthor", &[book.author.clone()]), GRAY));
    let generation = plain(
        translate(&format!("book.generation.{}", book.generation), &[]),
        GRAY,
    );
    author.into_iter().chain(std::iter::once(generation))
}

fn format_modifier(value: f64) -> String {
    let text = format!("{:.2}", value);
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

pub(crate) fn styled_hover_name(stack: &SlotStack) -> Vec<Span> {
    let def = item(stack.item);
    let rarity = def.map(|d| d.rarity.color()).unwrap_or(WHITE);
    match &stack.custom_name {
        Some(spans) => inherit(spans, rarity, true),
        None => {
            let name = book_title(stack)
                .or_else(|| potion_name(stack))
                .or_else(|| def.map(|d| d.name.to_string()))
                .unwrap_or_else(|| stack.item.to_string());
            plain(name, rarity)
        }
    }
}

fn book_title(stack: &SlotStack) -> Option<String> {
    let book = stack.book.as_deref()?;
    (book.signed && !book.title.trim().is_empty()).then(|| book.title.clone())
}

fn build_lines(stack: &SlotStack, advanced: bool) -> Vec<Vec<Span>> {
    let mut out: Vec<Vec<Span>> = Vec::new();

    out.push(styled_hover_name(stack));

    if let Some(b) = &stack.book
        && b.signed
    {
        out.extend(book_lines(b));
    }

    if let Some(contents) = &stack.potion
        && potions::is_potion_item(stack.item)
    {
        out.extend(potion_lines(contents));
    }

    for (id, level) in ordered_enchantments(&stack.enchantments) {
        out.push(enchantment_line(&id, level));
    }

    for line in &stack.lore {
        out.push(inherit(line, DARK_PURPLE, true));
    }

    if stack.unbreakable {
        out.push(plain(translate("item.unbreakable", &[]), BLUE));
    }

    if advanced {
        if stack.max_damage > 0 && stack.damage > 0 {
            let remaining = stack.max_damage.saturating_sub(stack.damage);
            let text = translate(
                "item.durability",
                &[remaining.to_string(), stack.max_damage.to_string()],
            );
            out.push(plain(text, WHITE));
        }
        out.push(plain(format!("minecraft:{}", stack.item), DARK_GRAY));
        if stack.component_count > 0 {
            let text = translate("item.components", &[stack.component_count.to_string()]);
            out.push(plain(text, DARK_GRAY));
        }
        out.push(plain(
            format!("Size: {}", format_bytes(stack.nbt_bytes)),
            DARK_GRAY,
        ));
    }

    out
}

fn box_height(rows: usize) -> f32 {
    let start = if rows == 1 { -TITLE_GAP } else { 0.0 };
    start + ROW_HEIGHT * rows as f32
}

fn position(mx: f32, my: f32, vw: f32, vh: f32, w: f32, h: f32) -> (f32, f32) {
    let mut x = mx.floor() + MOUSE_OFFSET;
    let mut y = my.floor() - MOUSE_OFFSET;
    if x + w > vw {
        x = (x - 2.0 * MOUSE_OFFSET - w).max(4.0);
    }
    let padded = h + PADDING;
    if y + padded > vh {
        y = vh - padded;
        y = y.max(0.0);
    }
    (x, y)
}

pub(in crate::gui) fn position_flat(mx: f32, my: f32, vw: f32, w: f32) -> (f32, f32) {
    let padded = w + 2.0 * PADDING;
    let x = if mx + MOUSE_OFFSET + padded > vw {
        (mx - MOUSE_OFFSET - padded).max(0.0)
    } else {
        mx + MOUSE_OFFSET
    };
    (x.floor(), (my - MOUSE_OFFSET).floor())
}

struct LineCache {
    stack: SlotStack,
    advanced: bool,
    lines: Vec<Vec<Span>>,
}

thread_local! {
    static LINES: RefCell<Option<LineCache>> = const { RefCell::new(None) };
}

pub fn draw(
    p: &mut Painter,
    stack: &SlotStack,
    mx: f32,
    my: f32,
    vw: f32,
    vh: f32,
    advanced: bool,
) {
    if stack.is_empty() {
        return;
    }
    LINES.with_borrow_mut(|cache| {
        let hit = cache
            .as_ref()
            .is_some_and(|c| c.advanced == advanced && c.stack == *stack);
        if !hit {
            *cache = Some(LineCache {
                stack: stack.clone(),
                advanced,
                lines: build_lines(stack, advanced),
            });
        }
        if let Some(c) = cache.as_ref() {
            draw_lines(p, &c.lines, mx, my, vw, vh);
        }
    });
}

pub(crate) fn draw_lines(p: &mut Painter, lines: &[Vec<Span>], mx: f32, my: f32, vw: f32, vh: f32) {
    if lines.is_empty() {
        return;
    }

    let w = lines
        .iter()
        .map(|l| p.atlas.font.width(l).ceil())
        .fold(0.0f32, f32::max);
    let h = box_height(lines.len());
    let (x, y) = position(mx, my, vw, vh, w, h);

    let (bx, by) = (x - PADDING - MARGIN, y - PADDING - MARGIN);
    let (bw, bh) = (w + 2.0 * (PADDING + MARGIN), h + 2.0 * (PADDING + MARGIN));
    p.sprite("tooltip/background", bx, by, bw, bh);
    p.sprite("tooltip/frame", bx, by, bw, bh);

    let mut row_y = y;
    for (i, line) in lines.iter().enumerate() {
        p.text(line, x, row_y, true);
        row_y += ROW_HEIGHT + if i == 0 { TITLE_GAP } else { 0.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(line: &[Span]) -> String {
        line.iter().map(|s| s.text.as_str()).collect()
    }

    fn lines_text(stack: &SlotStack, advanced: bool) -> Vec<String> {
        build_lines(stack, advanced)
            .iter()
            .map(|l| text_of(l))
            .collect()
    }

    #[test]
    fn translate_substitutes_both_placeholder_forms() {
        let two = |a: &str, b: &str| vec![a.to_string(), b.to_string()];
        assert_eq!(
            translate("book.pageIndicator", &two("1", "3")),
            "Page 1 of 3"
        );
        assert_eq!(
            translate("book.byAuthor", &["Steve".to_string()]),
            "by Steve"
        );
        assert_eq!(translate("book.generation.0", &[]), "Original");
        assert_eq!(translate("no.such.key", &[]), "no.such.key");
        assert_eq!(translate("book.pageIndicator", &[]), "Page %1$s of %2$s");
    }

    #[test]
    fn a_signed_book_is_named_by_its_title() {
        let mut stack = SlotStack {
            item: "written_book",
            count: 1,
            book: Some(Box::new(crate::session::Book {
                title: "Ars Goetia".into(),
                author: "Steve".into(),
                signed: true,
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(text_of(&styled_hover_name(&stack)), "Ars Goetia");
        assert!(!styled_hover_name(&stack)[0].style.italic);

        stack.book.as_mut().unwrap().title = "   ".into();
        assert_eq!(text_of(&styled_hover_name(&stack)), "Written Book");

        let draft = SlotStack {
            item: "writable_book",
            count: 1,
            book: Some(Box::new(crate::session::Book::default())),
            ..Default::default()
        };
        assert_eq!(text_of(&styled_hover_name(&draft)), "Book and Quill");
    }

    #[test]
    fn bytes_across_unit_boundaries() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(18), "18 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1434), "1.4 KB");
        assert_eq!(format_bytes(1024 * 1024 - 1), "1024.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(2202009), "2.1 MB");
    }

    #[test]
    fn roman_one_to_ten() {
        let want = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(roman(i as i32 + 1), *w);
        }
    }

    #[test]
    fn roman_agrees_with_lang() {
        for n in 1..=10 {
            let key = format!("enchantment.level.{n}");
            if let Some(v) = LANG.get(&key) {
                assert_eq!(&roman(n), v, "level {n}");
            }
        }
    }

    #[test]
    fn plain_item_is_one_line() {
        let stack = SlotStack {
            item: "stone",
            count: 1,
            ..Default::default()
        };
        assert_eq!(lines_text(&stack, false), vec!["Stone".to_string()]);
        assert_eq!(build_lines(&stack, false)[0][0].style.color, WHITE);
    }

    #[test]
    fn damaged_tool_advanced_and_not() {
        let stack = SlotStack {
            item: "diamond_sword",
            count: 1,
            damage: 100,
            max_damage: 1561,
            component_count: 2,
            nbt_bytes: 18,
            ..Default::default()
        };
        assert_eq!(lines_text(&stack, false), vec!["Diamond Sword".to_string()]);
        assert_eq!(
            lines_text(&stack, true),
            vec![
                "Diamond Sword".to_string(),
                "Durability: 1461 / 1561".to_string(),
                "minecraft:diamond_sword".to_string(),
                "2 component(s)".to_string(),
                "Size: 18 B".to_string(),
            ]
        );
    }

    #[test]
    fn undamaged_tool_hides_durability() {
        let stack = SlotStack {
            item: "diamond_sword",
            count: 1,
            max_damage: 1561,
            nbt_bytes: 4,
            ..Default::default()
        };
        let lines = lines_text(&stack, true);
        assert!(
            !lines.iter().any(|l| l.starts_with("Durability")),
            "{lines:?}"
        );
        assert_eq!(lines.last().unwrap(), "Size: 4 B");
    }

    fn potion_stack(item: &'static str, potion: Option<&str>) -> SlotStack {
        contents_stack(
            item,
            potions::PotionContents {
                potion: potion.map(str::to_string),
                ..Default::default()
            },
        )
    }

    fn contents_stack(item: &'static str, contents: potions::PotionContents) -> SlotStack {
        SlotStack {
            item,
            count: 1,
            potion: Some(contents),
            ..Default::default()
        }
    }

    #[test]
    fn potion_name_comes_from_its_contents() {
        for (item, potion, name) in [
            ("potion", Some("water"), "Water Bottle"),
            (
                "potion",
                Some("long_night_vision"),
                "Potion of Night Vision",
            ),
            ("splash_potion", Some("healing"), "Splash Potion of Healing"),
            (
                "lingering_potion",
                Some("strong_turtle_master"),
                "Lingering Potion of the Turtle Master",
            ),
            ("tipped_arrow", Some("poison"), "Arrow of Poison"),
            ("potion", None, "Uncraftable Potion"),
        ] {
            assert_eq!(lines_text(&potion_stack(item, potion), false)[0], name);
        }
    }

    #[test]
    fn potion_effect_lines() {
        assert_eq!(
            lines_text(&potion_stack("potion", Some("night_vision")), false),
            vec![
                "Potion of Night Vision".to_string(),
                "Night Vision (03:00)".to_string(),
            ]
        );
        assert_eq!(
            lines_text(&potion_stack("potion", Some("strong_regeneration")), false),
            vec![
                "Potion of Regeneration".to_string(),
                "Regeneration II (00:22)".to_string(),
            ]
        );
        assert_eq!(
            lines_text(
                &potion_stack("splash_potion", Some("strong_healing")),
                false
            ),
            vec![
                "Splash Potion of Healing".to_string(),
                "Instant Health II".to_string(),
            ]
        );
        assert_eq!(
            lines_text(&potion_stack("potion", Some("awkward")), false),
            vec!["Awkward Potion".to_string(), "No Effects".to_string()]
        );
    }

    #[test]
    fn potion_attribute_modifiers() {
        let lines = lines_text(&potion_stack("potion", Some("swiftness")), false);
        assert_eq!(
            lines,
            vec![
                "Potion of Swiftness".to_string(),
                "Speed (03:00)".to_string(),
                String::new(),
                "When Applied:".to_string(),
                "+20% Speed".to_string(),
            ]
        );
        let lines = lines_text(&potion_stack("potion", Some("weakness")), false);
        assert_eq!(lines.last().unwrap(), "-4 Attack Damage");
        let lines = lines_text(&potion_stack("potion", Some("turtle_master")), false);
        assert_eq!(lines[1], "Slowness IV (00:20)");
        assert_eq!(lines[2], "Resistance III (00:20)");
        assert_eq!(lines.last().unwrap(), "-60% Speed");
    }

    #[test]
    fn custom_effects_and_name_reach_the_tooltip() {
        let contents = potions::PotionContents {
            potion: Some("swiftness".into()),
            custom_name: Some("luck".into()),
            custom_effects: vec![potions::PotionEffectInstance {
                effect: "poison".into(),
                duration: 200,
                amplifier: 1,
            }],
            ..Default::default()
        };
        let lines = lines_text(&contents_stack("potion", contents), false);
        assert_eq!(
            &lines[..3],
            &[
                "Potion of Luck".to_string(),
                "Speed (03:00)".to_string(),
                "Poison II (00:10)".to_string(),
            ]
        );
        assert_eq!(lines.last().unwrap(), "+20% Speed");
    }

    #[test]
    fn infinite_effect_prints_as_infinite() {
        let contents = potions::PotionContents {
            custom_effects: vec![potions::PotionEffectInstance {
                effect: "night_vision".into(),
                duration: -1,
                amplifier: 0,
            }],
            ..Default::default()
        };
        let lines = lines_text(&contents_stack("potion", contents), false);
        assert_eq!(lines[1], "Night Vision (\u{221e})");
    }

    #[test]
    fn modifier_format_drops_trailing_zeroes() {
        assert_eq!(format_modifier(20.0), "20");
        assert_eq!(format_modifier(0.5), "0.5");
        assert_eq!(format_modifier(1.25), "1.25");
        assert_eq!(format_modifier(100.0), "100");
    }

    #[test]
    fn enchantments_then_lore() {
        let stack = SlotStack {
            item: "diamond_pickaxe",
            count: 1,
            lore: vec![
                crate::text::styled("first", Style::default()),
                crate::text::styled("second", Style::default()),
            ],
            enchantments: vec![("silk_touch".into(), 1), ("efficiency".into(), 5)],
            unbreakable: true,
            ..Default::default()
        };
        assert_eq!(
            lines_text(&stack, false),
            vec![
                "Diamond Pickaxe".to_string(),
                "Silk Touch".to_string(),
                "Efficiency V".to_string(),
                "first".to_string(),
                "second".to_string(),
                "Unbreakable".to_string(),
            ]
        );
        let lines = build_lines(&stack, false);
        assert_eq!(lines[1][0].style.color, GRAY);
        let lore_style = lines[3][0].style;
        assert_eq!(lore_style.color, DARK_PURPLE);
        assert!(lore_style.italic);
        assert_eq!(lines[5][0].style.color, BLUE);
    }

    #[test]
    fn level_one_numeral_rule() {
        assert_eq!(text_of(&enchantment_line("sharpness", 1)), "Sharpness I");
        assert_eq!(text_of(&enchantment_line("mending", 1)), "Mending");
        assert_eq!(enchantment_line("vanishing_curse", 1)[0].style.color, RED);
    }

    #[test]
    fn custom_name_is_italic_and_rarity_coloured() {
        let stack = SlotStack {
            item: "golden_apple",
            count: 1,
            custom_name: Some(crate::text::styled("Snack", Style::default())),
            ..Default::default()
        };
        let title = &build_lines(&stack, false)[0][0];
        assert_eq!(title.text, "Snack");
        assert!(title.style.italic);
        assert_eq!(
            title.style.color,
            item("golden_apple").unwrap().rarity.color()
        );
    }

    #[test]
    fn heights_match_vanilla() {
        assert_eq!(box_height(1), 8.0);
        assert_eq!(box_height(2), 20.0);
        assert_eq!(box_height(5), 50.0);
    }

    #[test]
    fn positioner_normal_case() {
        assert_eq!(
            position(100.0, 100.0, 320.0, 240.0, 60.0, 20.0),
            (112.0, 88.0)
        );
    }

    #[test]
    fn positioner_flips_left_at_right_edge() {
        assert_eq!(position(300.0, 100.0, 320.0, 240.0, 60.0, 20.0).0, 228.0);
    }

    #[test]
    fn positioner_clamps_left_when_flip_overflows() {
        assert_eq!(position(300.0, 100.0, 320.0, 240.0, 400.0, 20.0).0, 4.0);
    }

    #[test]
    fn positioner_pushes_up_at_bottom_edge() {
        assert_eq!(position(100.0, 238.0, 320.0, 240.0, 60.0, 20.0).1, 217.0);
    }

    #[test]
    fn positioner_keeps_top_edge_free() {
        assert_eq!(position(100.0, 30.0, 320.0, 240.0, 60.0, 20.0).1, 18.0);
    }

    #[test]
    fn positioner_taller_than_screen_pins_to_top() {
        assert_eq!(position(100.0, 100.0, 320.0, 240.0, 60.0, 400.0).1, 0.0);
    }

    #[test]
    fn preview_tooltip() {
        let atlas = crate::gui::atlas::build_gui_atlas(&crate::assets_root());
        let stack = SlotStack {
            item: "diamond_pickaxe",
            count: 1,
            damage: 900,
            max_damage: 1561,
            custom_name: Some(crate::text::parse_formatted("\u{a7}bDigger of Holes")),
            lore: vec![crate::text::styled(
                "Older than the mountain.",
                Style::default(),
            )],
            enchantments: vec![("efficiency".into(), 5), ("mending".into(), 1)],
            potion: None,
            unbreakable: true,
            component_count: 5,
            nbt_bytes: 1434,
            charged: false,
            map_id: None,
            ..Default::default()
        };
        let mut p = Painter::new(&atlas, 0);
        draw(&mut p, &stack, 40.0, 60.0, 320.0, 240.0, true);
        assert!(!p.positions.is_empty());
        let img = crate::gui::preview::rasterize(&p, 320, 240, [70, 110, 170, 255]);
        let path = crate::gui::preview::output_dir().join("gui_tooltip.png");
        crate::gui::preview::upscale(&img, 2).save(&path).unwrap();
        println!("[preview] wrote {}", path.display());
    }
}
