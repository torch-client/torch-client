use std::sync::Arc;

use crate::gui::dialog::CONFIRM_BUTTON_WIDTH;
use crate::gui::packdrop::{self, Drops, GRAY, LAYOUT, Search};
use crate::gui::painter::Painter;
use crate::gui::ping::IconPixels;
use crate::gui::tooltip::translate;
use crate::gui::widgets::{Button, FOOTER_GAP, TextBox, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx, confirmlink, focus};
use crate::platform::packfs;
use crate::resourcepacks::{self, Compat, Info, ServerPack};
use crate::session::SharedMutex;
use crate::text::{Span, Style};

use super::options::{SCROLLBAR_GAP, SCROLLBAR_W, draw_scrollbar, row_visible, scroll_masked};

mod tr {
    use crate::gui::tooltip::label;

    macro_rules! keys {
        ($($name:ident = $key:literal),* $(,)?) => {
            $(pub fn $name() -> &'static str { label!($key) })*
        };
    }

    keys! {
        title = "resourcePack.title",
        available = "pack.available.title",
        selected = "pack.selected.title",
        open_folder = "pack.openFolder",
        done = "gui.done",
        vanilla_name = "resourcePack.vanilla.name",
        vanilla_description = "resourcePack.vanilla.description",
        incompatible = "pack.incompatible",
        too_old = "pack.incompatible.old",
        too_new = "pack.incompatible.new",
        unknown = "pack.incompatible.unknown",
    }
}

const VANILLA_HAYSTACK: &str = "vanilla\ndefault\nthe default look and feel of minecraft";
const SAVE: &str = "Save";
const SAVED: &str = "Saved";
const SAVE_TOOLTIP: &str = "Copy this pack into the resource packs folder";
const SAVED_TOOLTIP: &str = "A copy is already in the resource packs folder";
const SAVE_W: f32 = 34.0;

pub(in crate::gui) const LIST_W: f32 = 200.0;
pub(in crate::gui) const LIST_GAP: f32 = 15.0;
pub(in crate::gui) const ROW_H: f32 = 36.0;
pub(in crate::gui) const HEADER_H: f32 = 13.0;
pub(in crate::gui) const ROW_W: f32 = LIST_W - 4.0;
pub(in crate::gui) const PADDING: f32 = 2.0;
pub(in crate::gui) const ICON: f32 = 32.0;
pub(in crate::gui) const TEXT_W: f32 = 157.0;
pub(in crate::gui) const BAR_ROW_W: f32 = LIST_W - SCROLLBAR_W - SCROLLBAR_GAP;

const INCOMPATIBLE_FILL: u32 = 0xFF77_0000;
const HOVER_FILL: u32 = 0xA090_9090;
pub(in crate::gui) const DESCRIPTION: u32 = 0x80_8080;
const OUTLINE_FOCUSED: u32 = 0xFFFF_FFFF;
const OUTLINE: u32 = 0xFF80_8080;
const OUTLINE_INSIDE: u32 = 0xFF00_0000;

pub(in crate::gui) fn icon_key(name: &str) -> String {
    format!("\u{0}resourcepack/{name}")
}

struct Row {
    name: String,
    info: Info,
    icon: Option<IconPixels>,
    haystack: String,
}

struct ServerRow {
    pack: Arc<ServerPack>,
    icon: Option<IconPixels>,
    description: Vec<Span>,
    haystack: String,
    saved: bool,
}

impl ServerRow {
    fn new(pack: Arc<ServerPack>) -> ServerRow {
        let mut description = pack.info.description.clone();
        for span in &mut description {
            if span.style.color == Style::default().color {
                span.style.color = GRAY;
            }
        }
        description.push(Span::colored(
            format!(" ({})", translate("pack.source.server", &[])),
            GRAY,
        ));
        let text: String = description.iter().map(|s| s.text.as_str()).collect();
        let haystack = format!(
            "server/{}\n{}\n{text}",
            pack.file_name,
            translate("resourcePack.server.name", &[])
        )
        .to_lowercase();
        ServerRow {
            icon: pack.info.icon.clone().map(Arc::new),
            saved: packfs::exists(&resourcepacks::dir().join(&pack.file_name)),
            pack,
            description,
            haystack,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Entry {
    Server(usize),
    Pack(usize),
    Default,
}

enum Action {
    Select(usize),
    Unselect(usize),
    Up(usize),
    Down(usize),
}

#[derive(Default)]
pub struct State {
    rows: Vec<Row>,
    selected: Vec<usize>,
    saved: Vec<String>,
    scroll: [f32; 2],
    drag: [Option<f32>; 2],
    search: TextBox,
    server: Vec<ServerRow>,
    server_generation: Option<u64>,
    cursor: [Option<Entry>; 2],
    take_focus: Option<usize>,
    incompatible: Option<usize>,
    pub(in crate::gui) drops: Drops,
}

impl State {
    pub fn open(&mut self) {
        self.saved = resourcepacks::selected();
        self.load(&self.saved.clone());
        self.search = packdrop::search_box();
        self.server_generation = None;
        self.take_focus = None;
        self.incompatible = None;
        self.drops.clear();
    }

    fn rescan(&mut self) {
        let selection = self.selected_names();
        let scroll = self.scroll;
        self.load(&selection);
        self.scroll = scroll;
        for server in &mut self.server {
            server.saved = packfs::exists(&resourcepacks::dir().join(&server.pack.file_name));
        }
    }

    fn load(&mut self, selection: &[String]) {
        self.rows = resourcepacks::list()
            .into_iter()
            .map(|pack| {
                let mut info = resourcepacks::info(&pack);
                let icon = info.icon.take().map(Arc::new);
                let text: String = info.description.iter().map(|s| s.text.as_str()).collect();
                let haystack = format!("{}\n{text}", pack.name).to_lowercase();
                Row {
                    name: pack.name,
                    info,
                    icon,
                    haystack,
                }
            })
            .collect();
        self.selected = selection
            .iter()
            .filter_map(|name| self.rows.iter().position(|row| &row.name == name))
            .collect();
        self.scroll = [0.0; 2];
        self.drag = [None; 2];
        self.cursor = [None; 2];
    }

    fn refresh_server(&mut self, shared: &SharedMutex) {
        let generation = crate::client::server_packs::generation();
        if self.server_generation == Some(generation) {
            return;
        }
        self.server_generation = Some(generation);
        let shown = shared.lock().unwrap().session.server_packs.shown();
        self.server = shown.into_iter().map(ServerRow::new).collect();
        if matches!(self.cursor[1], Some(Entry::Server(i)) if i >= self.server.len()) {
            self.cursor[1] = None;
        }
    }

    pub fn apply(&mut self) {
        let chosen = self.selected_names();
        if chosen != self.saved {
            resourcepacks::save_selected(&chosen);
            self.saved = chosen;
            crate::renderer::reload::request();
        }
    }

    pub fn cancel_prompt(&mut self) -> bool {
        self.incompatible.take().is_some() || self.drops.cancel()
    }

    fn selected_names(&self) -> Vec<String> {
        self.selected
            .iter()
            .map(|&i| self.rows[i].name.clone())
            .collect()
    }

    fn select(&mut self, index: usize) {
        if self.rows[index].info.compat == Compat::Compatible {
            self.act(Action::Select(index));
        } else {
            self.incompatible = Some(index);
        }
    }

    fn act(&mut self, action: Action) {
        let (index, side) = match action {
            Action::Select(index) => {
                self.selected.insert(0, index);
                (index, 1)
            }
            Action::Unselect(index) => {
                self.selected.retain(|&i| i != index);
                (index, 0)
            }
            Action::Up(index) | Action::Down(index) => {
                let Some(at) = self.selected.iter().position(|&i| i == index) else {
                    return;
                };
                let to = if matches!(action, Action::Up(_)) {
                    at.checked_sub(1)
                } else {
                    Some(at + 1)
                };
                match to.filter(|&to| to < self.selected.len()) {
                    Some(to) => self.selected.swap(at, to),
                    None => return,
                }
                (index, 1)
            }
        };
        self.cursor = [None; 2];
        self.cursor[side] = Some(Entry::Pack(index));
        self.take_focus = Some(side);
    }

    fn keyboard_select(&mut self, side: usize) {
        if let Some(Entry::Pack(index)) = self.cursor[side] {
            if side == 0 {
                self.select(index);
            } else {
                self.act(Action::Unselect(index));
            }
        }
    }

    fn keyboard_move(&mut self, up: bool) {
        if let Some(Entry::Pack(index)) = self.cursor[1] {
            self.act(if up {
                Action::Up(index)
            } else {
                Action::Down(index)
            });
        }
    }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, shared: &SharedMutex) {
    #[cfg(explore_modrinth)]
    if state.explore_packs.take_changed() {
        state.resourcepacks.rescan();
    }
    let packs = &mut state.resourcepacks;
    packs.refresh_server(shared);
    if packs.incompatible.is_some() {
        draw_incompatible(p, packs, ctx);
        return;
    }
    if let Some(copied) = packs.drops.draw(
        p,
        ctx,
        shared,
        &resourcepacks::dir(),
        resourcepacks::is_pack_dir,
    ) {
        if copied {
            packs.rescan();
        }
        return;
    }

    packdrop::header(p, ctx, tr::title(), &mut packs.search);

    let rect = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let list_y = rect.1;
    let selection = packs.selected_names();
    let failed: Vec<String> = resourcepacks::FAILED
        .get()
        .into_iter()
        .filter(|name| selection.contains(name))
        .collect();
    let failed = packdrop::failed_message(&failed);
    let message = failed
        .as_deref()
        .map(|text| (text, packdrop::MESSAGE_ERROR));
    let list_h = packdrop::message(p, ctx, rect, ROW_H, message);
    let list_w = LIST_W.min((ctx.vw / 2.0 - LIST_GAP - PADDING).floor());
    let row_w = list_w - (LIST_W - ROW_W);
    let bar_row_w = list_w - (LIST_W - BAR_ROW_W);
    let lefts = [
        (ctx.vw / 2.0 - LIST_GAP - list_w).floor(),
        (ctx.vw / 2.0 + LIST_GAP).floor(),
    ];

    let search = Search::of(&packs.search);
    let matches = |haystack: &str| search.matches(haystack);
    let available: Vec<Entry> = (0..packs.rows.len())
        .filter(|i| !packs.selected.contains(i) && matches(&packs.rows[*i].haystack))
        .map(Entry::Pack)
        .collect();
    let mut chosen: Vec<Entry> = (0..packs.server.len())
        .filter(|&i| matches(&packs.server[i].haystack))
        .map(Entry::Server)
        .collect();
    chosen.extend(
        packs
            .selected
            .iter()
            .filter(|&&i| matches(&packs.rows[i].haystack))
            .map(|&i| Entry::Pack(i)),
    );
    if matches(VANILLA_HAYSTACK) {
        chosen.push(Entry::Default);
    }
    let lists = [available, chosen];

    let mut action = None;
    let mut save = None;
    let mut tooltip = None;
    let mut focused = [false; 2];
    for (side, left) in lefts.into_iter().enumerate() {
        let list = &lists[side];
        let content_h = HEADER_H + list.len() as f32 * ROW_H;
        let input = scroll_masked(
            &mut packs.scroll[side],
            &mut packs.drag[side],
            ctx,
            left,
            bar_row_w,
            (left, list_y, list_w, list_h),
            content_h,
        );
        let scrolls = content_h > list_h;
        let list_ctx = ctx.with_input(&input);
        focused[side] = focus::next(true);
        if packs.take_focus == Some(side) {
            focus::claim();
            packs.take_focus = None;
        }
        focus::suspend();
        let clip = p.push_clip(left, list_y, list_w, list_h);
        let top = (list_y - packs.scroll[side]).floor();
        header(
            p,
            if side == 0 {
                tr::available()
            } else {
                tr::selected()
            },
            left,
            top,
            list_w,
        );

        let row_x = left + (list_w - row_w) / 2.0;
        let text_w = list_w - (LIST_W - TEXT_W) - if scrolls { SCROLLBAR_W } else { 0.0 };
        let mut clicked = None;
        for (slot, &entry) in list.iter().enumerate() {
            let y = top + HEADER_H + slot as f32 * ROW_H;
            if !row_visible(y, ROW_H, list_y, list_h) {
                continue;
            }
            let (cx, cy) = (row_x + PADDING, y + PADDING);
            let hovered = list_ctx.hovering(row_x, y, row_w, ROW_H);
            if hovered && list_ctx.input.left_click {
                clicked = Some(entry);
            }
            let is_cursor = packs.cursor[side] == Some(entry);
            if is_cursor {
                let w = row_w - if scrolls { SCROLLBAR_W } else { 0.0 };
                p.fill(
                    row_x,
                    y,
                    w,
                    ROW_H,
                    if focused[side] {
                        OUTLINE_FOCUSED
                    } else {
                        OUTLINE
                    },
                );
                p.fill(row_x + 1.0, y + 1.0, w - 2.0, ROW_H - 2.0, OUTLINE_INSIDE);
            }
            let index = match entry {
                Entry::Default => {
                    p.sprite("pack_icon", cx, cy, ICON, ICON);
                    entry_text(
                        p,
                        tr::vanilla_name(),
                        &plain(tr::vanilla_description()),
                        cx,
                        cy,
                        text_w,
                    );
                    continue;
                }
                Entry::Server(i) => {
                    let server = &packs.server[i];
                    if server.pack.info.compat != Compat::Compatible {
                        p.fill(
                            cx - 1.0,
                            cy - 1.0,
                            row_w - 2.0 * PADDING + 2.0,
                            ICON + 2.0,
                            INCOMPATIBLE_FILL,
                        );
                    }
                    let key = icon_key(&format!("\u{0}server/{}", server.pack.file_name));
                    draw_icon(p, &key, server.icon.as_ref(), cx, cy);
                    let name = translate("resourcePack.server.name", &[]);
                    let description_w = text_w - SAVE_W - PADDING;
                    entry_text_widths(p, &name, &server.description, cx, cy, text_w, description_w);
                    let (bx, by) = (cx + ICON + PADDING + description_w + PADDING, cy + 12.0);
                    let label = if server.saved { SAVED } else { SAVE };
                    let button = Button {
                        active: !server.saved,
                        ..Button::new(bx, by, SAVE_W, 18.0, label)
                    };
                    if button.draw(p, &list_ctx) {
                        save = Some(i);
                    }
                    if list_ctx.hovering(bx, by, SAVE_W, 18.0) {
                        tooltip = Some(if server.saved {
                            SAVED_TOOLTIP
                        } else {
                            SAVE_TOOLTIP
                        });
                    }
                    continue;
                }
                Entry::Pack(index) => index,
            };
            let row = &packs.rows[index];
            let compatible = row.info.compat == Compat::Compatible;
            if !compatible {
                p.fill(
                    cx - 1.0,
                    cy - 1.0,
                    row_w - 2.0 * PADDING + 2.0,
                    ICON + 2.0,
                    INCOMPATIBLE_FILL,
                );
            }
            draw_icon(p, &icon_key(&row.name), row.icon.as_ref(), cx, cy);

            let overlay = hovered || (is_cursor && focused[side]);
            if overlay && !compatible {
                let reason = match row.info.compat {
                    Compat::TooOld => tr::too_old(),
                    Compat::TooNew => tr::too_new(),
                    _ => tr::unknown(),
                };
                entry_text(p, tr::incompatible(), &plain(reason), cx, cy, text_w);
            } else {
                entry_text(p, &row.name, &row.info.description, cx, cy, text_w);
            }
            if !overlay {
                continue;
            }
            p.fill(cx, cy, ICON, ICON, HOVER_FILL);
            let (rx, ry) = list_ctx
                .mouse()
                .filter(|_| hovered)
                .map_or((-1.0, -1.0), |m| (m.x - cx, m.y - cy));
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
                action = Some(Action::Unselect(index));
            }
            let at = packs.selected.iter().position(|&i| i == index).unwrap_or(0);
            if at > 0 {
                let over = on_icon && !left_half && ry < ICON / 2.0;
                control(p, "move_up", over, cx, cy);
                if click && over {
                    action = Some(Action::Up(index));
                }
            }
            if at + 1 < packs.selected.len() {
                let over = on_icon && !left_half && ry >= ICON / 2.0;
                control(p, "move_down", over, cx, cy);
                if click && over {
                    action = Some(Action::Down(index));
                }
            }
        }
        p.pop_clip(clip);
        focus::resume();
        if let Some(entry) = clicked {
            packs.cursor[side] = Some(entry);
            focus::claim();
        }
        draw_scrollbar(
            p,
            packs.scroll[side],
            left,
            bar_row_w,
            list_y,
            list_h,
            content_h,
        );
    }

    match action {
        Some(Action::Select(index)) => packs.select(index),
        Some(action) => packs.act(action),
        None => list_keys(packs, ctx, &lists, &focused, list_h),
    }
    if let Some(i) = save {
        save_server_pack(packs, i, shared);
    }

    let footer = packdrop::footer(p, ctx, if cfg!(explore_modrinth) { 3 } else { 2 });
    if footer.reload {
        state.resourcepacks.rescan();
    }
    let mut x = footer.x;
    #[cfg(explore_modrinth)]
    {
        let explore = crate::gui::explore_packs::EXPLORE;
        if Button::new(x, footer.y, footer.w, WIDGET_HEIGHT, explore).draw(p, ctx) {
            state.explore_packs.open();
            state.nav = Some(Screen::ExplorePacks);
        }
        x += footer.w + FOOTER_GAP;
    }
    let folder_hovered =
        packdrop::open_folder(p, ctx, &footer, x, tr::open_folder(), &resourcepacks::dir());
    x += footer.w + FOOTER_GAP;
    if Button::new(x, footer.y, footer.w, WIDGET_HEIGHT, tr::done()).draw(p, ctx) {
        state.resourcepacks.apply();
        state.nav = Some(Screen::Options);
    }

    if let Some(text) = tooltip {
        packdrop::tooltip(p, ctx, text);
    } else if folder_hovered {
        packdrop::folder_tooltip(p, ctx);
    }
}

fn list_keys(
    packs: &mut State,
    ctx: &ScreenCtx,
    lists: &[Vec<Entry>; 2],
    focused: &[bool; 2],
    list_h: f32,
) {
    let input = ctx.input;
    let Some(side) = (0..2).find(|&side| focused[side]) else {
        return;
    };
    if input.enter {
        packs.keyboard_select(side);
        return;
    }
    if !(input.up_arrow || input.down_arrow) {
        return;
    }
    if input.shift {
        if side == 1 {
            packs.keyboard_move(input.up_arrow);
        }
        return;
    }
    let list = &lists[side];
    if list.is_empty() {
        return;
    }
    let at = packs.cursor[side].and_then(|entry| list.iter().position(|&e| e == entry));
    let next = match (at, input.up_arrow) {
        (Some(at), true) => at.saturating_sub(1),
        (Some(at), false) => (at + 1).min(list.len() - 1),
        (None, true) => list.len() - 1,
        (None, false) => 0,
    };
    packs.cursor[side] = Some(list[next]);
    let row_top = HEADER_H + next as f32 * ROW_H;
    let scroll = &mut packs.scroll[side];
    if row_top < *scroll {
        *scroll = row_top;
    } else if row_top + ROW_H > *scroll + list_h {
        *scroll = row_top + ROW_H - list_h;
    }
}

fn save_server_pack(packs: &mut State, i: usize, shared: &SharedMutex) {
    let pack = packs.server[i].pack.clone();
    match resourcepacks::install(&pack.file_name, &pack.zip) {
        Ok(()) => {
            crate::log_info!(
                "resourcepacks",
                "saved the server's pack as {}",
                pack.file_name
            );
            packs.rescan();
        }
        Err(e) => {
            crate::log_warn!(
                "resourcepacks",
                "could not save the server's pack as {}: {e}",
                pack.file_name
            );
            packdrop::copy_failure_toast(shared, &e);
        }
    }
}

fn draw_incompatible(p: &mut Painter, packs: &mut State, ctx: &ScreenCtx) {
    let Some(index) = packs.incompatible else {
        return;
    };
    let reason = match packs.rows[index].info.compat {
        Compat::TooOld => "pack.incompatible.confirm.old",
        Compat::TooNew => "pack.incompatible.confirm.new",
        _ => "pack.incompatible.confirm.unknown",
    };
    let title = translate("pack.incompatible.confirm.title", &[]);
    let yes_no = [translate("gui.yes", &[]), translate("gui.no", &[])];
    let message = translate(reason, &[]);
    let Some(chosen) = confirmlink::draw_box(
        p,
        ctx,
        &title,
        &message,
        None,
        &yes_no,
        CONFIRM_BUTTON_WIDTH,
    ) else {
        return;
    };
    packs.incompatible = None;
    if chosen == 0 {
        packs.act(Action::Select(index));
    }
}

fn draw_icon(p: &mut Painter, key: &str, pixels: Option<&IconPixels>, x: f32, y: f32) {
    let slot = pixels.and_then(|pixels| crate::gui::ping::ensure_icon(key, pixels));
    match slot.and_then(|slot| p.atlas.server_icon(slot)) {
        Some(region) => p.region(region, x, y, ICON, ICON),
        None => p.sprite("unknown_pack", x, y, ICON, ICON),
    }
}

pub(in crate::gui) fn header(p: &mut Painter, title: &str, left: f32, top: f32, list_w: f32) {
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
        (left + (list_w - w) / 2.0).floor(),
        top + (HEADER_H - crate::text::LINE_HEIGHT) / 2.0 + 1.0,
        true,
    );
}

pub(in crate::gui) fn entry_text(
    p: &mut Painter,
    name: &str,
    description: &[Span],
    cx: f32,
    cy: f32,
    w: f32,
) {
    entry_text_widths(p, name, description, cx, cy, w, w);
}

fn entry_text_widths(
    p: &mut Painter,
    name: &str,
    description: &[Span],
    cx: f32,
    cy: f32,
    name_w: f32,
    description_w: f32,
) {
    let x = cx + ICON + PADDING;
    let name = p
        .atlas
        .font
        .trim_to_width(&[Span::colored(name, 0xFF_FFFF)], name_w);
    p.text(&name, x, cy + 1.0, true);
    let lines = p.atlas.font.wrap(&grey(description), description_w);
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

pub(in crate::gui) fn plain(text: &str) -> Vec<Span> {
    vec![Span::colored(text, DESCRIPTION)]
}

fn grey(spans: &[Span]) -> Vec<Span> {
    let mut spans = spans.to_vec();
    for span in &mut spans {
        if span.style.color == Style::default().color {
            span.style.color = DESCRIPTION;
        }
    }
    spans
}
