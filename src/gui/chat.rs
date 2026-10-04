use std::collections::VecDeque;
use std::sync::Arc;

use crate::gui::ScreenCtx;
use crate::gui::clickevent::{self, Followup};
use crate::gui::confirmlink;
use crate::gui::painter::Painter;
use crate::gui::render::{GuiInput, clipboard_set};
use crate::gui::suggestions::{self, GHOST_COLOR};
use crate::gui::widgets::TextBox;
use crate::session::{ChatEntry, ChatTag, SharedMutex};
use crate::text::events::{self, Events, Hover, SpanEvent};
use crate::text::{Font, Span, Style};

#[cfg(not(feature = "mobile_ui"))]
const CHAT_WIDTH: f32 = 320.0;
#[cfg(feature = "mobile_ui")]
const CHAT_WIDTH: f32 = 240.0;
const LINE_HEIGHT: f32 = 9.0;
#[cfg_attr(feature = "mobile_ui", allow(dead_code))]
const BOTTOM_MARGIN: f32 = 40.0;
#[cfg(feature = "mobile_ui")]
const MOBILE_BOTTOM_MARGIN: f32 = 14.0 + suggestions::SUGGESTION_LINE_HEIGHT + 6.0;
#[cfg(feature = "mobile_ui")]
const MOBILE_TOP_GAP: f32 = 2.0 * LINE_HEIGHT;
const MESSAGE_INDENT: f32 = 4.0;
const TEXT_RISE: f32 = 8.0;
#[cfg_attr(feature = "mobile_ui", allow(dead_code))]
const FOCUSED_LINES: usize = 20;
#[cfg_attr(feature = "mobile_ui", allow(dead_code))]
const UNFOCUSED_LINES: usize = 10;
#[cfg(feature = "mobile_ui")]
const MOBILE_FOCUSED_LINES: usize = 8;
#[cfg(feature = "mobile_ui")]
const MOBILE_UNFOCUSED_LINES: usize = 5;
#[cfg(feature = "mobile_ui")]
const TEXTING_TOP_GAP: f32 = 4.0;
#[cfg(feature = "mobile_ui")]
const MOBILE_FLASH_FRAMES: u32 = 60;
const MAX_HISTORY: usize = 100;
const SCROLL_LINES: i32 = 7;
const MAX_MESSAGE_LEN: usize = 256;
const TAG_ICON_SIZE: f32 = 9.0;
const TAG_ICON_RESERVE: f32 = TAG_ICON_SIZE + 4.0 + 2.0;

const BG_ALPHA: f32 = 0.5;
const TEXT_OPACITY: f32 = 1.0;
const INPUT_BG: u32 = 0x8000_0000;
const SELECTION_BG: u32 = 0xFFFF_FFFF;
const SECURE_INDICATOR: u32 = 0x55FF55;

const MODIFIED_INDICATOR: u32 = 0xFFAA00;

const INPUT_TEXT_X: f32 = 4.0;

const USAGE_OFFSET_FROM_BOTTOM: f32 = 27.0;

const SUGGEST: suggestions::Style = suggestions::Style {
    limit: 10,
    line_start_offset: 1,
    anchor_to_bottom: true,
    fill: 0xD000_0000,
    bordered: false,
    options: crate::play::commands::Options::CHAT,
};

fn suggest_anchor(ctx: &ScreenCtx) -> suggestions::Anchor {
    suggestions::Anchor {
        text_x: INPUT_TEXT_X,
        inner_width: ctx.vw - 8.0,
        popup_y: chat_base(ctx) - 12.0,
        usage_y: chat_base(ctx) - USAGE_OFFSET_FROM_BOTTOM,
    }
}

#[derive(Clone)]
struct Message {
    spans: Vec<Span>,
    events: Events,
    tag: ChatTag,
    original: Option<Arc<str>>,
    added_tick: u32,
}

struct Line {
    id: u64,
    spans: Vec<Span>,
    events: Events,
    tag: ChatTag,
    original: Option<Arc<str>>,
    added_tick: u32,
    end_of_entry: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Caret {
    line: u64,
    ch: usize,
}

pub struct ChatState {
    messages: VecDeque<Message>,
    lines: VecDeque<Line>,
    history: VecDeque<String>,
    history_pos: usize,
    history_buffer: String,
    scroll: usize,
    new_message_since_scroll: bool,
    pub field: suggestions::Field,
    selection: Option<(Caret, Caret)>,
    dragging: bool,
    wrapped_at: f32,
    next_id: u64,
    just_opened: bool,
    warning: Option<(String, u32)>,
    enforces_secure_chat: bool,
    signing: bool,
    tag_tooltip: Option<(u64, Vec<Vec<Span>>)>,
    link_prompt: Option<String>,
    #[cfg(feature = "mobile_ui")]
    mobile_last_tap: Option<(Caret, u32)>,
    #[cfg(feature = "mobile_ui")]
    mobile_flash: Option<(u64, u32)>,
    #[cfg(feature = "mobile_ui")]
    mobile_drag_last_y: Option<f32>,
    #[cfg(feature = "mobile_ui")]
    texting_lines: usize,
    #[cfg(feature = "mobile_ui")]
    texting_size: (f32, f32),
}

impl Default for ChatState {
    fn default() -> Self {
        ChatState {
            messages: VecDeque::new(),
            lines: VecDeque::new(),
            history: VecDeque::new(),
            history_pos: 0,
            history_buffer: String::new(),
            scroll: 0,
            new_message_since_scroll: false,
            field: suggestions::Field::new(TextBox::new(MAX_MESSAGE_LEN, "")),
            selection: None,
            dragging: false,
            wrapped_at: 0.0,
            next_id: 0,
            just_opened: false,
            warning: None,
            enforces_secure_chat: false,
            signing: false,
            tag_tooltip: None,
            link_prompt: None,
            #[cfg(feature = "mobile_ui")]
            mobile_last_tap: None,
            #[cfg(feature = "mobile_ui")]
            mobile_flash: None,
            #[cfg(feature = "mobile_ui")]
            mobile_drag_last_y: None,
            #[cfg(feature = "mobile_ui")]
            texting_lines: 0,
            #[cfg(feature = "mobile_ui")]
            texting_size: (0.0, 0.0),
        }
    }
}

const WARNING_TICKS: u32 = 200;

impl ChatState {
    pub fn with_saved_history() -> Self {
        let history: VecDeque<String> = crate::gui::command_history::load().into();
        ChatState {
            history_pos: history.len(),
            history,
            ..Default::default()
        }
    }

    pub fn reset_for_session(&mut self) {
        let history = std::mem::take(&mut self.history);
        *self = ChatState {
            history_pos: history.len(),
            history,
            ..Default::default()
        };
    }

    pub fn open(&mut self, prefix: &str) {
        self.field.input.set_text(prefix);
        self.field.input.focused = true;
        self.just_opened = true;
        self.history_pos = self.history.len();
        self.history_buffer.clear();
        self.field.suggest = suggestions::State::default();
    }

    pub fn hide_suggestions(&mut self) -> bool {
        self.field.hide()
    }

    pub fn dismiss_link_prompt(&mut self) -> bool {
        self.link_prompt.take().is_some()
    }

    pub fn close(&mut self) {
        self.field.input.clear();
        self.field.input.focused = false;
        self.selection = None;
        self.dragging = false;
        self.reset_scroll();
        self.field.suggest = suggestions::State::default();
        self.link_prompt = None;
    }

