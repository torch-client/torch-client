use crate::gui::painter::Painter;
use crate::gui::widgets::{self, WIDGET_HEIGHT};
use crate::gui::{ScreenCtx, tooltip};
use crate::text::{LINE_HEIGHT, Span, Style};

pub(crate) const SPACING: f32 = 8.0;
const MESSAGE_MARGIN: f32 = 50.0;
const BUTTON_SPACING: f32 = 4.0;
const BUTTON_PADDING_TOP: f32 = 16.0;
const LINK_BUTTON_WIDTH: f32 = 100.0;
const WARNING_COLOR: u32 = 0xFFCC_CC;
const MAX_MESSAGE_ROWS: usize = 15;

pub(crate) fn draw_box(
    p: &mut Painter,
    ctx: &ScreenCtx,
    title: &str,
    message: &str,
    extra: Option<&str>,
    buttons: &[String],
    button_w: f32,
) -> Option<usize> {
    let max_message = (ctx.vw - MESSAGE_MARGIN).max(1.0);
    let message_lines: Vec<Vec<Span>> = message
        .split('\n')
        .flat_map(|paragraph| {
            let spans = [Span {
                text: paragraph.to_string(),
                style: Style::default(),
            }];
            p.atlas.font.wrap(&spans, max_message)
        })
        .take(MAX_MESSAGE_ROWS)
        .collect();

    let extra_h = extra.map(|_| SPACING + LINE_HEIGHT).unwrap_or(0.0);
    let total_h = LINE_HEIGHT
        + SPACING
        + message_lines.len() as f32 * LINE_HEIGHT
        + extra_h
        + SPACING
        + BUTTON_PADDING_TOP
        + WIDGET_HEIGHT;
    let mut y = ((ctx.vh - total_h) / 2.0).floor();

    let title_w = p.atlas.font.width_str(title);
    p.text_str(title, ((ctx.vw - title_w) / 2.0).floor(), y, 0xFFFFFF, true);
    y += LINE_HEIGHT + SPACING;
    for line in &message_lines {
        let w = p.atlas.font.width(line);
        p.text(line, ((ctx.vw - w) / 2.0).floor(), y, true);
        y += LINE_HEIGHT;
    }
    if let Some(extra) = extra {
        y += SPACING;
        let w = p.atlas.font.width_str(extra);
        p.text_str(extra, ((ctx.vw - w) / 2.0).floor(), y, WARNING_COLOR, true);
        y += LINE_HEIGHT;
    }
    y += SPACING + BUTTON_PADDING_TOP;

    let count = buttons.len() as f32;
    let total_w = button_w * count + BUTTON_SPACING * (count - 1.0);
    let mut x = ((ctx.vw - total_w) / 2.0).floor();
    let mut chosen = None;
    for (i, label) in buttons.iter().enumerate() {
        let spans = [Span {
            text: label.clone(),
            style: Style::default(),
        }];
        if widgets::draw_button_spans(p, ctx, x, y, button_w, WIDGET_HEIGHT, &spans, true) {
            chosen = Some(i);
        }
        x += button_w + BUTTON_SPACING;
    }
    chosen
}

pub(crate) fn draw(p: &mut Painter, ctx: &ScreenCtx, url: &str) -> bool {
    crate::gui::screens::dim_background(p, ctx.vw, ctx.vh);
    let buttons = [
        tooltip::translate("gui.yes", &[]),
        tooltip::translate("chat.copy", &[]),
        tooltip::translate("gui.no", &[]),
    ];
    let chosen = draw_box(
        p,
        ctx,
        &tooltip::translate("chat.link.confirm", &[]),
        url,
        Some(&tooltip::translate("chat.link.warning", &[])),
        &buttons,
        LINK_BUTTON_WIDTH,
    );
    match chosen {
        Some(0) => crate::platform::url::open(url),
        Some(1) => crate::gui::render::clipboard_set(url),
        Some(_) => {}
        None => return false,
    }
    true
}

pub(crate) fn masked(input: &crate::gui::render::GuiInput) -> crate::gui::render::GuiInput {
    let mut input = input.clone();
    input.mouse = None;
    input.left_click = false;
    input.left_down = false;
    input.typed.clear();
    input
}
