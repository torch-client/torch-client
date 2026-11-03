use std::sync::atomic::{AtomicU32, Ordering};

use crate::gui::ScreenCtx;
use crate::gui::focus;
use crate::gui::painter::Painter;
use crate::gui::render::{EditKey, GuiInput, clipboard_get, clipboard_set};
use crate::text::{Font, LINE_HEIGHT, Span, Style};

#[derive(Default)]
pub struct TextBox {
    pub text: String,
    pub cursor: usize,
    pub anchor: usize,
    pub focused: bool,
    pub scroll: usize,
    pub max_len: usize,
    pub placeholder: String,
    pub color: u32,
    pub bordered: bool,
    pub suggestion: Option<String>,
    pub no_shadow: bool,
    dragging: bool,
    pending_focus: bool,
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    keyboard_active: bool,
    pub browser_keyboard: bool,
    pub max_width: Option<f32>,
    pub strip_formatting: bool,
    pub raw: bool,
}

const BORDER_INSET: f32 = 4.0;

type Limit<'a> = Option<(&'a Font, f32)>;

impl TextBox {
    pub fn new(max_len: usize, placeholder: &str) -> TextBox {
        TextBox {
            max_len,
            placeholder: placeholder.to_string(),
            color: 0xE0E0E0,
            browser_keyboard: true,
            ..Default::default()
        }
    }

    pub fn bordered(max_len: usize, placeholder: &str) -> TextBox {
        TextBox {
            bordered: true,
            ..TextBox::new(max_len, placeholder)
        }
    }