    pub fn submit(&mut self) -> Option<String> {
        let msg = normalize(&self.field.input.text);
        self.field.input.clear();
        if msg.is_empty() {
            return None;
        }
        self.add_recent(&msg);
        self.reset_scroll();
        self.history_pos = self.history.len();
        self.history_buffer.clear();
        Some(msg)
    }

    fn add_recent(&mut self, msg: &str) {
        if self.history.back().map(String::as_str) != Some(msg) {
            if self.history.len() >= MAX_HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(msg.to_string());
        }
        self.history_pos = self.history.len();
        if msg.starts_with('/') {
            crate::gui::command_history::record(msg);
        }
    }

    pub fn push_entries(&mut self, entries: Vec<ChatEntry>, font: &Font, tick: u32) {
        for e in entries {
            let msg = Message {
                spans: e.spans,
                events: e.events,
                tag: e.tag,
                original: e.original.map(Arc::from),
                added_tick: tick,
            };
            self.push_message(msg, font);
        }
    }

    fn push_message(&mut self, msg: Message, font: &Font) {
        self.wrap_into_display(&msg, font);
        self.messages.push_front(msg);
        while self.messages.len() > MAX_HISTORY {
            self.messages.pop_back();
        }
    }

    fn push_error(&mut self, text: &str, font: &Font, tick: u32) {
        let style = Style {
            italic: true,
            ..Style::colored(0xFF5555)
        };
        let msg = Message {
            spans: vec![Span {
                text: text.to_string(),
                style,
            }],
            events: None,
            tag: ChatTag::Error,
            original: None,
            added_tick: tick,
        };
        self.push_message(msg, font);
    }

    fn wrap_into_display(&mut self, msg: &Message, font: &Font) {
        let mut width = if self.wrapped_at > 0.0 {
            self.wrapped_at
        } else {
            CHAT_WIDTH
        };
        if msg.tag == ChatTag::Modified {
            width -= TAG_ICON_RESERVE;
        }
        let mut wrapped = font.wrap(&msg.spans, width);
        if wrapped.is_empty() {
            wrapped.push(Vec::new());
        }
        let count = wrapped.len();
        for (i, mut spans) in wrapped.into_iter().enumerate() {
            if i > 0 {
                spans.insert(
                    0,
                    Span {
                        text: " ".to_string(),
                        style: Style::default(),
                    },
                );
            }
            if self.field.input.focused && self.scroll > 0 {
                self.new_message_since_scroll = true;
                self.scroll_by(1);
            }
            self.lines.push_front(Line {
                id: self.next_id,
                spans,
                events: msg.events.clone(),
                tag: msg.tag,
                original: msg.original.clone(),
                added_tick: msg.added_tick,
                end_of_entry: i == count - 1,
            });
            self.next_id += 1;
            debug_assert!(
                self.lines.len() < 2 || self.lines[0].id > self.lines[1].id,
                "chat lines must stay descending by id; `ChatState::rank` binary-searches them"
            );
            while self.lines.len() > MAX_HISTORY {
                self.lines.pop_back();
            }
        }
    }

    fn rewrap(&mut self, font: &Font) {
        let messages = std::mem::take(&mut self.messages);
        self.lines.clear();
        self.selection = None;
        for msg in messages.iter().rev() {
            self.wrap_into_display(msg, font);
        }
        self.messages = messages;
    }

    fn lines_per_page(&self) -> usize {
        #[cfg(feature = "mobile_ui")]
        {
            if self.field.input.focused && texting() {
                self.texting_lines.max(1)
            } else if self.field.input.focused {
                MOBILE_FOCUSED_LINES
            } else {
                MOBILE_UNFOCUSED_LINES
            }
        }
        #[cfg(not(feature = "mobile_ui"))]
        {
            if self.field.input.focused {
                FOCUSED_LINES
            } else {
                UNFOCUSED_LINES
            }
        }
    }

    fn scroll_by(&mut self, dir: i32) {
        let max = self.lines.len().saturating_sub(self.lines_per_page());
        let next = self.scroll as i32 + dir;
        self.scroll = next.clamp(0, max as i32) as usize;
        if self.scroll == 0 {
            self.new_message_since_scroll = false;
        }
    }

    fn reset_scroll(&mut self) {
        self.scroll = 0;
        self.new_message_since_scroll = false;
    }

    fn rank(&self, id: u64) -> Option<usize> {
        let idx = self.lines.binary_search_by(|l| id.cmp(&l.id)).ok()?;
        Some(self.lines.len() - 1 - idx)
    }

    fn ordered_selection(&self) -> Option<(Caret, Caret)> {
        let (a, b) = self.selection?;
        let (ra, rb) = (self.rank(a.line)?, self.rank(b.line)?);
        if (ra, a.ch) <= (rb, b.ch) {
            Some((a, b))
        } else {
            Some((b, a))
        }
    }

    fn selected_text(&self) -> String {
        let Some((start, end)) = self.ordered_selection() else {
            return String::new();
        };
        let (Some(rs), Some(re)) = (self.rank(start.line), self.rank(end.line)) else {
            return String::new();
        };
        let mut out = String::new();
        for rank in rs..=re {
            let idx = self.lines.len() - 1 - rank;
            let Some(line) = self.lines.get(idx) else {
                continue;
            };
            let text: String = plain(&line.spans);
            let chars: Vec<char> = text.chars().collect();
            let from = if rank == rs {
                start.ch.min(chars.len())
            } else {
                0
            };
            let to = if rank == re {
                end.ch.min(chars.len())
            } else {
                chars.len()
            };
            if rank != rs {
                out.push('\n');
            }
            out.extend(&chars[from.min(to)..to]);
        }
        out
    }

    fn has_selection(&self) -> bool {
        match self.ordered_selection() {
            Some((a, b)) => a != b,
            None => false,
        }
    }

    fn line_under(&self, my: f32, bottom: f32) -> Option<&Line> {
        let page = self.lines_per_page();
        let visible = self.lines.len().saturating_sub(self.scroll).min(page);
        if my >= bottom || my < bottom - visible as f32 * LINE_HEIGHT {
            return None;
        }
        for i in 0..visible {
            let entry_bottom = bottom - i as f32 * LINE_HEIGHT;
            if my >= entry_bottom - LINE_HEIGHT && my < entry_bottom {
                return self.lines.get(i + self.scroll);
            }
        }
        None
    }

    fn event_at(&self, font: &Font, mx: f32, my: f32, bottom: f32) -> Option<&SpanEvent> {
        if mx < MESSAGE_INDENT {
            return None;
        }
        let line = self.line_under(my, bottom)?;
        line.events.as_ref()?;
        let mut x = MESSAGE_INDENT;
        for span in &line.spans {
            for c in span.text.chars() {
                x += font.advance(c, span.style.bold);
                if mx < x {
                    return events::get(&line.events, span.style);
                }
            }
        }
        None
    }

    fn line_at(&self, font: &Font, mx: f32, my: f32, bottom: f32) -> Option<Caret> {
        let page = self.lines_per_page();
        let visible = (self.lines.len() - self.scroll).min(page);
        for i in 0..visible {
            let entry_bottom = bottom - i as f32 * LINE_HEIGHT;
            let entry_top = entry_bottom - LINE_HEIGHT;
            if my < entry_top || my >= entry_bottom {
                continue;
            }
            let line = self.lines.get(i + self.scroll)?;
            let offsets = char_offsets(font, &line.spans);
            let rel = mx - MESSAGE_INDENT;
            let mut best = 0usize;
            let mut best_d = f32::INFINITY;
            for (n, x) in offsets.iter().enumerate() {
                let d = (x - rel).abs();
                if d < best_d {
                    best_d = d;
                    best = n;
                }
            }
            return Some(Caret {
                line: line.id,
                ch: best,
            });
        }
        None
    }
}

fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_MESSAGE_LEN)
        .collect()
}

