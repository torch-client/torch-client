use crate::gui::accountlist::{self, Account, AccountKind, Accounts};
use crate::gui::focus;
use crate::gui::multiplayer::{
    self, BUTTON_GAP, CONTENT_PAD, FOOTER_H, ICON_SIZE, LAYOUT, LOWER_ROW_BUTTON_W, ROW_H, ROW_W,
    TEXT_INSET, row_left,
};
use crate::gui::painter::Painter;
use crate::gui::widgets::{self, Button, TextBox, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{GuiState, Screen, ScreenCtx, menu};
use crate::lang::Key;
use crate::platform::cli::valid_username;
use crate::text::{AQUA, GREEN};

const TITLE: Key = Key::of("profile.title");
const SELECT: Key = Key::of("profile.select");
const ADD: Key = Key::of("profile.add");
const ADD_TITLE: Key = Key::of("profile.add_title");
const EDIT_TITLE: Key = Key::of("profile.edit_title");
const USERNAME_LABEL: Key = Key::of("profile.username");
const EDIT: Key = Key::of("profile.edit");
const DELETE: Key = Key::of("profile.delete");
const BACK: Key = Key::of("profile.back");
const DONE: Key = Key::of("profile.done");
const CANCEL: Key = Key::of("profile.cancel");
const IN_USE: Key = Key::of("profile.in_use");
const EMPTY: Key = Key::of("profile.empty");
const DELETE_QUESTION: Key = Key::of("profile.delete_question");

const USERNAME_MAX_LEN: usize = 16;

const BAR_W: f32 = 3.0 * LOWER_ROW_BUTTON_W + 2.0 * BUTTON_GAP;
const TOP_ROW_BUTTON_W: f32 = (BAR_W - BUTTON_GAP) / 2.0;
const GREY: u32 = 0x808080;

pub struct ProfileState {
    pub accounts: Accounts,
    loaded: bool,
    pub selected: Option<usize>,
    pub scroll: f32,
    scrollbar_drag: Option<f32>,
    editing: Option<usize>,
    username: TextBox,
    #[cfg(eagler_skins)]
    skin: crate::gui::skinpick::SkinPick,
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
            scrollbar_drag: None,
            editing: None,
            username: TextBox::bordered(USERNAME_MAX_LEN, ""),
            #[cfg(eagler_skins)]
            skin: Default::default(),
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
        #[cfg(eagler_skins)]
        self.skin.open(None);
    }

    fn begin_edit(&mut self, index: usize) {
        let Some(account) = self.accounts.get(index).filter(|a| a.kind.editable()) else {
            return;
        };
        let name = account.name.clone();
        self.editing = Some(index);
        self.username.set_text(&name);
        self.username.focus();
        #[cfg(eagler_skins)]
        self.skin.open(Some(&name));
    }

    fn commit_form(&mut self) {
        let name = self.username.text.trim().to_string();
        if !valid_username(&name) {
            return;
        }
        #[cfg(eagler_skins)]
        {
            let old = self
                .editing
                .and_then(|i| self.accounts.get(i))
                .map(|a| a.name.clone());
            self.skin.commit(old.as_deref(), &name);
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
        #[cfg(feature = "online_mode")]
        crate::client::ban::account_changed();
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
    p.text_centered(TITLE.get(), ctx.vw / 2.0, LAYOUT.title_y(), 0xFFFFFF, true);

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

fn draw_rows(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, locked: bool) {
    let (lx, ly, lw, lh) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let st = &mut state.profile;
    multiplayer::clamp_list_scroll(&mut st.scroll, st.accounts.entries.len(), lh);

    let list_focused = focus::next(!locked && !st.accounts.entries.is_empty());

    if st.accounts.entries.is_empty() {
        p.text_centered(
            EMPTY.get(),
            ctx.vw / 2.0,
            (ly + lh / 2.0 - 4.0).floor(),
            GREY,
            true,
        );
        return;
    }

    let mut activate: Option<usize> = None;
    let over_list = !locked && ctx.hovering(lx, ly, lw, lh);

    let guard = p.push_clip(lx, ly, lw, lh);
    for i in 0..st.accounts.entries.len() {
        let rx = row_left(ctx.vw);
        let ry = ly + CONTENT_PAD - st.scroll + i as f32 * ROW_H;
        if ry + ROW_H < ly || ry > ly + lh {
            continue;
        }
        if st.selected == Some(i) {
            multiplayer::draw_selection_outline(p, rx, ry, list_focused);
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

        if over_list && ctx.input.left_click && ctx.hovering(rx, ry, ROW_W, ROW_H) {
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
    p.text_plain(
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
    p.text_plain(account.kind.label(), text_x, cy + 18.0, color, true);

    if active {
        let in_use = IN_USE.get();
        let w = p.atlas.font.width_str(in_use);
        p.text_plain(in_use, cx + cw - w, cy + 6.0, GREEN, true);
    }
}

fn draw_scrollbar(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    list_h: f32,
    locked: bool,
) {
    let st = &mut state.profile;
    let content_h = multiplayer::list_content_height(st.accounts.entries.len());
    multiplayer::list_scrollbar(
        p,
        ctx,
        &mut st.scroll,
        &mut st.scrollbar_drag,
        content_h,
        list_h,
        locked,
    );
}

fn draw_footer(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, locked: bool) {
    let top = ctx.vh - FOOTER_H + ((FOOTER_H - 2.0 * WIDGET_HEIGHT - BUTTON_GAP) / 2.0).floor();
    let bottom = top + WIDGET_HEIGHT + BUTTON_GAP;

    let bar_x = widgets::centered_row_x(ctx.vw, BAR_W);
    #[cfg(feature = "tutorial")]
    crate::gui::tutorial::mark(
        crate::gui::tutorial::Spot::Accounts,
        crate::gui::hud_layout::Rect::new(bar_x, top, BAR_W, bottom + WIDGET_HEIGHT - top),
    );

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

    let select_clicked = button(p, bar_x, top, TOP_ROW_BUTTON_W, SELECT.get(), can_select);
    let add = button(
        p,
        bar_x + TOP_ROW_BUTTON_W + BUTTON_GAP,
        top,
        TOP_ROW_BUTTON_W,
        ADD.get(),
        true,
    );

    let step = LOWER_ROW_BUTTON_W + BUTTON_GAP;
    let edit = button(p, bar_x, bottom, LOWER_ROW_BUTTON_W, EDIT.get(), can_edit);
    let delete = button(
        p,
        bar_x + step,
        bottom,
        LOWER_ROW_BUTTON_W,
        DELETE.get(),
        selected.is_some(),
    );
    let back = button(
        p,
        bar_x + 2.0 * step,
        bottom,
        LOWER_ROW_BUTTON_W,
        BACK.get(),
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
    multiplayer::arrow_select(
        &mut state.profile.selected,
        count,
        input.up_arrow,
        input.down_arrow,
    );
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
    p.text_centered(
        DELETE_QUESTION.get(),
        ctx.vw / 2.0,
        cy - 40.0,
        0xFFFFFF,
        true,
    );
    let warning = crate::lang::trf!("profile.delete_warning", name);
    p.text_centered(&warning, ctx.vw / 2.0, cy - 25.0, GREY, true);

    let x = widgets::centered_x(ctx.vw);
    if Button::new(x, cy, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, DELETE.get()).draw(p, ctx) {
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
    if Button::new(x, cy + 24.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, CANCEL.get()).draw(p, ctx)
        || ctx.input.escape
    {
        state.profile.confirm_delete = None;
    }
}

#[cfg(feature = "online_mode")]
pub fn draw_add(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    const QUESTION: Key = Key::of("profile.add_question");
    const REASSURANCE: Key = Key::of("profile.add_reassurance");
    const MICROSOFT: Key = Key::of("profile.kind.microsoft");
    const OFFLINE: Key = Key::of("profile.play_offline");

    state.profile.ensure_loaded();
    menu::background(p, ctx.vw, ctx.vh);
    p.text_centered(
        ADD_TITLE.get(),
        ctx.vw / 2.0,
        LAYOUT.title_y(),
        0xFFFFFF,
        true,
    );

    let first_run = state.profile.first_run;
    let base = (ctx.vh / 4.0).floor();
    p.text_centered(QUESTION.get(), ctx.vw / 2.0, base + 20.0, 0xFFFFFF, true);
    if first_run {
        p.text_centered(REASSURANCE.get(), ctx.vw / 2.0, base + 34.0, GREY, true);
    }

    let x = widgets::centered_x(ctx.vw);
    let microsoft = Button::new(
        x,
        base + 60.0,
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        MICROSOFT.get(),
    );
    let offline = Button::new(
        x,
        base + 84.0,
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        OFFLINE.get(),
    );

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
        let cancel = Button::new(
            x,
            base + 114.0,
            WIDGET_WIDTH_BIG,
            WIDGET_HEIGHT,
            CANCEL.get(),
        );
        if cancel.draw(p, ctx) || ctx.input.escape {
            state.nav = Some(Screen::Accounts);
        }
    }
}

#[cfg(feature = "online_mode")]
pub fn draw_microsoft(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    use crate::client::auth::{self, Auth};

    const MS_TITLE: Key = Key::of("profile.microsoft.title");
    const STARTING: Key = Key::of("profile.microsoft.starting");
    const INSTRUCTION: Key = Key::of("profile.microsoft.instruction");
    const ELSEWHERE: Key = Key::of("profile.microsoft.elsewhere");
    const OPEN: Key = Key::of("profile.microsoft.open");
    const COPY: Key = Key::of("profile.microsoft.copy");
    const COPIED: Key = Key::of("profile.microsoft.copied");
    const RETRY: Key = Key::of("profile.microsoft.retry");
    use crate::text::{RED, YELLOW};

    state.profile.ensure_loaded();
    menu::background(p, ctx.vw, ctx.vh);
    p.text_centered(
        MS_TITLE.get(),
        ctx.vw / 2.0,
        LAYOUT.title_y(),
        0xFFFFFF,
        true,
    );

    let x = widgets::centered_x(ctx.vw);
    let base = (ctx.vh / 4.0).floor();
    let centered = |p: &mut Painter, text: &str, y: f32, color: u32| {
        p.text_centered(text, ctx.vw / 2.0, y, color, true);
    };

    match auth::state() {
        None | Some(Auth::Starting) => {
            centered(p, STARTING.get(), base + 30.0, 0xFFFFFF);
        }
        Some(Auth::Prompt { url, code }) => {
            centered(p, INSTRUCTION.get(), base + 14.0, 0xFFFFFF);
            centered(p, &code, base + 30.0, YELLOW);
            centered(p, ELSEWHERE.get(), base + 46.0, GREY);

            if Button::new(x, base + 64.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, OPEN.get()).draw(p, ctx)
            {
                crate::platform::url::open(&url);
            }
            let copied = state.profile.copied_link;
            let copy_label = if copied { COPIED.get() } else { COPY.get() };
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
            if Button::new(x, base + 64.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, RETRY.get())
                .draw(p, ctx)
            {
                state.profile.copied_link = false;
                auth::begin();
            }
        }
    }

    let cancel = Button::new(
        x,
        base + 122.0,
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        CANCEL.get(),
    );
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
    let title = if editing {
        EDIT_TITLE.get()
    } else {
        ADD_TITLE.get()
    };
    let x = widgets::centered_x(ctx.vw);
    p.text_centered(title, ctx.vw / 2.0, 17.0, 0xFFFFFF, true);
    #[cfg(eagler_skins)]
    let (x, buttons, bottom) = {
        let x = x + 60.0;
        let buttons = 92.0 + crate::gui::skinpick::HEIGHT + 8.0;
        let bottom = buttons + WIDGET_HEIGHT + 6.0;
        p.fill(
            x - 124.0,
            44.0,
            WIDGET_WIDTH_BIG + 128.0,
            bottom - 44.0,
            0x80000000,
        );
        (x, buttons, bottom)
    };
    p.text_plain(USERNAME_LABEL.get(), x + 1.0, 53.0, 0xA0A0A0, true);

    let field = &mut state.profile.username;
    field.update(p, ctx, x, 66.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT);

    let valid = valid_username(&field.text);

    #[cfg(eagler_skins)]
    state.profile.skin.draw(
        p,
        ctx,
        x,
        92.0,
        WIDGET_WIDTH_BIG,
        [x - 118.0, 48.0, 108.0, bottom - 52.0],
    );

    let back = if state.profile.first_run {
        (entry_screen() != Screen::EditProfile).then(entry_screen)
    } else {
        Some(Screen::Accounts)
    };

    #[cfg(eagler_skins)]
    let (done_at, cancel_at) = {
        let half = (WIDGET_WIDTH_BIG - 4.0) / 2.0;
        let done_w = if back.is_some() {
            half
        } else {
            WIDGET_WIDTH_BIG
        };
        ([x, buttons, done_w], [x + half + 4.0, buttons, half])
    };
    #[cfg(not(eagler_skins))]
    let (done_at, cancel_at) = {
        let base = (ctx.vh / 4.0).floor();
        (
            [x, base + 114.0, WIDGET_WIDTH_BIG],
            [x, base + 138.0, WIDGET_WIDTH_BIG],
        )
    };
    let done = Button {
        x: done_at[0],
        y: done_at[1],
        w: done_at[2],
        h: WIDGET_HEIGHT,
        label: DONE.get(),
        active: valid,
    };

    if (done.draw(p, ctx) || ctx.input.enter) && valid {
        state.profile.commit_form();
        leave(state, Screen::Accounts);
    }

    if let Some(back) = back {
        let label = if state.profile.first_run {
            BACK.get()
        } else {
            CANCEL.get()
        };
        let [cx, cy, cw] = cancel_at;
        let cancel = Button::new(cx, cy, cw, WIDGET_HEIGHT, label);
        if cancel.draw(p, ctx) || ctx.input.escape {
            state.nav = Some(back);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