    pub fn pinned(max_len: usize, placeholder: &str) -> TextBox {
        TextBox {
            focused: true,
            browser_keyboard: false,
            ..TextBox::new(max_len, placeholder)
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.anchor = 0;
        self.scroll = 0;
    }

    pub fn set_text(&mut self, s: &str) {
        let text = s
            .chars()
            .filter(|c| allowed_char(*c))
            .take(self.max_len)
            .collect();
        self.replace_text(text);
    }

    fn replace_text(&mut self, text: String) {
        self.text = text;
        self.cursor = self.char_count();
        self.anchor = self.cursor;
        self.scroll = 0;
    }

    fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_index(&self, char_index: usize) -> usize {
        byte_at(&self.text, char_index)
    }

    fn selection_range(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.char_count();
    }

    fn selected_text(&self) -> String {
        let (a, b) = self.selection_range();
        let (ba, bb) = (self.byte_index(a), self.byte_index(b));
        self.text[ba..bb].to_string()
    }

    fn delete_span(&mut self, a: usize, b: usize) -> bool {
        let (lo, hi) = (a.min(b), a.max(b));
        if lo == hi {
            return false;
        }
        let (bl, bh) = (self.byte_index(lo), self.byte_index(hi));
        self.text.replace_range(bl..bh, "");
        self.cursor = lo;
        self.anchor = lo;
        true
    }

    fn delete_selection(&mut self) -> bool {
        self.delete_span(self.cursor, self.anchor)
    }

    pub(crate) fn insert_raw(&mut self, c: char) -> bool {
        self.delete_selection();
        if self.char_count() >= self.max_len {
            return false;
        }
        self.splice_at_cursor(c.encode_utf8(&mut [0; 4]));
        true
    }

    fn splice_at_cursor(&mut self, s: &str) {
        let at = self.byte_index(self.cursor);
        self.text.insert_str(at, s);
        self.cursor += s.chars().count();
        self.anchor = self.cursor;
    }

    pub fn insert_text(&mut self, s: &str) {
        self.insert(s, None);
    }

    fn insert_char(&mut self, c: char, limit: Limit<'_>) -> bool {
        let mut buf = [0u8; 4];
        self.insert(c.encode_utf8(&mut buf), limit)
    }

    fn insert(&mut self, s: &str, limit: Limit<'_>) -> bool {
        let raw = self.raw;
        let mut filtered = s
            .chars()
            .filter(|&c| if raw { code_char(c) } else { allowed_char(c) })
            .peekable();
        if filtered.peek().is_none() && !s.is_empty() {
            return false;
        }
        let (lo, hi) = self.selection_range();
        let room = self.max_len - (self.char_count() - (hi - lo)).min(self.max_len);
        let add: String = filtered.take(room).collect();
        if let Some((font, max)) = limit {
            let (bl, bh) = (self.byte_index(lo), self.byte_index(hi));
            let kept = font.width_str(&self.text) - font.width_str(&self.text[bl..bh]);
            if kept + font.width_str(&add) > max {
                self.cursor = lo;
                self.anchor = lo;
                return false;
            }
        }
        let deleted = self.delete_selection();
        if add.is_empty() {
            return deleted;
        }
        self.splice_at_cursor(&add);
        true
    }

    fn move_cursor_to(&mut self, pos: usize, extend: bool) {
        self.cursor = pos.min(self.char_count());
        if !extend {
            self.anchor = self.cursor;
        }
    }

    fn word_position_from(&self, dir: i32, from: usize) -> usize {
        let len = self.char_count();
        let from = from.min(len);
        if dir > 0 {
            let mut rest = self.text.chars().skip(from);
            let Some(off) = rest.by_ref().position(|c| c == ' ') else {
                return len;
            };
            from + off + 1 + rest.take_while(|&c| c == ' ').count()
        } else {
            let mut back = self.text[..self.byte_index(from)].chars().rev().peekable();
            let mut result = from;
            while back.next_if(|&c| c == ' ').is_some() {
                result -= 1;
            }
            while back.next_if(|&c| c != ' ').is_some() {
                result -= 1;
            }
            result
        }
    }

    fn word_position(&self, dir: i32) -> usize {
        self.word_position_from(dir, self.cursor)
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    pub fn sync_keyboard(&mut self) -> bool {
        self.sync_keyboard_inner(None)
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    fn sync_keyboard_inner(
        &mut self,
        #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))] limit: Limit<'_>,
    ) -> bool {
        if self.keyboard_active && !crate::platform::keyboard::has_focus() {
            self.keyboard_active = false;
        }
        #[cfg(target_os = "android")]
        let wants = self.focused && !crate::platform::keyboard::dismissed();
        #[cfg(not(target_os = "android"))]
        let wants = self.focused && self.browser_keyboard;

        if wants && !self.keyboard_active {
            crate::platform::keyboard::show(&self.text, self.anchor, self.cursor);
            self.keyboard_active = true;
        } else if !wants && self.keyboard_active {
            crate::platform::keyboard::hide();
            self.keyboard_active = false;
        }
        #[cfg(target_arch = "wasm32")]
        if self.keyboard_active
            && let Some((text, cursor, anchor)) = crate::platform::keyboard::poll()
        {
            let mut clamped: String = text
                .chars()
                .filter(|&c| allowed_char(c))
                .take(self.max_len)
                .collect();
            if let Some((font, max)) = limit
                && font.width_str(&clamped) > max
            {
                clamped = self.text.clone();
            }
            let changed = clamped != self.text;
            if changed {
                self.text = clamped.clone();
            }
            let len = self.char_count();
            self.cursor = cursor.min(len);
            self.anchor = anchor.min(len);
            if clamped != text {
                crate::platform::keyboard::sync_selection(&clamped, self.anchor, self.cursor);
            }
            return changed;
        }
        false
    }

    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    pub fn sync_keyboard(&mut self) -> bool {
        false
    }

    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    fn sync_keyboard_inner(&mut self, _limit: Limit<'_>) -> bool {
        false
    }

    pub fn note_tap(&mut self, inside: bool) {
        if inside {
            self.focused = true;
            #[cfg(target_os = "android")]
            {
                self.keyboard_active = false;
            }
            crate::platform::keyboard::note_tap(true);
        } else if self.focused {
            crate::platform::keyboard::note_tap(false);
        }
    }

    pub fn push_keyboard(&self) {
        #[cfg(target_arch = "wasm32")]
        if self.keyboard_active {
            crate::platform::keyboard::sync_selection(&self.text, self.anchor, self.cursor);
        }
    }

    pub fn handle_input(&mut self, input: &GuiInput) -> bool {
        self.handle_edits(input, None, |_, _| false)
    }

    pub fn handle_edits_validated(
        &mut self,
        input: &GuiInput,
        font: &Font,
        intercept: impl FnMut(&mut TextBox, EditKey) -> bool,
    ) -> bool {
        let limit = self.max_width.map(|w| (font, w));
        self.handle_edits(input, limit, intercept)
    }

    fn handle_edits(
        &mut self,
        input: &GuiInput,
        limit: Limit<'_>,
        mut intercept: impl FnMut(&mut TextBox, EditKey) -> bool,
    ) -> bool {
        let (mut changed, live) = self.frame_input(input, limit);
        if !live {
            return changed;
        }
        if let Some(edited) = self.apply_shortcut(input, limit) {
            changed |= edited;
        }
        if self.take_browser_backspace() {
            changed |= self.apply_edit(EditKey::Backspace, input, limit);
            self.push_browser_backspace();
        }
        for &key in &input.edits {
            if intercept(self, key) {
                continue;
            }
            changed |= self.apply_edit(key, input, limit);
        }
        changed
    }

    fn frame_input(&mut self, input: &GuiInput, limit: Limit<'_>) -> (bool, bool) {
        let changed = self.sync_keyboard_inner(limit);

        if input.defocus && self.focused {
            self.focused = false;
            crate::platform::keyboard::dismiss();
            crate::gui::focus::clear();
            return (changed, false);
        }
        (changed, self.focused)
    }

    fn apply_shortcut(&mut self, input: &GuiInput, limit: Limit<'_>) -> Option<bool> {
        if input.select_all {
            self.select_all();
            return Some(false);
        }
        if input.copy {
            clipboard_set(&self.selected_text());
            return Some(false);
        }
        if input.cut {
            clipboard_set(&self.selected_text());
            return Some(self.delete_selection());
        }
        if input.paste {
            let text = clipboard_get();
            let text = if self.strip_formatting {
                strip_format_codes(&text)
            } else if self.raw {
                code_text(&text)
            } else {
                text
            };
            return Some(self.insert(&text, limit));
        }
        None
    }

    fn take_browser_backspace(&mut self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            self.keyboard_active && crate::platform::keyboard::take_backspace()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            false
        }
    }

    fn push_browser_backspace(&self) {
        #[cfg(target_arch = "wasm32")]
        crate::platform::keyboard::sync(&self.text, self.cursor);
    }

    fn apply_edit(&mut self, key: EditKey, input: &GuiInput, limit: Limit<'_>) -> bool {
        match key {
            EditKey::Char(c) => {
                if input.ctrl && !input.alt {
                    return false;
                }
                self.insert_char(c, limit)
            }
            EditKey::Backspace => {
                if !self.delete_selection() {
                    if input.ctrl {
                        let target = self.word_position(-1);
                        self.delete_span(target, self.cursor);
                    } else if self.cursor > 0 {
                        self.delete_span(self.cursor - 1, self.cursor);
                    }
                }
                true
            }
            EditKey::Delete => {
                if !self.delete_selection() {
                    if input.ctrl {
                        let target = self.word_position(1);
                        self.delete_span(self.cursor, target);
                    } else if self.cursor < self.char_count() {
                        self.delete_span(self.cursor, self.cursor + 1);
                    }
                }
                true
            }
            EditKey::Left => {
                if input.ctrl {
                    let pos = self.word_position(-1);
                    self.move_cursor_to(pos, input.shift);
                } else if self.cursor > 0 {
                    self.move_cursor_to(self.cursor - 1, input.shift);
                }
                false
            }
            EditKey::Right => {
                if input.ctrl {
                    let pos = self.word_position(1);
                    self.move_cursor_to(pos, input.shift);
                } else if self.cursor < self.char_count() {
                    self.move_cursor_to(self.cursor + 1, input.shift);
                }
                false
            }
            EditKey::Home => {
                self.move_cursor_to(0, input.shift);
                false
            }
            EditKey::End => {
                self.move_cursor_to(self.char_count(), input.shift);
                false
            }
            EditKey::Up | EditKey::Down | EditKey::Enter => false,
        }
    }

    fn text_origin(&self, x: f32, y: f32, h: f32) -> (f32, f32) {
        if self.bordered {
            (x + BORDER_INSET, y + (h - 8.0) / 2.0)
        } else {
            (x, y)
        }
    }

    fn text_width(&self, w: f32) -> f32 {
        if self.bordered {
            (w - 2.0 * BORDER_INSET).max(0.0)
        } else {
            w
        }
    }

    fn clicked_pos(&self, font: &Font, tx: f32, tw: f32, mx: f32) -> usize {
        let offset = (mx - tx).clamp(0.0, tw);
        let mut acc = 0.0;
        let mut n = 0;
        for c in self.text.chars().skip(self.scroll) {
            let a = font.advance(c, false);
            if acc + a > offset {
                break;
            }
            acc += a;
            n += 1;
        }
        self.scroll + n
    }

    pub fn handle_mouse(&mut self, input: &GuiInput, font: &Font, x: f32, y: f32, w: f32, h: f32) {
        let was = (self.cursor, self.anchor);

        if input.left_release {
            self.dragging = false;
        }

        let (tx, _ty) = self.text_origin(x, y, h);

        let tw = self.text_width(w);

        if input.left_click {
            if let Some(m) = input.mouse {
                let inside = m.x >= x && m.x <= x + w && m.y >= y && m.y <= y + h;
                self.note_tap(inside);
                if inside {
                    let pos = self.clicked_pos(font, tx, tw, m.x);
                    if input.triple_click {
                        self.select_all();
                    } else if input.double_click {
                        let start = self.word_position_from(-1, pos);
                        let end = self.word_position_from(1, pos);
                        self.move_cursor_to(start, false);
                        self.move_cursor_to(end, true);
                    } else {
                        self.move_cursor_to(pos, input.shift);
                    }
                    self.dragging = true;
                }
            }
        } else if self.dragging && input.left_down {
            if let Some(m) = input.mouse {
                if m.x < tx && self.cursor > 0 {
                    self.scroll = self.scroll.saturating_sub(1);
                    let pos = self.cursor - 1;
                    self.move_cursor_to(pos, true);
                } else if m.x > tx + tw {
                    let pos = (self.clicked_pos(font, tx, tw, tx + tw) + 1).min(self.char_count());
                    self.move_cursor_to(pos, true);
                } else {
                    let pos = self.clicked_pos(font, tx, tw, m.x);
                    self.move_cursor_to(pos, true);
                }
            }
        }

        if (self.cursor, self.anchor) != was {
            self.push_keyboard();
        }
    }

    pub fn focus(&mut self) {
        self.focused = true;
        self.pending_focus = true;
        self.cursor = self.char_count();
        self.anchor = self.cursor;
    }

    pub fn update(&mut self, p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32, w: f32, h: f32) {
        let ring = focus::next(true);
        self.handle_mouse(ctx.input, &p.atlas.font, x, y, w, h);
        let took = std::mem::take(&mut self.pending_focus)
            || (ctx.input.left_click && ctx.hovering(x, y, w, h));
        if took {
            focus::claim();
        }
        if focus::enabled() {
            self.focused = ring || took;
        } else if took {
            self.focused = true;
        }
        #[cfg(target_os = "android")]
        if ctx.input.left_click && !took && self.focused {
            self.focused = false;
            if ring {
                focus::clear();
            }
        }
        #[cfg(feature = "mobile_ui")]
        if self.focused {
            keyboard_lift::note_focused_field(y + h);
        }
        self.handle_input(ctx.input);
        self.draw(p, x, y, w, h);
    }

    pub(crate) fn scroll_to_caret(&mut self, font: &Font, w: f32) -> usize {
        self.scroll = self.scroll.min(self.cursor);
        let mut start = self.byte_index(self.scroll);
        let caret_b = self.byte_index(self.cursor);
        let mut caret_w = font.width_str(&self.text[start..caret_b]);
        while caret_w > w && self.scroll < self.cursor {
            let Some(c) = self.text[start..].chars().next() else {
                break;
            };
            caret_w -= font.advance(c, false);
            start += c.len_utf8();
            self.scroll += 1;
        }
        start
    }

    pub fn draw(&mut self, p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
        if self.browser_keyboard {
            crate::platform::keyboard::arm();
        }
        if self.bordered {
            let frame_color = if self.focused {
                0xFFFF_FFFF
            } else {
                0xFFA0_A0A0
            };
            p.fill(x - 1.0, y - 1.0, w + 2.0, h + 2.0, frame_color);
            p.fill(x, y, w, h, 0xFF00_0000);
        }
        let font = &p.atlas.font;
        let (tx, text_y) = self.text_origin(x, y, h);
        let w = self.text_width(w);

        let start = self.scroll_to_caret(font, w);

        let mut shown_w = 0.0;
        let mut end = start;
        for c in self.text[start..].chars() {
            let a = font.advance(c, false);
            if shown_w + a > w {
                break;
            }
            shown_w += a;
            end += c.len_utf8();
        }
        let shown = &self.text[start..end];
        let at = |n: usize| byte_at(shown, n);

        let shadow = !self.no_shadow;
        if self.text.is_empty() && !self.focused {
            p.text_plain(&self.placeholder, tx, text_y, 0x707070, shadow);
        } else {
            p.text_plain(shown, tx, text_y, self.color, shadow);
            let (sa, sb) = self.selection_range();
            if sa != sb {
                let a = at(sa.max(self.scroll) - self.scroll);
                let b = at(sb.max(self.scroll) - self.scroll);
                let mid = &shown[a..b];
                let x0 = tx + font.width_str(&shown[..a]);
                let x1 = (x0 + font.width_str(mid)).min(tx + w);
                draw_selection(p, mid, x0, x1, text_y, text_y - 1.0, text_y + 10.0);
            }
        }

        let len = self.char_count();
        if let Some(suggestion) = &self.suggestion
            && self.cursor >= len
            && len < self.max_len
        {
            p.text_plain(suggestion, tx + shown_w, text_y, 0x808080, shadow);
        }

        if self.focused && blink_on(p.frame) {
            let cx = tx + font.width_str(&shown[..at(self.cursor - self.scroll)]);
            draw_caret(
                p,
                cx,
                text_y,
                self.cursor >= len,
                self.color,
                shadow,
                (-1.0, 10.0),
            );
        }
    }
}

#[derive(Default)]
pub struct MultiLineTextBox {
    pub inner: TextBox,
    lines: Vec<std::ops::Range<usize>>,
    width: f32,
    stale: bool,
    pub line_limit: usize,
}

impl MultiLineTextBox {
    pub fn new(max_len: usize, line_limit: usize) -> MultiLineTextBox {
        MultiLineTextBox {
            inner: TextBox::new(max_len, ""),
            lines: Vec::new(),
            width: 0.0,
            stale: true,
            line_limit,
        }
    }

    pub fn code(max_len: usize) -> MultiLineTextBox {
        let mut b = MultiLineTextBox::new(max_len, 0);
        b.inner.raw = true;
        b.inner.browser_keyboard = false;
        b
    }

    pub fn insert_code(&mut self, s: &str) -> bool {
        let changed = self.inner.insert(s, None);
        self.stale |= changed;
        changed
    }

    pub fn caret_row(&mut self, font: &Font, width: f32) -> usize {
        self.relayout(font, width);
        Self::line_at(&self.lines, self.inner.cursor)
    }

    pub fn text(&self) -> &str {
        &self.inner.text
    }

    pub fn set_text(&mut self, s: &str) {
        self.inner
            .replace_text(s.chars().take(self.inner.max_len).collect());
        self.stale = true;
    }

    fn relayout(&mut self, font: &Font, width: f32) {
        if !self.stale && self.width == width {
            return;
        }
        font.wrap_ranges(&self.inner.text, width, &mut self.lines);
        self.width = width;
        self.stale = false;
    }

    pub fn line_count(&mut self, font: &Font, width: f32) -> usize {
        self.relayout(font, width);
        self.lines.len()
    }

    fn line_at(lines: &[std::ops::Range<usize>], cursor: usize) -> usize {
        lines.iter().rposition(|r| r.start <= cursor).unwrap_or(0)
    }

    pub fn handle_input(&mut self, input: &GuiInput, font: &Font, width: f32) -> bool {
        self.relayout(font, width);
        if !self.inner.focused {
            return self.inner.handle_input(input);
        }

        let mutating = self.line_limit > 0
            && (input.paste
                || input.cut
                || input.edits.iter().any(|k| {
                    matches!(
                        k,
                        EditKey::Char(_) | EditKey::Backspace | EditKey::Delete | EditKey::Enter
                    )
                }));
        let undo = mutating.then(|| {
            (
                self.inner.text.clone(),
                self.inner.cursor,
                self.inner.anchor,
            )
        });

        let Self { inner, lines, .. } = self;
        let mut fresh = false;
        let mut entered = false;
        let mut changed = inner.handle_edits(input, None, |field, key| {
            match key {
                EditKey::Enter => {
                    entered |= field.insert_raw('\n');
                    fresh = false;
                    return true;
                }
                EditKey::Up | EditKey::Down | EditKey::Home | EditKey::End => {}
                EditKey::Left | EditKey::Right => return false,
                EditKey::Char(_) | EditKey::Backspace | EditKey::Delete => {
                    fresh = false;
                    return false;
                }
            }
            if !fresh {
                font.wrap_ranges(&field.text, width, lines);
                fresh = true;
            }
            let line = Self::line_at(lines, field.cursor);
            let column = field.cursor - lines[line].start;
            let same_column =
                |target: &std::ops::Range<usize>| (target.start + column).min(target.end);
            let pos = match key {
                EditKey::Up if line > 0 => same_column(&lines[line - 1]),
                EditKey::Down if line + 1 < lines.len() => same_column(&lines[line + 1]),
                EditKey::Home => lines[line].start,
                EditKey::End => lines[line].end,
                _ => return true,
            };
            field.move_cursor_to(pos, input.shift);
            true
        });
        changed |= entered;

        if changed {
            self.stale = true;
            self.relayout(font, width);
            if self.line_limit > 0
                && self.lines.len() > self.line_limit
                && let Some((text, cursor, anchor)) = undo
            {
                self.inner.text = text;
                self.inner.cursor = cursor;
                self.inner.anchor = anchor;
                self.stale = true;
                self.relayout(font, width);
                changed = false;
            }
        }
        changed
    }

    pub fn handle_mouse(&mut self, input: &GuiInput, font: &Font, x: f32, y: f32, width: f32) {
        if !input.left_click {
            return;
        }
        let Some(m) = input.mouse else { return };
        let page_rows = if self.line_limit > 0 {
            self.line_limit
        } else {
            self.lines.len() + 1
        };
        let on_page =
            m.x >= x && m.x <= x + width && m.y >= y && m.y <= y + page_rows as f32 * LINE_HEIGHT;
        if on_page {
            crate::platform::keyboard::note_tap(true);
        } else if self.inner.focused {
            crate::platform::keyboard::note_tap(false);
        }
        self.relayout(font, width);
        let rows = self.lines.len() as f32;
        if m.x < x || m.x > x + width || m.y < y || m.y > y + rows * LINE_HEIGHT {
            return;
        }
        let row = (((m.y - y) / LINE_HEIGHT) as usize).min(self.lines.len() - 1);
        let range = self.lines[row].clone();
        let mut pos = range.start;
        let mut w = 0.0f32;
        for c in char_slice(&self.inner.text, range).chars() {
            let adv = font.advance(c, false);
            if x + w + adv / 2.0 > m.x {
                break;
            }
            w += adv;
            pos += 1;
        }
        self.inner.cursor = pos;
        self.inner.anchor = pos;
    }

    pub fn draw(&mut self, p: &mut Painter, x: f32, y: f32, width: f32, color: u32) {
        self.draw_rows(p, x, y, width, color, 0, usize::MAX);
    }

    #[allow(clippy::too_many_arguments, reason = "one widget, flat arguments")]
    pub fn draw_rows(
        &mut self,
        p: &mut Painter,
        x: f32,
        y: f32,
        width: f32,
        color: u32,
        first: usize,
        count: usize,
    ) {
        self.relayout(&p.atlas.font, width);
        let end = first.saturating_add(count).min(self.lines.len());
        if first >= end {
            return;
        }
        let (sa, sb) = self.inner.selection_range();
        let cursor = self.inner.cursor;
        let caret_row = Self::line_at(&self.lines, cursor);
        let text = self.inner.text.as_str();
        let (mut at, mut byte) = (0usize, 0usize);
        let mut seek = |to: usize| {
            for c in text[byte..].chars().take(to - at) {
                byte += c.len_utf8();
            }
            at = to;
            byte
        };
        for row in first..end {
            let range = self.lines[row].clone();
            let b0 = seek(range.start);
            let b1 = seek(range.end);
            let line = &text[b0..b1];
            let ly = y + (row - first) as f32 * LINE_HEIGHT;
            p.text_plain(line, x, ly, color, false);
            let (a, b) = (
                sa.clamp(range.start, range.end),
                sb.clamp(range.start, range.end),
            );
            if a != b {
                let pre = char_slice(line, 0..a - range.start);
                let mid = char_slice(line, a - range.start..b - range.start);
                let x0 = x + p.atlas.font.width_str(pre);
                let x1 = x0 + p.atlas.font.width_str(mid);
                draw_selection(p, mid, x0, x1, ly, ly - 1.0, ly + 10.0);
            }
            if row == caret_row && self.inner.focused && blink_on(p.frame) {
                let pre = char_slice(line, 0..cursor - range.start);
                let cx = x + p.atlas.font.width_str(pre);
                let at_end = row + 1 == self.lines.len() && cursor == range.end;
                draw_caret(p, cx, ly, at_end, color, false, (-1.0, 10.0));
            }
        }
    }
}

pub(crate) fn code_char(c: char) -> bool {
    ((c as u32) >= 32 || c == '\n') && c != '\u{7f}'
}

pub(crate) fn code_text(s: &str) -> String {
    let s = s.strip_prefix('\u{feff}').unwrap_or(s);
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                chars.next_if_eq(&'\n');
                out.push('\n');
            }
            '\t' => out.push_str("    "),
            c if code_char(c) => out.push(c),
            _ => {}
        }
    }
    out
}

