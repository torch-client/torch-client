use std::sync::atomic::{AtomicU32, Ordering};

use crate::gui::ScreenCtx;
use crate::gui::focus;
use crate::gui::painter::Painter;
use crate::gui::render::{EditKey, GuiInput, clipboard_get, clipboard_set};
use crate::text::{LINE_HEIGHT, Span, Style};

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
}

const BORDER_INSET: f32 = 4.0;

type Limit<'a> = Option<(&'a crate::text::Font, f32)>;

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

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.anchor = 0;
        self.scroll = 0;
    }

    pub fn set_text(&mut self, s: &str) {
        self.text = s
            .chars()
            .filter(|c| allowed_char(*c))
            .take(self.max_len)
            .collect();
        self.cursor = self.char_count();
        self.anchor = self.cursor;
        self.scroll = 0;
    }

    fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_index(&self, char_index: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_index)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    fn selection_range(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    pub fn selected_text(&self) -> String {
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
        let at = self.byte_index(self.cursor);
        self.text.insert(at, c);
        self.cursor += 1;
        self.anchor = self.cursor;
        true
    }

    pub fn insert_text(&mut self, s: &str) {
        self.insert(s, None);
    }

    fn insert_char(&mut self, c: char, limit: Limit<'_>) -> bool {
        let mut buf = [0u8; 4];
        self.insert(c.encode_utf8(&mut buf), limit)
    }

    fn insert(&mut self, s: &str, limit: Limit<'_>) -> bool {
        let filtered: String = s.chars().filter(|c| allowed_char(*c)).collect();
        if filtered.is_empty() && !s.is_empty() {
            return false;
        }
        let (lo, hi) = self.selection_range();
        let room = self.max_len - (self.char_count() - (hi - lo)).min(self.max_len);
        let add: String = filtered.chars().take(room).collect();
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
        let at = self.byte_index(self.cursor);
        self.text.insert_str(at, &add);
        self.cursor += add.chars().count();
        self.anchor = self.cursor;
        true
    }

    fn move_cursor_to(&mut self, pos: usize, extend: bool) {
        self.cursor = pos.min(self.char_count());
        if !extend {
            self.anchor = self.cursor;
        }
    }

    fn word_position_from(&self, dir: i32, from: usize) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let len = chars.len();
        let mut result = from.min(len);
        if dir > 0 {
            match chars[result..].iter().position(|&c| c == ' ') {
                Some(off) => {
                    result += off;
                    while result < len && chars[result] == ' ' {
                        result += 1;
                    }
                }
                None => result = len,
            }
        } else {
            while result > 0 && chars[result - 1] == ' ' {
                result -= 1;
            }
            while result > 0 && chars[result - 1] != ' ' {
                result -= 1;
            }
        }
        result
    }

    fn word_position(&self, dir: i32) -> usize {
        self.word_position_from(dir, self.cursor)
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    pub fn sync_keyboard(&mut self) -> bool {
        self.sync_keyboard_inner(None)
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    fn sync_keyboard_inner(&mut self, _limit: Limit<'_>) -> bool {
        #[allow(unused_mut)]
        let mut changed = false;
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
            if let Some((font, max)) = _limit
                && font.width_str(&clamped) > max
            {
                clamped = self.text.clone();
            }
            if clamped != self.text {
                self.text = clamped.clone();
                changed = true;
            }
            let len = self.char_count();
            self.cursor = cursor.min(len);
            self.anchor = anchor.min(len);
            if clamped != text {
                crate::platform::keyboard::sync_selection(&clamped, self.anchor, self.cursor);
            }
        }
        changed
    }

    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    pub fn sync_keyboard(&mut self) -> bool {
        false
    }

    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    fn sync_keyboard_inner(&mut self, _limit: Limit<'_>) -> bool {
        false
    }

    pub fn push_keyboard(&self) {
        #[cfg(target_arch = "wasm32")]
        if self.keyboard_active {
            crate::platform::keyboard::sync_selection(&self.text, self.anchor, self.cursor);
        }
    }

    pub fn handle_input(&mut self, input: &GuiInput) -> bool {
        self.handle_input_inner(input, None)
    }

    pub fn handle_edits_validated(
        &mut self,
        input: &GuiInput,
        font: &crate::text::Font,
        mut intercept: impl FnMut(&mut TextBox, EditKey) -> bool,
    ) -> bool {
        let limit = self.max_width.map(|w| (font, w));
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

    fn handle_input_inner(&mut self, input: &GuiInput, limit: Limit<'_>) -> bool {
        let (mut changed, live) = self.frame_input(input, limit);
        if !live {
            return changed;
        }
        if let Some(edited) = self.apply_shortcut(input, limit) {
            return changed | edited;
        }

        for c in &input.typed {
            changed |= self.apply_edit(EditKey::Char(*c), input, limit);
        }

        let browser_backspace = self.take_browser_backspace();
        if input.backspace || browser_backspace {
            changed |= self.apply_edit(EditKey::Backspace, input, limit);
            if browser_backspace {
                self.push_browser_backspace();
            }
        }
        if input.delete {
            changed |= self.apply_edit(EditKey::Delete, input, limit);
        }
        for (down, key) in [
            (input.left_arrow, EditKey::Left),
            (input.right_arrow, EditKey::Right),
            (input.home, EditKey::Home),
            (input.end, EditKey::End),
        ] {
            if down {
                self.apply_edit(key, input, limit);
            }
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
            self.anchor = 0;
            self.cursor = self.char_count();
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
                strip_formatting(&text)
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
                        let from = self.byte_index(self.cursor - 1);
                        let to = self.byte_index(self.cursor);
                        self.text.replace_range(from..to, "");
                        self.cursor -= 1;
                        self.anchor = self.cursor;
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
                        let from = self.byte_index(self.cursor);
                        let to = self.byte_index(self.cursor + 1);
                        self.text.replace_range(from..to, "");
                    }
                }
                true
            }
            EditKey::Left => {
                if input.ctrl {
                    let pos = self.word_position(-1);
                    self.move_cursor_to(pos, input.shift);
                } else if self.cursor > 0 {
                    self.cursor -= 1;
                    if !input.shift {
                        self.anchor = self.cursor;
                    }
                }
                false
            }
            EditKey::Right => {
                if input.ctrl {
                    let pos = self.word_position(1);
                    self.move_cursor_to(pos, input.shift);
                } else if self.cursor < self.char_count() {
                    self.cursor += 1;
                    if !input.shift {
                        self.anchor = self.cursor;
                    }
                }
                false
            }
            EditKey::Home => {
                self.cursor = 0;
                if !input.shift {
                    self.anchor = 0;
                }
                false
            }
            EditKey::End => {
                self.cursor = self.char_count();
                if !input.shift {
                    self.anchor = self.cursor;
                }
                false
            }
            EditKey::Up | EditKey::Down | EditKey::Enter => false,
        }
    }

    fn text_origin(&self, x: f32, y: f32, h: f32) -> (f32, f32) {
        let inset = if self.bordered { BORDER_INSET } else { 0.0 };
        (x + inset, y + (h - 8.0) / 2.0)
    }

    fn text_width(&self, w: f32) -> f32 {
        if self.bordered {
            (w - 2.0 * BORDER_INSET).max(0.0)
        } else {
            w
        }
    }

    pub fn handle_mouse(
        &mut self,
        input: &GuiInput,
        font: &crate::text::Font,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let was = (self.cursor, self.anchor);

        if input.left_release {
            self.dragging = false;
        }

        let (tx, _ty) = self.text_origin(x, y, h);

        let tw = self.text_width(w);
        let clicked_pos = |mx: f32| -> usize {
            let offset = (mx - tx).clamp(0.0, tw);
            let visible: Vec<char> = self.text.chars().skip(self.scroll).collect();
            let mut acc = 0.0;
            let mut n = 0;
            for c in &visible {
                let a = font.advance(*c, false);
                if acc + a > offset {
                    break;
                }
                acc += a;
                n += 1;
            }
            self.scroll + n
        };

        if input.left_click {
            if let Some(m) = input.mouse {
                let inside = m.x >= x && m.x <= x + w && m.y >= y && m.y <= y + h;
                if inside {
                    self.focused = true;
                    #[cfg(target_os = "android")]
                    {
                        self.keyboard_active = false;
                    }
                    crate::platform::keyboard::note_tap(true);
                    let pos = clicked_pos(m.x);
                    if input.triple_click {
                        self.anchor = 0;
                        self.cursor = self.char_count();
                    } else if input.double_click {
                        let start = self.word_position_from(-1, pos);
                        let end = self.word_position_from(1, pos);
                        self.move_cursor_to(start, false);
                        self.move_cursor_to(end, true);
                    } else {
                        self.move_cursor_to(pos, input.shift);
                    }
                    self.dragging = true;
                } else if self.focused {
                    crate::platform::keyboard::note_tap(false);
                }
            }
        } else if self.dragging && input.left_down {
            if let Some(m) = input.mouse {
                if m.x < tx && self.cursor > 0 {
                    self.scroll = self.scroll.saturating_sub(1);
                    let pos = self.cursor - 1;
                    self.move_cursor_to(pos, true);
                } else if m.x > tx + tw {
                    let pos = (clicked_pos(tx + tw) + 1).min(self.char_count());
                    self.move_cursor_to(pos, true);
                } else {
                    let pos = clicked_pos(m.x);
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
        let frame = p.frame;

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
        self.draw(p, x, y, w, h, frame);
    }

    pub fn draw(&mut self, p: &mut Painter, x: f32, y: f32, w: f32, h: f32, frame: u32) {
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
        let x = tx;

        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        }
        loop {
            let visible: String = self.text.chars().skip(self.scroll).collect();
            let caret_off = self.cursor.saturating_sub(self.scroll);
            let upto: String = visible.chars().take(caret_off).collect();
            if font.width_str(&upto) <= w || self.scroll >= self.cursor {
                break;
            }
            self.scroll += 1;
        }

        let visible: String = self.text.chars().skip(self.scroll).collect();
        let shown = trim_to_width(font, &visible, w);

        let shadow = !self.no_shadow;
        if self.text.is_empty() && !self.focused {
            p.text(
                &[Span {
                    text: self.placeholder.clone(),
                    style: Style::colored(0x707070),
                }],
                tx,
                text_y,
                shadow,
            );
        } else {
            p.text(
                &[Span {
                    text: shown.clone(),
                    style: Style::colored(self.color),
                }],
                tx,
                text_y,
                shadow,
            );
            let (sa, sb) = self.selection_range();
            if sa != sb {
                let a = sa.max(self.scroll) - self.scroll;
                let b = sb.max(self.scroll) - self.scroll;
                let pre: String = shown.chars().take(a).collect();
                let mid: String = shown.chars().skip(a).take(b - a).collect();
                let x0 = tx + font.width_str(&pre);
                let x1 = (x0 + font.width_str(&mid)).min(x + w);
                draw_selection(p, &mid, x0, x1, text_y, text_y - 1.0, text_y + 10.0);
            }
        }

        if let Some(suggestion) = &self.suggestion
            && self.cursor >= self.char_count()
            && self.char_count() < self.max_len
        {
            let x = tx + font.width_str(&shown);
            p.text(
                &[Span {
                    text: suggestion.clone(),
                    style: Style::colored(0x808080),
                }],
                x,
                text_y,
                shadow,
            );
        }

        if self.focused && (frame / 6) % 2 == 0 {
            let pre: String = shown
                .chars()
                .take(self.cursor.saturating_sub(self.scroll))
                .collect();
            let cx = tx + font.width_str(&pre);
            if self.cursor >= self.char_count() {
                draw_append_caret(p, cx, text_y, self.color, shadow);
            } else {
                draw_insert_caret(p, cx - 1.0, text_y, 10.0, self.color);
            }
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

    pub fn text(&self) -> &str {
        &self.inner.text
    }

    pub fn set_text(&mut self, s: &str) {
        self.inner.text = s.chars().take(self.inner.max_len).collect();
        self.inner.cursor = self.inner.text.chars().count();
        self.inner.anchor = self.inner.cursor;
        self.stale = true;
    }

    fn relayout(&mut self, font: &crate::text::Font, width: f32) {
        if !self.stale && self.width == width {
            return;
        }
        font.wrap_ranges(&self.inner.text, width, &mut self.lines);
        self.width = width;
        self.stale = false;
    }

    pub fn line_count(&mut self, font: &crate::text::Font, width: f32) -> usize {
        self.relayout(font, width);
        self.lines.len()
    }

    fn line_at(&self, cursor: usize) -> usize {
        self.lines
            .iter()
            .rposition(|r| r.start <= cursor)
            .unwrap_or(0)
    }

    pub fn handle_input(&mut self, input: &GuiInput, font: &crate::text::Font, width: f32) -> bool {
        self.relayout(font, width);
        if !self.inner.focused {
            return self.inner.handle_input(input);
        }

        let mutating = input.enter
            || input.backspace
            || input.delete
            || input.paste
            || input.cut
            || !input.typed.is_empty();
        let undo = mutating.then(|| {
            (
                self.inner.text.clone(),
                self.inner.cursor,
                self.inner.anchor,
            )
        });

        let mut changed = if input.enter {
            self.inner.insert_raw('\n')
        } else {
            false
        };
        changed |= self.inner.handle_input(input);

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

        let line = self.line_at(self.inner.cursor);
        let column = self.inner.cursor - self.lines[line].start;
        let goto = |this: &mut Self, pos: usize| {
            this.inner.cursor = pos;
            if !input.shift {
                this.inner.anchor = pos;
            }
        };
        if input.up_arrow && line > 0 {
            let target = &self.lines[line - 1];
            let pos = (target.start + column).min(target.end);
            goto(self, pos);
        }
        if input.down_arrow && line + 1 < self.lines.len() {
            let target = &self.lines[line + 1];
            let pos = (target.start + column).min(target.end);
            goto(self, pos);
        }
        if input.home {
            let pos = self.lines[line].start;
            goto(self, pos);
        }
        if input.end {
            let pos = self.lines[line].end;
            goto(self, pos);
        }
        changed
    }

    pub fn handle_mouse(
        &mut self,
        input: &GuiInput,
        font: &crate::text::Font,
        x: f32,
        y: f32,
        width: f32,
    ) {
        if !input.left_click {
            return;
        }
        let Some(m) = input.mouse else { return };
        let page_rows = if self.line_limit > 0 {
            self.line_limit
        } else {
            self.lines.len() + 1
        };
        let on_page = m.x >= x
            && m.x <= x + width
            && m.y >= y
            && m.y <= y + page_rows as f32 * crate::text::LINE_HEIGHT;
        if on_page {
            crate::platform::keyboard::note_tap(true);
        } else if self.inner.focused {
            crate::platform::keyboard::note_tap(false);
        }
        self.relayout(font, width);
        let rows = self.lines.len() as f32;
        if m.x < x || m.x > x + width || m.y < y || m.y > y + rows * crate::text::LINE_HEIGHT {
            return;
        }
        let row = (((m.y - y) / crate::text::LINE_HEIGHT) as usize).min(self.lines.len() - 1);
        let range = self.lines[row].clone();
        let mut pos = range.start;
        let mut w = 0.0f32;
        for c in self.inner.text.chars().skip(range.start).take(range.len()) {
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

    pub fn draw(&mut self, p: &mut Painter, x: f32, y: f32, width: f32, color: u32, frame: u32) {
        self.relayout(&p.atlas.font, width);
        let (sa, sb) = (
            self.inner.cursor.min(self.inner.anchor),
            self.inner.cursor.max(self.inner.anchor),
        );
        for (row, range) in self.lines.iter().enumerate() {
            let ly = y + row as f32 * crate::text::LINE_HEIGHT;
            let line: String = self
                .inner
                .text
                .chars()
                .skip(range.start)
                .take(range.len())
                .collect();
            p.text_str(&line, x, ly, color, false);
            let (a, b) = (
                sa.clamp(range.start, range.end),
                sb.clamp(range.start, range.end),
            );
            if a != b {
                let pre: String = line.chars().take(a - range.start).collect();
                let mid: String = line.chars().skip(a - range.start).take(b - a).collect();
                let x0 = x + p.atlas.font.width_str(&pre);
                let x1 = x0 + p.atlas.font.width_str(&mid);
                draw_selection(p, &mid, x0, x1, ly, ly - 1.0, ly + 10.0);
            }
        }

        if self.inner.focused && (frame / 6) % 2 == 0 {
            let row = self.line_at(self.inner.cursor);
            let range = self.lines[row].clone();
            let pre: String = self
                .inner
                .text
                .chars()
                .skip(range.start)
                .take(self.inner.cursor - range.start)
                .collect();
            let cx = x + p.atlas.font.width_str(&pre);
            let cy = y + row as f32 * crate::text::LINE_HEIGHT;
            if self.inner.cursor >= self.inner.text.chars().count() {
                draw_append_caret(p, cx, cy, color, false);
            } else {
                draw_insert_caret(p, cx - 1.0, cy, 10.0, color);
            }
        }
    }
}

pub(crate) fn draw_append_caret(p: &mut Painter, x: f32, y: f32, color: u32, shadow: bool) {
    p.text_str("_", x, y, color, shadow);
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
    p.text_str(text, x0, text_y, 0x000000, false);
}

pub(crate) fn allowed_char(c: char) -> bool {
    c != '\u{a7}' && (c as u32) >= 32 && c != '\u{7f}'
}

fn strip_formatting(s: &str) -> String {
    let without_cr: Vec<char> = s.chars().filter(|&c| c != '\r').collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < without_cr.len() {
        let c = without_cr[i];
        let code = without_cr.get(i + 1).is_some_and(|n| {
            n.is_ascii_hexdigit() || matches!(n.to_ascii_lowercase(), 'k'..='o' | 'r')
        });
        if c == '\u{a7}' && code {
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn trim_to_width(font: &crate::text::Font, s: &str, max: f32) -> String {
    let mut out = String::new();
    let mut w = 0.0;
    for c in s.chars() {
        let a = font.advance(c, false);
        if w + a > max {
            break;
        }
        w += a;
        out.push(c);
    }
    out
}

#[cfg(feature = "audio")]
fn play_click() {
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

    pub const fn default_heights() -> HeaderFooter {
        HeaderFooter::new(HeaderFooter::DEFAULT_H, HeaderFooter::DEFAULT_H)
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

pub fn draw_title(p: &mut Painter, vw: f32, y: f32, title: &str) {
    let w = p.atlas.font.width_str(title);
    p.text_str(title, ((vw - w) / 2.0).floor(), y, 0xFFFFFF, true);
}

pub const WIDGET_HEIGHT: f32 = 20.0;

pub const WIDGET_WIDTH_BIG: f32 = 200.0;

const TEXT_MARGIN: f32 = 2.0;

const LABEL_ACTIVE: u32 = 0xFFFFFF;

const LABEL_INACTIVE: u32 = 0xA0A0A0;

const HANDLE_WIDTH: f32 = 8.0;

fn draw_label(p: &mut Painter, label: &str, x: f32, y: f32, w: f32, h: f32, color: u32) {
    let spans = [Span {
        text: label.to_string(),
        style: Style::colored(color),
    }];
    draw_label_spans(p, &spans, x, y, w, h);
}

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
    let focused = focus::next(active);
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
    let clicked = active && hovered && ctx.input.left_click;
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
        let hovered = ctx.hovering(self.x, self.y, self.w, self.h);
        let focused = focus::next(self.active);
        let sprite = match (self.active, hovered || focused) {
            (false, _) => "widget/button_disabled",
            (true, false) => "widget/button",
            (true, true) => "widget/button_highlighted",
        };
        p.sprite(sprite, self.x, self.y, self.w, self.h);
        let color = if self.active {
            LABEL_ACTIVE
        } else {
            LABEL_INACTIVE
        };
        draw_label(p, self.label, self.x, self.y, self.w, self.h, color);
        #[cfg(not(feature = "mobile_ui"))]
        let clicked = self.active && hovered && ctx.input.left_click;
        #[cfg(feature = "mobile_ui")]
        let clicked = self.active
            && hovered
            && ctx.input.left_release
            && touch_tap::pressed_in(self.x, self.y, self.w, self.h)
            && !touch_tap::scrolled();
        if clicked {
            focus::claim();
        }
        let pressed = clicked || (self.active && focused && ctx.input.select);
        #[cfg(feature = "audio")]
        if pressed {
            play_click();
        }
        pressed
    }
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
    }
    if SLIDER_DRAG.load(Ordering::Relaxed) == id
        && SLIDER_DRAG_SCALE.load(Ordering::Relaxed) != scale.to_bits()
    {
        SLIDER_DRAG.store(0, Ordering::Relaxed);
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
            focus::claim();
            SLIDER_MOUSE_ACTIVE.store(self.id, Ordering::Relaxed);
        }
        if !focused && SLIDER_MOUSE_ACTIVE.load(Ordering::Relaxed) == self.id {
            SLIDER_MOUSE_ACTIVE.store(0, Ordering::Relaxed);
        }
        let dragging = SLIDER_DRAG.load(Ordering::Relaxed) == self.id;

        let sprite = if focused && SLIDER_MOUSE_ACTIVE.load(Ordering::Relaxed) != self.id {
            "widget/slider_highlighted"
        } else {
            "widget/slider"
        };
        p.sprite(sprite, self.x, self.y, self.w, self.h);
        let handle = if hovered || dragging {
            "widget/slider_handle_highlighted"
        } else {
            "widget/slider_handle"
        };
        let travel = (slider_fraction(value, self.min, self.max) * (self.w - HANDLE_WIDTH)).floor();
        p.sprite(handle, self.x + travel, self.y, HANDLE_WIDTH, self.h);

        let label = (self.label)(value);
        draw_label(p, &label, self.x, self.y, self.w, self.h, LABEL_ACTIVE);
        value
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
