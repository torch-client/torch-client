use crate::gui::menu;
use crate::gui::options::{
    SCROLLBAR_GAP, SCROLLBAR_W, draw_scrollbar, masked_input, scroll_input_in,
};
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{
    Button, FOOTER_GAP, FOOTER_W, HeaderFooter, WIDGET_HEIGHT, centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::singleplayer::backed::{Control, is_backed};
use crate::text::{LINE_HEIGHT, Span};

pub(in crate::gui) const FEATURE_PACKS: [&str; 3] = [
    "minecart_improvements",
    "redstone_experiments",
    "trade_rebalance",
];

const LAYOUT: HeaderFooter = HeaderFooter::new(HeaderFooter::DEFAULT_H, HeaderFooter::DEFAULT_H);
const MAIN_W: f32 = 310.0;
const INFO_PAD: f32 = 15.0;
const SCROLL_AREA_MIN_H: f32 = 130.0;
const RESERVE: f32 = 4.0 + SCROLLBAR_W;
const AREA_W: f32 = MAIN_W + 2.0 * RESERVE;
const GRID_W: f32 = 299.0;
const SWITCH_W: f32 = 44.0;
const ROW_SPACING: f32 = 4.0;
const INFO_ROWS: usize = 2;
const BLOCK_H: f32 =
    ROW_SPACING + WIDGET_HEIGHT + ROW_SPACING + INFO_ROWS as f32 * LINE_HEIGHT - ROW_SPACING;
const RED: u32 = 0xFF5555;
const GRAY: u32 = crate::text::GRAY;

#[derive(Default)]
pub struct State {
    on: [bool; FEATURE_PACKS.len()],
    opened: Vec<String>,
    scroll: f32,
    drag: Option<f32>,
    pub result: Option<Vec<String>>,
}

impl State {
    pub fn open(&mut self, enabled: &[String]) {
        self.on = FEATURE_PACKS.map(|id| enabled.iter().any(|e| e == id));
        self.opened = enabled.to_vec();
        self.scroll = 0.0;
        self.drag = None;
        self.result = None;
    }

    fn enabled(&self) -> Vec<String> {
        if !is_backed(Control::Experiments) {
            return self.opened.clone();
        }
        FEATURE_PACKS
            .iter()
            .zip(self.on)
            .rev()
            .filter(|(_, on)| *on)
            .map(|(id, _)| (*id).to_owned())
            .collect()
    }
}

pub(in crate::gui) fn feature_title(id: &str) -> String {
    let key = format!("dataPack.{id}.name");
    let title = translate(&key, &[]);
    if title == key { id.to_owned() } else { title }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    p.text_centered(
        label!("selectWorld.experiments"),
        ctx.vw / 2.0,
        LAYOUT.title_y(),
        0xFFFFFF,
        true,
    );

    let screen = &mut state.worlds.experiments;
    let backed = is_backed(Control::Experiments);

    let info = p.atlas.font.wrap(
        &[Span::colored(
            translate("selectWorld.experiments.info", &[]),
            RED,
        )],
        MAIN_W,
    );
    let info_h = info.len() as f32 * LINE_HEIGHT;
    let grid_h = FEATURE_PACKS.len() as f32 * BLOCK_H;
    let first_h = grid_h.min(SCROLL_AREA_MIN_H);
    let top = LAYOUT.content_y(ctx.vh, info_h + INFO_PAD + first_h);
    let area_y = top + info_h + INFO_PAD;
    let extra = ctx.vh - LAYOUT.footer_h - (area_y + first_h);
    let area_h = grid_h.min((first_h + extra).max(0.0));
    let left = centered_row_x(ctx.vw, AREA_W);

    for (i, line) in info.iter().enumerate() {
        p.text(line, left, top + i as f32 * LINE_HEIGHT, true);
    }

    let bar_row_w = AREA_W - SCROLLBAR_W - SCROLLBAR_GAP;
    let rect = (left, area_y, AREA_W, area_h);
    scroll_input_in(
        &mut screen.scroll,
        &mut screen.drag,
        ctx,
        left,
        bar_row_w,
        (left, AREA_W),
        area_y,
        area_h,
        grid_h,
    );
    let input = masked_input(ctx, rect, screen.drag.is_some());
    let area_ctx = ctx.with_input(&input);

    let grid_x = left + RESERVE;
    let switch_x = grid_x + GRID_W - SWITCH_W;
    let label_dy = ((WIDGET_HEIGHT - LINE_HEIGHT) / 2.0).round();
    let mut unsupported = false;
    let clip = p.push_clip(left, area_y, AREA_W, area_h);
    for (i, id) in FEATURE_PACKS.iter().enumerate() {
        let y = (area_y - screen.scroll).floor() + ROW_SPACING + i as f32 * BLOCK_H;
        p.text(
            &[Span::colored(feature_title(id), 0xFFFFFF)],
            grid_x,
            y + label_dy,
            true,
        );

        let value = if screen.on[i] {
            label!("options.on")
        } else {
            label!("options.off")
        };
        let button = Button {
            active: backed,
            ..Button::new(switch_x, y, SWITCH_W, WIDGET_HEIGHT, value)
        };
        if button.draw(p, &area_ctx) {
            screen.on[i] = !screen.on[i];
        }
        if !backed && area_ctx.hovering(switch_x, y, SWITCH_W, WIDGET_HEIGHT) {
            unsupported = true;
        }

        let description = translate(&format!("dataPack.{id}.description"), &[]);
        let lines = p
            .atlas
            .font
            .wrap(&[Span::colored(description, GRAY)], GRID_W - SWITCH_W);
        for (row, line) in lines.iter().take(INFO_ROWS).enumerate() {
            p.text(
                line,
                grid_x,
                y + WIDGET_HEIGHT + row as f32 * LINE_HEIGHT,
                true,
            );
        }
    }
    p.pop_clip(clip);
    draw_scrollbar(p, screen.scroll, left, bar_row_w, area_y, area_h, grid_h);

    let footer_y = LAYOUT.footer_y(ctx.vh);
    let x = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    if Button::new(x, footer_y, FOOTER_W, WIDGET_HEIGHT, label!("gui.done")).draw(p, ctx) {
        screen.result = Some(screen.enabled());
        state.nav = Some(Screen::CreateWorld);
    } else if Button::new(
        x + FOOTER_W + FOOTER_GAP,
        footer_y,
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    )
    .draw(p, ctx)
        || ctx.input.escape
    {
        screen.result = None;
        state.nav = Some(Screen::CreateWorld);
    }

    if unsupported {
        super::unsupported_tip(p, ctx);
    }
}