pub(crate) fn byte_at(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map_or(s.len(), |(i, _)| i)
}

pub(crate) fn char_slice(s: &str, r: std::ops::Range<usize>) -> &str {
    let a = byte_at(s, r.start);
    let b = a + byte_at(&s[a..], r.end.saturating_sub(r.start));
    &s[a..b]
}

pub(crate) fn blink_on(frame: u32) -> bool {
    (frame / 6) % 2 == 0
}

pub(crate) fn draw_caret(
    p: &mut Painter,
    x: f32,
    y: f32,
    append: bool,
    color: u32,
    shadow: bool,
    insert: (f32, f32),
) {
    if append {
        draw_append_caret(p, x, y, color, shadow);
    } else {
        draw_insert_caret(p, x + insert.0, y, insert.1, color);
    }
}

pub(crate) fn draw_append_caret(p: &mut Painter, x: f32, y: f32, color: u32, shadow: bool) {
    p.text_plain("_", x, y, color, shadow);
}

pub(crate) fn draw_insert_caret(p: &mut Painter, x: f32, y: f32, line_height: f32, color: u32) {
    p.fill(x, y - 1.0, 1.0, line_height + 1.0, color | 0xFF00_0000);
}

pub(crate) fn draw_selection(
    p: &mut Painter,
    text: &str,
    x0: f32,
    x1: f32,
    text_y: f32,
    top: f32,
    bottom: f32,
) {
    p.fill(x0, top, (x1 - x0).max(1.0), bottom - top, 0xFFFF_FFFF);
    p.text_plain(text, x0, text_y, 0x000000, false);
}

