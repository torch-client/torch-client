use std::collections::VecDeque;

use crate::diag::Stat;
use crate::direction::Direction;
use crate::gui::ScreenCtx;
use crate::gui::hud_layout::{ElementId, Hud, Rect};
use crate::gui::painter::Painter;
use crate::session::{ChunkProfiling, Gamemode};
use crate::text::LINE_HEIGHT;

const DEBUG_COLOR: u32 = 0xE6FFA0;
const STATUS_COLOR: u32 = 0xFFFF55;
const GOLD: u32 = 0xFFAA00;
const GREEN: u32 = 0x55FF55;
const FLYING_COLOR: u32 = 0x55FFFF;
const DEBUG_BG: u32 = 0x9000_0000;
const HEADER_COLOR: u32 = 0xFFFFFF;
const LABEL_COLOR: u32 = 0xAAAAAA;
const VALUE_COLOR: u32 = DEBUG_COLOR;
const GOOD_COLOR: u32 = 0x55FF55;
const WARN_COLOR: u32 = 0xFFFF55;
const BAD_COLOR: u32 = 0xFF5555;

const MS_60FPS: f32 = 1000.0 / 60.0;
const MS_30FPS: f32 = 1000.0 / 30.0;

fn frame_ms_color(ms: f32) -> u32 {
    if ms <= MS_60FPS {
        GOOD_COLOR
    } else if ms <= MS_30FPS {
        WARN_COLOR
    } else {
        BAD_COLOR
    }
}

fn worker_ms_color(ms: f32) -> u32 {
    if ms <= 2.0 {
        VALUE_COLOR
    } else if ms <= 8.0 {
        WARN_COLOR
    } else {
        BAD_COLOR
    }
}

fn count_color(n: u64, warn_at: u64, bad_at: u64) -> u32 {
    if n >= bad_at {
        BAD_COLOR
    } else if n >= warn_at {
        WARN_COLOR
    } else {
        VALUE_COLOR
    }
}
const NAMETAG_BG: u32 = 0x3F00_0000;

const NAMETAG_BG_MODULE: u32 = 0x4B00_0000;

const BELOW_NAME_GAP: f32 = LINE_HEIGHT * 1.15;

#[derive(Clone, Debug, Default)]
pub struct NameTag {
    pub name: Vec<crate::text::Span>,
    pub below: Vec<crate::text::Span>,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub distance: f32,
    pub health: Option<f32>,
    pub max_health: f32,
    pub gamemode: TagGamemode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TagGamemode {
    #[default]
    Hidden,
    Unknown,
    Known(Gamemode),
}

const HEALTH_BAR_H: f32 = 1.5;
const HEALTH_BAR_GAP: f32 = 1.5;

pub const FRAME_HISTORY_LEN: usize = 160;

#[derive(Default)]
pub struct HudInfo {
    pub pos: [f32; 3],
    pub gamemode: Gamemode,
    pub yaw: f32,
    pub flying: bool,
    pub freecam: bool,
    pub status: Option<std::sync::Arc<str>>,
    pub fps: f32,
    pub in_world: bool,
    pub runtime: crate::diag::Runtime,
    pub frame_history: VecDeque<f32>,
    pub nametags: Vec<NameTag>,
    pub active_effects: Vec<crate::play::mob_effects::MobEffectInstance>,
    pub debug: Option<DebugInfo>,
}

#[derive(Default)]
pub struct DebugInfo {
    pub page: DebugPage,
    pub frame_ms: f32,
    pub memory: MemoryUse,
    pub profiling: ChunkProfiling,
    pub pipeline: crate::diag::Counts,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum DebugPage {
    #[default]
    Overview,
    Performance,
    World,
    Network,
    Workers,
    Memory,
    Help,
}

#[derive(Clone, Copy, Default)]
pub struct MemoryUse {
    pub terrain_bytes: u64,
    pub sections: usize,
    pub world_bytes: u64,
    pub world_chunks: usize,
    pub light_bytes: u64,
    pub light_sections: usize,
    pub light_engine_bytes: u64,
    pub light_engine_sections: usize,
    pub baked_bytes: u64,
    pub baked_states: usize,
    pub baked_models: usize,
    pub texture_bytes: u64,
    pub textures: usize,
    pub entities: usize,
    pub entity_nodes: usize,
    pub block_entities: usize,
}

fn bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else if value < 10.0 {
        format!("{value:.2} {}", UNITS[unit])
    } else if value < 100.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

fn rate(n: f64) -> String {
    format!("{}/s", bytes(n.max(0.0) as u64))
}

pub const TEXT_COLOR: u32 = 0xFFFFFF;

fn facing_name(d: Direction) -> &'static str {
    match d {
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
        Direction::Down => "down",
        Direction::Up => "up",
    }
}

fn facing_towards(d: Direction) -> &'static str {
    match d {
        Direction::North => "Towards negative Z",
        Direction::South => "Towards positive Z",
        Direction::West => "Towards negative X",
        Direction::East => "Towards positive X",
        Direction::Down => "Towards negative Y",
        Direction::Up => "Towards positive Y",
    }
}

fn facing_from_bevy_yaw(yaw: f32) -> Direction {
    let idx = (yaw / 90.0 + 0.5).floor() as i32;
    match idx.rem_euclid(4) {
        0 => Direction::North,
        1 => Direction::West,
        2 => Direction::South,
        _ => Direction::East,
    }
}

fn gamemode_name(g: Gamemode) -> &'static str {
    match g {
        Gamemode::Survival => "Survival",
        Gamemode::Creative => "Creative",
        Gamemode::Adventure => "Adventure",
        Gamemode::Spectator => "Spectator",
    }
}

struct Line {
    text: String,
    color: u32,
    text_w: f32,
    width: f32,
    fixed: bool,
    tag: Option<(String, f32, f32)>,
}

const WIDEST_XYZ: &str = "XYZ: -30000000.00 / -2048.00 / -30000000.00";

const GAMEMODES: [Gamemode; 4] = [
    Gamemode::Survival,
    Gamemode::Creative,
    Gamemode::Adventure,
    Gamemode::Spectator,
];

