use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::gui::menu;
use crate::gui::options::{
    SCROLLBAR_GAP, SCROLLBAR_W, draw_scrollbar, row_visible, scroll_input, scroll_masked,
};
use crate::gui::painter::Painter;
use crate::gui::ping::IconPixels;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{
    Button, FOOTER_GAP, FOOTER_W, HeaderFooter, TextBox, WIDGET_HEIGHT, centered_row_x,
};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::singleplayer::backed::{Control, is_backed};
use crate::text::{LINE_HEIGHT, Span, Style};

use super::hover_tooltip;

const TITLE_Y: f32 = 4.0;
const SEARCH_Y: f32 = TITLE_Y + LINE_HEIGHT + 4.0 + LINE_HEIGHT + 4.0;
const SEARCH_H: f32 = 15.0;
const LAYOUT: HeaderFooter = HeaderFooter::new(SEARCH_Y + SEARCH_H + 4.0, HeaderFooter::DEFAULT_H);
const LIST_W: f32 = 200.0;
const LIST_GAP: f32 = 15.0;
const SEARCH_MAX_LEN: usize = 32;
const ROW_H: f32 = 36.0;
const LIST_HEADER_H: f32 = 13.0;
const ROW_W: f32 = LIST_W - 4.0;
const PADDING: f32 = 2.0;
const ICON: f32 = 32.0;
const TEXT_W: f32 = 157.0;
const BAR_ROW_W: f32 = LIST_W - SCROLLBAR_W - SCROLLBAR_GAP;
const HOVER_FILL: u32 = 0xA090_9090;
const GRAY: u32 = crate::text::GRAY;
const RELOAD: &str = "R";

const CONFIRM_COLUMN_SPACING: f32 = 10.0;
const CONFIRM_ROW_SPACING: f32 = 20.0;
const DETAILS_BUTTON_W: f32 = 100.0;
const CONFIRM_MESSAGE_W: f32 = 310.0;
const DETAILS_ROW_H: f32 = (LINE_HEIGHT + 2.0) * 3.0;
const LIST_PAD: f32 = 4.0;

const VANILLA: usize = 0;

struct Pack {
    id: String,
    title: String,
    description: Vec<Span>,
    icon: Option<IconPixels>,
    experimental: Vec<String>,
    haystack: String,
}

impl Pack {
    fn vanilla() -> Pack {
        let description = translate("dataPack.vanilla.description", &[]);
        let title = translate("dataPack.vanilla.name", &[]);
        Pack {
            haystack: format!("vanilla\n{title}\n{description}").to_lowercase(),
            description: decorate(vec![Span::new(description)], "pack.source.builtin"),
            id: "vanilla".to_owned(),
            title,
            icon: None,
            experimental: Vec::new(),
        }
    }

    fn from_meta(name: String, meta: &str, png: Option<Vec<u8>>) -> Option<Pack> {
        let meta: serde_json::Value = serde_json::from_str(meta).ok()?;
        let section = meta.get("pack")?;
        let description = section
            .get("description")
            .and_then(|d| serde_json::from_value::<azalea_chat::FormattedText>(d.clone()).ok())
            .map(|text| crate::client::chat_text::to_spans(&text))
            .unwrap_or_default();
        let experimental = meta
            .get("features")
            .and_then(|f| f.get("enabled"))
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(|flag| {
                if flag.contains(':') {
                    flag.to_owned()
                } else {
                    format!("minecraft:{flag}")
                }
            })
            .filter(|flag| flag != "minecraft:vanilla")
            .collect();
        let id = format!("file/{name}");
        let plain: String = description.iter().map(|s| s.text.as_str()).collect();
        Some(Pack {
            haystack: format!("{id}\n{name}\n{plain}").to_lowercase(),
            description: decorate(description, "pack.source.world"),
            icon: png.as_deref().and_then(icon_pixels),
            experimental,
            title: name,
            id,
        })
    }
}

fn decorate(mut description: Vec<Span>, source: &str) -> Vec<Span> {
    for span in &mut description {
        if span.style.color == Style::default().color {
            span.style.color = GRAY;
        }
    }
    description.push(Span::colored(
        format!(" ({})", translate(source, &[])),
        GRAY,
    ));
    description
}

enum Prompt {
    Warning,
    Details,
}

enum Action {
    Select(usize),
    Unselect(usize),
    Up(usize),
    Down(usize),
}

