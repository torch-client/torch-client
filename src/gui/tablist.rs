use bevy::platform::collections::HashMap;

use crate::client::tablist::{TabList, TabRow};
use crate::gui::ScreenCtx;
use crate::gui::health::HealthState;
use crate::gui::painter::Painter;
use crate::text::Span;

const MAX_ROWS_PER_COL: usize = 20;

const LINE: f32 = 9.0;

const HEAD: f32 = 9.0;

const PANEL: u32 = 0x8000_0000;

const ROW: u32 = 0x20FF_FFFF;

const SPECTATOR_ALPHA: f32 = 0x90 as f32 / 255.0;

pub struct DefaultSkin {
    pub path: &'static str,
    pub face: &'static str,
    pub hat: &'static str,
}

macro_rules! skin {
    ($path:literal) => {
        DefaultSkin {
            path: $path,
            face: concat!("player_face/", $path),
            hat: concat!("player_hat/", $path),
        }
    };
}

pub const DEFAULT_SKINS: [DefaultSkin; 18] = [
    skin!("slim/alex"),
    skin!("slim/ari"),
    skin!("slim/efe"),
    skin!("slim/kai"),
    skin!("slim/makena"),
    skin!("slim/noor"),
    skin!("slim/steve"),
    skin!("slim/sunny"),
    skin!("slim/zuri"),
    skin!("wide/alex"),
    skin!("wide/ari"),
    skin!("wide/efe"),
    skin!("wide/kai"),
    skin!("wide/makena"),
    skin!("wide/noor"),
    skin!("wide/steve"),
    skin!("wide/sunny"),
    skin!("wide/zuri"),
];

#[derive(Default)]
pub struct TabListState {
    health: HashMap<u128, HealthState>,
    visible: bool,
    #[cfg(feature = "mobile_ui")]
    scroll: f32,
}

#[cfg_attr(feature = "mobile_ui", allow(dead_code))]
pub fn draw(
    p: &mut Painter,
    ctx: &ScreenCtx,
    list: &TabList,
    state: &mut TabListState,
    #[cfg(feature = "skins")] faces: &mut crate::gui::player_faces::PlayerFaces,
) {
    let (max_name_width, width_for_score) = begin(p, list, state);
    let tick = p.frame as u64;

    let atlas = p.atlas;
    let font = &atlas.font;
    let width_of = |spans: &[Span]| font.width(spans).ceil();
    let screen_w = ctx.vw;
    let rows = &list.rows;
    let slots = rows.len();

    let mut cols = 1usize;
    let mut rows_per_col = slots;
    while rows_per_col > MAX_ROWS_PER_COL {
        cols += 1;
        rows_per_col = slots.div_ceil(cols);
    }

    let half = |v: f32| (v / 2.0).floor();
    let mid = half(screen_w);

    let cols_f = cols as f32;
    let slot_width =
        ((cols_f * (HEAD + max_name_width + width_for_score + 13.0)).min(screen_w - 50.0) / cols_f)
            .floor();
    let total_width = slot_width * cols_f + (cols_f - 1.0) * 5.0;
    let xxo = mid - half(total_width);
    let mut yyo = 10.0_f32;
    let mut max_line_width = total_width;

    let wrap_width = screen_w - 50.0;
    let header_lines = wrap(font, &list.header, wrap_width);
    let footer_lines = wrap(font, &list.footer, wrap_width);
    for line in header_lines.iter().chain(footer_lines.iter()) {
        max_line_width = max_line_width.max(width_of(line));
    }

    let panel_x0 = mid - half(max_line_width) - 1.0;
    let panel_w = half(max_line_width) * 2.0 + 2.0;

    if !header_lines.is_empty() {
        p.fill(
            panel_x0,
            yyo - 1.0,
            panel_w,
            header_lines.len() as f32 * LINE + 1.0,
            PANEL,
        );
        for line in &header_lines {
            let x = mid - half(width_of(line));
            p.text(line, x, yyo, true);
            yyo += LINE;
        }
        yyo += 1.0;
    }

    p.fill(
        panel_x0,
        yyo - 1.0,
        panel_w,
        rows_per_col as f32 * LINE + 1.0,
        PANEL,
    );

    for (i, row) in rows.iter().enumerate() {
        let col = (i / rows_per_col.max(1)) as f32;
        let line = (i % rows_per_col.max(1)) as f32;
        let xo = xxo + col * slot_width + col * 5.0;
        let yo = yyo + line * LINE;
        draw_row(
            p,
            state,
            list,
            row,
            xo,
            yo,
            Columns {
                slot_width,
                max_name_width,
                width_for_score,
            },
            tick,
            #[cfg(feature = "skins")]
            faces,
        );
    }

    if !footer_lines.is_empty() {
        yyo += rows_per_col as f32 * LINE + 1.0;
        p.fill(
            panel_x0,
            yyo - 1.0,
            panel_w,
            footer_lines.len() as f32 * LINE + 1.0,
            PANEL,
        );
        for line in &footer_lines {
            let x = mid - half(width_of(line));
            p.text(line, x, yyo, true);
            yyo += LINE;
        }
    }
}