const HORIZONTAL: [Direction; 4] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub fn draw(
    p: &mut Painter,
    ctx: &ScreenCtx,
    info: &HudInfo,
    sidebar: &crate::client::tablist::Sidebar,
    chat_open: bool,
    show_fps: bool,
    hud: &mut Hud,
) {
    use crate::gui::hud_layout::{Anchor, COUNT, Home, Place};
    use bevy::math::Vec2;

    let (vw, vh) = (ctx.vw, ctx.vh);
    let editing = hud.editing();
    let atlas = p.atlas;
    let width = |s: &str| atlas.font.width_str(s);
    let line = |text: String, color: u32, widest: f32| {
        let text_w = width(&text);
        Line {
            text,
            color,
            text_w,
            width: text_w.max(widest),
            fixed: false,
            tag: None,
        }
    };
    let widest_of = |names: &mut dyn Iterator<Item = f32>| names.fold(0.0, f32::max);
    let widest_gamemode = widest_of(&mut GAMEMODES.iter().map(|g| width(gamemode_name(*g))));

    let mut lines: [Option<Line>; COUNT] = Default::default();
    let mut put = |id: ElementId, l: Line| lines[id.index()] = Some(l);

    let gamemode = gamemode_name(info.gamemode);
    let inline_tag =
        hud.place(ElementId::Gamemode) == Place::Fixed && hud.shown(ElementId::Gamemode);
    if hud.shown(ElementId::Coords) {
        let mut l = line(
            format!(
                "XYZ: {:.2} / {:.2} / {:.2}",
                info.pos[0], info.pos[1], info.pos[2]
            ),
            TEXT_COLOR,
            width(WIDEST_XYZ),
        );
        if inline_tag {
            let tag = format!("  [{gamemode}]");
            let w = width("  [") + widest_gamemode + width("]");
            let drawn = width(&tag);
            l.tag = Some((tag, w, drawn));
        }
        put(ElementId::Coords, l);
    }
    if !inline_tag && hud.shown(ElementId::Gamemode) {
        let w = width("[") + widest_gamemode + width("]");
        put(ElementId::Gamemode, line(format!("[{gamemode}]"), GOLD, w));
    }
    if hud.shown(ElementId::Direction) {
        let text = |d: Direction| format!("Direction: {} ({})", facing_name(d), facing_towards(d));
        let w = width("Direction: ")
            + widest_of(&mut HORIZONTAL.into_iter().map(|d| {
                width(facing_name(d)) + width(" (") + width(facing_towards(d)) + width(")")
            }));
        put(
            ElementId::Direction,
            line(text(facing_from_bevy_yaw(info.yaw)), TEXT_COLOR, w),
        );
    }
    if (info.flying || editing) && hud.shown(ElementId::Flying) {
        put(
            ElementId::Flying,
            line("Flying".to_owned(), FLYING_COLOR, 0.0),
        );
    }
    let status_live = info.status.as_deref().filter(|text| !text.is_empty());
    if (status_live.is_some() || editing) && hud.shown(ElementId::Status) {
        let text = status_live.unwrap_or("Connecting...").to_owned();
        put(ElementId::Status, line(text, STATUS_COLOR, 0.0));
    }
    if (show_fps || editing) && hud.shown(ElementId::Fps) {
        let mut l = line(
            format!("FPS: {:.0}", info.fps.max(0.0)),
            TEXT_COLOR,
            width("FPS: 9999"),
        );
        l.fixed = true;
        put(ElementId::Fps, l);
    }
    if crate::diag::MEMORY_METRICS && hud.shown(ElementId::Ram) {
        let mut l = line(
            format!("RAM: {}", bytes(info.runtime.rss_bytes)),
            TEXT_COLOR,
            width("RAM: 9999.99 GiB"),
        );
        l.fixed = true;
        put(ElementId::Ram, l);
    }
    if (info.freecam || editing) && hud.shown(ElementId::Freecam) {
        put(
            ElementId::Freecam,
            line("[ FREECAM ]".to_owned(), GREEN, 0.0),
        );
    }

    let mut sizes = [None; COUNT];
    for id in ElementId::ALL {
        if let Some(l) = &lines[id.index()] {
            let w = l.width + l.tag.as_ref().map_or(0.0, |(_, w, _)| *w);
            sizes[id.index()] = Some(Rect::new(0.0, 0.0, w, LINE_HEIGHT));
        }
    }
    let effects = !info.active_effects.is_empty() || editing;
    if effects && hud.shown(ElementId::Effects) {
        sizes[ElementId::Effects.index()] = Some(hud.natural_of(ElementId::Effects, vw, vh));
    }

    let home_at = |home: Home| -> Option<(Anchor, Vec2)> {
        #[cfg(feature = "mobile_ui")]
        if home == Home::Corner {
            if chat_open {
                return Some((Anchor::TopRight, Vec2::new(2.0, 2.0)));
            }
            let pad =
                crate::mobile::pad::Layout::new(vw, vh, 0.0, crate::mobile::pad::Movement::Buttons);
            return Some((
                Anchor::BottomRight,
                Vec2::new(2.0, vh - (pad.use_button.y - 2.0)),
            ));
        }
        let _ = (home, chat_open);
        None
    };
    #[cfg(feature = "mobile_ui")]
    let fixed_at = {
        let fixed_h = |id: ElementId| match hud.place(id) {
            Place::Fixed => sizes[id.index()].map_or(0.0, |r| r.h),
            _ => 0.0,
        };
        let ram_y = sidebar_top(ctx, sidebar) - 1.0 - fixed_h(ElementId::Ram);
        let fps_y = ram_y - fixed_h(ElementId::Fps);
        move |id: ElementId| -> Option<(Anchor, Vec2)> {
            let y = match id {
                ElementId::Fps => fps_y,
                ElementId::Ram => ram_y,
                _ => return None,
            };
            Some((Anchor::TopRight, Vec2::new(2.0, y)))
        }
    };
    #[cfg(not(feature = "mobile_ui"))]
    let fixed_at = {
        let _ = sidebar;
        |_: ElementId| -> Option<(Anchor, Vec2)> { None }
    };
    let slots = hud.arrange(vw, vh, &sizes, home_at, fixed_at);
    for id in ElementId::ALL {
        let (Some(slot), Some(natural)) = (slots[id.index()], sizes[id.index()]) else {
            continue;
        };
        if id == ElementId::Effects {
            hud.draw_at(p, id, natural, slot, true, |p| {
                draw_effects(p, ctx, &info.active_effects);
            });
            continue;
        }
        let Some(l) = lines[id.index()].take() else {
            continue;
        };
        let drawn = l.text_w + l.tag.as_ref().map_or(0.0, |(_, _, d)| *d);
        let x = match slot.align {
            _ if l.fixed => 0.0,
            2 => (natural.w - drawn).floor(),
            1 => ((natural.w - drawn) / 2.0).floor(),
            _ => 0.0,
        };
        hud.draw_at(p, id, natural, slot, false, |p| {
            let end = p.text_str(&l.text, x, 0.0, l.color, true);
            if let Some((tag, ..)) = &l.tag {
                p.text_str(tag, end, 0.0, GOLD, true);
            }
        });
        if let Some((_, _, w)) = l.tag {
            let s = slot.scale;
            hud.attach(
                ElementId::Gamemode,
                Rect::new(slot.x + (x + l.text_w) * s, slot.y, w * s, LINE_HEIGHT * s),
            );
        }
    }
}

struct Row {
    label: &'static str,
    value: String,
    color: u32,
}

fn row(label: &'static str, value: String) -> Row {
    Row {
        label,
        value,
        color: VALUE_COLOR,
    }
}

fn row_c(label: &'static str, value: String, color: u32) -> Row {
    Row {
        label,
        value,
        color,
    }
}

