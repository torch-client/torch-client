use crate::gui::painter::Painter;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::resourcepacks::{self, Compat, Info};
use crate::text::{Span, Style};

use super::options::{
    LAYOUT, SCROLLBAR_GAP, SCROLLBAR_W, draw_scrollbar, draw_title, masked_input, scroll_input,
};

const TITLE: &str = "Select Resource Packs";
pub(in crate::gui) const RESOURCE_PACKS: &str = "Resource Packs...";
const AVAILABLE: &str = "Available";
const SELECTED: &str = "Selected";
const OPEN_FOLDER: &str = "Open Pack Folder";
const DONE: &str = "Done";
const VANILLA_NAME: &str = "Default";
const VANILLA_DESCRIPTION: &str = "The default look and feel of Minecraft";
const INCOMPATIBLE: &str = "Incompatible";
const TOO_OLD: &str = "(Made for an older version of Minecraft)";
const TOO_NEW: &str = "(Made for a newer version of Minecraft)";
const UNKNOWN: &str = "(Broken or incompatible)";

const LIST_W: f32 = 200.0;
const LIST_GAP: f32 = 15.0;
const ROW_H: f32 = 36.0;
const HEADER_H: f32 = 13.0;
const ROW_W: f32 = LIST_W - 4.0;
const PADDING: f32 = 2.0;
const ICON: f32 = 32.0;
const TEXT_W: f32 = 157.0;
const BAR_ROW_W: f32 = LIST_W - SCROLLBAR_W - SCROLLBAR_GAP;
const FOOTER_W: f32 = 150.0;
const FOOTER_GAP: f32 = 8.0;

const INCOMPATIBLE_FILL: u32 = 0xFF77_0000;
const HOVER_FILL: u32 = 0xA090_9090;
const DESCRIPTION: u32 = 0x80_8080;

fn icon_key(name: &str) -> String {
    format!("\u{0}resourcepack/{name}")
}

struct Row {
    name: String,
    info: Info,
}

#[derive(Default)]
pub struct State {
    rows: Vec<Row>,
    selected: Vec<usize>,
    saved: Vec<String>,
    scroll: [f32; 2],
    drag: [Option<f32>; 2],
}

impl State {
    pub fn open(&mut self) {
        self.rows = resourcepacks::list()
            .into_iter()
            .map(|pack| {
                let info = resourcepacks::info(&pack);
                Row {
                    name: pack.name,
                    info,
                }
            })
            .collect();
        for row in &mut self.rows {
            if let Some(pixels) = row.info.icon.take() {
                crate::gui::ping::note_icon(&icon_key(&row.name), std::sync::Arc::new(pixels));
            }
        }
        self.saved = resourcepacks::selected();
        self.selected = self
            .saved
            .iter()
            .filter_map(|name| self.rows.iter().position(|row| &row.name == name))
            .collect();
        self.scroll = [0.0; 2];
        self.drag = [None; 2];
    }

    fn selected_names(&self) -> Vec<String> {
        self.selected
            .iter()
            .map(|&i| self.rows[i].name.clone())
            .collect()
    }
}

