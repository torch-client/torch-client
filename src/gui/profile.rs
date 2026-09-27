use crate::gui::accountlist::{self, Account, AccountKind, Accounts};
use crate::gui::focus;
use crate::gui::painter::Painter;
use crate::gui::widgets::{self, Button, HeaderFooter, TextBox, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{GuiState, Screen, ScreenCtx, menu};
use crate::platform::cli::valid_username;

const TITLE: &str = "Accounts";
const SELECT: &str = "Select Account";
const ADD: &str = "Add Account";
const ADD_TITLE: &str = "Add Account";
const EDIT_TITLE: &str = "Edit Account";
const USERNAME_LABEL: &str = "Username";
const EDIT: &str = "Edit";
const DELETE: &str = "Delete";
const BACK: &str = "Back";
const DONE: &str = "Done";
const CANCEL: &str = "Cancel";
const IN_USE: &str = "Selected";
const EMPTY: &str = "No accounts yet. Add one to choose a name to join under.";
const DELETE_QUESTION: &str = "Are you sure you want to remove this account?";

const USERNAME_MAX_LEN: usize = 16;

const LAYOUT: HeaderFooter = HeaderFooter::new(HeaderFooter::DEFAULT_H, 60.0);
const FOOTER_H: f32 = LAYOUT.footer_h;
const ROW_W: f32 = 305.0;
const ROW_H: f32 = 36.0;
const CONTENT_PAD: f32 = 2.0;
const ICON_SIZE: f32 = 32.0;
const TEXT_INSET: f32 = ICON_SIZE + 3.0;
const SCROLLBAR_W: f32 = 6.0;
const SCROLLBAR_MIN_H: f32 = 32.0;
const BUTTON_GAP: f32 = 4.0;
const LOWER_ROW_BUTTON_W: f32 = 74.0;
const BAR_W: f32 = 3.0 * LOWER_ROW_BUTTON_W + 2.0 * BUTTON_GAP;
const TOP_ROW_BUTTON_W: f32 = (BAR_W - BUTTON_GAP) / 2.0;
const GREY: u32 = 0x808080;
const GREEN: u32 = 0x55FF55;
const AQUA: u32 = 0x55FFFF;

pub struct ProfileState {
    pub accounts: Accounts,
    loaded: bool,
    pub selected: Option<usize>,
    pub scroll: f32,
    scrollbar_drag: bool,
    editing: Option<usize>,
    username: TextBox,
    confirm_delete: Option<usize>,
    pub first_run: bool,
    pub pending_connect: Option<String>,
    #[cfg(feature = "online_mode")]
    copied_link: bool,
}

impl Default for ProfileState {
    fn default() -> ProfileState {
        ProfileState {
            accounts: Accounts::default(),
            loaded: false,
            selected: None,
            scroll: 0.0,
            scrollbar_drag: false,
            editing: None,
            username: TextBox::bordered(USERNAME_MAX_LEN, ""),
            confirm_delete: None,
            first_run: false,
            pending_connect: None,
            #[cfg(feature = "online_mode")]
            copied_link: false,
        }
    }
}

impl ProfileState {
    pub fn starting(first_run: bool, pending_connect: Option<String>) -> ProfileState {
        let mut state = ProfileState {
            first_run,
            pending_connect,
            ..Default::default()
        };
        if first_run && entry_screen() == Screen::EditProfile {
            state.username.focus();
        }
        state
    }

    pub fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.accounts = accountlist::load();
        self.loaded = true;
    }

    pub fn open(&mut self) {
        self.ensure_loaded();
        self.selected = self.accounts.active;
        self.confirm_delete = None;
    }

    pub fn save(&self) {
        accountlist::save(&self.accounts);
    }

    fn begin_add(&mut self) {
        self.editing = None;
        self.username.set_text("");
        self.username.focus();
    }

    fn begin_edit(&mut self, index: usize) {
        let Some(account) = self.accounts.get(index).filter(|a| a.kind.editable()) else {
            return;
        };
        let name = account.name.clone();
        self.editing = Some(index);
        self.username.set_text(&name);
        self.username.focus();
    }

    fn commit_form(&mut self) {
        let name = self.username.text.trim().to_string();
        if !valid_username(&name) {
            return;
        }
        match self.editing {
            Some(i) if i < self.accounts.entries.len() => {
                self.accounts.entries[i].name = name;
                self.selected = Some(i);
            }
            _ => {
                self.accounts.add(Account {
                    name,
                    kind: AccountKind::Offline,
                    ..Account::default()
                });
                self.selected = self.accounts.active;
            }
        }
        self.save();
        self.publish();
    }

    fn publish(&self) {
        if let Some(account) = self.accounts.active() {
            crate::client::bot::set_username(account.name.clone());
        }
    }

    fn content_height(&self) -> f32 {
        self.accounts.entries.len() as f32 * ROW_H + 2.0 * CONTENT_PAD
    }

    fn max_scroll(&self, list_h: f32) -> f32 {
        (self.content_height() - list_h).max(0.0)
    }

    fn clamp_scroll(&mut self, list_h: f32) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll(list_h));
    }
}