fn na_row(label: &'static str, in_world: bool, real: String) -> Item<'static> {
    if in_world {
        row(label, real).into()
    } else {
        row(label, "N/A".to_string()).into()
    }
}

fn na_row_c(label: &'static str, in_world: bool, real: String, color: u32) -> Item<'static> {
    if in_world {
        row_c(label, real, color).into()
    } else {
        row(label, "N/A".to_string()).into()
    }
}

enum Item<'a> {
    Row(Row),
    Graph(&'a VecDeque<f32>),
    Divider,
}

const DIVIDER_HEIGHT: f32 = 4.0;

impl From<Row> for Item<'_> {
    fn from(r: Row) -> Self {
        Item::Row(r)
    }
}

const GRAPH_WIDTH: f32 = 130.0;
const GRAPH_HEIGHT: f32 = 22.0;
const SECTION_GAP: f32 = 3.0;
const PAD_X: f32 = 3.0;
const PAD_Y: f32 = 2.0;
const VALUE_GAP: f32 = 6.0;

fn draw_section(p: &mut Painter, title: &str, items: &[Item], right_x: f32, top_y: f32) -> f32 {
    if items.is_empty() {
        return top_y;
    }
    let label_w = items
        .iter()
        .filter_map(|it| match it {
            Item::Row(r) => Some(p.atlas.font.width_str(r.label)),
            Item::Graph(_) | Item::Divider => None,
        })
        .fold(0.0_f32, f32::max);
    let value_w = items
        .iter()
        .filter_map(|it| match it {
            Item::Row(r) => Some(p.atlas.font.width_str(&r.value)),
            Item::Graph(_) | Item::Divider => None,
        })
        .fold(0.0_f32, f32::max);
    let title_w = p.atlas.font.width_str(title);
    let content_w = (label_w + VALUE_GAP + value_w)
        .max(GRAPH_WIDTH)
        .max(title_w);
    let box_w = (content_w + PAD_X * 2.0).floor();
    let box_x = (right_x - box_w).floor();

    let content_h: f32 = items
        .iter()
        .map(|it| match it {
            Item::Graph(_) => GRAPH_HEIGHT,
            Item::Divider => DIVIDER_HEIGHT,
            Item::Row(_) => LINE_HEIGHT,
        })
        .sum();
    let rule_y = (top_y + PAD_Y + LINE_HEIGHT).floor();
    let box_h = (PAD_Y + LINE_HEIGHT + 2.0 + content_h + PAD_Y).floor();

    p.fill(box_x, top_y.floor(), box_w, box_h, DEBUG_BG);
    p.text_str(
        title,
        (box_x + PAD_X).floor(),
        (top_y + PAD_Y).floor(),
        HEADER_COLOR,
        true,
    );
    p.fill(box_x + PAD_X, rule_y, box_w - PAD_X * 2.0, 1.0, 0x40FF_FFFF);

    let mut y = rule_y + 2.0;
    for item in items {
        match item {
            Item::Row(r) => {
                p.text_str(
                    r.label,
                    (box_x + PAD_X).floor(),
                    y.floor(),
                    LABEL_COLOR,
                    true,
                );
                let vw = p.atlas.font.width_str(&r.value);
                p.text_str(
                    &r.value,
                    (right_x - PAD_X - vw).floor(),
                    y.floor(),
                    r.color,
                    true,
                );
                y += LINE_HEIGHT;
            }
            Item::Graph(samples) => {
                draw_frame_graph(p, samples, box_x + PAD_X, y, content_w, GRAPH_HEIGHT - 2.0);
                y += GRAPH_HEIGHT;
            }
            Item::Divider => {
                p.fill(
                    box_x + PAD_X,
                    (y + DIVIDER_HEIGHT / 2.0).floor(),
                    box_w - PAD_X * 2.0,
                    1.0,
                    0x40FF_FFFF,
                );
                y += DIVIDER_HEIGHT;
            }
        }
    }
    top_y + box_h + SECTION_GAP
}

fn draw_frame_graph(p: &mut Painter, samples: &VecDeque<f32>, x: f32, y: f32, w: f32, h: f32) {
    p.fill(x, y, w, h, 0x6000_0000);
    if samples.is_empty() {
        return;
    }
    let bar_w = (w / samples.len() as f32).max(1.0);
    for (i, &ms) in samples.iter().enumerate() {
        let frac = (ms / MS_30FPS).clamp(0.03, 1.0);
        let bar_h = (h * frac).floor();
        let bx = (x + i as f32 * bar_w).floor();
        p.fill(
            bx,
            (y + h - bar_h).floor(),
            bar_w.ceil(),
            bar_h,
            frame_ms_color(ms),
        );
    }
}

fn frame_history_stats(history: &VecDeque<f32>) -> (f32, f32, f32) {
    let mut buf = [0.0f32; FRAME_HISTORY_LEN];
    let n = history.len().min(FRAME_HISTORY_LEN);
    if n == 0 {
        return (0.0, 0.0, 0.0);
    }
    for (slot, ms) in buf.iter_mut().zip(history) {
        *slot = *ms;
    }
    let sorted = &mut buf[..n];
    sorted.sort_unstable_by(f32::total_cmp);
    let avg = sorted.iter().sum::<f32>() / n as f32;
    let p95 = sorted[((n - 1) as f32 * 0.95).round() as usize];
    (avg, p95, sorted[n - 1])
}

pub fn draw_debug(p: &mut Painter, ctx: &ScreenCtx, info: &HudInfo, d: &DebugInfo) {
    let right_x = ctx.vw - 2.0;
    let mut y = 2.0_f32;
    for (title, items) in sections_for(info, d) {
        y = draw_section(p, title, &items, right_x, y);
    }
    if d.page != DebugPage::Help {
        draw_footnote(p, "F3+0 for shortcuts", right_x, y);
    }
}

fn draw_footnote(p: &mut Painter, text: &str, right_x: f32, top_y: f32) {
    let w = p.atlas.font.width_str(text);
    p.text_str(
        text,
        (right_x - w).floor(),
        top_y.floor(),
        LABEL_COLOR,
        true,
    );
}

fn sections_for<'a>(info: &'a HudInfo, d: &DebugInfo) -> Vec<(&'static str, Vec<Item<'a>>)> {
    match d.page {
        DebugPage::Overview => overview_sections(info, d),
        DebugPage::Performance => vec![("PERFORMANCE", performance_items(info, d))],
        DebugPage::World => vec![("WORLD", world_items(info, d))],
        DebugPage::Network => vec![("NETWORK", network_items(info))],
        DebugPage::Workers => vec![("CHUNK WORKERS", workers_items(info, d))],
        DebugPage::Memory => vec![("MEMORY", memory_items(info, d))],
        DebugPage::Help => vec![("F3 SHORTCUTS", help_items())],
    }
}

fn help_items() -> Vec<Item<'static>> {
    vec![
        row("F3+1", "Overview".to_string()).into(),
        row("F3+2", "Performance".to_string()).into(),
        row("F3+3", "World".to_string()).into(),
        row("F3+4", "Network".to_string()).into(),
        row("F3+5", "Workers".to_string()).into(),
        row("F3+6", "Memory".to_string()).into(),
        row("F3+0", "This page".to_string()).into(),
        Item::Divider,
        row("F3+H", "Advanced tooltips".to_string()).into(),
        row("F3+G", "Chunk borders".to_string()).into(),
        row("F3+A", "Reload chunks".to_string()).into(),
    ]
}

fn performance_items<'a>(info: &'a HudInfo, d: &DebugInfo) -> Vec<Item<'a>> {
    let rt = &info.runtime;
    let (avg, p95, max) = frame_history_stats(&info.frame_history);
    vec![
        row_c(
            "FPS",
            format!("{:.0}", info.fps),
            frame_ms_color(d.frame_ms),
        )
        .into(),
        row_c(
            "Frame",
            format!("{:.2} ms", d.frame_ms),
            frame_ms_color(d.frame_ms),
        )
        .into(),
        row(
            "CPU",
            if cfg!(target_arch = "wasm32") {
                "n/a (no CPU-time API in a browser)".to_string()
            } else if !crate::diag::CPU_METRICS {
                "n/a (no process CPU clock read on this platform)".to_string()
            } else {
                format!("{:.0}% of {} cores", rt.cpu_percent, rt.cores)
            },
        )
        .into(),
        Item::Graph(&info.frame_history),
        row("Avg", format!("{avg:.2} ms")).into(),
        row("P95", format!("{p95:.2} ms")).into(),
        row_c("Max", format!("{max:.2} ms"), frame_ms_color(max)).into(),
    ]
}