enum Action {
    Select(usize),
    Unselect(usize),
    Up(usize),
    Down(usize),
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    draw_title(p, ctx, TITLE);
    let (_, list_y, _, list_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let lefts = [
        (ctx.vw / 2.0 - LIST_GAP - LIST_W).floor(),
        (ctx.vw / 2.0 + LIST_GAP).floor(),
    ];

    let packs = &mut state.resourcepacks;
    let available: Vec<usize> = (0..packs.rows.len())
        .filter(|i| !packs.selected.contains(i))
        .collect();
    let mut action = None;
    for (side, left) in lefts.into_iter().enumerate() {
        let count = if side == 0 {
            available.len()
        } else {
            packs.selected.len() + 1
        };
        let content_h = HEADER_H + count as f32 * ROW_H;
        scroll_input(
            &mut packs.scroll[side],
            &mut packs.drag[side],
            ctx,
            left,
            BAR_ROW_W,
            list_y,
            list_h,
            content_h,
        );
        let scrolls = content_h > list_h;
        let input = masked_input(
            ctx,
            (left, list_y, LIST_W, list_h),
            packs.drag[side].is_some(),
        );
        let list_ctx = ScreenCtx {
            input: &input,
            vw: ctx.vw,
            vh: ctx.vh,
        };
        let clip = p.push_clip(left, list_y, LIST_W, list_h);
        let top = (list_y - packs.scroll[side]).floor();
        header(p, if side == 0 { AVAILABLE } else { SELECTED }, left, top);

        let row_x = left + (LIST_W - ROW_W) / 2.0;
        let text_w = TEXT_W - if scrolls { SCROLLBAR_W } else { 0.0 };
        for slot in 0..count {
            let y = top + HEADER_H + slot as f32 * ROW_H;
            if y + ROW_H < list_y || y > list_y + list_h {
                continue;
            }
            let (cx, cy) = (row_x + PADDING, y + PADDING);
            let hovered = list_ctx.hovering(row_x, y, ROW_W, ROW_H);
            if side == 1 && slot == packs.selected.len() {
                p.sprite("pack_icon", cx, cy, ICON, ICON);
                entry_text(p, VANILLA_NAME, &plain(VANILLA_DESCRIPTION), cx, cy, text_w);
                continue;
            }
            let index = if side == 0 {
                available[slot]
            } else {
                packs.selected[slot]
            };
            let row = &packs.rows[index];
            let compatible = row.info.compat == Compat::Compatible;
            if !compatible {
                p.fill(
                    cx - 1.0,
                    cy - 1.0,
                    ROW_W - 2.0 * PADDING + 2.0,
                    ICON + 2.0,
                    INCOMPATIBLE_FILL,
                );
            }
            let icon = crate::gui::ping::icon_slot(&icon_key(&row.name))
                .and_then(|slot| p.atlas.server_icon(slot));
            match icon {
                Some(region) => p.region(region, cx, cy, ICON, ICON),
                None => p.sprite("unknown_pack", cx, cy, ICON, ICON),
            }

            if hovered && !compatible {
                let reason = match row.info.compat {
                    Compat::TooOld => TOO_OLD,
                    Compat::TooNew => TOO_NEW,
                    _ => UNKNOWN,
                };
                entry_text(p, INCOMPATIBLE, &plain(reason), cx, cy, text_w);
            } else {
                entry_text(p, &row.name, &row.info.description, cx, cy, text_w);
            }
            if !hovered {
                continue;
            }
            p.fill(cx, cy, ICON, ICON, HOVER_FILL);
            let Some(m) = list_ctx.mouse() else { continue };
            let (rx, ry) = (m.x - cx, m.y - cy);
            let on_icon = rx >= 0.0 && rx < ICON && ry >= 0.0 && ry < ICON;
            let click = list_ctx.input.left_click && on_icon;
            if side == 0 {
                control(p, "select", on_icon, cx, cy);
                if click {
                    action = Some(Action::Select(index));
                }
                continue;
            }
            let left_half = on_icon && rx < ICON / 2.0;
            control(p, "unselect", left_half, cx, cy);
            if click && left_half {
                action = Some(Action::Unselect(slot));
            }
            if slot > 0 {
                let over = on_icon && !left_half && ry < ICON / 2.0;
                control(p, "move_up", over, cx, cy);
                if click && over {
                    action = Some(Action::Up(slot));
                }
            }
            if slot + 1 < packs.selected.len() {
                let over = on_icon && !left_half && ry >= ICON / 2.0;
                control(p, "move_down", over, cx, cy);
                if click && over {
                    action = Some(Action::Down(slot));
                }
            }
        }
        p.pop_clip(clip);
        draw_scrollbar(
            p,
            packs.scroll[side],
            left,
            BAR_ROW_W,
            list_y,
            list_h,
            content_h,
        );
    }

    match action {
        Some(Action::Select(index)) => packs.selected.insert(0, index),
        Some(Action::Unselect(slot)) => {
            packs.selected.remove(slot);
        }
        Some(Action::Up(slot)) => packs.selected.swap(slot, slot - 1),
        Some(Action::Down(slot)) => packs.selected.swap(slot, slot + 1),
        None => {}
    }

    let footer_y = LAYOUT.footer_y(ctx.vh);
    let x = (ctx.vw / 2.0 - FOOTER_W - FOOTER_GAP / 2.0).floor();
    if Button::new(x, footer_y, FOOTER_W, WIDGET_HEIGHT, OPEN_FOLDER).draw(p, ctx) {
        crate::platform::url::open_folder(&resourcepacks::dir());
    }
    let done = Button::new(
        x + FOOTER_W + FOOTER_GAP,
        footer_y,
        FOOTER_W,
        WIDGET_HEIGHT,
        DONE,
    );
    if done.draw(p, ctx) {
        let chosen = state.resourcepacks.selected_names();
        if chosen != state.resourcepacks.saved {
            resourcepacks::save_selected(&chosen);
            resourcepacks::request_restart();
            state.quit = true;
        }
        state.nav = Some(Screen::Options);
    }
}

fn header(p: &mut Painter, title: &str, left: f32, top: f32) {
    let spans = [Span {
        text: title.to_owned(),
        style: Style {
            bold: true,
            underline: true,
            ..Style::colored(0xFF_FFFF)
        },
    }];
    let w = p.atlas.font.width(&spans);
    p.text(
        &spans,
        (left + (LIST_W - w) / 2.0).floor(),
        top + (HEADER_H - crate::text::LINE_HEIGHT) / 2.0 + 1.0,
        true,
    );
}

fn entry_text(p: &mut Painter, name: &str, description: &[Span], cx: f32, cy: f32, w: f32) {
    let x = cx + ICON + PADDING;
    let name = p.atlas.font.trim_to_width(&plain_white(name), w);
    p.text(&name, x, cy + 1.0, true);
    let lines = p.atlas.font.wrap(&grey(description), w);
    for (i, line) in lines.iter().take(2).enumerate() {
        p.text(
            line,
            x,
            cy + 12.0 + i as f32 * crate::text::LINE_HEIGHT,
            true,
        );
    }
}

fn control(p: &mut Painter, name: &str, highlighted: bool, x: f32, y: f32) {
    let sprite = if highlighted {
        format!("transferable_list/{name}_highlighted")
    } else {
        format!("transferable_list/{name}")
    };
    p.sprite(&sprite, x, y, ICON, ICON);
}

fn plain(text: &str) -> Vec<Span> {
    vec![Span {
        text: text.to_owned(),
        style: Style::colored(DESCRIPTION),
    }]
}

fn plain_white(text: &str) -> Vec<Span> {
    vec![Span {
        text: text.to_owned(),
        style: Style::colored(0xFF_FFFF),
    }]
}

fn grey(spans: &[Span]) -> Vec<Span> {
    spans
        .iter()
        .map(|span| {
            let mut span = span.clone();
            if span.style.color == Style::default().color {
                span.style.color = DESCRIPTION;
            }
            span
        })
        .collect()
}
