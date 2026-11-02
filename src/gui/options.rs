use crate::gui::focus;
use crate::gui::painter::Painter;
use crate::gui::render::{GuiInput, auto_gui_scale};
use crate::gui::widgets::{
    self, Button, FOOTER_GAP, FOOTER_W, HeaderFooter, Slider, WIDGET_HEIGHT, WIDGET_WIDTH_BIG,
    centered_row_x, centered_x,
};
use crate::gui::{GAMMA_MAX, GuiState, Screen, ScreenCtx, keybinds};
use crate::text::LINE_HEIGHT;

const TITLE: &str = "Options";

const VIDEO_TITLE: &str = "Video Settings";
const VIDEO: &str = "Video Settings...";

const CONTROLS_TITLE: &str = "Controls";
const CONTROLS: &str = "Controls...";

#[cfg(feature = "audio")]
const AUDIO_TITLE: &str = "Music & Sounds";
#[cfg(feature = "audio")]
const AUDIO: &str = "Music & Sounds...";

const GAME_TITLE: &str = "Game";
const GAME: &str = "Game...";

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
const STORAGE_HEADER: &str = "Storage";
#[cfg(feature = "asset_download")]
const CLEAR_ASSETS: &str = "Clear Cached Assets";

#[cfg(feature = "asset_download")]
const CLEAR_ASSETS_TITLE: &str = "Clear cached assets?";
#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
const CLEAR_ASSETS_MESSAGE: &str = concat!(
    "The stored copy of the game's assets will be deleted.\n",
    "The client downloads about 44 MiB from Mojang again\n",
    "the next time this page is opened.",
);
#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
const CLEAR_ASSETS_WARNING: &str = "The page reloads immediately.";
#[cfg(all(not(target_arch = "wasm32"), feature = "asset_download"))]
const CLEAR_ASSETS_MESSAGE: &str = concat!(
    "The downloaded copy of the game's assets will be deleted.\n",
    "The client downloads them from Mojang again\n",
    "the next time it starts.",
);
#[cfg(all(not(target_arch = "wasm32"), feature = "asset_download"))]
const CLEAR_ASSETS_WARNING: &str = "The client closes immediately.";
#[cfg(feature = "asset_download")]
const CLEAR_ASSETS_YES: &str = "Clear";
#[cfg(feature = "asset_download")]
const CLEAR_ASSETS_NO: &str = "Cancel";

const DISPLAY_HEADER: &str = "Display";

const QUALITY_HEADER: &str = "Quality & Performance";

const DONE: &str = "Done";

const GUI_SCALE: &str = "GUI Scale";

const CLOUDS: &str = "Clouds";

const PARTICLES: &str = "Particles";

#[cfg(feature = "builtin_shaders")]
const SHADER_QUALITY: &str = "Shader Quality";

#[cfg(feature = "builtin_shaders")]
const SHADERS: &str = "Shaders";

#[cfg(feature = "builtin_shaders")]
const PLAYER_SHADOWS: &str = "Player Shadows";

const ANTIALIASING: &str = "Antialiasing";

const ON: &str = "ON";
const OFF: &str = "OFF";

const SMOOTH_LIGHTING: &str = "Smooth Lighting";
const LIGHTING_ENGINE: &str = "Lighting Engine";
const MAX_FRAMERATE: &str = "Max Framerate";
const UNLIMITED: &str = "Unlimited";
const VSYNC: &str = "VSync";

const FPS_COUNTER: &str = "FPS Counter";

#[cfg(feature = "mobile_ui")]
const TOUCH_MOVEMENT: &str = "Movement";

const RESET_VIDEO: &str = "Reset Video Settings";

const RESET_GAME: &str = "Reset Game Settings";

const RESET_ALL: &str = "Reset All Options";

const MANAGE_STORAGE: &str = "Manage Storage...";
const MANAGE_STORAGE_TITLE: &str = "Manage Storage";
const OPEN_CLIENT_FOLDER: &str = "Open torch-client Folder";
#[cfg(feature = "asset_download")]
const CHECK_ASSETS: &str = "Install missing assets";
#[cfg(feature = "asset_download")]
const NOTHING_MISSING: &str = "Nothing is missing";

const AUTO_JUMP: &str = "Auto-Jump";

const SPRINT_MODE: &str = "Sprint Mode";
const SPRINT_MODE_TOGGLE: &str = "Toggle";
const SPRINT_MODE_HOLD: &str = "Hold";

const MAIN_HAND: &str = "Main Hand";
const MAIN_HAND_LEFT: &str = "Left";
const MAIN_HAND_RIGHT: &str = "Right";

#[cfg(feature = "skins")]
const SKIN_HEADER: &str = "Skin Customisation";

#[cfg(feature = "skins")]
const SKIN_PART_LABELS: [&str; 7] = [
    "Cape",
    "Jacket",
    "Left Sleeve",
    "Right Sleeve",
    "Left Pants Leg",
    "Right Pants Leg",
    "Hat",
];

const CHAT_HEADER: &str = "Chat Settings";

const SIGNED_CHAT: &str = "Allow Signed Chat";

const TOAST_HEADER: &str = "Toasts";

const TOAST_ADVANCEMENT: &str = "Advancements";
const TOAST_RECIPE: &str = "Recipes";
const TOAST_SYSTEM: &str = "System";

const TOAST_TIME: &str = "Toast Time";

const SIGNED_CHAT_UNAVAILABLE: &str = "Needs a Microsoft account";

const FOV: &str = "FOV";

const BRIGHTNESS: &str = "Brightness";

const BRIGHTNESS_MOODY: &str = "Moody";

const BRIGHTNESS_BRIGHT: &str = "Bright";

const RENDER_DISTANCE: &str = "Render Distance";

const CHUNKS: &str = "Chunks";

const FOV_NORMAL: &str = "Normal";

const FOV_QUAKE_PRO: &str = "Quake Pro";

const FOV_MIN: i32 = 30;
const FOV_MAX: i32 = 110;

const FOV_EFFECTS: &str = "FOV Effects";

const FOV_EFFECTS_MAX: i32 = 100;

const SCREEN_EFFECTS: &str = "Distortion Effects";

const SCREEN_EFFECTS_MAX: i32 = 100;

pub(crate) const RENDER_DISTANCE_MIN: i32 = 2;
pub(crate) const RENDER_DISTANCE_MAX: i32 = 32;

const GUI_SCALE_AUTO: &str = "Auto";

pub(in crate::gui) const LAYOUT: HeaderFooter =
    HeaderFooter::new(HeaderFooter::DEFAULT_H, HeaderFooter::DEFAULT_H);

const GUI_SCALE_SLIDER_ID: u32 = 1;

const FOV_SLIDER_ID: u32 = 2;

const RENDER_DISTANCE_SLIDER_ID: u32 = 3;

const BRIGHTNESS_SLIDER_ID: u32 = 4;

const TOAST_TIME_SLIDER_ID: u32 = 5;

const FOV_EFFECTS_SLIDER_ID: u32 = 6;

#[cfg(feature = "audio")]
const VOLUME_SLIDER_ID_BASE: u32 = 7;

const SCREEN_EFFECTS_SLIDER_ID: u32 = 30;

const MAX_FRAMERATE_SLIDER_ID: u32 = 31;

const FRAMERATE_STEP: u32 = 10;

const ROW_SPACING: f32 = 4.0;

fn menu_content_h(entries: usize) -> f32 {
    let rows = entries.div_ceil(2) as f32;
    rows * WIDGET_HEIGHT + (rows - 1.0) * ROW_SPACING
}

pub(in crate::gui) const ROW_H: f32 = 25.0;

pub(in crate::gui) const ROW_INSET: f32 = 2.0;

pub(in crate::gui) const GRID_LEFT_OFFSET: f32 = 155.0;

pub(in crate::gui) const COLUMN_STEP: f32 = 160.0;

pub(in crate::gui) const COLUMN_W: f32 = 150.0;

pub(in crate::gui) const BIG_W: f32 = 310.0;

const HEADER_PAD_TOP: f32 = 2.0 * LINE_HEIGHT;

const HEADER_H: f32 = LINE_HEIGHT + 4.0;

const Y_DISPLAY_HEADER: f32 = 0.0;
const Y_DISPLAY_ROW_0: f32 = Y_DISPLAY_HEADER + HEADER_H;
const Y_DISPLAY_ROW_1: f32 = Y_DISPLAY_ROW_0 + ROW_H;
const Y_DISPLAY_ROW_2: f32 = Y_DISPLAY_ROW_1 + ROW_H;
const Y_DISPLAY_ROW_3: f32 = Y_DISPLAY_ROW_2 + ROW_H;
#[cfg(feature = "render_lod")]
const Y_LOD_HEADER: f32 = Y_DISPLAY_ROW_3 + ROW_H;
#[cfg(feature = "render_lod")]
const Y_LOD_HINT: f32 = Y_LOD_HEADER + HEADER_PAD_TOP + HEADER_H;
#[cfg(feature = "render_lod")]
const LOD_HINT_H: f32 = LOD_HINT.len() as f32 * (LINE_HEIGHT + 1.0) - 1.0 + 4.0;
#[cfg(feature = "render_lod")]
const Y_LOD_ROW_0: f32 = Y_LOD_HINT + LOD_HINT_H;
#[cfg(feature = "render_lod")]
const Y_LOD_ROW_1: f32 = Y_LOD_ROW_0 + ROW_H;
#[cfg(feature = "render_lod")]
const Y_LOD_ROW_2: f32 = Y_LOD_ROW_1 + ROW_H;
#[cfg(feature = "render_lod")]
const Y_QUALITY_HEADER: f32 = Y_LOD_ROW_2 + ROW_H;
#[cfg(not(feature = "render_lod"))]
const Y_QUALITY_HEADER: f32 = Y_DISPLAY_ROW_3 + ROW_H;
const Y_QUALITY_ROW_0: f32 = Y_QUALITY_HEADER + HEADER_PAD_TOP + HEADER_H;
const Y_QUALITY_ROW_1: f32 = Y_QUALITY_ROW_0 + ROW_H;
const Y_QUALITY_ROW_2: f32 = Y_QUALITY_ROW_1 + ROW_H;
const Y_QUALITY_ROW_3: f32 = Y_QUALITY_ROW_2 + ROW_H;
#[cfg(feature = "builtin_shaders")]
const Y_QUALITY_ROW_4: f32 = Y_QUALITY_ROW_3 + ROW_H;

