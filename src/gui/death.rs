use std::sync::Arc;

use crate::gui::painter::Painter;
use crate::gui::widgets::{self, Button, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx};
use crate::session::SharedMutex;
use crate::text::{LINE_HEIGHT, Span, Style};

const TITLE: &str = "You Died!";
const TITLE_HARDCORE: &str = "Game Over!";
const RESPAWN: &str = "Respawn";
const SPECTATE: &str = "Spectate World";
const TITLE_SCREEN: &str = "Title Screen";
const QUIT_CONFIRM: &str = "Are you sure you want to quit?";

const TITLE_SCALE: f32 = 2.0;
const CONFIRM_BUTTON_W: f32 = 150.0;
const CONFIRM_GAP: f32 = 4.0;
const CONFIRM_PAD: f32 = 16.0;
const BUTTON_DELAY: u32 = 20;

const BG_TOP: u32 = 0x6050_0000;
const BG_BOTTOM: u32 = 0xA080_3030;

const SCORE_COLOR: u32 = 0xFFFF55;

#[derive(Default)]
pub struct DeathState {
    cause: Option<Vec<Span>>,
    score: Vec<Span>,
    hardcore: bool,
    opened: Option<u32>,
    respawn_sent: bool,
    confirm: Option<u32>,
}

impl DeathState {
    pub fn open(&mut self, cause: Option<Vec<Span>>, score: i32, hardcore: bool) {
        self.cause = cause;
        self.score = score_line(score);
        self.hardcore = hardcore;
        self.opened = None;
        self.respawn_sent = false;
        self.confirm = None;
    }
}

fn score_line(score: i32) -> Vec<Span> {
    let value = score.to_string();
    let text = crate::gui::tooltip::translate("deathScreen.score.value", &[value.clone()]);
    let Some(at) = text.find(&value) else {
        return crate::text::styled(&text, Style::default());
    };
    let mut spans = crate::text::styled(&text[..at], Style::default());
    spans.extend(crate::text::styled(&value, Style::colored(SCORE_COLOR)));
    spans.extend(crate::text::styled(
        &text[at + value.len()..],
        Style::default(),
    ));
    spans
}

fn background(p: &mut Painter, vw: f32, vh: f32) {
    p.gradient_v(0.0, 0.0, vw, vh, BG_TOP, BG_BOTTOM);
}

pub fn draw(
    p: &mut Painter,
    state: &mut DeathState,
    ctx: &ScreenCtx,
    shared: &Arc<SharedMutex>,
) -> Option<Screen> {
    background(p, ctx.vw, ctx.vh);

    if let Some(opened) = state.confirm {
        return draw_confirm(p, state, ctx, shared, opened);
    }

    let opened = *state.opened.get_or_insert(p.frame);

    let title = if state.hardcore {
        TITLE_HARDCORE
    } else {
        TITLE
    };
    p.scaled(TITLE_SCALE, 0.0, 0.0, |p| {
        widgets::draw_title(p, ctx.vw / TITLE_SCALE, 30.0, title);
    });
    if let Some(cause) = &state.cause {
        let w = p.atlas.font.width(cause);
        p.text(cause, ((ctx.vw - w) / 2.0).floor(), 85.0, true);
    }
    let w = p.atlas.font.width(&state.score);
    p.text(&state.score, ((ctx.vw - w) / 2.0).floor(), 100.0, true);

    let x = widgets::centered_x(ctx.vw);
    let top = (ctx.vh / 4.0).floor();
    let ready = p.frame.wrapping_sub(opened) >= BUTTON_DELAY;

    let respawn = Button {
        active: ready && !state.respawn_sent,
        ..Button::new(
            x,
            top + 72.0,
            WIDGET_WIDTH_BIG,
            WIDGET_HEIGHT,
            if state.hardcore { SPECTATE } else { RESPAWN },
        )
    };
    if respawn.draw(p, ctx) {
        shared.lock().unwrap().respawn_requested = true;
        state.respawn_sent = true;
    }

    let quit = Button {
        active: ready,
        ..Button::new(x, top + 96.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, TITLE_SCREEN)
    };
    if quit.draw(p, ctx) {
        if state.hardcore {
            return Some(Screen::Title);
        }
        state.confirm = Some(p.frame);
    }
    None
}

fn draw_confirm(
    p: &mut Painter,
    state: &mut DeathState,
    ctx: &ScreenCtx,
    shared: &Arc<SharedMutex>,
    opened: u32,
) -> Option<Screen> {
    let block_h = LINE_HEIGHT + CONFIRM_PAD + WIDGET_HEIGHT;
    let y = ((ctx.vh - block_h) / 2.0).floor().max(0.0);

    widgets::draw_title(p, ctx.vw, y, QUIT_CONFIRM);

    let row_y = y + LINE_HEIGHT + CONFIRM_PAD;
    let row_w = CONFIRM_BUTTON_W * 2.0 + CONFIRM_GAP;
    let x = ((ctx.vw - row_w) / 2.0).floor();
    let ready = p.frame.wrapping_sub(opened) >= BUTTON_DELAY;

    let yes = Button {
        active: ready,
        ..Button::new(x, row_y, CONFIRM_BUTTON_W, WIDGET_HEIGHT, TITLE_SCREEN)
    };
    if yes.draw(p, ctx) {
        return Some(Screen::Title);
    }
    let no = Button {
        active: ready,
        ..Button::new(
            x + CONFIRM_BUTTON_W + CONFIRM_GAP,
            row_y,
            CONFIRM_BUTTON_W,
            WIDGET_HEIGHT,
            RESPAWN,
        )
    };
    if no.draw(p, ctx) {
        shared.lock().unwrap().respawn_requested = true;
        state.respawn_sent = true;
        state.confirm = None;
    }
    None
}
