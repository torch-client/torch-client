use std::sync::Arc;

use crate::gui::ScreenCtx;
use crate::gui::painter::Painter;
use crate::gui::render::GuiInput;
use crate::gui::widgets::TextBox;
use crate::play::commands::{
    self, CommandInfo, Options, Source, Suggestion, SuggestionClient, Suggestions,
};
use crate::session::SharedMutex;
use crate::text::Font;

pub const SUGGESTION_LINE_HEIGHT: f32 = 12.0;
#[cfg(feature = "mobile_ui")]
const SUGGESTION_CHIP_PAD: f32 = 4.0;
const SUGGESTION_SELECTED: u32 = 0xFFFF00;
const SUGGESTION_UNSELECTED: u32 = 0xAAAAAA;
pub const GHOST_COLOR: u32 = 0x808080;

pub struct List {
    pub items: Vec<Suggestion>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    offset: usize,
    pub current: usize,
    original: String,
    tab_cycles: bool,
    last_mouse: Option<(f32, f32)>,
    cols: Vec<f32>,
}

#[derive(Default)]
pub struct State {
    pub info: CommandInfo,
    pending: Suggestions,
    last_value: String,
    last_cursor: usize,
    pub allow: bool,
    keep_once: bool,
    client: SuggestionClient,
    pub list: Option<List>,
    pub ghost: Option<String>,
    force: bool,
}

#[derive(Default)]
pub struct Consumed {
    pub scroll: bool,
    pub up_down: bool,
    pub click: bool,
}

const TOOLTIP_HEIGHT: f32 = 9.0 + 6.0;

#[derive(Clone, Copy)]
pub struct Style {
    pub limit: usize,
    pub line_start_offset: usize,
    pub anchor_to_bottom: bool,
    pub fill: u32,
    pub bordered: bool,
    pub options: Options,
}

#[derive(Clone, Copy)]
pub struct Anchor {
    pub text_x: f32,
    pub inner_width: f32,
    pub popup_y: f32,
    pub usage_y: f32,
}

#[derive(Default)]
pub struct Field {
    pub input: TextBox,
    pub suggest: State,
}

impl Field {
    pub fn new(input: TextBox) -> Field {
        Field {
            input,
            suggest: State::default(),
        }
    }

    pub fn invalidate(&mut self) {
        self.suggest.force = true;
    }

    pub fn hide(&mut self) -> bool {
        if self.suggest.list.is_none() {
            return false;
        }
        self.suggest.list = None;
        self.suggest.ghost = None;
        true
    }
}

impl Field {
    pub fn screen_x(&self, font: &Font, pos: usize, a: Anchor) -> f32 {
        if pos > self.input.text.chars().count() {
            return a.text_x;
        }
        let shown: String = self
            .input
            .text
            .chars()
            .skip(self.input.scroll)
            .take(pos.saturating_sub(self.input.scroll))
            .collect();
        a.text_x + font.width_str(&shown)
    }

    fn select(&mut self, index: i32) {
        let applied = {
            let Some(list) = self.suggest.list.as_mut() else {
                return;
            };
            let n = list.items.len() as i32;
            if n == 0 {
                return;
            }
            let mut current = index;
            if current < 0 {
                current += n;
            }
            if current >= n {
                current -= n;
            }
            list.current = current as usize;
            let original: Vec<char> = list.original.chars().collect();
            list.items[list.current].apply(&original)
        };
        self.suggest.ghost = applied
            .strip_prefix(self.input.text.as_str())
            .map(str::to_string);
    }

    pub fn cycle(&mut self, direction: i32, st: Style) {
        let Some(list) = self.suggest.list.as_ref() else {
            return;
        };
        let target = list.current as i32 + direction;
        self.select(target);
        let Some(list) = self.suggest.list.as_mut() else {
            return;
        };
        let max_offset = list.items.len().saturating_sub(st.limit);
        let first = list.offset;
        let last = list.offset + st.limit - 1;
        if list.current < first {
            list.offset = list.current.min(max_offset);
        } else if list.current > last {
            list.offset = (list.current + st.line_start_offset)
                .saturating_sub(st.limit)
                .min(max_offset);
        }
    }

