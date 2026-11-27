use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{self, Button, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx};
use crate::lang::Key;
use crate::session::SharedMutex;

const TITLE: Key = Key::of("gui.signed_chat.title");

const BODY: Key = Key::of("gui.signed_chat.body");

const GREY: u32 = 0xA0A0A0;

pub fn draw(p: &mut Painter, ctx: &ScreenCtx, shared: &SharedMutex) -> Option<Screen> {
    p.text_centered(
        TITLE.get(),
        ctx.vw / 2.0,
        (ctx.vh / 2.0 - 60.0).floor(),
        0xFFFFFF,
        true,
    );

    let cy = (ctx.vh / 2.0).floor();
    for (i, line) in BODY.get().split('\n').enumerate() {
        let w = p.atlas.font.width_str(line);
        p.text_plain(
            line,
            ((ctx.vw - w) / 2.0).floor(),
            cy - 40.0 + i as f32 * crate::text::LINE_HEIGHT,
            GREY,
            true,
        );
    }

    let x = widgets::centered_x(ctx.vw);
    let y = cy + 30.0;
    if Button::new(x, y, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, label!("gui.yes")).draw(p, ctx) {
        let mut s = shared.lock().unwrap();
        s.session.chat_signing_session = true;
        s.session.chat_signing_wait = 0;
        return Some(Screen::None);
    }
    if Button::new(
        x,
        y + 24.0,
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        label!("gui.no"),
    )
    .draw(p, ctx)
        || ctx.input.escape
    {
        shared.lock().unwrap().session.chat_signing_declined = true;
        return Some(Screen::None);
    }
    None
}
