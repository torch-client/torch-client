use std::path::Path;

use crate::gui::atlas::SERVER_ICON_PX;
use crate::gui::confirmlink;
use crate::gui::focus;
use crate::gui::menu;
use crate::gui::multiplayer::{CONTENT_PAD, arrow_select};
use crate::gui::options::{draw_scrollbar, row_visible, scroll_masked};
use crate::gui::painter::Painter;
use crate::gui::ping::{self, IconPixels};
use crate::gui::tooltip::{self, label};
use crate::gui::widgets::{Button, HeaderFooter, TextBox, WIDGET_HEIGHT, centered_row_x};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::singleplayer::backed::{Control, is_backed};
use crate::singleplayer::backend;
use crate::singleplayer::store::{self, GameMode, WorldSummary};
use crate::text::{LINE_HEIGHT, Span, contains_ci};

const HEADER_H: f32 = 8.0 + 9.0 + 8.0 + 20.0 + 4.0;
const FOOTER_H: f32 = 60.0;
const LAYOUT: HeaderFooter = HeaderFooter::new(HEADER_H, FOOTER_H);
const HEADER_GAP: f32 = 4.0;
const TITLE_Y: f32 = (HEADER_H - (LINE_HEIGHT + HEADER_GAP + WIDGET_HEIGHT)) / 2.0;
const SEARCH_Y: f32 = TITLE_Y + LINE_HEIGHT + HEADER_GAP;
const SEARCH_W: f32 = 200.0;
const SEARCH_MAX_LEN: usize = 32;

const ROW_W: f32 = 270.0;
const ROW_H: f32 = 36.0;
const ICON_SIZE: f32 = 32.0;
const TEXT_INSET: f32 = ICON_SIZE + 3.0;
const MAX_TEXT_W: f32 = ROW_W - (CONTENT_PAD + TEXT_INSET) - 2.0;
const GREY: u32 = 0x80_8080;
const HARDCORE_RED: u32 = 0xFF_0000;
const HOVER_FILL: u32 = 0xA090_9090;
const TOOLTIP_W: f32 = 170.0;
const LOCKED_TOOLTIP_W: f32 = 175.0;
const ELLIPSIS: &str = "...";

const COL_W: f32 = 71.0;
const COL_GAP: f32 = 8.0;
const ROW_GAP: f32 = 4.0;
const WIDE_W: f32 = 2.0 * COL_W + COL_GAP;
const GRID_W: f32 = 4.0 * COL_W + 3.0 * COL_GAP;
const DIALOG_BUTTON_W: f32 = 150.0;

type Tooltip = Vec<Vec<Span>>;

struct Row {
    summary: WorldSummary,
    icon: Option<IconPixels>,
    name: Vec<Span>,
    id_line: Vec<Span>,
    info: Vec<Span>,
}

pub struct State {
    rows: Vec<Row>,
    search: TextBox,
    selected: Option<String>,
    scroll: f32,
    scroll_drag: Option<f32>,
    rescanned: bool,
    empty_create: bool,
    confirm_delete: Option<(String, String)>,
    deleting: Option<std::sync::Arc<std::sync::Mutex<Option<Result<(), String>>>>>,
    message: Option<(String, String)>,
}

impl Default for State {
    fn default() -> State {
        State {
            rows: Vec::new(),
            search: TextBox::bordered(SEARCH_MAX_LEN, ""),
            selected: None,
            scroll: 0.0,
            scroll_drag: None,
            rescanned: false,
            empty_create: false,
            confirm_delete: None,
            deleting: None,
            message: None,
        }
    }
}

impl State {
    pub fn open(&mut self) {
        *self = State::default();
        self.search.placeholder = label!("gui.selectWorld.search").to_owned();
        self.search.focus();
        self.rescan();
    }

    pub fn reload(&mut self) {
        self.search.focus();
        self.rescan();
    }

