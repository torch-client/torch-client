use bevy::prelude::Vec2;

use super::{
    ClickGuiState, Control, Drag, FIELD_H, Focus, GAP, HINT_H, Overlay, PILL_H, PILL_W, Panel,
    SLIDER_H, STRIP_PAD, TAB_PAD, TAB_RADIUS, TABS, Ui, Window, inside, pill, search_field, text_y,
    theme,
};
use crate::gui::painter::{ClipGuard, Painter};
use crate::modules::registry::{Category, ModuleIx, module};
use crate::modules::store;

const PAD: f32 = 4.0;
const RAIL_W: f32 = 74.0;
const RAIL_ROW_H: f32 = 24.0;
const CARD_MIN_W: f32 = 128.0;
const CARD_H: f32 = 44.0;
const BUTTON_H: f32 = 20.0;
const DESC_LINES: usize = 2;
const DESC_LINE: f32 = 9.0;
const HEAD_H: f32 = 24.0;
const MAX_W: f32 = 460.0;
pub(super) const MIN_W: f32 = RAIL_W + CARD_MIN_W + 4.0 * PAD;
const TAB_BAR_H: f32 = 24.0;
const TAP_SLOP: f32 = 4.0;
const HOLD_SECS: f32 = 0.45;
const SHEET_SECS: f32 = 0.18;

pub(super) struct State {
    cat: u8,
    grid: Panel,
    sheet: Panel,
    touch: Touch,
    sheet_at: Option<f32>,
}

impl Default for State {
    fn default() -> Self {
        State {
            cat: 0,
            grid: Panel::at(0.0, 0.0),
            sheet: Panel::at(0.0, 0.0),
            touch: Touch::default(),
            sheet_at: None,
        }
    }
}

impl State {
    pub(super) fn view(&mut self, window: Window) -> &mut Panel {
        match window {
            Window::Sheet => &mut self.sheet,
            _ => &mut self.grid,
        }
    }

    pub(super) fn sheet_opened(&mut self) {
        self.sheet_at = None;
    }

    fn slide(&mut self, now: f32, h: f32, animate: bool) -> f32 {
        let at = *self.sheet_at.get_or_insert(now);
        if !animate {
            return 0.0;
        }
        let t = ((now - at) / SHEET_SECS).clamp(0.0, 1.0);
        ((1.0 - t).powi(3) * h).floor()
    }
}

#[derive(Clone, Copy)]
struct Touch {
    at: Option<Vec2>,
    focus: Focus,
    since: f32,
    moved: bool,
    swiping: bool,
    base_y: f32,
    base_scroll: f32,
    held: bool,
    dead: bool,
}

impl Default for Touch {
    fn default() -> Self {
        Touch {
            at: None,
            focus: Focus::None,
            since: 0.0,
            moved: false,
            swiping: false,
            base_y: 0.0,
            base_scroll: 0.0,
            held: false,
            dead: false,
        }
    }
}

pub(super) fn tab_bar_rect(ui: &Ui) -> (f32, f32, f32, f32) {
    let w = (ui.vw - 2.0 * GAP).min(MAX_W).floor();
    let x = ((ui.vw - w) * 0.5).floor();
    (x, (ui.vh - GAP - TAB_BAR_H).floor(), w, TAB_BAR_H)
}

pub(super) const FRAME_Y: f32 = GAP;

pub(super) fn frame(ui: &Ui) -> (f32, f32, f32, f32) {
    let (x, bar_y, w, _) = tab_bar_rect(ui);
    let y = FRAME_Y;
    (x, y, w, (bar_y - GAP - y).floor())
}

pub(super) struct SheetArea {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    pub(super) view: f32,
    pub(super) scroll: f32,
    pub(super) clip: ClipGuard,
}

impl ClickGuiState {
    pub(super) fn track_touch(&mut self, ui: &Ui) {
        let now = ui.input.time;
        if ui.input.left_click {
            self.modules.touch = Touch {
                at: ui.mouse(),
                focus: self.focus,
                since: now,
                ..Touch::default()
            };
            if self.overlay.is_some() && self.focus != Focus::On(Window::Overlay) {
                self.overlay = None;
                self.modules.touch.dead = true;
            }
        } else if !ui.input.left_down && !ui.input.left_release {
            self.modules.touch.at = None;
        }
        let t = &mut self.modules.touch;
        if ui.input.left_down
            && let (Some(at), Some(m)) = (t.at, ui.mouse())
        {
            t.moved |= m.distance(at) > TAP_SLOP;
            t.held |= !t.moved && now - t.since >= HOLD_SECS;
        }
    }

