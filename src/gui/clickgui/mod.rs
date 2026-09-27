mod picker;
pub(crate) mod theme;

use bevy::prelude::Vec2;

use crate::gui::keybinds::Bound;
use crate::modules::list::{self, BitList};
use crate::modules::registry::{self as model, CATEGORIES, Id, Kind, ListKind, Mode, SettingDef};
use crate::modules::{Value, flat_settings, store};
use picker::{Picker, Row};

use crate::gui::ScreenCtx;
use crate::gui::painter::{self, Painter};
use crate::gui::render::GuiInput;
use crate::gui::widgets::{MultiLineTextBox, TextBox};

const GAP: f32 = 4.0;
const TOP_H: f32 = 16.0;
const HEADER_H: f32 = 15.0;
const PAD_BOTTOM: f32 = 4.0;
const INDENT: f32 = 5.0;

const RESET_W: f32 = 7.0;
const RESET_EDGE: f32 = 3.0;

const TOGGLE_H: f32 = 12.0;
const SLIDER_H: f32 = 19.0;
const ENUM_H: f32 = 15.0;
const BIND_H: f32 = 12.0;
const BLOCK_H: f32 = 12.0;
const CHANNEL_H: f32 = 22.0;
const LIST_H: f32 = 13.0;
const TEXTBOX_ROW_H: f32 = 13.0 + FIELD_H + 3.0;
const OPTION_H: f32 = 11.0;
const SWATCH_H: f32 = 26.0;
const SWATCH: f32 = 11.0;
const TEXT_H: f32 = 8.0;

const POP_SECS: f32 = 0.16;
const POP_FROM: f32 = 0.93;

const BUTTON_W: f32 = PROFILE_W;
const BUTTON_H: f32 = 12.0;

const SEARCH_ROWS: usize = 6;

const TIP_W: f32 = 118.0;
const TIP_PAD: f32 = 4.0;
const TIP_LINE: f32 = 10.0;
const FIELD_H: f32 = 13.0;
const HINT_H: f32 = 11.0;

const CFG_ID: u16 = 0xF000;
const CFG_SCALE: u16 = CFG_ID;

const CFG_PROFILE: u16 = CFG_ID + 5;
const BAR_PROFILE: u16 = CFG_ID + 6;

const PROFILE_W: f32 = 92.0;

const PROFILE_TIP: &str = "Which modules may run. Legit permits only what the server cannot see. Ctrl+] raises, Ctrl+[ lowers.";

const SCALE_MIN: f32 = 0.6;
const SCALE_MAX: f32 = 1.8;
const AUTO_SCALE_MIN: f32 = 0.6;

#[derive(Clone, Copy)]
pub(crate) struct Look {
    accent: u8,
    surface: u8,
    pub(crate) scale: f32,
    opacity: f32,
    pub(crate) radius: f32,
    pub(crate) row_h: f32,
    panel_w: f32,
    animate: bool,
    pub(crate) shadow: bool,
    scrim: bool,
}

const SCRIM: u32 = 0x73_00_00_00;

const OVERLAY_SCRIM: u32 = 0x59_00_00_00;

impl Default for Look {
    fn default() -> Self {
        Look {
            accent: 0,
            surface: 0,
            scale: 1.0,
            opacity: 0.95,
            radius: 4.0,
            row_h: 13.0,
            panel_w: 106.0,
            animate: true,
            shadow: false,
            scrim: true,
        }
    }
}

impl Look {
    pub(crate) fn accent(self) -> u32 {
        0xFF00_0000 | theme::ACCENTS[self.accent as usize % theme::ACCENTS.len()].base
    }

    pub(crate) fn accent_hi(self) -> u32 {
        0xFF00_0000 | theme::ACCENTS[self.accent as usize % theme::ACCENTS.len()].hi
    }

    fn surface_def(self) -> &'static theme::Surface {
        &theme::SURFACES[self.surface as usize % theme::SURFACES.len()]
    }

    pub(crate) fn surface(self) -> u32 {
        self.surface_def().base
    }

    fn shade(self, amount: u32) -> u32 {
        let s = self.surface_def();
        let rgb = if s.light {
            theme::darken(s.base, amount)
        } else {
            theme::lighten(s.base, amount)
        };
        0xFF00_0000 | rgb
    }

    pub(crate) fn light(self) -> bool {
        self.surface_def().light
    }

    pub(crate) fn text(self) -> u32 {
        if self.light() {
            theme::TEXT_L
        } else {
            theme::TEXT
        }
    }

    pub(crate) fn text_title(self) -> u32 {
        if self.light() {
            theme::TEXT_TITLE_L
        } else {
            theme::TEXT_TITLE
        }
    }

    pub(crate) fn text_dim(self) -> u32 {
        if self.light() {
            theme::TEXT_DIM_L
        } else {
            theme::TEXT_DIM
        }
    }

    pub(crate) fn row_hover(self) -> u32 {
        if self.light() {
            theme::ROW_HOVER_L
        } else {
            theme::ROW_HOVER
        }
    }

    pub(crate) fn track(self) -> u32 {
        if self.light() {
            theme::TRACK_L
        } else {
            theme::TRACK
        }
    }

    pub(crate) fn panel(self) -> u32 {
        ((self.opacity.clamp(0.0, 1.0) * 255.0) as u32) << 24 | self.surface()
    }

    pub(crate) fn raised(self) -> u32 {
        self.shade(0x06)
    }

    pub(crate) fn solid(self) -> u32 {
        0xFF00_0000 | self.surface()
    }

    pub(crate) fn header(self) -> u32 {
        self.shade(0x0E)
    }

    pub(crate) fn nest(self) -> u32 {
        self.shade(0x07)
    }

    pub(crate) fn nest_hi(self) -> u32 {
        self.shade(0x18)
    }
}

#[derive(Clone, Copy)]
struct Panel {
    x: f32,
    y: f32,
    collapsed: bool,
    scroll: f32,
}

impl Panel {
    const fn at(x: f32, y: f32) -> Panel {
        Panel {
            x,
            y,
            collapsed: false,
            scroll: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
enum Drag {
    Window { window: u8, dx: f32, dy: f32 },
    Slider { id: u16, handle: u8 },
}

const SEARCH: u8 = u8::MAX;

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    None,
    Bar,
    Search,
    Config,
    Picker,
    Editor,
    Panel(u8),
}

struct Editor {
    id: u16,
    title: String,
    lines: usize,
    max_len: usize,
    text: MultiLineTextBox,
    fresh: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Modules,
    Config,
}

const TABS: [(Tab, &str); 2] = [(Tab::Modules, "Modules"), (Tab::Config, "Config")];

const TAB_PAD: f32 = 9.0;
const STRIP_PAD: f32 = 2.0;
const TAB_RADIUS: f32 = 3.5;

struct Ui<'a> {
    input: &'a GuiInput,
    mouse: Option<Vec2>,
    vw: f32,
    vh: f32,
}

impl Ui<'_> {
    fn mouse(&self) -> Option<Vec2> {
        self.mouse
    }

    fn hovering(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        match self.mouse {
            Some(m) => m.x >= x && m.x < x + w && m.y >= y && m.y < y + h,
            None => false,
        }
    }
}

pub struct ClickGuiState {
    look: Look,
    tab: Tab,
    panels: Box<[Panel]>,
    order: Vec<u8>,
    module_base: Box<[u16]>,
    setting_base: Box<[u16]>,
    expanded: Option<u16>,
    dropdown: Option<u16>,
    profile_seen: Mode,
    drag: Option<Drag>,
    focus: Focus,
    band: (f32, f32),
    search: Panel,
    config: Panel,
    field: TextBox,
    mouse_input: GuiInput,
    matches: Vec<u16>,
    selected: usize,
    match_top: usize,
    scratch: String,
    capturing: Option<u16>,
    picker: Option<Picker>,
    picker_win: Panel,
    picker_query: TextBox,
    editor: Option<Editor>,
    editor_win: Panel,
    text_boxes: Box<[TextBox]>,
    text_focus: Option<u16>,
    tip: Option<(&'static str, f32, f32)>,
    tip_lines: Vec<(usize, usize)>,
    placed: bool,
    applied: f32,
    scale_set: bool,
    pending_open: bool,
    opened_at: f32,
}

impl Default for ClickGuiState {
    fn default() -> Self {
        let mut module_base = Vec::with_capacity(CATEGORIES.len() + 1);
        let mut setting_base = Vec::new();
        let (mut modules, mut settings) = (0u16, 0u16);
        for cat in CATEGORIES {
            module_base.push(modules);
            for m in cat.modules {
                setting_base.push(settings);
                modules += 1;
                settings += m.settings.len() as u16;
            }
        }
        module_base.push(modules);
        setting_base.push(settings);
        let module_count = modules as usize;

        ClickGuiState {
            look: Look::default(),
            tab: Tab::Modules,
            panels: vec![Panel::at(0.0, 0.0); CATEGORIES.len()].into_boxed_slice(),
            order: (0..CATEGORIES.len() as u8).collect(),
            module_base: module_base.into_boxed_slice(),
            setting_base: setting_base.into_boxed_slice(),
            expanded: None,
            dropdown: None,
            profile_seen: store().profile(),
            drag: None,
            focus: Focus::None,
            band: (0.0, 0.0),
            search: Panel::at(0.0, 0.0),
            config: Panel::at(0.0, 0.0),
            field: {
                let mut f = TextBox::new(48, "Type to search");
                f.focused = true;
                f.browser_keyboard = false;
                f
            },
            mouse_input: GuiInput::default(),
            matches: Vec::with_capacity(module_count),
            selected: 0,
            match_top: 0,
            scratch: String::with_capacity(16),
            capturing: None,
            picker: None,
            picker_win: Panel::at(0.0, 0.0),
            picker_query: {
                let mut f = TextBox::new(32, "Search");
                f.browser_keyboard = false;
                f
            },
            editor: None,
            editor_win: Panel::at(0.0, 0.0),
            text_boxes: flat_settings()
                .enumerate()
                .map(|(i, s)| match s.kind {
                    Kind::Text { max_len, .. } => {
                        let mut b = TextBox::new(max_len, "");
                        b.browser_keyboard = false;
                        b.set_text(&store().text_at(i));
                        b
                    }
                    _ => TextBox::new(0, ""),
                })
                .collect(),
            text_focus: None,
            tip: None,
            tip_lines: Vec::with_capacity(4),
            placed: false,
            applied: 1.0,
            scale_set: false,
            pending_open: true,
            opened_at: 0.0,
        }
    }
}

pub fn draw(p: &mut Painter, st: &mut ClickGuiState, ctx: &ScreenCtx) {
    if !st.dragging_scale() {
        st.applied = st.look.scale.clamp(SCALE_MIN, SCALE_MAX);
    }
    let s = st.applied;
    let mut ui = Ui {
        input: ctx.input,
        mouse: ctx.mouse().map(|m| m / s),
        vw: ctx.vw / s,
        vh: ctx.vh / s,
    };

    st.place(&mut ui);
    let s = st.applied;
    st.tab_key(&ui);
    st.profile_key(&ui);
    st.keyboard(&ui);
    st.pointer(&ui);

    if st.look.scrim {
        p.fill(0.0, 0.0, ctx.vw, ctx.vh, SCRIM);
    }

    let pop = st.pop(ctx.input.time);
    let total = s * pop;
    let (cx, cy) = (ctx.vw * 0.5, ctx.vh * 0.5);
    p.scaled(total, cx * (1.0 - pop), cy * (1.0 - pop), |p| {
        st.contents(p, &ui)
    });
}

pub(crate) fn text_y(top: f32, h: f32) -> f32 {
    (top + (h - TEXT_H) * 0.5).floor()
}

fn profile_x(vw: f32) -> f32 {
    (vw - GAP - PROFILE_W).floor()
}

const EPS: f32 = 1e-3;

fn differs(a: f32, b: f32) -> bool {
    (a - b).abs() >= EPS
}

fn is_default(value: Value, kind: Kind) -> bool {
    match (value, Value::default_of(kind)) {
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Num(a), Value::Num(b)) => (a - b).abs() < EPS,
        (Value::Range(a0, a1), Value::Range(b0, b1)) => {
            (a0 - b0).abs() < EPS && (a1 - b1).abs() < EPS
        }
        (Value::Choice(a), Value::Choice(b)) => a == b,
        _ => true,
    }
}