#[cfg(feature = "builtin_shaders")]
const Y_QUALITY_LAST: f32 = Y_QUALITY_ROW_4;
#[cfg(not(feature = "builtin_shaders"))]
const Y_QUALITY_LAST: f32 = Y_QUALITY_ROW_3;

#[cfg(feature = "shader_support")]
const Y_SHADERPACKS_ROW: f32 = Y_QUALITY_LAST + ROW_H;

#[cfg(feature = "shader_support")]
const Y_RESET_ROW: f32 = Y_SHADERPACKS_ROW + ROW_H;
#[cfg(not(feature = "shader_support"))]
const Y_RESET_ROW: f32 = Y_QUALITY_LAST + ROW_H;

const VIDEO_CONTENT_H: f32 = Y_RESET_ROW + ROW_H;

const LIST_PAD_BOTTOM: f32 = 4.0;

const VIDEO_LIST_H: f32 = VIDEO_CONTENT_H + LIST_PAD_BOTTOM;

pub(in crate::gui) const SCROLLBAR_W: f32 = 6.0;

const SCROLLBAR_MIN_H: f32 = 32.0;

const SCROLLBAR_MAX_SLACK: f32 = 8.0;

pub(in crate::gui) const SCROLLBAR_GAP: f32 = SCROLLBAR_W + 2.0;

const SCROLL_RATE: f32 = ROW_H / 2.0;

#[cfg(feature = "mobile_ui")]
const SCROLLBAR_TOUCH_PAD: f32 = 8.0;
#[cfg(not(feature = "mobile_ui"))]
const SCROLLBAR_TOUCH_PAD: f32 = 0.0;

fn scroller_height(list_h: f32, content_h: f32) -> f32 {
    let h = list_h * list_h / content_h.max(1.0);
    h.min(list_h - SCROLLBAR_MAX_SLACK).max(SCROLLBAR_MIN_H)
}

fn scroller_y(scroll: f32, max_scroll: f32, list_y: f32, list_h: f32, scroller_h: f32) -> f32 {
    if max_scroll <= 0.0 {
        return list_y;
    }
    list_y + (scroll * (list_h - scroller_h) / max_scroll).max(0.0)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::gui) fn scroll_input(
    scroll: &mut f32,
    drag: &mut Option<f32>,
    ctx: &ScreenCtx,
    left: f32,
    row_w: f32,
    list_y: f32,
    list_h: f32,
    content_h: f32,
) {
    scroll_input_in(
        scroll,
        drag,
        ctx,
        left,
        row_w,
        (0.0, ctx.vw),
        list_y,
        list_h,
        content_h,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::gui) fn scroll_input_in(
    scroll: &mut f32,
    drag: &mut Option<f32>,
    ctx: &ScreenCtx,
    left: f32,
    row_w: f32,
    (list_x, list_w): (f32, f32),
    list_y: f32,
    list_h: f32,
    content_h: f32,
) {
    let max_scroll = (content_h - list_h).max(0.0);
    if max_scroll <= 0.0 {
        *scroll = 0.0;
        *drag = None;
        return;
    }

    if ctx.input.scroll != 0.0 && ctx.hovering(list_x, list_y, list_w, list_h) {
        *scroll -= ctx.input.scroll * SCROLL_RATE;
    }

    let scroller_h = scroller_height(list_h, content_h);
    let bar_x = left + row_w + SCROLLBAR_GAP;
    let over_bar = ctx.hovering(
        bar_x - SCROLLBAR_TOUCH_PAD,
        list_y,
        SCROLLBAR_W + 2.0 * SCROLLBAR_TOUCH_PAD,
        list_h,
    );

    #[cfg(feature = "mobile_ui")]
    content_drag_in(
        scroll, ctx, list_x, list_y, list_w, list_h, over_bar, max_scroll,
    );

    if !ctx.input.left_down || ctx.input.left_release {
        *drag = None;
    } else if ctx.input.left_click && over_bar {
        *drag = ctx.mouse().map(|m| m.y);
    } else if let (Some(previous), Some(m)) = (*drag, ctx.mouse()) {
        if m.y < list_y {
            *scroll = 0.0;
        } else if m.y > list_y + list_h {
            *scroll = max_scroll;
        } else {
            let per_unit = (max_scroll / (list_h - scroller_h)).max(1.0);
            *scroll += (m.y - previous) * per_unit;
        }
        *drag = Some(m.y);
    }

    *scroll = scroll.clamp(0.0, max_scroll);
}

pub(in crate::gui) fn masked_input(
    ctx: &ScreenCtx,
    rect: (f32, f32, f32, f32),
    dragging: bool,
) -> GuiInput {
    mask_input(ctx, rect, dragging, true)
}

pub(in crate::gui) fn mask_input(
    ctx: &ScreenCtx,
    rect: (f32, f32, f32, f32),
    dragging: bool,
    keep_slider_drag: bool,
) -> GuiInput {
    let (x, y, w, h) = rect;
    let over = ctx.hovering(x, y, w, h);
    let mut input = ctx.input.clone();
    if dragging || (!over && !(keep_slider_drag && widgets::slider_dragging())) {
        input.mouse = None;
    }
    if dragging || !over {
        input.left_click = false;
    }
    input
}

#[cfg_attr(not(any(feature = "shader_support", resource_packs)), allow(dead_code))]
pub(in crate::gui) fn scroll_masked(
    scroll: &mut f32,
    drag: &mut Option<f32>,
    ctx: &ScreenCtx,
    left: f32,
    row_w: f32,
    rect: (f32, f32, f32, f32),
    content_h: f32,
) -> GuiInput {
    scroll_input_in(
        scroll,
        drag,
        ctx,
        left,
        row_w,
        (rect.0, rect.2),
        rect.1,
        rect.3,
        content_h,
    );
    masked_input(ctx, rect, drag.is_some())
}

#[cfg_attr(not(any(feature = "shader_support", resource_packs)), allow(dead_code))]
pub(in crate::gui) fn row_visible(y: f32, row_h: f32, list_y: f32, list_h: f32) -> bool {
    y + row_h >= list_y && y <= list_y + list_h
}

#[cfg(feature = "mobile_ui")]
#[allow(clippy::too_many_arguments)]
pub(in crate::gui) fn content_drag_in(
    scroll: &mut f32,
    ctx: &ScreenCtx,
    list_x: f32,
    list_y: f32,
    list_w: f32,
    list_h: f32,
    over_bar: bool,
    max_scroll: f32,
) {
    use crate::gui::widgets::touch_tap;
    use std::sync::atomic::{AtomicU32, Ordering};

    const DRAG_THRESHOLD: f32 = 4.0;
    const NO_OWNER: u32 = u32::MAX;

    static OWNER: AtomicU32 = AtomicU32::new(NO_OWNER);
    static LAST_Y: AtomicU32 = AtomicU32::new(0);
    static START_Y: AtomicU32 = AtomicU32::new(0);

    let Some(m) = ctx.mouse() else {
        OWNER.store(NO_OWNER, Ordering::Relaxed);
        return;
    };

    if !ctx.input.left_down || ctx.input.left_release {
        OWNER.store(NO_OWNER, Ordering::Relaxed);
        return;
    }

    let me = list_x.to_bits();
    if ctx.input.left_click {
        if ctx.hovering(list_x, list_y, list_w, list_h) && !over_bar {
            OWNER.store(me, Ordering::Relaxed);
            LAST_Y.store(m.y.to_bits(), Ordering::Relaxed);
            START_Y.store(m.y.to_bits(), Ordering::Relaxed);
        } else if OWNER.load(Ordering::Relaxed) == me {
            OWNER.store(NO_OWNER, Ordering::Relaxed);
        }
        return;
    }

    if OWNER.load(Ordering::Relaxed) != me {
        return;
    }
    let start = f32::from_bits(START_Y.load(Ordering::Relaxed));
    let last = f32::from_bits(LAST_Y.load(Ordering::Relaxed));
    LAST_Y.store(m.y.to_bits(), Ordering::Relaxed);
    if (m.y - start).abs() < DRAG_THRESHOLD && !touch_tap::scrolled() {
        return;
    }
    touch_tap::mark_scrolled();
    *scroll = (*scroll - (m.y - last)).clamp(0.0, max_scroll);
}

pub(in crate::gui) fn draw_scrollbar(
    p: &mut Painter,
    scroll: f32,
    left: f32,
    row_w: f32,
    list_y: f32,
    list_h: f32,
    content_h: f32,
) {
    let max_scroll = content_h - list_h;
    if max_scroll <= 0.0 {
        return;
    }
    let x = left + row_w + SCROLLBAR_GAP;
    let scroller_h = scroller_height(list_h, content_h);
    p.sprite("widget/scroller_background", x, list_y, SCROLLBAR_W, list_h);
    p.sprite(
        "widget/scroller",
        x,
        scroller_y(scroll, max_scroll, list_y, list_h, scroller_h).floor(),
        SCROLLBAR_W,
        scroller_h,
    );
}

const Y_GAME_ROW_0: f32 = 0.0;
const Y_GAME_ROW_1: f32 = Y_GAME_ROW_0 + ROW_H;
const Y_CHAT_HEADER: f32 = Y_GAME_ROW_1 + ROW_H;
const Y_CHAT_ROW_0: f32 = Y_CHAT_HEADER + HEADER_PAD_TOP + HEADER_H;
const Y_TOAST_HEADER: f32 = Y_CHAT_ROW_0 + ROW_H;
const Y_TOAST_ROW_0: f32 = Y_TOAST_HEADER + HEADER_PAD_TOP + HEADER_H;
const Y_TOAST_ROW_1: f32 = Y_TOAST_ROW_0 + ROW_H;

#[cfg(feature = "skins")]
const Y_SKIN_HEADER: f32 = Y_TOAST_ROW_1 + ROW_H;
#[cfg(feature = "skins")]
const Y_SKIN_ROW_0: f32 = Y_SKIN_HEADER + HEADER_PAD_TOP + HEADER_H;
#[cfg(feature = "skins")]
const Y_SKIN_ROW_1: f32 = Y_SKIN_ROW_0 + ROW_H;
#[cfg(feature = "skins")]
const Y_SKIN_ROW_2: f32 = Y_SKIN_ROW_1 + ROW_H;
#[cfg(feature = "skins")]
const Y_SKIN_ROW_3: f32 = Y_SKIN_ROW_2 + ROW_H;

#[cfg(feature = "skins")]
const Y_GAME_LAST_GROUP_ROW: f32 = Y_SKIN_ROW_3;
#[cfg(not(feature = "skins"))]
const Y_GAME_LAST_GROUP_ROW: f32 = Y_TOAST_ROW_1;

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
const Y_STORAGE_HEADER: f32 = Y_GAME_LAST_GROUP_ROW + ROW_H;
#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
const Y_STORAGE_ROW_0: f32 = Y_STORAGE_HEADER + HEADER_PAD_TOP + HEADER_H;

#[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
const Y_GAME_RESET_ROW: f32 = Y_STORAGE_ROW_0 + ROW_H;
#[cfg(not(all(target_arch = "wasm32", feature = "asset_download")))]
const Y_GAME_RESET_ROW: f32 = Y_GAME_LAST_GROUP_ROW + ROW_H;

const GAME_CONTENT_H: f32 = Y_GAME_RESET_ROW + ROW_H;

const GAME_LIST_H: f32 = GAME_CONTENT_H + LIST_PAD_BOTTOM;

#[cfg(feature = "audio")]
const Y_AUDIO_MASTER: f32 = 0.0;
#[cfg(feature = "audio")]
const Y_AUDIO_ROW_0: f32 = Y_AUDIO_MASTER + ROW_H;
#[cfg(feature = "audio")]
const Y_AUDIO_ROW_1: f32 = Y_AUDIO_ROW_0 + ROW_H;
#[cfg(feature = "audio")]
const Y_AUDIO_ROW_2: f32 = Y_AUDIO_ROW_1 + ROW_H;
#[cfg(feature = "audio")]
const Y_AUDIO_ROW_3: f32 = Y_AUDIO_ROW_2 + ROW_H;
#[cfg(feature = "audio")]
const Y_AUDIO_ROW_4: f32 = Y_AUDIO_ROW_3 + ROW_H;

#[cfg(feature = "audio")]
const AUDIO_CONTENT_H: f32 = Y_AUDIO_ROW_4 + ROW_H;

fn fov_label(value: i32) -> String {
    match value {
        70 => format!("{FOV}: {FOV_NORMAL}"),
        FOV_MAX => format!("{FOV}: {FOV_QUAKE_PRO}"),
        v => format!("{FOV}: {v}"),
    }
}

fn percent_or_off(caption: &str, value: i32) -> String {
    if value == 0 {
        format!("{caption}: {OFF}")
    } else {
        format!("{caption}: {value}%")
    }
}

fn render_distance_label(value: i32) -> String {
    format!("{RENDER_DISTANCE}: {value} {CHUNKS}")
}

#[cfg(feature = "render_lod")]
const LOD_HEADER: &str = "Far terrain";

#[cfg(feature = "render_lod")]
const LOD_HINT: [&str; 2] = [
    "Real blocks draw to Render Distance, low-detail terrain to",
    "LOD Distance, from chunks the server sent this session.",
];

#[cfg(feature = "render_lod")]
const REAL_BLOCKS: &str = "(real blocks)";

#[cfg(feature = "render_lod")]
const LOD_DISTANCE: &str = "LOD Distance";

#[cfg(feature = "render_lod")]
const LOD_DISTANCE_FLOOR: &str = "(Render Distance + 1)";

#[cfg(feature = "render_lod")]
const LOD_QUALITY: &str = "LOD Detail";

#[cfg(feature = "render_lod")]
const LOD_DISTANCE_SLIDER_ID: u32 = 32;

#[cfg(feature = "render_lod")]
const LOD_QUALITY_SLIDER_ID: u32 = 33;

#[cfg(feature = "render_lod")]
const LOD_DISTANCE_STOPS: [u32; 44] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    28, 29, 30, 31, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048, 3072, 4096,
];

