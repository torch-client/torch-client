use crate::gui::clickevent::{self, Followup};
use crate::gui::confirmcommand::{Accepted, Answer, CommandPrompt};
use crate::gui::painter::Painter;
use crate::gui::render::GuiInput;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, MultiLineTextBox, TextBox, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx, confirmlink, tooltip};
use crate::play::dialog::{Click, Submit};
use crate::session::{Book, EditBookRequest, InteractionHand, SharedMutex};
use crate::text::events::{self, Events, Hover, SpanEvent};
use crate::text::{Font, Span};

pub const IMAGE_W: f32 = 192.0;
pub const IMAGE_H: f32 = 192.0;
pub const TEXT_WIDTH: f32 = 114.0;
pub const MAX_LINES: usize = 14;
pub const TEXT_X: f32 = 36.0;
pub const TEXT_Y: f32 = 30.0;
pub const CONTROLS_TOP: f32 = 196.0;
pub const MENU_BUTTON_SIZE: f32 = 98.0;
pub const MAX_PAGES: usize = 100;
pub const PAGE_EDIT_LENGTH: usize = 1024;
pub const TITLE_LENGTH: usize = 15;

pub const TEXT_COLOR: u32 = 0x00_0000;

const ARROW_W: f32 = 23.0;
const ARROW_H: f32 = 13.0;
const ARROW_BACK_X: f32 = 43.0;
const ARROW_FORWARD_X: f32 = 116.0;
const ARROW_Y: f32 = 157.0;

pub fn frame(p: &mut Painter, ctx: &ScreenCtx) -> (f32, f32) {
    let left = ((ctx.vw - IMAGE_W) / 2.0).floor();
    let top = 2.0;
    crate::gui::screens::dim_background(p, ctx.vw, ctx.vh);
    p.blit_sheet("book", 0.0, 0.0, IMAGE_W, IMAGE_H, left, top);
    (left, top)
}

pub fn indicator(p: &mut Painter, left: f32, top: f32, page: usize, num_pages: usize) {
    let text = tooltip::translate(
        "book.pageIndicator",
        &[(page + 1).to_string(), num_pages.max(1).to_string()],
    );
    let w = p.atlas.font.width_str(&text);
    p.text_plain(&text, left + 148.0 - w, top + 16.0, TEXT_COLOR, false);
}

pub fn arrows(
    p: &mut Painter,
    ctx: &ScreenCtx,
    left: f32,
    top: f32,
    back: bool,
    forward: bool,
    sound: bool,
) -> i32 {
    let mut turn = 0;
    if back && arrow(p, ctx, left + ARROW_BACK_X, top + ARROW_Y, "backward") {
        turn = -1;
    }
    if forward && arrow(p, ctx, left + ARROW_FORWARD_X, top + ARROW_Y, "forward") {
        turn = 1;
    }
    #[cfg(feature = "audio")]
    if turn != 0 && sound {
        crate::gui::widgets::play_ui("item.book.page_turn");
    }
    #[cfg(not(feature = "audio"))]
    let _ = sound;
    turn
}

fn arrow(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32, dir: &'static str) -> bool {
    let hovered = ctx.hovering(x, y, ARROW_W, ARROW_H);
    let sprite: &str = match (dir, hovered) {
        ("backward", false) => "widget/page_backward",
        ("backward", true) => "widget/page_backward_highlighted",
        (_, false) => "widget/page_forward",
        (_, true) => "widget/page_forward_highlighted",
    };
    p.sprite(sprite, x, y, ARROW_W, ARROW_H);
    hovered && ctx.input.left_click
}

pub fn page_key(input: &GuiInput) -> i32 {
    i32::from(input.page_down) - i32::from(input.page_up)
}

pub fn clamp_page(page: usize, num_pages: usize) -> usize {
    if num_pages == 0 {
        0
    } else {
        page.min(num_pages - 1)
    }
}

pub fn back_visible(page: usize) -> bool {
    page > 0
}

pub fn forward_visible(page: usize, num_pages: usize) -> bool {
    num_pages > 0 && page + 1 < num_pages
}

pub fn page_text(p: &mut Painter, left: f32, top: f32, spans: &[Span]) {
    let lines = p.atlas.font.wrap(spans, TEXT_WIDTH);
    draw_lines(p, left, top, &lines);
}