#[derive(Default)]
pub struct State {
    packs: Vec<Pack>,
    selected: Vec<usize>,
    available: Vec<usize>,
    opened: Vec<String>,
    scroll: [f32; 2],
    drag: [Option<f32>; 2],
    search: TextBox,
    prompt: Option<Prompt>,
    details_scroll: f32,
    details_drag: Option<f32>,
    pub result: Option<Vec<String>>,
}

impl State {
    pub fn open(&mut self, selected: &[String]) {
        self.opened = selected.to_vec();
        self.search =
            TextBox::bordered(SEARCH_MAX_LEN, &translate("gui.packSelection.search", &[]));
        self.search.focus();
        self.prompt = None;
        self.result = None;
        self.load(selected);
    }

    fn load(&mut self, selected: &[String]) {
        self.packs = std::iter::once(Pack::vanilla())
            .chain(scan(&dir()))
            .collect();
        let mut chosen: Vec<usize> = Vec::new();
        for id in selected.iter().rev() {
            if let Some(i) = self.packs.iter().position(|p| &p.id == id)
                && i != VANILLA
                && !chosen.contains(&i)
            {
                chosen.push(i);
            }
        }
        chosen.push(VANILLA);
        self.available = (0..self.packs.len())
            .filter(|i| !chosen.contains(i))
            .collect();
        self.selected = chosen;
        self.scroll = [0.0; 2];
        self.drag = [None; 2];
    }

    fn rescan(&mut self) {
        let ids = self.selected_ids();
        let scroll = self.scroll;
        self.load(&ids);
        self.scroll = scroll;
    }

    fn selected_ids(&self) -> Vec<String> {
        self.selected
            .iter()
            .rev()
            .map(|&i| self.packs[i].id.clone())
            .collect()
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Select(i) => {
                self.available.retain(|&a| a != i);
                self.selected.insert(0, i);
            }
            Action::Unselect(i) => {
                self.selected.retain(|&s| s != i);
                self.available.insert(0, i);
            }
            Action::Up(i) | Action::Down(i) => {
                let Some(at) = self.selected.iter().position(|&s| s == i) else {
                    return;
                };
                let to = if matches!(action, Action::Up(_)) {
                    at.checked_sub(1)
                } else {
                    Some(at + 1)
                };
                if let Some(to) = to.filter(|&to| to < self.selected.len()) {
                    self.selected.swap(at, to);
                }
            }
        }
    }

    fn commit(&mut self) -> bool {
        if !is_backed(Control::DataPacks) {
            self.result = Some(self.opened.clone());
            return true;
        }
        let ids = self.selected_ids();
        let experimental = self
            .selected
            .iter()
            .any(|&i| !self.packs[i].experimental.is_empty());
        if ids != self.opened && experimental {
            self.prompt = Some(Prompt::Warning);
            self.details_scroll = 0.0;
            self.details_drag = None;
            return false;
        }
        self.result = Some(ids);
        true
    }
}

fn dir() -> PathBuf {
    crate::platform::storage::dir().join("datapacks")
}

fn scan(dir: &Path) -> Vec<Pack> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut packs: Vec<Pack> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if name.starts_with('.') {
                return None;
            }
            let path = entry.path();
            let (meta, png) = if path.is_dir() {
                (
                    std::fs::read_to_string(path.join("pack.mcmeta")).ok()?,
                    std::fs::read(path.join("pack.png")).ok(),
                )
            } else if name.to_ascii_lowercase().ends_with(".zip") {
                read_zip(&path)?
            } else {
                return None;
            };
            Pack::from_meta(name, &meta, png)
        })
        .collect();
    packs.sort_by(|a, b| a.id.cmp(&b.id));
    packs
}

#[cfg(any(
    feature = "asset_download",
    feature = "shader_support",
    resource_packs,
    lua_mods
))]
fn read_zip(path: &Path) -> Option<(String, Option<Vec<u8>>)> {
    use crate::util::zip;
    let bytes = std::fs::read(path).ok()?;
    let entries = zip::entries(&bytes).ok()?;
    let read = |name: &str| {
        entries
            .iter()
            .find(|e| e.name == name)
            .and_then(|e| zip::read(&bytes, e).ok())
    };
    let meta = String::from_utf8(read("pack.mcmeta")?).ok()?;
    Some((meta, read("pack.png")))
}

