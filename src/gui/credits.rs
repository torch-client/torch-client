use crate::gui::painter::Painter;
use crate::gui::widgets::{self, Button, HeaderFooter, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx, title};
use crate::lang::Key;
use crate::text::LINE_HEIGHT;

const LAYOUT: HeaderFooter = HeaderFooter::new(HeaderFooter::DEFAULT_H, 42.0);

const GITHUB_URL: &str = "https://github.com/torch-client/torch-client";
const WEBSITE_URL: &str = "https://torchclient.com";
const WEBSITE_LABEL: &str = "torchclient.com";
const LINK_BLUE: u32 = 0x5555FF;
const DISCORD_URL: &str = "https://discord.gg/qqXSHEYXaq";
const MATRIX_URL: &str = "https://matrix.to/#/#torch-client:matrix.org";

const BUTTONS: [(&str, &str); 3] = [
    ("GitHub", GITHUB_URL),
    ("Discord", DISCORD_URL),
    ("Matrix", MATRIX_URL),
];

const LINES: &[(Key, &str, u32)] = &[
    (Key::NONE, "Torch Client", 0xFFFFFF),
    (Key::NONE, "", 0),
    (Key::of("credits.licensed_under"), "", 0xE0E0E0),
    (
        Key::NONE,
        "GNU Affero General Public License v3.0",
        0xE0E0E0,
    ),
    (Key::NONE, "(AGPL-3.0).", 0xE0E0E0),
    (Key::NONE, "", 0),
    (Key::of("credits.disclaimer"), "", 0xA0A0A0),
];

const BUTTON_GAP_Y: f32 = 12.0;
const BUTTON_WIDTH: f32 = 64.0;
const BUTTON_GAP_X: f32 = 4.0;
const WEBSITE_GAP_Y: f32 = 8.0;

pub fn draw(p: &mut Painter, ctx: &ScreenCtx, parent: Screen) -> Option<Screen> {
    p.text_centered(
        crate::lang::tr!("credits.title"),
        ctx.vw / 2.0,
        LAYOUT.title_y(),
        0xFFFFFF,
        true,
    );

    let block_h = LINES.len() as f32 * LINE_HEIGHT
        + BUTTON_GAP_Y
        + WIDGET_HEIGHT
        + WEBSITE_GAP_Y
        + LINE_HEIGHT;
    let (_, content_y, _, content_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let mut y = (content_y + (content_h - block_h) * 0.5)
        .floor()
        .max(content_y);
    for &(key, line, color) in LINES {
        let line = key.or(line);
        if !line.is_empty() {
            let w = p.atlas.font.width_str(line);
            p.text_plain(line, ((ctx.vw - w) * 0.5).floor(), y, color, true);
        }
        y += LINE_HEIGHT;
    }

    let x = widgets::centered_x(ctx.vw);
    let row = y + BUTTON_GAP_Y;
    let mut button_x = x;
    for (label, url) in BUTTONS {
        if Button::new(button_x, row, BUTTON_WIDTH, WIDGET_HEIGHT, label).draw(p, ctx) {
            crate::platform::url::open(url);
        }
        button_x += BUTTON_WIDTH + BUTTON_GAP_X;
    }

    let w = p.atlas.font.width_str(WEBSITE_LABEL);
    let website_x = ((ctx.vw - w) * 0.5).floor();
    let website_y = row + WIDGET_HEIGHT + WEBSITE_GAP_Y;
    if title::link(p, ctx, WEBSITE_LABEL, LINK_BLUE, true, website_x, website_y) {
        crate::platform::url::open(WEBSITE_URL);
    }

    let done = Button::new(
        x,
        LAYOUT.footer_y(ctx.vh),
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        crate::lang::tr!("credits.done"),
    )
    .draw(p, ctx);
    (done || ctx.input.escape).then_some(parent)
}