pub(crate) fn allowed_char(c: char) -> bool {
    c != '\u{a7}' && (c as u32) >= 32 && c != '\u{7f}'
}

fn strip_format_codes(s: &str) -> String {
    let mut chars = s.chars().filter(|&c| c != '\r').peekable();
    let mut out = String::with_capacity(s.len());
    while let Some(c) = chars.next() {
        if c == '\u{a7}'
            && chars
                .next_if(|n| {
                    n.is_ascii_hexdigit() || matches!(n.to_ascii_lowercase(), 'k'..='o' | 'r')
                })
                .is_some()
        {
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(feature = "audio")]
pub(in crate::gui) fn play_click() {
    crate::audio::play(
        "ui.button.click",
        crate::audio::SoundCategory::Ambient,
        0.25,
        1.0,
    );
}

#[derive(Clone, Copy, Debug)]
pub struct HeaderFooter {
    pub header_h: f32,
    pub footer_h: f32,
}

const CONTENT_MARGIN_TOP: f32 = 30.0;

impl HeaderFooter {
    pub const DEFAULT_H: f32 = 33.0;

    pub const fn new(header_h: f32, footer_h: f32) -> HeaderFooter {
        HeaderFooter { header_h, footer_h }
    }

    pub fn title_y(&self) -> f32 {
        ((self.header_h - LINE_HEIGHT) / 2.0).floor()
    }

    pub fn footer_y(&self, vh: f32) -> f32 {
        vh - self.footer_h + ((self.footer_h - WIDGET_HEIGHT) / 2.0).round()
    }

    pub fn content_rect(&self, vw: f32, vh: f32) -> (f32, f32, f32, f32) {
        (
            0.0,
            self.header_h,
            vw,
            (vh - self.header_h - self.footer_h).max(0.0),
        )
    }

    pub fn content_y(&self, vh: f32, content_h: f32) -> f32 {
        (self.header_h + CONTENT_MARGIN_TOP).min(vh - self.footer_h - content_h)
    }
}

pub fn centered_x(vw: f32) -> f32 {
    ((vw - WIDGET_WIDTH_BIG) / 2.0).floor()
}

pub(in crate::gui) const FOOTER_W: f32 = 150.0;
pub(in crate::gui) const FOOTER_GAP: f32 = 8.0;

pub(in crate::gui) fn centered_row_x(vw: f32, total_w: f32) -> f32 {
    ((vw - total_w) / 2.0).floor()
}

pub(crate) fn wrap(p: &Painter, lines: &[&str], w: f32) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let mut cur = String::new();
        for word in line.split(' ') {
            let next = if cur.is_empty() {
                word.to_owned()
            } else {
                format!("{cur} {word}")
            };
            if !cur.is_empty() && p.atlas.font.width_str(&next) > w - 4.0 {
                out.push(std::mem::replace(&mut cur, word.to_owned()));
            } else {
                cur = next;
            }
        }
        out.push(cur);
    }
    out
}

