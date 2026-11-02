use super::mods::{ERROR, OK, PENDING, Style};
use super::{BIND_H, ClickGuiState, FIELD_H, Focus, INDENT, TIP_LINE, Ui, Window, text_y};
use crate::gui::painter::Painter;
use crate::gui::widgets::{MultiLineTextBox, TextBox, code_text};
use crate::modules::script::{self, SyntaxError};
use crate::text::LINE_HEIGHT;

const EDIT_W: f32 = 480.0;
const NAME_MAX: usize = 72;
const CHECK_AFTER: f32 = 0.3;
const MIN_ROWS: usize = 4;
const BOX_PAD: f32 = 4.0;
const WHEEL_ROWS: f32 = 3.0;

pub(super) struct ModEditor {
    original: Option<String>,
    opened_name: String,
    opened_code: String,
    name: TextBox,
    code: MultiLineTextBox,
    on_code: bool,
    top_row: usize,
    pending: bool,
    edited_at: f32,
    syntax: Result<(), SyntaxError>,
    discard_armed: bool,
    error: Option<String>,
}

impl ClickGuiState {
    pub fn typing(&self) -> bool {
        self.tab == super::Tab::Mods && self.mod_editor.is_some()
    }

    pub(super) fn open_mod_editor(&mut self, file: Option<String>) {
        let (name, code) = match &file {
            None => (String::new(), script::TEMPLATE.to_string()),
            Some(f) => match script::open_source(f) {
                Ok(text) => (f.strip_suffix(".lua").unwrap_or(f).to_string(), text),
                Err(why) => return script::set_install_note(why),
            },
        };
        let mut name_box = TextBox::pinned(NAME_MAX, "name, e.g. autogg");
        name_box.set_text(&name);
        let mut code_box = MultiLineTextBox::code(script::SOURCE_MAX);
        code_box.set_text(&code_text(&code));
        code_box.inner.cursor = 0;
        code_box.inner.anchor = 0;
        self.mod_editor = Some(ModEditor {
            on_code: file.is_some(),
            original: file,
            opened_name: name_box.text.clone(),
            opened_code: code_box.text().to_string(),
            name: name_box,
            code: code_box,
            top_row: 0,
            pending: true,
            edited_at: f32::NEG_INFINITY,
            syntax: Ok(()),
            discard_armed: false,
            error: None,
        });
    }

    pub(super) fn cancel_mod_editor(&mut self) {
        if let Some(e) = self.mod_editor.as_mut()
            && (e.name.text != e.opened_name || e.code.text() != e.opened_code)
            && !e.discard_armed
        {
            e.discard_armed = true;
        } else {
            self.mod_editor = None;
        }
    }