    fn began_in(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let t = &self.modules.touch;
        !t.dead && t.at.is_some_and(|a| self.band_has(a, x, y, w, h))
    }

    pub(super) fn press(&self, ui: &Ui, x: f32, y: f32, w: f32, h: f32, active: bool) -> bool {
        let t = &self.modules.touch;
        active && ui.input.left_release && !t.moved && !t.held && self.began_in(x, y, w, h)
    }

    pub(super) fn lit(&self, ui: &Ui, x: f32, y: f32, w: f32, h: f32, active: bool) -> bool {
        active && ui.input.left_down && !self.modules.touch.moved && self.began_in(x, y, w, h)
    }

    pub(super) fn hint(&self, ui: &Ui, x: f32, y: f32, w: f32, h: f32, active: bool) -> bool {
        active && ui.input.left_down && self.modules.touch.held && self.began_in(x, y, w, h)
    }

    #[allow(clippy::too_many_arguments, reason = "one view, flat arguments")]
    pub(super) fn swipe(
        &mut self,
        ui: &Ui,
        owner: Window,
        view: Window,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        if !ui.input.left_down || matches!(self.drag, Some(Drag::Slider { .. })) {
            return;
        }
        let now = self.window_mut(view).scroll;
        let t = &mut self.modules.touch;
        let (Some(at), Some(m)) = (t.at, ui.mouse()) else {
            return;
        };
        if t.dead || t.focus != Focus::On(owner) || !inside(at, x, y, w, h) {
            return;
        }
        if !t.swiping {
            if (m.y - at.y).abs() <= TAP_SLOP {
                return;
            }
            t.swiping = true;
            t.base_y = m.y;
            t.base_scroll = now;
        }
        let to = (t.base_scroll - (m.y - t.base_y)).round();
        self.window_mut(view).scroll = to;
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    pub(super) fn grab(
        &mut self,
        ui: &Ui,
        id: Control,
        handle: u8,
        x: f32,
        w: f32,
        y: f32,
        active: bool,
    ) -> bool {
        let (gx, gy, gw, gh) = (x - 3.0, y + 8.0, w + 6.0, SLIDER_H - 8.0);
        if active && self.drag.is_none() {
            let t = &self.modules.touch;
            let slid = ui.input.left_down
                && !t.swiping
                && t.at.zip(ui.mouse()).is_some_and(|(a, m)| {
                    let (dx, dy) = ((m.x - a.x).abs(), (m.y - a.y).abs());
                    dx > TAP_SLOP && dx > dy
                });
            if self.began_in(gx, gy, gw, gh) && (slid || self.press(ui, gx, gy, gw, gh, active)) {
                self.drag = Some(Drag::Slider { id, handle });
            }
        }
        matches!(self.drag, Some(Drag::Slider { id: d, .. }) if d == id)
    }

    fn close_button(&mut self, p: &mut Painter, ui: &Ui, active: bool) {
        let look = self.look;
        let (fx, fy, fw, _) = frame(ui);
        let s = FIELD_H;
        let (x, y) = (fx + fw - PAD - s, fy + PAD);
        let lit = self.lit(ui, x, y, s, s, active);
        p.rounded_rect(
            x,
            y,
            s,
            s,
            TAB_RADIUS,
            if lit { look.accent() } else { look.header() },
        );
        let ink = if lit { theme::TEXT_ON } else { look.text() };
        theme::cross(
            p,
            x + ((s - 7.0) * 0.5).floor(),
            y + ((s - 6.0) * 0.5).floor(),
            0xFF00_0000 | ink,
        );
        if self.press(ui, x, y, s, s, active) {
            self.close = true;
        }
    }

    pub(super) fn tab_bar(&mut self, p: &mut Painter, ui: &Ui, active: bool) {
        let look = self.look;
        let (x, y, w, h) = tab_bar_rect(ui);
        p.rounded_rect(x, y, w, h, look.radius, look.panel());
        let tw = ((w - 2.0 * STRIP_PAD) / TABS.len() as f32).floor();
        let (ty, th) = (y + STRIP_PAD, h - 2.0 * STRIP_PAD);
        for (i, (tab, name)) in TABS.into_iter().enumerate() {
            let tx = x + STRIP_PAD + i as f32 * tw;
            let on = self.tab == tab;
            let lit = self.lit(ui, tx, ty, tw, th, active);
            if on || lit {
                p.rounded_rect(
                    tx,
                    ty,
                    tw,
                    th,
                    TAB_RADIUS,
                    if on { look.accent() } else { look.row_hover() },
                );
            }
            let lw = p.atlas.font.width_str(name);
            p.text_plain(
                name,
                (tx + (tw - lw) * 0.5).floor(),
                text_y(ty, th),
                if on { theme::TEXT_ON } else { look.text() },
                look.shadow,
            );
            if self.press(ui, tx, ty, tw, th, active) {
                self.set_tab(tab);
            }
        }
    }

    pub(super) fn config_frame(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        title: &str,
        content: f32,
        active: bool,
    ) -> (f32, f32, f32, f32, f32) {
        let look = self.look;
        let (x, y, w, h) = frame(ui);
        p.rounded_rect(x, y, w, h, look.radius, look.panel());
        self.record(Focus::On(Window::Config), x, y, w, h);
        let nx = x + PAD + 4.0;
        let nw = x + w - 2.0 * PAD - FIELD_H - nx;
        self.label(
            p,
            title,
            nx,
            text_y(y + PAD, FIELD_H),
            nw,
            look.text_title(),
        );
        self.close_button(p, ui, active);
        let head = PAD + FIELD_H;
        let (ix, iy, iw, ih) = (x + PAD, y + head + PAD, w - 2.0 * PAD, h - head - 2.0 * PAD);
        self.swipe(ui, Window::Config, Window::Config, ix, iy, iw, ih);
        let scroll = self.scroll(Window::Config, ui, active, content, ih);
        (ix, iy, iw, ih, scroll)
    }

    #[allow(clippy::too_many_arguments, reason = "one window, flat arguments")]
    pub(super) fn overlay_sheet(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        title: &str,
        fixed: f32,
        content: f32,
        active: bool,
    ) -> Option<SheetArea> {
        let look = self.look;
        let (fx, fy, fw, fh) = frame(ui);
        let h = (HEAD_H + fixed + content).min(fh).floor();
        let view = (h - HEAD_H - fixed).max(0.0);
        let off = self.modules.slide(ui.input.time, h, look.animate);
        let y = fy - off;

        let clip = p.push_clip(fx, fy, fw, fh);
        p.rounded_rect(fx, y, fw, h, look.radius, look.raised());
        self.record(Focus::On(Window::Overlay), fx, y, fw, h);
        self.band = (fy, fy + fh);

        const DONE: &str = "Done";
        let bw = (p.atlas.font.width_str(DONE) + 2.0 * TAB_PAD).floor();
        let (bx, by) = (fx + PAD, y + ((HEAD_H - BUTTON_H) * 0.5).floor());
        let lit = self.lit(ui, bx, by, bw, BUTTON_H, active);
        p.rounded_rect(
            bx,
            by,
            bw,
            BUTTON_H,
            TAB_RADIUS,
            if lit { look.accent_hi() } else { look.accent() },
        );
        p.text_plain(
            DONE,
            bx + TAB_PAD,
            text_y(by, BUTTON_H),
            theme::TEXT_ON,
            look.shadow,
        );
        let nx = bx + bw + 6.0;
        self.label(
            p,
            title,
            nx,
            text_y(y, HEAD_H),
            fx + fw - PAD - nx,
            look.text_title(),
        );
        if self.press(ui, bx, by, bw, BUTTON_H, active) {
            p.pop_clip(clip);
            return None;
        }

        let list_y = y + HEAD_H + fixed;
        self.swipe(ui, Window::Overlay, Window::Overlay, fx, list_y, fw, view);
        let scroll = self.scroll(Window::Overlay, ui, active, content, view);
        Some(SheetArea {
            x: fx,
            y: y + HEAD_H,
            w: fw,
            view,
            scroll,
            clip,
        })
    }

    pub(super) fn module_panel(&mut self, p: &mut Painter, ui: &Ui) {
        let look = self.look;
        let active = self.focus == Focus::On(Window::Grid) && self.drag.is_none();
        if self.expanded.is_some_and(|id| !store().allowed(id)) {
            self.close_sheet();
        }

        let (x, y, w, h) = frame(ui);
        p.rounded_rect(x, y, w, h, look.radius, look.panel());
        self.record(Focus::On(Window::Grid), x, y, w, h);
        self.band = (y, y + h);
        self.rail(p, ui, x + PAD, y + PAD, h - 2.0 * PAD, active);

        let ax = x + PAD + RAIL_W + PAD;
        let aw = x + w - PAD - ax;
        self.sync_pointer(ui, active);
        let sw = aw - FIELD_H - PAD;
        search_field(
            p,
            look,
            Some(&self.mouse_input),
            &mut self.field,
            ax,
            y + PAD,
            sw,
            FIELD_H,
            "Search modules",
        );
        self.close_button(p, ui, active);

        let ay = y + PAD + FIELD_H + PAD;
        let ah = y + h - PAD - ay;
        p.rounded_rect(ax, ay, aw, ah, look.radius, look.nest());
        match self.expanded {
            Some(id) => self.sheet(p, ui, ax, ay, aw, ah, id, active),
            None => self.grid(p, ui, ax, ay, aw, ah, active),
        }
    }

    pub(super) fn open_sheet(&mut self, id: ModuleIx) {
        self.expanded = Some(id);
        self.dropdown = None;
        self.owner.drop_field();
        self.modules.sheet.scroll = 0.0;
    }

    pub(super) fn toggle_settings(&mut self, id: ModuleIx) {
        if self.expanded == Some(id) {
            self.close_sheet();
        } else {
            self.open_sheet(id);
        }
    }

    pub(super) fn close_sheet(&mut self) {
        if matches!(self.overlay, Some(Overlay::Editor(_))) {
            self.overlay = None;
        }
        self.expanded = None;
        self.dropdown = None;
        self.owner.drop_field();
    }

    fn pick_category(&mut self, i: usize) {
        self.modules.cat = i as u8;
        self.modules.grid.scroll = 0.0;
        self.field.clear();
        self.refresh_matches();
        self.close_sheet();
    }

    #[allow(clippy::too_many_arguments, reason = "one view, flat arguments")]
    fn view_scroll(
        &mut self,
        ui: &Ui,
        window: Window,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        content_h: f32,
        active: bool,
    ) -> f32 {
        self.swipe(ui, Window::Grid, window, x, y, w, h);
        self.scroll(window, ui, active, content_h, h)
    }

    fn rail(&mut self, p: &mut Painter, ui: &Ui, x: f32, y: f32, h: f32, active: bool) {
        let look = self.look;
        p.rounded_rect(x, y, RAIL_W, h, look.radius, look.nest());
        let searching = !self.field.text.is_empty();
        let (bx, bw, bh) = (x + 2.0, RAIL_W - 4.0, RAIL_ROW_H - 2.0);
        let mut by = y + 2.0;
        for (i, cat) in Category::ALL.into_iter().enumerate() {
            let on = !searching && self.modules.cat as usize == i;
            let lit = self.lit(ui, bx, by, bw, bh, active);
            let fill = match (on, lit) {
                (true, _) => look.accent(),
                (false, true) => look.row_hover(),
                (false, false) => look.header(),
            };
            p.rounded_rect(bx, by, bw, bh, TAB_RADIUS, fill);
            let ty = text_y(by, bh);
            let armed = cat
                .modules()
                .filter(|m| store().allowed(m.id) && store().armed(m.id))
                .count();
            let mut nw = 0.0;
            if armed > 0 {
                self.number(armed as f32, 0);
                nw = p.atlas.font.width_str(&self.scratch);
                let dim = if on { theme::TEXT_ON } else { look.text_dim() };
                p.text_plain(&self.scratch, bx + bw - 5.0 - nw, ty, dim, look.shadow);
            }
            let text = if on { theme::TEXT_ON } else { look.text() };
            self.label(p, cat.name(), bx + 5.0, ty, bw - 14.0 - nw, text);
            if self.press(ui, bx, by, bw, bh, active) {
                self.pick_category(i);
            }
            by += RAIL_ROW_H;
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one view, flat arguments")]
    fn grid(&mut self, p: &mut Painter, ui: &Ui, x: f32, y: f32, w: f32, h: f32, active: bool) {
        let look = self.look;
        let cat = Category::ALL[self.modules.cat as usize % Category::ALL.len()];
        let searching = !self.field.text.is_empty();
        let n = if searching {
            self.matches.len()
        } else {
            cat.modules().filter(|m| store().allowed(m.id)).count()
        };
        let inner = w - 2.0 * PAD;
        let cols = ((inner + PAD) / (CARD_MIN_W + PAD)).floor().max(1.0);
        let cw = ((inner - (cols - 1.0) * PAD) / cols).floor();
        let content_h = if n == 0 {
            HINT_H + 2.0 * PAD
        } else {
            (n as f32 / cols).ceil() * (CARD_H + PAD) + PAD
        };
        let scroll = self.view_scroll(ui, Window::Grid, x, y, w, h, content_h, active);

        let guard = p.push_clip(x, y, w, h);
        self.band = (y, y + h);
        let outlined = self.outlined();
        let matches = std::mem::take(&mut self.matches);
        let in_cat = cat
            .modules()
            .filter(move |m| !searching && store().allowed(m.id))
            .map(|m| m.id);
        let cols = cols as usize;
        for (i, id) in matches.iter().copied().chain(in_cat).enumerate() {
            let cx = x + PAD + (i % cols) as f32 * (cw + PAD);
            let cy = y + PAD + (i / cols) as f32 * (CARD_H + PAD) - scroll;
            if cy + CARD_H > y && cy < y + h {
                self.card(p, ui, cx, cy, cw, id, outlined == Some(id), active);
            }
        }
        self.matches = matches;
        if n == 0 {
            let line = if searching {
                "No match"
            } else {
                "Nothing the profile allows"
            };
            self.label(p, line, x + 8.0, y + PAD + 2.0, w - 16.0, look.text_dim());
        }
        p.pop_clip(guard);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    fn card(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        id: ModuleIx,
        outlined: bool,
        active: bool,
    ) {
        let look = self.look;
        let m = module(id);
        let on = store().armed(id);
        let sw = PILL_W + 2.0 * PAD + 6.0;
        let bw = w - sw;
        let body_lit = self.lit(ui, x, y, bw, CARD_H, active);
        p.rounded_rect(
            x,
            y,
            w,
            CARD_H,
            look.radius,
            if body_lit {
                look.nest_hi()
            } else {
                look.header()
            },
        );
        if on {
            p.fill(
                x,
                y + look.radius,
                2.0,
                CARD_H - 2.0 * look.radius,
                look.accent(),
            );
        }
        if outlined {
            p.outline(x, y, w, CARD_H, look.accent_hi());
        }
        let (tx, tw) = (x + PAD + 2.0, bw - PAD - 4.0);
        self.label(p, m.name, tx, y + PAD + 2.0, tw, look.text_title());
        self.wrap(p, m.desc, tw);
        let mut ly = y + PAD + 14.0;
        for &(a, b) in self.tip_lines.iter().take(DESC_LINES) {
            p.text_plain(&m.desc[a..b], tx, ly, look.text_dim(), look.shadow);
            ly += DESC_LINE;
        }

        let (px, py) = (x + w - PAD - PILL_W, y + PAD + 2.0);
        if self.lit(ui, x + bw, y, sw, CARD_H, active) {
            p.rounded_rect(
                px - 4.0,
                py - 3.0,
                PILL_W + 8.0,
                PILL_H + 6.0,
                TAB_RADIUS,
                look.row_hover(),
            );
        }
        pill(p, look, px, py, on);

        if self.press(ui, x + bw, y, sw, CARD_H, active) {
            store().toggle(id);
        } else if self.press(ui, x, y, bw, CARD_H, active) {
            self.open_sheet(id);
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one view, flat arguments")]
    fn sheet(
        &mut self,
        p: &mut Painter,
        ui: &Ui,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        id: ModuleIx,
        active: bool,
    ) {
        let look = self.look;
        let content_h = HEAD_H + self.settings_height(id) + PAD;
        let scroll = self.view_scroll(ui, Window::Sheet, x, y, w, h, content_h, active);
        let guard = p.push_clip(x, y, w, h);
        self.band = (y, y + h);
        let top = y - scroll;

        const BACK: &str = "Back";
        let (bx, by) = (x + PAD, top + ((HEAD_H - BUTTON_H) * 0.5).floor());
        let bw = (p.atlas.font.width_str(BACK) + 2.0 * TAB_PAD).floor();
        let lit = self.lit(ui, bx, by, bw, BUTTON_H, active);
        let fill = if lit { look.nest_hi() } else { look.header() };
        p.rounded_rect(bx, by, bw, BUTTON_H, TAB_RADIUS, fill);
        p.text_plain(
            BACK,
            bx + TAB_PAD,
            text_y(by, BUTTON_H),
            look.text(),
            look.shadow,
        );

        let on = store().armed(id);
        let sx = x + w - PAD - PILL_W;
        pill(p, look, sx, top + ((HEAD_H - PILL_H) * 0.5).floor(), on);
        let nx = bx + bw + 6.0;
        self.label(
            p,
            module(id).name,
            nx,
            text_y(top, HEAD_H),
            sx - 6.0 - nx,
            look.text_title(),
        );

        if self.press(ui, bx, by, bw, BUTTON_H, active) {
            self.close_sheet();
            p.pop_clip(guard);
            return;
        }
        if self.press(ui, sx - PAD, top, PILL_W + 2.0 * PAD, HEAD_H, active) {
            store().toggle(id);
        }
        self.settings(p, ui, x, top + HEAD_H, w, id, active);
        p.pop_clip(guard);
    }
}
