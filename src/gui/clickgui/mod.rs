mod picker;
pub(crate) mod theme;

use bevy::prelude::Vec2;

use crate::gui::keybinds::Bound;
use crate::modules::list::BitList;
use crate::modules::registry::{COUNT, Category, Id, Kind, Mode, ModuleDef, SettingDef, module};
use crate::modules::{Value, store};
use picker::{Picker, Row, contains_ci};

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
const CHOICE_H: f32 = ENUM_H - 3.0;
const BIND_H: f32 = 12.0;
const BLOCK_H: f32 = 12.0;
const CHANNEL_H: f32 = 22.0;
const LIST_H: f32 = 13.0;
const TEXTBOX_ROW_H: f32 = 13.0 + FIELD_H + 3.0;
const OPTION_H: f32 = 11.0;

fn options_h(n: usize) -> f32 {
    2.0 + n as f32 * OPTION_H
}
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Setting(u16),
    Channel(u8),
    Config(Cfg),
    BarProfile,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cfg {
    Profile,
    Scale,
    Opacity,
    Radius,
    RowHeight,
    ColumnWidth,
}

#[derive(Clone, Copy)]
enum CfgRow {
    Profile,
    Accent,
    Surface,
    Scale,
    Slider {
        ctl: Cfg,
        name: &'static str,
        min: f32,
        max: f32,
        decimals: u8,
        shown: f32,
        whole: bool,
        field: fn(&mut Look) -> &mut f32,
    },
    Toggle {
        name: &'static str,
        field: fn(&mut Look) -> &mut bool,
    },
}

const CFG: [CfgRow; 11] = [
    CfgRow::Profile,
    CfgRow::Accent,
    CfgRow::Surface,
    CfgRow::Scale,
    CfgRow::Slider {
        ctl: Cfg::Opacity,
        name: "Opacity",
        min: 30.0,
        max: 100.0,
        decimals: 0,
        shown: 100.0,
        whole: false,
        field: |l| &mut l.opacity,
    },
    CfgRow::Slider {
        ctl: Cfg::Radius,
        name: "Corner radius",
        min: 0.0,
        max: 8.0,
        decimals: 1,
        shown: 1.0,
        whole: false,
        field: |l| &mut l.radius,
    },
    CfgRow::Slider {
        ctl: Cfg::RowHeight,
        name: "Row height",
        min: 11.0,
        max: 18.0,
        decimals: 0,
        shown: 1.0,
        whole: true,
        field: |l| &mut l.row_h,
    },
    CfgRow::Slider {
        ctl: Cfg::ColumnWidth,
        name: "Column width",
        min: 86.0,
        max: 150.0,
        decimals: 0,
        shown: 1.0,
        whole: true,
        field: |l| &mut l.panel_w,
    },
    CfgRow::Toggle {
        name: "Dim background",
        field: |l| &mut l.scrim,
    },
    CfgRow::Toggle {
        name: "Open animation",
        field: |l| &mut l.animate,
    },
    CfgRow::Toggle {
        name: "Text shadow",
        field: |l| &mut l.shadow,
    },
];

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Window {
    Column(u8),
    Search,
    Config,
    Overlay,
}

#[derive(Clone, Copy)]
enum Drag {
    Window { window: Window, dx: f32, dy: f32 },
    Slider { id: Control, handle: u8 },
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    None,
    Bar,
    On(Window),
}

#[derive(Clone, Copy)]
struct Hit {
    focus: Focus,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Clone, Copy)]
enum Tail {
    Dots,
    Category,
}

struct Editor {
    slot: u16,
    title: String,
    max_len: usize,
    text: MultiLineTextBox,
}

enum Overlay {
    Picker(Picker),
    Editor(Editor),
}

struct FieldEdit {
    slot: u16,
    text: TextBox,
}

enum KeyOwner {
    None,
    Capture(Id),
    Field(FieldEdit),
}

impl KeyOwner {
    fn capture(&self) -> Option<Id> {
        match self {
            KeyOwner::Capture(id) => Some(*id),
            _ => None,
        }
    }

    fn field(&self) -> Option<&FieldEdit> {
        match self {
            KeyOwner::Field(f) => Some(f),
            _ => None,
        }
    }

    fn drop_field(&mut self) {
        if matches!(self, KeyOwner::Field(_)) {
            *self = KeyOwner::None;
        }
    }

    fn field_on(&mut self, slot: u16) -> Option<&mut FieldEdit> {
        match self {
            KeyOwner::Field(f) if f.slot == slot => Some(f),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct TextSpec {
    max_len: usize,
    expand: bool,
    hint: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
enum Keys {
    Capture(Id),
    Overlay,
    Field,
    Search,
    Nothing,
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

#[derive(Clone, Copy)]
pub(crate) struct Pane<'a> {
    pub input: &'a GuiInput,
    pub mouse: Option<Vec2>,
}

impl Pane<'_> {
    pub fn mouse(&self) -> Option<Vec2> {
        self.mouse
    }

    pub fn hovering(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        match self.mouse {
            Some(m) => m.x >= x && m.x < x + w && m.y >= y && m.y < y + h,
            None => false,
        }
    }

    pub fn copy_pointer(&self, dst: &mut GuiInput, click: bool) {
        dst.mouse = self.mouse;
        dst.left_click = self.input.left_click && click;
        dst.left_down = self.input.left_down;
        dst.left_release = self.input.left_release;
        dst.shift = self.input.shift;
        dst.double_click = self.input.double_click;
        dst.triple_click = self.input.triple_click;
    }
}

struct Ui<'a> {
    pane: Pane<'a>,
    vw: f32,
    vh: f32,
}

impl<'a> std::ops::Deref for Ui<'a> {
    type Target = Pane<'a>;
    fn deref(&self) -> &Pane<'a> {
        &self.pane
    }
}

impl std::ops::DerefMut for Ui<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.pane
    }
}

