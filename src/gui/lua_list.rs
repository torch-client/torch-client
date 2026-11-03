use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use crate::gui::clickgui::Look;
use crate::gui::painter::{CORNERS_TOP, Painter};
use crate::gui::widgets::TextBox;
use crate::gui::{GuiState, Screen, ScreenCtx, options};
use crate::modules::script::{self, FileAsk, OverPrompt, UiButton, UiForm, UiList};
use crate::platform::packfs::{self, Kind};

const PANEL_MAX_W: f32 = 260.0;
const MARGIN: f32 = 16.0;
const PAD: f32 = 5.0;
const ROW_PAD: f32 = 3.0;
const HEAD_H: f32 = 16.0;
const ROW_H: f32 = 12.0;
const ICON_ROW_H: f32 = 18.0;
const ICON: f32 = 16.0;
const BUTTON_PAD: f32 = 3.0;
const BUTTON_GAP: f32 = 2.0;
const FIELD_H: f32 = 20.0;
const PROMPT_MAX: usize = 256;
const FOOT_H: f32 = 13.0;
const DETAIL_GAP: f32 = 8.0;
const GLYPH_H: f32 = 8.0;
const ELLIPSIS: &str = "...";

#[derive(Default)]
pub struct LuaListState {
    list: Option<Arc<UiList>>,
    scroll: f32,
    drag: Option<f32>,
    field: TextBox,
    field_for: u64,
    file_for: u64,
    file_error: Option<String>,
    canvas_press: Option<CanvasPress>,
}

static DROPPED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub(crate) fn collect_drops(
    mut drops: bevy::prelude::MessageReader<bevy::window::FileDragAndDrop>,
    state: bevy::prelude::Res<GuiState>,
) {
    let on_card = state.screen == Screen::LuaList
        && state
            .lua_list
            .list
            .as_ref()
            .is_some_and(|l| matches!(l.form, UiForm::File(_)));
    for drop in drops.read() {
        if let bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } = drop
            && on_card
        {
            DROPPED
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(path_buf.clone());
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        use crate::platform::upload::{self, Taker};
        upload::accept(Taker::ModFile, on_card);
        DROPPED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(upload::take(Taker::ModFile));
    }
}

fn read_dropped(paths: &[PathBuf]) -> Result<(String, Vec<u8>), String> {
    let path = &paths[0];
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if packfs::kind(path) != Some(Kind::File) {
        return Err(format!("{name} is not a file"));
    }
    if packfs::len(path).unwrap_or(0) > script::FILE_MAX {
        return Err(format!("{name} is over {} MiB", script::FILE_MAX >> 20));
    }
    packfs::read(path)
        .map(|bytes| (name.clone(), bytes))
        .map_err(|e| format!("could not read {name}: {e}"))
}

impl LuaListState {
    pub fn close(&mut self) {
        self.drag = None;
        self.canvas_press = None;
        if let Some(list) = self.list.take() {
            script::ui_closed(list.serial);
        }
    }

    pub fn windowed(&self) -> bool {
        self.list
            .as_ref()
            .is_some_and(|l| matches!(l.form, UiForm::Rows | UiForm::File(_)))
    }

    pub fn escape(&mut self) -> bool {
        let Some(list) = self
            .list
            .as_ref()
            .filter(|l| matches!(l.form, UiForm::Canvas(Some(_))))
        else {
            return false;
        };
        script::ui_prompt(list.serial, None);
        release_field(self);
        true
    }
}

pub fn poll(state: &mut GuiState) {
    let want = script::ui();
    let free = state.screen == Screen::None || over_click_gui(state);
    let st = &mut state.lua_list;
    if state.screen == Screen::LuaList {
        match want {
            None => {
                st.list = None;
                if state.nav.is_none() {
                    state.nav = Some(Screen::None);
                }
            }
            Some(list) if st.list.as_ref().is_none_or(|l| l.serial != list.serial) => {
                st.list = Some(list);
            }
            Some(_) => {}
        }
    } else if let Some(list) = want
        && free
        && state.nav.is_none()
    {
        st.list = Some(list);
        st.scroll = 0.0;
        st.drag = None;
        state.nav = Some(Screen::LuaList);
    }
}

fn over_click_gui(state: &GuiState) -> bool {
    #[cfg(feature = "click_gui")]
    {
        state.screen == Screen::ClickGui && !state.clickgui_parent.is_menu()
    }
    #[cfg(not(feature = "click_gui"))]
    {
        let _ = state;
        false
    }
}