fn draw_lines(p: &mut Painter, left: f32, top: f32, lines: &[Vec<Span>]) {
    for (i, line) in lines.iter().take(MAX_LINES).enumerate() {
        p.text(
            line,
            left + TEXT_X,
            top + TEXT_Y + i as f32 * LINE_HEIGHT,
            false,
        );
    }
}

const LINE_HEIGHT: f32 = 9.0;

#[derive(Default)]
pub struct PageView {
    page: Option<usize>,
    source: Vec<Span>,
    lines: Vec<Vec<Span>>,
}

impl PageView {
    fn sync(&mut self, font: &Font, page: usize, spans: &[Span]) {
        if self.page == Some(page) && self.source == spans {
            return;
        }
        self.lines = font.wrap(spans, TEXT_WIDTH);
        self.source.clear();
        self.source.extend_from_slice(spans);
        self.page = Some(page);
    }

    pub fn draw(&mut self, p: &mut Painter, left: f32, top: f32, page: usize, spans: &[Span]) {
        self.sync(&p.atlas.font, page, spans);
        draw_lines(p, left, top, &self.lines);
    }

    fn event_at<'e>(
        &self,
        font: &Font,
        left: f32,
        top: f32,
        mx: f32,
        my: f32,
        events: &'e Events,
    ) -> Option<&'e SpanEvent> {
        events.as_ref()?;
        let first = top + TEXT_Y;
        if my < first {
            return None;
        }
        let row = ((my - first) / LINE_HEIGHT) as usize;
        let line = self.lines.iter().take(MAX_LINES).nth(row)?;
        let mut x = left + TEXT_X;
        if mx < x {
            return None;
        }
        for span in line {
            for c in span.text.chars() {
                x += font.advance(c, span.style.bold);
                if mx < x {
                    return events::get(events, span.style);
                }
            }
        }
        None
    }
}

#[derive(Default)]
pub enum ReadOverlay {
    #[default]
    None,
    Link(String),
    Command(CommandPrompt),
}

pub struct Read<'a> {
    pub input: GuiInput,
    pub click: Option<Click>,
    hover: Option<&'a SpanEvent>,
}

#[allow(clippy::too_many_arguments, reason = "one page, flat arguments")]
pub fn read<'a>(
    p: &mut Painter,
    ctx: &ScreenCtx,
    view: &mut PageView,
    overlay: &ReadOverlay,
    left: f32,
    top: f32,
    page: usize,
    spans: &[Span],
    events: &'a Events,
) -> Read<'a> {
    view.draw(p, left, top, page, spans);
    let mut input = ctx.input.clone();
    if !matches!(overlay, ReadOverlay::None) {
        input = confirmlink::masked(&input);
        input.page_up = false;
        input.page_down = false;
        return Read {
            input,
            click: None,
            hover: None,
        };
    }
    let hover = input
        .mouse
        .and_then(|m| view.event_at(&p.atlas.font, left, top, m.x, m.y, events));
    let click = (input.left_click)
        .then(|| hover.and_then(|e| e.click.clone()))
        .flatten();
    if click.is_some() {
        input.left_click = false;
    }
    Read {
        input,
        click,
        hover,
    }
}

impl Read<'_> {
    pub fn tooltip(&self, p: &mut Painter, ctx: &ScreenCtx, advanced: bool) {
        let (Some(event), Some(m)) = (self.hover, ctx.mouse()) else {
            return;
        };
        match &event.hover {
            Some(Hover::Lines(rows)) => tooltip::draw_lines(p, rows, m.x, m.y, ctx.vw, ctx.vh),
            Some(Hover::Item(stack)) => {
                tooltip::draw(p, stack, m.x, m.y, ctx.vw, ctx.vh, advanced, false)
            }
            None => {}
        }
    }
}

pub fn shared_click(click: Click, overlay: &mut ReadOverlay, shared: &SharedMutex) {
    match clickevent::dispatch(click, shared) {
        Followup::Link(url) => *overlay = ReadOverlay::Link(url),
        Followup::Dialog(dialog) => shared.lock().unwrap().session.dialog_show = Some(dialog),
        Followup::Confirm(prompt) => *overlay = ReadOverlay::Command(prompt),
        Followup::None | Followup::Suggest(_) => {}
    }
}

pub fn trim_command(command: &str) -> String {
    command.strip_prefix('/').unwrap_or(command).to_string()
}

#[derive(Default)]
pub struct BookState {
    pages: Vec<String>,
    view: Vec<Vec<crate::text::Span>>,
    view_events: Vec<Events>,
    pub(crate) page_view: PageView,
    pub(crate) overlay: ReadOverlay,
    pub(crate) lectern_click: Option<Click>,
    page: usize,
    slot: u32,
    field: MultiLineTextBox,
    title: TextBox,
    author: String,
}