fn world_items(info: &HudInfo, d: &DebugInfo) -> Vec<Item<'static>> {
    let pipe = &d.pipeline;
    let mem = &d.memory;
    let in_world = info.in_world;
    let dropped_total =
        pipe[Stat::ChunksDropped] + pipe[Stat::LightJobsDropped] + pipe[Stat::MeshPanics];
    vec![
        na_row(
            "Chunks in",
            in_world,
            format!("{}", pipe[Stat::ChunksReceived]),
        ),
        na_row(
            "Columns lit",
            in_world,
            format!("{}", pipe[Stat::ColumnsLit]),
        ),
        na_row_c(
            "Dropped",
            in_world,
            format!(
                "{} chunk / {} light / {} panic",
                pipe[Stat::ChunksDropped],
                pipe[Stat::LightJobsDropped],
                pipe[Stat::MeshPanics]
            ),
            count_color(dropped_total, 1, 50),
        ),
        na_row(
            "World",
            in_world,
            format!("{} in {} chunks", bytes(mem.world_bytes), mem.world_chunks),
        ),
        na_row(
            "Terrain mesh",
            in_world,
            format!(
                "{} in {} slots{}",
                bytes(mem.terrain_bytes),
                mem.sections,
                crate::renderer::terrain_pool::stats()
                    .map(|s| &s.0)
                    .filter(|s| s.indirect.load(std::sync::atomic::Ordering::Relaxed) == 0)
                    .map_or(String::new(), |s| format!(
                        ", {} drawn in {} draws",
                        s.drawn.load(std::sync::atomic::Ordering::Relaxed),
                        s.draws.load(std::sync::atomic::Ordering::Relaxed)
                    ))
            ),
        ),
        na_row(
            "Light",
            in_world,
            format!(
                "{} in {} sections",
                bytes(mem.light_bytes),
                mem.light_sections
            ),
        ),
    ]
}

fn network_items(info: &HudInfo) -> Vec<Item<'static>> {
    let rt = &info.runtime;
    vec![
        row(
            "Down",
            format!("{} ({})", rate(rt.rx_per_sec), bytes(rt.rx_total)),
        )
        .into(),
        row(
            "Up",
            format!("{} ({})", rate(rt.tx_per_sec), bytes(rt.tx_total)),
        )
        .into(),
    ]
}

fn workers_items(info: &HudInfo, d: &DebugInfo) -> Vec<Item<'static>> {
    let prof = &d.profiling;
    let in_world = info.in_world;
    vec![
        row("Workers", format!("{}", prof.worker_count)).into(),
        na_row("Meshes", in_world, format!("{}", prof.chunks_meshed)),
        na_row(
            "Superseded",
            in_world,
            format!("{}", d.pipeline[Stat::MeshJobsSuperseded]),
        ),
        na_row_c(
            "Mesh avg",
            in_world,
            format!("{:.2} ms", prof.mesh_avg_ms),
            worker_ms_color(prof.mesh_avg_ms),
        ),
        na_row_c(
            "Mesh peak",
            in_world,
            format!("{:.2} ms", prof.mesh_peak_ms),
            worker_ms_color(prof.mesh_peak_ms),
        ),
        na_row_c(
            "Upload queue",
            in_world,
            format!("{}", prof.queue_depth),
            count_color(prof.queue_depth as u64, 4, 16),
        ),
        na_row("Poll avg", in_world, format!("{:.2} ms", prof.poll_avg_ms)),
        na_row(
            "Poll peak",
            in_world,
            format!("{:.2} ms", prof.poll_peak_ms),
        ),
        na_row(
            "Lock wait",
            in_world,
            format!("{:.3} ms", prof.lock_wait_avg_ms),
        ),
    ]
}

fn memory_items(info: &HudInfo, d: &DebugInfo) -> Vec<Item<'static>> {
    let rt = &info.runtime;
    let mem = &d.memory;
    let in_world = info.in_world;
    let accounted = mem.terrain_bytes
        + mem.world_bytes
        + mem.light_bytes
        + mem.light_engine_bytes
        + mem.baked_bytes
        + mem.texture_bytes;
    let other = rt.rss_bytes.saturating_sub(accounted);

    let mut items: Vec<Item<'static>> = Vec::new();
    if crate::diag::MEMORY_METRICS {
        items.push(row("RSS", bytes(rt.rss_bytes)).into());
    }
    items.extend([
        na_row(
            "World",
            in_world,
            format!("{} in {} chunks", bytes(mem.world_bytes), mem.world_chunks),
        ),
        na_row(
            "Terrain mesh",
            in_world,
            format!(
                "{} in {} column meshes",
                bytes(mem.terrain_bytes),
                mem.sections
            ),
        ),
        na_row(
            "Light",
            in_world,
            format!(
                "{} in {} sections",
                bytes(mem.light_bytes),
                mem.light_sections
            ),
        ),
        na_row(
            "Light engine",
            in_world,
            format!(
                "{} in {} sections",
                bytes(mem.light_engine_bytes),
                mem.light_engine_sections
            ),
        ),
        row(
            "Baked blocks",
            format!(
                "{} in {} states / {} models",
                bytes(mem.baked_bytes),
                mem.baked_states,
                mem.baked_models
            ),
        )
        .into(),
        row(
            "Textures",
            format!("{} in {}", bytes(mem.texture_bytes), mem.textures),
        )
        .into(),
        na_row(
            "Entities",
            in_world,
            format!("{} ({} nodes)", mem.entities, mem.entity_nodes),
        ),
        na_row(
            "Block entities",
            in_world,
            format!("{}", mem.block_entities),
        ),
    ]);
    if crate::diag::MEMORY_METRICS {
        items.push(row("Other", bytes(other)).into());
        items.push(
            row(
                "RSS anon/mapped",
                format!(
                    "{} / {}",
                    bytes(rt.rss_anon_bytes),
                    bytes(rt.rss_bytes.saturating_sub(rt.rss_anon_bytes))
                ),
            )
            .into(),
        );
    }
    items.extend([
        row(
            "Heap live",
            format!(
                "{} in {} allocs",
                bytes(rt.heap_live_bytes),
                rt.heap_live_allocs
            ),
        )
        .into(),
        row(
            "Heap churn",
            format!("{}/s", bytes(rt.heap_alloc_per_sec as u64)),
        )
        .into(),
        row("Churn frame", churn_row(rt, &[0, 1, 2, 9])).into(),
        row("Churn threads", churn_row(rt, &[3, 4, 5, 6, 7, 8])).into(),
    ]);
    items
}

fn churn_row(rt: &crate::diag::Runtime, sites: &[usize]) -> String {
    let named: Vec<String> = sites
        .iter()
        .filter(|&&i| rt.heap_churn_by_site[i] > 0.0)
        .map(|&i| {
            format!(
                "{} {}",
                crate::diag::alloc::SITES[i],
                bytes(rt.heap_churn_by_site[i] as u64)
            )
        })
        .collect();
    if named.is_empty() {
        "idle".into()
    } else {
        named.join("  ")
    }
}

