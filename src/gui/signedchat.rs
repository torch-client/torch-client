use std::sync::Arc;

use crate::gui::painter::Painter;
use crate::gui::widgets::{self, Button, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx};
use crate::session::SharedMutex;

const TITLE: &str = "Enable signed chat?";

const BODY: &[&str] = &[
    "The server rejected your message because it was not signed.",
    "Signing proves the message came from your Microsoft account,",
    "which also makes it reportable to Mojang.",
    "",
    "This lasts until you disconnect. Options -> Game -> Chat has",
    "the permanent setting.",
];

const YES: &str = "Yes";
const NO: &str = "No";

const GREY: u32 = 0xA0A0A0;

pub fn draw(p: &mut Painter, ctx: &ScreenCtx, shared: &Arc<SharedMutex>) -> Option<Screen> {
    widgets::draw_title(p, ctx.vw, (ctx.vh / 2.0 - 60.0).floor(), TITLE);

    let cy = (ctx.vh / 2.0).floor();
    for (i, line) in BODY.iter().enumerate() {
        let w = p.atlas.font.width_str(line);
        p.text_str(
            line,
            ((ctx.vw - w) / 2.0).floor(),
            cy - 40.0 + i as f32 * crate::text::LINE_HEIGHT,
            GREY,
            true,
        );
    }

    let x = widgets::centered_x(ctx.vw);
    let y = cy + 30.0;
    if Button::new(x, y, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, YES).draw(p, ctx) {
        let mut s = shared.lock().unwrap();
        s.session.chat_signing_session = true;
        s.session.chat_signing_wait = 0;
        return Some(Screen::None);
    }
    if Button::new(x, y + 24.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, NO).draw(p, ctx)
        || ctx.input.escape
    {
        shared.lock().unwrap().session.chat_signing_declined = true;
        return Some(Screen::None);
    }
    None
}
