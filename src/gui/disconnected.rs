use crate::gui::painter::Painter;
use crate::gui::widgets::{self, Button, HeaderFooter};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::text::{LINE_HEIGHT, Span};

const LAYOUT: HeaderFooter = HeaderFooter::new(HeaderFooter::DEFAULT_H, 42.0);

const WRAP_W: f32 = 300.0;

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) -> Option<Screen> {
    crate::gui::menu::background(p, ctx.vw, ctx.vh);
    widgets::draw_title(p, ctx.vw, LAYOUT.title_y(), "Disconnected");

    let width = WRAP_W.min(ctx.vw - 40.0).max(80.0);
    let lines: Vec<Vec<Span>> = match &state.disconnect_reason {
        Some(reason) if !reason.is_empty() => p.atlas.font.wrap(reason, width),
        _ => p
            .atlas
            .font
            .wrap(&crate::text::parse_formatted("Connection lost"), width),
    };

    let block_h = lines.len() as f32 * LINE_HEIGHT;
    let (_, content_y, _, content_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let mut y = (content_y + (content_h - block_h) * 0.5)
        .floor()
        .max(content_y);
    for line in &lines {
        let w = p.atlas.font.width(line);
        p.text(line, ((ctx.vw - w) * 0.5).floor(), y, true);
        y += LINE_HEIGHT;
    }

    let x = widgets::centered_x(ctx.vw);
    Button::new(
        x,
        LAYOUT.footer_y(ctx.vh),
        widgets::WIDGET_WIDTH_BIG,
        widgets::WIDGET_HEIGHT,
        "Back to title screen",
    )
    .draw(p, ctx)
    .then_some(Screen::Title)
}