fn overview_sections(info: &HudInfo, d: &DebugInfo) -> Vec<(&'static str, Vec<Item<'static>>)> {
    let rt = &info.runtime;
    let prof = &d.profiling;
    let pipe = &d.pipeline;
    let mem = &d.memory;
    let in_world = info.in_world;
    let dropped_total =
        pipe[Stat::ChunksDropped] + pipe[Stat::LightJobsDropped] + pipe[Stat::MeshPanics];
    let mut perf: Vec<Item<'static>> = vec![
        row_c(
            "FPS",
            format!("{:.0}", info.fps),
            frame_ms_color(d.frame_ms),
        )
        .into(),
        row_c(
            "Frame",
            format!("{:.2} ms", d.frame_ms),
            frame_ms_color(d.frame_ms),
        )
        .into(),
        row(
            "CPU",
            if cfg!(target_arch = "wasm32") || !crate::diag::CPU_METRICS {
                "n/a".to_string()
            } else {
                format!("{:.0}%", rt.cpu_percent)
            },
        )
        .into(),
    ];
    if crate::diag::MEMORY_METRICS {
        perf.push(row("Mem", bytes(rt.rss_bytes)).into());
    }
    vec![
        ("PERFORMANCE", perf),
        (
            "WORLD",
            vec![
                na_row("Chunks", in_world, format!("{}", mem.world_chunks)),
                na_row("Entities", in_world, format!("{}", mem.entities)),
                na_row_c(
                    "Dropped",
                    in_world,
                    format!("{dropped_total}"),
                    count_color(dropped_total, 1, 50),
                ),
            ],
        ),
        (
            "NETWORK",
            vec![
                row("Down", rate(rt.rx_per_sec)).into(),
                row("Up", rate(rt.tx_per_sec)).into(),
            ],
        ),
        (
            "WORKERS",
            vec![
                na_row_c(
                    "Upload queue",
                    in_world,
                    format!("{}", prof.queue_depth),
                    count_color(prof.queue_depth as u64, 4, 16),
                ),
                na_row_c(
                    "Mesh peak",
                    in_world,
                    format!("{:.2} ms", prof.mesh_peak_ms),
                    worker_ms_color(prof.mesh_peak_ms),
                ),
            ],
        ),
    ]
}

pub fn draw_nametags(p: &mut Painter, _ctx: &ScreenCtx, tags: &[NameTag]) {
    let mut scratch = String::new();
    let (on, show_distance, health_number, health_bar) = match crate::modules::nametags::config() {
        Some(c) => (true, c.distance, c.health.number(), c.health.bar()),
        None => (false, false, false, false),
    };
    let bg = if on { NAMETAG_BG_MODULE } else { NAMETAG_BG };
    let shadow = on;
    let space = p.atlas.font.width_str(" ");

    for tag in tags {
        if tag.scale <= 0.0 {
            continue;
        }
        p.scaled(tag.scale, tag.x, tag.y, |p| {
            let plate = |p: &mut Painter, x: f32, y: f32, w: f32| {
                p.fill(x - 1.0, y - 1.0, w + 2.0, 10.0, bg);
            };

            if !tag.below.is_empty() {
                let w = p.atlas.font.width(&tag.below);
                plate(p, -w / 2.0, 0.0, w);
                p.text(&tag.below, -w / 2.0, 0.0, shadow);
            }
            let y = if tag.below.is_empty() {
                0.0
            } else {
                -BELOW_NAME_GAP
            };

            let prefix = match tag.gamemode {
                TagGamemode::Hidden => "",
                TagGamemode::Unknown => "[BOT] ",
                TagGamemode::Known(mode) => gamemode_prefix(mode),
            };
            let health = tag.health.filter(|_| health_number);
            scratch.clear();
            if let Some(hp) = health {
                use std::fmt::Write;
                let _ = write!(scratch, " {}", hp.max(0.0).round() as i32);
            }
            let health_len = scratch.len();
            if show_distance {
                use std::fmt::Write;
                let _ = write!(scratch, " {:.1}m", tag.distance);
            }
            let (health_text, distance_text) = scratch.split_at(health_len);

            let prefix_w = p.atlas.font.width_str(prefix);
            let name_w = if on {
                white_run_width(p, &tag.name)
            } else {
                p.atlas.font.width(&tag.name)
            };
            let health_w = p.atlas.font.width_str(health_text);
            let distance_w = p.atlas.font.width_str(distance_text);
            let w = prefix_w + name_w + health_w + distance_w;

            let mut x = -w / 2.0;
            plate(p, x, y, w);
            if !prefix.is_empty() {
                x = p.text_plain(prefix, x, y, GAMEMODE_COLOR, shadow);
            }
            x = if on {
                white_run(p, &tag.name, x, y, shadow)
            } else {
                p.text(&tag.name, x, y, false)
            };
            if !health_text.is_empty() {
                let ratio = health.map_or(0.0, |hp| health_ratio(hp, tag.max_health));
                x = p.text_plain(health_text, x, y, health_color(ratio), shadow);
            }
            if !distance_text.is_empty() {
                p.text_plain(distance_text, x, y, DISTANCE_COLOR, shadow);
            }

            if let Some(hp) = tag.health.filter(|_| health_bar) {
                let ratio = health_ratio(hp, tag.max_health);
                let bar_y = y - 1.0 - HEALTH_BAR_GAP - HEALTH_BAR_H;
                let x = -w / 2.0;
                p.fill(
                    x - 1.0,
                    bar_y - 1.0,
                    w + 2.0,
                    HEALTH_BAR_H + 2.0,
                    NAMETAG_BG,
                );
                p.fill(
                    x,
                    bar_y,
                    w * ratio,
                    HEALTH_BAR_H,
                    0xFF00_0000 | health_color(ratio),
                );
            }
        });
    }
}

fn white_run_width(p: &Painter, spans: &[crate::text::Span]) -> f32 {
    spans.iter().map(|s| p.atlas.font.width_str(&s.text)).sum()
}

fn white_run(p: &mut Painter, spans: &[crate::text::Span], x: f32, y: f32, shadow: bool) -> f32 {
    let mut pen = x;
    for span in spans {
        pen = p.text_plain(&span.text, pen, y, 0xFF_FFFF, shadow);
    }
    pen
}

fn gamemode_prefix(mode: Gamemode) -> &'static str {
    match mode {
        Gamemode::Survival => "[S] ",
        Gamemode::Creative => "[C] ",
        Gamemode::Adventure => "[A] ",
        Gamemode::Spectator => "[Sp] ",
    }
}

fn health_ratio(health: f32, max_health: f32) -> f32 {
    let max = if max_health > 0.0 {
        max_health
    } else {
        crate::session::VANILLA_MAX_HEALTH
    };
    (health / health.max(max)).clamp(0.0, 1.0)
}

const GAMEMODE_COLOR: u32 = 0xE8_B923;
const DISTANCE_COLOR: u32 = 0x96_9696;

fn health_color(ratio: f32) -> u32 {
    if ratio <= 0.333 {
        0xFF_1919
    } else if ratio <= 0.666 {
        0xFF_6919
    } else {
        0x19_FC19
    }
}

fn effect_icon_alpha(duration: i32) -> f32 {
    if duration < 0 || duration > 200 {
        return 1.0;
    }
    let remaining = duration as f32;
    let used_seconds = 10.0 - remaining / 20.0;
    let alpha = (remaining / 10.0 / 5.0 * 0.5).clamp(0.0, 0.5)
        + (remaining * std::f32::consts::PI / 5.0).cos()
            * (used_seconds / 10.0 * 0.25).clamp(0.0, 0.25);
    alpha.clamp(0.0, 1.0)
}

