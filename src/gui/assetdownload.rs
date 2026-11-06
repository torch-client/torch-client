use std::sync::atomic::{AtomicBool, Ordering};

use crate::client::assets::{Download, Stage, download, offers_assets, offers_sounds};
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::util::bytes::Bytes;

const PROMPT: &str = "The Mojang assets must be downloaded";
const TOP_UP_PROMPT: &str = "Some Mojang assets are missing";

static TOP_UP: AtomicBool = AtomicBool::new(false);

static INSTALL_AUDIO: AtomicBool = AtomicBool::new(true);

pub(crate) fn check_missing(state: &mut GuiState) -> bool {
    let list_missing = crate::client::language::request_list();
    if !crate::client::assets::check_missing() {
        return list_missing;
    }
    TOP_UP.store(true, Ordering::Relaxed);
    INSTALL_AUDIO.store(true, Ordering::Relaxed);
    state.nav = Some(Screen::AssetDownload);
    true
}

fn begin(dl: &'static Download) {
    dl.start(
        crate::ASSET_VERSION.to_string(),
        INSTALL_AUDIO.load(Ordering::Relaxed),
    );
}

fn leave_label() -> &'static str {
    if TOP_UP.load(Ordering::Relaxed) {
        label!("gui.back")
    } else {
        "Exit"
    }
}

fn continue_label() -> &'static str {
    if TOP_UP.load(Ordering::Relaxed) {
        label!("gui.continue")
    } else {
        "Continue"
    }
}

fn cancel_label() -> &'static str {
    if TOP_UP.load(Ordering::Relaxed) {
        label!("gui.cancel")
    } else {
        "Cancel"
    }
}

fn leave(state: &mut GuiState) {
    if TOP_UP.swap(false, Ordering::Relaxed) {
        state.nav = Some(Screen::ManageStorage);
    } else {
        state.quit = true;
    }
}

const BUTTON_W: f32 = 100.0;
const BUTTON_H: f32 = 20.0;
const BUTTON_GAP: f32 = 8.0;

const BAR_W: f32 = 220.0;
const BAR_H: f32 = 10.0;

const LINE: f32 = 12.0;

const BOX_W: f32 = BUTTON_W * 2.0 + BUTTON_GAP;
const BOX_H: f32 = 26.0;
const BOX_GAP: f32 = 4.0;
const BOXES_TOP: f32 = -24.0;
const BOX_PAD: f32 = 6.0;
const BOX_TEXT_TOP: f32 = 3.0;
const PERCENT_INSET: f32 = 18.0;

const ASSETS_SIZE: &str = "43.7 MiB";
const AUDIO_SIZE: &str = "330 MiB";
const CONTACTING: &str = "Contacting Mojang";
const ASSETS_NAME: &str = "Built in";
const AUDIO_NAME: &str = "Install audio";

const CHECK_SIZE: f32 = 14.0;
const CHECK_INSET: f32 = 3.0;

