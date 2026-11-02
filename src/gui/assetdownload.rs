use std::sync::atomic::{AtomicBool, Ordering};

use crate::client::assets::{Stage, download};
use crate::gui::painter::Painter;
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::util::bytes::Bytes;

const PROMPT: &str = "The Mojang assets must be downloaded";
const TOP_UP_PROMPT: &str = "Some Mojang assets are missing";

static TOP_UP: AtomicBool = AtomicBool::new(false);

pub(crate) fn check_missing(state: &mut GuiState) -> bool {
    if !crate::client::assets::check_missing() {
        return false;
    }
    TOP_UP.store(true, Ordering::Relaxed);
    state.nav = Some(Screen::AssetDownload);
    true
}

fn leave_label() -> &'static str {
    if TOP_UP.load(Ordering::Relaxed) {
        "Back"
    } else {
        "Exit"
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
            let row = mid + 6.0;
            let left = (cx - BUTTON_W - BUTTON_GAP * 0.5).floor();
            let right = (cx + BUTTON_GAP * 0.5).floor();
            if button(p, ctx, left, row, "Continue") {
                dl.start(crate::ASSET_VERSION.to_string());
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
            let (done, total) = if stage == Stage::Sounds {
                dl.sound_progress()
            } else {
                dl.progress()
            };
            let frac = if total > 0 {
                (done as f32 / total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            bar(p, ctx, mid - 14.0, frac);

            let percent = format!("{}%", (frac * 100.0) as u32);
            p.text_centered(&percent, cx, mid + 2.0, 0xFFFFFF, false);

            let detail = match stage {
                Stage::Asking => "Contacting Mojang".to_string(),
                Stage::Downloading | Stage::Fetching => of_total(done, total),
                Stage::Sounds => format!("Sounds {}", of_total(done, total)),
                _ => "Unpacking the assets".to_string(),
            };
            p.text_centered(&detail, cx, mid + 2.0 + LINE, 0xA0A0A8, false);

            let sounds = dl.sounds_running();
            if sounds && stage != Stage::Sounds {
                let (done, total) = dl.sound_progress();
                let line = format!("Sounds {}", of_total(done, total));
                p.text_centered(&line, cx, mid + 2.0 + LINE * 2.0, 0x70707A, false);
            }

            let row = mid + 40.0;
            if sounds {
                let left = (cx - BUTTON_W - BUTTON_GAP * 0.5).floor();
                let right = (cx + BUTTON_GAP * 0.5).floor();
                if button(p, ctx, left, row, "Skip audio") {
                    dl.skip_sounds();
                }
                if button(p, ctx, right, row, "Cancel") {
                    dl.cancel();
                }
            } else if button(p, ctx, (cx - BUTTON_W * 0.5).floor(), row, "Cancel") {
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
                dl.start(crate::ASSET_VERSION.to_string());
            }
            if button(p, ctx, right, row, leave_label()) {
                leave(state);
            }
        }
    }
}

fn of_total(done: u64, total: u64) -> String {
    format!("{} of {}", Bytes(done), Bytes(total))
}

fn button(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32, label: &str) -> bool {
    let hovered = ctx.hovering(x, y, BUTTON_W, BUTTON_H);
    let (face, edge) = if hovered {
        (0xFF39_3944, 0xFFFF_FFFF)
    } else {
        (0xFF22_2229, 0xFF6E_6E7A)
    };
    p.fill(x, y, BUTTON_W, BUTTON_H, face);
    p.outline(x, y, BUTTON_W, BUTTON_H, edge);
    p.text_centered(
        label,
        x + BUTTON_W * 0.5,
        (y + (BUTTON_H - 8.0) * 0.5).floor(),
        0xFFFFFF,
        false,
    );
    hovered && ctx.input.left_click
}

fn bar(p: &mut Painter, ctx: &ScreenCtx, y: f32, frac: f32) {
    let w = BAR_W.min(ctx.vw - 40.0).max(60.0).floor();
    let x = ((ctx.vw - w) * 0.5).floor();
    p.fill(x, y, w, BAR_H, 0xFF0A0A0C);
    let filled = ((w - 2.0) * frac).floor();
    if filled >= 1.0 {
        p.fill(x + 1.0, y + 1.0, filled, BAR_H - 2.0, 0xFF4C_C46A);
    }
    p.outline(x, y, w, BAR_H, 0xFF6E_6E7A);
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