fn leave(state: &mut GuiState, back: Screen) {
    if !state.profile.first_run {
        state.nav = Some(back);
        return;
    }
    state.profile.first_run = false;
    match state.profile.pending_connect.take() {
        Some(address) => {
            state.connect = Some(address);
            state.nav = Some(Screen::None);
        }
        None => state.nav = Some(Screen::Title),
    }
}

pub fn entry_screen() -> Screen {
    #[cfg(feature = "online_mode")]
    {
        Screen::AddAccount
    }
    #[cfg(not(feature = "online_mode"))]
    {
        Screen::EditProfile
    }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    state.profile.ensure_loaded();
    menu::background(p, ctx.vw, ctx.vh);
    widgets::draw_title(p, ctx.vw, LAYOUT.title_y(), TITLE);

    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    menu::list_background(p, lx, ly, lw, lh, state.profile.scroll);

    let confirming = state.profile.confirm_delete.is_some();
    draw_rows(p, state, ctx, confirming);
    draw_scrollbar(p, state, ctx, lh, confirming);
    draw_footer(p, state, ctx, confirming);

    if confirming {
        draw_delete_confirm(p, state, ctx);
    } else {
        handle_list_keys(state, ctx);
    }
}

fn row_left(vw: f32) -> f32 {
    ((vw / 2.0).floor() - (ROW_W / 2.0).floor()).floor()
}

fn scrollbar_x(vw: f32) -> f32 {
    row_left(vw) + ROW_W + SCROLLBAR_W + 2.0
}