const FACE: u32 = 0xFF22_2229;
const EDGE: u32 = 0xFF6E_6E7A;
const FACE_HOVERED: u32 = 0xFF39_3944;
const EDGE_HOVERED: u32 = 0xFFFF_FFFF;
const GREEN: u32 = 0xFF4C_C46A;
const DETAIL: u32 = 0xA0A0A8;

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    p.gradient_v(0.0, 0.0, ctx.vw, ctx.vh, 0xFF1B1B21, 0xFF0C0C10);

    let cx = (ctx.vw * 0.5).floor();
    let mid = (ctx.vh * 0.5).floor();
    let dl = download();
    let stage = dl.stage();

    let top_up = TOP_UP.load(Ordering::Relaxed);
    let prompt = if top_up { TOP_UP_PROMPT } else { PROMPT };
    p.text_centered(prompt, cx, mid - 46.0, 0xFFFFFF, false);

    match stage {
        Stage::Idle | Stage::Cancelled => {
            if stage == Stage::Cancelled {
                p.text_centered("Download cancelled", cx, mid - 46.0 + LINE, 0xFFAA55, false);
            }
            let row = prompt_boxes(p, ctx, mid + BOXES_TOP);
            let left = (cx - BUTTON_W - BUTTON_GAP * 0.5).floor();
            let right = (cx + BUTTON_GAP * 0.5).floor();
            if button(p, ctx, left, row, continue_label()) {
                begin(dl);
            }
            if button(p, ctx, right, row, leave_label()) {
                leave(state);
            }
        }

        Stage::Asking
        | Stage::Downloading
        | Stage::Extracting
        | Stage::Fetching
        | Stage::Sounds => {
            let row = progress_boxes(p, ctx, mid + BOXES_TOP, dl, stage);
            if dl.sounds_running() {
                let left = (cx - BUTTON_W - BUTTON_GAP * 0.5).floor();
                let right = (cx + BUTTON_GAP * 0.5).floor();
                if button(p, ctx, left, row, "Skip audio") {
                    dl.skip_sounds();
                }
                if button(p, ctx, right, row, cancel_label()) {
                    dl.cancel();
                }
            } else if button(p, ctx, (cx - BUTTON_W * 0.5).floor(), row, cancel_label()) {
                dl.cancel();
            }
        }

        Stage::Done if top_up => {
            crate::client::assets::adopt_sounds();
            if !crate::client::assets::needs_restart() {
                leave(state);
                return;
            }
            p.text_centered("The assets are ready", cx, mid - 14.0, 0x7FDF7F, false);
            p.text_centered(
                "Restart the client to use them",
                cx,
                mid - 14.0 + LINE,
                0xA0A0A8,
                false,
            );
            if button(
                p,
                ctx,
                (cx - BUTTON_W * 0.5).floor(),
                mid + 20.0,
                leave_label(),
            ) {
                leave(state);
            }
        }
        Stage::Done => {
            prepare();
            p.text_centered("The assets are ready", cx, mid - 14.0, 0x7FDF7F, false);
            p.text_centered(
                "Preparing the atlases",
                cx,
                mid - 14.0 + LINE,
                0xA0A0A8,
                false,
            );
        }

        Stage::Failed => {
            let error = dl.error();
            let hint = if error == crate::client::assets::NETWORK_FAILURE {
                "Check the connection and try again"
            } else {
                "The log says more"
            };
            p.text_centered(error, cx, mid - 14.0, 0xFF5555, false);
            p.text_centered(hint, cx, mid - 14.0 + LINE, 0xA0A0A8, false);
            let row = mid + 20.0;
            let left = (cx - BUTTON_W - BUTTON_GAP * 0.5).floor();
            let right = (cx + BUTTON_GAP * 0.5).floor();
            if button(p, ctx, left, row, "Try again") {
                begin(dl);
            }
            if button(p, ctx, right, row, leave_label()) {
                leave(state);
            }
        }
    }
}

pub(super) fn of_total(done: u64, total: u64) -> String {
    format!("{} of {}", Bytes(done), Bytes(total))
}

fn box_span(ctx: &ScreenCtx) -> (f32, f32) {
    let w = fitted(BOX_W, ctx);
    (((ctx.vw - w) * 0.5).floor(), w)
}

fn prompt_boxes(p: &mut Painter, ctx: &ScreenCtx, mut y: f32) -> f32 {
    let (x, w) = box_span(ctx);

    if offers_assets() {
        chrome(p, x, y, w, BOX_H, false);
        box_text(p, x + BOX_PAD, y, ASSETS_NAME, ASSETS_SIZE, DETAIL);
        y += BOX_H + BOX_GAP;
    }
    if offers_sounds() {
        let hovered = ctx.hovering(x, y, w, BOX_H);
        if hovered && ctx.input.left_click {
            INSTALL_AUDIO.fetch_xor(true, Ordering::Relaxed);
        }
        chrome(p, x, y, w, BOX_H, hovered);
        let check_x = x + w - BOX_PAD - CHECK_SIZE;
        let check_y = y + ((BOX_H - CHECK_SIZE) * 0.5).floor();
        p.fill(check_x, check_y, CHECK_SIZE, CHECK_SIZE, 0xFF0A0A0C);
        if INSTALL_AUDIO.load(Ordering::Relaxed) {
            let inner = CHECK_SIZE - CHECK_INSET * 2.0;
            p.fill(
                check_x + CHECK_INSET,
                check_y + CHECK_INSET,
                inner,
                inner,
                GREEN,
            );
        }
        p.outline(check_x, check_y, CHECK_SIZE, CHECK_SIZE, EDGE);
        box_text(p, x + BOX_PAD, y, AUDIO_NAME, AUDIO_SIZE, DETAIL);
        y += BOX_H + BOX_GAP;
    }
    y + BOX_GAP
}

