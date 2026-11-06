use azalea_protocol::packets::game::c_award_stats::Stat;
use azalea_registry::Registry;
use azalea_registry::builtin::{CustomStat, EntityKind, ItemKind};

use crate::client::stats::StatsCounter;
use crate::gui::painter::Painter;
use crate::gui::tooltip::{self, label, translate};
use crate::gui::widgets::{self, Button, HeaderFooter, TAB_BAR_H, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx};
use crate::items::placed::{block_item, placed_block};
use crate::platform::time::Instant;
use crate::session::{SharedMutex, SlotStack};
use crate::text::{LINE_HEIGHT, Span};

use super::options::{draw_scrollbar, row_visible, scroll_masked};

const LAYOUT: HeaderFooter = HeaderFooter::new(TAB_BAR_H, HeaderFooter::DEFAULT_H);

const ROW_W: f32 = 280.0;
const PAD: f32 = 2.0;
const GENERAL_ROW_H: f32 = 14.0;
const ITEM_ROW_H: f32 = 22.0;
const MOB_ROW_H: f32 = 4.0 * LINE_HEIGHT;
const LIST_SLACK: f32 = 4.0;
const SLOT: f32 = 18.0;

const EVEN: u32 = 0xFF_FFFF;
const ODD: u32 = 0xBA_BABA;
const NEVER: u32 = 0x80_8080;

const GENERAL: usize = 0;
const ITEMS: usize = 1;
const MOBS: usize = 2;

const DOTS: [&str; 4] = ["O o o", "o O o", "o o O", "o O o"];
const DOTS_INTERVAL_MS: u128 = 300;
const LOADING_PAD_BOTTOM: f32 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Column {
    Mined,
    Broken,
    Crafted,
    Used,
    PickedUp,
    Dropped,
}

const COLUMNS: [Column; 6] = [
    Column::Mined,
    Column::Broken,
    Column::Crafted,
    Column::Used,
    Column::PickedUp,
    Column::Dropped,
];

impl Column {
    fn stat(self, item: ItemKind) -> Option<Stat> {
        Some(match self {
            Column::Mined => Stat::Mined(placed_block(bare(item.to_str()))?),
            Column::Broken => Stat::Broken(item),
            Column::Crafted => Stat::Crafted(item),
            Column::Used => Stat::Used(item),
            Column::PickedUp => Stat::PickedUp(item),
            Column::Dropped => Stat::Dropped(item),
        })
    }

    fn sprite(self) -> &'static str {
        match self {
            Column::Mined => "statistics/block_mined",
            Column::Broken => "statistics/item_broken",
            Column::Crafted => "statistics/item_crafted",
            Column::Used => "statistics/item_used",
            Column::PickedUp => "statistics/item_picked_up",
            Column::Dropped => "statistics/item_dropped",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Column::Mined => label!("stat_type.minecraft.mined"),
            Column::Broken => label!("stat_type.minecraft.broken"),
            Column::Crafted => label!("stat_type.minecraft.crafted"),
            Column::Used => label!("stat_type.minecraft.used"),
            Column::PickedUp => label!("stat_type.minecraft.picked_up"),
            Column::Dropped => label!("stat_type.minecraft.dropped"),
        }
    }

    fn x(index: usize) -> f32 {
        75.0 + 40.0 * index as f32
    }
}

