use crate::gui::focus;
use crate::gui::options::{self, SCROLLBAR_GAP};
use crate::gui::painter::Painter;
use crate::gui::ping::{self, PingState};
use crate::gui::serverlist::{self, PackPolicy, ServerEntry};
use crate::gui::tooltip::label;
use crate::gui::widgets::{self, Button, HeaderFooter, TextBox, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{GuiState, Screen, ScreenCtx, menu};
use crate::text::Style;

const TITLE: &str = "Play Multiplayer";
const JOIN: &str = "Join Server";
const DIRECT: &str = "Direct Connection";
const ADD: &str = "Add Server";
const EDIT: &str = "Edit";
const DELETE: &str = "Delete";
const REFRESH: &str = "Refresh";
const BACK: &str = "Back";
const DONE: &str = "Done";
const CANCEL: &str = "Cancel";
const MANAGE_TITLE: &str = "Edit Server Info";
const NAME_LABEL: &str = "Server Name";
const ADDRESS_LABEL: &str = "Server Address";
const DEFAULT_NAME: &str = "Minecraft Server";
const DELETE_QUESTION: &str = "Are you sure you want to remove this server?";
const PINGING: &str = "Pinging...";
const CANNOT_CONNECT: &str = "Can't connect to server";
const NO_CONNECTION: &str = "(no connection)";
const INCOMPATIBLE: &str = "Incompatible version!";

const LAYOUT: HeaderFooter = HeaderFooter::new(HeaderFooter::DEFAULT_H, 60.0);
const FOOTER_H: f32 = LAYOUT.footer_h;
const ROW_W: f32 = 305.0;
const ROW_H: f32 = 36.0;
const CONTENT_PAD: f32 = 2.0;
const ICON_SIZE: f32 = 32.0;
const TEXT_INSET: f32 = ICON_SIZE + 3.0;
const PING_W: f32 = 10.0;
const PING_H: f32 = 8.0;
const SPACING: f32 = 5.0;
const TOP_ROW_BUTTON_W: f32 = 100.0;
const LOWER_ROW_BUTTON_W: f32 = 74.0;
const BUTTON_GAP: f32 = 4.0;
const GREY: u32 = 0x808080;
const DARK_RED: u32 = 0xAA0000;
const VERSION_W: f32 = 64.0;
const VERSION_X: f32 = 4.0;
const VERSION_Y: f32 = 4.0;
const VERSION_GAP: f32 = 2.0;

const ADDRESS_MAX_LEN: usize = 128;
const NAME_MAX_LEN: usize = 32;

pub struct MenuState {
    pub servers: Vec<ServerEntry>,
    loaded: bool,
    pub selected: Option<usize>,
    pub scroll: f32,
    scrollbar_drag: Option<f32>,
    editing: Option<usize>,
    name: TextBox,
    address: TextBox,
    resource_packs: PackPolicy,
    direct: TextBox,
    confirm_delete: Option<usize>,
    confirm_delete_text: String,
    version_open: bool,
}

impl Default for MenuState {
    fn default() -> MenuState {
        MenuState {
            servers: Vec::new(),
            loaded: false,
            selected: None,
            scroll: 0.0,
            scrollbar_drag: None,
            editing: None,
            name: TextBox::bordered(NAME_MAX_LEN, DEFAULT_NAME),
            address: TextBox::bordered(ADDRESS_MAX_LEN, ""),
            resource_packs: PackPolicy::default(),
            direct: TextBox::bordered(ADDRESS_MAX_LEN, ""),
            confirm_delete: None,
            confirm_delete_text: String::new(),
            version_open: false,
        }
    }
}

impl MenuState {
    pub fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.servers = serverlist::load();
        self.loaded = true;
    }

    pub fn save(&self) {
        serverlist::save(&self.servers);
    }

    fn begin_add(&mut self) {
        self.editing = None;
        self.name.set_text(DEFAULT_NAME);
        self.address.set_text("");
        self.resource_packs = PackPolicy::default();
        self.focus_name();
    }

    fn begin_edit(&mut self, index: usize) {
        let Some(entry) = self.servers.get(index) else {
            return;
        };
        self.editing = Some(index);
        self.name.set_text(&entry.name);
        self.address.set_text(&entry.address);
        self.resource_packs = entry.resource_packs;
        self.focus_name();
    }

    pub fn set_pack_policy(&mut self, address: &str, policy: PackPolicy) {
        self.ensure_loaded();
        let mut changed = false;
        for entry in self.servers.iter_mut().filter(|e| e.address == address) {
            changed |= entry.resource_packs != policy;
            entry.resource_packs = policy;
        }
        if changed {
            self.save();
        }
    }

    fn focus_name(&mut self) {
        self.name.focus();
        self.address.focused = false;
    }

    fn commit_form(&mut self) {
        let name = self.name.text.trim().to_string();
        let address = self.address.text.trim().to_string();
        if address.is_empty() {
            return;
        }
        let resource_packs = self.resource_packs;
        match self.editing {
            Some(i) if i < self.servers.len() => {
                ping::forget(&self.servers[i].address);
                self.servers[i] = ServerEntry {
                    name,
                    address,
                    resource_packs,
                };
            }
            Some(_) => {}
            None => {
                self.servers.push(ServerEntry {
                    name,
                    address,
                    resource_packs,
                });
                self.selected = Some(self.servers.len() - 1);
            }
        }
    }

    fn swap(&mut self, index: usize, delta: i32) {
        let other = index as i32 + delta;
        if other < 0 || other as usize >= self.servers.len() {
            return;
        }
        let other = other as usize;
        self.servers.swap(index, other);
        self.selected = Some(other);
    }

    fn delete(&mut self, index: usize) {
        if index >= self.servers.len() {
            return;
        }
        ping::forget(&self.servers[index].address);
        self.servers.remove(index);
        self.selected = None;
    }

    fn content_height(&self) -> f32 {
        self.servers.len() as f32 * ROW_H + 2.0 * CONTENT_PAD
    }

    fn max_scroll(&self, list_h: f32) -> f32 {
        (self.content_height() - list_h).max(0.0)
    }

    fn clamp_scroll(&mut self, list_h: f32) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll(list_h));
    }
}