    fn rescan(&mut self) {
        self.rows = store::scan().into_iter().map(Row::new).collect();
        for row in &self.rows {
            if let Some(pixels) = &row.icon {
                let key = icon_key(&row.summary.folder);
                if ping::icon_slot(&key).is_some() {
                    ping::note_icon(&key, pixels.clone());
                }
            }
        }
        if let Some(folder) = &self.selected
            && !self.rows.iter().any(|r| &r.summary.folder == folder)
        {
            self.selected = None;
        }
        self.rescanned = true;
    }
}

impl Row {
    fn new(summary: WorldSummary) -> Row {
        let icon = summary
            .icon
            .as_deref()
            .and_then(|path| read_icon(path, &summary.folder));
        let mut id = summary.folder.clone();
        if summary.last_played != 0 {
            id = format!("{id} ({})", format_date(summary.last_played));
        }
        Row {
            name: vec![Span::new(summary.settings.name.clone())],
            id_line: vec![Span::colored(id, GREY)],
            info: info_line(&summary),
            icon,
            summary,
        }
    }
}

fn read_icon(path: &Path, folder: &str) -> Option<IconPixels> {
    let decoded = std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| image::load_from_memory(&bytes).map_err(|e| e.to_string()));
    let image = match decoded {
        Ok(image) => image.to_rgba8(),
        Err(e) => {
            crate::log_warn!("worlds", "Invalid icon for world {folder}: {e}");
            return None;
        }
    };
    if image.width() != SERVER_ICON_PX || image.height() != SERVER_ICON_PX {
        crate::log_warn!(
            "worlds",
            "Invalid icon for world {folder}: must be 64x64, but was {}x{}",
            image.width(),
            image.height()
        );
        return None;
    }
    Some(IconPixels::new(image.into_raw()))
}

fn icon_key(folder: &str) -> String {
    format!("\u{0}world/{folder}")
}

fn format_date(millis: u64) -> String {
    let secs = (millis / 1000) as i64;
    let (year, month, day) = crate::util::calendar::civil_from_days(secs.div_euclid(86_400));
    let minutes = secs.rem_euclid(86_400) / 60;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        minutes / 60,
        minutes % 60
    )
}

fn info_line(w: &WorldSummary) -> Vec<Span> {
    if w.locked {
        return vec![Span::colored(
            label!("selectWorld.locked"),
            crate::text::RED,
        )];
    }
    let mut out = vec![match w.settings.game_mode {
        GameMode::Hardcore => Span::colored(label!("gameMode.hardcore"), HARDCORE_RED),
        GameMode::Survival => Span::colored(label!("gameMode.survival"), GREY),
        GameMode::Creative => Span::colored(label!("gameMode.creative"), GREY),
    }];
    if w.settings.effective_allow_commands() {
        out.push(Span::colored(
            format!(", {}", label!("selectWorld.commands")),
            GREY,
        ));
    }
    if !w.settings.experiments.is_empty() {
        out.push(Span::colored(", ", GREY));
        out.push(Span::colored(
            label!("selectWorld.experimental"),
            crate::text::YELLOW,
        ));
    }
    let version = if w.version.is_empty() {
        label!("selectWorld.versionUnknown")
    } else {
        w.version.as_str()
    };
    out.push(Span::colored(
        format!(", {} {version}", label!("selectWorld.version")),
        GREY,
    ));
    out
}

fn row_left(vw: f32) -> f32 {
    (vw / 2.0).floor() - (ROW_W / 2.0).floor()
}

fn scroll_to(scroll: &mut f32, index: usize, list_h: f32, content_h: f32) {
    let top = index as f32 * ROW_H;
    if top - *scroll < 0.0 {
        *scroll = top;
    }
    let bottom_delta = list_h - (CONTENT_PAD - *scroll + top) - ROW_H - CONTENT_PAD;
    if bottom_delta < 0.0 {
        *scroll -= bottom_delta;
    }
    *scroll = scroll.clamp(0.0, (content_h - list_h).max(0.0));
}

