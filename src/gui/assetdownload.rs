use std::sync::atomic::{AtomicBool, Ordering};

use crate::client::assets::{Download, Stage, download, offers_assets, offers_sounds};
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::lang::{Key, tr, trf};
use crate::util::bytes::Bytes;

const PROMPT: Key = Key::of("install.prompt");
const TOP_UP_PROMPT: Key = Key::of("install.top_up_prompt");

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
    #[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
    let sounds =
        INSTALL_AUDIO.load(Ordering::Relaxed) && !crate::client::assets::transfer::kept_sounds();
    #[cfg(not(all(target_arch = "wasm32", feature = "asset_download")))]
    let sounds = INSTALL_AUDIO.load(Ordering::Relaxed);
    dl.start(crate::ASSET_VERSION.to_string(), sounds);
}

fn leave_label() -> &'static str {
    if TOP_UP.load(Ordering::Relaxed) {
        label!("gui.back")
    } else {
        tr!("install.exit")
    }
}

fn continue_label() -> &'static str {
    if TOP_UP.load(Ordering::Relaxed) {
        label!("gui.continue")
    } else {
        tr!("install.continue")
    }
}

fn cancel_label() -> &'static str {
    if TOP_UP.load(Ordering::Relaxed) {
        label!("gui.cancel")
    } else {
        tr!("install.cancel")
    }
}

const CAN_EXIT: bool = !cfg!(any(target_arch = "wasm32", target_os = "android"));

fn action_and_leave(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    cx: f32,
    row: f32,
    action: &str,
) -> bool {
    if !CAN_EXIT && !TOP_UP.load(Ordering::Relaxed) {
        return button_centred(p, ctx, cx, row, action);
    }
    button_pair(p, ctx, cx, row, action, leave_label(), || leave(state))
}

fn button_pair(
    p: &mut Painter,
    ctx: &ScreenCtx,
    cx: f32,
    row: f32,
    left: &str,
    right: &str,
    on_right: impl FnOnce(),
) -> bool {
    let w = button_w(p, &[left, right]);
    let left_x = (cx - w - BUTTON_GAP * 0.5).floor();
    let right_x = (cx + BUTTON_GAP * 0.5).floor();
    let pressed = button(p, ctx, left_x, row, w, left);
    if button(p, ctx, right_x, row, w, right) {
        on_right();
    }
    pressed
}

fn button_centred(p: &mut Painter, ctx: &ScreenCtx, cx: f32, row: f32, label: &str) -> bool {
    let w = button_w(p, &[label]);
    button(p, ctx, (cx - w * 0.5).floor(), row, w, label)
}

fn button_w(p: &Painter, labels: &[&str]) -> f32 {
    labels
        .iter()
        .map(|l| p.atlas.font.width_str(l) + BUTTON_PAD * 2.0)
        .fold(BUTTON_W, f32::max)
        .ceil()
}

fn leave(state: &mut GuiState) {
    if TOP_UP.swap(false, Ordering::Relaxed) {
        state.nav = Some(Screen::ManageStorage);
    } else {
        state.quit = true;
    }
}

const BUTTON_W: f32 = 100.0;
const BUTTON_PAD: f32 = 8.0;
const BUTTON_H: f32 = 20.0;
const BUTTON_GAP: f32 = 8.0;

#[cfg(self_update)]
const BAR_W: f32 = 220.0;
#[cfg(self_update)]
const BAR_H: f32 = 10.0;

const LINE: f32 = 12.0;

const BOX_W: f32 = BUTTON_W * 2.0 + BUTTON_GAP;
const BOX_H: f32 = 26.0;
const BOX_GAP: f32 = 4.0;
const BOXES_TOP: f32 = -24.0;
const BOX_PAD: f32 = 6.0;
const BOX_TEXT_TOP: f32 = 3.0;
const PERCENT_INSET: f32 = 18.0;
const BOX_RIGHT: f32 = PERCENT_INSET * 2.0;

const ASSETS_SIZE: &str = "43.7 MiB";
const AUDIO_SIZE: &str = if cfg!(feature = "small_audio") {
    "2.1 MiB"
} else {
    "330 MiB"
};
const CONTACTING: Key = Key::of("install.contacting");
const ASSETS_NAME: Key = Key::of("install.built_in");
const AUDIO_NAME: Key = Key::of("install.audio");
#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
const IMPORT_NAME: Key = Key::of("install.import");
#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
const IMPORT_DETAIL: Key = Key::of("install.import_detail");
const FROM_FILE: Key = Key::of("install.from_file");
const UNPACKING: Key = Key::of("install.unpacking");
const SKIPPED: Key = Key::of("install.skipped");
const WAITING: Key = Key::of("install.waiting");
const ALREADY_INSTALLED: Key = Key::of("install.already_installed");
const UNMOUNTABLE: Key = Key::of("install.fail.unmountable");