fn begin(p: &Painter, list: &TabList, state: &mut TabListState) -> (f32, f32) {
    if !state.visible {
        state.visible = true;
        state.health.clear();
    }
    let rows = &list.rows;

    if !state.health.is_empty() {
        state
            .health
            .retain(|uuid, _| rows.iter().any(|r| r.uuid == *uuid));
    }

    let font = &p.atlas.font;
    let width_of = |spans: &[Span]| font.width(spans).ceil();
    let spacer = font.width_str(" ").ceil();
    let mut max_name_width: f32 = 0.0;
    let mut max_score_width: f32 = 0.0;
    for row in rows {
        max_name_width = max_name_width.max(width_of(&row.name));
        let score = width_of(&row.score_text);
        if score > 0.0 {
            max_score_width = max_score_width.max(spacer + score);
        }
    }

    let width_for_score = match &list.objective {
        Some(o) if o.hearts => 90.0,
        Some(_) => max_score_width,
        None => 0.0,
    };
    (max_name_width, width_for_score)
}

#[derive(Clone, Copy)]
struct Columns {
    slot_width: f32,
    max_name_width: f32,
    width_for_score: f32,
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    p: &mut Painter,
    state: &mut TabListState,
    list: &TabList,
    row: &TabRow,
    xo: f32,
    yo: f32,
    cols: Columns,
    tick: u64,
    #[cfg(feature = "skins")] faces: &mut crate::gui::player_faces::PlayerFaces,
) {
    let Columns {
        slot_width,
        max_name_width,
        width_for_score,
    } = cols;
    p.fill(xo, yo, slot_width, 8.0, ROW);

    #[cfg(feature = "skins")]
    let drawn = match faces
        .slot_of(row.uuid)
        .and_then(|slot| p.atlas.player_face(slot))
    {
        Some((face, hat)) => {
            p.atlas_region_flipped(face, xo, yo, 8.0, 8.0, row.upside_down);
            if row.show_hat {
                p.atlas_region_flipped(hat, xo, yo, 8.0, 8.0, row.upside_down);
            }
            true
        }
        None => false,
    };
    #[cfg(not(feature = "skins"))]
    let drawn = false;
    if !drawn {
        let skin = &DEFAULT_SKINS[(row.skin as usize).min(DEFAULT_SKINS.len() - 1)];
        p.skin_face(
            skin.face,
            row.show_hat.then_some(skin.hat),
            xo,
            yo,
            8.0,
            row.upside_down,
        );
    }
    let name_x = xo + HEAD;

    let alpha = if row.spectator { SPECTATOR_ALPHA } else { 1.0 };
    p.text_faded(&row.name, name_x, yo, true, alpha);

    if let Some(objective) = &list.objective
        && !row.spectator
    {
        let left = name_x + max_name_width + 1.0;
        let right = left + width_for_score;
        if right - left > 5.0 {
            if objective.hearts {
                hearts(p, state, row, yo, left, right, tick);
            } else if !row.score_text.is_empty() {
                let w = p.atlas.font.width(&row.score_text).ceil();
                p.text(&row.score_text, right - w, yo, true);
            }
        }
    }

    ping_icon(p, slot_width, xo, yo, row.latency);
}

#[cfg(feature = "mobile_ui")]
const SIDE_MARGIN: f32 = 4.0;

#[cfg(feature = "mobile_ui")]
const SIDE_MIN_SCALE: f32 = 0.5;