fn default_bind(_gm: u16) -> Bound {
    Bound::Unbound
}

fn cat_tag(cat: &'static str) -> &'static str {
    &cat[..3.min(cat.len())]
}

fn cat_w(p: &Painter, cat: &str) -> f32 {
    p.atlas.font.width_str(cat)
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    if n.is_empty() || n.len() > h.len() {
        return false;
    }
    (0..=h.len() - n.len()).any(|i| {
        n.iter()
            .zip(&h[i..])
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

impl ClickGuiState {
    fn contents(&mut self, p: &mut Painter, ui: &Ui) {
        self.tip = None;
        let profile = store().profile();
        if self.profile_seen != profile {
            self.profile_seen = profile;
            self.refresh_matches();
        }
        self.top_bar(p, ui);
        match self.tab {
            Tab::Modules => {
                for i in 0..self.order.len() {
                    let pi = self.order[i] as usize;
                    let active = self.focus == Focus::Panel(pi as u8) && self.drag.is_none();
                    self.panel(p, ui, pi, active);
                }
                self.search_window(p, ui);
                self.overlay_scrim(p, ui);
                self.picker_window(p, ui);
                self.editor_window(p, ui);
            }
            Tab::Config => {
                self.config_window(p, ui);
                self.overlay_scrim(p, ui);
                self.picker_window(p, ui);
                self.editor_window(p, ui);
            }
        }
        self.profile_control(p, ui);
        self.tooltip(p, ui);
    }

    fn tooltip(&mut self, p: &mut Painter, ui: &Ui) {
        let Some((text, mx, my)) = self.tip else {
            return;
        };
        let look = self.look;
        let inner = TIP_W - 2.0 * TIP_PAD;
        self.wrap(p, text, inner);
        let lines = self.tip_lines.len() as f32;
        let w = TIP_W;
        let h = 2.0 * TIP_PAD + lines * TIP_LINE;

        let x = (mx + 9.0).min(ui.vw - w - 2.0).max(2.0).floor();
        let y = if my + 6.0 + h > ui.vh {
            (my - 6.0 - h).max(2.0)
        } else {
            my + 6.0
        }
        .floor();

        p.rounded_rect(x, y, w, h, look.radius, look.solid());
        p.fill(
            x,
            y + look.radius,
            1.5,
            h - 2.0 * look.radius,
            look.accent(),
        );
        for (i, &(a, b)) in self.tip_lines.iter().enumerate() {
            p.text_plain(
                &text[a..b],
                x + TIP_PAD,
                y + TIP_PAD + i as f32 * TIP_LINE,
                look.text(),
                look.shadow,
            );
        }
    }

    fn wrap(&mut self, p: &Painter, text: &str, w: f32) {
        self.tip_lines.clear();
        let font = &p.atlas.font;
        let space = font.advance(' ', false);
        let (mut start, mut end, mut width) = (0usize, 0usize, 0.0f32);
        for (at, word) in text.split_whitespace().map(|s| {
            let at = s.as_ptr() as usize - text.as_ptr() as usize;
            (at, s)
        }) {
            let ww = font.width_str(word);
            let gap = if end == start { 0.0 } else { space };
            if end > start && width + gap + ww > w {
                self.tip_lines.push((start, end));
                (start, width) = (at, ww);
            } else {
                width += gap + ww;
            }
            end = at + word.len();
        }
        if end > start {
            self.tip_lines.push((start, end));
        }
    }

    pub fn escape(&mut self) -> bool {
        if self.capturing.is_some() {
            return true;
        }
        if self.editor.is_some() {
            self.close_editor();
            return true;
        }
        self.picker.take().is_some()
    }

    pub fn opened(&mut self) {
        self.pending_open = true;
        self.field.clear();
        self.field.focused = true;
        self.matches.clear();
        self.selected = 0;
        self.match_top = 0;
    }

    fn pop(&mut self, now: f32) -> f32 {
        if std::mem::take(&mut self.pending_open) {
            self.opened_at = now;
        }
        if !self.look.animate {
            return 1.0;
        }
        let t = ((now - self.opened_at) / POP_SECS).clamp(0.0, 1.0);
        POP_FROM + (1.0 - POP_FROM) * (1.0 - (1.0 - t).powi(3))
    }

    fn place(&mut self, ui: &mut Ui) {
        let top = GAP + TOP_H + GAP;
        if !self.placed {
            self.placed = true;
            let n = self.panels.len() as f32 + 1.0;
            let need = n * (self.look.panel_w + GAP) + GAP;
            if !self.scale_set && need > ui.vw {
                let was = self.look.scale;
                self.look.scale = (was * ui.vw / need).clamp(AUTO_SCALE_MIN, 1.0);
                let ratio = was / self.look.scale;
                ui.vw *= ratio;
                ui.vh *= ratio;
                ui.mouse = ui.mouse.map(|m| m * ratio);
                self.applied = self.look.scale;
            }
            let step = (self.look.panel_w + GAP).min(((ui.vw - 2.0 * GAP) / n).max(24.0));
            for (i, panel) in self.panels.iter_mut().enumerate() {
                panel.x = (GAP + i as f32 * step).floor();
                panel.y = top;
            }
            self.search.x = (GAP + self.panels.len() as f32 * step).floor();
            self.search.y = top;
            self.config.x = ((ui.vw - CONFIG_W) * 0.5).floor();
            self.config.y = top;
        }
        let (w, right) = (self.look.panel_w, (ui.vw - 24.0).floor());
        let pick_w = self.picker_w();
        let bottom = (ui.vh - HEADER_H).max(0.0).floor();
        for (panel, pw) in self.panels.iter_mut().map(|p| (p, w)).chain([
            (&mut self.search, w),
            (&mut self.config, CONFIG_W),
            (&mut self.picker_win, pick_w),
            (&mut self.editor_win, EDITOR_W),
        ]) {
            panel.x = panel.x.clamp((GAP - pw + 24.0).min(right), right);
            panel.y = panel.y.clamp(0.0, bottom);
        }
    }

    fn pointer(&mut self, ui: &Ui) {
        if !ui.input.left_down {
            self.drag = None;
        }
        match self.drag {
            Some(Drag::Window { window, dx, dy }) => {
                if let Some(m) = ui.mouse() {
                    let p = self.window_mut(window);
                    p.x = (m.x - dx).round();
                    p.y = (m.y - dy).round();
                }
                self.focus = match window {
                    SEARCH => Focus::Search,
                    CONFIG => Focus::Config,
                    PICKER => Focus::Picker,
                    EDITOR => Focus::Editor,
                    w => Focus::Panel(w),
                };
                return;
            }
            Some(Drag::Slider { .. }) => return,
            None => {}
        }

        self.focus = Focus::None;
        let list_h = BUTTON_H + 2.0 + Mode::ALL.len() as f32 * OPTION_H;
        if ui.hovering(0.0, 0.0, ui.vw, GAP + TOP_H)
            || (self.dropdown == Some(BAR_PROFILE)
                && ui.hovering(profile_x(ui.vw), GAP, PROFILE_W, list_h))
        {
            self.focus = Focus::Bar;
            return;
        }
        if self.editor.is_some() {
            let h = HEADER_H + self.editor_view_h();
            if ui.hovering(self.editor_win.x, self.editor_win.y, EDITOR_W, h) {
                self.focus = Focus::Editor;
                return;
            }
        }
        if self.picker.is_some() {
            let h = HEADER_H + self.picker_view_h(ui.vh);
            if ui.hovering(self.picker_win.x, self.picker_win.y, self.picker_w(), h) {
                self.focus = Focus::Picker;
                return;
            }
        }
        if self.tab == Tab::Config {
            let h = HEADER_H + self.config_view_h(ui.vh);
            if ui.hovering(self.config.x, self.config.y, CONFIG_W, h) {
                self.focus = Focus::Config;
            }
            return;
        }
        if ui.hovering(
            self.search.x,
            self.search.y,
            self.look.panel_w,
            self.search_h(),
        ) {
            self.focus = Focus::Search;
            return;
        }
        for i in (0..self.order.len()).rev() {
            let pi = self.order[i];
            let panel = self.panels[pi as usize];
            if ui.hovering(
                panel.x,
                panel.y,
                self.look.panel_w,
                self.height(pi as usize, ui.vh),
            ) {
                self.focus = Focus::Panel(pi);
                if ui.input.left_click || ui.input.right_click {
                    let v = self.order.remove(i);
                    self.order.push(v);
                }
                break;
            }
        }
    }

    fn window_mut(&mut self, window: u8) -> &mut Panel {
        match window {
            SEARCH => &mut self.search,
            CONFIG => &mut self.config,
            PICKER => &mut self.picker_win,
            EDITOR => &mut self.editor_win,
            w => &mut self.panels[w as usize],
        }
    }

    fn keyboard(&mut self, ui: &Ui) {
        if self.tab != Tab::Modules {
            return;
        }
        if let Some(gm) = self.capturing {
            if let Some(key) = ui.input.pressed_key {
                let bound = if key == bevy::prelude::KeyCode::Escape {
                    Bound::Unbound
                } else {
                    Bound::Key(key)
                };
                store().set_bind_at(gm as usize, bound);
                self.capturing = None;
            } else if let Some(button) = ui.input.pressed_mouse {
                store().set_bind_at(gm as usize, Bound::Mouse(button));
                self.capturing = None;
            }
            return;
        }

        if self.editor.is_some() {
            return;
        }

        if let Some(id) = self.text_focus {
            if ui.input.pressed_key == Some(bevy::prelude::KeyCode::Escape) {
                self.text_boxes[id as usize].focused = false;
                self.text_focus = None;
                return;
            }
            self.text_boxes[id as usize].focused = true;
            if self.text_boxes[id as usize].handle_input(ui.input) {
                let s = self.text_boxes[id as usize].text.clone();
                store().set_text_at(id as usize, &s);
            }
            return;
        }

        self.field.focused = true;
        if self.field.handle_input(ui.input) {
            self.refresh_matches();
        }
        if self.matches.is_empty() {
            return;
        }
        if ui.input.down_arrow {
            self.selected = (self.selected + 1) % self.matches.len();
        }
        if ui.input.up_arrow {
            self.selected = (self.selected + self.matches.len() - 1) % self.matches.len();
        }
        self.match_top = self
            .match_top
            .min(self.selected)
            .max((self.selected + 1).saturating_sub(SEARCH_ROWS))
            .min(self.matches.len().saturating_sub(SEARCH_ROWS));
        if ui.input.enter
            && let Some(&gm) = self.matches.get(self.selected)
        {
            store().toggle_at(gm as usize);
        }
    }

    fn refresh_matches(&mut self) {
        self.matches.clear();
        self.selected = 0;
        self.match_top = 0;
        if self.field.text.is_empty() {
            return;
        }
        let mut gm = 0u16;
        for cat in CATEGORIES {
            for m in cat.modules {
                if store().allowed_at(gm as usize) && contains_ci(m.name, &self.field.text) {
                    self.matches.push(gm);
                }
                gm += 1;
            }
        }
    }

    fn hit(&self, ui: &Ui, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (top, bottom) = self.band;
        let (y0, y1) = (y.max(top), (y + h).min(bottom));
        y1 > y0 && ui.hovering(x, y0, w, y1 - y0)
    }

    fn content_height(&self, pi: usize) -> f32 {
        if self.panels[pi].collapsed {
            return 0.0;
        }
        let cat = &CATEGORIES[pi];
        let base = self.module_base[pi];
        let rows = (0..cat.modules.len())
            .filter(|mi| store().allowed_at((base + *mi as u16) as usize))
            .count();
        let mut h = rows as f32 * self.look.row_h + PAD_BOTTOM;
        if let Some(gm) = self.expanded
            && gm >= base
            && gm < self.module_base[pi + 1]
            && store().allowed_at(gm as usize)
        {
            h += self.settings_height(gm, cat.modules[(gm - base) as usize].settings);
        }
        h
    }

    fn view_height(&self, pi: usize, vh: f32) -> f32 {
        self.fit(self.content_height(pi), self.panels[pi].y, vh)
    }

    fn fit(&self, content_h: f32, y: f32, vh: f32) -> f32 {
        content_h.min((vh - y - HEADER_H - GAP).max(3.0 * self.look.row_h))
    }

    fn picker_w(&self) -> f32 {
        self.picker.as_ref().map_or(PICKER_W, |k| k.list.width)
    }

    fn open_editor(
        &mut self,
        id: u16,
        gm: u16,
        name: &'static str,
        max_len: usize,
        font: &crate::text::Font,
        ui: &Ui,
    ) {
        let mut text = MultiLineTextBox::new(max_len, EDITOR_MAX_LINES);
        text.set_text(&store().text_at(id as usize));
        text.inner.focused = true;
        let lines = text.line_count(font, EDITOR_W - 2.0 * EDITOR_PAD);
        let mut title = String::with_capacity(32);
        title.push_str(self.module_def(gm).name);
        title.push_str(" / ");
        title.push_str(name);
        text.inner.browser_keyboard = true;
        self.editor = Some(Editor {
            id,
            title,
            lines,
            max_len,
            text,
            fresh: true,
        });
        self.picker = None;
        if let Some(old) = self.text_focus.take() {
            self.text_boxes[old as usize].focused = false;
        }
        self.editor_win.scroll = 0.0;
        self.editor_win.collapsed = false;
        let top = GAP + TOP_H + GAP;
        let h = HEADER_H + editor_view_of(lines);
        self.editor_win.x = ((ui.vw - EDITOR_W) * 0.5).floor();
        self.editor_win.y = (top + ((ui.vh - GAP - top - h) / 3.0).max(0.0)).floor();
    }

    fn close_editor(&mut self) {
        if let Some(e) = self.editor.take() {
            self.text_boxes[e.id as usize].set_text(e.text.text());
        }
    }

    fn editor_view_h(&self) -> f32 {
        let Some(e) = self.editor.as_ref() else {
            return 0.0;
        };
        if self.editor_win.collapsed {
            return 0.0;
        }
        editor_view_of(e.lines)
    }

    fn open_picker(&mut self, list: &'static BitList, ui: &Ui) {
        self.close_editor();
        self.picker = Some(Picker::new(list));
        self.picker_query.clear();
        self.picker_win.x = ((ui.vw - list.width) * 0.5).floor();
        self.picker_win.y = GAP + TOP_H + GAP;
        self.picker_win.scroll = 0.0;
        self.picker_win.collapsed = false;
    }

    fn picker_view_h(&self, vh: f32) -> f32 {
        let Some(k) = self.picker.as_ref() else {
            return 0.0;
        };
        if self.picker_win.collapsed {
            return 0.0;
        }
        let presets = k.list.presets;
        let head = FIELD_H + 6.0 + if presets.is_empty() { 0.0 } else { PRESET_H };
        let content = k.rows.len() as f32 * BLOCK_H
            + k.channels.map_or(0.0, |_| 3.0 * CHANNEL_H)
            + PAD_BOTTOM;
        head + self.fit(content, self.picker_win.y + head, vh)
    }

    fn height(&self, pi: usize, vh: f32) -> f32 {
        HEADER_H + self.view_height(pi, vh)
    }

    fn settings_height(&self, gm: u16, defs: &'static [SettingDef]) -> f32 {
        let mut h = 6.0 + BIND_H;
        for (si, d) in defs.iter().enumerate() {
            h += self.setting_height(self.setting_base[gm as usize] + si as u16, d);
        }
        h
    }

    fn setting_height(&self, id: u16, d: &SettingDef) -> f32 {
        match d.kind {
            Kind::Toggle { .. } => TOGGLE_H,
            Kind::Slider { .. } | Kind::Range { .. } => SLIDER_H,
            Kind::Enum { options, .. } => {
                if self.dropdown == Some(id) {
                    ENUM_H + options.len() as f32 * OPTION_H + 2.0
                } else {
                    ENUM_H
                }
            }
            Kind::List { .. } => LIST_H,
            Kind::Text { .. } => TEXTBOX_ROW_H,
        }
    }

    fn search_h(&self) -> f32 {
        let body = if self.matches.is_empty() {
            HINT_H
        } else {
            self.matches.len().min(SEARCH_ROWS) as f32 * self.look.row_h
        };
        HEADER_H + 3.0 + FIELD_H + 3.0 + body + PAD_BOTTOM
    }

    fn top_bar(&mut self, p: &mut Painter, ui: &Ui) {
        let look = self.look;
        let active = self.focus == Focus::Bar && self.drag.is_none();
        let y = GAP;

        let hovered = active && ui.hovering(GAP, y, BUTTON_W, BUTTON_H);
        p.rounded_rect(
            GAP,
            y,
            BUTTON_W,
            BUTTON_H,
            TAB_RADIUS,
            if hovered {
                look.accent()
            } else {
                look.header()
            },
        );
        const LABEL: &str = "Reset layout";
        let tw = p.atlas.font.width_str(LABEL);
        p.text_plain(
            LABEL,
            GAP + ((BUTTON_W - tw) * 0.5).floor(),
            text_y(y, BUTTON_H),
            if hovered { theme::TEXT_ON } else { look.text() },
            look.shadow,
        );
        if hovered && ui.input.left_click {
            self.reset_layout();
        }

        let widths = TABS.map(|(_, name)| (p.atlas.font.width_str(name) + 2.0 * TAB_PAD).floor());
        let strip_w = widths.iter().sum::<f32>() + 2.0 * STRIP_PAD;
        let sx = ((ui.vw - strip_w) * 0.5).floor();
        p.rounded_rect(sx, y, strip_w, BUTTON_H, TAB_RADIUS, look.header());

        let mut tx = sx + STRIP_PAD;
        for (i, (tab, name)) in TABS.into_iter().enumerate() {
            let tw = widths[i];
            let on = self.tab == tab;
            let over = active && ui.hovering(tx, y + 1.0, tw, BUTTON_H - 2.0);
            if on || over {
                p.rounded_rect(
                    tx,
                    y + 1.0,
                    tw,
                    BUTTON_H - 2.0,
                    TAB_RADIUS - 1.0,
                    if on { look.accent() } else { look.row_hover() },
                );
            }
            p.text_plain(
                name,
                (tx + TAB_PAD).floor(),
                text_y(y, BUTTON_H),
                if on { theme::TEXT_ON } else { look.text() },
                look.shadow,
            );
            if over && ui.input.left_click {
                self.tab = tab;
                self.dropdown = None;
            }
            tx += tw;
        }
    }

    fn profile_control(&mut self, p: &mut Painter, ui: &Ui) {
        let active = self.focus == Focus::Bar && self.drag.is_none();
        let x = profile_x(ui.vw);
        self.band = (0.0, ui.vh);
        if !active && ui.input.left_click {
            self.dropdown = self.dropdown.filter(|d| *d != BAR_PROFILE);
        }
        let hovered = active && self.hit(ui, x, GAP, PROFILE_W, BUTTON_H);
        let now = store().profile();
        let picked = self.choice(
            p,
            ui,
            x,
            GAP,
            PROFILE_W,
            "Profile",
            now as u8,
            &Mode::NAMES,
            BAR_PROFILE,
            active,
        );
        if picked != now as u8 {
            store().set_profile(Mode::ALL[picked as usize]);
        }
        if hovered
            && self.dropdown != Some(BAR_PROFILE)
            && let Some(cursor) = ui.mouse()
        {
            self.tip = Some((PROFILE_TIP, cursor.x, cursor.y));
        }
    }

    fn profile_key(&self, ui: &Ui) {
        if self.capturing.is_some() {
            return;
        }
        let step = ui.input.profile_up as i8 - ui.input.profile_down as i8;
        if step != 0 {
            store().set_profile(store().profile().step(step));
        }
    }

    fn tab_key(&mut self, ui: &Ui) {
        if !ui.input.tab {
            return;
        }
        let here = TABS.iter().position(|(t, _)| *t == self.tab).unwrap_or(0);
        let step = if ui.input.shift { TABS.len() - 1 } else { 1 };
        self.tab = TABS[(here + step) % TABS.len()].0;
        self.dropdown = None;
    }

    fn reset_layout(&mut self) {
        for panel in self
            .panels
            .iter_mut()
            .chain([&mut self.search, &mut self.config])
        {
            panel.collapsed = false;
            panel.scroll = 0.0;
        }
        self.order.clear();
        self.order.extend(0..self.panels.len() as u8);
        self.close_editor();
        self.expanded = None;
        self.dropdown = None;
        self.drag = None;
        self.placed = false;
    }

    fn panel(&mut self, p: &mut Painter, ui: &Ui, pi: usize, active: bool) {
        let look = self.look;
        let cat = &CATEGORIES[pi];
        let Panel {
            x, y, collapsed, ..
        } = self.panels[pi];
        let w = look.panel_w;
        let content_h = self.content_height(pi);
        let view_h = self.fit(content_h, y, ui.vh);
        let scroll = self.scroll(pi_window(pi), ui, active, content_h, view_h);

        p.rounded_rect(x, y, w, HEADER_H + view_h, look.radius, look.panel());
        self.title_bar(p, ui, pi as u8, x, y, w, cat.name, collapsed, active);
        if collapsed {
            return;
        }

        let guard = p.push_clip(x, y + HEADER_H, w, view_h);
        self.band = (y + HEADER_H, y + HEADER_H + view_h);
        let outlined = self.outlined();
        let mut cy = y + HEADER_H - scroll;
        for (mi, m) in cat.modules.iter().enumerate() {
            let gm = self.module_base[pi] + mi as u16;
            if !store().allowed_at(gm as usize) {
                continue;
            }
            let on = store().armed_at(gm as usize);
            let hovered = active && self.hit(ui, x, cy, w, look.row_h);
            let bg = match (on, hovered) {
                (true, true) => look.accent_hi(),
                (true, false) => look.accent(),
                (false, true) => look.row_hover(),
                (false, false) => theme::ROW,
            };
            if bg >> 24 != 0 {
                p.fill(x, cy, w, look.row_h, bg);
            }
            if outlined == Some(gm) {
                p.outline(x, cy, w, look.row_h, look.accent_hi());
            }
            self.label(
                p,
                m.name,
                x + 8.0,
                text_y(cy, look.row_h),
                w - 21.0,
                if on { theme::TEXT_ON } else { look.text() },
            );
            theme::dots(
                p,
                x + w - 10.0,
                (cy + look.row_h * 0.5).floor() - 2.0,
                if on { theme::TEXT_ON } else { look.text_dim() },
            );
            if hovered {
                if let Some(cursor) = ui.mouse() {
                    self.tip = Some((m.desc, cursor.x, cursor.y));
                }
                if ui.input.left_click {
                    store().toggle_at(gm as usize);
                }
                if ui.input.right_click {
                    self.expanded = (self.expanded != Some(gm)).then_some(gm);
                    self.dropdown = None;
                    self.text_focus = None;
                }
            }
            cy += look.row_h;

            if self.expanded == Some(gm) {
                cy = self.settings(p, ui, x, cy, gm, m.settings, active);
            }
        }
        p.pop_clip(guard);
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one window chrome, flat arguments"
    )]
    fn title_bar(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        window: u8,
        x: f32,
        y: f32,
        w: f32,
        name: &str,
        collapsed: bool,
        active: bool,
    ) {
        let look = self.look;
        let corners = if collapsed {
            painter::CORNERS_ALL
        } else {
            painter::CORNERS_TOP
        };
        p.rounded_rect_corners(x, y, w, HEADER_H, look.radius, corners, look.header());
        p.text_plain(
            name,
            x + 8.0,
            text_y(y, HEADER_H),
            look.text_title(),
            look.shadow,
        );
        theme::chevron(
            p,
            x + w - 14.0,
            (y + HEADER_H * 0.5).floor() - 2.0,
            7.0,
            !collapsed,
            look.text_dim(),
        );
        if active && ui.hovering(x, y, w, HEADER_H) {
            if ui.input.right_click {
                self.window_mut(window).collapsed = !collapsed;
            } else if ui.input.left_click
                && let Some(m) = ui.mouse()
            {
                self.drag = Some(Drag::Window {
                    window,
                    dx: m.x - x,
                    dy: m.y - y,
                });
            }
        }
    }

    fn scroll(&mut self, window: u8, ui: &Ui, active: bool, content_h: f32, view_h: f32) -> f32 {
        let limit = (content_h - view_h).max(0.0).floor();
        let step = 3.0 * self.look.row_h;
        let panel = self.window_mut(window);
        if active && ui.input.scroll != 0.0 && limit > 0.0 {
            panel.scroll -= ui.input.scroll * step;
        }
        panel.scroll = panel.scroll.clamp(0.0, limit);
        panel.scroll
    }

    fn outlined(&self) -> Option<u16> {
        self.matches.get(self.selected).copied()
    }

    fn dragging_scale(&self) -> bool {
        matches!(self.drag, Some(Drag::Slider { id: CFG_SCALE, .. }))
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn settings(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        gm: u16,
        defs: &'static [SettingDef],
        active: bool,
    ) -> f32 {
        let look = self.look;
        let h = self.settings_height(gm, defs);
        p.fill(x, y, look.panel_w, h, look.nest());
        p.fill(x, y, 1.5, h, look.accent());

        let ix = x + INDENT;
        let rx = x + look.panel_w - RESET_EDGE - RESET_W;
        let iw = rx - ix - 2.0;
        let mut sy = y + 3.0;
        self.bind_row(p, ui, ix, sy, rx - ix - 2.0, gm, active);
        sy += BIND_H;
        for (si, d) in defs.iter().enumerate() {
            let id = self.setting_base[gm as usize] + si as u16;
            let row_h = self.setting_height(id, d);
            if active && !d.tip.is_empty() && self.hit(ui, x, sy, look.panel_w, row_h) {
                if let Some(cursor) = ui.mouse() {
                    self.tip = Some((d.tip, cursor.x, cursor.y));
                }
            }
            let label_y = match d.kind {
                Kind::Toggle { .. } => text_y(sy, TOGGLE_H),
                Kind::Slider { .. }
                | Kind::Range { .. }
                | Kind::List { .. }
                | Kind::Text { .. } => sy + 1.0,
                Kind::Enum { .. } => text_y(sy, ENUM_H - 3.0),
            };
            let changed = if let Kind::Text { default, .. } = d.kind {
                store().text_at(id as usize) != default
            } else {
                !is_default(store().value(id as usize), d.kind)
            };
            if self.revert(p, ui, rx, label_y, changed, active) {
                if let Kind::Text { default, .. } = d.kind {
                    store().set_text_at(id as usize, default);
                    self.text_boxes[id as usize].set_text(default);
                    if self.text_focus == Some(id) {
                        self.text_focus = None;
                    }
                    if let Some(e) = self.editor.as_mut().filter(|e| e.id == id) {
                        e.text.set_text(default);
                    }
                } else {
                    store().set_value(id as usize, Value::default_of(d.kind));
                }
                if self.dropdown == Some(id) {
                    self.dropdown = None;
                }
            }
            match (d.kind, store().value(id as usize)) {
                (Kind::Toggle { .. }, Value::Bool(on)) => {
                    let on = self.toggle(p, ui, ix, sy, iw, d.name, on, active);
                    store().set_value(id as usize, Value::Bool(on));
                }
                (
                    Kind::Slider {
                        min, max, decimals, ..
                    },
                    Value::Num(v),
                ) => {
                    let v =
                        self.slider(p, ui, ix, sy, iw, d.name, v, min, max, decimals, id, active);
                    store().set_value(id as usize, Value::Num(v));
                }
                (
                    Kind::Range {
                        min, max, decimals, ..
                    },
                    Value::Range(lo, hi),
                ) => {
                    let (lo, hi) = self.range(
                        p, ui, ix, sy, iw, d.name, lo, hi, min, max, decimals, id, active,
                    );
                    store().set_value(id as usize, Value::Range(lo, hi));
                }
                (Kind::Enum { options, .. }, Value::Choice(index)) => {
                    let index = self.choice(p, ui, ix, sy, iw, d.name, index, options, id, active);
                    store().set_value(id as usize, Value::Choice(index));
                }
                (Kind::List { which }, _) => {
                    self.list_button(p, ui, ix, sy, iw, d.name, which, active);
                }
                (
                    Kind::Text {
                        max_len, expand, ..
                    },
                    _,
                ) => {
                    self.text_field(p, ui, ix, sy, iw, d.name, id, gm, max_len, expand, active);
                }
                _ => {}
            }
            sy += self.setting_height(id, d);
        }
        y + h
    }

    fn search_window(&mut self, p: &mut Painter, ui: &Ui) {
        let look = self.look;
        let active = self.focus == Focus::Search && self.drag.is_none();
        let Panel {
            x, y, collapsed, ..
        } = self.search;
        let h = if collapsed { HEADER_H } else { self.search_h() };
        p.rounded_rect(x, y, self.look.panel_w, h, look.radius, look.solid());
        self.title_bar(
            p,
            ui,
            SEARCH,
            x,
            y,
            self.look.panel_w,
            "Search",
            collapsed,
            active,
        );
        if collapsed {
            return;
        }

        let fx = x + 5.0;
        let fw = self.look.panel_w - 10.0;
        let fy = y + HEADER_H + 3.0;
        p.rounded_rect(fx, fy, fw, FIELD_H, 3.0, look.nest());
        p.fill(fx + 2.0, fy + FIELD_H - 1.0, fw - 4.0, 1.0, look.accent());
        self.mouse_input.mouse = ui.mouse;
        self.mouse_input.left_click = ui.input.left_click && active;
        self.mouse_input.left_down = ui.input.left_down;
        self.mouse_input.left_release = ui.input.left_release;
        self.mouse_input.shift = ui.input.shift;
        self.mouse_input.double_click = ui.input.double_click;
        self.mouse_input.triple_click = ui.input.triple_click;
        self.field.color = look.text_title();
        let guard = p.push_clip(fx + 3.0, fy, fw - 6.0, FIELD_H);
        self.field.handle_mouse(
            &self.mouse_input,
            &p.atlas.font,
            fx + 4.0,
            fy,
            fw - 8.0,
            FIELD_H,
        );
        let frame = p.frame;
        if self.field.text.is_empty() {
            p.text_plain(
                "Type to search",
                fx + 9.0,
                text_y(fy, FIELD_H),
                look.text_dim(),
                look.shadow,
            );
        }
        self.field.draw(p, fx + 4.0, fy, fw - 8.0, FIELD_H, frame);
        p.pop_clip(guard);

        let mut ry = fy + FIELD_H + 3.0;
        self.band = (ry, ry + SEARCH_ROWS as f32 * look.row_h);
        let shown = self.matches.len().min(SEARCH_ROWS);
        for i in 0..shown {
            let Some(&gm) = self.matches.get(self.match_top + i) else {
                break;
            };
            let on = store().armed_at(gm as usize);
            let picked = self.match_top + i == self.selected;
            let hovered = active && self.hit(ui, x, ry, self.look.panel_w, look.row_h);
            let bg = match (on, hovered) {
                (true, true) => look.accent_hi(),
                (true, false) => look.accent(),
                (false, true) => look.row_hover(),
                (false, false) => theme::ROW,
            };
            if bg >> 24 != 0 {
                p.fill(x, ry, self.look.panel_w, look.row_h, bg);
            }
            if picked {
                p.outline(x, ry, self.look.panel_w, look.row_h, look.accent_hi());
            }
            let (cat, def) = self.module_at(gm);
            let name = def.name;
            let cat = cat_tag(cat);
            let cw = cat_w(p, cat);
            self.label(
                p,
                name,
                x + 8.0,
                text_y(ry, look.row_h),
                self.look.panel_w - 22.0 - cw,
                if on { theme::TEXT_ON } else { look.text() },
            );
            p.text_plain(
                cat,
                x + self.look.panel_w - 8.0 - cw,
                text_y(ry, look.row_h),
                if on { theme::TEXT_ON } else { look.text_dim() },
                look.shadow,
            );
            if hovered {
                if let Some(cursor) = ui.mouse() {
                    self.tip = Some((self.module_def(gm).desc, cursor.x, cursor.y));
                }
                if ui.input.left_click {
                    store().toggle_at(gm as usize);
                    self.selected = self.match_top + i;
                }
            }
            ry += look.row_h;
        }
        if self.matches.is_empty() {
            let line = if self.field.text.is_empty() {
                "Enter toggles"
            } else {
                "No match"
            };
            self.label(
                p,
                line,
                x + 8.0,
                ry + 2.0,
                self.look.panel_w - 16.0,
                look.text_dim(),
            );
        }
    }

    fn module_at(&self, gm: u16) -> (&'static str, &'static model::ModuleDef) {
        for (pi, cat) in CATEGORIES.iter().enumerate() {
            let base = self.module_base[pi];
            if gm >= base && gm < self.module_base[pi + 1] {
                return (cat.name, &cat.modules[(gm - base) as usize]);
            }
        }
        (CATEGORIES[0].name, &CATEGORIES[0].modules[0])
    }

    fn module_def(&self, gm: u16) -> &'static model::ModuleDef {
        self.module_at(gm).1
    }

    fn picker_window(&mut self, p: &mut Painter, ui: &Ui) {
        let Some(list) = self.picker.as_ref().map(|k| k.list) else {
            return;
        };
        let look = self.look;
        let active = self.focus == Focus::Picker && self.drag.is_none();
        let Panel {
            x, y, collapsed, ..
        } = self.picker_win;

        self.picker_query.focused = true;
        let dirty = self.picker_query.handle_input(ui.input);
        let presets = list.presets;
        let head = HEADER_H + FIELD_H + 6.0 + if presets.is_empty() { 0.0 } else { PRESET_H };

        if let Some(k) = self.picker.as_mut() {
            k.rebuild(&self.picker_query.text, dirty);
        }
        let rows = self.picker.as_ref().map_or(0, |k| k.rows.len());
        let content = rows as f32 * BLOCK_H
            + self
                .picker
                .as_ref()
                .and_then(|k| k.channels)
                .map_or(0.0, |_| 3.0 * CHANNEL_H)
            + PAD_BOTTOM;
        let view = self.fit(content, y + head - HEADER_H, ui.vh);
        let scroll = self.scroll(PICKER, ui, active, content, view);

        let pw = list.width;
        p.rounded_rect(x, y, pw, head + view, look.radius, look.raised());
        self.title_bar(p, ui, PICKER, x, y, pw, list.title, collapsed, active);
        if collapsed {
            return;
        }

        let (fx, fw, fy) = (x + 5.0, pw - 10.0, y + HEADER_H + 3.0);
        p.rounded_rect(fx, fy, fw, FIELD_H, 3.0, look.nest());
        p.fill(fx + 2.0, fy + FIELD_H - 1.0, fw - 4.0, 1.0, look.accent());
        self.picker_query.color = look.text_title();
        self.mouse_input.mouse = ui.mouse;
        self.mouse_input.left_click = ui.input.left_click && active;
        self.mouse_input.left_down = ui.input.left_down;
        self.mouse_input.left_release = ui.input.left_release;
        self.mouse_input.shift = ui.input.shift;
        let guard = p.push_clip(fx + 3.0, fy, fw - 6.0, FIELD_H);
        self.picker_query.handle_mouse(
            &self.mouse_input,
            &p.atlas.font,
            fx + 4.0,
            fy,
            fw - 8.0,
            FIELD_H,
        );
        if self.picker_query.text.is_empty() {
            p.text_plain(
                "Search",
                fx + 9.0,
                text_y(fy, FIELD_H),
                look.text_dim(),
                look.shadow,
            );
        }
        let frame = p.frame;
        self.picker_query
            .draw(p, fx + 4.0, fy, fw - 8.0, FIELD_H, frame);
        p.pop_clip(guard);

        if !presets.is_empty() {
            let py = fy + FIELD_H + 3.0;
            let step = (pw - 10.0) / presets.len() as f32;
            for (i, (label, _)) in presets.iter().enumerate() {
                let bx = (x + 5.0 + i as f32 * step).floor();
                let bw = step.floor() - 2.0;
                let over = active && self.hit(ui, bx, py, bw, PRESET_H - 4.0);
                p.rounded_rect(
                    bx,
                    py,
                    bw,
                    PRESET_H - 4.0,
                    3.0,
                    if over { look.accent() } else { look.header() },
                );
                let lw = p.atlas.font.width_str(label);
                p.text_plain(
                    label,
                    (bx + (bw - lw) * 0.5).floor(),
                    text_y(py, PRESET_H - 4.0),
                    if over { theme::TEXT_ON } else { look.text() },
                    look.shadow,
                );
                if over && ui.input.left_click {
                    list.apply_preset(i);
                }
            }
        }

        let top = y + head;
        let guard = p.push_clip(x, top, pw, view);
        self.band = (top, top + view);
        let first = (scroll / BLOCK_H).floor().max(0.0) as usize;
        let last = (((scroll + view) / BLOCK_H).ceil() as usize + 1).min(rows);
        let mut ry = top - scroll + first as f32 * BLOCK_H;
        for r in first..last {
            let Some(row) = self.picker.as_ref().and_then(|k| k.rows.get(r).copied()) else {
                break;
            };
            match row {
                Row::Header(g) => self.picker_header(p, ui, x, ry, list, g as usize, active),
                Row::Item(i) => {
                    ry = self.picker_item(p, ui, x, ry, list, i as usize, active);
                    continue;
                }
            }
            ry += BLOCK_H;
        }
        p.pop_clip(guard);
    }

    fn overlay_scrim(&self, p: &mut Painter, ui: &Ui) {
        if self.picker.is_none() && self.editor.is_none() {
            return;
        }
        let m = ui.vw.max(ui.vh);
        p.fill(-m, -m, ui.vw + 2.0 * m, ui.vh + 2.0 * m, OVERLAY_SCRIM);
    }

    fn editor_window(&mut self, p: &mut Painter, ui: &Ui) {
        let Some(mut e) = self.editor.take() else {
            return;
        };
        let look = self.look;
        let active = self.focus == Focus::Editor && self.drag.is_none();
        let Panel {
            x, y, collapsed, ..
        } = self.editor_win;
        let inner_w = EDITOR_W - 2.0 * EDITOR_PAD;

        if ui.input.enter {
            self.editor = Some(e);
            self.close_editor();
            return;
        }
        if !e.fresh && !active && (ui.input.left_click || ui.input.right_click) {
            self.editor = Some(e);
            self.close_editor();
            return;
        }
        e.fresh = false;

        e.text.inner.focused = true;
        if e.text.handle_input(ui.input, &p.atlas.font, inner_w) {
            store().set_text_at(e.id as usize, e.text.text());
        }
        e.lines = e.text.line_count(&p.atlas.font, inner_w);
        let view = if collapsed {
            0.0
        } else {
            editor_view_of(e.lines)
        };

        p.rounded_rect(x, y, EDITOR_W, HEADER_H + view, look.radius, look.raised());
        self.title_bar(p, ui, EDITOR, x, y, EDITOR_W, &e.title, collapsed, active);
        if collapsed {
            self.editor = Some(e);
            return;
        }

        let box_h = editor_box_h(e.lines);
        let (bx, by, bw) = (x + 5.0, y + HEADER_H + 3.0, EDITOR_W - 10.0);
        p.rounded_rect(bx, by, bw, box_h, 3.0, look.nest());
        p.fill(bx + 2.0, by + box_h - 1.0, bw - 4.0, 1.0, look.accent());

        self.band = (y + HEADER_H, y + HEADER_H + view);
        self.mouse_input.mouse = ui.mouse;
        self.mouse_input.left_click = ui.input.left_click && active;
        self.mouse_input.left_down = ui.input.left_down;
        self.mouse_input.left_release = ui.input.left_release;
        self.mouse_input.shift = ui.input.shift;
        self.mouse_input.double_click = ui.input.double_click;
        self.mouse_input.triple_click = ui.input.triple_click;
        let tx = x + EDITOR_PAD;
        let ty = by + 3.0;
        e.text
            .handle_mouse(&self.mouse_input, &p.atlas.font, tx, ty, inner_w);
        let frame = p.frame;
        e.text.draw(p, tx, ty, inner_w, look.text_title(), frame);

        let used = e.text.text().chars().count();
        self.scratch.clear();
        {
            use std::fmt::Write;
            let _ = write!(self.scratch, "{used}/{}", e.max_len);
        }
        let cw = p.atlas.font.width_str(&self.scratch);
        let cy = by + box_h + 4.0;
        p.text_plain(
            &self.scratch,
            (x + EDITOR_W - 5.0 - cw).floor(),
            cy,
            if used >= e.max_len {
                look.accent()
            } else {
                look.text_dim()
            },
            look.shadow,
        );
        p.text_plain("Enter to close", x + 5.0, cy, look.text_dim(), look.shadow);

        self.editor = Some(e);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn picker_header(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        list: &'static BitList,
        g: usize,
        active: bool,
    ) {
        let look = self.look;
        let open = self.picker.as_ref().is_some_and(|k| k.is_open(g));
        let (on, total) = list.group_on(g);
        let pw = list.width;
        let hovered = active && self.hit(ui, x, y, pw, BLOCK_H);
        p.fill(x, y, pw, BLOCK_H, look.nest());
        theme::chevron(
            p,
            x + 5.0,
            (y + BLOCK_H * 0.5).floor() - 2.0,
            6.0,
            !open,
            look.text_dim(),
        );
        p.text_plain(
            list.groups[g],
            x + 14.0,
            text_y(y, BLOCK_H),
            look.text_title(),
            look.shadow,
        );
        self.scratch.clear();
        {
            use std::fmt::Write;
            let _ = write!(self.scratch, "{on}/{total}");
        }
        let cw = p.atlas.font.width_str(&self.scratch);
        let sw = 22.0;
        p.text_plain(
            &self.scratch,
            x + pw - sw - cw - 4.0,
            text_y(y, BLOCK_H),
            look.text_dim(),
            look.shadow,
        );
        let all = on == total && total > 0;
        let (sww, ph) = (16.0, 8.0);
        let (px, py) = (x + pw - sww - 4.0, y + (BLOCK_H - ph) * 0.5);
        let over = active && self.hit(ui, px, y, sww + 4.0, BLOCK_H);
        p.rounded_rect(
            px,
            py,
            sww,
            ph,
            ph * 0.5,
            if all { look.accent() } else { look.track() },
        );
        let knob = ph - 1.0;
        let kx = if all { px + sww - knob - 0.5 } else { px + 0.5 };
        p.rounded_rect(kx, py + 0.5, knob, knob, knob * 0.5, theme::KNOB);
        if over && ui.input.left_click {
            list.set_group(g, !all);
            return;
        }
        if hovered
            && ui.input.left_click
            && let Some(k) = self.picker.as_mut()
        {
            k.toggle_group(g);
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn picker_item(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        list: &'static BitList,
        i: usize,
        active: bool,
    ) -> f32 {
        let look = self.look;
        let on = list.enabled(i);
        let has_color = list.colored;
        let chip = if has_color { 9.0 } else { 0.0 };
        let inset = 10.0;
        let pw = list.width;
        let w = pw - inset - 6.0 - chip - if has_color { 4.0 } else { 0.0 };

        if self.toggle(p, ui, x + inset, y, w, list.label(i), on, active) != on {
            list.set_enabled(i, !on);
        }

        let mut ry = y + BLOCK_H;
        if let Some(rgb) = list.colored.then(|| list.color(i)) {
            let open = self.picker.as_ref().and_then(|k| k.channels) == Some(i as u32);
            let cx = x + pw - chip - 6.0;
            let over = active && self.hit(ui, cx - 1.0, y + 1.0, chip + 2.0, chip);
            if open || over {
                p.rounded_rect(cx - 1.5, y + 0.5, chip + 3.0, chip + 3.0, 3.0, theme::KNOB);
            }
            p.rounded_rect(cx, y + 2.0, chip, chip - 2.0, 2.0, 0xFF00_0000 | rgb);
            if over
                && ui.input.left_click
                && let Some(k) = self.picker.as_mut()
            {
                k.channels = (!open).then_some(i as u32);
            }
            if open {
                let mut out = 0u32;
                for (c, (name, shift)) in [("R", 16), ("G", 8), ("B", 0)].into_iter().enumerate() {
                    let v = ((rgb >> shift) & 0xFF) as f32;
                    let id = CFG_ID + 0x200 + c as u16;
                    let v = self
                        .slider(
                            p,
                            ui,
                            x + inset,
                            ry + 3.0,
                            pw - inset - 8.0,
                            name,
                            v,
                            0.0,
                            255.0,
                            0,
                            id,
                            active,
                        )
                        .round()
                        .clamp(0.0, 255.0) as u32;
                    out |= v << shift;
                    ry += CHANNEL_H;
                }
                if out != rgb {
                    list.set_color(i, out);
                }
            }
        }
        ry
    }

    const CFG_ROWS: [f32; 11] = [
        ENUM_H, SWATCH_H, SWATCH_H, SLIDER_H, SLIDER_H, SLIDER_H, SLIDER_H, SLIDER_H, TOGGLE_H,
        TOGGLE_H, TOGGLE_H,
    ];

    fn profile_open_h(&self) -> f32 {
        if self.dropdown == Some(CFG_PROFILE) {
            Mode::ALL.len() as f32 * OPTION_H + 2.0
        } else {
            0.0
        }
    }

    fn config_content_h(&self) -> f32 {
        6.0 + Self::CFG_ROWS.iter().sum::<f32>() + self.profile_open_h()
    }

    fn config_view_h(&self, vh: f32) -> f32 {
        self.fit(self.config_content_h(), self.config.y, vh)
    }

    fn config_window(&mut self, p: &mut Painter, ui: &Ui) {
        let look = self.look;
        let active = self.focus == Focus::Config && self.drag.is_none();
        let Panel {
            x, y, collapsed, ..
        } = self.config;
        let content_h = self.config_content_h();
        let view_h = self.fit(content_h, y, ui.vh);
        let scroll = self.scroll(CONFIG, ui, active, content_h, view_h);

        p.rounded_rect(x, y, CONFIG_W, HEADER_H + view_h, look.radius, look.panel());
        self.title_bar(p, ui, CONFIG, x, y, CONFIG_W, "Config", collapsed, active);
        if collapsed {
            return;
        }

        let guard = p.push_clip(x, y + HEADER_H, CONFIG_W, view_h);
        self.band = (y + HEADER_H, y + HEADER_H + view_h);
        let ix = x + INDENT;
        let rx = x + CONFIG_W - RESET_EDGE - RESET_W;
        let iw = rx - ix - 2.0;
        let mut cy = y + HEADER_H + 3.0 - scroll;
        let def = Look::default();

        let profile = store().profile();
        let picked = self.choice(
            p,
            ui,
            ix,
            cy,
            iw,
            "Profile",
            profile as u8,
            &Mode::NAMES,
            CFG_PROFILE,
            active,
        );
        if picked != profile as u8 {
            store().set_profile(Mode::ALL[picked as usize]);
        }
        cy += ENUM_H + self.profile_open_h();

        self.look.accent =
            self.swatches(p, ui, ix, cy, iw, "Accent", self.look.accent, true, active);
        if self.revert(p, ui, rx, cy + 1.0, look.accent != def.accent, active) {
            self.look.accent = def.accent;
        }
        cy += SWATCH_H;

        self.look.surface = self.swatches(
            p,
            ui,
            ix,
            cy,
            iw,
            "Surface",
            self.look.surface,
            false,
            active,
        );
        if self.revert(p, ui, rx, cy + 1.0, look.surface != def.surface, active) {
            self.look.surface = def.surface;
        }
        cy += SWATCH_H;

        let s = self.look.scale * 100.0;
        let s = self.slider(
            p,
            ui,
            ix,
            cy,
            iw,
            "Scale",
            s,
            SCALE_MIN * 100.0,
            SCALE_MAX * 100.0,
            0,
            CFG_SCALE,
            active,
        );
        self.look.scale = (s / 100.0).clamp(SCALE_MIN, SCALE_MAX);
        self.scale_set |= self.dragging_scale();
        if self.revert(p, ui, rx, cy + 1.0, self.scale_set, active) {
            self.look.scale = def.scale;
            self.scale_set = false;
            self.placed = false;
        }
        cy += SLIDER_H;

        let o = self.look.opacity * 100.0;
        let o = self.slider(
            p,
            ui,
            ix,
            cy,
            iw,
            "Opacity",
            o,
            30.0,
            100.0,
            0,
            CFG_ID + 1,
            active,
        );
        self.look.opacity = o / 100.0;
        if self.revert(
            p,
            ui,
            rx,
            cy + 1.0,
            differs(look.opacity, def.opacity),
            active,
        ) {
            self.look.opacity = def.opacity;
        }
        cy += SLIDER_H;

        self.look.radius = self.slider(
            p,
            ui,
            ix,
            cy,
            iw,
            "Corner radius",
            look.radius,
            0.0,
            8.0,
            1,
            CFG_ID + 2,
            active,
        );
        if self.revert(
            p,
            ui,
            rx,
            cy + 1.0,
            differs(look.radius, def.radius),
            active,
        ) {
            self.look.radius = def.radius;
        }
        cy += SLIDER_H;

        self.look.row_h = self
            .slider(
                p,
                ui,
                ix,
                cy,
                iw,
                "Row height",
                look.row_h,
                11.0,
                18.0,
                0,
                CFG_ID + 3,
                active,
            )
            .round();
        if self.revert(p, ui, rx, cy + 1.0, differs(look.row_h, def.row_h), active) {
            self.look.row_h = def.row_h;
        }
        cy += SLIDER_H;

        self.look.panel_w = self
            .slider(
                p,
                ui,
                ix,
                cy,
                iw,
                "Column width",
                look.panel_w,
                86.0,
                150.0,
                0,
                CFG_ID + 4,
                active,
            )
            .round();
        if self.revert(
            p,
            ui,
            rx,
            cy + 1.0,
            differs(look.panel_w, def.panel_w),
            active,
        ) {
            self.look.panel_w = def.panel_w;
        }
        cy += SLIDER_H;

        self.look.scrim = self.toggle(p, ui, ix, cy, iw, "Dim background", look.scrim, active);
        if self.revert(
            p,
            ui,
            rx,
            text_y(cy, TOGGLE_H),
            look.scrim != def.scrim,
            active,
        ) {
            self.look.scrim = def.scrim;
        }
        cy += TOGGLE_H;
        self.look.animate = self.toggle(p, ui, ix, cy, iw, "Open animation", look.animate, active);
        if self.revert(
            p,
            ui,
            rx,
            text_y(cy, TOGGLE_H),
            look.animate != def.animate,
            active,
        ) {
            self.look.animate = def.animate;
        }
        cy += TOGGLE_H;
        self.look.shadow = self.toggle(p, ui, ix, cy, iw, "Text shadow", look.shadow, active);
        if self.revert(
            p,
            ui,
            rx,
            text_y(cy, TOGGLE_H),
            look.shadow != def.shadow,
            active,
        ) {
            self.look.shadow = def.shadow;
        }

        p.pop_clip(guard);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn swatches(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        name: &str,
        index: u8,
        accents: bool,
        active: bool,
    ) -> u8 {
        let shadow = self.look.shadow;
        let count = if accents {
            theme::ACCENTS.len()
        } else {
            theme::SURFACES.len()
        };
        let picked_name = if accents {
            theme::ACCENTS[index as usize % count].name
        } else {
            theme::SURFACES[index as usize % count].name
        };
        let nw = p.atlas.font.width_str(picked_name);
        p.text_plain(
            picked_name,
            x + w - nw,
            y + 1.0,
            self.look.text_dim(),
            shadow,
        );
        self.label(p, name, x, y + 1.0, w - nw - 4.0, self.look.text());
        let step = SWATCH + 3.0;
        let sy = y + 11.0;
        let mut picked = index;
        for i in 0..count {
            let sx = x + i as f32 * step;
            let rgb = if accents {
                theme::ACCENTS[i].base
            } else {
                theme::lighten(theme::SURFACES[i].base, 0x1E)
            };
            let over = active && self.hit(ui, sx, sy, SWATCH, SWATCH);
            let on = i as u8 == index;
            if on || over {
                p.rounded_rect(
                    sx - 1.5,
                    sy - 1.5,
                    SWATCH + 3.0,
                    SWATCH + 3.0,
                    4.0,
                    if on {
                        theme::KNOB
                    } else {
                        self.look.row_hover()
                    },
                );
            }
            p.rounded_rect(sx, sy, SWATCH, SWATCH, 3.0, 0xFF00_0000 | rgb);
            if over && ui.input.left_click {
                picked = i as u8;
            }
        }
        picked
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn toggle(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        name: &str,
        on: bool,
        active: bool,
    ) -> bool {
        let look = self.look;
        let hovered = active && self.hit(ui, x, y, w, TOGGLE_H);
        let (pw, ph) = (16.0, 8.0);
        self.label(
            p,
            name,
            x,
            text_y(y, TOGGLE_H),
            w - pw - 4.0,
            if on { look.text() } else { look.text_dim() },
        );
        let (px, py) = (x + w - pw, y + (TOGGLE_H - ph) * 0.5);
        p.rounded_rect(
            px,
            py,
            pw,
            ph,
            ph * 0.5,
            if on { look.accent() } else { look.track() },
        );
        let knob = ph - 1.0;
        let kx = if on { px + pw - knob - 0.5 } else { px + 0.5 };
        p.rounded_rect(kx, py + 0.5, knob, knob, knob * 0.5, theme::KNOB);
        on ^ (hovered && ui.input.left_click)
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn list_button(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        name: &str,
        which: ListKind,
        active: bool,
    ) {
        let look = self.look;
        let list = list::of(which);
        let open = self
            .picker
            .as_ref()
            .is_some_and(|k| std::ptr::eq(k.list, list));
        self.scratch.clear();
        {
            use std::fmt::Write;
            let _ = write!(self.scratch, "{}", list.selected());
        }
        let bw = (p.atlas.font.width_str(&self.scratch) + 16.0).max(30.0);
        let bx = x + w - bw;
        let hovered = active && self.hit(ui, bx, y, bw, LIST_H - 2.0);

        self.label(p, name, x, text_y(y, LIST_H), w - bw - 4.0, look.text());
        p.rounded_rect(
            bx,
            y,
            bw,
            LIST_H - 2.0,
            3.0,
            if open {
                look.accent()
            } else if hovered {
                look.nest_hi()
            } else {
                look.header()
            },
        );
        p.text_plain(
            &self.scratch,
            bx + 5.0,
            text_y(y, LIST_H - 2.0),
            if open { theme::TEXT_ON } else { look.text() },
            look.shadow,
        );
        theme::chevron(
            p,
            bx + bw - 9.0,
            (y + (LIST_H - 2.0) * 0.5).floor() - 2.0,
            6.0,
            !open,
            if open {
                theme::TEXT_ON
            } else {
                look.text_dim()
            },
        );
        if hovered && ui.input.left_click {
            if open {
                self.picker = None;
            } else {
                self.open_picker(list, ui);
            }
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn text_field(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        name: &'static str,
        id: u16,
        gm: u16,
        max_len: usize,
        expand: bool,
        active: bool,
    ) {
        let look = self.look;
        let cw = if expand { 9.0 } else { 0.0 };
        self.label(p, name, x, y + 1.0, w - cw, look.text());
        if expand {
            let cx = x + w - 7.0;
            let over = active && self.hit(ui, cx - 2.0, y, cw, 11.0);
            theme::chevron(
                p,
                cx,
                y + 3.0,
                6.0,
                true,
                if over { look.accent() } else { look.text_dim() },
            );
            if over && ui.input.left_click {
                self.open_editor(id, gm, name, max_len, &p.atlas.font, ui);
            }
        }

        let fy = y + 13.0;
        p.rounded_rect(x, fy, w, FIELD_H, 3.0, look.nest());

        let hovered = active && self.hit(ui, x, fy, w, FIELD_H);
        if active && ui.input.left_click {
            if hovered && self.text_focus != Some(id) {
                if let Some(old) = self.text_focus {
                    self.text_boxes[old as usize].focused = false;
                }
                self.text_focus = Some(id);
            } else if !hovered && self.text_focus == Some(id) {
                self.text_boxes[id as usize].focused = false;
                self.text_focus = None;
            }
        }
        let focused = self.text_focus == Some(id);
        if focused {
            p.fill(x + 2.0, fy + FIELD_H - 1.0, w - 4.0, 1.0, look.accent());
        }

        self.mouse_input.mouse = ui.mouse;
        self.mouse_input.left_click = ui.input.left_click && active;
        self.mouse_input.left_down = ui.input.left_down;
        self.mouse_input.left_release = ui.input.left_release;
        self.mouse_input.shift = ui.input.shift;
        self.mouse_input.double_click = ui.input.double_click;
        self.mouse_input.triple_click = ui.input.triple_click;

        let field = &mut self.text_boxes[id as usize];
        field.color = look.text_title();
        let guard = p.push_clip(x + 3.0, fy, w - 6.0, FIELD_H);
        field.handle_mouse(
            &self.mouse_input,
            &p.atlas.font,
            x + 4.0,
            fy,
            w - 8.0,
            FIELD_H,
        );
        if field.text.is_empty() && !focused {
            p.text_plain(
                "Type a phrase",
                x + 9.0,
                text_y(fy, FIELD_H),
                look.text_dim(),
                look.shadow,
            );
        }
        let frame = p.frame;
        self.text_boxes[id as usize].draw(p, x + 4.0, fy, w - 8.0, FIELD_H, frame);
        p.pop_clip(guard);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn bind_row(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        gm: u16,
        active: bool,
    ) {
        let look = self.look;
        let capturing = self.capturing == Some(gm);
        let bound = store().bind_at(gm as usize);
        let label = if capturing {
            "> ? <"
        } else {
            bound.display_name()
        };
        let lw = p.atlas.font.width_str(label);
        let bw = (lw + 8.0).max(28.0);
        let bx = x + w - bw;
        let hovered = active && self.hit(ui, bx, y, bw, BIND_H - 1.0);

        self.label(
            p,
            "Bind",
            x,
            text_y(y, BIND_H),
            w - bw - 4.0,
            look.text_dim(),
        );
        p.rounded_rect(
            bx,
            y,
            bw,
            BIND_H - 1.0,
            3.0,
            if capturing {
                look.accent()
            } else if hovered {
                look.nest_hi()
            } else {
                look.header()
            },
        );
        p.text_plain(
            label,
            (bx + (bw - lw) * 0.5).floor(),
            text_y(y, BIND_H - 1.0),
            if capturing {
                theme::TEXT_ON
            } else if bound == Bound::Unbound {
                look.text_dim()
            } else {
                look.text()
            },
            look.shadow,
        );
        if hovered && ui.input.left_click {
            self.capturing = (!capturing).then_some(gm);
        }

        let rx = x + w + 2.0;
        if self.revert(
            p,
            ui,
            rx,
            text_y(y, BIND_H),
            bound != default_bind(gm),
            active,
        ) {
            store().set_bind_at(gm as usize, default_bind(gm));
            self.capturing = None;
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn slider(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        name: &str,
        value: f32,
        min: f32,
        max: f32,
        decimals: u8,
        id: u16,
        active: bool,
    ) -> f32 {
        let look = self.look;
        let mut v = value;
        let ty = y + 13.0;
        if self.grab(ui, id, 0, x, w, y, active)
            && let Some(t) = self.track_t(ui, x, w)
        {
            v = min + (max - min) * t;
        }
        let f = ((v - min) / (max - min).max(f32::EPSILON)).clamp(0.0, 1.0);

        self.number(v, decimals);
        let tw = p.atlas.font.width_str(&self.scratch);
        p.text_plain(
            &self.scratch,
            x + w - tw,
            y + 1.0,
            look.text_dim(),
            look.shadow,
        );
        self.label(p, name, x, y + 1.0, w - tw - 4.0, look.text());

        p.rounded_rect(x, ty, w, 2.0, 1.0, look.track());
        p.rounded_rect(x, ty, w * f, 2.0, 1.0, look.accent());
        p.rounded_rect(x + w * f - 3.0, ty - 2.0, 6.0, 6.0, 3.0, theme::KNOB);
        v
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn range(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        name: &str,
        low: f32,
        high: f32,
        min: f32,
        max: f32,
        decimals: u8,
        id: u16,
        active: bool,
    ) -> (f32, f32) {
        let look = self.look;
        let (mut lo, mut hi) = (low, high);
        let ty = y + 13.0;
        let span = (max - min).max(f32::EPSILON);

        let nearer = ui
            .mouse()
            .map(|m| {
                let t = ((m.x - x) / w).clamp(0.0, 1.0);
                u8::from((t - (hi - min) / span).abs() < (t - (lo - min) / span).abs())
            })
            .unwrap_or(0);
        if self.grab(ui, id, nearer, x, w, y, active)
            && let Some(t) = self.track_t(ui, x, w)
            && let Some(Drag::Slider { handle, .. }) = self.drag
        {
            let v = min + span * t;
            if handle == 0 {
                lo = v.min(hi);
            } else {
                hi = v.max(lo);
            }
        }
        let (fl, fh) = (
            ((lo - min) / span).clamp(0.0, 1.0),
            ((hi - min) / span).clamp(0.0, 1.0),
        );

        self.number(lo, decimals);
        self.scratch.push_str(" - ");
        self.append(hi, decimals);
        let tw = p.atlas.font.width_str(&self.scratch);
        p.text_plain(
            &self.scratch,
            x + w - tw,
            y + 1.0,
            look.text_dim(),
            look.shadow,
        );
        self.label(p, name, x, y + 1.0, w - tw - 4.0, look.text());

        p.rounded_rect(x, ty, w, 2.0, 1.0, look.track());
        p.rounded_rect(x + w * fl, ty, w * (fh - fl), 2.0, 1.0, look.accent());
        p.rounded_rect(x + w * fl - 3.0, ty - 2.0, 6.0, 6.0, 3.0, theme::KNOB);
        p.rounded_rect(x + w * fh - 3.0, ty - 2.0, 6.0, 6.0, 3.0, theme::KNOB);
        (lo, hi)
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn choice(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        name: &str,
        index: u8,
        options: &'static [&'static str],
        id: u16,
        active: bool,
    ) -> u8 {
        let look = self.look;
        let mut index = index;
        let open = self.dropdown == Some(id);
        let box_h = ENUM_H - 3.0;
        let hovered = active && self.hit(ui, x, y, w, box_h);
        p.rounded_rect(
            x,
            y,
            w,
            box_h,
            TAB_RADIUS,
            if hovered {
                look.nest_hi()
            } else {
                look.header()
            },
        );
        let picked = options.get(index as usize).copied().unwrap_or("");
        let vw = p.atlas.font.width_str(picked);
        p.text_plain(
            picked,
            x + w - 12.0 - vw,
            text_y(y, box_h),
            look.text(),
            look.shadow,
        );
        self.label(
            p,
            name,
            x + 4.0,
            text_y(y, box_h),
            w - 20.0 - vw,
            look.text_dim(),
        );
        theme::chevron(
            p,
            x + w - 9.0,
            (y + box_h * 0.5).floor() - 2.0,
            6.0,
            !open,
            look.text_dim(),
        );
        if hovered && ui.input.left_click {
            self.dropdown = if open { None } else { Some(id) };
        }
        if !open {
            return index;
        }

        let mut oy = y + box_h + 2.0;
        for (i, option) in options.iter().enumerate() {
            let on = i as u8 == index;
            let over = active && self.hit(ui, x, oy, w, OPTION_H);
            if on || over {
                p.rounded_rect(
                    x,
                    oy,
                    w,
                    OPTION_H,
                    2.0,
                    if on { look.accent() } else { look.row_hover() },
                );
            }
            p.text_plain(
                option,
                x + 5.0,
                text_y(oy, OPTION_H),
                if on { theme::TEXT_ON } else { look.text() },
                look.shadow,
            );
            if over && ui.input.left_click {
                index = i as u8;
                self.dropdown = None;
            }
            oy += OPTION_H;
        }
        index
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn grab(&mut self, ui: &Ui, id: u16, handle: u8, x: f32, w: f32, y: f32, active: bool) -> bool {
        if active && ui.input.left_click && self.hit(ui, x - 3.0, y + 8.0, w + 6.0, SLIDER_H - 8.0)
        {
            self.drag = Some(Drag::Slider { id, handle });
        }
        matches!(self.drag, Some(Drag::Slider { id: d, .. }) if d == id)
    }

    fn revert(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        changed: bool,
        active: bool,
    ) -> bool {
        if !changed {
            return false;
        }
        let hovered = active && self.hit(ui, x - 1.0, y - 1.0, RESET_W + 2.0, 10.0);
        theme::revert(
            p,
            x,
            y,
            if hovered {
                self.look.accent_hi()
            } else {
                0xFF00_0000 | self.look.text_dim()
            },
        );
        hovered && ui.input.left_click
    }

    fn track_t(&self, ui: &Ui, x: f32, w: f32) -> Option<f32> {
        ui.mouse().map(|m| ((m.x - x) / w).clamp(0.0, 1.0))
    }

    fn label(&self, p: &mut Painter, name: &str, x: f32, y: f32, w: f32, argb: u32) {
        let guard = p.push_clip(x, y - 1.0, w.max(0.0), TEXT_H + 2.0);
        p.text_plain(name, x, y, argb, self.look.shadow);
        p.pop_clip(guard);
    }

    fn number(&mut self, v: f32, decimals: u8) {
        self.scratch.clear();
        self.append(v, decimals);
    }

    fn append(&mut self, v: f32, decimals: u8) {
        use std::fmt::Write;
        let _ = match decimals {
            0 => write!(self.scratch, "{v:.0}"),
            1 => write!(self.scratch, "{v:.1}"),
            _ => write!(self.scratch, "{v:.2}"),
        };
    }
}

const CONFIG_W: f32 = 150.0;
const CONFIG: u8 = u8::MAX - 1;
const PICKER: u8 = u8::MAX - 2;
const EDITOR: u8 = u8::MAX - 3;

const EDITOR_W: f32 = 240.0;
const EDITOR_MIN_LINES: usize = 2;
const EDITOR_MAX_LINES: usize = 8;
const EDITOR_PAD: f32 = 7.0;

fn editor_box_h(lines: usize) -> f32 {
    3.0 + lines.clamp(EDITOR_MIN_LINES, EDITOR_MAX_LINES) as f32 * crate::text::LINE_HEIGHT
}

fn editor_view_of(lines: usize) -> f32 {
    3.0 + editor_box_h(lines) + 4.0 + TEXT_H + PAD_BOTTOM
}

const PICKER_W: f32 = 214.0;
const PRESET_H: f32 = 15.0;

const fn pi_window(pi: usize) -> u8 {
    pi as u8
}