fn bare(id: &'static str) -> &'static str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

struct Lists {
    general: Vec<(CustomStat, String)>,
    items: Vec<ItemKind>,
    mobs: Vec<(EntityKind, String)>,
}

impl Lists {
    fn build(stats: &StatsCounter) -> Lists {
        let mut general: Vec<(CustomStat, String)> = (0..)
            .map_while(CustomStat::from_u32)
            .map(|s| (s, translate(&custom_key(s), &[])))
            .collect();
        general.sort_by(|a, b| a.1.cmp(&b.1));

        let mut items = Vec::new();
        let mut mobs = Vec::new();
        for (stat, value) in stats.iter() {
            if value <= 0 {
                continue;
            }
            match stat {
                Stat::Mined(block) => items.extend(block_item(block)),
                Stat::Broken(i)
                | Stat::Crafted(i)
                | Stat::Used(i)
                | Stat::PickedUp(i)
                | Stat::Dropped(i) => items.push(i),
                Stat::Killed(e) | Stat::KilledBy(e) => mobs.push(e),
                Stat::Custom(_) => {}
            }
        }
        items.retain(|i| *i != ItemKind::Air);
        items.sort_by_key(|i| i.to_u32());
        items.dedup();
        mobs.sort_by_key(|e| e.to_u32());
        mobs.dedup();
        let mobs = mobs
            .into_iter()
            .map(|e| {
                let name = translate(&format!("entity.minecraft.{}", bare(e.to_str())), &[]);
                (e, name)
            })
            .collect();
        Lists {
            general,
            items,
            mobs,
        }
    }

    fn has_rows(&self, tab: usize) -> bool {
        match tab {
            ITEMS => !self.items.is_empty(),
            MOBS => !self.mobs.is_empty(),
            _ => true,
        }
    }
}

fn custom_key(stat: CustomStat) -> String {
    format!("stat.{}", stat.to_str().replace(':', "."))
}

fn format_custom(stat: CustomStat, value: i32) -> String {
    use CustomStat::*;
    let id = stat.to_str();
    if matches!(
        stat,
        PlayTime | TotalWorldTime | TimeSinceDeath | TimeSinceRest | SneakTime
    ) {
        format_time(value)
    } else if id.ends_with("_one_cm") {
        format_distance(value)
    } else if id.starts_with("minecraft:damage_") {
        format!("{:.2}", value as f64 * 0.1)
    } else {
        format_default(value)
    }
}

fn format_default(value: i32) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if value < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn format_distance(cm: i32) -> String {
    let meters = cm as f64 / 100.0;
    let kilometers = meters / 1000.0;
    if kilometers > 0.5 {
        format!("{kilometers:.2} km")
    } else if meters > 0.5 {
        format!("{meters:.2} m")
    } else {
        format!("{cm} cm")
    }
}

fn format_time(ticks: i32) -> String {
    let seconds = ticks as f64 / 20.0;
    let minutes = seconds / 60.0;
    let hours = minutes / 60.0;
    let days = hours / 24.0;
    let years = days / 365.0;
    if years > 0.5 {
        format!("{years:.2} y")
    } else if days > 0.5 {
        format!("{days:.2} d")
    } else if hours > 0.5 {
        format!("{hours:.2} h")
    } else if minutes > 0.5 {
        format!("{minutes:.2} min")
    } else {
        format!("{seconds:?} s")
    }
}

#[derive(Default)]
pub struct StatsUi {
    opened_at: u32,
    opened: Option<Instant>,
    lists: Option<Lists>,
    tab: usize,
    scroll: [f32; 3],
    drag: Option<f32>,
    selected: [Option<usize>; 3],
    sort: Option<(Column, i32)>,
}

impl StatsUi {
    pub fn open(&mut self, updates: u32) {
        *self = StatsUi {
            opened_at: updates,
            opened: Some(Instant::now()),
            ..StatsUi::default()
        };
    }

    fn cycle_sort(&mut self, column: Column, stats: &StatsCounter) {
        self.sort = match self.sort {
            Some((c, -1)) if c == column => Some((column, 1)),
            Some((c, _)) if c == column => None,
            _ => Some((column, -1)),
        };
        let (Some((column, order)), Some(lists)) = (self.sort, self.lists.as_mut()) else {
            return;
        };
        let key = |item: ItemKind| column.stat(item).map_or(-1, |s| stats.get(s));
        lists.items.sort_by(|&a, &b| {
            let ord = key(a)
                .cmp(&key(b))
                .then_with(|| a.to_u32().cmp(&b.to_u32()));
            if order < 0 { ord.reverse() } else { ord }
        });
    }
}

enum Tip {
    Text(&'static str),
    Item(ItemKind, f32, f32),
}

pub fn draw(
    p: &mut Painter,
    ui: &mut StatsUi,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
    advanced_tooltips: bool,
) -> Option<Screen> {
    let stats = shared.lock().unwrap().session.stats.clone();
    if ui.lists.is_none() && stats.updates != ui.opened_at {
        ui.lists = Some(Lists::build(&stats));
        ui.tab = GENERAL;
    }

    let mut tip = None;
    let labels = [
        label!("stat.generalButton"),
        label!("stat.itemsButton"),
        label!("stat.mobsButton"),
    ];
    let active = |i: usize| match &ui.lists {
        Some(lists) => lists.has_rows(i),
        None => i == GENERAL,
    };
    if let Some(tab) = widgets::tab_bar_with(p, ctx, &labels, ui.tab, active) {
        ui.tab = tab;
        ui.drag = None;
    }
    if let Some(lists) = &ui.lists {
        let (x0, tab_w) = widgets::tab_layout(ctx.vw, labels.len());
        for i in 0..labels.len() {
            if !lists.has_rows(i) && ctx.hovering(x0 + i as f32 * tab_w, 0.0, tab_w, TAB_BAR_H) {
                tip = Some(Tip::Text(label!("gui.stats.none_found")));
            }
        }
    }

    let (_, list_y, _, list_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = (ctx.vw / 2.0).floor() - ROW_W / 2.0;
    if ui.lists.is_none() {
        loading(p, ui, ctx, list_y, list_h);
    } else {
        {
            let tab = ui.tab;
            let rows = {
                let lists = ui.lists.as_ref().unwrap();
                match tab {
                    ITEMS => lists.items.len() + 1,
                    MOBS => lists.mobs.len(),
                    _ => lists.general.len(),
                }
            };
            let row_h = match tab {
                ITEMS => ITEM_ROW_H,
                MOBS => MOB_ROW_H,
                _ => GENERAL_ROW_H,
            };
            let content_h = rows as f32 * row_h + LIST_SLACK;
            let rect = (0.0, list_y, ctx.vw, list_h);
            let masked = scroll_masked(
                &mut ui.scroll[tab],
                &mut ui.drag,
                ctx,
                left,
                ROW_W,
                rect,
                content_h,
            );
            let lctx = ctx.with_input(&masked);
            let scroll = ui.scroll[tab];
            let first_top = list_y + PAD - scroll;

            let guard = p.push_clip(0.0, list_y, ctx.vw, list_h);
            match tab {
                ITEMS => items(
                    p, ui, &lctx, &stats, left, first_top, list_y, list_h, &mut tip,
                ),
                MOBS | GENERAL => {
                    for i in 0..rows {
                        let top = first_top + i as f32 * row_h;
                        if !row_visible(top, row_h, list_y, list_h) {
                            continue;
                        }
                        if lctx.input.left_click && lctx.hovering(left, top, ROW_W, row_h) {
                            ui.selected[tab] = Some(i);
                        }
                        if ui.selected[tab] == Some(i) {
                            p.fill(left, top, ROW_W, row_h, 0xFFFF_FFFF);
                            p.fill(left + 1.0, top + 1.0, ROW_W - 2.0, row_h - 2.0, 0xFF00_0000);
                        }
                        let lists = ui.lists.as_ref().unwrap();
                        if tab == MOBS {
                            mob_row(p, &stats, &lists.mobs[i], left, top);
                        } else {
                            general_row(p, &stats, &lists.general[i], i, left, top);
                        }
                    }
                }
                _ => {}
            }
            p.pop_clip(guard);
            draw_scrollbar(p, scroll, left, ROW_W, list_y, list_h, content_h);
        }
    }

    p.sprite(
        "footer_separator",
        0.0,
        ctx.vh - LAYOUT.footer_h,
        ctx.vw,
        2.0,
    );
    let done = Button::new(
        widgets::centered_x(ctx.vw),
        LAYOUT.footer_y(ctx.vh),
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        label!("gui.done"),
    )
    .draw(p, ctx);

    match tip {
        Some(Tip::Text(text)) => {
            if let Some(m) = ctx.mouse() {
                tooltip::draw_lines(
                    p,
                    &[vec![Span::colored(text, EVEN)]],
                    m.x,
                    m.y,
                    ctx.vw,
                    ctx.vh,
                );
            }
        }
        Some(Tip::Item(item, x, y)) => {
            let stack = SlotStack {
                item: bare(item.to_str()),
                count: 1,
                ..SlotStack::default()
            };
            tooltip::draw(p, &stack, x, y, ctx.vw, ctx.vh, advanced_tooltips, false);
        }
        None => {}
    }

    done.then_some(Screen::Pause)
}

fn loading(p: &mut Painter, ui: &StatsUi, ctx: &ScreenCtx, area_y: f32, area_h: f32) {
    let message = label!("multiplayer.downloadingStats");
    let widget_h = 3.0 * LINE_HEIGHT;
    let top = area_y + ((area_h - widget_h - LOADING_PAD_BOTTOM) * 0.5).floor();
    let center_y = top + (widget_h / 2.0).floor();
    let w = p.atlas.font.width_str(message);
    p.text_plain(
        message,
        ((ctx.vw - w) / 2.0).floor(),
        center_y - LINE_HEIGHT,
        EVEN,
        true,
    );
    let ms = ui.opened.map_or(0, |t| t.elapsed().as_millis());
    let dots = DOTS[(ms / DOTS_INTERVAL_MS) as usize % DOTS.len()];
    let w = p.atlas.font.width_str(dots);
    p.text_plain(
        dots,
        ((ctx.vw - w) / 2.0).floor(),
        center_y + LINE_HEIGHT,
        NEVER,
        true,
    );
}

fn content(left: f32, top: f32, row_h: f32) -> (f32, f32, f32) {
    let cx = left + PAD;
    let cy = top + PAD;
    let text_y = cy + ((row_h - 2.0 * PAD) / 2.0).floor() - (LINE_HEIGHT / 2.0).floor();
    (cx, cy, text_y)
}

fn general_row(
    p: &mut Painter,
    stats: &StatsCounter,
    (stat, name): &(CustomStat, String),
    index: usize,
    left: f32,
    top: f32,
) {
    let (cx, _, y) = content(left, top, GENERAL_ROW_H);
    let color = if index % 2 == 0 { EVEN } else { ODD };
    p.text_plain(name, cx + 2.0, y, color, true);
    let value = format_custom(*stat, stats.get(Stat::Custom(*stat)));
    let right = cx + ROW_W - 2.0 * PAD;
    let w = p.atlas.font.width_str(&value);
    p.text_plain(&value, right - w - 4.0, y, color, true);
}

fn mob_row(
    p: &mut Painter,
    stats: &StatsCounter,
    (entity, name): &(EntityKind, String),
    left: f32,
    top: f32,
) {
    let (cx, cy, _) = content(left, top, MOB_ROW_H);
    p.text_plain(name, cx + 2.0, cy + 1.0, EVEN, true);

    let kills = stats.get(Stat::Killed(*entity));
    let (text, color) = if kills == 0 {
        (
            translate(
                "stat_type.minecraft.killed.none",
                std::slice::from_ref(name),
            ),
            NEVER,
        )
    } else {
        (
            translate(
                "stat_type.minecraft.killed",
                &[kills.to_string(), name.clone()],
            ),
            ODD,
        )
    };
    p.text_plain(&text, cx + 12.0, cy + 1.0 + LINE_HEIGHT, color, true);

    let killed_by = stats.get(Stat::KilledBy(*entity));
    let (text, color) = if killed_by == 0 {
        (
            translate(
                "stat_type.minecraft.killed_by.none",
                std::slice::from_ref(name),
            ),
            NEVER,
        )
    } else {
        (
            translate(
                "stat_type.minecraft.killed_by",
                &[name.clone(), killed_by.to_string()],
            ),
            ODD,
        )
    };
    p.text_plain(&text, cx + 12.0, cy + 1.0 + 2.0 * LINE_HEIGHT, color, true);
}

#[allow(clippy::too_many_arguments)]
fn items(
    p: &mut Painter,
    ui: &mut StatsUi,
    ctx: &ScreenCtx,
    stats: &StatsCounter,
    left: f32,
    first_top: f32,
    list_y: f32,
    list_h: f32,
    tip: &mut Option<Tip>,
) {
    if row_visible(first_top, ITEM_ROW_H, list_y, list_h) {
        let (cx, cy, _) = content(left, first_top, ITEM_ROW_H);
        let mut clicked = None;
        for (i, column) in COLUMNS.iter().enumerate() {
            let x = cx + Column::x(i) - SLOT;
            let y = cy + 1.0;
            p.sprite("statistics/header", x, y, SLOT, SLOT);
            p.sprite(column.sprite(), x, y, SLOT, SLOT);
            if ctx.hovering(x, y, SLOT, SLOT) {
                *tip = Some(Tip::Text(column.name()));
                if ctx.input.left_click {
                    clicked = Some(*column);
                }
            }
        }
        if let Some((column, order)) = ui.sort {
            let index = COLUMNS.iter().position(|c| *c == column).unwrap_or(0);
            let sprite = if order == 1 {
                "statistics/sort_up"
            } else {
                "statistics/sort_down"
            };
            p.sprite(
                sprite,
                cx + Column::x(index) - 2.0 * SLOT,
                cy + 1.0,
                SLOT,
                SLOT,
            );
        }
        if let Some(column) = clicked {
            ui.cycle_sort(column, stats);
        }
    }

    let Some(lists) = ui.lists.as_ref() else {
        return;
    };
    let none = label!("stats.none");
    for (i, &item) in lists.items.iter().enumerate() {
        let index = i + 1;
        let top = first_top + index as f32 * ITEM_ROW_H;
        if !row_visible(top, ITEM_ROW_H, list_y, list_h) {
            continue;
        }
        let (cx, cy, y) = content(left, top, ITEM_ROW_H);
        p.sprite("container/slot", cx, cy, SLOT, SLOT);
        p.item_icon(bare(item.to_str()), cx + 1.0, cy + 1.0);
        if ctx.hovering(cx, cy, SLOT, SLOT) {
            *tip = Some(Tip::Item(item, cx + SLOT, cy + SLOT));
        }
        let color = if index % 2 == 0 { EVEN } else { ODD };
        for (c, column) in COLUMNS.iter().enumerate() {
            let text = column.stat(item).map(|s| format_default(stats.get(s)));
            let text = text.as_deref().unwrap_or(none);
            let w = p.atlas.font.width_str(text);
            p.text_plain(text, cx + Column::x(c) - w, y, color, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_groups_thousands() {
        assert_eq!(format_default(0), "0");
        assert_eq!(format_default(999), "999");
        assert_eq!(format_default(1000), "1,000");
        assert_eq!(format_default(1234567), "1,234,567");
        assert_eq!(format_default(-45678), "-45,678");
    }

    #[test]
    fn distance_switches_units_past_half() {
        assert_eq!(format_distance(50), "50 cm");
        assert_eq!(format_distance(51), "0.51 m");
        assert_eq!(format_distance(50_001), "0.50 km");
        assert_eq!(format_distance(123_456), "1.23 km");
    }

    #[test]
    fn time_keeps_a_decimal_on_seconds() {
        assert_eq!(format_time(0), "0.0 s");
        assert_eq!(format_time(20), "1.0 s");
        assert_eq!(format_time(247), "12.35 s");
        assert_eq!(format_time(20 * 60), "1.00 min");
        assert_eq!(format_time(20 * 60 * 60 * 24), "1.00 d");
    }

    #[test]
    fn custom_stats_pick_their_formatter() {
        assert_eq!(format_custom(CustomStat::DamageDealt, 125), "12.50");
        assert_eq!(format_custom(CustomStat::Jump, 1500), "1,500");
        assert_eq!(custom_key(CustomStat::Jump), "stat.minecraft.jump");
    }

    #[test]
    fn sort_cycles_down_up_off() {
        let stats = StatsCounter::default();
        let mut ui = StatsUi::default();
        ui.cycle_sort(Column::Used, &stats);
        assert_eq!(ui.sort, Some((Column::Used, -1)));
        ui.cycle_sort(Column::Used, &stats);
        assert_eq!(ui.sort, Some((Column::Used, 1)));
        ui.cycle_sort(Column::Used, &stats);
        assert_eq!(ui.sort, None);
        ui.cycle_sort(Column::Used, &stats);
        ui.cycle_sort(Column::Mined, &stats);
        assert_eq!(ui.sort, Some((Column::Mined, -1)));
    }
}
