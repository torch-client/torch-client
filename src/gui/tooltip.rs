use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use azalea_inventory::bundle::{
    Fraction, MAX_SHOWN_GRID_ITEMS, MAX_SHOWN_GRID_ITEMS_X, number_of_items_to_show,
};

use crate::generated_items::item;
use crate::gui::painter::Painter;
use crate::items::potions;
use crate::modules::better_tooltips::{self, FoodRows};
use crate::play::mob_effects;
use crate::session::{BundleView, SlotStack};
use crate::text::{Font, Span};
use crate::util::reload::Reloadable;

const MOUSE_OFFSET: f32 = 12.0;
const PADDING: f32 = 3.0;
const MARGIN: f32 = 9.0;
const ROW_HEIGHT: f32 = 10.0;
const TITLE_GAP: f32 = 2.0;

const GRAY: u32 = crate::text::GRAY;
const DARK_GRAY: u32 = crate::text::DARK_GRAY;
const BLUE: u32 = 0x5555FF;
const RED: u32 = crate::text::RED;
const DARK_PURPLE: u32 = 0xAA00AA;
const WHITE: u32 = crate::text::WHITE;

static LANG: Reloadable<HashMap<String, String>> = Reloadable::new(load_lang);

fn load_lang() -> HashMap<String, String> {
    let mut out = HashMap::new();
    #[cfg_attr(not(feature = "asset_download"), allow(unused_mut))]
    let mut layers =
        crate::platform::assets::read_stack(crate::assets_root().join("lang/en_us.json"));
    #[cfg(feature = "asset_download")]
    if let Some((code, text)) = crate::client::language::strings() {
        layers.push(std::borrow::Cow::Owned(text.to_string()));
        layers.extend(crate::platform::assets::read_stack(
            crate::assets_root().join(format!("lang/{code}.json")),
        ));
    }
    for text in layers {
        let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&text) else {
            continue;
        };
        out.extend(
            map.into_iter()
                .filter_map(|(k, v)| Some((k, v.as_str()?.to_owned()))),
        );
    }
    out
}

pub(crate) fn reset_lang() {
    LANG.reset();
}

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

pub(crate) fn resolve_label(
    key: &'static str,
    cell: &'static crate::util::reload::Label,
) -> &'static str {
    cell.get(LANG.epoch(), || translate(key, &[]))
}

macro_rules! label {
    ($key:literal) => {{
        static CELL: $crate::util::reload::Label = $crate::util::reload::Label::new();
        $crate::gui::tooltip::resolve_label($key, &CELL)
    }};
}
pub(crate) use label;

pub(crate) fn translate(key: &str, args: &[String]) -> String {
    let lang = LANG.get();
    let template: &str = lang.get(key).map_or(key, String::as_str);
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

pub(crate) fn roman(n: i32) -> String {
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
    vec![Span::colored(text, color)]
}

pub(crate) fn ordered_enchantments(
    list: &[(String, i32)],
) -> impl Iterator<Item = &(String, i32)> + '_ {
    let order: &'static [String] = &TOOLTIP_ORDER;
    order
        .iter()
        .filter_map(move |id| list.iter().find(|(n, _)| n == id))
        .chain(list.iter().filter(move |e| !order.contains(&e.0)))
}

pub(crate) fn is_curse(id: &str) -> bool {
    CURSES.contains(id)
}

pub(crate) fn short_enchantment(id: &str, level: i32, length: usize, out: &mut String) {
    use std::fmt::Write;
    let name = translate(&format!("enchantment.minecraft.{id}"), &[]);
    out.extend(name.chars().take(length));
    let _ = write!(out, " {level}");
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
    let contents = stack.extra.potion.as_ref()?;
    if !potions::is_potion_item(stack.item) {
        return None;
    }
    Some(translate(&contents.name_key(stack.item), &[]))
}

