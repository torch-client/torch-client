use crate::gui::painter::Painter;
use crate::gui::widgets::{self, Button, HeaderFooter, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx};
use crate::text::LINE_HEIGHT;

const LAYOUT: HeaderFooter = HeaderFooter::new(HeaderFooter::DEFAULT_H, 42.0);

const GITHUB_URL: &str = "https://github.com/torch-client/torch-client";
const WEBSITE_URL: &str = "https://torchclient.com";
const DISCORD_URL: &str = "https://discord.gg/qqXSHEYXaq";

const LINKS: &[(&str, &str)] = &[
    ("GitHub", GITHUB_URL),
    ("Website", WEBSITE_URL),
    ("Discord", DISCORD_URL),
];

const LINES: &[(&str, u32)] = &[
    ("Torch Client", 0xFFFFFF),
    ("", 0),
    ("Free software, licensed under the", 0xE0E0E0),
    ("GNU Affero General Public License v3.0", 0xE0E0E0),
    ("(AGPL-3.0).", 0xE0E0E0),
    ("", 0),
    ("Not affiliated with Mojang or Microsoft.", 0xA0A0A0),
];

const BUTTON_GAP_Y: f32 = 12.0;
const LINK_WIDTH: f32 = 64.0;
const LINK_GAP: f32 = 4.0;

pub fn draw(p: &mut Painter, ctx: &ScreenCtx, parent: Screen) -> Option<Screen> {
    p.text_centered("Credits", ctx.vw / 2.0, LAYOUT.title_y(), 0xFFFFFF, true);

    let block_h = LINES.len() as f32 * LINE_HEIGHT + BUTTON_GAP_Y + WIDGET_HEIGHT;
    let (_, content_y, _, content_h) = LAYOUT.content_rect(ctx.vw, ctx.vh);
    let mut y = (content_y + (content_h - block_h) * 0.5)
        .floor()
        .max(content_y);
    for &(line, color) in LINES {
        if !line.is_empty() {
            let w = p.atlas.font.width_str(line);
            p.text_str(line, ((ctx.vw - w) * 0.5).floor(), y, color, true);
        }
        y += LINE_HEIGHT;
    }

    let x = widgets::centered_x(ctx.vw);
    let row = y + BUTTON_GAP_Y;
    let mut link_x = x;
    for &(label, url) in LINKS {
        if Button::new(link_x, row, LINK_WIDTH, WIDGET_HEIGHT, label).draw(p, ctx) {
            crate::platform::url::open(url);
        }
        link_x += LINK_WIDTH + LINK_GAP;
    }

    let done = Button::new(
        x,
        LAYOUT.footer_y(ctx.vh),
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        "Done",
    )
    .draw(p, ctx);
    (done || ctx.input.escape).then_some(parent)
}