impl BookState {
    pub fn open_view(&mut self, book: &Book) {
        self.view = book.pages.to_vec();
        self.view_events = book.events.to_vec();
        self.pages.clear();
        self.page = 0;
        self.clear_overlay();
    }

    pub fn clear_overlay(&mut self) {
        self.overlay = ReadOverlay::None;
        self.lectern_click = None;
    }

    pub fn escape_overlay(&mut self, lectern: bool, shared: &SharedMutex) -> bool {
        match std::mem::take(&mut self.overlay) {
            ReadOverlay::None => false,
            ReadOverlay::Link(_) => true,
            ReadOverlay::Command(_) => {
                if lectern {
                    shared
                        .lock()
                        .unwrap()
                        .session
                        .inv_actions
                        .push(crate::session::InvAction::Close);
                }
                true
            }
        }
    }

    pub fn open_edit(&mut self, book: &Book, hand: InteractionHand, selected: u8, author: &str) {
        self.pages = book.raw_pages();
        if self.pages.is_empty() {
            self.pages.push(String::new());
        }
        self.view.clear();
        self.page = 0;
        self.slot = hand.edit_book_slot(selected);
        self.field = MultiLineTextBox::new(PAGE_EDIT_LENGTH, MAX_LINES);
        self.field.set_text(&self.pages[0]);
        self.field.inner.focused = true;
        self.title = TextBox::new(TITLE_LENGTH, "");
        self.title.color = TEXT_COLOR;
        self.title.no_shadow = true;
        self.title.centered = true;
        self.author = author.to_string();
    }

    fn submit(&self, title: Option<String>) -> EditBookRequest {
        let mut pages = self.pages.clone();
        while pages.last().is_some_and(|p| p.is_empty()) {
            pages.pop();
        }
        EditBookRequest {
            slot: self.slot,
            pages,
            title,
        }
    }

    fn flush(&mut self) {
        if let Some(page) = self.pages.get_mut(self.page) {
            if *page != self.field.text() {
                page.clear();
                page.push_str(self.field.text());
            }
        }
    }

    fn turn(&mut self, by: i32) {
        if by == 0 {
            return;
        }
        if by > 0 {
            if self.page + 1 == self.pages.len() && self.pages.len() >= MAX_PAGES {
                self.flush();
                self.field.set_text(&self.pages[self.page]);
                return;
            }
        } else if self.page == 0 {
            return;
        }
        self.flush();
        if by > 0 {
            if self.page + 1 == self.pages.len() {
                self.pages.push(String::new());
            }
            self.page += 1;
        } else {
            self.page -= 1;
        }
        self.field.set_text(&self.pages[self.page]);
    }
}