#[cfg(not(any(
    feature = "asset_download",
    feature = "shader_support",
    resource_packs,
    lua_mods
)))]
fn read_zip(_: &Path) -> Option<(String, Option<Vec<u8>>)> {
    None
}

fn icon_pixels(png: &[u8]) -> Option<IconPixels> {
    let side = crate::gui::atlas::SERVER_ICON_PX;
    let image = image::load_from_memory(png).ok()?.to_rgba8();
    let image = image::imageops::resize(&image, side, side, image::imageops::FilterType::Triangle);
    Some(Arc::new(image.into_raw()))
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    menu::background(p, ctx.vw, ctx.vh);
    let packs = &mut state.worlds.datapacks;
    if packs.prompt.is_some() {
        if draw_prompt(p, packs, ctx) {
            state.nav = Some(Screen::CreateWorld);
        }
        return;
    }
    let backed = is_backed(Control::DataPacks);

    p.text_centered(
        label!("dataPack.title"),
        ctx.vw / 2.0,
        TITLE_Y,
        0xFFFFFF,
        true,
    );
    packs.search.update(
        p,
        ctx,
        centered_row_x(ctx.vw, LIST_W),
        SEARCH_Y,
        LIST_W,
        SEARCH_H,
    );

    let (_, list_y, _, list_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let list_w = LIST_W.min((ctx.vw / 2.0 - LIST_GAP - PADDING).floor());
    let row_w = list_w - (LIST_W - ROW_W);
    let bar_row_w = list_w - (LIST_W - BAR_ROW_W);
    let lefts = [
        (ctx.vw / 2.0 - LIST_GAP - list_w).floor(),
        (ctx.vw / 2.0 + LIST_GAP).floor(),
    ];

    let needle = packs.search.text.to_lowercase();
    let blank = needle.trim().is_empty();
    let shown = |list: &[usize]| -> Vec<usize> {
        list.iter()
            .copied()
            .filter(|&i| blank || packs.packs[i].haystack.contains(&needle))
            .collect()
    };
    let lists = [shown(&packs.available), shown(&packs.selected)];
    let titles = [
        label!("pack.available.title"),
        label!("pack.selected.title"),
    ];

    let mut action = None;
    let mut unsupported = false;
    for (side, left) in lefts.into_iter().enumerate() {
        let list = &lists[side];
        let content_h = LIST_HEADER_H + list.len() as f32 * ROW_H;
        let rect = (left, list_y, list_w, list_h);
        let input = scroll_masked(
            &mut packs.scroll[side],
            &mut packs.drag[side],
            ctx,
            left,
            bar_row_w,
            rect,
            content_h,
        );
        let list_ctx = ctx.with_input(&input);
        let clip = p.push_clip(left, list_y, list_w, list_h);
        let top = (list_y - packs.scroll[side]).floor();
        list_header(p, titles[side], left, top, list_w);

        let row_x = left + (list_w - row_w) / 2.0;
        let text_w =
            list_w - (LIST_W - TEXT_W) - if content_h > list_h { SCROLLBAR_W } else { 0.0 };
        for (slot, &index) in list.iter().enumerate() {
            let y = top + LIST_HEADER_H + slot as f32 * ROW_H;
            if !row_visible(y, ROW_H, list_y, list_h) {
                continue;
            }
            let (cx, cy) = (row_x + PADDING, y + PADDING);
            let pack = &packs.packs[index];
            draw_icon(p, pack, cx, cy);
            entry_text(p, &pack.title, &pack.description, cx, cy, text_w);
            if index == VANILLA || !list_ctx.hovering(row_x, y, row_w, ROW_H) {
                continue;
            }
            p.fill(cx, cy, ICON, ICON, HOVER_FILL);
            let (rx, ry) = list_ctx
                .mouse()
                .map_or((-1.0, -1.0), |m| (m.x - cx, m.y - cy));
            let on_icon = rx >= 0.0 && rx < ICON && ry >= 0.0 && ry < ICON;
            if on_icon && !backed {
                unsupported = true;
            }
            let click = backed && list_ctx.input.left_click && on_icon;
            if side == 0 {
                control(p, "select", backed && on_icon, cx, cy);
                if click {
                    action = Some(Action::Select(index));
                }
                continue;
            }
            let left_half = on_icon && rx < ICON / 2.0;
            control(p, "unselect", backed && left_half, cx, cy);
            if click && left_half {
                action = Some(Action::Unselect(index));
            }
            let at = packs.selected.iter().position(|&i| i == index).unwrap_or(0);
            if at > 0 {
                let over = on_icon && !left_half && ry < ICON / 2.0;
                control(p, "move_up", backed && over, cx, cy);
                if click && over {
                    action = Some(Action::Up(index));
                }
            }
            if packs
                .selected
                .get(at + 1)
                .is_some_and(|&next| next != VANILLA)
            {
                let over = on_icon && !left_half && ry >= ICON / 2.0;
                control(p, "move_down", backed && over, cx, cy);
                if click && over {
                    action = Some(Action::Down(index));
                }
            }
        }
        p.pop_clip(clip);
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
    if let Some(action) = action {
        packs.act(action);
    }

    let footer_y = LAYOUT.footer_y(ctx.vh);
    let mut x = centered_row_x(ctx.vw, WIDGET_HEIGHT + 2.0 * (FOOTER_W + FOOTER_GAP));
    if Button::new(x, footer_y, WIDGET_HEIGHT, WIDGET_HEIGHT, RELOAD).draw(p, ctx) {
        packs.rescan();
    }
    x += WIDGET_HEIGHT + FOOTER_GAP;
    if Button::new(
        x,
        footer_y,
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("pack.openFolder"),
    )
    .draw(p, ctx)
    {
        crate::platform::url::open_folder(&dir());
    }
    let folder_hovered = ctx.hovering(x, footer_y, FOOTER_W, WIDGET_HEIGHT);
    x += FOOTER_W + FOOTER_GAP;
    let done = Button::new(x, footer_y, FOOTER_W, WIDGET_HEIGHT, label!("gui.done")).draw(p, ctx);
    if (done || ctx.input.escape) && packs.commit() {
        state.nav = Some(Screen::CreateWorld);
    }

    if unsupported {
        super::unsupported_tip(p, ctx);
    } else if folder_hovered {
        hover_tooltip(p, ctx, &translate("pack.folderInfo", &[]));
    }
}

fn draw_prompt(p: &mut Painter, packs: &mut State, ctx: &ScreenCtx) -> bool {
    match packs.prompt {
        Some(Prompt::Details) => {
            draw_details(p, packs, ctx);
            false
        }
        Some(Prompt::Warning) => draw_warning(p, packs, ctx),
        None => false,
    }
}

fn draw_warning(p: &mut Painter, packs: &mut State, ctx: &ScreenCtx) -> bool {
    let message: Vec<Vec<Span>> = translate("selectWorld.experimental.message", &[])
        .split('\n')
        .flat_map(|line| p.atlas.font.wrap(&[Span::new(line)], CONFIRM_MESSAGE_W))
        .collect();
    let message_h = message.len() as f32 * LINE_HEIGHT;
    let total_h = LINE_HEIGHT + message_h + 2.0 * WIDGET_HEIGHT + 3.0 * CONFIRM_ROW_SPACING;
    let mut y = ((ctx.vh - total_h) / 2.0).trunc();
    let cx = ctx.vw / 2.0;

    p.text_centered(
        label!("selectWorld.experimental.title"),
        cx,
        y,
        0xFFFFFF,
        true,
    );
    y += LINE_HEIGHT + CONFIRM_ROW_SPACING;
    for line in &message {
        let w = p.atlas.font.width(line);
        p.text(line, (cx - w / 2.0).floor(), y, true);
        y += LINE_HEIGHT;
    }
    y += CONFIRM_ROW_SPACING;
    let details_x = centered_row_x(ctx.vw, DETAILS_BUTTON_W);
    if Button::new(
        details_x,
        y,
        DETAILS_BUTTON_W,
        WIDGET_HEIGHT,
        label!("selectWorld.experimental.details"),
    )
    .draw(p, ctx)
    {
        packs.prompt = Some(Prompt::Details);
        return false;
    }
    y += WIDGET_HEIGHT + CONFIRM_ROW_SPACING;
    let x = centered_row_x(ctx.vw, 2.0 * FOOTER_W + CONFIRM_COLUMN_SPACING);
    if Button::new(x, y, FOOTER_W, WIDGET_HEIGHT, label!("gui.proceed")).draw(p, ctx) {
        packs.prompt = None;
        packs.result = Some(packs.selected_ids());
        return true;
    }
    let back = Button::new(
        x + FOOTER_W + CONFIRM_COLUMN_SPACING,
        y,
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("gui.back"),
    );
    if back.draw(p, ctx) || ctx.input.escape {
        packs.prompt = None;
        let opened = packs.opened.clone();
        packs.load(&opened);
    }
    false
}

fn draw_details(p: &mut Painter, packs: &mut State, ctx: &ScreenCtx) {
    let layout = HeaderFooter::new(HeaderFooter::DEFAULT_H, HeaderFooter::DEFAULT_H);
    p.text_centered(
        label!("selectWorld.experimental.details.title"),
        ctx.vw / 2.0,
        layout.title_y(),
        0xFFFFFF,
        true,
    );

    let (_, list_y, _, list_h) = layout.content_rect(ctx.vw, ctx.vh);
    let row_w = (ctx.vw * 3.0 / 4.0).floor();
    let row_left = ((ctx.vw - row_w) / 2.0).floor();
    let entries: Vec<usize> = packs
        .selected
        .iter()
        .copied()
        .filter(|&i| !packs.packs[i].experimental.is_empty())
        .collect();
    let content_h = entries.len() as f32 * DETAILS_ROW_H + 2.0 * LIST_PAD;
    scroll_input(
        &mut packs.details_scroll,
        &mut packs.details_drag,
        ctx,
        row_left,
        row_w,
        list_y,
        list_h,
        content_h,
    );

    let clip = p.push_clip(0.0, list_y, ctx.vw, list_h);
    let top = (list_y + LIST_PAD - packs.details_scroll).floor();
    for (slot, &i) in entries.iter().enumerate() {
        let y = top + slot as f32 * DETAILS_ROW_H;
        if !row_visible(y, DETAILS_ROW_H, list_y, list_h) {
            continue;
        }
        let pack = &packs.packs[i];
        let title = [Span {
            text: pack.title.clone(),
            style: Style {
                bold: true,
                ..Style::colored(0xFF_FFFF)
            },
        }];
        p.text(&title, row_left + PADDING, y + PADDING, true);
        let message = translate(
            "selectWorld.experimental.details.entry",
            &[pack.experimental.join(", ")],
        );
        let lines = p
            .atlas
            .font
            .wrap(&[Span::colored(message, 0xFF_FFFF)], row_w);
        for (row, line) in lines.iter().enumerate() {
            p.text(
                line,
                row_left + PADDING,
                y + PADDING + 12.0 + row as f32 * LINE_HEIGHT,
                true,
            );
        }
    }
    p.pop_clip(clip);
    draw_scrollbar(
        p,
        packs.details_scroll,
        row_left,
        row_w,
        list_y,
        list_h,
        content_h,
    );

    let back = Button::new(
        centered_row_x(ctx.vw, FOOTER_W),
        layout.footer_y(ctx.vh),
        FOOTER_W,
        WIDGET_HEIGHT,
        label!("gui.back"),
    );
    if back.draw(p, ctx) || ctx.input.escape {
        packs.prompt = Some(Prompt::Warning);
    }
}

fn draw_icon(p: &mut Painter, pack: &Pack, x: f32, y: f32) {
    if pack.id == "vanilla" {
        p.sprite("pack_icon", x, y, ICON, ICON);
        return;
    }
    let key = format!("\u{0}datapack/{}", pack.id);
    let slot = pack
        .icon
        .as_ref()
        .and_then(|pixels| crate::gui::ping::ensure_icon(&key, pixels));
    match slot.and_then(|slot| p.atlas.server_icon(slot)) {
        Some(region) => p.region(region, x, y, ICON, ICON),
        None => p.sprite("unknown_pack", x, y, ICON, ICON),
    }
}

fn list_header(p: &mut Painter, title: &str, left: f32, top: f32, list_w: f32) {
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
        top + (LIST_HEADER_H - LINE_HEIGHT) / 2.0 + 1.0,
        true,
    );
}

fn entry_text(p: &mut Painter, title: &str, description: &[Span], cx: f32, cy: f32, w: f32) {
    let x = cx + ICON + PADDING;
    let title = p
        .atlas
        .font
        .trim_to_width(&[Span::colored(title, 0xFF_FFFF)], w);
    p.text(&title, x, cy + 1.0, true);
    let lines = p.atlas.font.wrap(description, w);
    for (i, line) in lines.iter().take(2).enumerate() {
        p.text(line, x, cy + 12.0 + i as f32 * LINE_HEIGHT, true);
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