pub const WIDGET_HEIGHT: f32 = 20.0;

pub const TAB_BAR_H: f32 = 24.0;

pub fn tab_bar(p: &mut Painter, ctx: &ScreenCtx, labels: &[&str], current: usize) -> Option<usize> {
    tab_bar_with(p, ctx, labels, current, |_| true)
}

pub fn tab_layout(vw: f32, count: usize) -> (f32, f32) {
    const MAX_W: f32 = 400.0;
    const MARGIN: f32 = 14.0;
    let even = |n: f32| (n / 2.0).ceil() * 2.0;
    let tabs_w = (MAX_W.min(vw) - 2.0 * MARGIN).floor();
    let tab_w = even((tabs_w / count.max(1) as f32).floor());
    let x0 = even(((vw - tabs_w) / 2.0).floor());
    (x0, tab_w)
}

pub fn tab_bar_with(
    p: &mut Painter,
    ctx: &ScreenCtx,
    labels: &[&str],
    current: usize,
    active: impl Fn(usize) -> bool,
) -> Option<usize> {
    const INACTIVE: u32 = 0xA0_A0A0;
    let (x0, tab_w) = tab_layout(ctx.vw, labels.len());
    let right = x0 + tab_w * labels.len() as f32;
    p.sprite("header_separator", 0.0, TAB_BAR_H - 2.0, x0, 2.0);
    p.sprite(
        "header_separator",
        right,
        TAB_BAR_H - 2.0,
        (ctx.vw - right).max(0.0),
        2.0,
    );

    let mut picked = None;
    for (i, label) in labels.iter().enumerate() {
        let x = x0 + i as f32 * tab_w;
        let selected = i == current;
        let enabled = active(i);
        let color = if enabled { 0xFF_FFFF } else { INACTIVE };
        let hovered = ctx.hovering(x, 0.0, tab_w, TAB_BAR_H);
        let sprite = match (selected, false) {
            (true, false) => "widget/tab_selected",
            (true, true) => "widget/tab_selected_highlighted",
            (false, false) => "widget/tab",
            (false, true) => "widget/tab_highlighted",
        };
        p.sprite(sprite, x, 0.0, tab_w, TAB_BAR_H);
        let text_w = p.atlas.font.width_str(label);
        if selected {
            p.sprite(
                "menu_background",
                x + 2.0,
                2.0,
                tab_w - 4.0,
                TAB_BAR_H - 2.0,
            );
            let line_w = text_w.min(tab_w - 4.0);
            p.fill(
                x + ((tab_w - line_w) / 2.0).floor(),
                TAB_BAR_H - 2.0,
                line_w,
                1.0,
                0xFF00_0000 | color,
            );
        }
        let top = if selected { 0.0 } else { 3.0 };
        let y = ((top + TAB_BAR_H - LINE_HEIGHT) / 2.0).floor() + 1.0;
        p.text_plain(label, x + ((tab_w - text_w) / 2.0).floor(), y, color, true);
        if !selected && enabled && hovered && ctx.input.left_click {
            picked = Some(i);
        }
    }
    picked
}