fn progress_boxes(
    p: &mut Painter,
    ctx: &ScreenCtx,
    mut y: f32,
    dl: &Download,
    stage: Stage,
) -> f32 {
    let (done, total) = dl.progress();
    if offers_assets() || total > 0 {
        let line = match stage {
            Stage::Extracting => "Unpacking the assets".to_string(),
            _ if total > 0 => of_total(done, total),
            _ => CONTACTING.to_string(),
        };
        running_box(p, ctx, y, ASSETS_NAME, &line, done, total);
        y += BOX_H + BOX_GAP;
    }
    if offers_sounds() {
        let (done, total) = dl.sound_progress();
        let line = if dl.sounds_skipped() {
            "Skipped".to_string()
        } else if total > 0 {
            of_total(done, total)
        } else if stage == Stage::Asking || dl.sounds_running() {
            CONTACTING.to_string()
        } else {
            "Already installed".to_string()
        };
        let total = if dl.sounds_skipped() { 0 } else { total };
        running_box(p, ctx, y, AUDIO_NAME, &line, done, total);
        y += BOX_H + BOX_GAP;
    }
    y + BOX_GAP
}

fn running_box(
    p: &mut Painter,
    ctx: &ScreenCtx,
    y: f32,
    name: &str,
    line: &str,
    done: u64,
    total: u64,
) {
    let (x, w) = box_span(ctx);
    chrome(p, x, y, w, BOX_H, false);
    if total > 0 {
        let frac = (done as f32 / total as f32).clamp(0.0, 1.0);
        let filled = ((w - 2.0) * frac).floor();
        if filled >= 1.0 {
            p.fill(x + 1.0, y + 1.0, filled, BOX_H - 2.0, GREEN);
        }
        let percent = format!("{}%", (frac * 100.0) as u32);
        let percent_y = y + ((BOX_H - 8.0) * 0.5).floor();
        p.text_centered(&percent, x + w - PERCENT_INSET, percent_y, 0xFFFFFF, true);
    }
    box_text(p, x + BOX_PAD, y, name, line, 0xFFFFFF);
}

fn box_text(p: &mut Painter, x: f32, y: f32, name: &str, line: &str, line_rgb: u32) {
    p.text_plain(name, x, y + BOX_TEXT_TOP, 0xFFFFFF, true);
    p.text_plain(line, x, y + BOX_TEXT_TOP + LINE, line_rgb, true);
}

fn chrome(p: &mut Painter, x: f32, y: f32, w: f32, h: f32, hovered: bool) {
    let (face, edge) = if hovered {
        (FACE_HOVERED, EDGE_HOVERED)
    } else {
        (FACE, EDGE)
    };
    p.fill(x, y, w, h, face);
    p.outline(x, y, w, h, edge);
}

fn fitted(ideal: f32, ctx: &ScreenCtx) -> f32 {
    ideal.min(ctx.vw - 40.0).max(60.0).floor()
}

fn button(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32, label: &str) -> bool {
    let hovered = ctx.hovering(x, y, BUTTON_W, BUTTON_H);
    chrome(p, x, y, BUTTON_W, BUTTON_H, hovered);
    p.text_centered(
        label,
        x + BUTTON_W * 0.5,
        (y + (BUTTON_H - 8.0) * 0.5).floor(),
        0xFFFFFF,
        false,
    );
    hovered && ctx.input.left_click
}

pub(super) fn bar(p: &mut Painter, ctx: &ScreenCtx, y: f32, frac: f32) {
    let w = fitted(BAR_W, ctx);
    let x = ((ctx.vw - w) * 0.5).floor();
    p.fill(x, y, w, BAR_H, 0xFF0A0A0C);
    let filled = ((w - 2.0) * frac).floor();
    if filled >= 1.0 {
        p.fill(x + 1.0, y + 1.0, filled, BAR_H - 2.0, GREEN);
    }
    p.outline(x, y, w, BAR_H, EDGE);
}

static PREPARING: AtomicBool = AtomicBool::new(false);

fn prepare() {
    if PREPARING.swap(true, Ordering::Relaxed) {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    build();
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::Builder::new()
        .name("asset-prepare".into())
        .spawn(build)
        .ok();
}

fn build() {
    if !crate::client::assets::mount() {
        PREPARING.store(false, Ordering::Relaxed);
        download().report_failure(
            "The assets could not be read",
            "the set that just installed will not mount",
        );
        return;
    }
    crate::renderer::reload::install_downloaded();
    #[cfg(target_arch = "wasm32")]
    if let Some(archive) = crate::client::assets::take_installed() {
        crate::client::mesh_worker::spawn(&archive);
    }
}