fn plain(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

fn char_offsets(font: &Font, spans: &[Span]) -> Vec<f32> {
    let mut out = vec![0.0];
    let mut x = 0.0;
    for span in spans {
        for c in span.text.chars() {
            x += font.advance(c, span.style.bold);
            out.push(x);
        }
    }
    out
}

fn caret_count(spans: &[Span]) -> usize {
    1 + spans.iter().map(|s| s.text.chars().count()).sum::<usize>()
}

fn line_alpha(added: u32, now: u32, focused: bool) -> f32 {
    if focused {
        return 1.0;
    }
    let d = now.saturating_sub(added) as f32;
    let t = (10.0 * (1.0 - d / 200.0)).clamp(0.0, 1.0);
    t * t
}

fn argb(alpha: f32, rgb: u32) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    (a << 24) | (rgb & 0x00FF_FFFF)
}

#[cfg(feature = "mobile_ui")]
static TEXTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn texting() -> bool {
    #[cfg(feature = "mobile_ui")]
    return TEXTING.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(feature = "mobile_ui"))]
    false
}

#[cfg(feature = "mobile_ui")]
pub fn set_texting(on: bool) {
    if TEXTING.swap(on, std::sync::atomic::Ordering::Relaxed) == on {
        return;
    }
    crate::platform::orientation::set_portrait(on);
    crate::platform::keyboard::pin(on);
}

fn chat_width(vw: f32) -> f32 {
    if texting() {
        (vw - MESSAGE_INDENT - 12.0).floor().max(1.0)
    } else {
        CHAT_WIDTH
    }
}

fn chat_base(ctx: &ScreenCtx) -> f32 {
    let above_keyboard = ctx.vh - keyboard_inset(ctx);
    #[cfg(feature = "mobile_ui")]
    {
        if texting() {
            above_keyboard
        } else {
            mobile_base(above_keyboard)
        }
    }
    #[cfg(not(feature = "mobile_ui"))]
    {
        above_keyboard
    }
}

#[cfg(feature = "mobile_ui")]
fn mobile_base(limit: f32) -> f32 {
    (MOBILE_TOP_GAP + MOBILE_FOCUSED_LINES as f32 * LINE_HEIGHT + MOBILE_BOTTOM_MARGIN).min(limit)
}

#[cfg(feature = "mobile_ui")]
pub fn hud_box(vh: f32) -> crate::gui::hud_layout::Rect {
    let bottom = mobile_base(vh) - MOBILE_BOTTOM_MARGIN;
    let h = MOBILE_UNFOCUSED_LINES as f32 * LINE_HEIGHT;
    crate::gui::hud_layout::Rect::new(0.0, bottom - h, CHAT_WIDTH + MESSAGE_INDENT + 8.0, h)
}

fn chat_bottom(ctx: &ScreenCtx) -> f32 {
    #[cfg(feature = "mobile_ui")]
    {
        chat_base(ctx) - MOBILE_BOTTOM_MARGIN
    }
    #[cfg(not(feature = "mobile_ui"))]
    {
        chat_base(ctx) - BOTTOM_MARGIN
    }
}

fn keyboard_inset(ctx: &ScreenCtx) -> f32 {
    if ctx.input.scale <= 0.0 {
        return 0.0;
    }
    crate::platform::keyboard::inset_px() / ctx.input.scale
}

pub fn draw(
    p: &mut Painter,
    chat: &mut ChatState,
    ctx: &ScreenCtx,
    focused: bool,
    visible: bool,
    incoming: Vec<ChatEntry>,
    error: Option<String>,
    enforces_secure_chat: bool,
    signing: bool,
    advanced_tooltips: bool,
    shared: &Arc<SharedMutex>,
) {
    let tick = p.frame;
    chat.field.input.focused = focused;
    chat.enforces_secure_chat |= enforces_secure_chat;
    chat.signing = signing;

    let width = chat_width(ctx.vw);
    if chat.wrapped_at != width {
        chat.wrapped_at = width;
        if !chat.messages.is_empty() {
            let font = &p.atlas.font;
            chat.rewrap(font);
        }
    }

    if !incoming.is_empty() {
        let font = &p.atlas.font;
        chat.push_entries(incoming, font, tick);
    }
    if let Some(reason) = error {
        let font = &p.atlas.font;
        chat.push_error(&format!("Message not sent: {reason}"), font, tick);
        chat.warning = Some((reason, tick));
    }

    #[cfg(feature = "mobile_ui")]
    if texting() && focused {
        let top = crate::mobile::pad::top_controls_bottom(ctx.vw, ctx.vh) + TEXTING_TOP_GAP;
        chat.texting_lines = ((chat_bottom(ctx) - top) / LINE_HEIGHT).floor().max(1.0) as usize;
        let resized = chat.texting_size != (ctx.vw, ctx.vh);
        chat.texting_size = (ctx.vw, ctx.vh);
        if resized || ctx.input.left_click {
            crate::platform::keyboard::reassert();
        }
    }

    let prompt_up = chat.link_prompt.is_some();
    if focused && !prompt_up {
        handle_input(p, chat, ctx, shared);
    }

    if visible {
        draw_lines(p, chat, ctx, focused);
    }

    if focused {
        draw_input(p, chat, ctx);
        suggestions::draw(p, &mut chat.field, ctx, SUGGEST, suggest_anchor(ctx));
        if visible
            && chat.link_prompt.is_none()
            && !draw_span_tooltip(p, chat, ctx, advanced_tooltips)
        {
            draw_tag_tooltip(p, chat, ctx);
        }
    }
    if focused
        && let Some(url) = chat.link_prompt.clone()
        && confirmlink::draw(p, ctx, &url)
    {
        chat.link_prompt = None;
    }
}

fn draw_span_tooltip(
    p: &mut Painter,
    chat: &ChatState,
    ctx: &ScreenCtx,
    advanced_tooltips: bool,
) -> bool {
    let Some(m) = ctx.mouse() else { return false };
    if chat.field.contains(m.x, m.y) {
        return false;
    }
    let hover = chat
        .event_at(&p.atlas.font, m.x, m.y, chat_bottom(ctx))
        .and_then(|event| event.hover.as_ref());
    match hover {
        Some(Hover::Lines(rows)) => {
            crate::gui::tooltip::draw_lines(p, rows, m.x, m.y, ctx.vw, ctx.vh);
            true
        }
        Some(Hover::Item(stack)) => {
            crate::gui::tooltip::draw(p, stack, m.x, m.y, ctx.vw, ctx.vh, advanced_tooltips);
            true
        }
        None => false,
    }
}