fn potion_lines(
    effects: &[potions::PotionEffectInstance],
    scale: f32,
    tick_rate: f32,
) -> Vec<Vec<Span>> {
    let mut out: Vec<Vec<Span>> = Vec::new();

    if effects.is_empty() {
        out.push(plain(translate("effect.none", &[]), GRAY));
        return out;
    }

    let mut modifiers: Vec<(String, f64, u8)> = Vec::new();
    for effect in effects {
        let (duration, amplifier) = (effect.details.duration, effect.details.amplifier);
        if let Some(m) = mob_effects::attribute_modifier(&effect.effect, amplifier) {
            modifiers.push(m);
        }
        let name = mob_effects::display_name(&effect.effect, 0);
        let mut text = if amplifier > 0 {
            translate(
                "potion.withAmplifier",
                &[name, translate(&format!("potion.potency.{amplifier}"), &[])],
            )
        } else {
            name
        };
        if !mob_effects::ends_within(duration, 20) {
            text = translate(
                "potion.withDuration",
                &[
                    text,
                    mob_effects::format_duration(duration, scale, tick_rate),
                ],
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

fn painting_lines(stack: &SlotStack, creative: bool) -> Vec<Vec<Span>> {
    let Some(id) = stack.extra.painting_variant.as_deref() else {
        return if creative {
            vec![plain(translate("painting.random", &[]), GRAY)]
        } else {
            Vec::new()
        };
    };
    let Some(variant) = crate::util::variants::painting(id) else {
        return Vec::new();
    };
    let mut out: Vec<Vec<Span>> = [&variant.title, &variant.author]
        .into_iter()
        .flatten()
        .map(crate::client::chat_text::to_spans)
        .collect();
    out.push(plain(
        translate(
            "painting.dimensions",
            &[variant.width.to_string(), variant.height.to_string()],
        ),
        WHITE,
    ));
    out
}

fn format_modifier(value: f64) -> String {
    let text = format!("{:.2}", value);
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

pub(crate) fn styled_hover_name(stack: &SlotStack) -> Vec<Span> {
    let def = item(stack.item);
    let rarity = def.map(|d| d.rarity.color()).unwrap_or(WHITE);
    match &stack.extra.custom_name {
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
    let book = stack.extra.book.as_deref()?;
    (book.signed && !book.title.trim().is_empty()).then(|| book.title.clone())
}

fn build_lines(stack: &SlotStack, advanced: bool, creative: bool) -> Vec<Vec<Span>> {
    let mut out: Vec<Vec<Span>> = Vec::new();
    let tick_rate = tick_rate();

    out.push(styled_hover_name(stack));

    if stack.item == "painting" {
        out.extend(painting_lines(stack, creative));
    }

    if let Some(b) = &stack.extra.book
        && b.signed
    {
        out.extend(book_lines(b));
    }

    if let Some(contents) = &stack.extra.potion
        && potions::is_potion_item(stack.item)
    {
        let scale = contents
            .duration_scale
            .unwrap_or_else(|| potions::default_duration_scale(stack.item));
        out.extend(potion_lines(&contents.all_effects(), scale, tick_rate));
    }

    for (id, level) in ordered_enchantments(&stack.extra.enchantments) {
        out.push(enchantment_line(id, *level));
    }

    for line in &stack.extra.lore {
        out.push(inherit(line, DARK_PURPLE, true));
    }

    if stack.unbreakable {
        out.push(plain(translate("item.unbreakable", &[]), BLUE));
    }

    if let Some(amplifier) = stack.extra.ominous_amplifier {
        let effect = potions::PotionEffectInstance::new("bad_omen", 120000, amplifier);
        out.extend(potion_lines(&[effect], 1.0, tick_rate));
    }

    if creative && let Some(stew) = &stack.extra.stew_effects {
        let effects: Vec<_> = stew
            .iter()
            .map(|(id, duration)| potions::PotionEffectInstance::new(id.as_str(), *duration, 0))
            .collect();
        out.extend(potion_lines(&effects, 1.0, tick_rate));
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
            format!(
                "Size: {}",
                crate::util::bytes::Bytes(stack.nbt_bytes as u64)
            ),
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

const SLOT_SIZE: f32 = 16.0;
const SLOT_GAP: f32 = 6.0;

fn position_over_slot(sx: f32, sy: f32, vw: f32, vh: f32, w: f32, h: f32) -> (f32, f32) {
    let pad = PADDING + 1.0;
    let x = if sx + SLOT_SIZE / 2.0 > vw / 2.0 {
        sx + SLOT_SIZE - w
    } else {
        sx
    };
    let x = x.min(vw - pad - w).max(pad);
    let above = sy - SLOT_GAP - pad - h;
    let y = if above >= pad {
        above
    } else {
        (sy + SLOT_SIZE + SLOT_GAP + pad).min(vh - pad - h).max(0.0)
    };
    (x.floor(), y.floor())
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
    creative: bool,
    tick_rate: f32,
    lang_epoch: u32,
    food_values: bool,
    lines: Vec<Vec<Span>>,
    bundle: BundleText,
    food: Option<(usize, FoodRows)>,
}

const BUNDLE_WIDTH: f32 = 96.0;
const BUNDLE_SLOT: f32 = 24.0;
const BUNDLE_SLOT_MARGIN: f32 = 4.0;
const BUNDLE_BAR_HEIGHT: f32 = 13.0;
const BUNDLE_BAR_MARGIN: f32 = 4.0;
const BUNDLE_BAR_FILL_MAX: i32 = 94;
const LINE_HEIGHT: f32 = 9.0;

#[derive(Default)]
struct BundleText {
    empty_description: Vec<Vec<Span>>,
    selected_name: Vec<Vec<Span>>,
    hidden: String,
}

impl BundleText {
    fn new(stack: &SlotStack, font: &Font) -> BundleText {
        let Some(view) = stack.bundle.as_deref().filter(|view| view.shows) else {
            return BundleText::default();
        };
        let description = translate("item.minecraft.bundle.empty.description", &[]);
        let shown = number_of_items_to_show(view.items.len());
        let hidden: u32 = view
            .items
            .iter()
            .skip(shown)
            .map(|item| u32::from(item.count))
            .sum();
        BundleText {
            empty_description: if view.items.is_empty() {
                font.wrap(&plain(description, GRAY), BUNDLE_WIDTH)
            } else {
                Vec::new()
            },
            selected_name: usize::try_from(view.selected)
                .ok()
                .and_then(|i| view.items.get(i))
                .map(styled_hover_name)
                .into_iter()
                .collect(),
            hidden: format!("+{hidden}"),
        }
    }
}

struct BundleImage<'a> {
    view: &'a BundleView,
    text: &'a BundleText,
    screen: (f32, f32),
}

impl<'a> BundleImage<'a> {
    fn of(stack: &'a SlotStack, text: &'a BundleText, vw: f32, vh: f32) -> Option<Self> {
        let view = stack.bundle.as_deref().filter(|view| view.shows)?;
        Some(BundleImage {
            view,
            text,
            screen: (vw, vh),
        })
    }

    fn grid_rows(&self) -> usize {
        self.view
            .items
            .len()
            .min(MAX_SHOWN_GRID_ITEMS)
            .div_ceil(MAX_SHOWN_GRID_ITEMS_X)
    }

    fn description_height(&self) -> f32 {
        self.text.empty_description.len() as f32 * LINE_HEIGHT
    }

    fn height(&self) -> f32 {
        let content = if self.view.items.is_empty() {
            self.description_height()
        } else {
            self.grid_rows() as f32 * BUNDLE_SLOT
        };
        content + BUNDLE_BAR_HEIGHT + 2.0 * BUNDLE_BAR_MARGIN
    }

    fn draw(&self, p: &mut Painter, x: f32, y: f32, w: f32) {
        let Some(weight) = self.view.weight else {
            return;
        };
        let left = x + ((w - BUNDLE_WIDTH) / 2.0).floor();
        if self.view.items.is_empty() {
            for (i, line) in self.text.empty_description.iter().enumerate() {
                p.text(line, left, y + i as f32 * LINE_HEIGHT, true);
            }
            let bar_y = y + self.description_height() + BUNDLE_BAR_MARGIN;
            Self::progress_bar(p, left, bar_y, weight);
            return;
        }
        self.draw_grid(p, left, y);
        self.draw_selected_name(p, x, y, w);
        let bar_y = y + self.grid_rows() as f32 * BUNDLE_SLOT + BUNDLE_BAR_MARGIN;
        Self::progress_bar(p, left, bar_y, weight);
    }

    fn draw_grid(&self, p: &mut Painter, left: f32, y: f32) {
        let items = &self.view.items;
        let overflowing = items.len() > MAX_SHOWN_GRID_ITEMS;
        let shown = &items[..number_of_items_to_show(items.len()).min(items.len())];
        let rows = self.grid_rows();
        let (x_start, y_start) = (left + BUNDLE_WIDTH, y + rows as f32 * BUNDLE_SLOT);
        let mut slot_number = 1;
        for row in 1..=rows {
            for column in 1..=MAX_SHOWN_GRID_ITEMS_X {
                let draw_x = x_start - column as f32 * BUNDLE_SLOT;
                let draw_y = y_start - row as f32 * BUNDLE_SLOT;
                if overflowing && column * row == 1 {
                    let half = (p.atlas.font.width_str(&self.text.hidden) / 2.0).floor();
                    let x = draw_x + BUNDLE_SLOT / 2.0 - half;
                    p.text_plain(&self.text.hidden, x, draw_y + 10.0, WHITE, true);
                } else if shown.len() >= slot_number {
                    self.draw_slot(p, shown, shown.len() - slot_number, draw_x, draw_y);
                    slot_number += 1;
                }
            }
        }
    }

    fn draw_slot(&self, p: &mut Painter, shown: &[SlotStack], index: usize, x: f32, y: f32) {
        let selected = i32::try_from(index).is_ok_and(|index| index == self.view.selected);
        let back = if selected {
            "container/bundle/slot_highlight_back"
        } else {
            "container/bundle/slot_background"
        };
        p.sprite(back, x, y, BUNDLE_SLOT, BUNDLE_SLOT);
        crate::gui::slots::draw_stack(
            p,
            &shown[index],
            x + BUNDLE_SLOT_MARGIN,
            y + BUNDLE_SLOT_MARGIN,
        );
        if selected {
            p.sprite(
                "container/bundle/slot_highlight_front",
                x,
                y,
                BUNDLE_SLOT,
                BUNDLE_SLOT,
            );
        }
    }

    fn draw_selected_name(&self, p: &mut Painter, x: f32, y: f32, w: f32) {
        let Some(name) = self.text.selected_name.first() else {
            return;
        };
        let center = x + (w / 2.0).floor() - 12.0;
        let mx = center - (p.atlas.font.width(name).ceil() / 2.0).floor();
        let (vw, vh) = self.screen;
        place_lines(p, &self.text.selected_name, None, None, |bw, bh| {
            position(mx, y - 15.0, vw, vh, bw, bh)
        });
    }

    fn progress_bar(p: &mut Painter, x: f32, y: f32, weight: Fraction) {
        let fill = weight
            .mul_and_truncate(BUNDLE_BAR_FILL_MAX)
            .clamp(0, BUNDLE_BAR_FILL_MAX);
        let sprite = if weight.is_full() {
            "container/bundle/bundle_progressbar_full"
        } else {
            "container/bundle/bundle_progressbar_fill"
        };
        if fill > 0 {
            p.sprite(sprite, x + 1.0, y, fill as f32, BUNDLE_BAR_HEIGHT);
        }
        let border = "container/bundle/bundle_progressbar_border";
        p.sprite(border, x, y, BUNDLE_WIDTH, BUNDLE_BAR_HEIGHT);
        let text = if weight.is_zero() {
            Some(label!("item.minecraft.bundle.empty"))
        } else if weight.is_full() {
            Some(label!("item.minecraft.bundle.full"))
        } else {
            None
        };
        if let Some(text) = text {
            let half = (p.atlas.font.width_str(text) / 2.0).floor();
            p.text_plain(text, x + BUNDLE_WIDTH / 2.0 - half, y + 3.0, WHITE, true);
        }
    }
}

thread_local! {
    static LINES: RefCell<Option<LineCache>> = const { RefCell::new(None) };
    static TICK_RATE: Cell<f32> = const { Cell::new(20.0) };
}

pub(crate) fn set_tick_rate(rate: f32) {
    TICK_RATE.set(rate);
}

fn tick_rate() -> f32 {
    TICK_RATE.get()
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &mut Painter,
    stack: &SlotStack,
    mx: f32,
    my: f32,
    vw: f32,
    vh: f32,
    advanced: bool,
    creative: bool,
) {
    let atlas = p.atlas;
    with_lines(
        stack,
        advanced,
        creative,
        &atlas.font,
        |lines, bundle, food| {
            let image = BundleImage::of(stack, bundle, vw, vh);
            place_lines(p, lines, image, food, |w, h| position(mx, my, vw, vh, w, h));
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_with_line(
    p: &mut Painter,
    stack: &SlotStack,
    extra: Option<String>,
    mx: f32,
    my: f32,
    vw: f32,
    vh: f32,
    advanced: bool,
) {
    let atlas = p.atlas;
    with_lines(stack, advanced, false, &atlas.font, |lines, _, food| {
        let mut all = lines.to_vec();
        all.extend(extra.map(|text| plain(text, WHITE)));
        place_lines(p, &all, None, food, |w, h| position(mx, my, vw, vh, w, h));
    });
}

pub fn draw_over_slot(
    p: &mut Painter,
    stack: &SlotStack,
    slot: (f32, f32),
    vw: f32,
    vh: f32,
    advanced: bool,
    creative: bool,
) {
    let atlas = p.atlas;
    with_lines(
        stack,
        advanced,
        creative,
        &atlas.font,
        |lines, bundle, food| {
            let image = BundleImage::of(stack, bundle, vw, vh);
            place_lines(p, lines, image, food, |w, h| {
                position_over_slot(slot.0, slot.1, vw, vh, w, h)
            })
        },
    );
}

fn with_lines(
    stack: &SlotStack,
    advanced: bool,
    creative: bool,
    font: &Font,
    draw: impl FnOnce(&[Vec<Span>], &BundleText, Option<(usize, &FoodRows)>),
) {
    if stack.is_empty() {
        return;
    }
    let lang_epoch = LANG.epoch();
    let tick_rate = tick_rate();
    let food_values = better_tooltips::food_values();
    LINES.with_borrow_mut(|cache| {
        let hit = cache.as_ref().is_some_and(|c| {
            c.advanced == advanced
                && c.creative == creative
                && c.tick_rate == tick_rate
                && c.lang_epoch == lang_epoch
                && c.food_values == food_values
                && c.stack == *stack
        });
        if !hit {
            let mut lines = build_lines(stack, advanced, creative);
            let food = food_values
                .then(|| FoodRows::of(stack, font))
                .flatten()
                .map(|rows| {
                    let at = lines.len();
                    lines.extend([Vec::new(), Vec::new()]);
                    (at, rows)
                });
            *cache = Some(LineCache {
                stack: stack.clone(),
                advanced,
                creative,
                tick_rate,
                lang_epoch,
                food_values,
                lines,
                bundle: BundleText::new(stack, font),
                food,
            });
        }
        if let Some(c) = cache.as_ref() {
            draw(
                &c.lines,
                &c.bundle,
                c.food.as_ref().map(|(at, rows)| (*at, rows)),
            );
        }
    });
}

pub(crate) fn draw_lines(p: &mut Painter, lines: &[Vec<Span>], mx: f32, my: f32, vw: f32, vh: f32) {
    place_lines(p, lines, None, None, |w, h| position(mx, my, vw, vh, w, h));
}

fn place_lines(
    p: &mut Painter,
    lines: &[Vec<Span>],
    image: Option<BundleImage<'_>>,
    food: Option<(usize, &FoodRows)>,
    place: impl FnOnce(f32, f32) -> (f32, f32),
) {
    if lines.is_empty() {
        return;
    }

    let w = lines
        .iter()
        .map(|l| p.atlas.font.width(l).ceil())
        .fold(
            if image.is_some() {
                BUNDLE_WIDTH
            } else {
                0.0f32
            },
            f32::max,
        )
        .max(food.map_or(0.0, |(_, rows)| rows.width()));
    let image_height = image.as_ref().map(BundleImage::height);
    let h = match image_height {
        Some(height) => ROW_HEIGHT * lines.len() as f32 + height,
        None => box_height(lines.len()),
    };
    let (x, y) = place(w, h);

    let (bx, by) = (x - PADDING - MARGIN, y - PADDING - MARGIN);
    let (bw, bh) = (w + 2.0 * (PADDING + MARGIN), h + 2.0 * (PADDING + MARGIN));
    p.sprite("tooltip/background", bx, by, bw, bh);
    p.sprite("tooltip/frame", bx, by, bw, bh);

    let mut row_y = y;
    let mut food_y = None;
    for (i, line) in lines.iter().enumerate() {
        if food.is_some_and(|(at, _)| at == i) {
            food_y = Some(row_y);
        }
        p.text(line, x, row_y, true);
        row_y += ROW_HEIGHT + if i == 0 { TITLE_GAP } else { 0.0 };
        if i == 0 {
            row_y += image_height.unwrap_or(0.0);
        }
    }
    if let Some(image) = image {
        image.draw(p, x, y + ROW_HEIGHT + TITLE_GAP, w);
    }
    if let (Some((_, rows)), Some(food_y)) = (food, food_y) {
        rows.draw(p, x, food_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::StackExtra;
    use crate::text::Style;

    fn text_of(line: &[Span]) -> String {
        line.iter().map(|s| s.text.as_str()).collect()
    }

    fn lines_text(stack: &SlotStack, advanced: bool) -> Vec<String> {
        build_lines(stack, advanced, false)
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
            extra: StackExtra {
                book: Some(Box::new(crate::session::Book {
                    title: "Ars Goetia".into(),
                    author: "Steve".into(),
                    signed: true,
                    ..Default::default()
                })),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        assert_eq!(text_of(&styled_hover_name(&stack)), "Ars Goetia");
        assert!(!styled_hover_name(&stack)[0].style.italic);

        stack.extra.make_mut().book.as_mut().unwrap().title = "   ".into();
        assert_eq!(text_of(&styled_hover_name(&stack)), "Written Book");

        let draft = SlotStack {
            item: "writable_book",
            count: 1,
            extra: StackExtra {
                book: Some(Box::new(crate::session::Book::default())),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        assert_eq!(text_of(&styled_hover_name(&draft)), "Book and Quill");
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
            if let Some(v) = LANG.get().get(&key) {
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
        assert_eq!(build_lines(&stack, false, false)[0][0].style.color, WHITE);
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
            extra: StackExtra {
                potion: Some(contents),
                ..Default::default()
            }
            .into(),
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
            custom_effects: vec![potions::PotionEffectInstance::new("poison", 200, 1)],
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
            custom_effects: vec![potions::PotionEffectInstance::new("night_vision", -1, 0)],
            ..Default::default()
        };
        let lines = lines_text(&contents_stack("potion", contents), false);
        assert_eq!(lines[1], "Night Vision (\u{221e})");
    }

    #[test]
    fn duration_scale_shortens_the_printed_time() {
        let contents = |duration_scale| potions::PotionContents {
            custom_effects: vec![potions::PotionEffectInstance::new("poison", 800, 0)],
            duration_scale,
            ..Default::default()
        };
        let line =
            |item, scale| lines_text(&contents_stack(item, contents(scale)), false)[1].clone();
        assert_eq!(line("potion", None), "Poison (00:40)");
        assert_eq!(line("lingering_potion", None), "Poison (00:10)");
        assert_eq!(line("tipped_arrow", None), "Poison (00:05)");
        assert_eq!(line("potion", Some(0.5)), "Poison (00:20)");
    }

    #[test]
    fn ominous_bottle_and_stew_lines() {
        let bottle = SlotStack {
            item: "ominous_bottle",
            count: 1,
            extra: StackExtra {
                ominous_amplifier: Some(1),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        assert_eq!(lines_text(&bottle, false)[1], "Bad Omen II (01:40:00)");

        let stew = SlotStack {
            item: "suspicious_stew",
            count: 1,
            extra: StackExtra {
                stew_effects: Some(vec![("night_vision".into(), 100)]),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        assert_eq!(build_lines(&stew, false, false).len(), 1);
        assert_eq!(
            text_of(&build_lines(&stew, false, true)[1]),
            "Night Vision (00:05)"
        );
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
            extra: StackExtra {
                lore: vec![
                    crate::text::styled("first", Style::default()),
                    crate::text::styled("second", Style::default()),
                ],
                enchantments: vec![("silk_touch".into(), 1), ("efficiency".into(), 5)],
                ..Default::default()
            }
            .into(),
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
        let lines = build_lines(&stack, false, false);
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
            extra: StackExtra {
                custom_name: Some(crate::text::styled("Snack", Style::default())),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        let title = &build_lines(&stack, false, false)[0][0];
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
    fn a_slot_tooltip_sits_above_and_leans_toward_the_middle() {
        assert_eq!(
            position_over_slot(40.0, 100.0, 320.0, 240.0, 60.0, 20.0),
            (40.0, 70.0),
            "a left-half slot: the box starts at the slot and grows right"
        );
        assert_eq!(
            position_over_slot(260.0, 100.0, 320.0, 240.0, 60.0, 20.0),
            (216.0, 70.0),
            "a right-half slot: the box ends at the slot and grows left"
        );
    }

    #[test]
    fn a_slot_tooltip_flips_below_without_room_above() {
        assert_eq!(
            position_over_slot(40.0, 10.0, 320.0, 240.0, 60.0, 20.0).1,
            36.0
        );
    }

    #[test]
    fn a_slot_tooltip_stays_on_screen() {
        let (x, _) = position_over_slot(2.0, 100.0, 320.0, 240.0, 400.0, 20.0);
        assert_eq!(x, 4.0);
        let (x, _) = position_over_slot(300.0, 100.0, 320.0, 240.0, 100.0, 20.0);
        assert_eq!(x, 216.0);
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
}