fn draw_rows(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, locked: bool) {
    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let st = &mut state.profile;
    st.clamp_scroll(lh);

    let list_focused = focus::next(!locked && !st.accounts.entries.is_empty());

    if st.accounts.entries.is_empty() {
        let w = p.atlas.font.width_str(EMPTY);
        p.text_str(
            EMPTY,
            ((ctx.vw - w) / 2.0).floor(),
            (ly + lh / 2.0 - 4.0).floor(),
            GREY,
            true,
        );
        return;
    }

    let mut activate: Option<usize> = None;

    let guard = p.push_clip(lx, ly, lw, lh);
    for i in 0..st.accounts.entries.len() {
        let rx = row_left(ctx.vw);
        let ry = ly + CONTENT_PAD - st.scroll + i as f32 * ROW_H;
        if ry + ROW_H < ly || ry > ly + lh {
            continue;
        }
        if st.selected == Some(i) {
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
        draw_row_content(
            p,
            &st.accounts.entries[i],
            st.accounts.active == Some(i),
            cx,
            cy,
            cw,
        );

        if !locked && ctx.input.left_click && ctx.hovering(rx, ry, ROW_W, ROW_H) {
            st.selected = Some(i);
            focus::claim();
            if ctx.input.double_click {
                activate = Some(i);
            }
        }
    }
    p.pop_clip(guard);

    if let Some(i) = activate {
        select(state, i);
    }
}

fn draw_row_content(p: &mut Painter, account: &Account, active: bool, cx: f32, cy: f32, cw: f32) {
    p.sprite("pack_icon", cx, cy, ICON_SIZE, ICON_SIZE);

    let text_x = cx + TEXT_INSET;
    p.text_str(
        &account.name,
        text_x,
        cy + 6.0,
        if active { GREEN } else { 0xFFFFFF },
        true,
    );
    let color = match account.kind {
        AccountKind::Microsoft => AQUA,
        AccountKind::Offline => GREY,
    };
    p.text_str(account.kind.label(), text_x, cy + 18.0, color, true);

    if active {
        let w = p.atlas.font.width_str(IN_USE);
        p.text_str(IN_USE, cx + cw - w, cy + 6.0, GREEN, true);
    }
}

fn draw_scrollbar(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    list_h: f32,
    locked: bool,
) {
    let (_, ly, _, _) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let st = &mut state.profile;
    let max = st.max_scroll(list_h);

    if !locked {
        if ctx.input.scroll != 0.0 && ctx.mouse().is_some() {
            st.scroll -= ctx.input.scroll * (ROW_H / 2.0);
        }
        if ctx.input.left_release {
            st.scrollbar_drag = false;
        }
    }
    if max <= 0.0 {
        st.scroll = 0.0;
        st.scrollbar_drag = false;
        return;
    }

    let x = scrollbar_x(ctx.vw);
    let thumb_h = ((list_h * list_h) / st.content_height()).clamp(SCROLLBAR_MIN_H, list_h - 8.0);

    #[cfg(feature = "mobile_ui")]
    if !locked {
        let over_bar = ctx.hovering(x, ly, SCROLLBAR_W, list_h);
        crate::gui::options::content_drag(&mut st.scroll, ctx, ly, list_h, over_bar, max);
    }

    if !locked && ctx.input.left_click && ctx.hovering(x, ly, SCROLLBAR_W, list_h) {
        st.scrollbar_drag = true;
    }
    if st.scrollbar_drag
        && ctx.input.left_down
        && let Some(m) = ctx.mouse()
    {
        let travel = (list_h - thumb_h).max(1.0);
        st.scroll = ((m.y - ly - thumb_h / 2.0) / travel * max).clamp(0.0, max);
    }
    st.clamp_scroll(list_h);

    let thumb_y = ly + st.scroll / max * (list_h - thumb_h);
    p.sprite("widget/scroller_background", x, ly, SCROLLBAR_W, list_h);
    p.sprite("widget/scroller", x, thumb_y, SCROLLBAR_W, thumb_h);
}

fn draw_footer(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, locked: bool) {
    let top = ctx.vh - FOOTER_H + ((FOOTER_H - 2.0 * WIDGET_HEIGHT - BUTTON_GAP) / 2.0).floor();
    let bottom = top + WIDGET_HEIGHT + BUTTON_GAP;

    let bar_x = ((ctx.vw - BAR_W) / 2.0).floor();

    let selected = state
        .profile
        .selected
        .filter(|i| *i < state.profile.accounts.entries.len());
    let can_select = selected.is_some_and(|i| state.profile.accounts.active != Some(i));
    let can_edit = selected
        .and_then(|i| state.profile.accounts.get(i))
        .is_some_and(|a| a.kind.editable());

    let button = |p: &mut Painter, x: f32, y: f32, w: f32, label: &str, active: bool| {
        Button {
            x,
            y,
            w,
            h: WIDGET_HEIGHT,
            label,
            active: active && !locked,
        }
        .draw(p, ctx)
    };

    let select_clicked = button(p, bar_x, top, TOP_ROW_BUTTON_W, SELECT, can_select);
    let add = button(
        p,
        bar_x + TOP_ROW_BUTTON_W + BUTTON_GAP,
        top,
        TOP_ROW_BUTTON_W,
        ADD,
        true,
    );

    let step = LOWER_ROW_BUTTON_W + BUTTON_GAP;
    let edit = button(p, bar_x, bottom, LOWER_ROW_BUTTON_W, EDIT, can_edit);
    let delete = button(
        p,
        bar_x + step,
        bottom,
        LOWER_ROW_BUTTON_W,
        DELETE,
        selected.is_some(),
    );
    let back = button(
        p,
        bar_x + 2.0 * step,
        bottom,
        LOWER_ROW_BUTTON_W,
        BACK,
        true,
    );

    if select_clicked && let Some(i) = selected {
        select(state, i);
    }
    if add {
        state.profile.begin_add();
        state.nav = Some(entry_screen());
    }
    if edit && let Some(i) = selected {
        state.profile.begin_edit(i);
        state.nav = Some(Screen::EditProfile);
    }
    if delete && let Some(i) = selected {
        state.profile.confirm_delete = Some(i);
    }
    if back {
        state.nav = Some(Screen::Title);
    }
}

fn select(state: &mut GuiState, index: usize) {
    state.profile.accounts.activate(index);
    state.profile.selected = Some(index);
    state.profile.save();
    state.profile.publish();
}

fn handle_list_keys(state: &mut GuiState, ctx: &ScreenCtx) {
    let input = ctx.input;
    if input.escape {
        state.nav = Some(Screen::Title);
        return;
    }
    let count = state.profile.accounts.entries.len();
    if count > 0 && (input.up_arrow || input.down_arrow) {
        let delta = if input.up_arrow { -1i32 } else { 1 };
        let next = match state.profile.selected {
            Some(i) => (i as i32 + delta).clamp(0, count as i32 - 1) as usize,
            None if delta > 0 => 0,
            None => count - 1,
        };
        state.profile.selected = Some(next);
    }
    if input.enter
        && let Some(i) = state.profile.selected.filter(|i| *i < count)
    {
        select(state, i);
    }
}

fn draw_delete_confirm(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let Some(index) = state.profile.confirm_delete else {
        return;
    };
    let name = state
        .profile
        .accounts
        .get(index)
        .map(|a| a.name.clone())
        .unwrap_or_default();

    p.fill(0.0, 0.0, ctx.vw, ctx.vh, 0xC0_000000);

    let cy = (ctx.vh / 2.0).floor();
    let question_w = p.atlas.font.width_str(DELETE_QUESTION);
    p.text_str(
        DELETE_QUESTION,
        ((ctx.vw - question_w) / 2.0).floor(),
        cy - 40.0,
        0xFFFFFF,
        true,
    );
    let warning = format!("'{name}' will be removed from this client.");
    let warn_w = p.atlas.font.width_str(&warning);
    p.text_str(
        &warning,
        ((ctx.vw - warn_w) / 2.0).floor(),
        cy - 25.0,
        GREY,
        true,
    );

    let x = widgets::centered_x(ctx.vw);
    if Button::new(x, cy, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, DELETE).draw(p, ctx) {
        state.profile.accounts.remove(index);
        if state.profile.accounts.active.is_none() && !state.profile.accounts.entries.is_empty() {
            state.profile.accounts.activate(0);
        }
        state.profile.selected = state.profile.accounts.active;
        state.profile.confirm_delete = None;
        state.profile.save();
        state.profile.publish();
        if state.profile.accounts.entries.is_empty() {
            state.profile.first_run = true;
            state.profile.begin_add();
            state.nav = Some(entry_screen());
        }
    }
    if Button::new(x, cy + 24.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, CANCEL).draw(p, ctx)
        || ctx.input.escape
    {
        state.profile.confirm_delete = None;
    }
}

#[cfg(feature = "online_mode")]
pub fn draw_add(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    const QUESTION: &str = "Do you want to add a Microsoft account or play offline?";
    const REASSURANCE: &str = "(This option can be changed later)";
    const MICROSOFT: &str = "Microsoft Account";
    const OFFLINE: &str = "Play Offline";

    state.profile.ensure_loaded();
    menu::background(p, ctx.vw, ctx.vh);
    widgets::draw_title(p, ctx.vw, LAYOUT.title_y(), ADD_TITLE);

    let first_run = state.profile.first_run;
    let question_w = p.atlas.font.width_str(QUESTION);
    let base = (ctx.vh / 4.0).floor();
    p.text_str(
        QUESTION,
        ((ctx.vw - question_w) / 2.0).floor(),
        base + 20.0,
        0xFFFFFF,
        true,
    );
    if first_run {
        let w = p.atlas.font.width_str(REASSURANCE);
        p.text_str(
            REASSURANCE,
            ((ctx.vw - w) / 2.0).floor(),
            base + 34.0,
            GREY,
            true,
        );
    }

    let x = widgets::centered_x(ctx.vw);
    let microsoft = Button::new(x, base + 60.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, MICROSOFT);
    let offline = Button::new(x, base + 84.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, OFFLINE);

    if microsoft.draw(p, ctx) {
        state.profile.copied_link = false;
        crate::client::auth::begin();
        state.nav = Some(Screen::MicrosoftLogin);
    }
    if offline.draw(p, ctx) {
        state.profile.begin_add();
        state.nav = Some(Screen::EditProfile);
    }

    if !first_run {
        let cancel = Button::new(x, base + 114.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, CANCEL);
        if cancel.draw(p, ctx) || ctx.input.escape {
            state.nav = Some(Screen::Accounts);
        }
    }
}

#[cfg(feature = "online_mode")]
pub fn draw_microsoft(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    use crate::client::auth::{self, Auth};

    const MS_TITLE: &str = "Sign in with Microsoft";
    const STARTING: &str = "Contacting Microsoft...";
    const INSTRUCTION: &str = "Open the link below and enter this code:";
    const ELSEWHERE: &str = "You can do this on another device, such as a phone.";
    const OPEN: &str = "Open Link";
    const COPY: &str = "Copy Link";
    const COPIED: &str = "Copied";
    const RETRY: &str = "Try Again";
    const YELLOW: u32 = 0xFFFF55;
    const RED: u32 = 0xFF5555;

    state.profile.ensure_loaded();
    menu::background(p, ctx.vw, ctx.vh);
    widgets::draw_title(p, ctx.vw, LAYOUT.title_y(), MS_TITLE);

    let x = widgets::centered_x(ctx.vw);
    let base = (ctx.vh / 4.0).floor();
    let centered = |p: &mut Painter, text: &str, y: f32, color: u32| {
        let w = p.atlas.font.width_str(text);
        p.text_str(text, ((ctx.vw - w) / 2.0).floor(), y, color, true);
    };

    match auth::state() {
        None | Some(Auth::Starting) => {
            centered(p, STARTING, base + 30.0, 0xFFFFFF);
        }
        Some(Auth::Prompt { url, code }) => {
            centered(p, INSTRUCTION, base + 14.0, 0xFFFFFF);
            centered(p, &code, base + 30.0, YELLOW);
            centered(p, ELSEWHERE, base + 46.0, GREY);

            if Button::new(x, base + 64.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, OPEN).draw(p, ctx) {
                crate::platform::url::open(&url);
            }
            let copied = state.profile.copied_link;
            let copy_label = if copied { COPIED } else { COPY };
            if Button::new(x, base + 88.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, copy_label).draw(p, ctx)
            {
                crate::platform::clipboard::set(&url);
                state.profile.copied_link = true;
            }
        }
        Some(Auth::Done {
            name,
            uuid,
            token,
            session,
            session_expires,
        }) => {
            state.profile.accounts.add(Account {
                name,
                kind: AccountKind::Microsoft,
                token: Some(token),
                uuid: Some(uuid),
                session: Some(session),
                session_expires: Some(session_expires),
            });
            state.profile.selected = state.profile.accounts.active;
            state.profile.save();
            state.profile.publish();
            state.profile.copied_link = false;
            auth::clear();
            leave(state, Screen::Accounts);
            return;
        }
        Some(Auth::Failed(message)) => {
            centered(p, &message, base + 30.0, RED);
            if Button::new(x, base + 64.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, RETRY).draw(p, ctx) {
                state.profile.copied_link = false;
                auth::begin();
            }
        }
    }

    let cancel = Button::new(x, base + 122.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, CANCEL);
    if cancel.draw(p, ctx) || ctx.input.escape {
        auth::cancel();
        state.profile.copied_link = false;
        state.nav = Some(entry_screen());
    }
}

pub fn draw_edit(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    state.profile.ensure_loaded();
    menu::background(p, ctx.vw, ctx.vh);

    let editing = state.profile.editing.is_some();
    let title = if editing { EDIT_TITLE } else { ADD_TITLE };
    let x = widgets::centered_x(ctx.vw);
    let title_w = p.atlas.font.width_str(title);
    p.text_str(
        title,
        ((ctx.vw - title_w) / 2.0).floor(),
        17.0,
        0xFFFFFF,
        true,
    );
    p.text_str(USERNAME_LABEL, x + 1.0, 53.0, 0xA0A0A0, true);

    let field = &mut state.profile.username;
    field.update(p, ctx, x, 66.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT);

    let valid = valid_username(&field.text);

    let base = (ctx.vh / 4.0).floor();
    let done = Button {
        x,
        y: base + 114.0,
        w: WIDGET_WIDTH_BIG,
        h: WIDGET_HEIGHT,
        label: DONE,
        active: valid,
    };

    if (done.draw(p, ctx) || (ctx.input.enter && valid)) && valid {
        state.profile.commit_form();
        leave(state, Screen::Accounts);
    }

    let back = if state.profile.first_run {
        (entry_screen() != Screen::EditProfile).then(entry_screen)
    } else {
        Some(Screen::Accounts)
    };
    if let Some(back) = back {
        let label = if state.profile.first_run {
            BACK
        } else {
            CANCEL
        };
        let cancel = Button::new(x, base + 138.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, label);
        if cancel.draw(p, ctx) || ctx.input.escape {
            state.nav = Some(back);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_and_scrollbar_geometry() {
        assert_eq!(row_left(854.0), 275.0);
        assert_eq!(scrollbar_x(854.0), 275.0 + 305.0 + 8.0);
    }

    #[test]
    fn the_footer_rows_line_up() {
        assert_eq!(BAR_W, 230.0);
        assert_eq!(2.0 * TOP_ROW_BUTTON_W + BUTTON_GAP, BAR_W);
        assert_eq!(3.0 * LOWER_ROW_BUTTON_W + 2.0 * BUTTON_GAP, BAR_W);
        assert_eq!(TOP_ROW_BUTTON_W.fract(), 0.0);
    }

    #[test]
    fn list_fills_between_header_and_footer() {
        let (x, y, w, h) = LAYOUT.content_rect(854.0, 480.0);
        assert_eq!((x, y, w), (0.0, 33.0, 854.0));
        assert_eq!(h, 480.0 - 33.0 - 60.0);
    }

    #[test]
    fn the_form_adds_and_renames() {
        let mut st = ProfileState {
            loaded: true,
            ..Default::default()
        };
        st.begin_add();
        st.username.set_text("Player");
        st.commit_form();
        assert_eq!(st.accounts.active, Some(0));
        assert_eq!(st.accounts.active().unwrap().name, "Player");

        st.begin_edit(0);
        st.username.set_text("Renamed");
        st.commit_form();
        assert_eq!(st.accounts.entries.len(), 1);
        assert_eq!(st.accounts.active().unwrap().name, "Renamed");
    }

    #[test]
    fn a_microsoft_account_cannot_be_edited() {
        let mut st = ProfileState {
            loaded: true,
            accounts: Accounts {
                entries: vec![Account {
                    name: "Notch".into(),
                    kind: AccountKind::Microsoft,
                    token: Some("refresh-token".into()),
                    uuid: Some("069a79f4-44e9-4726-a5be-fca90e38aaf5".into()),
                    session: Some("session-token".into()),
                    session_expires: Some(1_800_000_000),
                }],
                active: Some(0),
            },
            ..Default::default()
        };
        st.begin_edit(0);
        assert_eq!(st.editing, None);
    }
}