#[cfg(feature = "mobile_ui")]
pub fn draw_side(
    p: &mut Painter,
    ctx: &ScreenCtx,
    list: &TabList,
    state: &mut TabListState,
    right: f32,
    #[cfg(feature = "skins")] faces: &mut crate::gui::player_faces::PlayerFaces,
) {
    let avail_w = right - 2.0 * SIDE_MARGIN;
    let avail_h = ctx.vh - 2.0 * SIDE_MARGIN;
    if avail_w <= 0.0 || avail_h <= 0.0 {
        hide(state);
        return;
    }

    let (max_name_width, width_for_score) = begin(p, list, state);
    let tick = p.frame as u64;
    let atlas = p.atlas;
    let font = &atlas.font;
    let width_of = |spans: &[Span]| font.width(spans).ceil();

    let wrap_width = ctx.vw - 50.0;
    let header_lines = wrap(font, &list.header, wrap_width);
    let footer_lines = wrap(font, &list.footer, wrap_width);

    let slot_width = HEAD + max_name_width + width_for_score + 13.0;
    let mut inner_w = slot_width;
    for line in header_lines.iter().chain(footer_lines.iter()) {
        inner_w = inner_w.max(width_of(line));
    }
    let panel_w = inner_w + 2.0;

    let scale = (avail_w / panel_w).clamp(SIDE_MIN_SCALE, 1.0);
    let w = panel_w.min(avail_w / scale).floor();
    let local_h = avail_h / scale;

    let block = |lines: usize| {
        if lines == 0 {
            0.0
        } else {
            lines as f32 * LINE + 1.0
        }
    };
    let header_h = block(header_lines.len());
    let footer_h = block(footer_lines.len());
    let content_h = list.rows.len() as f32 * LINE;
    let view_h = content_h
        .min(local_h - header_h - footer_h - 1.0)
        .max(0.0)
        .floor();
    let max_scroll = (content_h - view_h).max(0.0);

    let view_x = SIDE_MARGIN;
    let view_y = SIDE_MARGIN + (header_h + 1.0) * scale;
    let view_sw = w * scale;
    let view_sh = view_h * scale;
    let max_scroll_screen = max_scroll * scale;
    if ctx.input.scroll != 0.0 && ctx.hovering(view_x, view_y, view_sw, view_sh) {
        state.scroll -= ctx.input.scroll * LINE * scale;
    }
    crate::gui::options::content_drag_in(
        &mut state.scroll,
        ctx,
        view_x,
        view_y,
        view_sw,
        view_sh,
        false,
        max_scroll_screen,
    );
    state.scroll = state.scroll.clamp(0.0, max_scroll_screen);
    let offset = (state.scroll / scale).round();

    let total_h = header_h + view_h + 1.0 + footer_h;
    let centred = |lw: f32| ((w - lw) / 2.0).floor().max(1.0);

    p.scaled(scale, SIDE_MARGIN, SIDE_MARGIN, |p| {
        let outer = p.push_clip(0.0, 0.0, w, total_h);

        let mut y = 0.0;
        if header_h > 0.0 {
            p.fill(0.0, y, w, header_h, PANEL);
            for (i, line) in header_lines.iter().enumerate() {
                p.text(
                    line,
                    centred(width_of(line)),
                    y + 1.0 + i as f32 * LINE,
                    true,
                );
            }
            y += header_h;
        }

        p.fill(0.0, y, w, view_h + 1.0, PANEL);
        let rows_top = y + 1.0;
        let inner = p.push_clip(0.0, rows_top, w, view_h);
        let xo = centred(slot_width);
        let cols = Columns {
            slot_width,
            max_name_width,
            width_for_score,
        };
        let first = (offset / LINE).floor() as usize;
        let last = ((offset + view_h) / LINE).ceil() as usize;
        for (i, row) in list.rows.iter().enumerate().take(last).skip(first) {
            let yo = rows_top + i as f32 * LINE - offset;
            draw_row(
                p,
                state,
                list,
                row,
                xo,
                yo,
                cols,
                tick,
                #[cfg(feature = "skins")]
                faces,
            );
        }
        p.pop_clip(inner);

        if max_scroll > 0.0 && view_h > 0.0 {
            let thumb_h = (view_h * view_h / content_h).max(4.0).min(view_h);
            let thumb_y = rows_top + (view_h - thumb_h) * offset / max_scroll;
            p.fill(w - 1.0, thumb_y.floor(), 1.0, thumb_h.floor(), SCROLL_THUMB);
        }
        y += view_h + 1.0;

        if footer_h > 0.0 {
            p.fill(0.0, y, w, footer_h, PANEL);
            for (i, line) in footer_lines.iter().enumerate() {
                p.text(
                    line,
                    centred(width_of(line)),
                    y + 1.0 + i as f32 * LINE,
                    true,
                );
            }
        }

        p.pop_clip(outer);
    });
}