    fn use_suggestion(&mut self) {
        let Some(list) = self.suggest.list.as_ref() else {
            return;
        };
        let suggestion = list.items[list.current].clone();
        let original: Vec<char> = list.original.chars().collect();
        let value = suggestion.apply(&original);
        let end = suggestion.range.start + suggestion.text.chars().count();
        let current = list.current as i32;

        self.input.set_text(&value);
        self.input.cursor = end.min(self.input.text.chars().count());
        self.input.anchor = self.input.cursor;
        self.input.push_keyboard();
        self.suggest.keep_once = true;
        self.select(current);
        if let Some(list) = self.suggest.list.as_mut() {
            list.tab_cycles = true;
        }
    }

    #[cfg(not(feature = "mobile_ui"))]
    pub fn show(&mut self, font: &Font, st: Style, a: Anchor) {
        if self.suggest.pending.is_empty() {
            return;
        }
        let items = sort_suggestions(&self.suggest.pending, &self.input.text, self.input.cursor);
        let max_width = items
            .iter()
            .fold(0.0f32, |w, s| w.max(font.width_str(&s.text)));
        let anchor = self.screen_x(font, self.suggest.pending.range.start, a);
        let right = (a.text_x + a.inner_width - max_width).max(0.0);
        let x = anchor.clamp(0.0, right);
        let rows = items.len().min(st.limit) as f32;
        self.suggest.list = Some(List {
            items,
            x: x - if st.bordered { 0.0 } else { 1.0 },
            y: if st.anchor_to_bottom {
                a.popup_y - 3.0 - rows * SUGGESTION_LINE_HEIGHT
            } else {
                a.popup_y - if st.bordered { 1.0 } else { 0.0 }
            },
            w: max_width + 1.0,
            h: rows * SUGGESTION_LINE_HEIGHT,
            offset: 0,
            current: 0,
            original: self.input.text.clone(),
            tab_cycles: false,
            last_mouse: None,
            cols: Vec::new(),
        });
        self.select(0);
    }

    #[cfg(feature = "mobile_ui")]
    pub fn show(&mut self, font: &Font, st: Style, a: Anchor) {
        if self.suggest.pending.is_empty() {
            return;
        }
        let all = sort_suggestions(&self.suggest.pending, &self.input.text, self.input.cursor);
        let inner_width = a.inner_width;
        let mut items = Vec::new();
        let mut cols = Vec::new();
        let mut x = 0.0f32;
        for item in all {
            let w = font.width_str(&item.text) + SUGGESTION_CHIP_PAD * 2.0;
            if !cols.is_empty() && x + w > inner_width {
                break;
            }
            cols.push(x);
            x += w;
            items.push(item);
        }
        self.suggest.list = Some(List {
            items,
            x: a.text_x - if st.bordered { 0.0 } else { 1.0 },
            y: if st.anchor_to_bottom {
                a.popup_y - SUGGESTION_LINE_HEIGHT - 4.0
            } else {
                a.popup_y
            },
            w: x,
            h: SUGGESTION_LINE_HEIGHT,
            offset: 0,
            current: 0,
            original: self.input.text.clone(),
            tab_cycles: false,
            last_mouse: None,
            cols,
        });
        self.select(0);
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        match &self.suggest.list {
            Some(l) => x >= l.x && x < l.x + l.w && y >= l.y && y < l.y + l.h,
            None => false,
        }
    }