fn draw_tag_tooltip(p: &mut Painter, chat: &mut ChatState, ctx: &ScreenCtx) {
    const TOOLTIP_WRAP: f32 = 200.0;
    const QUOTE: u32 = 0xAAAAAA;
    const BAR_X: std::ops::Range<f32> = 0.0..2.0;

    let Some(m) = ctx.mouse() else { return };
    if let Some(list) = &chat.field.suggest.list
        && m.x >= list.x
        && m.x < list.x + list.w
        && m.y >= list.y
        && m.y < list.y + list.h
    {
        return;
    }

    let Some(line) = chat.line_under(m.y, chat_bottom(ctx)) else {
        return;
    };
    let (id, tag) = (line.id, line.tag);
    let on_icon = tag == ChatTag::Modified && line.end_of_entry && {
        let x = MESSAGE_INDENT + p.atlas.font.width(&line.spans) + 4.0;
        m.x >= x && m.x < x + TAG_ICON_SIZE
    };
    if !BAR_X.contains(&m.x) && !on_icon {
        return;
    }
    let Some(title) = tag.tooltip() else { return };
    let rebuild = match &chat.tag_tooltip {
        Some((cached, _)) if *cached == id => None,
        _ => Some(line.original.clone()),
    };

    if let Some(signed) = rebuild {
        let mut rows: Vec<Vec<Span>> = vec![vec![Span {
            text: title.to_string(),
            style: Style::colored(indicator_color(tag, true).unwrap_or(SECURE_INDICATOR)),
        }]];
        if let Some(signed) = signed {
            let quoted = vec![Span {
                text: format!("\"{signed}\""),
                style: Style::colored(QUOTE),
            }];
            rows.extend(p.atlas.font.wrap(&quoted, TOOLTIP_WRAP));
        }
        chat.tag_tooltip = Some((id, rows));
    }

    if let Some((_, rows)) = &chat.tag_tooltip {
        crate::gui::tooltip::draw_lines(p, rows, m.x, m.y, ctx.vw, ctx.vh);
    }
}

fn click_span(
    p: &Painter,
    chat: &mut ChatState,
    mx: f32,
    my: f32,
    bottom: f32,
    shift: bool,
    shared: &Arc<SharedMutex>,
) -> bool {
    let Some(event) = chat.event_at(&p.atlas.font, mx, my, bottom) else {
        return false;
    };
    if shift {
        let insertion = event.insertion.clone();
        if let Some(insertion) = insertion {
            chat.field.input.insert_text(&insertion);
            chat.field.suggest.allow = true;
        }
        return true;
    }
    let Some(click) = event.click.clone() else {
        return false;
    };
    match clickevent::dispatch(click, shared) {
        Followup::None => {}
        Followup::Suggest(command) => {
            chat.field.input.set_text(&command);
            chat.field.suggest.allow = true;
        }
        Followup::Link(url) => chat.link_prompt = Some(url),
        Followup::Dialog(dialog) => {
            shared.lock().unwrap().session.dialog_show = Some(dialog);
        }
    }
    true
}

fn handle_input(p: &mut Painter, chat: &mut ChatState, ctx: &ScreenCtx, shared: &Arc<SharedMutex>) {
    let input: &GuiInput = ctx.input;
    let bottom = chat_bottom(ctx);

    let mut consumed = {
        let font = &p.atlas.font;
        chat.field
            .handle_input(input, font, SUGGEST, suggest_anchor(ctx))
    };

    if input.scroll != 0.0 && !consumed.scroll {
        let dir = input.scroll.clamp(-1.0, 1.0);
        let step = if input.shift { 1 } else { SCROLL_LINES };
        chat.scroll_by((dir * step as f32) as i32);
    }
    let page = chat.lines_per_page() as i32 - 1;
    if input.page_up {
        chat.scroll_by(page);
    }
    if input.page_down {
        chat.scroll_by(-page);
    }

    if !consumed.up_down {
        if input.up_arrow {
            move_in_history(chat, -1);
        }
        if input.down_arrow {
            move_in_history(chat, 1);
        }
    }

    let input_top = chat_base(ctx) - 14.0;
    let over_input = input.mouse.map(|m| m.y >= input_top).unwrap_or(false);

    #[cfg(feature = "mobile_ui")]
    {
        const DOUBLE_TAP_FRAMES: u32 = 20;
        const SCROLL_DRAG_PX: f32 = LINE_HEIGHT;

        if let Some(m) = input.mouse
            && !consumed.click
            && !over_input
        {
            let font = &p.atlas.font;
            if input.left_click && click_span(p, chat, m.x, m.y, bottom, false, shared) {
                consumed.click = true;
                chat.mobile_last_tap = None;
            } else if input.left_click {
                let hit = chat.line_at(font, m.x, m.y, bottom);
                let doubled = chat.mobile_last_tap.is_some_and(|(prev, t)| {
                    Some(prev) == hit && p.frame.saturating_sub(t) <= DOUBLE_TAP_FRAMES
                });
                if doubled && let Some(caret) = hit {
                    if let Some(line) = chat.lines.iter().find(|l| l.id == caret.line) {
                        clipboard_set(&plain(&line.spans));
                    }
                    chat.mobile_flash = Some((caret.line, p.frame + MOBILE_FLASH_FRAMES));
                    chat.mobile_last_tap = None;
                } else {
                    chat.mobile_last_tap = hit.map(|c| (c, p.frame));
                }
                chat.dragging = true;
                chat.mobile_drag_last_y = Some(m.y);
            } else if chat.dragging
                && input.left_down
                && let Some(last_y) = chat.mobile_drag_last_y
            {
                let dy = m.y - last_y;
                if dy.abs() >= SCROLL_DRAG_PX {
                    let lines = (dy / SCROLL_DRAG_PX) as i32;
                    chat.scroll_by(lines);
                    chat.mobile_drag_last_y = Some(last_y + lines as f32 * SCROLL_DRAG_PX);
                }
            }
        }
        if input.left_release {
            chat.dragging = false;
            chat.mobile_drag_last_y = None;
        }
    }

    #[cfg(not(feature = "mobile_ui"))]
    {
        if let Some(m) = input.mouse
            && !consumed.click
        {
            let font = &p.atlas.font;
            if input.left_click
                && !over_input
                && click_span(p, chat, m.x, m.y, bottom, input.shift, shared)
            {
                consumed.click = true;
            } else if input.left_click && !over_input {
                match chat.line_at(&p.atlas.font, m.x, m.y, bottom) {
                    Some(c) => {
                        chat.selection = Some((c, c));
                        chat.dragging = true;
                    }
                    None => chat.selection = None,
                }
            }
            if chat.dragging
                && input.left_down
                && let Some(c) = chat.line_at(font, m.x, m.y, bottom)
                && let Some((anchor, _)) = chat.selection
            {
                chat.selection = Some((anchor, c));
            }
        }
        if input.left_release {
            chat.dragging = false;
        }
    }

    let font = &p.atlas.font;
    if !consumed.click {
        chat.field.input.handle_mouse(
            input,
            font,
            INPUT_TEXT_X,
            chat_base(ctx) - 14.0,
            ctx.vw - 8.0,
            12.0,
        );
    }
    if chat.just_opened {
        chat.just_opened = false;
        chat.field.input.sync_keyboard();
    } else if chat.field.input.handle_input(input) {
        chat.field.suggest.allow = true;
    }

    if input.copy && chat.has_selection() {
        clipboard_set(&chat.selected_text());
    }

    let font = &p.atlas.font;
    chat.field
        .refresh(font, shared, SUGGEST, suggest_anchor(ctx));
}

fn move_in_history(chat: &mut ChatState, dir: i32) {
    let max = chat.history.len();
    let next = (chat.history_pos as i32 + dir).clamp(0, max as i32) as usize;
    if next == chat.history_pos {
        return;
    }
    chat.field.suggest.allow = false;
    if next == max {
        let buffer = std::mem::take(&mut chat.history_buffer);
        chat.field.input.set_text(&buffer);
    } else {
        if chat.history_pos == max {
            chat.history_buffer = chat.field.input.text.clone();
        }
        let text = chat.history[next].clone();
        chat.field.input.set_text(&text);
    }
    chat.field.input.push_keyboard();
    chat.history_pos = next;
}

