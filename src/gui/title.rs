use std::sync::OnceLock;

use crate::gui::menu;
use crate::gui::painter::Painter;
use crate::gui::widgets::{Button, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::platform::time::{SystemTime, UNIX_EPOCH};
use crate::text::texts;

const MULTIPLAYER: &str = "Multiplayer";
const OPTIONS: &str = "Options...";
const QUIT: &str = "Quit Game";
const EDIT_PROFILE: &str = "Edit Profile";
const PROFILE_LABEL_PAD: f32 = 8.0;

const LOGO_W: f32 = 256.0;
const LOGO_H: f32 = 44.0;
const LOGO_Y: f32 = 30.0;
const EDITION_W: f32 = 128.0;
const EDITION_H: f32 = 14.0;
const EDITION_OVERLAP: f32 = 7.0;

const BOTTOM_ROW_STEP: f32 = 36.0;

const HALF_WIDTH: f32 = 98.0;
#[allow(dead_code, reason = "asserted on by this module's layout test")]
const BUTTON_GAP: f32 = 4.0;

use crate::ASSET_VERSION;

fn first_row_y(vh: f32) -> f32 {
    (vh / 4.0).floor() + 48.0
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    if crate::renderer::panorama::absent() {
        menu::background(p, ctx.vw, ctx.vh);
    }

    let logo_x = (ctx.vw / 2.0 - LOGO_W / 2.0).floor();
    p.sprite("title/minecraft", logo_x, LOGO_Y, LOGO_W, LOGO_H);
    p.sprite(
        "title/edition",
        (ctx.vw / 2.0 - EDITION_W / 2.0).floor(),
        LOGO_Y + LOGO_H - EDITION_OVERLAP,
        EDITION_W,
        EDITION_H,
    );

    if let Some(splash) = session_splash() {
        let splash_x = logo_x + LOGO_W * 0.7;
        let splash_y = LOGO_Y + LOGO_H - 3.0;
        p.scaled(1.5, splash_x, splash_y, |p| {
            p.text_str(splash, 0.0, 0.0, 0xFFFF00, true);
        });
    }

    let x = ((ctx.vw - WIDGET_WIDTH_BIG) / 2.0).floor();
    let top = first_row_y(ctx.vh);

    if Button::new(x, top, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, MULTIPLAYER).draw(p, ctx) {
        state.nav = Some(Screen::Multiplayer);
    }

    let bottom = top + BOTTOM_ROW_STEP;
    if Button::new(x, bottom, HALF_WIDTH, WIDGET_HEIGHT, OPTIONS).draw(p, ctx) {
        state.options_parent = Screen::Title;
        state.nav = Some(Screen::Options);
    }
    if Button::new(
        (ctx.vw / 2.0).floor() + 2.0,
        bottom,
        HALF_WIDTH,
        WIDGET_HEIGHT,
        QUIT,
    )
    .draw(p, ctx)
    {
        state.quit = true;
    }

    let version = format!(
        "Minecraft {ASSET_VERSION} (protocol {})",
        azalea_protocol::packets::PROTOCOL_VERSION
    );
    p.text_str(&version, 2.0, ctx.vh - 10.0, 0xFFFFFF, true);

    p.text_str(DISCLAIMER, 2.0, ctx.vh - 20.0, 0xA0A0A0, true);

    #[cfg(all(feature = "mobile_ui", target_arch = "wasm32"))]
    {
        const W: f32 = 90.0;
        let label = if crate::platform::fullscreen::is_fullscreen() {
            "Exit Fullscreen"
        } else {
            "Fullscreen"
        };
        if Button::new(ctx.vw - W - 4.0, 4.0, W, WIDGET_HEIGHT, label).draw(p, ctx) {
            crate::platform::fullscreen::toggle();
        }
    }

    {
        let w = p.atlas.font.width_str(EDIT_PROFILE) + 2.0 * PROFILE_LABEL_PAD;
        if Button::new(
            (ctx.vw - w - 4.0).floor(),
            ctx.vh - WIDGET_HEIGHT - 14.0,
            w,
            WIDGET_HEIGHT,
            EDIT_PROFILE,
        )
        .draw(p, ctx)
        {
            state.profile.open();
            state.nav = Some(Screen::Accounts);
        }
    }

    if crate::platform::address::EAGLER {
        let w = p.atlas.font.width_str(EAGLER_MODE);
        p.text_str(
            EAGLER_MODE,
            (ctx.vw - w - 2.0).floor(),
            ctx.vh - 10.0,
            0xFFAA00,
            true,
        );
    }
}

const DISCLAIMER: &str = "Unofficial. Not affiliated with Mojang or Microsoft.";

const EAGLER_MODE: &str = "Eagler mode (wss:// enabled)";

fn session_splash() -> Option<&'static str> {
    static SPLASH: OnceLock<Option<&'static str>> = OnceLock::new();
    *SPLASH.get_or_init(|| {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        texts::splash_for(seed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_row_matches_the_java_layout() {
        assert_eq!(first_row_y(240.0), 108.0);
        assert_eq!(first_row_y(480.0), 168.0);
    }

    #[test]
    fn the_bottom_row_lines_up_with_the_column() {
        let vw = 854.0;
        let left = ((vw - WIDGET_WIDTH_BIG) / 2.0).floor();
        let right = (vw / 2.0).floor() + 2.0;
        assert_eq!(left + HALF_WIDTH + BUTTON_GAP, right);
        assert_eq!(right + HALF_WIDTH, left + WIDGET_WIDTH_BIG);
    }

    #[test]
    fn the_bottom_row_sits_36_below_the_last_menu_row() {
        let top = first_row_y(240.0);
        assert_eq!(top + BOTTOM_ROW_STEP - (top + WIDGET_HEIGHT), 16.0);
    }
}
