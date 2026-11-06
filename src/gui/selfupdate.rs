use crate::client::self_update::{self, Stage};
use crate::client::update_checker::{RELEASES_URL, available};
use crate::gui::assetdownload::{bar, of_total};
use crate::gui::confirmlink::draw_box;
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx};

const TITLE: &str = "Update available";
const FAILED_TITLE: &str = "The update failed";
const DONT_SHOW: &str = "Don't show me again";
const TRY_AGAIN: &str = "Try Again";
const RELEASES_PAGE: &str = "Releases Page";

const ROW_BUTTON_W: f32 = 120.0;
const ROW_MARGIN: f32 = 16.0;

const CANCEL_GAP: f32 = 24.0;
const CANCEL_W: f32 = 100.0;

fn row_button_w(ctx: &ScreenCtx) -> f32 {
    ((ctx.vw - ROW_MARGIN) / 3.0).min(ROW_BUTTON_W).floor()
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    match self_update::stage() {
        Stage::Idle => prompt(p, state, ctx),
        Stage::Asking | Stage::Downloading => downloading(p, ctx),
        Stage::Installing => status(p, ctx, "Installing"),
        Stage::Done => {
            status(p, ctx, "Restarting");
            state.quit = true;
        }
        Stage::Failed(error) => failed(p, state, ctx, &error),
    }
}

fn prompt(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let Some(tag) = available() else {
        state.nav = Some(Screen::Title);
        return;
    };
    let message = format!(
        "Torch Client {tag} is out, and this is {}.\nDownload and install it now?\nThe client restarts when it finishes.",
        env!("CARGO_PKG_VERSION"),
    );
    let warning = format!("Don't show me again skips every release up to {tag}.");
    let buttons = [
        label!("gui.yes").to_owned(),
        label!("gui.no").to_owned(),
        DONT_SHOW.to_owned(),
    ];
    let chosen = draw_box(
        p,
        ctx,
        TITLE,
        &message,
        Some(warning.as_str()),
        &buttons,
        row_button_w(ctx),
    );
    match chosen {
        Some(0) => self_update::start(),
        Some(1) => state.nav = Some(Screen::Title),
        Some(_) => {
            self_update::skip(tag);
            state.nav = Some(Screen::Title);
        }
        None if ctx.input.escape => state.nav = Some(Screen::Title),
        None => {}
    }
}

fn downloading(p: &mut Painter, ctx: &ScreenCtx) {
    let cx = (ctx.vw * 0.5).floor();
    let mid = (ctx.vh * 0.5).floor();
    let (done, total) = self_update::progress();
    let frac = if total > 0 {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.text_centered("Downloading the update", cx, mid - 30.0, 0xFFFFFF, true);
    bar(p, ctx, mid - 14.0, frac);
    p.text_centered(&of_total(done, total), cx, mid + 2.0, 0xA0A0A8, true);

    let x = (cx - CANCEL_W * 0.5).floor();
    let cancel = Button::new(
        x,
        mid + 2.0 + CANCEL_GAP,
        CANCEL_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    );
    if cancel.draw(p, ctx) || ctx.input.escape {
        self_update::cancel();
    }
}

fn status(p: &mut Painter, ctx: &ScreenCtx, text: &str) {
    let mid = (ctx.vh * 0.5).floor();
    p.text_centered(text, (ctx.vw * 0.5).floor(), mid, 0xFFFFFF, true);
}

fn failed(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, error: &str) {
    let buttons = [
        TRY_AGAIN.to_owned(),
        RELEASES_PAGE.to_owned(),
        label!("gui.back").to_owned(),
    ];
    let chosen = draw_box(
        p,
        ctx,
        FAILED_TITLE,
        error,
        None,
        &buttons,
        row_button_w(ctx),
    );
    match chosen {
        Some(0) => self_update::start(),
        Some(1) => crate::platform::url::open(RELEASES_URL),
        Some(_) => leave(state),
        None if ctx.input.escape => leave(state),
        None => {}
    }
}

fn leave(state: &mut GuiState) {
    self_update::dismiss();
    state.nav = Some(Screen::Title);
}