    pub(super) fn mod_editor_window(&mut self, p: &mut Painter, ui: &Ui) {
        let Some(mut e) = self.mod_editor.take() else {
            return;
        };
        let look = self.look;
        let active = self.focus == Focus::On(Window::Config) && self.drag.is_none();
        let title = match &e.original {
            None => "New mod".to_string(),
            Some(f) => format!("Edit {f}"),
        };
        let shell = self.config_shell(p, ui, &title, 1.0e5, active, EDIT_W);
        self.config.scroll = 0.0;
        let Some((x, top, cw, view_h, _)) = shell else {
            self.mod_editor = Some(e);
            return;
        };
        let guard = p.push_clip(x, top, cw, view_h);
        self.band = (top, top + view_h);
        let ix = x + INDENT;
        let iw = cw - 2.0 * INDENT;
        let bh = BIND_H - 1.0;
        let clicked = |s: &Self, x: f32, y: f32, w: f32, h: f32| {
            active && ui.input.left_click && s.hit(ui, x, y, w, h)
        };

        let name_y = top + 4.0 + TIP_LINE;
        let box_y = name_y + FIELD_H + 2.0 + TIP_LINE + 4.0;
        let footer_h = 2.0 * TIP_LINE + 3.0 + bh + 4.0;
        let min_box = MIN_ROWS as f32 * LINE_HEIGHT + 2.0 * BOX_PAD;
        let box_h = (top + view_h - box_y - footer_h - 4.0).max(min_box).floor();
        let rows = ((box_h - 2.0 * BOX_PAD) / LINE_HEIGHT).floor().max(1.0) as usize;
        let (tx, ty) = (ix + BOX_PAD, box_y + BOX_PAD);
        let tw = iw - 2.0 * BOX_PAD - 4.0;
        let in_box = active && self.hit(ui, ix, box_y, iw, box_h);

        if clicked(self, ix, name_y, iw, FIELD_H) {
            e.on_code = false;
        } else if in_box && ui.input.left_click {
            e.on_code = true;
        }

        let font = &p.atlas.font;
        let mut changed = false;
        if !e.on_code {
            e.name.focused = true;
            e.code.inner.focused = false;
            changed = e.name.handle_input(ui.input);
            e.on_code = ui.input.enter || (ui.input.tab && !ui.input.shift);
        } else {
            e.name.focused = false;
            e.code.inner.focused = true;
            let mut code_changed = ui.input.tab && !ui.input.shift && e.code.insert_code("    ");
            code_changed |= e.code.handle_input(ui.input, font, tw);
            if code_changed {
                e.pending = true;
                e.edited_at = ui.input.time;
            }
            changed = code_changed;
            if !ui.input.edits.is_empty() || ui.input.paste || ui.input.tab {
                let caret = e.code.caret_row(font, tw);
                e.top_row = e.top_row.clamp((caret + 1).saturating_sub(rows), caret);
            }
        }
        if changed {
            e.discard_armed = false;
            e.error = None;
        }
        let total = e.code.line_count(font, tw);
        let max_top = total.saturating_sub(rows);
        if in_box && ui.input.scroll != 0.0 {
            e.top_row = (e.top_row as isize - (ui.input.scroll * WHEEL_ROWS).round() as isize)
                .max(0) as usize;
        }
        e.top_row = e.top_row.min(max_top);

        if e.pending && ui.input.time - e.edited_at >= CHECK_AFTER {
            e.syntax = script::syntax(e.code.text());
            e.pending = false;
        }

        let original = e.original.as_deref();
        let id = script::mod_id(&e.name.text);
        let taken = id.as_ref().ok().is_some_and(|id| {
            script::with_mods(|mods| {
                mods.iter()
                    .any(|m| &m.id == id && original != Some(m.file.as_str()))
            })
        });
        self.label(p, "Name", ix, top + 4.0, iw, look.text_dim());
        p.rounded_rect(ix, name_y, iw, FIELD_H, 3.0, look.nest());
        if !e.on_code {
            p.fill(
                ix + 2.0,
                name_y + FIELD_H - 1.0,
                iw - 4.0,
                1.0,
                look.accent(),
            );
        }
        self.sync_pointer(ui, active);
        e.name.color = look.text_title();
        let name_clip = p.push_clip(ix + 3.0, name_y, iw - 6.0, FIELD_H);
        e.name.handle_mouse(
            &self.mouse_input,
            &p.atlas.font,
            ix + 4.0,
            name_y,
            iw - 8.0,
            FIELD_H,
        );
        e.name
            .draw(p, ix + 4.0, text_y(name_y, FIELD_H), iw - 8.0, FIELD_H);
        p.pop_clip(name_clip);
        let (line, ink) = match &id {
            Err(why) => (why.clone(), ERROR),
            Ok(id) if taken => (format!("`{id}` is already a mod; pick another name"), ERROR),
            Ok(id) => match original {
                Some(f) if f.strip_suffix(".lua") != Some(id.as_str()) => {
                    (format!("Saved as {id}.lua, replacing {f}"), PENDING)
                }
                _ => (format!("Saved as {id}.lua"), look.text_dim()),
            },
        };
        self.label(p, &line, ix, name_y + FIELD_H + 2.0, iw, ink);

        p.rounded_rect(ix, box_y, iw, box_h, 3.0, look.nest());
        if e.on_code {
            p.fill(ix + 2.0, box_y + box_h - 1.0, iw - 4.0, 1.0, look.accent());
        }
        self.sync_pointer(ui, in_box);
        let shifted = ty - e.top_row as f32 * LINE_HEIGHT;
        e.code
            .handle_mouse(&self.mouse_input, &p.atlas.font, tx, shifted, tw);
        let code_clip = p.push_clip(ix + 1.0, box_y + 1.0, iw - 2.0, box_h - 2.0);
        e.code
            .draw_rows(p, tx, ty, tw, look.text_title(), e.top_row, rows);
        p.pop_clip(code_clip);
        if total > rows {
            let track = box_h - 4.0;
            let thumb = (track * rows as f32 / total as f32).max(6.0);
            let at = (track - thumb) * e.top_row as f32 / max_top.max(1) as f32;
            p.rounded_rect(
                ix + iw - 4.0,
                box_y + 2.0 + at,
                2.0,
                thumb,
                1.0,
                0xFF00_0000 | look.text_dim(),
            );
        }
        let mut cy = box_y + box_h + 3.0;

        let (line, ink, jump): (std::borrow::Cow<str>, u32, _) = match &e.syntax {
            _ if e.pending => ("Checking...".into(), look.text_dim(), None),
            Ok(()) => ("No syntax errors".into(), OK, None),
            Err(SyntaxError {
                line: Some(n),
                message,
            }) => (format!("Line {n}: {message}").into(), ERROR, Some(*n)),
            Err(err) => (err.message.as_str().into(), ERROR, None),
        };
        self.label(p, &line, ix, cy, iw, ink);
        if let Some(n) = jump
            && clicked(self, ix, cy, iw, TIP_LINE)
        {
            let text = e.code.text();
            let at = text
                .split('\n')
                .take(n - 1)
                .map(|l| l.chars().count() + 1)
                .sum::<usize>();
            e.code.inner.cursor = at.min(text.chars().count());
            e.code.inner.anchor = e.code.inner.cursor;
            e.on_code = true;
            let caret = e.code.caret_row(&p.atlas.font, tw);
            e.top_row = caret.saturating_sub(rows / 2).min(max_top);
        }
        cy += TIP_LINE;

        let notice = match &e.error {
            Some(why) => Some((why.as_str(), ERROR)),
            None if e.discard_armed => Some((
                "Unsaved changes: Cancel or Escape again to discard them",
                PENDING,
            )),
            None if cfg!(target_arch = "wasm32") => {
                Some(("Saved mods last as long as this tab", look.text_dim()))
            }
            None => None,
        };
        if let Some((text, ink)) = notice {
            self.label(p, text, ix, cy, iw, ink);
        }
        cy += TIP_LINE + 3.0;

        let busy = script::busy();
        let can_save = id.is_ok() && !taken && !e.code.text().trim().is_empty() && !busy;
        let half = ((iw - 4.0) * 0.5).floor();
        let label = if busy { "Reloading..." } else { "Save" };
        let style = if can_save {
            Style::Primary
        } else {
            Style::Idle
        };
        let save = self.button(p, ui, ix, cy, half, bh, label, style, active) && can_save;
        let cancel = self.button(
            p,
            ui,
            ix + half + 4.0,
            cy,
            iw - half - 4.0,
            bh,
            "Cancel",
            Style::Plain,
            active,
        );
        p.pop_clip(guard);

        if save && let Ok(id) = id {
            match script::save_source(e.original.as_deref(), &id, e.code.text()) {
                Ok(()) => return self.mod_selected = Some(id),
                Err(why) => e.error = Some(why),
            }
        }
        self.mod_editor = Some(e);
        if cancel {
            self.cancel_mod_editor();
        }
    }
}