fn draw_lines(p: &mut Painter, chat: &ChatState, ctx: &ScreenCtx, focused: bool) {
    let tick = p.frame;
    let bottom = chat_bottom(ctx);
    let page = chat.lines_per_page();
    let total = chat.lines.len();
    let visible = total.saturating_sub(chat.scroll).min(page);

    let selection = chat
        .ordered_selection()
        .and_then(|(a, b)| Some((chat.rank(a.line)?, a.ch, chat.rank(b.line)?, b.ch)));

    let mut drawn = 0usize;
    for i in 0..visible {
        let Some(line) = chat.lines.get(i + chat.scroll) else {
            continue;
        };
        let alpha = line_alpha(line.added_tick, tick, focused);
        if alpha <= 1.0e-5 {
            continue;
        }
        drawn += 1;

        let entry_bottom = bottom - i as f32 * LINE_HEIGHT;
        let entry_top = entry_bottom - LINE_HEIGHT;
        let text_top = entry_bottom - TEXT_RISE;

        p.fill(
            0.0,
            entry_top,
            chat.wrapped_at + MESSAGE_INDENT + 8.0,
            LINE_HEIGHT,
            argb(alpha * BG_ALPHA, 0x000000),
        );

        #[cfg(feature = "mobile_ui")]
        if let Some((id, until)) = chat.mobile_flash
            && id == line.id
            && tick < until
        {
            const FLASH_ALPHA: f32 = 0.5;
            let left = (until - tick) as f32 / MOBILE_FLASH_FRAMES as f32;
            p.fill(
                0.0,
                entry_top,
                chat.wrapped_at + MESSAGE_INDENT + 8.0,
                LINE_HEIGHT,
                argb(left * FLASH_ALPHA, 0x000000),
            );
        }

        if let Some(rgb) = indicator_color(line.tag, focused) {
            p.fill(
                0.0,
                entry_top,
                2.0,
                LINE_HEIGHT,
                argb(alpha * TEXT_OPACITY, rgb),
            );
        }

        let mut sel: Option<(usize, usize)> = None;
        if let Some((rs, sch, re, ech)) = selection {
            let rank = total - 1 - (i + chat.scroll);
            if rank >= rs && rank <= re {
                let last = caret_count(&line.spans) - 1;
                let from = if rank == rs { sch.min(last) } else { 0 };
                let to = if rank == re { ech.min(last) } else { last };
                sel = (from != to).then(|| (from.min(to), from.max(to)));
            }
        }

        draw_line_text(
            p,
            &line.spans,
            MESSAGE_INDENT,
            text_top,
            alpha * TEXT_OPACITY,
            sel,
        );

        if focused && line.end_of_entry && line.tag == ChatTag::Modified {
            let x = MESSAGE_INDENT + p.atlas.font.width(&line.spans) + 4.0;
            p.sprite(
                "icon/chat_modified",
                x,
                text_top - 1.0,
                TAG_ICON_SIZE,
                TAG_ICON_SIZE,
            );
        }
    }

    if focused && total > 0 {
        draw_scrollbar(p, chat, bottom, drawn, total);
    }
}

fn draw_line_text(
    p: &mut Painter,
    spans: &[Span],
    x: f32,
    y: f32,
    alpha: f32,
    sel: Option<(usize, usize)>,
) {
    let Some((from, to)) = sel else {
        p.text_faded(spans, x, y, true, alpha);
        return;
    };

    let offsets = char_offsets(&p.atlas.font, spans);
    let (x0, x1) = (x + offsets[from], x + offsets[to]);
    p.text_faded(&slice_spans(spans, 0, from), x, y, true, alpha);
    p.text_faded(
        &slice_spans(spans, to, offsets.len() - 1),
        x1,
        y,
        true,
        alpha,
    );
    p.fill(x0, y - 1.0, x1 - x0, LINE_HEIGHT + 1.0, SELECTION_BG);
    let inverted: Vec<Span> = slice_spans(spans, from, to)
        .into_iter()
        .map(|s| Span {
            text: s.text,
            style: Style {
                color: 0x000000,
                ..s.style
            },
        })
        .collect();
    p.text(&inverted, x0, y, false);
}

fn slice_spans(spans: &[Span], from: usize, to: usize) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut i = 0usize;
    for span in spans {
        for c in span.text.chars() {
            if i >= from && i < to {
                match out.last_mut() {
                    Some(last) if last.style == span.style => last.text.push(c),
                    _ => out.push(Span {
                        text: c.to_string(),
                        style: span.style,
                    }),
                }
            }
            i += 1;
        }
    }
    out
}

fn indicator_color(tag: ChatTag, focused: bool) -> Option<u32> {
    match tag {
        ChatTag::Secure if focused => Some(SECURE_INDICATOR),
        ChatTag::Modified => Some(MODIFIED_INDICATOR),
        _ => tag.indicator_color(),
    }
}

fn draw_scrollbar(p: &mut Painter, chat: &ChatState, bottom: f32, drawn: usize, total: usize) {
    let drawn_h = drawn as f32 * LINE_HEIGHT;
    let virtual_h = total as f32 * LINE_HEIGHT;
    if virtual_h <= drawn_h || drawn == 0 {
        return;
    }
    let offset = chat.scroll as f32 * drawn_h / total as f32;
    let height = drawn_h * drawn_h / virtual_h;
    let top = bottom - offset - height;
    let alpha = if chat.scroll > 0 { 170 } else { 96 };
    let rgb = if chat.new_message_since_scroll {
        0xCC3333
    } else {
        0x3333AA
    };
    let x = MESSAGE_INDENT + chat.wrapped_at + 4.0;
    p.fill(x, top, 2.0, height, (alpha << 24) | rgb);
    p.fill(x + 1.0, top, 1.0, height, (alpha << 24) | 0xCCCCCC);
}

fn draw_input(p: &mut Painter, chat: &mut ChatState, ctx: &ScreenCtx) {
    let tick = p.frame;
    p.fill(2.0, chat_base(ctx) - 14.0, ctx.vw - 4.0, 12.0, INPUT_BG);
    draw_input_line(p, chat, ctx, tick);

    let warning = if chat.enforces_secure_chat && !chat.signing {
        Some(
            "This server enforces signed chat and signing is off, so it will reject \
             everything you type here. Options -> Game -> Chat turns it on."
                .to_string(),
        )
    } else {
        chat.warning
            .as_ref()
            .filter(|(_, at)| tick.saturating_sub(*at) < WARNING_TICKS)
            .map(|(t, _)| t.clone())
    };
    let Some(text) = warning else { return };

    let font = &p.atlas.font;
    let max = (ctx.vw - 8.0).max(64.0);
    let spans = vec![Span {
        text,
        style: Style::colored(0xFF5555),
    }];
    let wrapped = font.wrap(&spans, max);
    let h = wrapped.len() as f32 * LINE_HEIGHT + 2.0;
    let top = chat_base(ctx) - 14.0 - h - 1.0;
    p.fill(2.0, top, ctx.vw - 4.0, h, 0xC0000000);
    for (i, line) in wrapped.iter().enumerate() {
        p.text(line, 4.0, top + 1.0 + i as f32 * LINE_HEIGHT, true);
    }
}