    pub fn handle_input(
        &mut self,
        input: &GuiInput,
        font: &Font,
        st: Style,
        a: Anchor,
    ) -> Consumed {
        let mut consumed = Consumed::default();
        if self.suggest.list.is_none() {
            if input.tab {
                self.show(font, st, a);
            }
            return consumed;
        }

        if input.left_click
            && let Some(m) = input.mouse
            && self.contains(m.x, m.y)
        {
            #[cfg(not(feature = "mobile_ui"))]
            let hit = {
                let list = self.suggest.list.as_ref().unwrap();
                let row = ((m.y - list.y) / SUGGESTION_LINE_HEIGHT) as usize + list.offset;
                (row < list.items.len()).then_some(row)
            };
            #[cfg(feature = "mobile_ui")]
            let hit = {
                let list = self.suggest.list.as_ref().unwrap();
                let rel = m.x - list.x;
                list.cols
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|&(_, &col)| rel >= col)
                    .map(|(i, _)| i)
            };
            if let Some(hit) = hit {
                self.select(hit as i32);
                self.use_suggestion();
            }
            consumed.click = true;
            return consumed;
        }

        if input.scroll != 0.0
            && let Some(m) = input.mouse
            && self.contains(m.x, m.y)
        {
            let list = self.suggest.list.as_mut().unwrap();
            let max_offset = list.items.len().saturating_sub(st.limit) as i32;
            let step = input.scroll.clamp(-1.0, 1.0) as i32;
            list.offset = (list.offset as i32 - step).clamp(0, max_offset) as usize;
            consumed.scroll = true;
        }

        if input.up_arrow {
            self.cycle(-1, st);
            consumed.up_down = true;
        } else if input.down_arrow {
            self.cycle(1, st);
            consumed.up_down = true;
        }
        if consumed.up_down
            && let Some(list) = self.suggest.list.as_mut()
        {
            list.tab_cycles = false;
        }

        if input.tab {
            if self.suggest.list.as_ref().is_some_and(|l| l.tab_cycles) {
                self.cycle(if input.shift { -1 } else { 1 }, st);
            }
            self.use_suggestion();
        }
        consumed
    }

    pub fn refresh(&mut self, font: &Font, shared: &Arc<SharedMutex>, st: Style, a: Anchor) {
        let reply = shared.lock().unwrap().session.suggestion_reply.take();
        let accepted = match reply {
            Some((id, suggestions)) => self.suggest.client.accept(id, suggestions),
            None => false,
        };

        let value = self.input.text.clone();
        let cursor = self.input.cursor;
        let changed = std::mem::take(&mut self.suggest.force)
            || value != self.suggest.last_value
            || cursor != self.suggest.last_cursor;
        if !changed && !accepted {
            return;
        }

        let (tree, source) = {
            let s = shared.lock().unwrap();
            (
                s.session.command_tree.clone(),
                Source {
                    player_names: s.session.player_names.clone(),
                    custom_completions: s.session.custom_completions.clone(),
                },
            )
        };
        let keep = std::mem::take(&mut self.suggest.keep_once);
        if changed && !keep {
            self.suggest.list = None;
            self.suggest.ghost = None;
        }

        self.suggest.info =
            commands::update_command_info(tree.as_deref(), &value, cursor, &source, st.options);

        if let Some(request) = self.suggest.info.ask_server.clone()
            && let Some(packet) = self.suggest.client.request(&request)
        {
            shared.lock().unwrap().session.suggestion_request = Some(packet);
        }

        let input_chars: Vec<char> = value.chars().collect();
        let mut parts = vec![self.suggest.info.suggestions.clone()];
        if let Some(request) = &self.suggest.info.ask_server
            && let Some(answer) = self.suggest.client.reply_for(request)
        {
            parts.push(answer.clone());
        }
        self.suggest.pending = Suggestions::merge(&input_chars, parts);

        self.suggest.last_value = value;
        self.suggest.last_cursor = cursor;
        if keep {
            let current = self.suggest.list.as_ref().map_or(0, |l| l.current as i32);
            self.select(current);
        } else if self.suggest.allow {
            self.show(font, st, a);
        }
    }
}