#[cfg(feature = "mobile_ui")]
const SCROLL_THUMB: u32 = 0x80FF_FFFF;

pub fn hide(state: &mut TabListState) {
    if state.visible {
        state.visible = false;
        state.health.clear();
        #[cfg(feature = "mobile_ui")]
        {
            state.scroll = 0.0;
        }
    }
}

fn wrap(font: &crate::text::Font, spans: &[Span], width: f32) -> Vec<Vec<Span>> {
    if spans.is_empty() {
        return Vec::new();
    }
    font.wrap(spans, width)
}

fn ping_icon(p: &mut Painter, slot_width: f32, xo: f32, yo: f32, latency: i32) {
    let sprite = match latency {
        l if l < 0 => "icon/ping_unknown",
        l if l < 150 => "icon/ping_5",
        l if l < 300 => "icon/ping_4",
        l if l < 600 => "icon/ping_3",
        l if l < 1000 => "icon/ping_2",
        _ => "icon/ping_1",
    };
    p.sprite(sprite, xo + slot_width - 11.0, yo, 10.0, 8.0);
}

#[allow(clippy::too_many_arguments)]
fn hearts(
    p: &mut Painter,
    state: &mut TabListState,
    row: &TabRow,
    yo: f32,
    left: f32,
    right: f32,
    tick: u64,
) {
    let score = row.score;
    let health = state
        .health
        .entry(row.uuid)
        .or_insert_with(|| HealthState::new(score));
    health.update(score, tick);
    let displayed = health.displayed_value;
    let blink = health.is_blinking(tick);

    let peak = score.max(displayed);
    let full_hearts = -((-peak).div_euclid(2));
    if full_hearts <= 0 {
        return;
    }
    let hearts_to_render = score.max(displayed.max(20)) / 2;
    let width_per_heart = ((right - left - 4.0) / hearts_to_render as f32)
        .min(9.0)
        .floor();

    if width_per_heart <= 3.0 {
        let pct = (score as f32 / 20.0).clamp(0.0, 1.0);
        let rgb = (((1.0 - pct) * 255.0) as u32) << 16 | (((pct * 255.0) as u32) << 8);
        let hearts = score as f32 / 2.0;
        let hp =
            crate::gui::tooltip::translate("multiplayer.player.list.hp", &[format!("{hearts:.1}")]);
        let short = format!("{hearts:.1}");
        let text = if right - p.atlas.font.width_str(&hp).ceil() >= left {
            hp
        } else {
            short
        };
        let x = ((right + left - p.atlas.font.width_str(&text).ceil()) / 2.0).floor();
        p.text_str(&text, x, yo, rgb, true);
        return;
    }

    let container = if blink {
        "hud/heart/container_blinking"
    } else {
        "hud/heart/container"
    };
    for heart in full_hearts..hearts_to_render {
        p.sprite(
            container,
            left + heart as f32 * width_per_heart,
            yo,
            9.0,
            9.0,
        );
    }
    for heart in 0..full_hearts {
        let x = left + heart as f32 * width_per_heart;
        p.sprite(container, x, yo, 9.0, 9.0);
        if blink {
            if heart * 2 + 1 < displayed {
                p.sprite("hud/heart/full_blinking", x, yo, 9.0, 9.0);
            }
            if heart * 2 + 1 == displayed {
                p.sprite("hud/heart/half_blinking", x, yo, 9.0, 9.0);
            }
        }
        if heart * 2 + 1 < score {
            p.sprite(
                if heart >= 10 {
                    "hud/heart/absorbing_full_blinking"
                } else {
                    "hud/heart/full"
                },
                x,
                yo,
                9.0,
                9.0,
            );
        }
        if heart * 2 + 1 == score {
            p.sprite(
                if heart >= 10 {
                    "hud/heart/absorbing_half_blinking"
                } else {
                    "hud/heart/half"
                },
                x,
                yo,
                9.0,
                9.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_skin_table_matches_vanilla() {
        assert_eq!(DEFAULT_SKINS.len(), 18);
        assert_eq!(DEFAULT_SKINS[0].path, "slim/alex");
        assert_eq!(DEFAULT_SKINS[9].path, "wide/alex");
        assert_eq!(DEFAULT_SKINS[6].face, "player_face/slim/steve");
    }

    #[test]
    fn health_state_delays_and_blinks() {
        let mut h = HealthState::new(20);
        h.update(10, 100);
        assert_eq!(h.displayed_value, 20);
        assert!(!h.is_blinking(100));
        assert!(h.is_blinking(103));
        h.update(10, 130);
        assert_eq!(h.displayed_value, 10);
        assert!(!h.is_blinking(130));
    }
}

#[cfg(test)]
mod visual {
    use super::*;
    use crate::client::tablist::TabObjective;
    use crate::gui::atlas::build_gui_atlas;
    use crate::gui::preview::{output_dir, rasterize, upscale};
    use crate::gui::render::GuiInput;
    use crate::text::Style;
    use bevy::prelude::Vec2;

    fn span(text: &str, color: u32) -> Span {
        Span {
            text: text.to_string(),
            style: Style::colored(color),
        }
    }

    fn rows(objective: bool) -> Vec<TabRow> {
        let names = [
            ("Alex", 0xFFFFFF, 12),
            ("Steve", 0xFFFFFF, 180),
            ("aVeryLongPlayerName", 0xFFFFFF, 350),
            ("Dinnerbone", 0xFFFFFF, 700),
            ("Grumm", 0xFFFFFF, 1400),
            ("Watcher", 0xFFFFFF, -1),
        ];
        names
            .iter()
            .enumerate()
            .map(|(i, &(name, color, latency))| {
                let mut spans = Vec::new();
                if i == 1 {
                    spans.push(span("[Admin] ", 0xFF5555));
                }
                spans.push(span(name, color));
                let spectator = name == "Watcher";
                if spectator {
                    for s in &mut spans {
                        s.style.italic = true;
                    }
                }
                TabRow {
                    uuid: i as u128,
                    name: spans,
                    spectator,
                    latency,
                    skin: (i * 3) as u8,
                    show_hat: true,
                    upside_down: name == "Dinnerbone" || name == "Grumm",
                    score: (20 - i as i32 * 3).max(1),
                    score_text: objective
                        .then(|| vec![span(&format!("{}", 1200 - i * 137), 0xFFFF55)])
                        .unwrap_or_default(),
                    team: None,
                }
            })
            .collect()
    }

    fn dump(atlas: &crate::gui::atlas::GuiAtlas, list: &TabList, name: &str, vw: f32, vh: f32) {
        let mut p = Painter::new(atlas, 0);
        let input = GuiInput {
            size: Vec2::new(vw, vh),
            scale: 3.0,
            ..Default::default()
        };
        let ctx = ScreenCtx {
            input: &input,
            vw,
            vh,
        };
        let mut state = TabListState::default();
        draw(
            &mut p,
            &ctx,
            list,
            &mut state,
            #[cfg(feature = "skins")]
            &mut Default::default(),
        );
        let img = rasterize(&p, vw as u32, vh as u32, [70, 110, 170, 255]);
        upscale(&img, 2).save(output_dir().join(name)).unwrap();
    }

    #[test]
    fn preview_tab_list() {
        let atlas = build_gui_atlas(&crate::assets_root());
        let (vw, vh) = (427.0_f32, 240.0_f32);

        dump(
            &atlas,
            &TabList {
                rows: rows(false),
                ..Default::default()
            },
            "tablist_plain.png",
            vw,
            vh,
        );

        dump(
            &atlas,
            &TabList {
                header: vec![span("A Server", 0xFFAA00)],
                footer: vec![span("play.example.com", 0xAAAAAA)],
                rows: rows(true),
                objective: Some(TabObjective { hearts: false }),
            },
            "tablist_scores.png",
            vw,
            vh,
        );

        dump(
            &atlas,
            &TabList {
                rows: rows(false),
                objective: Some(TabObjective { hearts: true }),
                ..Default::default()
            },
            "tablist_hearts.png",
            vw,
            vh,
        );
    }
}