pub fn draw_effects(
    p: &mut Painter,
    ctx: &ScreenCtx,
    effects: &[crate::play::mob_effects::MobEffectInstance],
) {
    let mut sorted: Vec<&crate::play::mob_effects::MobEffectInstance> = effects.iter().collect();
    sorted.sort_by(|a, b| crate::play::mob_effects::compare(a, b));

    let mut beneficial_count = 0u32;
    let mut harmful_count = 0u32;
    for effect in sorted.iter().rev() {
        if !effect.show_icon {
            continue;
        }
        let mut x = ctx.vw;
        let mut y = 1.0_f32;
        if crate::play::mob_effects::is_beneficial(&effect.id) {
            beneficial_count += 1;
            x -= 25.0 * beneficial_count as f32;
        } else {
            harmful_count += 1;
            x -= 25.0 * harmful_count as f32;
            y += 26.0;
        }

        let (bg, alpha) = if effect.ambient {
            ("hud/effect_background_ambient", 1.0)
        } else {
            ("hud/effect_background", effect_icon_alpha(effect.duration))
        };
        p.sprite(bg, x.floor(), y.floor(), 24.0, 24.0);

        let icon = format!("mob_effect/{}", effect.id);
        let tint = ((alpha.clamp(0.0, 1.0) * 255.0).round() as u32) << 24 | 0x00FF_FFFF;
        p.sprite_tinted(
            &icon,
            (x + 3.0).floor(),
            (y + 3.0).floor(),
            18.0,
            18.0,
            tint,
        );
    }
}

const TITLE_FADE_IN: f32 = 10.0;
const TITLE_STAY: f32 = 70.0;
const TITLE_FADE_OUT: f32 = 20.0;
const ACTION_BAR_TICKS: f32 = 60.0;
const HELD_NAME_TICKS: f32 = 40.0;
const TICKS_PER_SECOND: f32 = 20.0;

pub struct HudOverlays {
    title: Vec<crate::text::Span>,
    subtitle: Vec<crate::text::Span>,
    fade_in: f32,
    stay: f32,
    fade_out: f32,
    title_ends: f32,
    action_bar: Vec<crate::text::Span>,
    action_bar_ends: f32,
    held: Vec<crate::text::Span>,
    held_ends: f32,
    held_item: &'static str,
    held_custom: Option<Vec<crate::text::Span>>,
    held_potion: Option<crate::items::potions::PotionContents>,
}

impl Default for HudOverlays {
    fn default() -> Self {
        HudOverlays {
            title: Vec::new(),
            subtitle: Vec::new(),
            fade_in: TITLE_FADE_IN,
            stay: TITLE_STAY,
            fade_out: TITLE_FADE_OUT,
            title_ends: 0.0,
            action_bar: Vec::new(),
            action_bar_ends: 0.0,
            held: Vec::new(),
            held_ends: 0.0,
            held_item: "",
            held_custom: None,
            held_potion: None,
        }
    }
}

impl HudOverlays {
    fn title_total(&self) -> f32 {
        self.fade_in + self.stay + self.fade_out
    }

    pub fn update(&mut self, snap: &mut crate::gui::Snapshot, time_secs: f32) {
        let now = time_secs * TICKS_PER_SECOND;
        if let Some(reset_times) = snap.titles_clear.take() {
            self.title.clear();
            self.subtitle.clear();
            self.title_ends = 0.0;
            if reset_times {
                self.fade_in = TITLE_FADE_IN;
                self.stay = TITLE_STAY;
                self.fade_out = TITLE_FADE_OUT;
            }
        }
        if let Some((fade_in, stay, fade_out)) = snap.title_times.take() {
            if let Some(v) = fade_in {
                self.fade_in = v as f32;
            }
            if let Some(v) = stay {
                self.stay = v as f32;
            }
            if let Some(v) = fade_out {
                self.fade_out = v as f32;
            }
            if self.title_ends > now {
                self.title_ends = now + self.title_total();
            }
        }
        if let Some(title) = snap.title_text.take() {
            self.title = title;
            self.title_ends = now + self.title_total();
        }
        if let Some(subtitle) = snap.subtitle_text.take() {
            self.subtitle = subtitle;
        }
        if self.title_ends <= now && !self.title.is_empty() {
            self.title.clear();
            self.subtitle.clear();
        }
        if let Some(text) = snap.action_bar.take() {
            self.action_bar = text;
            self.action_bar_ends = now + ACTION_BAR_TICKS;
        }
        self.update_held(snap.hotbar(snap.selected as usize), now);
    }

    fn update_held(&mut self, stack: &crate::session::SlotStack, now: f32) {
        if stack.is_empty() {
            self.held_ends = 0.0;
            self.held_item = "";
            self.held_custom = None;
            self.held_potion = None;
            return;
        }
        if self.held_item == stack.item
            && self.held_custom == stack.custom_name
            && self.held_potion == stack.potion
        {
            return;
        }
        self.held_item = stack.item;
        self.held_custom.clone_from(&stack.custom_name);
        self.held_potion.clone_from(&stack.potion);
        self.held = crate::gui::tooltip::styled_hover_name(stack);
        self.held_ends = now + HELD_NAME_TICKS;
    }
}

fn held_name_alpha(remaining: f32) -> f32 {
    if remaining <= 0.0 {
        return 0.0;
    }
    (remaining.ceil() * 256.0 / 10.0).floor().min(255.0) / 255.0
}

fn action_bar_alpha(remaining: f32) -> f32 {
    if remaining <= 0.0 {
        return 0.0;
    }
    (remaining * 255.0 / 20.0).floor().min(255.0) / 255.0
}

fn title_alpha(remaining: f32, fade_in: f32, stay: f32, fade_out: f32) -> f32 {
    if remaining <= 0.0 {
        return 0.0;
    }
    let ticks_left = remaining.ceil();
    let mut alpha = 255.0;
    if ticks_left > fade_out + stay {
        alpha = ((fade_in + stay + fade_out) - remaining) * 255.0 / fade_in;
    }
    if ticks_left <= fade_out {
        alpha = remaining * 255.0 / fade_out;
    }
    alpha.floor().clamp(0.0, 255.0) / 255.0
}

pub fn draw_overlays(
    p: &mut Painter,
    ctx: &ScreenCtx,
    ov: &HudOverlays,
    gamemode: Gamemode,
    elements: &mut Hud,
) {
    let now = ctx.input.time * TICKS_PER_SECOND;
    let (vw, vh) = (gui_width(ctx), gui_height(ctx));

    elements.follow(p, ctx.vw, ctx.vh, ElementId::Hotbar, |p| {
        if gamemode != Gamemode::Spectator {
            let alpha = held_name_alpha(ov.held_ends - now);
            if alpha > 0.0 {
                let width = p.atlas.font.width(&ov.held).ceil();
                let x = ((vw - width) / 2.0).floor();
                let y = if gamemode == Gamemode::Creative {
                    vh - 45.0
                } else {
                    vh - 59.0
                };
                p.text_faded(&ov.held, x, y, true, alpha);
            }
        }

        let alpha = action_bar_alpha(ov.action_bar_ends - now);
        if alpha > 0.0 {
            let width = p.atlas.font.width(&ov.action_bar).ceil();
            let x = (vw / 2.0).floor() - (width / 2.0).floor();
            p.text_faded(&ov.action_bar, x, vh - 72.0, true, alpha);
        }
    });

    if !ov.title.is_empty() {
        let alpha = title_alpha(ov.title_ends - now, ov.fade_in, ov.stay, ov.fade_out);
        if alpha > 0.0 {
            let (cx, cy) = ((vw / 2.0).floor(), (vh / 2.0).floor());
            let width = p.atlas.font.width(&ov.title).ceil();
            p.scaled(4.0, cx, cy, |p| {
                p.text_faded(&ov.title, -(width / 2.0).floor(), -10.0, true, alpha);
            });
            if !ov.subtitle.is_empty() {
                let width = p.atlas.font.width(&ov.subtitle).ceil();
                p.scaled(2.0, cx, cy, |p| {
                    p.text_faded(&ov.subtitle, -(width / 2.0).floor(), 5.0, true, alpha);
                });
            }
        }
    }
}