fn join(st: &mut State, index: usize) {
    let row = &st.rows[index];
    if row.summary.locked {
        return;
    }
    match backend::open(&row.summary.folder) {
        Ok(()) => {}
        Err(e) => st.message = Some((label!("selectWorld.unable_to_load").to_owned(), e)),
    }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    let worlds = &mut state.worlds;
    let st = &mut worlds.select;

    if std::mem::take(&mut st.rescanned) {
        if st.rows.is_empty() {
            if std::mem::take(&mut st.empty_create) {
                state.nav = Some(Screen::Title);
            } else {
                st.empty_create = true;
                worlds.create.open_fresh();
                state.nav = Some(Screen::CreateWorld);
            }
            return;
        }
        st.empty_create = false;
    }

    if let Some(slot) = &st.deleting {
        let done = slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some(result) = done else {
            p.text_centered(
                label!("menu.working"),
                ctx.vw / 2.0,
                ctx.vh / 2.0,
                0xFF_FFFF,
                true,
            );
            return;
        };
        st.deleting = None;
        st.reload();
        if let Err(e) = result {
            st.message = Some((label!("selectWorld.delete_failure").to_owned(), e));
        }
    }

    if let Some((title, text)) = &st.message {
        let back = [label!("gui.back").to_owned()];
        let chosen = confirmlink::draw_box(p, ctx, title, text, None, &back, DIALOG_BUTTON_W);
        if chosen.is_some() || ctx.input.escape {
            st.message = None;
        }
        return;
    }
    if let Some((folder, warning)) = st.confirm_delete.clone() {
        let buttons = [
            label!("selectWorld.deleteButton").to_owned(),
            label!("gui.cancel").to_owned(),
        ];
        let chosen = confirmlink::draw_box(
            p,
            ctx,
            label!("selectWorld.deleteQuestion"),
            &warning,
            None,
            &buttons,
            DIALOG_BUTTON_W,
        );
        let answer = match chosen {
            Some(0) => Some(true),
            Some(_) => Some(false),
            None if ctx.input.escape => Some(false),
            None => None,
        };
        if let Some(confirmed) = answer {
            st.confirm_delete = None;
            if confirmed {
                let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
                let out = slot.clone();
                let spawned = std::thread::Builder::new()
                    .name("world-delete".into())
                    .spawn(move || {
                        let result = store::delete(&folder);
                        *out.lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(result);
                    });
                match spawned {
                    Ok(_) => st.deleting = Some(slot),
                    Err(e) => {
                        st.message = Some((
                            label!("selectWorld.delete_failure").to_owned(),
                            e.to_string(),
                        ))
                    }
                }
            } else {
                st.reload();
            }
        }
        return;
    }

    p.text_centered(
        label!("selectWorld.title"),
        ctx.vw / 2.0,
        TITLE_Y,
        0xFF_FFFF,
        true,
    );
    st.search.update(
        p,
        ctx,
        centered_row_x(ctx.vw, SEARCH_W),
        SEARCH_Y,
        SEARCH_W,
        WIDGET_HEIGHT,
    );

    let needle = st.search.text.as_str();
    let shown: Vec<usize> = (0..st.rows.len())
        .filter(|&i| {
            let s = &st.rows[i].summary;
            contains_ci(&s.settings.name, needle) || contains_ci(&s.folder, needle)
        })
        .collect();
    let mut sel = st
        .selected
        .as_ref()
        .and_then(|f| shown.iter().position(|&i| &st.rows[i].summary.folder == f));
    if sel.is_none() {
        st.selected = None;
    }

    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = row_left(ctx.vw);
    let content_h = shown.len() as f32 * ROW_H + 2.0 * CONTENT_PAD;
    let input = scroll_masked(
        &mut st.scroll,
        &mut st.scroll_drag,
        ctx,
        left,
        ROW_W,
        (lx, ly, lw, lh),
        content_h,
    );
    menu::list_background(p, lx, ly, lw, lh, st.scroll);
    let list_ctx = ctx.with_input(&input);
    let cursor = list_ctx.mouse().map(|m| (m.x, m.y));

    let list_focused = focus::next(!shown.is_empty());
    let mut tip: Option<Tooltip> = None;
    let mut clicked: Option<(usize, bool)> = None;
    let top = (ly + CONTENT_PAD - st.scroll).floor();
    let clip = p.push_clip(lx, ly, lw, lh);
    for (n, &i) in shown.iter().enumerate() {
        let y = top + n as f32 * ROW_H;
        if !row_visible(y, ROW_H, ly, lh) {
            continue;
        }
        let row = &st.rows[i];
        if sel == Some(n) {
            let border = if list_focused {
                0xFFFF_FFFF
            } else {
                0xFF80_8080
            };
            p.fill(left, y, ROW_W, ROW_H, border);
            p.fill(left + 1.0, y + 1.0, ROW_W - 2.0, ROW_H - 2.0, 0xFF00_0000);
        }
        let (cx, cy) = (left + CONTENT_PAD, y + CONTENT_PAD);
        if let Some(t) = draw_row(p, row, cx, cy, cursor) {
            tip = Some(t);
        }

        if !list_ctx.hovering(left, y, ROW_W, ROW_H) {
            continue;
        }
        p.fill(cx, cy, ICON_SIZE, ICON_SIZE, HOVER_FILL);
        let over_icon = list_ctx.hovering(cx, cy, ICON_SIZE, ICON_SIZE);
        if row.summary.locked {
            let sprite = if over_icon {
                "world_list/error_highlighted"
            } else {
                "world_list/error"
            };
            p.sprite(sprite, cx, cy, ICON_SIZE, ICON_SIZE);
            if over_icon {
                let locked = [Span::colored(
                    label!("selectWorld.locked"),
                    crate::text::RED,
                )];
                tip = Some(p.atlas.font.wrap(&locked, LOCKED_TOOLTIP_W));
            }
        } else {
            let sprite = if over_icon {
                "world_list/join_highlighted"
            } else {
                "world_list/join"
            };
            p.sprite(sprite, cx, cy, ICON_SIZE, ICON_SIZE);
        }
        if list_ctx.input.left_click {
            focus::claim();
            clicked = Some((n, over_icon || list_ctx.input.double_click));
        }
    }
    p.pop_clip(clip);
    draw_scrollbar(p, st.scroll, left, ROW_W, ly, lh, content_h);

    let mut join_index = None;
    if let Some((n, joins)) = clicked {
        sel = Some(n);
        st.selected = Some(st.rows[shown[n]].summary.folder.clone());
        scroll_to(&mut st.scroll, n, lh, content_h);
        if joins {
            join_index = Some(shown[n]);
        }
    }

    let selected_row = sel.map(|n| shown[n]);
    let unlocked = selected_row.is_some_and(|i| !st.rows[i].summary.locked);
    let row1 = ctx.vh - FOOTER_H + ((FOOTER_H - 2.0 * WIDGET_HEIGHT - ROW_GAP) / 2.0).floor();
    let row2 = row1 + WIDGET_HEIGHT + ROW_GAP;
    let x0 = centered_row_x(ctx.vw, GRID_W);
    let col = |c: f32| x0 + c * (COL_W + COL_GAP);
    let button = |x: f32, y: f32, w: f32, label: &'static str, active: bool| Button {
        active,
        ..Button::new(x, y, w, WIDGET_HEIGHT, label)
    };
    let play = button(
        col(0.0),
        row1,
        WIDE_W,
        label!("selectWorld.select"),
        unlocked,
    )
    .draw(p, ctx);
    let create = button(col(2.0), row1, WIDE_W, label!("selectWorld.create"), true).draw(p, ctx);
    let edit = button(col(0.0), row2, COL_W, label!("selectWorld.edit"), unlocked).draw(p, ctx);
    let delete = button(
        col(1.0),
        row2,
        COL_W,
        label!("selectWorld.delete"),
        selected_row.is_some(),
    )
    .draw(p, ctx);
    let recreate_active = unlocked && is_backed(Control::Recreate);
    let recreate = button(
        col(2.0),
        row2,
        COL_W,
        label!("selectWorld.recreate"),
        recreate_active,
    )
    .draw(p, ctx);
    let back = button(col(3.0), row2, COL_W, label!("gui.back"), true).draw(p, ctx);

    if let Some(lines) = tip
        && let Some(m) = ctx.mouse()
    {
        tooltip::draw_lines(p, &lines, m.x, m.y, ctx.vw, ctx.vh);
    }

    if list_focused && !ctx.input.escape {
        let before = sel;
        arrow_select(
            &mut sel,
            shown.len(),
            ctx.input.up_arrow,
            ctx.input.down_arrow,
        );
        if let Some(n) = sel
            && sel != before
        {
            st.selected = Some(st.rows[shown[n]].summary.folder.clone());
            scroll_to(&mut st.scroll, n, lh, content_h);
        }
        if ctx.input.select
            && let Some(n) = sel
        {
            join_index = Some(shown[n]);
        }
    }

    if play && let Some(i) = selected_row {
        join_index = Some(i);
    }
    if let Some(i) = join_index {
        join(st, i);
    }
    if create {
        worlds.create.open_fresh();
        state.nav = Some(Screen::CreateWorld);
    }
    if edit && let Some(i) = selected_row {
        worlds.edit.open(&worlds.select.rows[i].summary);
        state.nav = Some(Screen::EditWorld);
    }
    if recreate && let Some(i) = selected_row {
        worlds.create.open_from(&worlds.select.rows[i].summary);
        state.nav = Some(Screen::CreateWorld);
    }
    if delete && let Some(i) = selected_row {
        let st = &mut worlds.select;
        let s = &st.rows[i].summary;
        let warning = tooltip::translate("selectWorld.deleteWarning", &[s.settings.name.clone()]);
        st.confirm_delete = Some((s.folder.clone(), warning));
    }
    if back || ctx.input.escape {
        state.nav = Some(Screen::Title);
    }
}