pub const WIDGET_WIDTH_BIG: f32 = 200.0;

const TEXT_MARGIN: f32 = 2.0;

const LABEL_ACTIVE: u32 = 0xFFFFFF;

const LABEL_INACTIVE: u32 = 0xA0A0A0;

const HANDLE_WIDTH: f32 = 8.0;

pub(in crate::gui) fn draw_label_spans(
    p: &mut Painter,
    label: &[Span],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let lw = p.atlas.font.width(label);
    let (left, right) = (x + TEXT_MARGIN, x + w - TEXT_MARGIN);
    let mut cx = x + w / 2.0;
    if lw <= right - left {
        cx = cx.clamp(left + lw / 2.0, right - lw / 2.0);
    }
    let top = ((y + y + h - LINE_HEIGHT) / 2.0 + 1.0).floor();
    p.text(label, (cx - lw / 2.0).floor(), top, true);
}

pub(in crate::gui) fn draw_button_spans(
    p: &mut Painter,
    ctx: &ScreenCtx,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &[Span],
    active: bool,
) -> bool {
    let hovered = ctx.hovering(x, y, w, h);
    let focused = focus::next(active) && focus::by_keyboard();
    let sprite = match (active, hovered || focused) {
        (false, _) => "widget/button_disabled",
        (true, false) => "widget/button",
        (true, true) => "widget/button_highlighted",
    };
    p.sprite(sprite, x, y, w, h);
    if active {
        draw_label_spans(p, label, x, y, w, h);
    } else {
        let greyed: Vec<Span> = label
            .iter()
            .map(|s| Span {
                text: s.text.clone(),
                style: Style {
                    color: LABEL_INACTIVE,
                    ..s.style
                },
            })
            .collect();
        draw_label_spans(p, &greyed, x, y, w, h);
    }
    let clicked = active && click_in(ctx, x, y, w, h);
    #[cfg(not(feature = "mobile_ui"))]
    if clicked {
        focus::claim();
    }
    let pressed = clicked || (active && focused && ctx.input.select);
    #[cfg(feature = "audio")]
    if pressed {
        play_click();
    }
    pressed
}

pub struct Button<'a> {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub label: &'a str,
    pub active: bool,
}