const BAR_BACKGROUND: [&str; 7] = [
    "boss_bar/pink_background",
    "boss_bar/blue_background",
    "boss_bar/red_background",
    "boss_bar/green_background",
    "boss_bar/yellow_background",
    "boss_bar/purple_background",
    "boss_bar/white_background",
];
const BAR_PROGRESS: [&str; 7] = [
    "boss_bar/pink_progress",
    "boss_bar/blue_progress",
    "boss_bar/red_progress",
    "boss_bar/green_progress",
    "boss_bar/yellow_progress",
    "boss_bar/purple_progress",
    "boss_bar/white_progress",
];
const OVERLAY_BACKGROUND: [&str; 4] = [
    "boss_bar/notched_6_background",
    "boss_bar/notched_10_background",
    "boss_bar/notched_12_background",
    "boss_bar/notched_20_background",
];
const OVERLAY_PROGRESS: [&str; 4] = [
    "boss_bar/notched_6_progress",
    "boss_bar/notched_10_progress",
    "boss_bar/notched_12_progress",
    "boss_bar/notched_20_progress",
];

const BAR_WIDTH: f32 = 182.0;
const BAR_HEIGHT: f32 = 5.0;

fn lerp_discrete(progress: f32) -> f32 {
    (progress * (BAR_WIDTH - 1.0)).floor() + if progress > 0.0 { 1.0 } else { 0.0 }
}

fn draw_bar(
    p: &mut Painter,
    bar: &crate::client::bossbar::BossBar,
    x: f32,
    y: f32,
    width: f32,
    filled: bool,
) {
    let color = bar.color as usize;
    let sprite = if filled {
        BAR_PROGRESS[color]
    } else {
        BAR_BACKGROUND[color]
    };
    p.sprite_part(sprite, 0.0, 0.0, width, BAR_HEIGHT, x, y);
    if bar.overlay > 0 {
        let notch = bar.overlay as usize - 1;
        let sprite = if filled {
            OVERLAY_PROGRESS[notch]
        } else {
            OVERLAY_BACKGROUND[notch]
        };
        p.sprite_part(sprite, 0.0, 0.0, width, BAR_HEIGHT, x, y);
    }
}

pub fn draw_boss_bars(p: &mut Painter, ctx: &ScreenCtx, bars: &[crate::client::bossbar::BossBar]) {
    if bars.is_empty() {
        return;
    }
    let (vw, vh) = (gui_width(ctx), gui_height(ctx));
    let center = (vw / 2.0).floor();
    let mut y = 12.0;
    for bar in bars {
        let x = center - 91.0;
        draw_bar(p, bar, x, y, BAR_WIDTH, false);
        let width = lerp_discrete(bar.progress());
        if width > 0.0 {
            draw_bar(p, bar, x, y, width, true);
        }
        let name_width = p.atlas.font.width(&bar.name).ceil();
        let name_x = center - (name_width / 2.0).floor();
        p.text(&bar.name, name_x, y - LINE_HEIGHT, true);
        y += 10.0 + LINE_HEIGHT;
        if y >= (vh / 3.0).floor() {
            break;
        }
    }
}

const SIDEBAR_BG: u32 = 0x4C00_0000;
const SIDEBAR_HEADER_BG: u32 = 0x6600_0000;

#[cfg(feature = "mobile_ui")]
pub fn sidebar_top(ctx: &ScreenCtx, sidebar: &crate::client::tablist::Sidebar) -> f32 {
    let count = if sidebar.active {
        sidebar
            .rows
            .len()
            .min(crate::client::tablist::MAX_SIDEBAR_ROWS)
    } else {
        0
    };
    let height = count as f32 * LINE_HEIGHT;
    let bottom = (gui_height(ctx) / 2.0).floor() + (height / 3.0).floor();
    bottom - height - LINE_HEIGHT - 1.0
}

pub fn draw_sidebar(p: &mut Painter, ctx: &ScreenCtx, sidebar: &crate::client::tablist::Sidebar) {
    if !sidebar.active {
        return;
    }
    let atlas = p.atlas;
    let font = &atlas.font;
    let rows = &sidebar.rows;

    let header_width = font.width(&sidebar.title).ceil();
    let spacer = font.width_str(": ").ceil();
    let mut score_widths = [0.0f32; crate::client::tablist::MAX_SIDEBAR_ROWS];
    let mut biggest = header_width;
    for (row, width) in rows.iter().zip(&mut score_widths) {
        *width = font.width(&row.score).ceil();
        let score = if *width > 0.0 { spacer + *width } else { 0.0 };
        biggest = biggest.max(font.width(&row.name).ceil() + score);
    }

    let (vw, vh) = (gui_width(ctx), gui_height(ctx));
    let count = rows.len().min(score_widths.len()) as f32;
    let height = count * LINE_HEIGHT;
    let bottom = (vh / 2.0).floor() + (height / 3.0).floor();
    let left = vw - biggest - 3.0;
    let right = vw - 1.0;
    let header_y = bottom - height;

    let plate_width = right - (left - 2.0);
    p.fill(
        left - 2.0,
        header_y - LINE_HEIGHT - 1.0,
        plate_width,
        LINE_HEIGHT,
        SIDEBAR_HEADER_BG,
    );
    p.fill(
        left - 2.0,
        header_y - 1.0,
        plate_width,
        bottom - (header_y - 1.0),
        SIDEBAR_BG,
    );

    let header_x = left + (biggest / 2.0).floor() - (header_width / 2.0).floor();
    p.text(&sidebar.title, header_x, header_y - LINE_HEIGHT, false);
    for (i, (row, score_width)) in rows.iter().zip(&score_widths).enumerate() {
        let y = bottom - (count - i as f32) * LINE_HEIGHT;
        p.text(&row.name, left, y, false);
        p.text(&row.score, right - *score_width, y, false);
    }
}

fn gui_width(ctx: &ScreenCtx) -> f32 {
    ctx.vw.ceil()
}