fn draw_row(
    p: &mut Painter,
    row: &Row,
    cx: f32,
    cy: f32,
    cursor: Option<(f32, f32)>,
) -> Option<Tooltip> {
    let x = cx + TEXT_INSET;
    let mut tip = None;
    for (spans, y) in [
        (&row.name, cy + 1.0),
        (&row.id_line, cy + LINE_HEIGHT + 3.0),
        (&row.info, cy + 2.0 * LINE_HEIGHT + 3.0),
    ] {
        if let Some(t) = draw_clipped(p, spans, x, y, cursor) {
            tip = Some(t);
        }
    }

    let slot = row
        .icon
        .as_ref()
        .and_then(|pixels| ping::ensure_icon(&icon_key(&row.summary.folder), pixels));
    match slot.and_then(|slot| p.atlas.server_icon(slot)) {
        Some(region) => p.region(region, cx, cy, ICON_SIZE, ICON_SIZE),
        None => p.sprite("unknown_server", cx, cy, ICON_SIZE, ICON_SIZE),
    }
    tip
}

fn draw_clipped(
    p: &mut Painter,
    spans: &[Span],
    x: f32,
    y: f32,
    cursor: Option<(f32, f32)>,
) -> Option<Tooltip> {
    let font = &p.atlas.font;
    if font.width(spans) <= MAX_TEXT_W {
        p.text(spans, x, y, true);
        return None;
    }
    let mut clipped = font.trim_to_width(spans, MAX_TEXT_W - font.width_str(ELLIPSIS));
    clipped.push(Span::new(ELLIPSIS));
    p.text(&clipped, x, y, true);
    let (mx, my) = cursor?;
    (mx >= x && mx < x + MAX_TEXT_W && my >= y && my < y + LINE_HEIGHT)
        .then(|| p.atlas.font.wrap(spans, TOOLTIP_W))
}