#[cfg(feature = "render_lod")]
fn lod_distance_stop(chunks: u32) -> i32 {
    LOD_DISTANCE_STOPS
        .iter()
        .position(|&s| s >= chunks)
        .unwrap_or(LOD_DISTANCE_STOPS.len() - 1) as i32
}

#[cfg(feature = "render_lod")]
fn lod_render_distance_label(value: i32) -> String {
    format!("{} {REAL_BLOCKS}", render_distance_label(value))
}

#[cfg(feature = "render_lod")]
fn lod_distance_label(stop: i32, render_distance: u32) -> String {
    let chunks = LOD_DISTANCE_STOPS[(stop.max(0) as usize).min(LOD_DISTANCE_STOPS.len() - 1)];
    let floor = render_distance + 1;
    if chunks >= floor {
        format!("{LOD_DISTANCE}: {chunks} {CHUNKS}")
    } else {
        format!("{LOD_DISTANCE}: {floor} {CHUNKS} {LOD_DISTANCE_FLOOR}")
    }
}

#[cfg(feature = "render_lod")]
fn lod_quality_label(index: i32) -> String {
    let q = crate::gui::LodQuality::ALL
        [(index.max(0) as usize).min(crate::gui::LodQuality::ALL.len() - 1)];
    format!("{LOD_QUALITY}: {}", q.caption())
}

#[cfg(feature = "render_lod")]
fn draw_lod_group(
    p: &mut Painter,
    ctx: &ScreenCtx,
    state: &mut GuiState,
    cell: &dyn Fn(f32, f32) -> (f32, f32),
    header: &dyn Fn(&mut Painter, f32, f32, &str),
) {
    header(p, Y_LOD_HEADER, HEADER_PAD_TOP, LOD_HEADER);

    let (x, y) = cell(0.0, Y_LOD_HINT);
    for (i, line) in LOD_HINT.iter().enumerate() {
        p.text_str(line, x, y + i as f32 * (LINE_HEIGHT + 1.0), 0xA0A0A0, true);
    }

    let (x, y) = cell(0.0, Y_LOD_ROW_1);
    let stop = lod_distance_stop(state.options.lod_distance);
    let render_distance = state.options.render_distance;
    let picked = Slider {
        id: LOD_DISTANCE_SLIDER_ID,
        x,
        y,
        w: BIG_W,
        h: WIDGET_HEIGHT,
        min: 0,
        max: LOD_DISTANCE_STOPS.len() as i32 - 1,
        value: stop,
        label: |v| lod_distance_label(v, render_distance),
    }
    .draw(p, ctx)
    .clamp(0, LOD_DISTANCE_STOPS.len() as i32 - 1);
    if picked != stop {
        state.options.lod_distance = LOD_DISTANCE_STOPS[picked as usize];
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(0.0, Y_LOD_ROW_2);
    let all = crate::gui::LodQuality::ALL;
    let current = all
        .iter()
        .position(|q| *q == state.options.lod_quality)
        .unwrap_or(0) as i32;
    let picked = Slider {
        id: LOD_QUALITY_SLIDER_ID,
        x,
        y,
        w: BIG_W,
        h: WIDGET_HEIGHT,
        min: 0,
        max: all.len() as i32 - 1,
        value: current,
        label: lod_quality_label,
    }
    .draw(p, ctx)
    .clamp(0, all.len() as i32 - 1);
    if picked != current {
        state.options.lod_quality = all[picked as usize];
        crate::gui::save_options(&state.options);
    }
}

fn brightness_label(value: i32) -> String {
    match value {
        0 => format!("{BRIGHTNESS}: {BRIGHTNESS_MOODY}"),
        100 => format!("{BRIGHTNESS}: {BRIGHTNESS_BRIGHT}"),
        v => format!("{BRIGHTNESS}: {v}%"),
    }
}

fn toast_time_label(value: i32) -> String {
    format!("{TOAST_TIME}: {}.{}s", value / 10, value % 10)
}

fn max_framerate_label(fps: u32) -> String {
    if fps >= crate::gui::MAX_FPS_UNLIMITED {
        format!("{MAX_FRAMERATE}: {UNLIMITED}")
    } else {
        format!("{MAX_FRAMERATE}: {fps} fps")
    }
}

fn gui_scale_label(value: i32) -> String {
    if value == 0 {
        format!("{GUI_SCALE}: {GUI_SCALE_AUTO}")
    } else {
        format!("{GUI_SCALE}: {value}")
    }
}

fn toggle(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32, label: &str, value: &mut bool) -> bool {
    let caption = format!("{label}: {}", if *value { ON } else { OFF });

    if !Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, &caption).draw(p, ctx) {
        return false;
    }

    *value = !*value;
    true
}