const BOX_LINES: [Key; 8] = [
    ASSETS_NAME,
    AUDIO_NAME,
    FROM_FILE,
    CONTACTING,
    UNPACKING,
    SKIPPED,
    WAITING,
    ALREADY_INSTALLED,
];

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
enum ImportLine {
    LanguageApplied,
    Kept(String),
    Refused(crate::client::assets::transfer::Refused),
}

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
static IMPORT_LINE: std::sync::Mutex<Option<ImportLine>> = std::sync::Mutex::new(None);

fn sounds_from_file() -> bool {
    #[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
    return crate::client::assets::transfer::kept_sounds();
    #[cfg(not(all(target_arch = "wasm32", feature = "asset_download")))]
    false
}

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
    p.text_centered(prompt.get(), cx, mid - 46.0, 0xFFFFFF, false);

    match stage {
        Stage::Idle | Stage::Cancelled => {
            if stage == Stage::Cancelled {
                p.text_centered(
                    tr!("install.cancelled"),
                    cx,
                    mid - 46.0 + LINE,
                    0xFFAA55,
                    false,
                );
            }
            let row = prompt_boxes(p, ctx, mid + BOXES_TOP);
            if action_and_leave(p, state, ctx, cx, row, continue_label()) {
                begin(dl);
            }
        }

        Stage::Asking
        | Stage::Downloading
        | Stage::Extracting
        | Stage::Fetching
        | Stage::Sounds => {
            let row = progress_boxes(p, ctx, mid + BOXES_TOP, dl, stage);
            if dl.sounds_running() {
                if button_pair(
                    p,
                    ctx,
                    cx,
                    row,
                    tr!("install.skip_audio"),
                    cancel_label(),
                    || dl.cancel(),
                ) {
                    dl.skip_sounds();
                }
            } else if button_centred(p, ctx, cx, row, cancel_label()) {
                dl.cancel();
            }
        }

        Stage::Done if top_up => {
            crate::client::assets::adopt_sounds();
            if !crate::client::assets::needs_restart() {
                leave(state);
                return;
            }
            p.text_centered(tr!("install.ready"), cx, mid - 14.0, 0x7FDF7F, false);
            p.text_centered(
                if cfg!(target_arch = "wasm32") {
                    tr!("install.reload_page")
                } else {
                    tr!("install.restart_client")
                },
                cx,
                mid - 14.0 + LINE,
                0xA0A0A8,
                false,
            );
            if button_centred(p, ctx, cx, mid + 20.0, leave_label()) {
                leave(state);
            }
        }
        Stage::Done => {
            prepare();
            p.text_centered(tr!("install.ready"), cx, mid - 14.0, 0x7FDF7F, false);
            p.text_centered(
                tr!("install.preparing"),
                cx,
                mid - 14.0 + LINE,
                0xA0A0A8,
                false,
            );
        }

        Stage::Failed => {
            let hint = if dl.network_failed() {
                tr!("install.check_connection")
            } else {
                tr!("install.see_log")
            };
            p.text_centered(dl.error(), cx, mid - 14.0, 0xFF5555, false);
            p.text_centered(hint, cx, mid - 14.0 + LINE, 0xA0A0A8, false);
            if action_and_leave(p, state, ctx, cx, mid + 20.0, tr!("install.try_again")) {
                begin(dl);
            }
        }
    }
}

pub(super) fn of_total(done: u64, total: u64) -> String {
    trf!("install.of_total", Bytes(done), Bytes(total))
}

fn box_span(p: &Painter, ctx: &ScreenCtx, plain: &[&str]) -> (f32, f32) {
    let font = &p.atlas.font;
    let beside = BOX_LINES
        .iter()
        .map(|k| font.width_str(k.get()) + BOX_PAD + BOX_RIGHT);
    let alone = plain.iter().map(|s| font.width_str(s) + BOX_PAD * 2.0);
    let need = beside.chain(alone).fold(BOX_W, f32::max).ceil();
    let w = fitted(need, ctx);
    (((ctx.vw - w) * 0.5).floor(), w)
}

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
fn import_lines() -> Vec<String> {
    if TOP_UP.load(Ordering::Relaxed) {
        return Vec::new();
    }
    let (name, detail, _) = import_text();
    vec![name.to_owned(), detail]
}

#[cfg(not(all(target_arch = "wasm32", feature = "asset_download")))]
fn import_lines() -> Vec<String> {
    Vec::new()
}