fn sort_suggestions(pending: &Suggestions, value: &str, cursor: usize) -> Vec<Suggestion> {
    let typed: Vec<char> = value.chars().take(cursor).collect();
    let start = last_word_start(&typed);
    let last_word: String = typed[start..].iter().collect::<String>().to_lowercase();
    let namespaced = format!("minecraft:{last_word}");
    let (mut head, tail): (Vec<Suggestion>, Vec<Suggestion>) = pending
        .list
        .iter()
        .cloned()
        .partition(|s| s.text.starts_with(&last_word) || s.text.starts_with(&namespaced));
    head.extend(tail);
    head
}

fn last_word_start(text: &[char]) -> usize {
    let mut result = 0usize;
    let mut i = 0usize;
    while i < text.len() {
        if text[i].is_whitespace() {
            while i < text.len() && text[i].is_whitespace() {
                i += 1;
            }
            result = i;
        } else {
            i += 1;
        }
    }
    result
}

pub fn draw(p: &mut Painter, f: &mut Field, ctx: &ScreenCtx, st: Style, a: Anchor) {
    if f.suggest.list.is_some() {
        draw_list(p, f, ctx, st);
        return;
    }
    draw_usage(p, f, st, a);
}

fn draw_list(p: &mut Painter, f: &mut Field, ctx: &ScreenCtx, st: Style) {
    #[cfg(feature = "mobile_ui")]
    draw_row(p, f, ctx, st);
    #[cfg(not(feature = "mobile_ui"))]
    draw_column(p, f, ctx, st);
}

#[cfg(feature = "mobile_ui")]
fn draw_row(p: &mut Painter, f: &mut Field, ctx: &ScreenCtx, st: Style) {
    let mouse = ctx.input.mouse.map(|m| (m.x, m.y));
    let (rect_x, rect_y, rect_h, current, cols) = {
        let list = f.suggest.list.as_ref().unwrap();
        (list.x, list.y, list.h, list.current, list.cols.clone())
    };

    let moved = f
        .suggest
        .list
        .as_ref()
        .is_some_and(|l| l.last_mouse != mouse);
    if let Some(list) = f.suggest.list.as_mut() {
        list.last_mouse = mouse;
    }

    let mut hovered: Option<usize> = None;
    if let Some((mx, my)) = mouse
        && my > rect_y
        && my < rect_y + rect_h
    {
        let rel = mx - rect_x;
        hovered = cols
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, &col)| rel >= col)
            .map(|(i, _)| i);
    }
    if moved && let Some(i) = hovered {
        f.select(i as i32);
    }
    let current = f.suggest.list.as_ref().map_or(current, |l| l.current);

    let list = f.suggest.list.as_ref().unwrap();
    for (i, item) in list.items.iter().enumerate() {
        let x = rect_x + cols[i];
        let w = p.atlas.font.width_str(&item.text) + SUGGESTION_CHIP_PAD * 2.0;
        p.fill(x, rect_y, w, rect_h, st.fill);
        let color = if i == current {
            SUGGESTION_SELECTED
        } else {
            SUGGESTION_UNSELECTED
        };
        p.text_str(
            &item.text,
            x + SUGGESTION_CHIP_PAD,
            rect_y + 2.0,
            color,
            true,
        );
    }

    if let Some(i) = hovered
        && let Some(tooltip) = list.items[i].tooltip.clone()
        && let Some((mx, my)) = mouse
    {
        draw_tooltip(p, &tooltip, mx, my, ctx);
    }
}