fn gui_height(ctx: &ScreenCtx) -> f32 {
    ctx.vh.ceil()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_matches_axis_aligned_yaws() {
        assert_eq!(facing_from_bevy_yaw(0.0), Direction::North);
        assert_eq!(facing_from_bevy_yaw(90.0), Direction::West);
        assert_eq!(facing_from_bevy_yaw(180.0), Direction::South);
        assert_eq!(facing_from_bevy_yaw(270.0), Direction::East);
        assert_eq!(facing_from_bevy_yaw(-90.0), Direction::East);
        assert_eq!(facing_from_bevy_yaw(360.0), Direction::North);
    }

    #[test]
    fn facing_snaps_within_a_quadrant_and_words_match_vanilla() {
        assert_eq!(facing_from_bevy_yaw(44.0), Direction::North);
        assert_eq!(facing_from_bevy_yaw(46.0), Direction::West);
        assert_eq!(facing_towards(Direction::North), "Towards negative Z");
        assert_eq!(facing_towards(Direction::South), "Towards positive Z");
        assert_eq!(facing_towards(Direction::West), "Towards negative X");
        assert_eq!(facing_towards(Direction::East), "Towards positive X");
    }

    use crate::client::bossbar::BossBar;
    use crate::gui::Snapshot;
    use crate::session::{SessionState, SlotStack};

    fn snapshot_of(session: &mut SessionState) -> Snapshot {
        Snapshot {
            action_bar: session.action_bar.take(),
            title_text: session.title_text.take(),
            subtitle_text: session.subtitle_text.take(),
            title_times: session.title_times.take(),
            titles_clear: session.titles_clear.take(),
            hotbar: vec![SlotStack::default(); 10].into(),
            ..Default::default()
        }
    }

    fn text(s: &str) -> Vec<crate::text::Span> {
        crate::text::styled(s, crate::text::Style::default())
    }

    #[test]
    fn held_name_holds_then_ramps_over_ten_ticks() {
        assert_eq!(held_name_alpha(40.0), 1.0);
        assert_eq!(held_name_alpha(11.0), 1.0);
        assert_eq!(held_name_alpha(10.0), 1.0);
        assert_eq!(held_name_alpha(0.0), 0.0);
        assert_eq!(held_name_alpha(-1.0), 0.0);
        assert_eq!(held_name_alpha(5.5), held_name_alpha(6.0));
        assert!(held_name_alpha(5.0) < held_name_alpha(9.0));
    }

    #[test]
    fn action_bar_holds_for_forty_ticks_then_fades_over_twenty() {
        assert_eq!(action_bar_alpha(60.0), 1.0);
        assert_eq!(action_bar_alpha(20.0), 1.0);
        assert_eq!(action_bar_alpha(0.0), 0.0);
        assert!(action_bar_alpha(10.0) < 1.0);
        assert!(action_bar_alpha(10.5) > action_bar_alpha(10.0));
    }

    #[test]
    fn title_fades_in_holds_then_fades_out() {
        let (fi, st, fo) = (TITLE_FADE_IN, TITLE_STAY, TITLE_FADE_OUT);
        assert_eq!(title_alpha(100.0, fi, st, fo), 0.0);
        assert!(title_alpha(95.0, fi, st, fo) > 0.0);
        assert!(title_alpha(95.0, fi, st, fo) < 1.0);
        assert_eq!(title_alpha(90.0, fi, st, fo), 1.0);
        assert_eq!(title_alpha(21.0, fi, st, fo), 1.0);
        assert!(title_alpha(10.0, fi, st, fo) < 1.0);
        assert!(title_alpha(10.0, fi, st, fo) > title_alpha(5.0, fi, st, fo));
        assert_eq!(title_alpha(0.0, fi, st, fo), 0.0);
    }

    #[test]
    fn title_with_no_fade_is_opaque_throughout() {
        assert_eq!(title_alpha(20.0, 0.0, 20.0, 0.0), 1.0);
        assert_eq!(title_alpha(1.0, 0.0, 20.0, 0.0), 1.0);
    }

    #[test]
    fn negative_times_leave_their_field_unchanged() {
        let mut session = SessionState::default();
        session.set_title_times(5, -1, u32::MAX as i32);
        assert_eq!(session.title_times, Some((Some(5), None, None)));

        let mut ov = HudOverlays::default();
        let mut snap = snapshot_of(&mut session);
        ov.update(&mut snap, 0.0);
        assert_eq!(ov.fade_in, 5.0);
        assert_eq!(ov.stay, TITLE_STAY);
        assert_eq!(ov.fade_out, TITLE_FADE_OUT);
    }

    #[test]
    fn a_clear_after_a_title_leaves_nothing_on_screen() {
        let mut session = SessionState::default();
        session.set_title(text("hello"));
        session.set_subtitle(text("world"));
        session.clear_titles(false);

        let mut ov = HudOverlays::default();
        let mut snap = snapshot_of(&mut session);
        ov.update(&mut snap, 0.0);
        assert!(ov.title.is_empty());
        assert!(ov.subtitle.is_empty());
        assert_eq!(ov.title_ends, 0.0);
    }

    #[test]
    fn a_title_after_a_clear_still_shows() {
        let mut session = SessionState::default();
        session.clear_titles(false);
        session.set_title(text("hello"));

        let mut ov = HudOverlays::default();
        let mut snap = snapshot_of(&mut session);
        ov.update(&mut snap, 1.0);
        assert!(!ov.title.is_empty());
        assert_eq!(ov.title_ends, 20.0 + 100.0);
    }

    #[test]
    fn new_times_restart_only_an_in_flight_title() {
        let mut ov = HudOverlays::default();
        let mut session = SessionState::default();
        session.set_title(text("hello"));
        let mut snap = snapshot_of(&mut session);
        ov.update(&mut snap, 0.0);
        assert_eq!(ov.title_ends, 100.0);

        session.set_title_times(0, 40, 0);
        let mut snap = snapshot_of(&mut session);
        ov.update(&mut snap, 1.0);
        assert_eq!(ov.title_ends, 20.0 + 40.0);

        let mut snap = snapshot_of(&mut session);
        ov.update(&mut snap, 100.0);
        session.set_title_times(1, 2, 3);
        let mut snap = snapshot_of(&mut session);
        ov.update(&mut snap, 100.0);
        assert_eq!(ov.title_ends, 60.0);
    }

    fn stack(item: &'static str) -> SlotStack {
        SlotStack {
            item,
            count: 1,
            ..Default::default()
        }
    }

    #[test]
    fn held_name_restarts_on_a_real_change_only() {
        let mut ov = HudOverlays::default();
        ov.update_held(&stack("diamond_sword"), 0.0);
        assert_eq!(ov.held_ends, 40.0);

        ov.update_held(&stack("diamond_sword"), 10.0);
        assert_eq!(ov.held_ends, 40.0);

        ov.update_held(&stack("stone"), 10.0);
        assert_eq!(ov.held_ends, 50.0);

        ov.update_held(&SlotStack::default(), 12.0);
        assert_eq!(ov.held_ends, 0.0);
        ov.update_held(&stack("stone"), 12.0);
        assert_eq!(ov.held_ends, 52.0);
    }

    #[test]
    fn boss_bar_width_matches_lerp_discrete() {
        assert_eq!(lerp_discrete(0.0), 0.0);
        assert_eq!(lerp_discrete(1.0), 182.0);
        assert_eq!(lerp_discrete(0.5), 91.0);
        assert_eq!(lerp_discrete(0.001), 1.0);
    }

    fn bar(from: f32, target: f32, set_at: crate::platform::time::Instant) -> BossBar {
        BossBar {
            name: Vec::new(),
            from,
            target,
            set_at,
            color: 0,
            overlay: 0,
            darken: false,
            fog: false,
        }
    }

    #[test]
    fn boss_bar_progress_lerps_over_a_tenth_of_a_second() {
        use crate::platform::time::Instant;
        use std::time::Duration;
        let now = Instant::now();
        assert!((bar(0.2, 1.0, now).progress() - 0.2).abs() < 0.05);
        let half = now - Duration::from_millis(50);
        let mid = bar(0.0, 1.0, half).progress();
        assert!(mid > 0.4 && mid < 0.6, "{mid}");
        let done = now - Duration::from_millis(500);
        assert_eq!(bar(0.0, 1.0, done).progress(), 1.0);
        assert_eq!(bar(0.0, 5.0, done).progress(), 1.0);
        assert_eq!(bar(0.0, -5.0, done).progress(), 0.0);
    }

    #[test]
    fn renaming_the_held_stack_pops_the_name_again() {
        let mut ov = HudOverlays::default();
        let mut renamed = stack("stone");
        ov.update_held(&renamed, 0.0);
        assert_eq!(ov.held_ends, 40.0);

        renamed.custom_name = Some(text("Bob"));
        ov.update_held(&renamed, 5.0);
        assert_eq!(ov.held_ends, 45.0);
        ov.update_held(&renamed, 6.0);
        assert_eq!(ov.held_ends, 45.0);
    }
}