pub fn draw_view(
    p: &mut Painter,
    state: &mut BookState,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
    advanced_tooltips: bool,
) -> Option<BookOut> {
    let (left, top) = frame(p, ctx);
    let num_pages = state.view.len();
    state.page = clamp_page(state.page, num_pages);

    const NO_EVENTS: Events = None;
    let overlay_was_up = !matches!(state.overlay, ReadOverlay::None);
    let page = state.page;
    let spans = state.view.get(page).map_or(&[][..], Vec::as_slice);
    let events = state.view_events.get(page).unwrap_or(&NO_EVENTS);
    let read = read(
        p,
        ctx,
        &mut state.page_view,
        &state.overlay,
        left,
        top,
        page,
        spans,
        events,
    );
    indicator(p, left, top, page, num_pages);
    let wctx = ctx.with_input(&read.input);

    let turn = arrows(
        p,
        &wctx,
        left,
        top,
        back_visible(page),
        forward_visible(page, num_pages),
        true,
    )
    .saturating_add(page_key(wctx.input))
    .signum();
    let mut next_page = clamp_page(page.saturating_add_signed(turn as isize), num_pages);

    let x = ((ctx.vw - WIDGET_WIDTH_BIG) / 2.0).floor();
    let done = Button::new(
        x,
        CONTROLS_TOP,
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        label!("gui.done"),
    );
    let mut out = done.draw(p, &wctx).then_some(BookOut::Nav(Screen::None));

    if matches!(state.overlay, ReadOverlay::None) {
        read.tooltip(p, ctx, advanced_tooltips);
    }
    let click = read.click;
    if let Some(click) = click {
        match click {
            Click::ChangePage(target) => {
                next_page = clamp_page(target.saturating_sub(1).max(0) as usize, num_pages);
            }
            Click::RunCommand(command) => {
                let command = trim_command(&command);
                match crate::gui::confirmcommand::check(&command, shared) {
                    Some(prompt) => state.overlay = ReadOverlay::Command(prompt),
                    None => {
                        crate::play::dialog::queue_submit(shared, Submit::RunCommand(command));
                        out = Some(BookOut::Nav(Screen::None));
                    }
                }
            }
            other => shared_click(other, &mut state.overlay, shared),
        }
    }
    state.page = next_page;

    let fresh_input;
    let ctx = if overlay_was_up {
        ctx.with_input(ctx.input)
    } else {
        fresh_input = GuiInput {
            left_click: false,
            ..ctx.input.clone()
        };
        ctx.with_input(&fresh_input)
    };
    let ctx = &ctx;
    match &state.overlay {
        ReadOverlay::None => {}
        ReadOverlay::Link(url) => {
            if confirmlink::draw(p, ctx, url) {
                state.overlay = ReadOverlay::None;
            }
        }
        ReadOverlay::Command(prompt) => {
            if let Some(answer) = prompt.draw(p, ctx, true, true) {
                let accepted = matches!(answer, Answer::Accept).then(|| prompt.accept(true));
                state.overlay = ReadOverlay::None;
                match accepted {
                    None => {}
                    Some(Accepted::Run(command)) => {
                        crate::play::dialog::queue_submit(shared, Submit::RunCommand(command));
                        out = Some(BookOut::Nav(Screen::None));
                    }
                    Some(Accepted::Chat(text)) => out = Some(BookOut::Chat(text)),
                    Some(Accepted::Copied) => out = Some(BookOut::Nav(Screen::None)),
                }
            }
        }
    }
    out
}

pub enum BookOut {
    Nav(Screen),
    Send(EditBookRequest),
    Chat(String),
}

pub fn draw_edit(p: &mut Painter, state: &mut BookState, ctx: &ScreenCtx) -> Option<BookOut> {
    let (left, top) = frame(p, ctx);

    let box_x = ((ctx.vw - TEXT_WIDTH) / 2.0).floor() - 8.0;
    let box_y = 28.0;
    let area = [box_x, box_y, TEXT_WIDTH + 8.0, 134.0];
    let (field_x, field_y) = (box_x + 4.0, box_y + 4.0);

    let on_box = ctx.mouse().is_some_and(|m| {
        m.x >= area[0] && m.x < area[0] + area[2] && m.y >= area[1] && m.y < area[1] + area[3]
    });
    let arrow_input;
    let arrow_ctx = if on_box && ctx.input.left_click {
        arrow_input = GuiInput {
            left_click: false,
            ..ctx.input.clone()
        };
        ctx.with_input(&arrow_input)
    } else {
        ctx.with_input(ctx.input)
    };

    let turn = arrows(
        p,
        &arrow_ctx,
        left,
        top,
        back_visible(state.page),
        true,
        true,
    )
    .saturating_add(page_key(ctx.input))
    .signum();
    state.turn(turn);
    let num_pages = state.pages.len();

    let field_input;
    let field_in = if ctx.input.page_up || ctx.input.page_down {
        field_input = GuiInput {
            page_up: false,
            page_down: false,
            ..ctx.input.clone()
        };
        &field_input
    } else {
        ctx.input
    };
    state
        .field
        .handle_mouse_in(field_in, &p.atlas.font, field_x, field_y, TEXT_WIDTH, area);
    state
        .field
        .handle_input(field_in, &p.atlas.font, TEXT_WIDTH);
    state
        .field
        .draw(p, field_x, field_y, TEXT_WIDTH, TEXT_COLOR);

    indicator(p, left, top, state.page, num_pages);

    let mid = ctx.vw / 2.0;
    let sign = Button::new(
        mid - MENU_BUTTON_SIZE - 2.0,
        CONTROLS_TOP,
        MENU_BUTTON_SIZE,
        WIDGET_HEIGHT,
        label!("book.signButton"),
    );
    if sign.draw(p, ctx) {
        state.flush();
        return Some(BookOut::Nav(Screen::BookSign));
    }
    let done = Button::new(
        mid + 2.0,
        CONTROLS_TOP,
        MENU_BUTTON_SIZE,
        WIDGET_HEIGHT,
        label!("gui.done"),
    );
    if done.draw(p, ctx) {
        state.flush();
        return Some(BookOut::Send(state.submit(None)));
    }
    None
}