fn draw_input_line(p: &mut Painter, chat: &mut ChatState, ctx: &ScreenCtx, tick: u32) {
    let (x, y, w, h) = (INPUT_TEXT_X, chat_base(ctx) - 14.0, ctx.vw - 8.0, 12.0);
    let text_y = y + (h - 8.0) / 2.0;
    let count = chat.field.input.text.chars().count();
    let cursor = chat.field.input.cursor.min(count);

    if cursor < chat.field.input.scroll {
        chat.field.input.scroll = cursor;
    }
    loop {
        let upto: String = chat
            .field
            .input
            .text
            .chars()
            .skip(chat.field.input.scroll)
            .take(cursor - chat.field.input.scroll)
            .collect();
        if p.atlas.font.width_str(&upto) <= w || chat.field.input.scroll >= cursor {
            break;
        }
        chat.field.input.scroll += 1;
    }
    let scroll = chat.field.input.scroll;

    let mut shown = 0usize;
    let mut width = 0.0f32;
    for c in chat.field.input.text.chars().skip(scroll) {
        let advance = p.atlas.font.advance(c, false);
        if width + advance > w {
            break;
        }
        width += advance;
        shown += 1;
    }

    let spans: Vec<Span> = {
        let info = &chat.field.suggest.info;
        let total: usize = info.spans.iter().map(|s| s.text.chars().count()).sum();
        if info.is_command && total == count {
            slice_spans(&info.spans, scroll, scroll + shown)
        } else {
            let text: String = chat
                .field
                .input
                .text
                .chars()
                .skip(scroll)
                .take(shown)
                .collect();
            vec![Span {
                text,
                style: Style::colored(chat.field.input.color),
            }]
        }
    };
    p.text(&spans, x, text_y, true);

    if let Some(ghost) = &chat.field.suggest.ghost
        && !ghost.is_empty()
        && scroll + shown >= count
    {
        p.text_str(ghost, x + width, text_y, GHOST_COLOR, true);
    }

    let (sa, sb) = (
        chat.field.input.cursor.min(chat.field.input.anchor),
        chat.field.input.cursor.max(chat.field.input.anchor),
    );
    if sa != sb {
        let visible: String = chat
            .field
            .input
            .text
            .chars()
            .skip(scroll)
            .take(shown)
            .collect();
        let a = sa.max(scroll) - scroll;
        let b = sb.max(scroll) - scroll;
        let pre: String = visible.chars().take(a).collect();
        let mid: String = visible.chars().skip(a).take(b.saturating_sub(a)).collect();
        let x0 = x + p.atlas.font.width_str(&pre);
        let x1 = (x0 + p.atlas.font.width_str(&mid)).min(x + w);
        p.fill(x0, text_y - 1.0, (x1 - x0).max(1.0), 11.0, SELECTION_BG);
        p.text(
            &[Span {
                text: mid,
                style: Style::colored(0x000000),
            }],
            x0,
            text_y,
            false,
        );
    }

    if (tick / 6) % 2 == 0 {
        let pre: String = chat
            .field
            .input
            .text
            .chars()
            .skip(scroll)
            .take(cursor - scroll)
            .collect();
        let mut cx = x + p.atlas.font.width_str(&pre);
        if cursor >= count {
            p.text_str("_", cx, text_y, chat.field.input.color, true);
        } else {
            cx -= 1.0;
            p.fill(
                cx,
                text_y - 1.0,
                1.0,
                11.0,
                chat.field.input.color | 0xFF00_0000,
            );
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn preview_tree() -> crate::play::commands::CommandTree {
        use azalea_protocol::packets::game::c_commands::{
            BrigadierNodeStub, BrigadierParser, NodeType,
        };
        let names = [
            "gamemode", "gamerule", "give", "gc", "glow", "god", "grant", "group", "guild", "gtp",
            "gui", "gm",
        ];
        let mut stubs = vec![BrigadierNodeStub {
            is_executable: false,
            children: (1..=names.len() as u32 + 1).collect(),
            redirect_node: None,
            node_type: NodeType::Root,
            is_restricted: false,
        }];
        for (i, name) in names.iter().enumerate() {
            let children = if i == 0 {
                vec![names.len() as u32 + 2]
            } else {
                vec![]
            };
            stubs.push(BrigadierNodeStub {
                is_executable: true,
                children,
                redirect_node: None,
                node_type: NodeType::Literal {
                    name: name.to_string(),
                },
                is_restricted: false,
            });
        }
        stubs.push(BrigadierNodeStub {
            is_executable: true,
            children: vec![],
            redirect_node: None,
            node_type: NodeType::Literal {
                name: "team".to_string(),
            },
            is_restricted: false,
        });
        stubs.push(BrigadierNodeStub {
            is_executable: true,
            children: vec![],
            redirect_node: None,
            node_type: NodeType::Argument {
                name: "mode".to_string(),
                parser: BrigadierParser::GameMode,
                suggestions_type: None,
            },
            is_restricted: false,
        });
        crate::play::commands::CommandTree::from_stubs(&stubs, 0)
    }

    #[test]
    fn normalize_matches_vanilla() {
        assert_eq!(normalize("  hello   world  "), "hello world");
        assert_eq!(normalize("\t a \n b "), "a b");
        assert_eq!(normalize("   "), "");
        assert_eq!(normalize(&"x".repeat(300)).chars().count(), MAX_MESSAGE_LEN);
    }

    #[test]
    fn fade_curve_matches_vanilla() {
        assert_eq!(line_alpha(0, 0, false), 1.0);
        assert_eq!(line_alpha(0, 180, false), 1.0);
        assert_eq!(line_alpha(0, 200, false), 0.0);
        assert_eq!(line_alpha(0, 500, false), 0.0);
        let mid = line_alpha(0, 190, false);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
        assert_eq!(line_alpha(0, 1000, true), 1.0);
    }

    #[test]
    fn scroll_clamps_to_the_page() {
        let mut chat = ChatState::default();
        chat.scroll_by(7);
        assert_eq!(chat.scroll, 0);

        for i in 0..30u64 {
            chat.lines.push_front(Line {
                id: i,
                spans: Vec::new(),
                events: None,
                tag: ChatTag::Secure,
                original: None,
                added_tick: 0,
                end_of_entry: true,
            });
        }
        chat.scroll_by(7);
        assert_eq!(chat.scroll, 7);
        chat.scroll_by(7);
        assert_eq!(chat.scroll, 14);
        chat.scroll_by(100);
        assert_eq!(chat.scroll, 20);
        chat.scroll_by(-1000);
        assert_eq!(chat.scroll, 0);
    }

    #[test]
    fn recent_chat_dedupes_consecutive() {
        let mut chat = ChatState::default();
        chat.add_recent("a");
        chat.add_recent("a");
        chat.add_recent("b");
        assert_eq!(chat.history.len(), 2);
        assert_eq!(chat.history_pos, 2);
    }

    #[test]
    fn only_commands_are_saved() {
        let mut chat = ChatState::default();
        chat.add_recent("plain message");
        let before = crate::gui::command_history::load();
        assert_ne!(before.last().map(String::as_str), Some("plain message"));
        chat.add_recent("/test only_commands_are_saved");
        let after = crate::gui::command_history::load();
        assert_eq!(
            after.last().map(String::as_str),
            Some("/test only_commands_are_saved")
        );
        assert_eq!(chat.history.len(), 2, "both reach the recall list");
    }

    #[test]
    fn reset_keeps_the_recall_list() {
        let mut chat = ChatState::default();
        chat.add_recent("first");
        chat.lines.push_front(line(0, "hello"));
        chat.scroll = 3;
        chat.field.input.set_text("half typed");
        chat.reset_for_session();
        assert_eq!(chat.history.len(), 1);
        assert_eq!(chat.history_pos, 1);
        assert!(chat.lines.is_empty());
        assert_eq!(chat.scroll, 0);
        assert!(chat.field.input.text.is_empty());
    }

    fn line(id: u64, text: &str) -> Line {
        Line {
            id,
            spans: vec![Span {
                text: text.to_string(),
                style: Style::default(),
            }],
            events: None,
            tag: ChatTag::Secure,
            original: None,
            added_tick: 0,
            end_of_entry: true,
        }
    }

    #[test]
    fn selection_reads_in_display_order() {
        let mut chat = ChatState::default();
        for (i, t) in ["one", "two", "three"].iter().enumerate() {
            chat.lines.push_front(line(i as u64, t));
        }
        let start = Caret { line: 0, ch: 1 };
        let end = Caret { line: 2, ch: 3 };

        chat.selection = Some((start, end));
        assert_eq!(chat.selected_text(), "ne\ntwo\nthr");
        chat.selection = Some((end, start));
        assert_eq!(chat.selected_text(), "ne\ntwo\nthr");
    }

    #[test]
    fn drag_selects_across_lines() {
        use crate::gui::atlas::build_gui_atlas;
        use crate::gui::render::GuiInput;
        use bevy::prelude::Vec2;

        let atlas = build_gui_atlas(&crate::assets_root());
        let (vw, vh) = (426.0_f32, 240.0_f32);
        let mut chat = ChatState::default();
        chat.field.input.focused = true;
        chat.push_entries(
            ["first line", "second line", "third line"]
                .iter()
                .map(|t| ChatEntry {
                    spans: vec![Span {
                        text: t.to_string(),
                        style: Style::default(),
                    }],
                    events: None,
                    tag: ChatTag::Secure,
                    original: None,
                })
                .collect(),
            &atlas.font,
            0,
        );
        assert_eq!(chat.lines.len(), 3);

        let mut p = Painter::new(&atlas, 0);
        let shared: Arc<SharedMutex> = Arc::new(SharedMutex::default());
        let press = GuiInput {
            size: Vec2::new(vw, vh),
            mouse: Some(Vec2::new(20.0, 186.0)),
            left_click: true,
            left_down: true,
            scale: 1.0,
            ..Default::default()
        };
        handle_input(
            &mut p,
            &mut chat,
            &ScreenCtx {
                input: &press,
                vw,
                vh,
            },
            &shared,
        );
        assert!(chat.dragging, "the press should start a drag");

        let drag = GuiInput {
            size: Vec2::new(vw, vh),
            mouse: Some(Vec2::new(200.0, 196.0)),
            left_down: true,
            scale: 1.0,
            ..Default::default()
        };
        handle_input(
            &mut p,
            &mut chat,
            &ScreenCtx {
                input: &drag,
                vw,
                vh,
            },
            &shared,
        );

        let text = chat.selected_text();
        assert!(
            text.contains('\n'),
            "selection should span two lines, got {text:?}"
        );
        assert!(text.ends_with("third line"), "got {text:?}");
    }

    fn popup_fixture(
        atlas: &crate::gui::atlas::GuiAtlas,
    ) -> (ChatState, std::sync::Arc<crate::session::SharedMutex>) {
        use std::sync::{Arc, Mutex};
        let shared: Arc<SharedMutex> = Arc::new(SharedMutex::default());
        shared.lock().unwrap().session.command_tree = Some(Arc::new(preview_tree()));
        let mut chat = ChatState::default();
        chat.open("/");
        chat.just_opened = false;
        chat.field.suggest.allow = true;
        chat.field.input.set_text("/g");
        let mut p = Painter::new(atlas, 0);
        let input = idle_input();
        handle_input(
            &mut p,
            &mut chat,
            &ScreenCtx {
                input: &input,
                vw: 427.0,
                vh: 240.0,
            },
            &shared,
        );
        (chat, shared)
    }

    fn idle_input() -> GuiInput {
        use bevy::prelude::Vec2;
        GuiInput {
            size: Vec2::new(427.0, 240.0),
            ..Default::default()
        }
    }

    fn press(
        chat: &mut ChatState,
        shared: &std::sync::Arc<crate::session::SharedMutex>,
        atlas: &crate::gui::atlas::GuiAtlas,
        input: GuiInput,
    ) {
        let mut p = Painter::new(atlas, 0);
        handle_input(
            &mut p,
            chat,
            &ScreenCtx {
                input: &input,
                vw: 427.0,
                vh: 240.0,
            },
            shared,
        );
    }

    #[test]
    fn tab_applies_then_cycles() {
        use crate::gui::atlas::build_gui_atlas;
        let atlas = build_gui_atlas(&crate::assets_root());
        let (mut chat, shared) = popup_fixture(&atlas);
        assert!(
            chat.field.suggest.list.is_some(),
            "the popup should be up after an edit"
        );

        press(
            &mut chat,
            &shared,
            &atlas,
            GuiInput {
                tab: true,
                ..idle_input()
            },
        );
        assert_eq!(chat.field.input.text, "/gamemode");
        assert_eq!(
            chat.field.input.cursor,
            chat.field.input.text.chars().count()
        );
        assert!(chat.field.suggest.list.is_some());

        press(
            &mut chat,
            &shared,
            &atlas,
            GuiInput {
                tab: true,
                ..idle_input()
            },
        );
        assert_eq!(chat.field.input.text, "/gamerule");
        press(
            &mut chat,
            &shared,
            &atlas,
            GuiInput {
                tab: true,
                shift: true,
                ..idle_input()
            },
        );
        assert_eq!(chat.field.input.text, "/gamemode");
    }

    #[test]
    fn arrows_move_the_selection_before_the_history() {
        use crate::gui::atlas::build_gui_atlas;
        let atlas = build_gui_atlas(&crate::assets_root());
        let (mut chat, shared) = popup_fixture(&atlas);
        chat.history.push_back("/earlier".to_string());
        chat.history_pos = chat.history.len();

        press(
            &mut chat,
            &shared,
            &atlas,
            GuiInput {
                down_arrow: true,
                ..idle_input()
            },
        );
        assert_eq!(chat.field.suggest.list.as_ref().unwrap().current, 1);
        assert_eq!(
            chat.field.input.text, "/g",
            "the history must not have moved"
        );

        assert!(chat.hide_suggestions());
        assert!(!chat.hide_suggestions());
        press(
            &mut chat,
            &shared,
            &atlas,
            GuiInput {
                up_arrow: true,
                ..idle_input()
            },
        );
        assert_eq!(chat.field.input.text, "/earlier");
    }

    #[test]
    fn server_side_nodes_round_trip_through_shared_state() {
        use crate::gui::atlas::build_gui_atlas;
        use crate::play::commands::{Range, Suggestion, Suggestions};
        use std::sync::{Arc, Mutex};

        let atlas = build_gui_atlas(&crate::assets_root());
        let shared: Arc<SharedMutex> = Arc::new(SharedMutex::default());
        shared.lock().unwrap().session.command_tree = Some(Arc::new(ask_server_tree()));
        let mut chat = ChatState::default();
        chat.open("/");
        chat.just_opened = false;
        chat.field.suggest.allow = true;
        chat.field.input.set_text("/team re");
        press(&mut chat, &shared, &atlas, idle_input());

        let (id, command) = shared
            .lock()
            .unwrap()
            .session
            .suggestion_request
            .take()
            .expect("a request");
        assert_eq!(command, "/team re");
        shared.lock().unwrap().session.suggestion_reply =
            Some((id.wrapping_add(7), Suggestions::default()));
        press(&mut chat, &shared, &atlas, idle_input());
        assert!(chat.field.suggest.list.is_none());

        let answer = Suggestions::create(
            &"/team re".chars().collect::<Vec<_>>(),
            vec![Suggestion {
                range: Range::between(6, 8),
                text: "red".to_string(),
                tooltip: None,
            }],
        );
        shared.lock().unwrap().session.suggestion_reply = Some((id, answer));
        press(&mut chat, &shared, &atlas, idle_input());
        let list = chat
            .field
            .suggest
            .list
            .as_ref()
            .expect("the reply should open the popup");
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].text, "red");

        assert!(shared.lock().unwrap().session.suggestion_request.is_none());
    }

    fn ask_server_tree() -> crate::play::commands::CommandTree {
        use azalea_protocol::packets::game::c_commands::{
            BrigadierNodeStub, BrigadierParser, BrigadierString, NodeType,
        };
        let stubs = vec![
            BrigadierNodeStub {
                is_executable: false,
                children: vec![1],
                redirect_node: None,
                node_type: NodeType::Root,
                is_restricted: false,
            },
            BrigadierNodeStub {
                is_executable: false,
                children: vec![2],
                redirect_node: None,
                node_type: NodeType::Literal {
                    name: "team".to_string(),
                },
                is_restricted: false,
            },
            BrigadierNodeStub {
                is_executable: true,
                children: vec![],
                redirect_node: None,
                node_type: NodeType::Argument {
                    name: "team".to_string(),
                    parser: BrigadierParser::String(BrigadierString::SingleWord),
                    suggestions_type: Some(azalea_registry::identifier::Identifier::new(
                        "minecraft:ask_server",
                    )),
                },
                is_restricted: false,
            },
        ];
        crate::play::commands::CommandTree::from_stubs(&stubs, 0)
    }

    #[test]
    fn empty_selection_copies_nothing() {
        let mut chat = ChatState::default();
        chat.lines.push_front(line(0, "hello"));
        let c = Caret { line: 0, ch: 2 };
        chat.selection = Some((c, c));
        assert!(!chat.has_selection());
        assert_eq!(chat.selected_text(), "");
    }
}

