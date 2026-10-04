use crate::gui::painter::Painter;
use crate::gui::render::GuiInput;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, MultiLineTextBox, TextBox, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx, tooltip};
use crate::session::{Book, EditBookRequest, InteractionHand};

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
) -> i32 {
    let mut turn = 0;
    if back && arrow(p, ctx, left + ARROW_BACK_X, top + ARROW_Y, "backward") {
        turn = -1;
    }
    if forward && arrow(p, ctx, left + ARROW_FORWARD_X, top + ARROW_Y, "forward") {
        turn = 1;
    }
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

pub fn page_text(p: &mut Painter, left: f32, top: f32, spans: &[crate::text::Span]) {
    let mut lines = p.atlas.font.wrap(spans, TEXT_WIDTH);
    for line in &mut lines {
        for span in line.iter_mut() {
            span.style.color = TEXT_COLOR;
        }
    }
    for (i, line) in lines.iter().take(MAX_LINES).enumerate() {
        p.text(line, left + TEXT_X, top + TEXT_Y + i as f32 * 9.0, false);
    }
}

#[derive(Default)]
pub struct BookState {
    pages: Vec<String>,
    view: Vec<Vec<crate::text::Span>>,
    page: usize,
    slot: u32,
    field: MultiLineTextBox,
    title: TextBox,
    author: String,
}

impl BookState {
    pub fn open_view(&mut self, book: &Book) {
        self.view = book.pages.to_vec();
        self.pages.clear();
        self.page = 0;
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

pub fn draw_view(p: &mut Painter, state: &mut BookState, ctx: &ScreenCtx) -> Option<Screen> {
    let (left, top) = frame(p, ctx);
    let num_pages = state.view.len();
    state.page = clamp_page(state.page, num_pages);

    if let Some(spans) = state.view.get(state.page) {
        page_text(p, left, top, spans);
    }
    indicator(p, left, top, state.page, num_pages);

    let turn = arrows(
        p,
        ctx,
        left,
        top,
        back_visible(state.page),
        forward_visible(state.page, num_pages),
    )
    .saturating_add(page_key(ctx.input))
    .signum();
    state.page = clamp_page(state.page.saturating_add_signed(turn as isize), num_pages);

    let x = ((ctx.vw - WIDGET_WIDTH_BIG) / 2.0).floor();
    let done = Button::new(
        x,
        CONTROLS_TOP,
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        label!("gui.done"),
    );
    done.draw(p, ctx).then_some(Screen::None)
}

pub enum BookOut {
    Nav(Screen),
    Send(EditBookRequest),
}

pub fn draw_edit(p: &mut Painter, state: &mut BookState, ctx: &ScreenCtx) -> Option<BookOut> {
    let (left, top) = frame(p, ctx);
    let blink = p.frame;

    let turn = arrows(p, ctx, left, top, back_visible(state.page), true)
        .saturating_add(page_key(ctx.input))
        .signum();
    state.turn(turn);
    let num_pages = state.pages.len();

    let field_x = ((ctx.vw - TEXT_WIDTH) / 2.0).floor() - 4.0;
    let field_y = 28.0;
    state
        .field
        .handle_mouse(ctx.input, &p.atlas.font, field_x, field_y, TEXT_WIDTH);
    state
        .field
        .handle_input(ctx.input, &p.atlas.font, TEXT_WIDTH);
    state
        .field
        .draw(p, field_x, field_y, TEXT_WIDTH, TEXT_COLOR, blink);

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
    let blink = p.frame;

    let label_text = label!("book.editTitle");
    let w = p.atlas.font.width_str(label_text);
    p.text_plain(
        label_text,
        text_left + (TEXT_WIDTH - w) / 2.0,
        top + 32.0,
        TEXT_COLOR,
        false,
    );

    let title_w = p.atlas.font.width_str(&state.title.text);
    let box_x = ((ctx.vw - TEXT_WIDTH) / 2.0).floor() - 3.0 + (TEXT_WIDTH - title_w) / 2.0;
    let box_y = top + 48.0;
    const GLYPH_H: f32 = 8.0;
    let box_h = GLYPH_H;
    state.title.focused = true;
    state
        .title
        .handle_mouse(ctx.input, &p.atlas.font, box_x, box_y, title_w, box_h);
    state.title.handle_input(ctx.input);
    state.title.draw(p, box_x, box_y, TEXT_WIDTH, box_h, blink);

    let byline = tooltip::translate("book.byAuthor", &[state.author.clone()]);
    let w = p.atlas.font.width_str(&byline);
    p.text_plain(
        &byline,
        text_left + (TEXT_WIDTH - w) / 2.0,
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
    let pressed = finalize.draw(p, ctx) || (finalize.active && ctx.input.enter);
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

const DARK_GRAY: u32 = 0x555555;

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