fn prompt_boxes(p: &mut Painter, ctx: &ScreenCtx, mut y: f32) -> f32 {
    let import = import_lines();
    let extra: Vec<&str> = import.iter().map(String::as_str).collect();
    let (x, w) = box_span(p, ctx, &extra);

    if offers_assets() {
        chrome(p, x, y, w, BOX_H, false);
        box_text(p, x + BOX_PAD, y, ASSETS_NAME.get(), ASSETS_SIZE, DETAIL);
        y += BOX_H + BOX_GAP;
    }
    if offers_sounds() && sounds_from_file() {
        chrome(p, x, y, w, BOX_H, false);
        box_text(p, x + BOX_PAD, y, AUDIO_NAME.get(), FROM_FILE.get(), DETAIL);
        y += BOX_H + BOX_GAP;
    } else if offers_sounds() {
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
        box_text(p, x + BOX_PAD, y, AUDIO_NAME.get(), AUDIO_SIZE, DETAIL);
        y += BOX_H + BOX_GAP;
    }
    #[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
    if !TOP_UP.load(Ordering::Relaxed) {
        y = import_box(p, ctx, x, y, w);
    }
    y + BOX_GAP
}

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
fn import_box(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32, w: f32) -> f32 {
    use crate::client::assets::transfer::{self, Prepared};

    if let Some(bytes) = crate::platform::upload::take_assets() {
        let line = match transfer::import_for_install(&bytes) {
            Ok(Prepared::Installed) => None,
            Ok(Prepared::Kept(words)) if words.is_empty() => Some(ImportLine::LanguageApplied),
            Ok(Prepared::Kept(words)) => Some(ImportLine::Kept(words)),
            Err(e) => Some(ImportLine::Refused(e)),
        };
        *IMPORT_LINE.lock().unwrap() = line;
    }

    let hovered = ctx.hovering(x, y, w, BOX_H);
    if hovered && ctx.input.left_click {
        crate::platform::upload::choose_assets();
    }
    chrome(p, x, y, w, BOX_H, hovered);
    let (name, detail, rgb) = import_text();
    box_text(p, x + BOX_PAD, y, name, &detail, rgb);
    y + BOX_H + BOX_GAP
}

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
fn import_text() -> (&'static str, String, u32) {
    match &*IMPORT_LINE.lock().unwrap() {
        None => (IMPORT_NAME.get(), IMPORT_DETAIL.get().to_owned(), DETAIL),
        Some(ImportLine::LanguageApplied) => {
            let line = tr!("install.language_applied");
            (IMPORT_NAME.get(), line.to_owned(), GREEN & 0xFF_FFFF)
        }
        Some(ImportLine::Kept(words)) => (
            tr!("install.ready_for_jar"),
            words.clone(),
            GREEN & 0xFF_FFFF,
        ),
        Some(ImportLine::Refused(e)) => (IMPORT_NAME.get(), e.short(), 0xFF5555),
    }
}

fn progress_boxes(
    p: &mut Painter,
    ctx: &ScreenCtx,
    mut y: f32,
    dl: &Download,
    stage: Stage,
) -> f32 {
    let (done, total) = dl.progress();
    let (sound_done, sound_total) = dl.sound_progress();
    let assets_line = if offers_assets() || total > 0 {
        Some(match stage {
            Stage::Extracting => UNPACKING.get().to_owned(),
            _ if total > 0 => of_total(done, total),
            _ => CONTACTING.get().to_owned(),
        })
    } else {
        None
    };
    let sounds_line = if offers_sounds() {
        Some(if sounds_from_file() {
            FROM_FILE.get().to_owned()
        } else if dl.sounds_skipped() {
            SKIPPED.get().to_owned()
        } else if sound_total > 0 {
            of_total(sound_done, sound_total)
        } else if stage == Stage::Asking || dl.sounds_running() {
            CONTACTING.get().to_owned()
        } else if cfg!(target_arch = "wasm32") {
            WAITING.get().to_owned()
        } else {
            ALREADY_INSTALLED.get().to_owned()
        })
    } else {
        None
    };
    let span = box_span(p, ctx, &[]);

    if let Some(line) = &assets_line {
        running_box(p, span, y, ASSETS_NAME.get(), line, done, total);
        y += BOX_H + BOX_GAP;
    }
    if let Some(line) = &sounds_line {
        let total = if dl.sounds_skipped() || sounds_from_file() {
            0
        } else {
            sound_total
        };
        running_box(p, span, y, AUDIO_NAME.get(), line, sound_done, total);
        y += BOX_H + BOX_GAP;
    }
    y + BOX_GAP
}

fn running_box(
    p: &mut Painter,
    (x, w): (f32, f32),
    y: f32,
    name: &str,
    line: &str,
    done: u64,
    total: u64,
) {
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

fn button(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32, w: f32, label: &str) -> bool {
    let hovered = ctx.hovering(x, y, w, BUTTON_H);
    chrome(p, x, y, w, BUTTON_H, hovered);
    p.text_centered(
        label,
        x + w * 0.5,
        (y + (BUTTON_H - 8.0) * 0.5).floor(),
        0xFFFFFF,
        false,
    );
    hovered && ctx.input.left_click
}

#[cfg(self_update)]
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
        download().report_failure(UNMOUNTABLE, "the set that just installed will not mount");
        return;
    }
    crate::renderer::reload::install_downloaded();
    #[cfg(target_arch = "wasm32")]
    if let Some(archive) = crate::client::assets::take_installed() {
        crate::client::mesh_worker::spawn(&archive);
    }
}