pub(in crate::gui) fn draw_title(p: &mut Painter, ctx: &ScreenCtx, title: &str) {
    p.text_centered(title, ctx.vw / 2.0, LAYOUT.title_y(), 0xFFFFFF, true);
}

pub(in crate::gui) fn draw_done(p: &mut Painter, ctx: &ScreenCtx) -> bool {
    Button::new(
        centered_x(ctx.vw),
        LAYOUT.footer_y(ctx.vh),
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        DONE,
    )
    .draw(p, ctx)
}

const VIDEO_KEYS: &[&str] = &[
    "guiScale",
    "fov",
    "fovEffectScale",
    "screenEffectScale",
    "renderClouds",
    "particles",
    "renderDistance",
    "gamma",
    "antialiasing",
    "ao",
    "lightingEnabled",
    "maxFps",
    "vsync",
    "fpsCounter",
    "shaders",
    "shaderQuality",
    "playerShadows",
    "touchMovement",
    #[cfg(feature = "render_lod")]
    "lodDistance",
    #[cfg(feature = "render_lod")]
    "lodQuality",
];

const GAME_KEYS: &[&str] = &[
    "autoJump",
    "toggleSprint",
    "mainHand",
    "allowSignedChat",
    "toastAdvancement",
    "toastRecipe",
    "toastSystem",
    "toastDisplayTime",
];

fn reset_video_options(o: &mut crate::gui::GuiOptions) {
    crate::gui::reset_options(o, |key| VIDEO_KEYS.contains(&key));
}

fn reset_game_options(o: &mut crate::gui::GuiOptions) {
    crate::gui::reset_options(o, |key| {
        GAME_KEYS.contains(&key) || key.starts_with("modelPart_")
    });
}

#[cfg(feature = "audio")]
fn reset_audio_options(o: &mut crate::gui::GuiOptions) {
    crate::gui::reset_options(o, |key| key.starts_with("soundCategory_"));
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    draw_title(p, ctx, TITLE);

    enum Entry {
        Screen(Screen),
        #[cfg(resource_packs)]
        Packs,
        #[cfg(feature = "hud_editor")]
        Hud,
    }

    let mut entries = vec![
        (VIDEO, Entry::Screen(Screen::VideoSettings)),
        (CONTROLS, Entry::Screen(Screen::Controls)),
        (GAME, Entry::Screen(Screen::GameSettings)),
    ];
    #[cfg(feature = "audio")]
    entries.push((AUDIO, Entry::Screen(Screen::AudioSettings)));
    #[cfg(resource_packs)]
    entries.push((crate::gui::resourcepacks::RESOURCE_PACKS, Entry::Packs));
    #[cfg(feature = "hud_editor")]
    entries.push(("Edit HUD...", Entry::Hud));
    entries.push((MANAGE_STORAGE, Entry::Screen(Screen::ManageStorage)));

    let left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();
    let top = LAYOUT.content_y(ctx.vh, menu_content_h(entries.len()));
    let row = |i: usize| top + i as f32 * (WIDGET_HEIGHT + ROW_SPACING);

    for (i, (label, entry)) in entries.iter().enumerate() {
        let x = left + (i % 2) as f32 * COLUMN_STEP;
        let button = Button::new(x, row(i / 2), COLUMN_W, WIDGET_HEIGHT, label);
        #[cfg(feature = "tutorial")]
        {
            let r = crate::gui::hud_layout::Rect::new(x, row(i / 2), COLUMN_W, WIDGET_HEIGHT);
            match entry {
                #[cfg(feature = "hud_editor")]
                Entry::Hud => crate::gui::tutorial::mark(crate::gui::tutorial::Spot::EditHud, r),
                Entry::Screen(Screen::ManageStorage) => {
                    crate::gui::tutorial::mark(crate::gui::tutorial::Spot::Storage, r)
                }
                _ => {}
            }
        }
        if !button.draw(p, ctx) {
            continue;
        }
        match entry {
            Entry::Screen(screen) => {
                state.video_scroll = 0.0;
                state.video_scroll_drag = None;
                state.game_scroll = 0.0;
                state.game_scroll_drag = None;
                #[cfg(feature = "asset_download")]
                {
                    state.confirm_clear_assets = false;
                    state.storage_nothing_missing = false;
                }
                state.controls_scroll = 0.0;
                state.controls_scroll_drag = None;
                state.keybinds.capturing = None;
                state.nav = Some(*screen);
            }
            #[cfg(resource_packs)]
            Entry::Packs => {
                state.resourcepacks.open();
                state.nav = Some(Screen::ResourcePacks);
            }
            #[cfg(feature = "hud_editor")]
            Entry::Hud => {
                state.hud_state.parent = Screen::Options;
                state.hud_state.over_menu = state.in_menu();
                state.nav = Some(Screen::HudEditor);
            }
        }
    }

    if draw_done(p, ctx) {
        state.nav = Some(state.options_parent);
    }
}

pub fn draw_manage_storage(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    #[cfg(all(not(target_arch = "wasm32"), feature = "asset_download"))]
    if state.confirm_clear_assets {
        draw_clear_assets(p, state, ctx);
        return;
    }

    draw_title(p, ctx, MANAGE_STORAGE_TITLE);

    let mut count = 1;
    if cfg!(not(any(target_arch = "wasm32", target_os = "android"))) {
        count += 1;
    }
    if cfg!(feature = "asset_download") {
        count += 1;
    }
    if cfg!(all(not(target_arch = "wasm32"), feature = "asset_download")) {
        count += 1;
    }
    if cfg!(feature = "tutorial") {
        count += 1;
    }
    let left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();
    let top = LAYOUT.content_y(ctx.vh, menu_content_h(count));
    #[allow(unused)]
    let cell = |i: usize| {
        (
            left + (i % 2) as f32 * COLUMN_STEP,
            top + (i / 2) as f32 * (WIDGET_HEIGHT + ROW_SPACING),
        )
    };
    #[allow(unused)]
    let mut next = 1;

    if Button::new(left, top, COLUMN_W, WIDGET_HEIGHT, RESET_ALL).draw(p, ctx) {
        reset_video_options(&mut state.options);
        reset_game_options(&mut state.options);
        #[cfg(feature = "audio")]
        {
            reset_audio_options(&mut state.options);
            crate::audio::set_volumes(&state.options.volumes);
        }
        state.keybinds.reset_all();
        crate::gui::save_options(&state.options);
    }

    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    {
        let (x, y) = cell(next);
        next += 1;
        if Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, OPEN_CLIENT_FOLDER).draw(p, ctx) {
            crate::platform::url::open_folder(&crate::platform::storage::dir());
        }
    }

    #[cfg(feature = "tutorial")]
    {
        let (x, y) = cell(next);
        next += 1;
        crate::gui::tutorial::mark(
            crate::gui::tutorial::Spot::Restart,
            crate::gui::hud_layout::Rect::new(x, y, COLUMN_W, WIDGET_HEIGHT),
        );
        if Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, "Restart Tutorial").draw(p, ctx) {
            crate::gui::tutorial::restart(state);
        }
    }

    #[cfg(feature = "asset_download")]
    {
        let (x, y) = cell(next);
        let label = if state.storage_nothing_missing {
            NOTHING_MISSING
        } else {
            CHECK_ASSETS
        };
        if Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, label).draw(p, ctx) {
            state.storage_nothing_missing = !crate::gui::assetdownload::check_missing(state);
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "asset_download"))]
    {
        let (x, y) = cell(next);
        next += 1;
        if Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, CLEAR_ASSETS).draw(p, ctx) {
            state.confirm_clear_assets = true;
        }
    }

    if draw_done(p, ctx) {
        state.nav = Some(Screen::Options);
    }
}