pub fn draw_sign(p: &mut Painter, state: &mut BookState, ctx: &ScreenCtx) -> Option<BookOut> {
    let (left, top) = frame(p, ctx);
    let text_left = left + TEXT_X;

    let label_text = label!("book.editTitle");
    let w = p.atlas.font.width_str(label_text);
    p.text_plain(
        label_text,
        text_left + ((TEXT_WIDTH - w) / 2.0).floor(),
        top + 32.0,
        TEXT_COLOR,
        false,
    );

    let box_x = ((ctx.vw - TEXT_WIDTH) / 2.0).floor() - 3.0;
    let box_y = top + 48.0;
    let box_h = 20.0;
    state.title.focused = true;
    state
        .title
        .handle_mouse(ctx.input, &p.atlas.font, box_x, box_y, TEXT_WIDTH, box_h);
    state.title.handle_input(ctx.input);
    state.title.draw(p, box_x, box_y, TEXT_WIDTH, box_h);

    let byline = tooltip::translate("book.byAuthor", &[state.author.clone()]);
    let w = p.atlas.font.width_str(&byline);
    p.text_plain(
        &byline,
        text_left + ((TEXT_WIDTH - w) / 2.0).floor(),
        top + 58.0,
        DARK_GRAY,
        false,
    );

    let warning = tooltip::translate("book.finalizeWarning", &[]);
    let spans = crate::text::styled(&warning, crate::text::Style::colored(TEXT_COLOR));
    let lines = p.atlas.font.wrap(&spans, TEXT_WIDTH);
    for (i, line) in lines.iter().enumerate() {
        p.text(line, text_left, top + 80.0 + i as f32 * 9.0, false);
    }

    let title = state.title.text.trim().to_string();
    let mut finalize = Button::new(
        ctx.vw / 2.0 - 100.0,
        CONTROLS_TOP,
        MENU_BUTTON_SIZE,
        WIDGET_HEIGHT,
        label!("book.finalizeButton"),
    );
    finalize.active = !title.is_empty();
    let pressed = finalize.draw(p, ctx) || (!state.title.text.is_empty() && ctx.input.enter);
    let cancel = Button::new(
        ctx.vw / 2.0 + 2.0,
        CONTROLS_TOP,
        MENU_BUTTON_SIZE,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    );
    if cancel.draw(p, ctx) {
        return Some(BookOut::Nav(Screen::BookEdit));
    }
    if pressed {
        return Some(BookOut::Send(state.submit(Some(title))));
    }
    None
}

const DARK_GRAY: u32 = crate::text::DARK_GRAY;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_book_clamps_to_page_zero() {
        assert_eq!(clamp_page(0, 0), 0);
        assert_eq!(clamp_page(5, 0), 0);
    }

    #[test]
    fn page_clamps_to_the_last_page() {
        assert_eq!(clamp_page(0, 3), 0);
        assert_eq!(clamp_page(2, 3), 2);
        assert_eq!(clamp_page(9, 3), 2);
    }

    #[test]
    fn arrow_visibility_follows_the_ends_of_the_book() {
        assert!(!back_visible(0));
        assert!(back_visible(1));

        assert!(!forward_visible(0, 0));
        assert!(!forward_visible(2, 3));
        assert!(forward_visible(1, 3));
    }

    #[test]
    fn turning_past_the_last_page_appends_one() {
        let mut state = BookState {
            pages: vec![String::from("a")],
            field: MultiLineTextBox::new(PAGE_EDIT_LENGTH, MAX_LINES),
            ..BookState::default()
        };
        state.turn(1);
        assert_eq!(state.pages.len(), 2);
        assert_eq!(state.page, 1);

        state.pages = vec![String::new(); MAX_PAGES];
        state.page = MAX_PAGES - 1;
        state.turn(1);
        assert_eq!(
            state.pages.len(),
            MAX_PAGES,
            "the hundredth page is the last"
        );
        assert_eq!(state.page, MAX_PAGES - 1);
    }

    #[test]
    fn trailing_empty_pages_are_erased_on_send() {
        let state = BookState {
            pages: vec![
                String::from("one"),
                String::new(),
                String::from("three"),
                String::new(),
                String::new(),
            ],
            ..BookState::default()
        };
        assert_eq!(state.submit(None).pages, vec!["one", "", "three"]);
    }
}
