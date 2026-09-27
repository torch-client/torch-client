use crate::gui::book;
use crate::gui::{tooltip, widgets};

use super::{GuiState, InvAction, Painter, ScreenCtx, SlotStack, Snapshot};

const BUTTON_PREV_PAGE: u8 = 1;
const BUTTON_NEXT_PAGE: u8 = 2;
const BUTTON_TAKE_BOOK: u8 = 3;

const DONE: &str = "Done";

pub fn draw(
    p: &mut Painter,
    _state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = book::frame(p, ctx);

    let pages = snap
        .menu_slots
        .first()
        .and_then(|s| s.book.as_deref())
        .map(|b| &*b.pages);
    let num_pages = pages.map_or(0, |p| p.len());

    let page = book::clamp_page(snap.container_data[0].max(0) as usize, num_pages);

    if let Some(spans) = pages.and_then(|pages| pages.get(page)) {
        book::page_text(p, left, top, spans);
    }
    book::indicator(p, left, top, page, num_pages);

    let turn = book::arrows(
        p,
        ctx,
        left,
        top,
        book::back_visible(page),
        book::forward_visible(page, num_pages),
    )
    .saturating_add(book::page_key(ctx.input))
    .signum();
    match turn {
        -1 => out.push(InvAction::ButtonClick(BUTTON_PREV_PAGE)),
        1 => out.push(InvAction::ButtonClick(BUTTON_NEXT_PAGE)),
        _ => {}
    }

    let mid = ctx.vw / 2.0;
    if widgets::Button::new(
        mid - book::MENU_BUTTON_SIZE - 2.0,
        book::CONTROLS_TOP,
        book::MENU_BUTTON_SIZE,
        widgets::WIDGET_HEIGHT,
        DONE,
    )
    .draw(p, ctx)
    {
        out.push(InvAction::Close);
    }
    if widgets::Button::new(
        mid + 2.0,
        book::CONTROLS_TOP,
        book::MENU_BUTTON_SIZE,
        widgets::WIDGET_HEIGHT,
        take_book_label(),
    )
    .draw(p, ctx)
    {
        out.push(InvAction::ButtonClick(BUTTON_TAKE_BOOK));
    }

    None
}

fn take_book_label() -> &'static str {
    use std::sync::OnceLock;
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| tooltip::translate("lectern.take_book", &[]))
}