pub fn valid_address(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    if crate::platform::address::is_websocket(s) {
        return crate::platform::address::EAGLER;
    }
    azalea_protocol::address::ServerAddr::try_from(s).is_ok()
}

fn row_left(vw: f32) -> f32 {
    ((vw / 2.0).floor() - (ROW_W / 2.0).floor()).floor()
}

fn scrollbar_x(vw: f32) -> f32 {
    row_left(vw) + ROW_W + SCROLLBAR_GAP
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    state.menu.ensure_loaded();
    menu::background(p, ctx.vw, ctx.vh);

    p.text_centered(TITLE, ctx.vw / 2.0, LAYOUT.title_y(), 0xFFFFFF, true);

    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    menu::list_background(p, lx, ly, lw, lh, state.menu.scroll);

    let confirming = state.menu.confirm_delete.is_some();
    let dropped = state.menu.version_open && !confirming;
    let locked = confirming || dropped;
    draw_rows(p, state, ctx, locked);
    draw_scrollbar(p, &mut state.menu, ctx, lh, locked);
    draw_footer(p, state, ctx, locked);
    draw_version_menu(p, state, ctx, confirming);

    if confirming {
        draw_delete_confirm(p, state, ctx);
    } else if !dropped {
        handle_list_keys(state, ctx);
    }
}