impl<'a> Button<'a> {
    pub fn new(x: f32, y: f32, w: f32, h: f32, label: &'a str) -> Button<'a> {
        Button {
            x,
            y,
            w,
            h,
            label,
            active: true,
        }
    }

    pub fn draw(&self, p: &mut Painter, ctx: &ScreenCtx) -> bool {
        let label = [Span::colored(self.label, LABEL_ACTIVE)];
        draw_button_spans(p, ctx, self.x, self.y, self.w, self.h, &label, self.active)
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::gui) fn icon_button(
    p: &mut Painter,
    ctx: &ScreenCtx,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    icon: &str,
    (icon_w, icon_h): (f32, f32),
) -> bool {
    let pressed = draw_button_spans(p, ctx, x, y, w, h, &[], true);
    let half = |n: f32| (n / 2.0).floor();
    p.sprite(
        icon,
        x + half(w) - half(icon_w),
        y + half(h) - half(icon_h),
        icon_w,
        icon_h,
    );
    pressed
}

pub(in crate::gui) fn click_in(ctx: &ScreenCtx, x: f32, y: f32, w: f32, h: f32) -> bool {
    #[cfg(not(feature = "mobile_ui"))]
    return ctx.hovering(x, y, w, h) && ctx.input.left_click;
    #[cfg(feature = "mobile_ui")]
    return ctx.hovering(x, y, w, h)
        && ctx.input.left_release
        && touch_tap::pressed_in(x, y, w, h)
        && !touch_tap::scrolled();
}

#[cfg(feature = "mobile_ui")]
pub(in crate::gui) mod keyboard_lift {
    use std::sync::atomic::{AtomicU32, Ordering};

    const NONE: u32 = u32::MAX;
    const MARGIN: f32 = 6.0;

    static FIELD_BOTTOM: AtomicU32 = AtomicU32::new(NONE);
    static LIFT: AtomicU32 = AtomicU32::new(0);

    pub(in crate::gui) fn note_focused_field(bottom: f32) {
        FIELD_BOTTOM.store(bottom.to_bits(), Ordering::Relaxed);
    }

    pub(in crate::gui) fn begin(enabled: bool, vh: f32, inset: f32, held: bool) -> f32 {
        let bottom = FIELD_BOTTOM.swap(NONE, Ordering::Relaxed);
        if enabled && held {
            return current();
        }
        let lift = if enabled && inset > 0.0 && bottom != NONE {
            let keyboard_top = vh - inset;
            (f32::from_bits(bottom) + MARGIN - keyboard_top).clamp(0.0, inset)
        } else {
            0.0
        };
        let lift = lift.floor();
        LIFT.store(lift.to_bits(), Ordering::Relaxed);
        lift
    }

    pub(in crate::gui) fn current() -> f32 {
        f32::from_bits(LIFT.load(Ordering::Relaxed))
    }
}

#[cfg(feature = "mobile_ui")]
pub(in crate::gui) mod touch_tap {
    use bevy::math::Vec2;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    const NONE: u64 = u64::MAX;
    static PRESS: AtomicU64 = AtomicU64::new(NONE);
    static SCROLLED: AtomicBool = AtomicBool::new(false);

    pub(in crate::gui) fn note_press(at: Option<Vec2>) {
        let packed = match at {
            Some(p) => ((p.x.to_bits() as u64) << 32) | p.y.to_bits() as u64,
            None => NONE,
        };
        PRESS.store(packed, Ordering::Relaxed);
        SCROLLED.store(false, Ordering::Relaxed);
        super::SLIDER_PENDING.store(0, Ordering::Relaxed);
    }

    pub(in crate::gui) fn press_pos() -> Option<Vec2> {
        let packed = PRESS.load(Ordering::Relaxed);
        if packed == NONE {
            return None;
        }
        Some(Vec2::new(
            f32::from_bits((packed >> 32) as u32),
            f32::from_bits(packed as u32) + super::keyboard_lift::current(),
        ))
    }

    pub(in crate::gui) fn pressed_in(x: f32, y: f32, w: f32, h: f32) -> bool {
        let Some(at) = press_pos() else {
            return false;
        };
        at.x >= x && at.x < x + w && at.y >= y && at.y < y + h
    }

    pub(in crate::gui) fn mark_scrolled() {
        SCROLLED.store(true, Ordering::Relaxed);
    }

    pub(in crate::gui) fn scrolled() -> bool {
        SCROLLED.load(Ordering::Relaxed)
    }
}

#[cfg(feature = "mobile_ui")]
static SLIDER_PENDING: AtomicU32 = AtomicU32::new(0);

static SLIDER_DRAG: AtomicU32 = AtomicU32::new(0);

static SLIDER_DRAG_SCALE: AtomicU32 = AtomicU32::new(0);

static SLIDER_MOUSE_ACTIVE: AtomicU32 = AtomicU32::new(0);

pub(in crate::gui) fn slider_track_cursor(
    id: u32,
    scale: f32,
    hovered: bool,
    input: &GuiInput,
) -> bool {
    if input.left_release && SLIDER_DRAG.load(Ordering::Relaxed) == id {
        SLIDER_DRAG.store(0, Ordering::Relaxed);
        super::flush_pending_options();
    }
    if SLIDER_DRAG.load(Ordering::Relaxed) == id
        && SLIDER_DRAG_SCALE.load(Ordering::Relaxed) != scale.to_bits()
    {
        SLIDER_DRAG.store(0, Ordering::Relaxed);
        super::flush_pending_options();
        return false;
    }
    #[cfg(not(feature = "mobile_ui"))]
    if input.left_click && hovered {
        SLIDER_DRAG.store(id, Ordering::Relaxed);
        SLIDER_DRAG_SCALE.store(scale.to_bits(), Ordering::Relaxed);
        return true;
    }
    #[cfg(feature = "mobile_ui")]
    if touch_slider_claims(id, scale, hovered, input) {
        return true;
    }
    input.left_down && SLIDER_DRAG.load(Ordering::Relaxed) == id
}

#[cfg(feature = "mobile_ui")]
fn touch_slider_claims(id: u32, scale: f32, hovered: bool, input: &GuiInput) -> bool {
    const SLIDER_DRAG_THRESHOLD: f32 = 4.0;

    if input.left_click {
        if hovered {
            SLIDER_PENDING.store(id, Ordering::Relaxed);
        }
        return false;
    }
    if SLIDER_PENDING.load(Ordering::Relaxed) != id {
        return false;
    }
    let (Some(at), Some(from)) = (input.mouse, touch_tap::press_pos()) else {
        return false;
    };
    if touch_tap::scrolled() {
        SLIDER_PENDING.store(0, Ordering::Relaxed);
        return false;
    }
    if input.left_release {
        SLIDER_PENDING.store(0, Ordering::Relaxed);
        return hovered;
    }
    let moved = at - from;
    if moved.x.abs() < SLIDER_DRAG_THRESHOLD || moved.x.abs() <= moved.y.abs() {
        return false;
    }
    SLIDER_PENDING.store(0, Ordering::Relaxed);
    SLIDER_DRAG.store(id, Ordering::Relaxed);
    SLIDER_DRAG_SCALE.store(scale.to_bits(), Ordering::Relaxed);
    true
}

pub(in crate::gui) fn slider_dragging() -> bool {
    SLIDER_DRAG.load(Ordering::Relaxed) != 0
}

fn map(v: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    c + (d - c) * (v - a) / (b - a)
}

pub fn slider_fraction(value: i32, min: i32, max: i32) -> f32 {
    if max <= min {
        return 0.0;
    }
    if value <= min {
        return 0.0;
    }
    if value >= max {
        return 1.0;
    }
    map(value as f32 + 0.5, min as f32, max as f32 + 1.0, 0.0, 1.0)
}

pub fn slider_value_from_fraction(fraction: f32, min: i32, max: i32) -> i32 {
    let f = if fraction >= 1.0 {
        0.999_989_99
    } else {
        fraction.max(0.0)
    };
    let v = map(f, 0.0, 1.0, min as f32, max as f32 + 1.0).floor() as i32;
    v.clamp(min, max)
}

pub fn slider_value_from_mouse(mouse_x: f32, x: f32, w: f32, min: i32, max: i32) -> i32 {
    let travel = w - HANDLE_WIDTH;
    if travel <= 0.0 {
        return min;
    }
    let fraction = ((mouse_x - (x + HANDLE_WIDTH / 2.0)) / travel).clamp(0.0, 1.0);
    slider_value_from_fraction(fraction, min, max)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::gui) fn draw_slider(
    p: &mut Painter,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    frac: f32,
    track_lit: bool,
    handle_lit: bool,
) {
    let track = if track_lit {
        "widget/slider_highlighted"
    } else {
        "widget/slider"
    };
    p.sprite(track, x, y, w, h);
    let handle = if handle_lit {
        "widget/slider_handle_highlighted"
    } else {
        "widget/slider_handle"
    };
    p.sprite(
        handle,
        x + (frac * (w - HANDLE_WIDTH)).floor(),
        y,
        HANDLE_WIDTH,
        h,
    );
}

pub struct Slider<F: Fn(i32) -> String> {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub min: i32,
    pub max: i32,
    pub value: i32,
    pub label: F,
}

impl<F: Fn(i32) -> String> Slider<F> {
    pub fn draw(&self, p: &mut Painter, ctx: &ScreenCtx) -> i32 {
        let hovered = ctx.hovering(self.x, self.y, self.w, self.h);
        let focused = focus::next(true);
        let mut value = self.value.clamp(self.min, self.max);

        let track = slider_track_cursor(self.id, ctx.input.scale, hovered, ctx.input);
        if track && let Some(m) = ctx.mouse() {
            value = slider_value_from_mouse(m.x, self.x, self.w, self.min, self.max);
        }
        if track {
            #[cfg(not(feature = "mobile_ui"))]
            focus::claim();
            SLIDER_MOUSE_ACTIVE.store(self.id, Ordering::Relaxed);
        }
        if !focused && SLIDER_MOUSE_ACTIVE.load(Ordering::Relaxed) == self.id {
            SLIDER_MOUSE_ACTIVE.store(0, Ordering::Relaxed);
        }
        let dragging = SLIDER_DRAG.load(Ordering::Relaxed) == self.id;

        let track_lit = focused && SLIDER_MOUSE_ACTIVE.load(Ordering::Relaxed) != self.id;
        let frac = slider_fraction(value, self.min, self.max);
        draw_slider(
            p,
            self.x,
            self.y,
            self.w,
            self.h,
            frac,
            track_lit,
            hovered || dragging,
        );

        let label = (self.label)(value);
        draw_label_spans(
            p,
            &[Span::colored(label, LABEL_ACTIVE)],
            self.x,
            self.y,
            self.w,
            self.h,
        );
        value
    }

    pub fn draw_into(self, p: &mut Painter, ctx: &ScreenCtx, value: &mut u32) -> bool {
        let new = self.draw(p, ctx).clamp(self.min, self.max) as u32;
        let changed = new != *value;
        *value = new;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_pins_the_endpoints() {
        assert_eq!(slider_fraction(0, 0, 4), 0.0);
        assert_eq!(slider_fraction(4, 0, 4), 1.0);
        assert!((slider_fraction(2, 0, 4) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn fraction_round_trips_every_value() {
        for max in 1..=8 {
            for v in 0..=max {
                let f = slider_fraction(v, 0, max);
                assert_eq!(
                    slider_value_from_fraction(f, 0, max),
                    v,
                    "max {max} value {v}"
                );
            }
        }
    }

    #[test]
    fn mouse_maps_across_the_track() {
        let (x, w) = (100.0, 200.0);
        assert_eq!(slider_value_from_mouse(0.0, x, w, 0, 4), 0);
        assert_eq!(slider_value_from_mouse(x + 4.0, x, w, 0, 4), 0);
        assert_eq!(slider_value_from_mouse(x + w - 4.0, x, w, 0, 4), 4);
        assert_eq!(slider_value_from_mouse(x + w + 50.0, x, w, 0, 4), 4);
        assert_eq!(slider_value_from_mouse(x + 4.0 + 96.0, x, w, 0, 4), 2);
    }

    #[test]
    fn single_value_range_is_pinned() {
        assert_eq!(slider_fraction(0, 0, 0), 0.0);
        assert_eq!(slider_value_from_mouse(500.0, 0.0, 200.0, 0, 0), 0);
    }
}
