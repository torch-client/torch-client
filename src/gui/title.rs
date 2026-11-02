use std::sync::OnceLock;

use crate::gui::menu;
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, WIDGET_HEIGHT, WIDGET_WIDTH_BIG, centered_x};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::platform::time::{SystemTime, UNIX_EPOCH};
use crate::text::{Span, Style, texts};

#[cfg(feature = "asset_download")]
const LANGUAGE_X_OFFSET: f32 = 124.0;
#[cfg(feature = "asset_download")]
const LANGUAGE_ICON: (f32, f32) = (15.0, 15.0);
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

pub(crate) fn logo(p: &mut Painter, vw: f32, y: f32) {
    p.sprite(
        "title/minecraft",
        (vw / 2.0 - LOGO_W / 2.0).floor(),
        y,
        LOGO_W,
        LOGO_H,
    );
    p.sprite(
        "title/edition",
        (vw / 2.0 - EDITION_W / 2.0).floor(),
        y + LOGO_H - EDITION_OVERLAP,
        EDITION_W,
        EDITION_H,
    );
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    if crate::renderer::panorama::absent() {
        menu::background(p, ctx.vw, ctx.vh);
    }

    let logo_x = (ctx.vw / 2.0 - LOGO_W / 2.0).floor();
    logo(p, ctx.vw, LOGO_Y);

    if let Some(splash) = session_splash() {
        let splash_x = logo_x + LOGO_W * 0.7;
        let splash_y = LOGO_Y + LOGO_H - 3.0;
        p.scaled(1.5, splash_x, splash_y, |p| {
            p.text_str(splash, 0.0, 0.0, 0xFFFF00, true);
        });
    }

    let x = centered_x(ctx.vw);
    let top = first_row_y(ctx.vh);

    #[cfg(feature = "online_mode")]
    crate::client::ban::check_once();
    if Button::new(
        x,
        top,
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        label!("menu.multiplayer"),
    )
    .draw(p, ctx)
    {
        state.nav = Some(Screen::Multiplayer);
        #[cfg(feature = "online_mode")]
        crate::gui::bannotice::before_multiplayer(state);
    }

    let bottom = top + BOTTOM_ROW_STEP;
    #[cfg(feature = "asset_download")]
    if crate::gui::widgets::icon_button(
        p,
        ctx,
        (ctx.vw / 2.0).floor() - LANGUAGE_X_OFFSET,
        bottom,
        WIDGET_HEIGHT,
        WIDGET_HEIGHT,
        "icon/language",
        LANGUAGE_ICON,
    ) {
        state.language.open();
        state.nav = Some(Screen::Language);
    }
    if Button::new(x, bottom, HALF_WIDTH, WIDGET_HEIGHT, label!("menu.options")).draw(p, ctx) {
        state.options_parent = Screen::Title;
        state.nav = Some(Screen::Options);
    }
    if Button::new(
        (ctx.vw / 2.0).floor() + 2.0,
        bottom,
        HALF_WIDTH,
        WIDGET_HEIGHT,
        label!("menu.quit"),
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

    let disclaimer_y = ctx.vh - 20.0;
    let credits_x =
        p.text_str(DISCLAIMER, 2.0, disclaimer_y, 0xA0A0A0, true) + p.atlas.font.width_str(" ");
    if credits_link(p, ctx, credits_x, disclaimer_y) {
        state.credits_parent = Screen::Title;
        state.nav = Some(Screen::Credits);
    }

    #[cfg(all(feature = "update_checker", not(target_arch = "wasm32")))]
    if let Some(tag) = crate::client::update_checker::available() {
        let label = format!("Update available: {tag}");
        if link(p, ctx, &label, 0xFFFF00, 2.0, ctx.vh - 30.0) {
            crate::platform::url::open(crate::client::update_checker::RELEASES_URL);
        }
    }

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
        #[cfg(feature = "tutorial")]
        crate::gui::tutorial::mark(
            crate::gui::tutorial::Spot::EditProfile,
            crate::gui::hud_layout::Rect::new(
                (ctx.vw - w - 4.0).floor(),
                ctx.vh - WIDGET_HEIGHT - 14.0,
                w,
                WIDGET_HEIGHT,
            ),
        );
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

const DISCLAIMER: &str = "Not affiliated with Mojang or Microsoft.";
const CREDITS: &str = "(Open Credits)";

fn credits_link(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32) -> bool {
    #[cfg(feature = "tutorial")]
    crate::gui::tutorial::mark(
        crate::gui::tutorial::Spot::Credits,
        crate::gui::hud_layout::Rect::new(
            x,
            y - 1.0,
            p.atlas.font.width_str(CREDITS),
            crate::text::LINE_HEIGHT + 1.0,
        ),
    );
    link(p, ctx, CREDITS, 0xFFFFFF, x, y)
}

fn link(p: &mut Painter, ctx: &ScreenCtx, text: &str, color: u32, x: f32, y: f32) -> bool {
    let w = p.atlas.font.width_str(text);
    let (hx, hy, hw, hh) = (x, y - 1.0, w, crate::text::LINE_HEIGHT + 1.0);
    let hovered = ctx.hovering(hx, hy, hw, hh);
    let span = Span {
        text: text.to_owned(),
        style: Style {
            color,
            underline: hovered,
            ..Style::default()
        },
    };
    p.text(std::slice::from_ref(&span), x, y, true);
    let clicked = crate::gui::widgets::click_in(ctx, hx, hy, hw, hh);
    #[cfg(feature = "audio")]
    if clicked {
        crate::gui::widgets::play_click();
    }
    clicked
}

const EAGLER_MODE: &str = "Eagler mode (wss:// enabled)";

fn session_splash() -> Option<&'static str> {
    static SPLASH: OnceLock<Option<String>> = OnceLock::new();
    SPLASH
        .get_or_init(|| {
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            texts::splash_for(seed)
        })
        .as_deref()
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
        let left = centered_x(vw);
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
