use crate::gui::book::{self, ReadOverlay};
use crate::gui::confirmcommand::{Accepted, Answer};
use crate::gui::{Screen, confirmlink, tooltip, widgets};
use crate::play::dialog::Click;
use crate::session::{Gamemode, SharedMutex};
use crate::text::events::Events;

use super::{GuiState, InvAction, Painter, ScreenCtx, SlotStack, Snapshot};

const BUTTON_PREV_PAGE: u8 = 1;
const BUTTON_NEXT_PAGE: u8 = 2;
const BUTTON_TAKE_BOOK: u8 = 3;
const BUTTON_PAGE_JUMP_FIRST: i32 = 100;

pub fn draw(
    p: &mut Painter,
    state: &mut GuiState,
    ctx: &ScreenCtx,
    snap: &Snapshot,
    out: &mut Vec<InvAction>,
) -> Option<SlotStack> {
    let (left, top) = book::frame(p, ctx);

    let lectern_book = snap
        .menu_slots
        .first()
        .and_then(|s| s.extra.book.as_deref());
    let pages = lectern_book.map(|b| &*b.pages);
    let num_pages = pages.map_or(0, |p| p.len());

    let page = book::clamp_page(snap.container_data[0].max(0) as usize, num_pages);

    const NO_EVENTS: Events = None;
    let spans = pages
        .and_then(|pages| pages.get(page))
        .map_or(&[][..], Vec::as_slice);
    let events = lectern_book.map_or(&NO_EVENTS, |b| b.page_events(page));
    let read = book::read(
        p,
        ctx,
        &mut state.book.page_view,
        &state.book.overlay,
        left,
        top,
        page,
        spans,
        events,
    );
    book::indicator(p, left, top, page, num_pages);
    let wctx = ctx.with_input(&read.input);

    let turn = book::arrows(
        p,
        &wctx,
        left,
        top,
        book::back_visible(page),
        book::forward_visible(page, num_pages),
        false,
    )
    .saturating_add(book::page_key(wctx.input))
    .signum();
    match turn {
        -1 => out.push(InvAction::ButtonClick(BUTTON_PREV_PAGE)),
        1 => out.push(InvAction::ButtonClick(BUTTON_NEXT_PAGE)),
        _ => {}
    }

    let may_build = !matches!(snap.gamemode, Gamemode::Adventure | Gamemode::Spectator);
    if may_build {
        let mid = ctx.vw / 2.0;
        if widgets::Button::new(
            mid - book::MENU_BUTTON_SIZE - 2.0,
            book::CONTROLS_TOP,
            book::MENU_BUTTON_SIZE,
            widgets::WIDGET_HEIGHT,
            tooltip::label!("gui.done"),
        )
        .draw(p, &wctx)
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
        .draw(p, &wctx)
        {
            out.push(InvAction::ButtonClick(BUTTON_TAKE_BOOK));
        }
    } else if widgets::Button::new(
        ((ctx.vw - widgets::WIDGET_WIDTH_BIG) / 2.0).floor(),
        book::CONTROLS_TOP,
        widgets::WIDGET_WIDTH_BIG,
        widgets::WIDGET_HEIGHT,
        tooltip::label!("gui.done"),
    )
    .draw(p, &wctx)
    {
        out.push(InvAction::Close);
    }

    if matches!(state.book.overlay, ReadOverlay::None) {
        read.tooltip(p, ctx, state.advanced_tooltips);
    }
    if let Some(click) = read.click {
        state.book.lectern_click = Some(click);
    }

    draw_overlay(p, state, ctx, out);

    None
}

pub fn click(
    state: &mut GuiState,
    click: Click,
    snap: &Snapshot,
    shared: &SharedMutex,
    out: &mut Vec<InvAction>,
) {
    match click {
        Click::ChangePage(target) => {
            let target = target.saturating_sub(1);
            if target != i32::from(snap.container_data[0]) {
                let id = BUTTON_PAGE_JUMP_FIRST
                    .saturating_add(target)
                    .clamp(0, u8::MAX as i32);
                out.push(InvAction::ButtonClick(id as u8));
            }
        }
        Click::RunCommand(command) => {
            let command = book::trim_command(&command);
            match crate::gui::confirmcommand::check(&command, shared) {
                Some(prompt) => state.book.overlay = ReadOverlay::Command(prompt),
                None => {
                    out.push(InvAction::Close);
                    out.push(InvAction::RunCommand(command));
                }
            }
        }
        other => book::shared_click(other, &mut state.book.overlay, shared),
    }
}

fn draw_overlay(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx, out: &mut Vec<InvAction>) {
    match &state.book.overlay {
        ReadOverlay::None => {}
        ReadOverlay::Link(url) => {
            if confirmlink::draw(p, ctx, url) {
                state.book.overlay = ReadOverlay::None;
            }
        }
        ReadOverlay::Command(prompt) => {
            let Some(answer) = prompt.draw(p, ctx, false, true) else {
                return;
            };
            let accepted = matches!(answer, Answer::Accept).then(|| prompt.accept(true));
            state.book.overlay = ReadOverlay::None;
            match accepted {
                Some(Accepted::Run(command)) => {
                    out.push(InvAction::Close);
                    out.push(InvAction::RunCommand(command));
                }
                Some(Accepted::Chat(text)) => {
                    state.chat.open(&text);
                    state.nav = Some(Screen::Chat);
                }
                None | Some(Accepted::Copied) => out.push(InvAction::Close),
            }
        }
    }
}

fn take_book_label() -> &'static str {
    tooltip::label!("lectern.take_book")
}