fn fit<'a>(p: &Painter, s: &'a str, w: f32) -> Cow<'a, str> {
    let font = &p.atlas.font;
    if font.width_str(s) <= w {
        return Cow::Borrowed(s);
    }
    let room = w - font.width_str(ELLIPSIS);
    let mut used = 0.0;
    let mut end = 0;
    for (i, c) in s.char_indices() {
        used += font.advance(c, false);
        if used > room {
            break;
        }
        end = i + c.len_utf8();
    }
    Cow::Owned(format!("{}{ELLIPSIS}", &s[..end]))
}

#[allow(clippy::too_many_arguments)]
fn draw_prompt(
    p: &mut Painter,
    st: &mut LuaListState,
    look: Look,
    ctx: &ScreenCtx,
    serial: u64,
    title: &str,
    default: &str,
    cancel: bool,
) -> Option<Option<String>> {
    if st.field_for != serial {
        st.field = TextBox::bordered(PROMPT_MAX, "");
        st.field.set_text(default);
        st.field.focus();
        st.field_for = serial;
    }
    let panel_w = PANEL_MAX_W.min(ctx.vw - 2.0 * MARGIN).floor();
    let panel_h = HEAD_H + 3.0 * PAD + FIELD_H + ROW_H + 2.0;
    let x = ((ctx.vw - panel_w) / 2.0).floor();
    let y = ((ctx.vh - panel_h) / 2.0).floor();
    p.rounded_rect(x, y, panel_w, panel_h, look.radius, look.panel());
    p.rounded_rect_corners(
        x,
        y,
        panel_w,
        HEAD_H,
        look.radius,
        CORNERS_TOP,
        look.header(),
    );
    p.fill(x, y + HEAD_H - 1.0, panel_w, 1.0, look.accent());
    let title = fit(p, title, panel_w - 2.0 * PAD);
    p.text_centered(
        &title,
        x + panel_w / 2.0,
        y + ((HEAD_H - GLYPH_H) / 2.0).floor(),
        look.text_title(),
        look.shadow,
    );

    let fy = y + HEAD_H + PAD;
    st.field
        .update(p, ctx, x + PAD, fy, panel_w - 2.0 * PAD, FIELD_H);
    let by = fy + FIELD_H + PAD;
    let right = x + panel_w - PAD;
    let (done_w, done) = card_button(p, look, ctx, right, by, "Done");
    if ctx.input.enter || done {
        return Some(Some(st.field.text.clone()));
    }
    if cancel && card_button(p, look, ctx, right - done_w - BUTTON_GAP, by, "Cancel").1 {
        return Some(None);
    }
    None
}

fn release_field(st: &mut LuaListState) {
    st.field.focused = false;
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    st.field.sync_keyboard();
}

fn card_button(
    p: &mut Painter,
    look: Look,
    ctx: &ScreenCtx,
    right: f32,
    y: f32,
    label: &str,
) -> (f32, bool) {
    let bw = (p.atlas.font.width_str(label) + 4.0 * BUTTON_PAD).floor();
    let bx = right - bw;
    let bh = ROW_H + 2.0;
    let hover = ctx.hovering(bx, y, bw, bh);
    p.fill(
        bx,
        y,
        bw,
        bh,
        if hover {
            look.row_hover()
        } else {
            look.header()
        },
    );
    p.text_centered(
        label,
        bx + bw / 2.0,
        y + ((bh - GLYPH_H) / 2.0).floor(),
        look.text_title(),
        look.shadow,
    );
    (bw, hover && ctx.input.left_click)
}

const ERROR: u32 = 0xFF_55_55;