#[cfg(test)]
mod visual {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::gui::atlas::build_gui_atlas;
    use crate::gui::preview::{output_dir, rasterize, upscale};
    use crate::gui::render::GuiInput;
    use crate::gui::{GuiState, Screen};
    use bevy::prelude::Vec2;

    fn entry(text: &str, color: u32, tag: ChatTag) -> ChatEntry {
        ChatEntry {
            spans: vec![Span {
                text: text.to_string(),
                style: Style::colored(color),
            }],
            events: None,
            tag,
            original: None,
        }
    }

    fn sample() -> Vec<ChatEntry> {
        vec![
            entry(
                "Server started. Type /help for commands.",
                0xFFFF55,
                ChatTag::System,
            ),
            entry("<Alex> hey, anyone near spawn?", 0xFFFFFF, ChatTag::Secure),
            entry("<Steve> yeah give me a sec", 0xFFFFFF, ChatTag::Secure),
            entry(
                "[Rank] <Bot> this line was rewritten by a chat plugin",
                0xFFFFFF,
                ChatTag::Modified,
            ),
            entry(
                "<Console> unsigned relay message",
                0xFFFFFF,
                ChatTag::NotSecure,
            ),
            entry(
                "<Alex> a much longer line that has to wrap because it does not fit \
                 inside the 320 unit chat width vanilla uses at scale one",
                0xFFFFFF,
                ChatTag::Secure,
            ),
        ]
    }