fn draw_version_menu(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, locked: bool) {
    use crate::protocol::version;

    let current = version::selectable()
        .find(|v| v.protocol == crate::protocol::preselected())
        .unwrap_or(version::NATIVE);

    let mut open = state.menu.version_open && !locked;
    #[cfg(feature = "tutorial")]
    crate::gui::tutorial::mark(
        crate::gui::tutorial::Spot::Version,
        crate::gui::hud_layout::Rect::new(VERSION_X, VERSION_Y, VERSION_W, WIDGET_HEIGHT),
    );
    let head = Button {
        x: VERSION_X,
        y: VERSION_Y,
        w: VERSION_W,
        h: WIDGET_HEIGHT,
        label: current.name,
        active: !locked,
    };
    if head.draw(p, ctx) {
        open = !open;
    }
    state.menu.version_open = open;
    if !open {
        return;
    }

    let step = WIDGET_HEIGHT + VERSION_GAP;
    let mut picked = None;
    let mut count = 0.0;
    for (i, v) in version::selectable().enumerate() {
        let y = VERSION_Y + (i as f32 + 1.0) * step;
        if Button::new(VERSION_X, y, VERSION_W, WIDGET_HEIGHT, v.name).draw(p, ctx) {
            picked = Some(v.protocol);
        }
        count = i as f32 + 1.0;
    }

    if let Some(protocol) = picked {
        crate::protocol::set_forced(Some(protocol));
        state.menu.version_open = false;
        return;
    }
    let menu_h = (count + 1.0) * step;
    if ctx.input.escape
        || (ctx.input.left_click && !ctx.hovering(VERSION_X, VERSION_Y, VERSION_W, menu_h))
    {
        state.menu.version_open = false;
    }
}

fn row_rect(vw: f32, ly: f32, scroll: f32, index: usize) -> (f32, f32) {
    (
        row_left(vw),
        ly + CONTENT_PAD - scroll + index as f32 * ROW_H,
    )
}

fn draw_rows(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, locked: bool) {
    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let GuiState {
        menu: st,
        nav,
        connect,
        ..
    } = state;
    st.clamp_scroll(lh);

    let mut click_join: Option<usize> = None;
    let mut swap: Option<(usize, i32)> = None;

    let list_focused = focus::next(!locked && !st.servers.is_empty());
    let guard = p.push_clip(lx, ly, lw, lh);
    for i in 0..st.servers.len() {
        let (rx, ry) = row_rect(ctx.vw, ly, st.scroll, i);
        if ry + ROW_H < ly || ry > ly + lh {
            continue;
        }
        let hovered = !locked && ctx.hovering(rx, ry, ROW_W, ROW_H);
        let selected = st.selected == Some(i);

        if selected {
            let border = if list_focused {
                0xFFFF_FFFF
            } else {
                0xFF80_8080
            };
            p.fill(rx, ry, ROW_W, ROW_H, border);
            p.fill(rx + 1.0, ry + 1.0, ROW_W - 2.0, ROW_H - 2.0, 0xFF00_0000);
        }

        let (cx, cy) = (rx + CONTENT_PAD, ry + CONTENT_PAD);
        let cw = ROW_W - 2.0 * CONTENT_PAD;
        draw_row_content(p, &st.servers[i], cx, cy, cw);

        if hovered {
            let action = icon_action(ctx, cx, cy);
            draw_icon_overlay(p, cx, cy, action, i, st.servers.len());
            if ctx.input.left_click
                && let Some(action) = action
            {
                match action {
                    IconAction::Join => click_join = Some(i),
                    IconAction::MoveUp => swap = Some((i, -1)),
                    IconAction::MoveDown => swap = Some((i, 1)),
                }
            }
        }

        if !locked && ctx.input.left_click && ctx.hovering(rx, ry, ROW_W, ROW_H) {
            st.selected = Some(i);
            focus::claim();
            if ctx.input.double_click {
                click_join = Some(i);
            }
        }
    }
    p.pop_clip(guard);

    if let Some((i, delta)) = swap {
        st.swap(i, delta);
        st.save();
    }
    if let Some(i) = click_join
        && let Some(entry) = st.servers.get(i)
    {
        *connect = Some(entry.address.clone());
        *nav = Some(Screen::None);
    }
}