fn draw_file(
    p: &mut Painter,
    st: &mut LuaListState,
    look: Look,
    ctx: &ScreenCtx,
    list: &UiList,
    ask: &FileAsk,
) {
    if st.file_for != list.serial {
        st.file_for = list.serial;
        st.file_error = None;
        #[cfg(target_arch = "wasm32")]
        let _ = crate::platform::upload::take_mod_file();
    }

    let dropped = std::mem::take(&mut *DROPPED.lock().unwrap_or_else(PoisonError::into_inner));
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
    let mut file = (!dropped.is_empty()).then(|| read_dropped(&dropped));
    #[cfg(target_arch = "wasm32")]
    {
        crate::platform::upload::discard(&dropped);
        if let Some(chosen) = crate::platform::upload::take_mod_file() {
            file = Some(Ok(chosen));
        }
    }
    match file {
        Some(Ok((name, bytes))) => match script::ui_file(list.serial, &name, &bytes) {
            Ok(()) => {
                st.list = None;
                return;
            }
            Err(why) => st.file_error = Some(why),
        },
        Some(Err(why)) => st.file_error = Some(why),
        None => {}
    }

    let lines = [
        (format!("{} asks for a file", ask.mod_id), look.text_dim()),
        (
            if ask.accept.is_empty() {
                "Any file".to_string()
            } else {
                format!("Takes {}", ask.accept.join(", "))
            },
            look.text_dim(),
        ),
        match &st.file_error {
            Some(why) => (why.clone(), ERROR),
            #[cfg(target_arch = "wasm32")]
            None => (
                "Choose one, or drop it on the page".to_string(),
                look.text_dim(),
            ),
            #[cfg(not(target_arch = "wasm32"))]
            None => ("Drop it on this window".to_string(), look.text_dim()),
        },
    ];
    let panel_w = PANEL_MAX_W.min(ctx.vw - 2.0 * MARGIN).floor();
    let panel_h = HEAD_H + 3.0 * PAD + lines.len() as f32 * ROW_H + ROW_H + 2.0;
    let x = ((ctx.vw - panel_w) / 2.0).floor();
    let y = ((ctx.vh - panel_h) / 2.0).floor();
    p.rounded_rect(x, y, panel_w, panel_h, look.radius, look.panel());
    p.rounded_rect_corners(
        x,
        y,
        panel_w,
        HEAD_H,
        look.radius,
        CORNERS_TOP,
        look.header(),
    );
    p.fill(x, y + HEAD_H - 1.0, panel_w, 1.0, look.accent());
    let title = fit(p, &list.title, panel_w - 2.0 * PAD);
    p.text_centered(
        &title,
        x + panel_w / 2.0,
        y + ((HEAD_H - GLYPH_H) / 2.0).floor(),
        look.text_title(),
        look.shadow,
    );

    let mut ly = y + HEAD_H + PAD;
    for (text, rgb) in &lines {
        let text = fit(p, text, panel_w - 2.0 * PAD);
        p.text_plain(
            &text,
            x + PAD,
            ly + ((ROW_H - GLYPH_H) / 2.0).floor(),
            *rgb,
            look.shadow,
        );
        ly += ROW_H;
    }
    let by = ly + PAD;
    let right = x + panel_w - PAD;
    let (cancel_w, cancel) = card_button(p, look, ctx, right, by, "Cancel");
    #[cfg(target_arch = "wasm32")]
    if card_button(
        p,
        look,
        ctx,
        right - cancel_w - BUTTON_GAP,
        by,
        "Choose file",
    )
    .1
    {
        crate::platform::upload::choose_mod_file(&ask.accept);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = cancel_w;
    if cancel {
        st.list = None;
        script::ui_closed(list.serial);
    }
}

const BACKDROP: u32 = 0xFF_10_12_14;
const DRAG_SLOP: f32 = 3.0;
const ZOOM_STEP: f64 = 1.25;

#[derive(Clone, Copy)]
pub struct CanvasPress {
    start: bevy::math::Vec2,
    last: bevy::math::Vec2,
    drag: bool,
}

fn draw_canvas(
    p: &mut Painter,
    st: &mut LuaListState,
    look: Look,
    ctx: &ScreenCtx,
    list: &UiList,
    over: Option<&OverPrompt>,
) {
    let input = ctx.input;
    let mut clicks: Vec<(UiButton, bevy::math::Vec2)> = Vec::new();
    if over.is_some() {
        st.canvas_press = None;
    }
    let cam = {
        let mut cam = script::CANVAS_CAMERA
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        cam.vw = ctx.vw;
        cam.vh = ctx.vh;
        cam.mouse = ctx.mouse().map(|m| [m.x, m.y]);
        if let Some(m) = ctx.mouse().filter(|_| over.is_none()) {
            if input.left_click {
                st.canvas_press = Some(CanvasPress {
                    start: m,
                    last: m,
                    drag: false,
                });
            }
            if let Some(press) = &mut st.canvas_press
                && input.left_down
            {
                press.drag |= press.start.distance(m) > DRAG_SLOP;
                if press.drag {
                    cam.x -= f64::from(m.x - press.last.x) / cam.scale;
                    cam.y -= f64::from(m.y - press.last.y) / cam.scale;
                }
                press.last = m;
            }
            if input.right_click {
                clicks.push((UiButton::Right, m));
            }
            if input.middle_click {
                clicks.push((UiButton::Middle, m));
            }
            if input.scroll != 0.0 {
                let before = cam.canvas_at(m.x, m.y);
                let zoomed = cam.scale * ZOOM_STEP.powf(f64::from(input.scroll));
                cam.scale =
                    zoomed.clamp(*script::Camera::SCALE.start(), *script::Camera::SCALE.end());
                let after = cam.canvas_at(m.x, m.y);
                cam.x += before[0] - after[0];
                cam.y += before[1] - after[1];
            }
        }
        if input.left_release || !input.left_down {
            if let Some(press) = st.canvas_press.take()
                && !press.drag
            {
                clicks.push((UiButton::Left, press.start));
            }
        }
        *cam
    };
    for (button, at) in clicks {
        script::ui_canvas_click(list.serial, button, at.x, at.y);
    }

    p.fill(0.0, 0.0, ctx.vw, ctx.vh, BACKDROP);
    script::draw_canvas(p, cam, ctx.vw, ctx.vh);
    script::draw_canvas_overlay(p);
    p.fill(0.0, 0.0, ctx.vw, HEAD_H, look.header());
    p.fill(0.0, HEAD_H - 1.0, ctx.vw, 1.0, look.accent());
    let title = fit(p, &list.title, ctx.vw - 2.0 * PAD);
    p.text_centered(
        &title,
        ctx.vw / 2.0,
        ((HEAD_H - GLYPH_H) / 2.0).floor(),
        look.text_title(),
        look.shadow,
    );

    if let Some(over) = over
        && let Some(answer) = draw_prompt(
            p,
            st,
            look,
            ctx,
            list.serial,
            &over.title,
            &over.default,
            true,
        )
    {
        script::ui_prompt(list.serial, answer);
        release_field(st);
    }
}

fn buttons_w(p: &Painter, labels: &[String]) -> f32 {
    labels
        .iter()
        .map(|l| (p.atlas.font.width_str(l) + 2.0 * BUTTON_PAD).floor() + BUTTON_GAP)
        .sum()
}

pub fn draw(p: &mut Painter, st: &mut LuaListState, look: Look, ctx: &ScreenCtx) {
    let Some(list) = st.list.clone() else {
        return;
    };
    match &list.form {
        UiForm::Rows => {}
        UiForm::Prompt(default) => {
            if let Some(answer) =
                draw_prompt(p, st, look, ctx, list.serial, &list.title, default, false)
            {
                st.list = None;
                script::ui_prompt(list.serial, answer);
            }
            return;
        }
        UiForm::File(ask) => return draw_file(p, st, look, ctx, &list, ask),
        UiForm::Canvas(over) => return draw_canvas(p, st, look, ctx, &list, over.as_ref()),
    }
    let rows = list.rows.len();
    let row_h = if list.rows.iter().any(|r| r.item.is_some()) {
        ICON_ROW_H
    } else {
        ROW_H
    };
    let content_h = rows as f32 * row_h;
    let foot_h = if list.footer.is_empty() { 0.0 } else { FOOT_H };

    let panel_w = PANEL_MAX_W.min(ctx.vw - 2.0 * MARGIN).floor();
    let room = (ctx.vh - 2.0 * MARGIN - HEAD_H - foot_h - 2.0 * PAD).max(row_h);
    let list_h = content_h.min(room).max(row_h);
    let panel_h = HEAD_H + 2.0 * PAD + list_h + foot_h;
    let x = ((ctx.vw - panel_w) / 2.0).floor();
    let y = ((ctx.vh - panel_h) / 2.0).floor();

    p.rounded_rect(x, y, panel_w, panel_h, look.radius, look.panel());
    p.rounded_rect_corners(
        x,
        y,
        panel_w,
        HEAD_H,
        look.radius,
        CORNERS_TOP,
        look.header(),
    );
    p.fill(x, y + HEAD_H - 1.0, panel_w, 1.0, look.accent());
    let title = fit(p, &list.title, panel_w - 2.0 * PAD);
    p.text_centered(
        &title,
        x + panel_w / 2.0,
        y + ((HEAD_H - GLYPH_H) / 2.0).floor(),
        look.text_title(),
        look.shadow,
    );

    let overflow = content_h > list_h;
    let list_x = x + PAD;
    let list_y = y + HEAD_H + PAD;
    let bar = if overflow {
        options::SCROLLBAR_GAP + options::SCROLLBAR_W
    } else {
        0.0
    };
    let row_w = panel_w - 2.0 * PAD - bar;

    let max_scroll = (content_h - list_h).max(0.0);
    let keys = ctx.input.down_arrow as i32 - ctx.input.up_arrow as i32;
    let pages = ctx.input.page_down as i32 - ctx.input.page_up as i32;
    st.scroll = (st.scroll + keys as f32 * row_h + pages as f32 * list_h).clamp(0.0, max_scroll);
    options::scroll_input_in(
        &mut st.scroll,
        &mut st.drag,
        ctx,
        list_x,
        row_w,
        (x, panel_w),
        list_y,
        list_h,
        content_h,
    );
    let input = options::masked_input(ctx, (list_x, list_y, row_w, list_h), st.drag.is_some());
    let rctx = ctx.with_input(&input);
    let button = if input.left_click {
        Some(UiButton::Left)
    } else if input.right_click {
        Some(UiButton::Right)
    } else if input.middle_click {
        Some(UiButton::Middle)
    } else {
        None
    };

    let clip = p.push_clip(list_x, list_y, row_w, list_h);
    let first = (st.scroll / row_h) as usize;
    let shown = (list_h / row_h).ceil() as usize + 1;
    for (i, row) in list.rows.iter().enumerate().skip(first).take(shown) {
        let ry = (list_y + i as f32 * row_h - st.scroll).floor();
        let mut tx = list_x + ROW_PAD;
        let mut text_w = row_w - 2.0 * ROW_PAD;
        let hovered = rctx.hovering(list_x, ry, row_w, row_h);
        if hovered {
            p.fill(list_x, ry, row_w, row_h, look.row_hover());
        }
        let mut pressed = None;
        if !row.buttons.is_empty() {
            let mut bx = list_x + row_w - ROW_PAD - buttons_w(p, &row.buttons) + BUTTON_GAP;
            let bh = ROW_H - 2.0;
            let by = ry + ((row_h - bh) / 2.0).floor();
            for (b, label) in row.buttons.iter().enumerate() {
                let bw = (p.atlas.font.width_str(label) + 2.0 * BUTTON_PAD).floor();
                let over = rctx.hovering(bx, by, bw, bh);
                p.fill(
                    bx,
                    by,
                    bw,
                    bh,
                    if over { look.accent() } else { look.header() },
                );
                p.text_plain(
                    label,
                    bx + BUTTON_PAD,
                    by + ((bh - GLYPH_H) / 2.0).floor(),
                    look.text_title(),
                    look.shadow,
                );
                if over {
                    pressed = Some(b);
                }
                bx += bw + BUTTON_GAP;
            }
            text_w -= buttons_w(p, &row.buttons) + DETAIL_GAP;
        }
        if hovered && let Some(b) = button {
            script::ui_click(list.serial, i, b, pressed);
        }
        if let Some(id) = row.item {
            p.item_icon(id, tx, ry + ((row_h - ICON) / 2.0).floor());
            tx += ICON + ROW_PAD;
            text_w -= ICON + ROW_PAD;
        }
        let ty = ry + ((row_h - GLYPH_H) / 2.0).floor();
        let mut left_w = text_w;
        if !row.detail.is_empty() {
            let detail = fit(p, &row.detail, text_w / 2.0);
            let dw = p.atlas.font.width_str(&detail);
            p.text_plain(&detail, tx + text_w - dw, ty, look.text_dim(), look.shadow);
            left_w = text_w - dw - DETAIL_GAP;
        }
        let text = fit(p, &row.text, left_w.max(0.0));
        p.text_plain(&text, tx, ty, row.rgb & 0xFF_FFFF, look.shadow);
    }
    p.pop_clip(clip);
    options::draw_scrollbar(p, st.scroll, list_x, row_w, list_y, list_h, content_h);

    if foot_h > 0.0 {
        let footer = fit(p, &list.footer, panel_w - 2.0 * PAD);
        let fy = y + panel_h - foot_h + ((foot_h - GLYPH_H) / 2.0).floor() - 1.0;
        p.text_centered(&footer, x + panel_w / 2.0, fy, look.text_dim(), look.shadow);
    }
}