    fn frame(
        atlas: &crate::gui::atlas::GuiAtlas,
        state: &mut GuiState,
        shared: &Arc<SharedMutex>,
        input: &GuiInput,
        tick: u32,
        name: &str,
        vw: f32,
        vh: f32,
    ) {
        let mut p = Painter::new(atlas, tick);
        crate::gui::screens::draw(
            &mut p,
            state,
            input,
            shared,
            #[cfg(feature = "skins")]
            &mut Default::default(),
        );
        let img = rasterize(&p, vw as u32, vh as u32, [70, 110, 170, 255]);
        upscale(&img, 2).save(output_dir().join(name)).unwrap();
    }

    #[test]
    fn preview_chat_states() {
        let atlas = build_gui_atlas(&crate::assets_root());
        let (vw, vh) = (427.0_f32, 240.0_f32);
        let shared: Arc<SharedMutex> = Arc::new(SharedMutex::default());
        let mk = |mouse: Option<Vec2>| GuiInput {
            size: Vec2::new(vw, vh),
            scale: 3.0,
            mouse,
            ..Default::default()
        };

        shared.lock().unwrap().session.chat_incoming = sample();
        let mut state = GuiState::default();
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            0,
            "chat_hud.png",
            vw,
            vh,
        );
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            190,
            "chat_hud_fading.png",
            vw,
            vh,
        );

        let mut state = GuiState::default();
        shared.lock().unwrap().session.chat_incoming = sample();
        state.screen = Screen::Chat;
        state.chat.open("");
        state.chat.field.input.set_text("hello world");
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            300,
            "chat_focused.png",
            vw,
            vh,
        );

        let ids: Vec<u64> = state.chat.lines.iter().map(|l| l.id).collect();
        state.chat.selection = Some((
            Caret {
                line: ids[4],
                ch: 6,
            },
            Caret {
                line: ids[1],
                ch: 12,
            },
        ));
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            300,
            "chat_selection.png",
            vw,
            vh,
        );
        assert!(!state.chat.selected_text().is_empty());

        state.chat.field.input.anchor = 2;
        state.chat.field.input.cursor = 8;
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            300,
            "chat_input_selection.png",
            vw,
            vh,
        );
        state.chat.field.input.anchor = state.chat.field.input.cursor;

        let mut state = GuiState::default();
        state.screen = Screen::Chat;
        state.chat.open("/");
        {
            let mut s = shared.lock().unwrap();
            s.session.command_tree = Some(std::sync::Arc::new(super::tests::preview_tree()));
            s.session.player_names = vec!["Alex".to_string(), "Steve".to_string()];
        }
        state.chat.field.suggest.allow = true;
        state.chat.field.input.set_text("/g");
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            300,
            "chat_suggestions.png",
            vw,
            vh,
        );
        assert!(
            state.chat.field.suggest.list.is_some(),
            "the popup should be up"
        );

        for _ in 0..11 {
            state.chat.field.cycle(1, SUGGEST);
        }
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            300,
            "chat_suggestions_scrolled.png",
            vw,
            vh,
        );

        let mut state = GuiState::default();
        state.screen = Screen::Chat;
        state.chat.open("/");
        state.chat.field.suggest.allow = true;
        state.chat.field.input.set_text("/gamemode creative Steve");
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            300,
            "chat_command_colored.png",
            vw,
            vh,
        );

        let mut state = GuiState::default();
        state.screen = Screen::Chat;
        state.chat.open("/");
        state.chat.field.suggest.allow = true;
        state.chat.field.input.set_text("/nosuchcommand");
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            300,
            "chat_command_error.png",
            vw,
            vh,
        );

        let mut state = GuiState::default();
        shared.lock().unwrap().session.chat_incoming = sample();
        state.screen = Screen::Chat;
        state.chat.open("");
        shared.lock().unwrap().session.chat_send_error = Some(
            "Chat disabled due to missing profile public key. Please try reconnecting.".into(),
        );
        frame(
            &atlas,
            &mut state,
            &shared,
            &mk(None),
            320,
            "chat_warning.png",
            vw,
            vh,
        );
    }
}
