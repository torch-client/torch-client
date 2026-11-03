use super::{
    ClickGuiState, FIELD_H, Focus, HEADER_H, HINT_H, PAD_BOTTOM, Panel, SEARCH_ROWS, Ui, Window,
    search_field, text_y, theme,
};
use crate::gui::painter::Painter;
use crate::modules::registry::{Category, ModuleDef, ModuleIx, module};
use crate::modules::store;

#[derive(Clone, Copy)]
enum Tail {
    Dots,
    Category,
}

fn cat_tag(cat: &'static str) -> &'static str {
    &cat[..3.min(cat.len())]
}

fn cat_w(p: &Painter, cat: &str) -> f32 {
    p.atlas.font.width_str(cat)
}

impl ClickGuiState {
    pub(super) fn toggle_settings(&mut self, id: ModuleIx) {
        self.expanded = (self.expanded != Some(id)).then_some(id);
        self.dropdown = None;
        self.owner.drop_field();
    }

    pub(super) fn columns(&mut self, p: &mut Painter, ui: &Ui) {
        for i in 0..self.order.len() {
            let pi = self.order[i] as usize;
            let active = self.focus == Focus::On(Window::Column(pi as u8)) && self.drag.is_none();
            self.panel(p, ui, pi, active);
        }
        self.search_window(p, ui);
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
                    self.toggle_settings(m.id);
                }
            }
            cy += look.row_h;

            if self.expanded == Some(m.id) {
                cy = self.settings(p, ui, x, cy, w, m.id, active);
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

    fn search_h(&self) -> f32 {
        let body = if self.matches.is_empty() {
            HINT_H
        } else {
            self.matches.len().min(SEARCH_ROWS) as f32 * self.look.row_h
        };
        HEADER_H + 3.0 + FIELD_H + 3.0 + body + PAD_BOTTOM
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
        self.arm_search(ui, x + 5.0, fy, w - 10.0, FIELD_H);
        search_field(
            p,
            look,
            Some(&self.mouse_input),
            &mut self.field,
            x + 5.0,
            fy,
            w - 10.0,
            FIELD_H,
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
}