#[cfg(not(feature = "mobile_ui"))]
fn draw_column(p: &mut Painter, f: &mut Field, ctx: &ScreenCtx, st: Style) {
    let mouse = ctx.input.mouse.map(|m| (m.x, m.y));
    let (rect_x, rect_y, rect_w, rect_h, offset, current, limit, total) = {
        let list = f.suggest.list.as_ref().unwrap();
        let limit = list.items.len().min(st.limit);
        (
            list.x,
            list.y,
            list.w,
            list.h,
            list.offset,
            list.current,
            limit,
            list.items.len(),
        )
    };

    let has_previous = offset > 0;
    let has_next = total > offset + limit;
    if has_previous || has_next {
        p.fill(rect_x, rect_y - 1.0, rect_w, 1.0, st.fill);
        p.fill(rect_x, rect_y + rect_h, rect_w, 1.0, st.fill);
        let mut column = 0.0;
        while column < rect_w {
            if has_previous {
                p.fill(rect_x + column, rect_y - 1.0, 1.0, 1.0, 0xFFFF_FFFF);
            }
            if has_next {
                p.fill(rect_x + column, rect_y + rect_h, 1.0, 1.0, 0xFFFF_FFFF);
            }
            column += 2.0;
        }
    }

    let moved = f
        .suggest
        .list
        .as_ref()
        .is_some_and(|l| l.last_mouse != mouse);
    if let Some(list) = f.suggest.list.as_mut() {
        list.last_mouse = mouse;
    }
    let mut hovered_row: Option<usize> = None;
    if let Some((mx, my)) = mouse
        && mx > rect_x
        && mx < rect_x + rect_w
        && my > rect_y
        && my < rect_y + rect_h
    {
        hovered_row = Some(((my - rect_y) / SUGGESTION_LINE_HEIGHT) as usize + offset);
    }
    if moved
        && let Some(row) = hovered_row
        && row < total
    {
        f.select(row as i32);
    }
    let current = f.suggest.list.as_ref().map_or(current, |l| l.current);

    let list = f.suggest.list.as_ref().unwrap();
    for i in 0..limit {
        let index = i + offset;
        let row_y = rect_y + SUGGESTION_LINE_HEIGHT * i as f32;
        p.fill(rect_x, row_y, rect_w, SUGGESTION_LINE_HEIGHT, st.fill);
        let color = if index == current {
            SUGGESTION_SELECTED
        } else {
            SUGGESTION_UNSELECTED
        };
        p.text_str(
            &list.items[index].text,
            rect_x + 1.0,
            row_y + 2.0,
            color,
            true,
        );
    }

    if let Some(row) = hovered_row
        && row < total
        && let Some(tooltip) = list.items[current].tooltip.clone()
        && let Some((mx, my)) = mouse
    {
        draw_tooltip(p, &tooltip, mx, my, ctx);
    }
}

fn draw_tooltip(p: &mut Painter, text: &str, mx: f32, my: f32, ctx: &ScreenCtx) {
    let w = p.atlas.font.width_str(text);
    let x = (mx + 12.0).min(ctx.vw - w - 6.0).max(0.0);
    let y = (my - 12.0).max(0.0);
    p.fill(x - 3.0, y - 3.0, w + 6.0, TOOLTIP_HEIGHT, 0xF0100010);
    p.text_str(text, x, y, 0xFFFFFF, true);
}

fn draw_usage(p: &mut Painter, f: &Field, st: Style, a: Anchor) {
    let usage = &f.suggest.info.usage;
    if usage.is_empty() {
        return;
    }
    let width = usage
        .iter()
        .fold(0.0f32, |w, (line, _)| w.max(p.atlas.font.width_str(line)));
    let anchor = f.screen_x(&p.atlas.font, f.suggest.info.usage_start, a);
    let right = (a.text_x + a.inner_width - width).max(0.0);
    let x = anchor.clamp(0.0, right);
    for (i, (line, is_error)) in usage.iter().enumerate() {
        let y = if st.anchor_to_bottom {
            a.usage_y - SUGGESTION_LINE_HEIGHT * i as f32
        } else {
            a.usage_y + SUGGESTION_LINE_HEIGHT * i as f32
        };
        p.fill(x - 1.0, y, width + 2.0, SUGGESTION_LINE_HEIGHT, st.fill);
        let color = if *is_error { 0xFFFFFF } else { 0xAAAAAA };
        p.text_str(line, x, y + 2.0, color, true);
    }
}