const COLUMNS: usize = Category::ALL.len();

pub struct ClickGuiState {
    look: Look,
    tab: Tab,
    panels: [Panel; COLUMNS],
    order: Vec<u8>,
    expanded: Option<Id>,
    dropdown: Option<Control>,
    profile_seen: Mode,
    drag: Option<Drag>,
    focus: Focus,
    hits: Vec<Hit>,
    band: (f32, f32),
    search: Panel,
    config: Panel,
    field: TextBox,
    mouse_input: GuiInput,
    matches: Vec<Id>,
    selected: usize,
    match_top: usize,
    scratch: String,
    owner: KeyOwner,
    overlay: Option<Overlay>,
    overlay_win: Panel,
    tip: Option<(&'static str, f32, f32)>,
    tip_lines: Vec<(usize, usize)>,
    placed: bool,
    applied: f32,
    scale_set: bool,
    opened_at: Option<f32>,
}

impl Default for ClickGuiState {
    fn default() -> Self {
        ClickGuiState {
            look: Look::default(),
            tab: Tab::Modules,
            panels: [Panel::at(0.0, 0.0); COLUMNS],
            order: (0..COLUMNS as u8).collect(),
            expanded: None,
            dropdown: None,
            profile_seen: store().profile(),
            drag: None,
            focus: Focus::None,
            hits: Vec::with_capacity(COLUMNS + 5),
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
            matches: Vec::with_capacity(COUNT),
            selected: 0,
            match_top: 0,
            scratch: String::with_capacity(16),
            owner: KeyOwner::None,
            overlay: None,
            overlay_win: Panel::at(0.0, 0.0),
            tip: None,
            tip_lines: Vec::with_capacity(4),
            placed: false,
            applied: 1.0,
            scale_set: false,
            opened_at: None,
        }
    }
}

pub fn draw(p: &mut Painter, st: &mut ClickGuiState, ctx: &ScreenCtx) {
    if !st.dragging_scale() {
        st.applied = st.look.scale.clamp(SCALE_MIN, SCALE_MAX);
    }
    let s = st.applied;
    let mut ui = Ui {
        pane: Pane {
            input: ctx.input,
            mouse: ctx.mouse().map(|m| m / s),
        },
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

fn changed(slot: usize, kind: Kind) -> bool {
    match (kind, store().value(slot), Value::default_of(kind)) {
        (Kind::Text { default, .. }, _, _) => !store().with_text(slot, |t| t == default),
        (_, Some(Value::Num(a)), Some(Value::Num(b))) => differs(a, b),
        (_, Some(Value::Range(a0, a1)), Some(Value::Range(b0, b1))) => {
            differs(a0, b0) || differs(a1, b1)
        }
        (_, Some(a), Some(b)) => a != b,
        _ => false,
    }
}

fn cat_tag(cat: &'static str) -> &'static str {
    &cat[..3.min(cat.len())]
}

fn cat_w(p: &Painter, cat: &str) -> f32 {
    p.atlas.font.width_str(cat)
}

impl ClickGuiState {
    fn contents(&mut self, p: &mut Painter, ui: &Ui) {
        self.tip = None;
        self.hits.clear();
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
                    let active =
                        self.focus == Focus::On(Window::Column(pi as u8)) && self.drag.is_none();
                    self.panel(p, ui, pi, active);
                }
                self.search_window(p, ui);
                self.overlay_window(p, ui);
            }
            Tab::Config => {
                self.config_window(p, ui);
                self.overlay_window(p, ui);
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
        match self.keys() {
            Keys::Capture(id) => {
                store().set_bind(id, Bound::Unbound);
                self.owner = KeyOwner::None;
            }
            Keys::Overlay => self.overlay = None,
            Keys::Field => self.owner = KeyOwner::None,
            Keys::Search | Keys::Nothing => return false,
        }
        true
    }

    fn keys(&self) -> Keys {
        if let Some(id) = self.owner.capture()
            && self.block_open(id)
        {
            Keys::Capture(id)
        } else if self.overlay.is_some() {
            Keys::Overlay
        } else if self.owner.field().is_some_and(|f| self.field_alive(f.slot)) {
            Keys::Field
        } else if self.tab == Tab::Modules {
            Keys::Search
        } else {
            Keys::Nothing
        }
    }

    fn block_open(&self, id: Id) -> bool {
        self.tab == Tab::Modules
            && self.expanded == Some(id)
            && store().allowed(id)
            && !self.panels[module(id).category as usize].collapsed
    }

    fn field_alive(&self, slot: u16) -> bool {
        self.expanded
            .is_some_and(|id| module(id).slots().contains(&(slot as usize)) && self.block_open(id))
    }

    pub fn opened(&mut self) {
        self.opened_at = None;
        self.field.clear();
        self.field.focused = true;
        self.matches.clear();
        self.selected = 0;
        self.match_top = 0;
    }

    fn pop(&mut self, now: f32) -> f32 {
        let at = *self.opened_at.get_or_insert(now);
        if !self.look.animate {
            return 1.0;
        }
        let t = ((now - at) / POP_SECS).clamp(0.0, 1.0);
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
        let overlay_w = self.overlay_w();
        let bottom = (ui.vh - HEADER_H).max(0.0).floor();
        for (panel, pw) in self.panels.iter_mut().map(|p| (p, w)).chain([
            (&mut self.search, w),
            (&mut self.config, CONFIG_W),
            (&mut self.overlay_win, overlay_w),
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
                self.focus = Focus::On(window);
                return;
            }
            Some(Drag::Slider { .. }) => return,
            None => {}
        }

        let found = self
            .hits
            .iter()
            .rev()
            .find(|h| self.live(h.focus) && ui.hovering(h.x, h.y, h.w, h.h))
            .map(|h| h.focus);
        self.focus = found.unwrap_or(Focus::None);
        let press = ui.input.left_click || ui.input.right_click;
        if press
            && let Focus::On(Window::Column(pi)) = self.focus
            && let Some(i) = self.order.iter().position(|&o| o == pi)
        {
            let v = self.order.remove(i);
            self.order.push(v);
        }
        if press
            && matches!(self.overlay, Some(Overlay::Editor(_)))
            && self.focus != Focus::On(Window::Overlay)
        {
            self.overlay = None;
        }
    }

    fn live(&self, focus: Focus) -> bool {
        match focus {
            Focus::None => false,
            Focus::Bar => true,
            Focus::On(Window::Column(_) | Window::Search) => self.tab == Tab::Modules,
            Focus::On(Window::Config) => self.tab == Tab::Config,
            Focus::On(Window::Overlay) => {
                self.overlay.is_some() && matches!(self.tab, Tab::Modules | Tab::Config)
            }
        }
    }

    fn record(&mut self, focus: Focus, x: f32, y: f32, w: f32, h: f32) {
        self.hits.push(Hit { focus, x, y, w, h });
    }

    fn window_mut(&mut self, window: Window) -> &mut Panel {
        match window {
            Window::Column(i) => &mut self.panels[i as usize],
            Window::Search => &mut self.search,
            Window::Config => &mut self.config,
            Window::Overlay => &mut self.overlay_win,
        }
    }

    fn keyboard(&mut self, ui: &Ui) {
        let stale = match &self.owner {
            KeyOwner::None => false,
            KeyOwner::Capture(id) => !self.block_open(*id),
            KeyOwner::Field(f) => !self.field_alive(f.slot),
        };
        if stale {
            self.owner = KeyOwner::None;
        }
        match self.keys() {
            Keys::Capture(id) => {
                if let Some(key) = ui.input.pressed_key {
                    if key != bevy::prelude::KeyCode::Escape {
                        store().set_bind(id, Bound::Key(key));
                        self.owner = KeyOwner::None;
                    }
                } else if let Some(button) = ui.input.pressed_mouse {
                    store().set_bind(id, Bound::Mouse(button));
                    self.owner = KeyOwner::None;
                }
            }
            Keys::Overlay | Keys::Nothing => {}
            Keys::Field => {
                let KeyOwner::Field(f) = &mut self.owner else {
                    return;
                };
                f.text.focused = true;
                if f.text.handle_input(ui.input) {
                    store().set_text_at(f.slot as usize, &f.text.text);
                }
            }
            Keys::Search => self.search_keys(ui),
        }
    }

    fn search_keys(&mut self, ui: &Ui) {
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
            && let Some(&id) = self.matches.get(self.selected)
        {
            store().toggle(id);
        }
    }

    fn refresh_matches(&mut self) {
        self.matches.clear();
        self.selected = 0;
        self.match_top = 0;
        if self.field.text.is_empty() {
            return;
        }
        for m in Category::ALL.iter().flat_map(|c| c.modules()) {
            if store().allowed(m.id) && contains_ci(m.name, &self.field.text) {
                self.matches.push(m.id);
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
        let cat = Category::ALL[pi];
        let rows = cat.modules().filter(|m| store().allowed(m.id)).count();
        let mut h = rows as f32 * self.look.row_h + PAD_BOTTOM;
        if let Some(id) = self.expanded
            && module(id).category == cat
            && store().allowed(id)
        {
            h += self.settings_height(id);
        }
        h
    }

    fn fit(&self, content_h: f32, y: f32, vh: f32) -> f32 {
        content_h.min((vh - y - HEADER_H - GAP).max(3.0 * self.look.row_h))
    }

    fn overlay_w(&self) -> f32 {
        match &self.overlay {
            Some(Overlay::Picker(k)) => k.list.width,
            Some(Overlay::Editor(_)) => EDITOR_W,
            None => crate::modules::list::widest(),
        }
    }

    fn open_editor(
        &mut self,
        slot: u16,
        id: Id,
        name: &'static str,
        max_len: usize,
        font: &crate::text::Font,
        ui: &Ui,
    ) {
        let mut text = MultiLineTextBox::new(max_len, EDITOR_MAX_LINES);
        text.set_text(&store().text_at(slot as usize));
        text.inner.focused = true;
        text.inner.browser_keyboard = true;
        let lines = text.line_count(font, EDITOR_W - 2.0 * EDITOR_PAD);
        let mut title = String::with_capacity(32);
        title.push_str(module(id).name);
        title.push_str(" / ");
        title.push_str(name);
        self.overlay = Some(Overlay::Editor(Editor {
            slot,
            title,
            max_len,
            text,
        }));
        self.owner.drop_field();
        let top = GAP + TOP_H + GAP;
        let h = HEADER_H + editor_view_of(lines);
        self.overlay_win = Panel::at(
            ((ui.vw - EDITOR_W) * 0.5).floor(),
            (top + ((ui.vh - GAP - top - h) / 3.0).max(0.0)).floor(),
        );
    }

    fn open_picker(&mut self, list: &'static BitList, ui: &Ui) {
        self.overlay = Some(Overlay::Picker(Picker::new(list)));
        self.overlay_win = Panel::at(((ui.vw - list.width) * 0.5).floor(), GAP + TOP_H + GAP);
    }

    fn settings_height(&self, id: Id) -> f32 {
        let m = module(id);
        6.0 + BIND_H
            + m.slots()
                .zip(m.settings())
                .map(|(slot, d)| self.setting_height(slot, d))
                .sum::<f32>()
    }

    fn setting_height(&self, slot: usize, d: &SettingDef) -> f32 {
        match d.kind {
            Kind::Toggle { .. } => TOGGLE_H,
            Kind::Slider { .. } | Kind::Range { .. } => SLIDER_H,
            Kind::Enum { options, .. } => {
                if self.dropdown == Some(Control::Setting(slot as u16)) {
                    ENUM_H + options_h(options.len())
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
                self.set_tab(tab);
            }
            tx += tw;
        }
    }

    fn profile_control(&mut self, p: &mut Painter, ui: &Ui) {
        let active = self.focus == Focus::Bar && self.drag.is_none();
        let x = profile_x(ui.vw);
        self.band = (0.0, ui.vh);
        if !active && ui.input.left_click {
            self.dropdown = self.dropdown.filter(|d| *d != Control::BarProfile);
        }
        let hovered = active && self.hit(ui, x, GAP, PROFILE_W, CHOICE_H);
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
            Control::BarProfile,
            active,
        );
        if picked != now as u8 {
            store().set_profile(Mode::ALL[picked as usize]);
        }
        if hovered
            && self.dropdown != Some(Control::BarProfile)
            && let Some(cursor) = ui.mouse()
        {
            self.tip = Some((PROFILE_TIP, cursor.x, cursor.y));
        }
        self.record(Focus::Bar, 0.0, 0.0, ui.vw, GAP + TOP_H);
        if self.dropdown == Some(Control::BarProfile) {
            let h = CHOICE_H + options_h(Mode::ALL.len());
            self.record(Focus::Bar, x, GAP, PROFILE_W, h);
        }
    }

    fn profile_key(&self, ui: &Ui) {
        if matches!(self.keys(), Keys::Capture(_)) {
            return;
        }
        let step = ui.input.profile_up as i8 - ui.input.profile_down as i8;
        if step != 0 {
            store().set_profile(store().profile().step(step));
        }
    }

    fn tab_key(&mut self, ui: &Ui) {
        if !ui.input.tab || matches!(self.keys(), Keys::Capture(_)) {
            return;
        }
        let here = TABS.iter().position(|(t, _)| *t == self.tab).unwrap_or(0);
        let step = if ui.input.shift { TABS.len() - 1 } else { 1 };
        self.set_tab(TABS[(here + step) % TABS.len()].0);
    }

    fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
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
        self.order.extend(0..COLUMNS as u8);
        if matches!(self.overlay, Some(Overlay::Editor(_))) {
            self.overlay = None;
        }
        self.expanded = None;
        self.dropdown = None;
        self.drag = None;
        self.placed = false;
    }

    fn panel(&mut self, p: &mut Painter, ui: &Ui, pi: usize, active: bool) {
        let look = self.look;
        let cat = Category::ALL[pi];
        let Panel {
            x, y, collapsed, ..
        } = self.panels[pi];
        let w = look.panel_w;
        let content_h = self.content_height(pi);
        let view_h = self.fit(content_h, y, ui.vh);
        let window = Window::Column(pi as u8);
        let scroll = self.scroll(window, ui, active, content_h, view_h);

        p.rounded_rect(x, y, w, HEADER_H + view_h, look.radius, look.panel());
        self.record(Focus::On(window), x, y, w, HEADER_H + view_h);
        self.title_bar(p, ui, window, x, y, w, cat.name(), collapsed, active);
        if collapsed {
            return;
        }

        let guard = p.push_clip(x, y + HEADER_H, w, view_h);
        self.band = (y + HEADER_H, y + HEADER_H + view_h);
        let outlined = self.outlined();
        let mut cy = y + HEADER_H - scroll;
        for m in cat.modules() {
            if !store().allowed(m.id) {
                continue;
            }
            let hovered = self.module_row(
                p,
                ui,
                x,
                cy,
                w,
                m,
                outlined == Some(m.id),
                Tail::Dots,
                active,
            );
            if hovered {
                if ui.input.left_click {
                    store().toggle(m.id);
                }
                if ui.input.right_click {
                    self.expanded = (self.expanded != Some(m.id)).then_some(m.id);
                    self.dropdown = None;
                    self.owner.drop_field();
                }
            }
            cy += look.row_h;

            if self.expanded == Some(m.id) {
                cy = self.settings(p, ui, x, cy, m.id, active);
            }
        }
        p.pop_clip(guard);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn module_row(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        m: &'static ModuleDef,
        outlined: bool,
        tail: Tail,
        active: bool,
    ) -> bool {
        let look = self.look;
        let on = store().armed(m.id);
        let hovered = active && self.hit(ui, x, y, w, look.row_h);
        let bg = match (on, hovered) {
            (true, true) => look.accent_hi(),
            (true, false) => look.accent(),
            (false, true) => look.row_hover(),
            (false, false) => theme::ROW,
        };
        if bg >> 24 != 0 {
            p.fill(x, y, w, look.row_h, bg);
        }
        if outlined {
            p.outline(x, y, w, look.row_h, look.accent_hi());
        }
        let ty = text_y(y, look.row_h);
        let name = if on { theme::TEXT_ON } else { look.text() };
        let dim = if on { theme::TEXT_ON } else { look.text_dim() };
        match tail {
            Tail::Dots => {
                self.label(p, m.name, x + 8.0, ty, w - 21.0, name);
                theme::dots(p, x + w - 10.0, (y + look.row_h * 0.5).floor() - 2.0, dim);
            }
            Tail::Category => {
                let cat = cat_tag(m.category.name());
                let cw = cat_w(p, cat);
                self.label(p, m.name, x + 8.0, ty, w - 22.0 - cw, name);
                p.text_plain(cat, x + w - 8.0 - cw, ty, dim, look.shadow);
            }
        }
        if hovered && let Some(cursor) = ui.mouse() {
            self.tip = Some((m.desc, cursor.x, cursor.y));
        }
        hovered
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one window chrome, flat arguments"
    )]
    fn title_bar(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        window: Window,
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

    fn scroll(
        &mut self,
        window: Window,
        ui: &Ui,
        active: bool,
        content_h: f32,
        view_h: f32,
    ) -> f32 {
        let limit = (content_h - view_h).max(0.0).floor();
        let step = 3.0 * self.look.row_h;
        let panel = self.window_mut(window);
        if active && ui.input.scroll != 0.0 && limit > 0.0 {
            panel.scroll -= ui.input.scroll * step;
        }
        panel.scroll = panel.scroll.clamp(0.0, limit);
        panel.scroll
    }

    fn outlined(&self) -> Option<Id> {
        self.matches.get(self.selected).copied()
    }

    fn dragging_scale(&self) -> bool {
        matches!(
            self.drag,
            Some(Drag::Slider {
                id: Control::Config(Cfg::Scale),
                ..
            })
        )
    }

    fn settings(&mut self, p: &mut Painter, ui: &Ui, x: f32, y: f32, id: Id, active: bool) -> f32 {
        let look = self.look;
        let h = self.settings_height(id);
        p.fill(x, y, look.panel_w, h, look.nest());
        p.fill(x, y, 1.5, h, look.accent());

        let ix = x + INDENT;
        let rx = x + look.panel_w - RESET_EDGE - RESET_W;
        let iw = rx - ix - 2.0;
        let mut sy = y + 3.0;
        self.bind_row(p, ui, ix, sy, rx - ix - 2.0, id, active);
        sy += BIND_H;
        let m = module(id);
        for (slot, d) in m.slots().zip(m.settings()) {
            let ctl = Control::Setting(slot as u16);
            let row_h = self.setting_height(slot, d);
            if active && !d.tip.is_empty() && self.hit(ui, x, sy, look.panel_w, row_h) {
                if let Some(cursor) = ui.mouse() {
                    self.tip = Some((d.tip, cursor.x, cursor.y));
                }
            }
            let label_y = match d.kind {
                Kind::Toggle { .. } => text_y(sy, TOGGLE_H),
                Kind::Enum { .. } => text_y(sy, ENUM_H - 3.0),
                _ => sy + 1.0,
            };
            if self.revert(p, ui, rx, label_y, changed(slot, d.kind), active) {
                match d.kind {
                    Kind::Text { default, .. } => self.set_text(slot as u16, default),
                    kind => {
                        if let Some(v) = Value::default_of(kind) {
                            store().set_value(slot, v);
                        }
                    }
                }
                if self.dropdown == Some(ctl) {
                    self.dropdown = None;
                }
            }
            match (d.kind, store().value(slot)) {
                (Kind::Toggle { .. }, Some(Value::Bool(on))) => {
                    let v = self.toggle(p, ui, ix, sy, iw, d.name, on, active);
                    if v != on {
                        store().set_value(slot, Value::Bool(v));
                    }
                }
                (
                    Kind::Slider {
                        min, max, decimals, ..
                    },
                    Some(Value::Num(v)),
                ) => {
                    let was = v;
                    let v = self.slider(
                        p, ui, ix, sy, iw, d.name, v, min, max, decimals, ctl, active,
                    );
                    if v != was {
                        store().set_value(slot, Value::Num(v));
                    }
                }
                (
                    Kind::Range {
                        min, max, decimals, ..
                    },
                    Some(Value::Range(lo, hi)),
                ) => {
                    let was = (lo, hi);
                    let (lo, hi) = self.range(
                        p, ui, ix, sy, iw, d.name, lo, hi, min, max, decimals, ctl, active,
                    );
                    if (lo, hi) != was {
                        store().set_value(slot, Value::Range(lo, hi));
                    }
                }
                (Kind::Enum { options, .. }, Some(Value::Choice(index))) => {
                    let v = self.choice(p, ui, ix, sy, iw, d.name, index, options, ctl, active);
                    if v != index {
                        store().set_value(slot, Value::Choice(v));
                    }
                }
                (Kind::List { list }, _) => {
                    self.list_button(p, ui, ix, sy, iw, d.name, list, active);
                }
                (
                    Kind::Text {
                        max_len,
                        expand,
                        hint,
                        ..
                    },
                    _,
                ) => {
                    self.text_field(
                        p,
                        ui,
                        ix,
                        sy,
                        iw,
                        d.name,
                        slot as u16,
                        id,
                        TextSpec {
                            max_len,
                            expand,
                            hint,
                        },
                        active,
                    );
                }
                _ => {}
            }
            sy += self.setting_height(slot, d);
        }
        y + h
    }

    fn search_window(&mut self, p: &mut Painter, ui: &Ui) {
        let look = self.look;
        let active = self.focus == Focus::On(Window::Search) && self.drag.is_none();
        let Panel {
            x, y, collapsed, ..
        } = self.search;
        let w = look.panel_w;
        let h = if collapsed { HEADER_H } else { self.search_h() };
        p.rounded_rect(x, y, w, h, look.radius, look.solid());
        self.record(Focus::On(Window::Search), x, y, w, h);
        self.title_bar(p, ui, Window::Search, x, y, w, "Search", collapsed, active);
        if collapsed {
            return;
        }

        let fy = y + HEADER_H + 3.0;
        self.sync_pointer(ui, active);
        search_field(
            p,
            look,
            &self.mouse_input,
            &mut self.field,
            x + 5.0,
            fy,
            w - 10.0,
            "Type to search",
        );

        let mut ry = fy + FIELD_H + 3.0;
        self.band = (ry, ry + SEARCH_ROWS as f32 * look.row_h);
        let shown = self.matches.len().min(SEARCH_ROWS);
        for i in 0..shown {
            let Some(&id) = self.matches.get(self.match_top + i) else {
                break;
            };
            let picked = self.match_top + i == self.selected;
            let hovered =
                self.module_row(p, ui, x, ry, w, module(id), picked, Tail::Category, active);
            if hovered {
                if ui.input.left_click {
                    store().toggle(id);
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
            self.label(p, line, x + 8.0, ry + 2.0, w - 16.0, look.text_dim());
        }
    }

    fn sync_pointer(&mut self, ui: &Ui, active: bool) {
        ui.copy_pointer(&mut self.mouse_input, active);
    }

    fn set_text(&mut self, slot: u16, text: &str) {
        store().set_text_at(slot as usize, text);
        if let Some(f) = self.owner.field_on(slot) {
            f.text.set_text(text);
        }
        if let Some(Overlay::Editor(e)) = self.overlay.as_mut()
            && e.slot == slot
        {
            e.text.set_text(text);
        }
    }

    fn overlay_window(&mut self, p: &mut Painter, ui: &Ui) {
        let Some(overlay) = self.overlay.take() else {
            return;
        };
        let m = ui.vw.max(ui.vh);
        p.fill(-m, -m, ui.vw + 2.0 * m, ui.vh + 2.0 * m, OVERLAY_SCRIM);
        self.overlay = match overlay {
            Overlay::Picker(k) => Some(Overlay::Picker(self.picker_window(p, ui, k))),
            Overlay::Editor(e) => self.editor_window(p, ui, e).map(Overlay::Editor),
        };
    }

    fn picker_window(&mut self, p: &mut Painter, ui: &Ui, mut k: Picker) -> Picker {
        let list = k.list;
        let look = self.look;
        let active = self.focus == Focus::On(Window::Overlay) && self.drag.is_none();
        let Panel {
            x, y, collapsed, ..
        } = self.overlay_win;

        k.query.focused = true;
        let dirty = k.query.handle_input(ui.input);
        k.rebuild(dirty);
        let pw = list.width;
        if collapsed {
            p.rounded_rect(x, y, pw, HEADER_H, look.radius, look.raised());
            self.record(Focus::On(Window::Overlay), x, y, pw, HEADER_H);
            self.title_bar(p, ui, Window::Overlay, x, y, pw, list.title, true, active);
            return k;
        }
        let head = HEADER_H + picker_head(list);
        let rows = k.rows.len();
        let content =
            rows as f32 * BLOCK_H + k.channels.map_or(0.0, |_| 3.0 * CHANNEL_H) + PAD_BOTTOM;
        let view = self.fit(content, y + head - HEADER_H, ui.vh);
        let scroll = self.scroll(Window::Overlay, ui, active, content, view);

        p.rounded_rect(x, y, pw, head + view, look.radius, look.raised());
        self.record(Focus::On(Window::Overlay), x, y, pw, head + view);
        self.title_bar(p, ui, Window::Overlay, x, y, pw, list.title, false, active);

        let fy = y + HEADER_H + 3.0;
        self.sync_pointer(ui, active);
        search_field(
            p,
            look,
            &self.mouse_input,
            &mut k.query,
            x + 5.0,
            fy,
            pw - 10.0,
            "Search",
        );

        if !list.presets.is_empty() {
            let py = fy + FIELD_H + 3.0;
            let step = (pw - 10.0) / list.presets.len() as f32;
            for (i, &preset) in list.presets.iter().enumerate() {
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
                let label = preset.label();
                let lw = p.atlas.font.width_str(label);
                p.text_plain(
                    label,
                    (bx + (bw - lw) * 0.5).floor(),
                    text_y(py, PRESET_H - 4.0),
                    if over { theme::TEXT_ON } else { look.text() },
                    look.shadow,
                );
                if over && ui.input.left_click {
                    list.apply_preset(preset);
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
            let Some(&row) = k.rows.get(r) else {
                break;
            };
            match row {
                Row::Header(g) => self.picker_header(p, ui, &mut k, x, ry, g as usize, active),
                Row::Item(i) => {
                    ry = self.picker_item(p, ui, &mut k, x, ry, i as usize, active);
                    continue;
                }
            }
            ry += BLOCK_H;
        }
        p.pop_clip(guard);
        k
    }

    fn editor_window(&mut self, p: &mut Painter, ui: &Ui, mut e: Editor) -> Option<Editor> {
        let look = self.look;
        let active = self.focus == Focus::On(Window::Overlay) && self.drag.is_none();
        let Panel {
            x, y, collapsed, ..
        } = self.overlay_win;
        let inner_w = EDITOR_W - 2.0 * EDITOR_PAD;

        if ui.input.enter {
            return None;
        }

        e.text.inner.focused = true;
        if e.text.handle_input(ui.input, &p.atlas.font, inner_w) {
            store().set_text_at(e.slot as usize, e.text.text());
        }
        let lines = e.text.line_count(&p.atlas.font, inner_w);
        let view = if collapsed {
            0.0
        } else {
            editor_view_of(lines)
        };

        p.rounded_rect(x, y, EDITOR_W, HEADER_H + view, look.radius, look.raised());
        self.record(Focus::On(Window::Overlay), x, y, EDITOR_W, HEADER_H + view);
        self.title_bar(
            p,
            ui,
            Window::Overlay,
            x,
            y,
            EDITOR_W,
            &e.title,
            collapsed,
            active,
        );
        if collapsed {
            return Some(e);
        }

        let box_h = editor_box_h(lines);
        let (bx, by, bw) = (x + 5.0, y + HEADER_H + 3.0, EDITOR_W - 10.0);
        p.rounded_rect(bx, by, bw, box_h, 3.0, look.nest());
        p.fill(bx + 2.0, by + box_h - 1.0, bw - 4.0, 1.0, look.accent());

        self.band = (y + HEADER_H, y + HEADER_H + view);
        self.sync_pointer(ui, active);
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
        Some(e)
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn picker_header(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        k: &mut Picker,
        x: f32,
        y: f32,
        g: usize,
        active: bool,
    ) {
        let look = self.look;
        let list = k.list;
        let open = k.is_open(g);
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
        pill(p, look, px, py, all);
        if over && ui.input.left_click {
            list.set_group(g, !all);
            return;
        }
        if hovered && ui.input.left_click {
            k.toggle_group(g);
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn picker_item(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        k: &mut Picker,
        x: f32,
        y: f32,
        i: usize,
        active: bool,
    ) -> f32 {
        let list = k.list;
        let on = list.enabled(i);
        let chip = if list.colored { 9.0 } else { 0.0 };
        let inset = 10.0;
        let pw = list.width;
        let w = pw - inset - 6.0 - chip - if list.colored { 4.0 } else { 0.0 };

        if self.toggle(p, ui, x + inset, y, w, list.label(i), on, active) != on {
            list.set_enabled(i, !on);
        }

        let mut ry = y + BLOCK_H;
        if list.colored {
            let rgb = list.color(i);
            let open = k.channels == Some(i as u32);
            let cx = x + pw - chip - 6.0;
            let over = active && self.hit(ui, cx - 1.0, y + 1.0, chip + 2.0, chip);
            if open || over {
                p.rounded_rect(cx - 1.5, y + 0.5, chip + 3.0, chip + 3.0, 3.0, theme::KNOB);
            }
            p.rounded_rect(cx, y + 2.0, chip, chip - 2.0, 2.0, 0xFF00_0000 | rgb);
            if over && ui.input.left_click {
                k.channels = (!open).then_some(i as u32);
            }
            if open {
                let mut out = 0u32;
                for (c, (name, shift)) in [("R", 16), ("G", 8), ("B", 0)].into_iter().enumerate() {
                    let v = ((rgb >> shift) & 0xFF) as f32;
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
                            Control::Channel(c as u8),
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

    fn cfg_row_h(&self, row: CfgRow) -> f32 {
        match row {
            CfgRow::Profile if self.dropdown == Some(Control::Config(Cfg::Profile)) => {
                ENUM_H + options_h(Mode::ALL.len())
            }
            CfgRow::Profile => ENUM_H,
            CfgRow::Accent | CfgRow::Surface => SWATCH_H,
            CfgRow::Scale | CfgRow::Slider { .. } => SLIDER_H,
            CfgRow::Toggle { .. } => TOGGLE_H,
        }
    }

    fn config_content_h(&self) -> f32 {
        6.0 + CFG.iter().map(|&row| self.cfg_row_h(row)).sum::<f32>()
    }

    fn config_window(&mut self, p: &mut Painter, ui: &Ui) {
        let look = self.look;
        let active = self.focus == Focus::On(Window::Config) && self.drag.is_none();
        let Panel {
            x, y, collapsed, ..
        } = self.config;
        let content_h = if collapsed {
            0.0
        } else {
            self.config_content_h()
        };
        let view_h = self.fit(content_h, y, ui.vh);
        let scroll = self.scroll(Window::Config, ui, active, content_h, view_h);

        p.rounded_rect(x, y, CONFIG_W, HEADER_H + view_h, look.radius, look.panel());
        self.record(Focus::On(Window::Config), x, y, CONFIG_W, HEADER_H + view_h);
        self.title_bar(
            p,
            ui,
            Window::Config,
            x,
            y,
            CONFIG_W,
            "Config",
            collapsed,
            active,
        );
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

        for row in CFG {
            match row {
                CfgRow::Profile => {
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
                        Control::Config(Cfg::Profile),
                        active,
                    );
                    if picked != profile as u8 {
                        store().set_profile(Mode::ALL[picked as usize]);
                    }
                }
                CfgRow::Accent => {
                    self.look.accent =
                        self.swatches(p, ui, ix, cy, iw, "Accent", look.accent, true, active);
                    if self.revert(p, ui, rx, cy + 1.0, look.accent != def.accent, active) {
                        self.look.accent = def.accent;
                    }
                }
                CfgRow::Surface => {
                    self.look.surface =
                        self.swatches(p, ui, ix, cy, iw, "Surface", look.surface, false, active);
                    if self.revert(p, ui, rx, cy + 1.0, look.surface != def.surface, active) {
                        self.look.surface = def.surface;
                    }
                }
                CfgRow::Scale => self.scale_row(p, ui, ix, cy, iw, rx, active),
                CfgRow::Slider {
                    ctl,
                    name,
                    min,
                    max,
                    decimals,
                    shown,
                    whole,
                    field,
                } => {
                    let now = *field(&mut { look });
                    let v = self.slider(
                        p,
                        ui,
                        ix,
                        cy,
                        iw,
                        name,
                        now * shown,
                        min,
                        max,
                        decimals,
                        Control::Config(ctl),
                        active,
                    );
                    if v != now * shown {
                        *field(&mut self.look) = if whole { v.round() } else { v } / shown;
                    }
                    let was = *field(&mut { def });
                    if self.revert(p, ui, rx, cy + 1.0, differs(now, was), active) {
                        *field(&mut self.look) = was;
                    }
                }
                CfgRow::Toggle { name, field } => {
                    let on = *field(&mut { look });
                    if self.toggle(p, ui, ix, cy, iw, name, on, active) != on {
                        *field(&mut self.look) = !on;
                    }
                    let was = *field(&mut { def });
                    if self.revert(p, ui, rx, text_y(cy, TOGGLE_H), on != was, active) {
                        *field(&mut self.look) = was;
                    }
                }
            }
            cy += self.cfg_row_h(row);
        }

        p.pop_clip(guard);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn scale_row(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        ix: f32,
        cy: f32,
        iw: f32,
        rx: f32,
        active: bool,
    ) {
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
            Control::Config(Cfg::Scale),
            active,
        );
        self.look.scale = (s / 100.0).clamp(SCALE_MIN, SCALE_MAX);
        self.scale_set |= self.dragging_scale();
        if self.revert(p, ui, rx, cy + 1.0, self.scale_set, active) {
            self.look.scale = Look::default().scale;
            self.scale_set = false;
            self.placed = false;
        }
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
        pill(p, look, x + w - pw, y + (TOGGLE_H - ph) * 0.5, on);
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
        list: &'static BitList,
        active: bool,
    ) {
        let look = self.look;
        let open = matches!(&self.overlay, Some(Overlay::Picker(k)) if std::ptr::eq(k.list, list));
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
                self.overlay = None;
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
        slot: u16,
        id: Id,
        spec: TextSpec,
        active: bool,
    ) {
        let TextSpec {
            max_len,
            expand,
            hint,
        } = spec;
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
                self.open_editor(slot, id, name, max_len, &p.atlas.font, ui);
            }
        }

        let fy = y + 13.0;
        p.rounded_rect(x, fy, w, FIELD_H, 3.0, look.nest());

        let hovered = active && self.hit(ui, x, fy, w, FIELD_H);
        let mine = self.owner.field_on(slot).is_some();
        if active && ui.input.left_click {
            if hovered && !mine {
                let mut text = TextBox::new(max_len, "");
                text.browser_keyboard = false;
                text.focused = true;
                store().with_text(slot as usize, |t| text.set_text(t));
                self.owner = KeyOwner::Field(FieldEdit { slot, text });
            } else if !hovered && mine {
                self.owner = KeyOwner::None;
            }
        }

        let guard = p.push_clip(x + 3.0, fy, w - 6.0, FIELD_H);
        if self.owner.field_on(slot).is_some() {
            self.sync_pointer(ui, active);
        }
        if let Some(f) = self.owner.field_on(slot) {
            p.fill(x + 2.0, fy + FIELD_H - 1.0, w - 4.0, 1.0, look.accent());
            f.text.color = look.text_title();
            f.text.handle_mouse(
                &self.mouse_input,
                &p.atlas.font,
                x + 4.0,
                fy,
                w - 8.0,
                FIELD_H,
            );
            let frame = p.frame;
            f.text.draw(p, x + 4.0, fy, w - 8.0, FIELD_H, frame);
        } else {
            let shadow = look.shadow;
            store().with_text(slot as usize, |t| {
                if t.is_empty() {
                    p.text_plain(hint, x + 9.0, text_y(fy, FIELD_H), look.text_dim(), shadow);
                } else {
                    p.text_plain(t, x + 4.0, text_y(fy, FIELD_H), look.text_title(), shadow);
                }
            });
        }
        p.pop_clip(guard);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn bind_row(&mut self, p: &mut Painter, ui: &Ui, x: f32, y: f32, w: f32, id: Id, active: bool) {
        let look = self.look;
        let capturing = self.owner.capture() == Some(id);
        let bound = store().bind(id);
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
            self.owner = if capturing {
                KeyOwner::None
            } else {
                KeyOwner::Capture(id)
            };
        }

        let rx = x + w + 2.0;
        if self.revert(
            p,
            ui,
            rx,
            text_y(y, BIND_H),
            bound != Bound::Unbound,
            active,
        ) {
            store().set_bind(id, Bound::Unbound);
            if self.owner.capture().is_some() {
                self.owner = KeyOwner::None;
            }
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
        id: Control,
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
        id: Control,
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
        id: Control,
        active: bool,
    ) -> u8 {
        let look = self.look;
        let mut index = index;
        let open = self.dropdown == Some(id);
        let box_h = CHOICE_H;
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

        let mut oy = y + box_h + options_h(0);
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
    fn grab(
        &mut self,
        ui: &Ui,
        id: Control,
        handle: u8,
        x: f32,
        w: f32,
        y: f32,
        active: bool,
    ) -> bool {
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
const EDITOR_W: f32 = 240.0;
const EDITOR_MIN_LINES: usize = 2;
const EDITOR_MAX_LINES: usize = 8;
const EDITOR_PAD: f32 = 7.0;

#[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
fn search_field(
    p: &mut Painter,
    look: Look,
    pointer: &GuiInput,
    field: &mut TextBox,
    x: f32,
    y: f32,
    w: f32,
    hint: &str,
) {
    p.rounded_rect(x, y, w, FIELD_H, 3.0, look.nest());
    p.fill(x + 2.0, y + FIELD_H - 1.0, w - 4.0, 1.0, look.accent());
    field.color = look.text_title();
    let guard = p.push_clip(x + 3.0, y, w - 6.0, FIELD_H);
    field.handle_mouse(pointer, &p.atlas.font, x + 4.0, y, w - 8.0, FIELD_H);
    if field.text.is_empty() {
        p.text_plain(
            hint,
            x + 9.0,
            text_y(y, FIELD_H),
            look.text_dim(),
            look.shadow,
        );
    }
    let frame = p.frame;
    field.draw(p, x + 4.0, y, w - 8.0, FIELD_H, frame);
    p.pop_clip(guard);
}

fn pill(p: &mut Painter, look: Look, x: f32, y: f32, on: bool) {
    let (w, h) = (16.0, 8.0);
    p.rounded_rect(
        x,
        y,
        w,
        h,
        h * 0.5,
        if on { look.accent() } else { look.track() },
    );
    let knob = h - 1.0;
    let kx = if on { x + w - knob - 0.5 } else { x + 0.5 };
    p.rounded_rect(kx, y + 0.5, knob, knob, knob * 0.5, theme::KNOB);
}

fn editor_box_h(lines: usize) -> f32 {
    3.0 + lines.clamp(EDITOR_MIN_LINES, EDITOR_MAX_LINES) as f32 * crate::text::LINE_HEIGHT
}

fn editor_view_of(lines: usize) -> f32 {
    3.0 + editor_box_h(lines) + 4.0 + TEXT_H + PAD_BOTTOM
}

const PRESET_H: f32 = 15.0;

fn picker_head(list: &BitList) -> f32 {
    FIELD_H
        + 6.0
        + if list.presets.is_empty() {
            0.0
        } else {
            PRESET_H
        }
}