fn draw_row_content(p: &mut Painter, entry: &ServerEntry, cx: f32, cy: f32, cw: f32) {
    ping::request(&entry.address);
    let state = ping::get(&entry.address);

    let icon = ping::icon_slot(&entry.address).and_then(|slot| p.atlas.server_icon(slot));
    match icon {
        Some(region) => p.region(region, cx, cy, ICON_SIZE, ICON_SIZE),
        None => p.sprite("unknown_server", cx, cy, ICON_SIZE, ICON_SIZE),
    }

    let text_x = cx + TEXT_INSET;
    let text_w = cw - TEXT_INSET;
    p.text_str(
        serverlist::display_name(entry),
        text_x,
        cy + 1.0,
        0xFFFFFF,
        true,
    );
    p.text_str(&entry.address, text_x, cy + 12.0, GREY, true);

    let font = &p.atlas.font;
    match &state {
        Some(PingState::Ok(info)) => {
            let spans = font.trim_to_width(&info.motd, text_w);
            p.text(&spans, text_x, cy + 23.0, true);
        }
        Some(PingState::Failed(reason)) => {
            let line = format!("{CANNOT_CONNECT}: {reason}");
            let shown = font.trim_to_width(
                &crate::text::styled(&line, Style::colored(DARK_RED)),
                text_w,
            );
            p.text(&shown, text_x, cy + 23.0, true);
        }
        _ => {
            p.text_str(PINGING, text_x, cy + 23.0, GREY, true);
        }
    }

    let icon_x = cx + cw - PING_W - SPACING;
    let (sprite, status, color) = match &state {
        Some(PingState::Ok(info)) if info.incompatible() => (
            "server_list/incompatible".to_string(),
            INCOMPATIBLE.to_string(),
            DARK_RED,
        ),
        Some(PingState::Ok(info)) => (
            format!("server_list/ping_{}", info.bars()),
            format!("{}/{}", info.online, info.max),
            GREY,
        ),
        Some(PingState::Failed(_)) => (
            "server_list/unreachable".to_string(),
            NO_CONNECTION.to_string(),
            GREY,
        ),
        _ => {
            let step = (p.frame / 3) % 5 + 1;
            (format!("server_list/pinging_{step}"), String::new(), GREY)
        }
    };
    p.sprite(&sprite, icon_x, cy, PING_W, PING_H);
    if !status.is_empty() {
        let w = p.atlas.font.width_str(&status);
        p.text_str(&status, icon_x - w - SPACING, cy + 1.0, color, true);
    }
    if let Some(PingState::Ok(info)) = &state {
        let ms = format!("{} ms", info.latency_ms);
        let w = p.atlas.font.width_str(&ms);
        p.text_str(&ms, cx + cw - w, cy + 12.0, GREY, true);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IconAction {
    Join,
    MoveUp,
    MoveDown,
}

fn icon_action(ctx: &ScreenCtx, cx: f32, cy: f32) -> Option<IconAction> {
    let m = ctx.mouse()?;
    if m.x < cx || m.x >= cx + ICON_SIZE || m.y < cy || m.y >= cy + ICON_SIZE {
        return None;
    }
    if m.x - cx >= ICON_SIZE / 2.0 {
        Some(IconAction::Join)
    } else if m.y - cy < ICON_SIZE / 2.0 {
        Some(IconAction::MoveUp)
    } else {
        Some(IconAction::MoveDown)
    }
}

fn draw_icon_overlay(
    p: &mut Painter,
    cx: f32,
    cy: f32,
    action: Option<IconAction>,
    index: usize,
    count: usize,
) {
    p.fill(cx, cy, ICON_SIZE, ICON_SIZE, 0xA050_5050);
    let sprite = match action {
        Some(IconAction::Join) => "server_list/join_highlighted",
        Some(IconAction::MoveUp) if index > 0 => "server_list/move_up_highlighted",
        Some(IconAction::MoveDown) if index + 1 < count => "server_list/move_down_highlighted",
        _ => return,
    };
    p.sprite(sprite, cx, cy, ICON_SIZE, ICON_SIZE);
}

fn draw_scrollbar(p: &mut Painter, st: &mut MenuState, ctx: &ScreenCtx, list_h: f32, locked: bool) {
    let content_h = st.content_height();
    list_scrollbar(
        p,
        ctx,
        &mut st.scroll,
        &mut st.scrollbar_drag,
        content_h,
        list_h,
        locked,
    );
}

pub(in crate::gui) fn list_scrollbar(
    p: &mut Painter,
    ctx: &ScreenCtx,
    scroll: &mut f32,
    bar_drag: &mut Option<f32>,
    content_h: f32,
    list_h: f32,
    locked: bool,
) {
    let (_, ly, _, _) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let left = scrollbar_x(ctx.vw) - SCROLLBAR_GAP;
    if locked {
        let max = (content_h - list_h).max(0.0);
        *scroll = scroll.clamp(0.0, max);
    } else {
        options::scroll_input(scroll, bar_drag, ctx, left, 0.0, ly, list_h, content_h);
    }
    options::draw_scrollbar(p, *scroll, left, 0.0, ly, list_h, content_h);
}

fn footer_row<const N: usize>(
    p: &mut Painter,
    ctx: &ScreenCtx,
    locked: bool,
    w: f32,
    y: f32,
    buttons: [(&str, bool); N],
) -> [bool; N] {
    let step = w + BUTTON_GAP;
    let x0 = widgets::centered_row_x(ctx.vw, N as f32 * step - BUTTON_GAP);
    let mut clicked = [false; N];
    for (i, (label, active)) in buttons.into_iter().enumerate() {
        clicked[i] = Button {
            x: x0 + i as f32 * step,
            y,
            w,
            h: WIDGET_HEIGHT,
            label,
            active: active && !locked,
        }
        .draw(p, ctx);
    }
    clicked
}

fn draw_footer(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, locked: bool) {
    let top = ctx.vh - FOOTER_H + ((FOOTER_H - 2.0 * WIDGET_HEIGHT - BUTTON_GAP) / 2.0).floor();
    let bottom = top + WIDGET_HEIGHT + BUTTON_GAP;

    let selected = state
        .menu
        .selected
        .filter(|i| *i < state.menu.servers.len());
    let has_selection = selected.is_some();

    let [join, direct, add] = footer_row(
        p,
        ctx,
        locked,
        TOP_ROW_BUTTON_W,
        top,
        [(JOIN, has_selection), (DIRECT, true), (ADD, true)],
    );
    let [edit, delete, refresh, back] = footer_row(
        p,
        ctx,
        locked,
        LOWER_ROW_BUTTON_W,
        bottom,
        [
            (EDIT, has_selection),
            (DELETE, has_selection),
            (REFRESH, true),
            (BACK, true),
        ],
    );

    if join && let Some(i) = selected {
        state.connect = Some(state.menu.servers[i].address.clone());
        state.nav = Some(Screen::None);
    }
    if direct {
        state.menu.direct.focus();
        state.nav = Some(Screen::DirectConnect);
    }
    if add {
        state.menu.begin_add();
        state.nav = Some(Screen::ManageServer);
    }
    if edit && let Some(i) = selected {
        state.menu.begin_edit(i);
        state.nav = Some(Screen::ManageServer);
    }
    if delete && let Some(i) = selected {
        state.menu.confirm_delete_text = format!(
            "'{}' will be lost forever! (A long time!)",
            serverlist::display_name(&state.menu.servers[i])
        );
        state.menu.confirm_delete = Some(i);
    }
    if refresh {
        ping::refresh();
    }
    if back {
        state.nav = Some(Screen::Title);
    }
}

fn handle_list_keys(state: &mut GuiState, ctx: &ScreenCtx) {
    let input = ctx.input;
    if input.escape {
        state.nav = Some(Screen::Title);
        return;
    }
    let count = state.menu.servers.len();
    if count > 0 && (input.up_arrow || input.down_arrow) {
        let delta = if input.up_arrow { -1i32 } else { 1 };
        let next = match state.menu.selected {
            Some(i) => (i as i32 + delta).clamp(0, count as i32 - 1) as usize,
            None if delta > 0 => 0,
            None => count - 1,
        };
        state.menu.selected = Some(next);
    }
    if input.enter
        && let Some(i) = state.menu.selected
        && let Some(entry) = state.menu.servers.get(i)
    {
        state.connect = Some(entry.address.clone());
        state.nav = Some(Screen::None);
    }
}

fn draw_delete_confirm(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let Some(index) = state.menu.confirm_delete else {
        return;
    };

    p.fill(0.0, 0.0, ctx.vw, ctx.vh, 0xC0_000000);

    let cy = (ctx.vh / 2.0).floor();
    let mid = ctx.vw / 2.0;
    p.text_centered(DELETE_QUESTION, mid, cy - 40.0, 0xFFFFFF, true);
    p.text_centered(&state.menu.confirm_delete_text, mid, cy - 25.0, GREY, true);

    let x = widgets::centered_x(ctx.vw);
    if Button::new(x, cy, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, DELETE).draw(p, ctx) {
        state.menu.delete(index);
        state.menu.save();
        state.menu.confirm_delete = None;
    }
    if Button::new(x, cy + 24.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, CANCEL).draw(p, ctx)
        || ctx.input.escape
    {
        state.menu.confirm_delete = None;
    }
}

struct Form {
    title: &'static str,
    title_y: f32,
    ok_label: &'static str,
    button_y: f32,
}

fn draw_form(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    form: &Form,
    fields: impl FnOnce(&mut Painter, &mut MenuState, &ScreenCtx, f32) -> bool,
) -> (bool, bool) {
    state.menu.ensure_loaded();
    menu::background(p, ctx.vw, ctx.vh);

    let x = widgets::centered_x(ctx.vw);
    p.text_centered(form.title, ctx.vw / 2.0, form.title_y, 0xFFFFFF, true);
    let valid = fields(p, &mut state.menu, ctx, x);

    let base = (ctx.vh / 4.0).floor() + form.button_y;
    let ok = Button {
        x,
        y: base,
        w: WIDGET_WIDTH_BIG,
        h: WIDGET_HEIGHT,
        label: form.ok_label,
        active: valid,
    };
    let cancel = Button::new(x, base + 24.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, CANCEL);

    let clicked = ok.draw(p, ctx);
    let cancelled = cancel.draw(p, ctx) || ctx.input.escape;
    ((clicked || ctx.input.enter) && valid, cancelled)
}

pub fn draw_manage(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let quarter = (ctx.vh / 4.0).floor();

    #[cfg(feature = "mobile_ui")]
    let (name_label, name_y, address_label, address_y) = (33.0, 46.0, 74.0, 86.0);
    #[cfg(not(feature = "mobile_ui"))]
    let (name_label, name_y, address_label, address_y) = (53.0, 66.0, 94.0, 106.0);
    let below_address = address_y + WIDGET_HEIGHT + 4.0;
    #[cfg(feature = "mobile_ui")]
    let (cycle_y, ok_y) = (below_address, below_address + 24.0);
    #[cfg(not(feature = "mobile_ui"))]
    let (cycle_y, ok_y) = {
        let cycle_y = (quarter + 72.0).max(below_address);
        (cycle_y, cycle_y + 42.0)
    };

    let form = Form {
        title: MANAGE_TITLE,
        title_y: 17.0,
        ok_label: DONE,
        button_y: ok_y - quarter,
    };
    let (confirmed, cancelled) = draw_form(p, state, ctx, &form, |p, menu, ctx, x| {
        p.text_str(NAME_LABEL, x + 1.0, name_label, 0xA0A0A0, true);
        p.text_str(ADDRESS_LABEL, x + 1.0, address_label, 0xA0A0A0, true);

        menu.name
            .update(p, ctx, x, name_y, WIDGET_WIDTH_BIG, WIDGET_HEIGHT);
        menu.address
            .update(p, ctx, x, address_y, WIDGET_WIDTH_BIG, WIDGET_HEIGHT);

        let label = format!(
            "{}: {}",
            label!("manageServer.resourcePack"),
            menu.resource_packs.caption()
        );
        if Button::new(x, cycle_y, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, &label).draw(p, ctx) {
            menu.resource_packs = menu.resource_packs.next();
        }
        valid_address(&menu.address.text)
    });

    if confirmed {
        state.menu.commit_form();
        state.menu.save();
        state.nav = Some(Screen::Multiplayer);
    }
    if cancelled {
        state.nav = Some(Screen::Multiplayer);
    }
}

pub fn draw_direct(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let form = Form {
        title: DIRECT,
        title_y: 20.0,
        ok_label: JOIN,
        button_y: 108.0,
    };
    let (confirmed, cancelled) = draw_form(p, state, ctx, &form, direct_fields);

    if confirmed {
        state.connect = Some(state.menu.direct.text.trim().to_string());
        state.nav = Some(Screen::None);
    }
    if cancelled {
        state.nav = Some(Screen::Multiplayer);
    }
}

fn direct_fields(p: &mut Painter, menu: &mut MenuState, ctx: &ScreenCtx, x: f32) -> bool {
    #[cfg(feature = "mobile_ui")]
    let (label_y, field_y) = (53.0, 66.0);
    #[cfg(not(feature = "mobile_ui"))]
    let (label_y, field_y) = (100.0, 116.0);
    p.text_str(ADDRESS_LABEL, x + 1.0, label_y, 0xA0A0A0, true);

    menu.direct
        .update(p, ctx, x, field_y, WIDGET_WIDTH_BIG, WIDGET_HEIGHT);
    valid_address(&menu.direct.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_validation_matches_azalea() {
        assert!(valid_address("localhost"));
        assert!(valid_address("127.0.0.1:25565"));
        assert!(valid_address("  example.com:25565  "));
        assert!(!valid_address(""));
        assert!(!valid_address("   "));
        assert!(!valid_address("example.com:not-a-port"));
        assert!(!valid_address("example.com:70000"));
    }

    #[test]
    fn row_and_scrollbar_geometry() {
        assert_eq!(row_left(854.0), 275.0);
        assert_eq!(scrollbar_x(854.0), 275.0 + 305.0 + 8.0);
    }

    #[test]
    fn list_fills_between_header_and_footer() {
        let (x, y, w, h) = LAYOUT.content_rect(854.0, 480.0);
        assert_eq!((x, y, w), (0.0, 33.0, 854.0));
        assert_eq!(h, 480.0 - 33.0 - 60.0);
    }

    #[test]
    fn scroll_is_clamped_to_the_content() {
        let mut st = MenuState::default();
        assert_eq!(st.max_scroll(200.0), 0.0);
        st.servers = (0..10)
            .map(|i| ServerEntry {
                name: format!("s{i}"),
                address: "localhost".into(),
                ..Default::default()
            })
            .collect();
        assert_eq!(st.max_scroll(200.0), 10.0 * 36.0 + 4.0 - 200.0);
        st.scroll = 10_000.0;
        st.clamp_scroll(200.0);
        assert_eq!(st.scroll, 164.0);
    }

    #[test]
    fn swapping_follows_the_moved_entry() {
        let mut st = MenuState::default();
        st.loaded = true;
        st.servers = ["a", "b", "c"]
            .iter()
            .map(|n| ServerEntry {
                name: n.to_string(),
                address: format!("{n}.example.com"),
                ..Default::default()
            })
            .collect();
        st.swap(2, -1);
        assert_eq!(st.selected, Some(1));
        assert_eq!(st.servers[1].name, "c");
        st.swap(0, -1);
        st.swap(2, 1);
        assert_eq!(st.servers.len(), 3);
    }

    #[test]
    fn editing_a_row_that_is_gone_adds_nothing() {
        let mut st = MenuState::default();
        st.loaded = true;
        st.servers = vec![ServerEntry {
            name: "a".into(),
            address: "a.example.com".into(),
            ..Default::default()
        }];
        st.begin_edit(0);
        st.servers.clear();
        st.name.set_text("a");
        st.address.set_text("a.example.com");
        st.commit_form();
        assert!(st.servers.is_empty());
    }

    #[test]
    fn the_form_adds_when_adding_and_replaces_when_editing() {
        let mut st = MenuState::default();
        st.loaded = true;
        st.begin_add();
        st.name.set_text("first");
        st.address.set_text("one.example.com");
        st.commit_form();
        assert_eq!(st.servers.len(), 1);
        assert_eq!(st.selected, Some(0));

        st.begin_edit(0);
        st.name.set_text("renamed");
        st.address.set_text("two.example.com");
        st.commit_form();
        assert_eq!(st.servers.len(), 1);
        assert_eq!(st.servers[0].name, "renamed");
        assert_eq!(st.servers[0].address, "two.example.com");
    }
}