pub fn draw_video(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    draw_title(p, ctx, VIDEO_TITLE);

    let max = auto_gui_scale(ctx.vw * ctx.input.scale, ctx.vh * ctx.input.scale) as i32;

    let left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();
    let (list_x, list_y, list_w, list_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    #[cfg(feature = "tutorial")]
    crate::gui::tutorial::mark(
        crate::gui::tutorial::Spot::Video,
        crate::gui::hud_layout::Rect::new(list_x, list_y, list_w, list_h),
    );
    scroll_input(
        &mut state.video_scroll,
        &mut state.video_scroll_drag,
        ctx,
        left,
        BIG_W,
        list_y,
        list_h,
        VIDEO_LIST_H,
    );
    let top = (list_y - state.video_scroll).floor();

    let list_input = masked_input(
        ctx,
        (list_x, list_y, list_w, list_h),
        state.video_scroll_drag.is_some(),
    );
    let list_ctx = &ctx.with_input(&list_input);
    let screen_ctx = ctx;
    let ctx = list_ctx;

    let clip = p.push_clip(list_x, list_y, list_w, list_h);

    let cell = |column: f32, row_y: f32| (left + column * COLUMN_STEP, top + row_y + ROW_INSET);
    let header = |p: &mut Painter, row_y: f32, pad_top: f32, text: &str| {
        p.text_str(
            text,
            left,
            top + row_y + ROW_INSET + pad_top,
            0xFFFFFF,
            true,
        );
    };

    header(p, Y_DISPLAY_HEADER, 0.0, DISPLAY_HEADER);

    let stored_fps = state.options.max_fps;
    let stored_step = (stored_fps / FRAMERATE_STEP) as i32;
    let (x, y) = cell(0.0, Y_DISPLAY_ROW_0);
    let max_fps = Slider {
        id: MAX_FRAMERATE_SLIDER_ID,
        x,
        y,
        w: COLUMN_W,
        h: WIDGET_HEIGHT,
        min: (crate::gui::MAX_FPS_MIN / FRAMERATE_STEP) as i32,
        max: (crate::gui::MAX_FPS_UNLIMITED / FRAMERATE_STEP) as i32,
        value: stored_step,
        label: |step: i32| {
            let fps = if step == stored_step {
                stored_fps
            } else {
                step.max(0) as u32 * FRAMERATE_STEP
            };
            max_framerate_label(fps)
        },
    };
    let new_step = max_fps.draw(p, ctx);
    if new_step != stored_step {
        state.options.max_fps = (new_step.max(0) as u32 * FRAMERATE_STEP)
            .clamp(crate::gui::MAX_FPS_MIN, crate::gui::MAX_FPS_UNLIMITED);
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(1.0, Y_DISPLAY_ROW_0);
    let gui_scale = Slider {
        id: GUI_SCALE_SLIDER_ID,
        x,
        y,
        w: COLUMN_W,
        h: WIDGET_HEIGHT,
        min: 0,
        max,
        value: state.options.gui_scale as i32,
        label: gui_scale_label,
    };
    let new_gui_scale = gui_scale.draw(p, ctx).max(0) as u32;
    if new_gui_scale != state.options.gui_scale {
        state.options.gui_scale = new_gui_scale;
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(0.0, Y_DISPLAY_ROW_1);
    let brightness = Slider {
        id: BRIGHTNESS_SLIDER_ID,
        x,
        y,
        w: COLUMN_W,
        h: WIDGET_HEIGHT,
        min: 0,
        max: GAMMA_MAX as i32,
        value: state.options.gamma as i32,
        label: brightness_label,
    };
    let new_gamma = brightness.draw(p, ctx).clamp(0, GAMMA_MAX as i32) as u32;
    if new_gamma != state.options.gamma {
        state.options.gamma = new_gamma;
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(1.0, Y_DISPLAY_ROW_1);
    let fov = Slider {
        id: FOV_SLIDER_ID,
        x,
        y,
        w: COLUMN_W,
        h: WIDGET_HEIGHT,
        min: FOV_MIN,
        max: FOV_MAX,
        value: state.options.fov as i32,
        label: fov_label,
    };
    let new_fov = fov.draw(p, ctx).clamp(FOV_MIN, FOV_MAX) as u32;
    if new_fov != state.options.fov {
        state.options.fov = new_fov;
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(0.0, Y_DISPLAY_ROW_2);
    if toggle(p, ctx, x, y, FPS_COUNTER, &mut state.options.fps_counter) {
        crate::gui::save_options(&state.options);
    }

    #[cfg(not(target_arch = "wasm32"))]
    let vsync_cell = Some(cell(1.0, Y_DISPLAY_ROW_2));
    #[cfg(target_arch = "wasm32")]
    let vsync_cell: Option<(f32, f32)> = None;
    if let Some((x, y)) = vsync_cell
        && toggle(p, ctx, x, y, VSYNC, &mut state.options.vsync)
    {
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(0.0, Y_DISPLAY_ROW_3);
    let fov_effects = Slider {
        id: FOV_EFFECTS_SLIDER_ID,
        x,
        y,
        w: COLUMN_W,
        h: WIDGET_HEIGHT,
        min: 0,
        max: FOV_EFFECTS_MAX,
        value: state.options.fov_effects as i32,
        label: |v| percent_or_off(FOV_EFFECTS, v),
    };
    let new_fov_effects = fov_effects.draw(p, ctx).clamp(0, FOV_EFFECTS_MAX) as u32;
    if new_fov_effects != state.options.fov_effects {
        state.options.fov_effects = new_fov_effects;
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(1.0, Y_DISPLAY_ROW_3);
    let screen_effects = Slider {
        id: SCREEN_EFFECTS_SLIDER_ID,
        x,
        y,
        w: COLUMN_W,
        h: WIDGET_HEIGHT,
        min: 0,
        max: SCREEN_EFFECTS_MAX,
        value: state.options.screen_effects as i32,
        label: |v| percent_or_off(SCREEN_EFFECTS, v),
    };
    let new_screen_effects = screen_effects.draw(p, ctx).clamp(0, SCREEN_EFFECTS_MAX) as u32;
    if new_screen_effects != state.options.screen_effects {
        state.options.screen_effects = new_screen_effects;
        crate::gui::save_options(&state.options);
    }

    header(p, Y_QUALITY_HEADER, HEADER_PAD_TOP, QUALITY_HEADER);

    #[cfg(feature = "render_lod")]
    let (x, y) = cell(0.0, Y_LOD_ROW_0);
    #[cfg(not(feature = "render_lod"))]
    let (x, y) = cell(0.0, Y_QUALITY_ROW_0);
    let render_distance = Slider {
        id: RENDER_DISTANCE_SLIDER_ID,
        x,
        y,
        #[cfg(feature = "render_lod")]
        w: BIG_W,
        #[cfg(not(feature = "render_lod"))]
        w: COLUMN_W,
        h: WIDGET_HEIGHT,
        min: RENDER_DISTANCE_MIN,
        max: RENDER_DISTANCE_MAX,
        value: state.options.render_distance as i32,
        #[cfg(feature = "render_lod")]
        label: lod_render_distance_label,
        #[cfg(not(feature = "render_lod"))]
        label: render_distance_label,
    };
    let new_render_distance = render_distance
        .draw(p, ctx)
        .clamp(RENDER_DISTANCE_MIN, RENDER_DISTANCE_MAX) as u32;
    if new_render_distance != state.options.render_distance {
        state.options.render_distance = new_render_distance;
        crate::gui::save_options(&state.options);
    }

    #[cfg(feature = "render_lod")]
    draw_lod_group(p, ctx, state, &cell, &header);

    let clouds_label = format!("{CLOUDS}: {}", state.options.clouds.caption());
    let (x, y) = cell(1.0, Y_QUALITY_ROW_0);
    let clouds = Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, &clouds_label);
    if clouds.draw(p, ctx) {
        state.options.clouds = state.options.clouds.next();
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(0.0, Y_QUALITY_ROW_1);
    if toggle(
        p,
        ctx,
        x,
        y,
        SMOOTH_LIGHTING,
        &mut state.options.smooth_lighting,
    ) {
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(1.0, Y_QUALITY_ROW_1);
    if toggle(p, ctx, x, y, ANTIALIASING, &mut state.options.antialiasing) {
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(0.0, Y_QUALITY_ROW_2);
    if toggle(
        p,
        ctx,
        x,
        y,
        LIGHTING_ENGINE,
        &mut state.options.lighting_enabled,
    ) {
        crate::gui::save_options(&state.options);
    }

    let particles_label = format!("{PARTICLES}: {}", state.options.particles.caption());
    let (x, y) = cell(0.0, Y_QUALITY_ROW_3);
    let particles = Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, &particles_label);
    if particles.draw(p, ctx) {
        state.options.particles = state.options.particles.next();
        crate::gui::save_options(&state.options);
    }

    #[cfg(feature = "builtin_shaders")]
    {
        let (x, y) = cell(1.0, Y_QUALITY_ROW_2);
        if toggle(p, ctx, x, y, SHADERS, &mut state.options.shaders_enabled) {
            crate::gui::save_options(&state.options);
        }

        let enabled = state.options.shaders_enabled;

        let label = format!(
            "{SHADER_QUALITY}: {}",
            state.options.shader_quality.caption()
        );
        let (x, y) = cell(0.0, Y_QUALITY_ROW_4);
        let mut quality = Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, &label);
        quality.active = enabled;
        if quality.draw(p, ctx) {
            state.options.shader_quality = state.options.shader_quality.next();
            crate::gui::save_options(&state.options);
        }

        let label = format!(
            "{PLAYER_SHADOWS}: {}",
            if state.options.player_shadows {
                ON
            } else {
                OFF
            }
        );
        let (x, y) = cell(1.0, Y_QUALITY_ROW_4);
        let mut shadows = Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, &label);
        shadows.active = enabled;
        if shadows.draw(p, ctx) {
            state.options.player_shadows = !state.options.player_shadows;
            crate::gui::save_options(&state.options);
        }
    }

    #[cfg(feature = "shader_support")]
    {
        let (x, y) = cell(0.0, Y_SHADERPACKS_ROW);
        let packs = Button::new(
            x,
            y,
            BIG_W,
            WIDGET_HEIGHT,
            crate::gui::shaderpacks::SHADER_PACKS,
        );
        #[cfg(feature = "tutorial")]
        crate::gui::tutorial::mark(
            crate::gui::tutorial::Spot::ShaderButton,
            crate::gui::hud_layout::Rect::new(x, y, BIG_W, WIDGET_HEIGHT),
        );
        if packs.draw(p, ctx) {
            state.shaderpacks.refresh();
            state.nav = Some(Screen::ShaderPacks);
        }
    }

    let (x, y) = cell(0.0, Y_RESET_ROW);
    let reset_video = Button::new(x, y, BIG_W, WIDGET_HEIGHT, RESET_VIDEO);
    if reset_video.draw(p, ctx) {
        reset_video_options(&mut state.options);
        crate::gui::save_options(&state.options);
    }

    p.pop_clip(clip);
    draw_scrollbar(
        p,
        state.video_scroll,
        left,
        BIG_W,
        list_y,
        list_h,
        VIDEO_LIST_H,
    );

    if draw_done(p, screen_ctx) {
        state.nav = Some(Screen::Options);
    }
}

enum BindRow {
    Category(&'static str),
    Bind(usize),
}

fn bind_rows() -> Vec<BindRow> {
    let mut rows = Vec::with_capacity(keybinds::BINDS.len() + 8);
    let mut category = "";
    for (i, def) in keybinds::BINDS.iter().enumerate() {
        if def.category != category {
            category = def.category;
            rows.push(BindRow::Category(category));
        }
        rows.push(BindRow::Bind(i));
    }
    rows
}

const BIND_ROW_H: f32 = 20.0;

const BIND_ROW_W: f32 = 340.0;

const CHANGE_W: f32 = 75.0;
const RESET_W: f32 = 50.0;

const BIND_PADDING: f32 = 10.0;

const BIND_BUTTON_GAP: f32 = 5.0;

const COLLISION_COLOR: u32 = 0xFFFF_FF00;
const COLLISION_W: f32 = 3.0;

const RESET: &str = "Reset";
const RESET_ALL_KEYS: &str = "Reset Keys";

const DUPLICATE: &str = "This key is also used for:";

pub fn draw_controls(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    draw_title(p, ctx, CONTROLS_TITLE);

    let captured = state
        .keybinds
        .resolve_capture(ctx.input.pressed_key, ctx.input.pressed_mouse);
    let mut screen_input = ctx.input.clone();
    if captured {
        screen_input.left_click = false;
    }
    let screen_ctx = &ctx.with_input(&screen_input);

    let rows = bind_rows();
    let content_h = rows.len() as f32 * BIND_ROW_H;
    let list_h_total = content_h + LIST_PAD_BOTTOM;

    let left = (ctx.vw / 2.0 - BIND_ROW_W / 2.0).floor();
    let (list_x, list_y, list_w, list_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);

    #[cfg(feature = "mobile_ui")]
    let (list_y, list_h) = {
        let x = centered_x(ctx.vw) + (WIDGET_WIDTH_BIG - COLUMN_W) / 2.0;
        let label = format!(
            "{TOUCH_MOVEMENT}: {}",
            state.options.touch_movement.caption()
        );
        if Button::new(x, list_y, COLUMN_W, WIDGET_HEIGHT, &label).draw(p, ctx) {
            state.options.touch_movement = state.options.touch_movement.next();
            crate::gui::save_options(&state.options);
        }
        let advance = WIDGET_HEIGHT + ROW_SPACING;
        (list_y + advance, (list_h - advance).max(0.0))
    };

    scroll_input(
        &mut state.controls_scroll,
        &mut state.controls_scroll_drag,
        screen_ctx,
        left,
        BIND_ROW_W,
        list_y,
        list_h,
        list_h_total,
    );
    let top = (list_y - state.controls_scroll).floor();

    let list_input = mask_input(
        screen_ctx,
        (list_x, list_y, list_w, list_h),
        state.controls_scroll_drag.is_some(),
        false,
    );
    let list_ctx = &ctx.with_input(&list_input);

    let bar_x = left + BIND_ROW_W + SCROLLBAR_GAP;
    let reset_x = bar_x - RESET_W - BIND_PADDING;
    let change_x = reset_x - BIND_BUTTON_GAP - CHANGE_W;

    focus::suspend();
    let clip = p.push_clip(list_x, list_y, list_w, list_h);
    let mut tooltip: Option<Vec<String>> = None;

    for (i, row) in rows.iter().enumerate() {
        let row_y = top + i as f32 * BIND_ROW_H;
        if !row_visible(row_y, BIND_ROW_H, list_y, list_h) {
            continue;
        }
        match row {
            BindRow::Category(name) => {
                p.text_centered(
                    name,
                    ctx.vw / 2.0,
                    row_y + BIND_ROW_H - LINE_HEIGHT - ROW_INSET,
                    0xFFFFFF,
                    true,
                );
            }
            BindRow::Bind(bind) => {
                let def = &keybinds::BINDS[*bind];
                let collisions = state.keybinds.collisions(*bind);
                let selected = state.keybinds.capturing == Some(*bind);

                p.text_str(def.name, left, row_y + 6.0, 0xFFFFFF, true);

                if !collisions.is_empty() {
                    p.fill(
                        change_x - 6.0,
                        row_y + ROW_INSET - 1.0,
                        COLLISION_W,
                        BIND_ROW_H - 2.0 * ROW_INSET + 1.0,
                        COLLISION_COLOR,
                    );
                }

                let key_name = state.keybinds.row(*bind).display_name();
                let (label, color) = if selected {
                    (format!("> {key_name} <"), 0xFFFF55)
                } else if collisions.is_empty() {
                    (key_name.to_string(), 0xFFFFFF)
                } else {
                    (format!("[ {key_name} ]"), 0xFFFF55)
                };
                let change = Button::new(change_x, row_y, CHANGE_W, BIND_ROW_H, "");
                if change.draw(p, list_ctx) {
                    state.keybinds.capturing = Some(*bind);
                }
                p.text_centered(&label, change_x + CHANGE_W / 2.0, row_y + 6.0, color, true);

                if !collisions.is_empty()
                    && list_ctx.hovering(change_x, row_y, CHANGE_W, BIND_ROW_H)
                {
                    tooltip = Some(vec![DUPLICATE.to_string(), collisions.join(", ")]);
                }

                let reset = Button {
                    x: reset_x,
                    y: row_y,
                    w: RESET_W,
                    h: BIND_ROW_H,
                    label: RESET,
                    active: !state.keybinds.is_default(*bind),
                };
                if reset.draw(p, list_ctx) {
                    state.keybinds.reset(*bind);
                }
            }
        }
    }

    p.pop_clip(clip);
    focus::resume();
    draw_scrollbar(
        p,
        state.controls_scroll,
        left,
        BIND_ROW_W,
        list_y,
        list_h,
        list_h_total,
    );

    if let (Some(lines), Some(m)) = (tooltip, list_ctx.mouse()) {
        draw_bind_tooltip(p, &lines, m.x, m.y, ctx.vw);
    }

    let footer_left = centered_row_x(ctx.vw, 2.0 * FOOTER_W + FOOTER_GAP);
    let footer_y = LAYOUT.footer_y(ctx.vh);
    let reset_all = Button {
        x: footer_left,
        y: footer_y,
        w: FOOTER_W,
        h: WIDGET_HEIGHT,
        label: RESET_ALL_KEYS,
        active: state.keybinds.any_rebound(),
    };
    if reset_all.draw(p, screen_ctx) {
        state.keybinds.reset_all();
    }
    let done = Button::new(
        footer_left + FOOTER_W + FOOTER_GAP,
        footer_y,
        FOOTER_W,
        WIDGET_HEIGHT,
        DONE,
    );
    if done.draw(p, screen_ctx) {
        state.keybinds.capturing = None;
        state.nav = Some(Screen::Options);
    }
}

pub(in crate::gui) fn draw_bind_tooltip(
    p: &mut Painter,
    lines: &[String],
    mx: f32,
    my: f32,
    vw: f32,
) {
    let w = lines
        .iter()
        .map(|l| p.atlas.font.width_str(l))
        .fold(0.0f32, f32::max);
    let h = lines.len() as f32 * LINE_HEIGHT + (lines.len() as f32 - 1.0);
    let (x, y) = crate::gui::tooltip::position_flat(mx, my, vw, w);
    p.fill(x - 3.0, y - 3.0, w + 6.0, h + 6.0, 0xF010_0010);
    for (i, line) in lines.iter().enumerate() {
        p.text_str(line, x, y + i as f32 * (LINE_HEIGHT + 1.0), 0xFFFFFF, true);
    }
}

#[cfg(feature = "asset_download")]
fn draw_clear_assets(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    crate::gui::screens::dim_background(p, ctx.vw, ctx.vh);
    let buttons = [CLEAR_ASSETS_NO.to_owned(), CLEAR_ASSETS_YES.to_owned()];
    let chosen = crate::gui::confirmlink::draw_box(
        p,
        ctx,
        CLEAR_ASSETS_TITLE,
        CLEAR_ASSETS_MESSAGE,
        Some(CLEAR_ASSETS_WARNING),
        &buttons,
        COLUMN_W,
    );
    match chosen {
        Some(1) => {
            #[cfg(target_arch = "wasm32")]
            if let Err(e) = crate::client::assets::clear_web_cache() {
                crate::log_warn!("assets", "could not clear the stored archive: {e}");
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Err(e) = crate::client::assets::clear_native_cache() {
                    crate::log_warn!("assets", "the cached assets were not all cleared: {e}");
                }
                state.quit = true;
            }
            state.confirm_clear_assets = false;
        }
        Some(_) => state.confirm_clear_assets = false,
        None => {}
    }
}

pub fn draw_game(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    #[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
    if state.confirm_clear_assets {
        draw_clear_assets(p, state, ctx);
        return;
    }

    draw_title(p, ctx, GAME_TITLE);

    let left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();
    let (list_x, list_y, list_w, list_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    scroll_input(
        &mut state.game_scroll,
        &mut state.game_scroll_drag,
        ctx,
        left,
        BIG_W,
        list_y,
        list_h,
        GAME_LIST_H,
    );
    let top = (list_y - state.game_scroll).floor();

    let list_input = masked_input(
        ctx,
        (list_x, list_y, list_w, list_h),
        state.game_scroll_drag.is_some(),
    );
    let list_ctx = &ctx.with_input(&list_input);
    let screen_ctx = ctx;
    let ctx = list_ctx;
    let clip = p.push_clip(list_x, list_y, list_w, list_h);

    let cell = |column: f32, row_y: f32| (left + column * COLUMN_STEP, top + row_y + ROW_INSET);

    let (x, y) = cell(0.0, Y_GAME_ROW_0);
    if toggle(p, ctx, x, y, AUTO_JUMP, &mut state.options.auto_jump) {
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(1.0, Y_GAME_ROW_0);
    let caption = format!(
        "{MAIN_HAND}: {}",
        if state.options.main_hand_left {
            MAIN_HAND_LEFT
        } else {
            MAIN_HAND_RIGHT
        }
    );
    if Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, &caption).draw(p, ctx) {
        state.options.main_hand_left = !state.options.main_hand_left;
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(0.0, Y_GAME_ROW_1);
    let caption = format!(
        "{SPRINT_MODE}: {}",
        if state.options.toggle_sprint {
            SPRINT_MODE_TOGGLE
        } else {
            SPRINT_MODE_HOLD
        }
    );
    if Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, &caption).draw(p, ctx) {
        state.options.toggle_sprint = !state.options.toggle_sprint;
        crate::gui::save_options(&state.options);
    }

    p.text_str(
        CHAT_HEADER,
        left,
        top + Y_CHAT_HEADER + ROW_INSET + HEADER_PAD_TOP,
        0xFFFFFF,
        true,
    );

    let (x, y) = cell(0.0, Y_CHAT_ROW_0);
    let can_sign = signing_available(state);
    if can_sign {
        if toggle(
            p,
            ctx,
            x,
            y,
            SIGNED_CHAT,
            &mut state.options.allow_signed_chat,
        ) {
            crate::gui::save_options(&state.options);
        }
    } else {
        let caption = format!("{SIGNED_CHAT}: {OFF}");
        Button {
            active: false,
            ..Button::new(x, y, COLUMN_W, WIDGET_HEIGHT, &caption)
        }
        .draw(p, ctx);
        p.text_str(
            SIGNED_CHAT_UNAVAILABLE,
            x + COLUMN_STEP,
            y + (WIDGET_HEIGHT - LINE_HEIGHT) / 2.0,
            0xA0A0A0,
            true,
        );
    }

    p.text_str(
        TOAST_HEADER,
        left,
        top + Y_TOAST_HEADER + ROW_INSET + HEADER_PAD_TOP,
        0xFFFFFF,
        true,
    );

    let (x, y) = cell(0.0, Y_TOAST_ROW_0);
    if toggle(
        p,
        ctx,
        x,
        y,
        TOAST_ADVANCEMENT,
        &mut state.options.toast_advancement,
    ) {
        crate::gui::save_options(&state.options);
    }
    let (x, y) = cell(1.0, Y_TOAST_ROW_0);
    if toggle(p, ctx, x, y, TOAST_RECIPE, &mut state.options.toast_recipe) {
        crate::gui::save_options(&state.options);
    }
    let (x, y) = cell(0.0, Y_TOAST_ROW_1);
    if toggle(p, ctx, x, y, TOAST_SYSTEM, &mut state.options.toast_system) {
        crate::gui::save_options(&state.options);
    }

    let (x, y) = cell(1.0, Y_TOAST_ROW_1);
    let toast_time = Slider {
        id: TOAST_TIME_SLIDER_ID,
        x,
        y,
        w: COLUMN_W,
        h: WIDGET_HEIGHT,
        min: crate::gui::TOAST_TIME_MIN,
        max: crate::gui::TOAST_TIME_MAX,
        value: state.options.toast_time as i32,
        label: toast_time_label,
    };
    let new_toast_time = toast_time
        .draw(p, ctx)
        .clamp(crate::gui::TOAST_TIME_MIN, crate::gui::TOAST_TIME_MAX)
        as u32;
    if new_toast_time != state.options.toast_time {
        state.options.toast_time = new_toast_time;
        crate::gui::save_options(&state.options);
    }

    #[cfg(feature = "skins")]
    {
        p.text_str(
            SKIN_HEADER,
            left,
            top + Y_SKIN_HEADER + ROW_INSET + HEADER_PAD_TOP,
            0xFFFFFF,
            true,
        );
        let rows = [Y_SKIN_ROW_0, Y_SKIN_ROW_1, Y_SKIN_ROW_2, Y_SKIN_ROW_3];
        for (i, (label, bit)) in SKIN_PART_LABELS
            .iter()
            .zip(crate::gui::MODEL_PART_KEYS.map(|(_, bit)| bit))
            .enumerate()
        {
            let (x, y) = cell((i % 2) as f32, rows[i / 2]);
            let mut on = state.options.skin_parts & (1 << bit) != 0;
            if toggle(p, ctx, x, y, label, &mut on) {
                state.options.skin_parts ^= 1 << bit;
                crate::gui::save_options(&state.options);
            }
        }
    }

    #[cfg(all(target_arch = "wasm32", feature = "asset_download"))]
    {
        p.text_str(
            STORAGE_HEADER,
            left,
            top + Y_STORAGE_HEADER + ROW_INSET + HEADER_PAD_TOP,
            0xFFFFFF,
            true,
        );
        let (x, y) = cell(0.0, Y_STORAGE_ROW_0);
        if Button::new(x, y, BIG_W, WIDGET_HEIGHT, CLEAR_ASSETS).draw(p, ctx) {
            state.confirm_clear_assets = true;
        }
    }

    let (x, y) = cell(0.0, Y_GAME_RESET_ROW);
    if Button::new(x, y, BIG_W, WIDGET_HEIGHT, RESET_GAME).draw(p, ctx) {
        reset_game_options(&mut state.options);
        crate::gui::save_options(&state.options);
    }

    p.pop_clip(clip);
    draw_scrollbar(
        p,
        state.game_scroll,
        left,
        BIG_W,
        list_y,
        list_h,
        GAME_LIST_H,
    );

    if draw_done(p, screen_ctx) {
        state.nav = Some(Screen::Options);
    }
}

#[cfg(feature = "audio")]
pub fn draw_audio(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    use crate::audio::category::SoundCategory;

    draw_title(p, ctx, AUDIO_TITLE);

    let left = (ctx.vw / 2.0 - GRID_LEFT_OFFSET).floor();
    let top = LAYOUT.content_y(ctx.vh, AUDIO_CONTENT_H);
    let cell = |column: f32, row_y: f32| (left + column * COLUMN_STEP, top + row_y + ROW_INSET);

    let master = SoundCategory::Master;
    let (x, y) = (left, top + Y_AUDIO_MASTER + ROW_INSET);
    let master_slider = Slider {
        id: VOLUME_SLIDER_ID_BASE + master.index() as u32,
        x,
        y,
        w: BIG_W,
        h: WIDGET_HEIGHT,
        min: 0,
        max: 100,
        value: state.options.volumes[master.index()] as i32,
        label: |v| percent_or_off(master.label(), v),
    };
    let new_master = master_slider.draw(p, ctx).clamp(0, 100) as u32;
    if new_master != state.options.volumes[master.index()] {
        state.options.volumes[master.index()] = new_master;
        crate::gui::save_options(&state.options);
        crate::audio::set_volumes(&state.options.volumes);
    }

    let rows = [
        Y_AUDIO_ROW_0,
        Y_AUDIO_ROW_1,
        Y_AUDIO_ROW_2,
        Y_AUDIO_ROW_3,
        Y_AUDIO_ROW_4,
    ];
    for (i, category) in SoundCategory::ALL
        .into_iter()
        .filter(|c| *c != SoundCategory::Master)
        .enumerate()
    {
        let row_y = rows[i / 2];
        let column = (i % 2) as f32;
        let (x, y) = cell(column, row_y);
        let slider = Slider {
            id: VOLUME_SLIDER_ID_BASE + category.index() as u32,
            x,
            y,
            w: COLUMN_W,
            h: WIDGET_HEIGHT,
            min: 0,
            max: 100,
            value: state.options.volumes[category.index()] as i32,
            label: |v| percent_or_off(category.label(), v),
        };
        let new_value = slider.draw(p, ctx).clamp(0, 100) as u32;
        if new_value != state.options.volumes[category.index()] {
            state.options.volumes[category.index()] = new_value;
            crate::gui::save_options(&state.options);
            crate::audio::set_volumes(&state.options.volumes);
        }
    }

    if draw_done(p, ctx) {
        state.nav = Some(Screen::Options);
    }
}

fn signing_available(state: &mut GuiState) -> bool {
    #[cfg(feature = "online_mode")]
    {
        use crate::gui::accountlist::AccountKind;
        state.profile.ensure_loaded();
        state
            .profile
            .accounts
            .active()
            .is_some_and(|a| a.kind == AccountKind::Microsoft)
    }
    #[cfg(not(feature = "online_mode"))]
    {
        let _ = state;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::GuiInput;
    use crate::gui::render::effective_gui_scale;
    use crate::gui::widgets::{slider_track_cursor, slider_value_from_mouse};

    const WIN_W: f32 = 1280.0;
    const WIN_H: f32 = 720.0;

    const CURSOR_PX: f32 = 520.0;

    fn feedback_step(option: u32) -> (f32, f32, i32) {
        let scale = effective_gui_scale(WIN_W, WIN_H, option);
        let x = centered_x(WIN_W / scale);
        let mouse_x = CURSOR_PX / scale;
        let max = auto_gui_scale(WIN_W, WIN_H) as i32;
        (
            scale,
            mouse_x,
            slider_value_from_mouse(mouse_x, x, WIDGET_WIDTH_BIG, 0, max),
        )
    }

    #[test]
    fn the_scale_feedback_loop_oscillates() {
        let mut option = 0u32;
        let mut seen = Vec::new();
        for _ in 0..8 {
            option = feedback_step(option).2.max(0) as u32;
            seen.push(option);
        }
        assert_eq!(seen, vec![1, 0, 1, 0, 1, 0, 1, 0]);
    }

    #[test]
    #[cfg(not(feature = "mobile_ui"))]
    fn a_scale_change_ends_the_drag() {
        const ID: u32 = GUI_SCALE_SLIDER_ID;
        let mut option = 0u32;
        let mut values = Vec::new();
        let mut scales = Vec::new();
        for frame in 0..8 {
            let (scale, mouse_x, from_mouse) = feedback_step(option);
            let x = centered_x(WIN_W / scale);
            let hovered = mouse_x >= x && mouse_x < x + WIDGET_WIDTH_BIG;
            let input = GuiInput {
                scale,
                left_click: frame == 0,
                left_down: true,
                ..Default::default()
            };
            if slider_track_cursor(ID, scale, hovered, &input) {
                option = from_mouse.max(0) as u32;
            }
            values.push(option);
            scales.push(scale);
        }
        slider_track_cursor(
            ID,
            1.0,
            false,
            &GuiInput {
                left_release: true,
                ..Default::default()
            },
        );

        assert_eq!(values, vec![1; 8]);
        assert_eq!(scales, vec![3.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn layout_follows_header_and_footer_layout() {
        assert_eq!(LAYOUT.title_y(), 12.0);
        assert_eq!(LAYOUT.content_y(480.0, WIDGET_HEIGHT), 63.0);
        assert_eq!(LAYOUT.footer_y(480.0), 454.0);
        assert_eq!(centered_x(854.0), 327.0);
    }

    #[test]
    fn short_window_pulls_the_content_up() {
        assert_eq!(LAYOUT.content_y(100.0, WIDGET_HEIGHT), 47.0);
    }

    #[test]
    fn video_grid_stacks_like_the_options_list() {
        assert_eq!(Y_DISPLAY_ROW_0, 13.0);
        assert_eq!(Y_DISPLAY_ROW_1, 38.0);
        assert_eq!(Y_DISPLAY_ROW_2, 63.0);
        assert_eq!(Y_DISPLAY_ROW_3, 88.0);
        const LOD_GROUP: f32 = if cfg!(feature = "render_lod") {
            31.0 + 23.0 + 3.0 * ROW_H
        } else {
            0.0
        };
        #[cfg(feature = "render_lod")]
        {
            assert_eq!(Y_LOD_HEADER, 113.0);
            assert_eq!(Y_LOD_HINT, 144.0);
            assert_eq!(Y_LOD_ROW_0, 167.0);
            assert_eq!(Y_LOD_ROW_1, 192.0);
            assert_eq!(Y_LOD_ROW_2, 217.0);
        }
        assert_eq!(Y_QUALITY_HEADER, 113.0 + LOD_GROUP);
        assert_eq!(Y_QUALITY_ROW_0, 144.0 + LOD_GROUP);
        const EXTRA_ROWS: f32 = cfg!(feature = "builtin_shaders") as u32 as f32
            + cfg!(feature = "shader_support") as u32 as f32;
        assert_eq!(Y_RESET_ROW, 244.0 + EXTRA_ROWS * ROW_H + LOD_GROUP);
        assert_eq!(VIDEO_CONTENT_H, Y_RESET_ROW + ROW_H);
    }

    #[test]
    fn scroller_height_follows_the_scroll_area() {
        assert_eq!(scroller_height(200.0, 400.0), 100.0);
        assert_eq!(scroller_height(200.0, 4000.0), SCROLLBAR_MIN_H);
        assert_eq!(scroller_height(200.0, 205.0), 192.0);
        assert_eq!(scroller_height(20.0, 400.0), SCROLLBAR_MIN_H);
    }

    #[test]
    fn scroller_travels_the_whole_track() {
        let (list_y, list_h, content_h) = (33.0, 200.0, 400.0);
        let max_scroll = content_h - list_h;
        let h = scroller_height(list_h, content_h);
        assert_eq!(scroller_y(0.0, max_scroll, list_y, list_h, h), list_y);
        assert_eq!(
            scroller_y(max_scroll, max_scroll, list_y, list_h, h),
            list_y + list_h - h
        );
    }

    #[test]
    #[cfg(not(all(
        feature = "render_lod",
        any(feature = "builtin_shaders", feature = "shader_support")
    )))]
    fn a_tall_window_has_nothing_to_scroll() {
        let (_, _, _, list_h) = LAYOUT.content_rect(854.0, 480.0);
        assert_eq!(list_h, 414.0);
        assert!(VIDEO_LIST_H < list_h);
    }

    #[test]
    fn video_columns_span_the_big_row() {
        let left = (854.0f32 / 2.0 - GRID_LEFT_OFFSET).floor();
        assert_eq!(left, 272.0);
        assert_eq!(left + COLUMN_STEP + COLUMN_W, left + BIG_W);
    }

    #[test]
    fn fov_label_matches_the_option_stringifier() {
        assert_eq!(fov_label(70), "FOV: Normal");
        assert_eq!(fov_label(110), "FOV: Quake Pro");
        assert_eq!(fov_label(90), "FOV: 90");
        assert_eq!(fov_label(30), "FOV: 30");
    }

    #[test]
    fn label_matches_the_option_stringifier() {
        assert_eq!(gui_scale_label(0), "GUI Scale: Auto");
        assert_eq!(gui_scale_label(3), "GUI Scale: 3");
    }

    #[test]
    fn max_framerate_label_matches_vanilla() {
        assert_eq!(max_framerate_label(10), "Max Framerate: 10 fps");
        assert_eq!(max_framerate_label(144), "Max Framerate: 144 fps");
        assert_eq!(max_framerate_label(260), "Max Framerate: Unlimited");
    }

    #[test]
    fn render_distance_label_matches_the_option_stringifier() {
        assert_eq!(
            render_distance_label(RENDER_DISTANCE_MIN),
            "Render Distance: 2 Chunks"
        );
        assert_eq!(render_distance_label(12), "Render Distance: 12 Chunks");
        assert_eq!(
            render_distance_label(RENDER_DISTANCE_MAX),
            "Render Distance: 32 Chunks"
        );
    }

    #[test]
    fn a_zero_percentage_reads_as_off() {
        assert_eq!(percent_or_off("Master Volume", 0), "Master Volume: OFF");
        assert_eq!(percent_or_off("Music", 55), "Music: 55%");
        assert_eq!(percent_or_off("Voice", 100), "Voice: 100%");
    }

    #[cfg(feature = "audio")]
    #[test]
    fn saved_volume_keys_match_vanillas_sound_category_names() {
        use crate::audio::category::SoundCategory;
        assert_eq!(
            format!("soundCategory_{}", SoundCategory::Master.key()),
            "soundCategory_master"
        );
        assert_eq!(
            format!("soundCategory_{}", SoundCategory::Records.key()),
            "soundCategory_record"
        );
        assert_eq!(
            format!("soundCategory_{}", SoundCategory::Voice.key()),
            "soundCategory_voice"
        );
    }

    #[cfg(feature = "audio")]
    #[test]
    fn a_saved_volume_percentage_round_trips_through_json() {
        use crate::audio::category::SoundCategory;
        use crate::gui::VOLUME_DEFAULT;

        let key = format!("soundCategory_{}", SoundCategory::Music.key());
        let json = serde_json::json!({ key.clone(): 63u32 });
        let value = |k: &str| json.get(k).cloned();

        let read_back = value(&key)
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(VOLUME_DEFAULT)
            .min(100);
        assert_eq!(read_back, 63);

        let missing = value("soundCategory_weather")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(VOLUME_DEFAULT)
            .min(100);
        assert_eq!(missing, VOLUME_DEFAULT);
    }
}
