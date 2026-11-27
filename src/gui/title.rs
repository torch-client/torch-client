use std::sync::OnceLock;

use crate::gui::menu;
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, WIDGET_HEIGHT, WIDGET_WIDTH_BIG, centered_x};
use crate::gui::{GuiState, Screen, ScreenCtx};
use crate::lang::Key;
use crate::platform::time::{SystemTime, UNIX_EPOCH};
use crate::text::{Span, Style, texts};

#[cfg(feature = "asset_download")]
const LANGUAGE_X_OFFSET: f32 = 124.0;
#[cfg(feature = "asset_download")]
const LANGUAGE_ICON: (f32, f32) = (15.0, 15.0);
#[cfg(not(singleplayer))]
const SINGLEPLAYER_SOON: Key = Key::of("title.singleplayer_soon");
#[cfg(singleplayer)]
const SINGLEPLAYER_TIP: Key = Key::of("title.singleplayer_tip");
const EDIT_PROFILE: Key = Key::of("title.edit_profile");
const PROFILE_LABEL_PAD: f32 = 8.0;

const LOGO_W: f32 = 256.0;
const LOGO_H: f32 = 44.0;
const LOGO_Y: f32 = 30.0;
const EDITION_W: f32 = 128.0;
const EDITION_H: f32 = 14.0;
const EDITION_OVERLAP: f32 = 7.0;

const BOTTOM_ROW_STEP: f32 = 36.0;
const MENU_ROW_STEP: f32 = 24.0;

const HALF_WIDTH: f32 = 98.0;
#[allow(dead_code, reason = "asserted on by this module's layout test")]
const BUTTON_GAP: f32 = 4.0;

use crate::ASSET_VERSION;

fn first_row_y(vh: f32) -> f32 {
    (vh / 4.0).floor() + 48.0
}

fn last_row_y(vh: f32) -> f32 {
    first_row_y(vh) + MENU_ROW_STEP
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
            p.text_plain(splash, 0.0, 0.0, 0xFFFF00, true);
        });
    }

    let x = centered_x(ctx.vw);
    let last = last_row_y(ctx.vh);

    #[cfg(singleplayer)]
    if Button::new(
        x,
        first_row_y(ctx.vh),
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        label!("menu.singleplayer"),
    )
    .draw(p, ctx)
    {
        state.worlds.select.open();
        state.nav = Some(Screen::SelectWorld);
    }
    #[cfg(not(singleplayer))]
    Button {
        active: false,
        ..Button::new(
            x,
            first_row_y(ctx.vh),
            WIDGET_WIDTH_BIG,
            WIDGET_HEIGHT,
            SINGLEPLAYER_SOON.get(),
        )
    }
    .draw(p, ctx);

    #[cfg(feature = "online_mode")]
    crate::client::ban::check_once();
    if Button::new(
        x,
        last,
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

    let bottom = last + BOTTOM_ROW_STEP;
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
    let quit = Button::new(
        (ctx.vw / 2.0).floor() + 2.0,
        bottom,
        HALF_WIDTH,
        WIDGET_HEIGHT,
        label!("menu.quit"),
    );
    #[cfg(target_arch = "wasm32")]
    let quit = Button {
        active: false,
        ..quit
    };
    if quit.draw(p, ctx) {
        state.quit = true;
    }

    let version = format!(
        "Minecraft {ASSET_VERSION} (protocol {})",
        azalea_protocol::packets::PROTOCOL_VERSION
    );
    p.text_plain(&version, 2.0, ctx.vh - 10.0, 0xFFFFFF, true);

    let disclaimer_y = ctx.vh - 20.0;
    let credits_x = p.text_plain(DISCLAIMER.get(), 2.0, disclaimer_y, 0xA0A0A0, true)
        + p.atlas.font.width_str(" ");
    if credits_link(p, ctx, credits_x, disclaimer_y) {
        state.credits_parent = Screen::Title;
        state.nav = Some(Screen::Credits);
    }

    #[cfg(all(feature = "update_checker", not(target_arch = "wasm32")))]
    if let Some(tag) = crate::client::update_checker::available() {
        let label = crate::lang::trf!("title.update_available", tag);
        if link(p, ctx, &label, 0xFFFF00, false, 2.0, ctx.vh - 30.0) {
            crate::platform::url::open(crate::client::update_checker::RELEASES_URL);
        }
    }

    #[cfg(self_update)]
    {
        #[cfg(feature = "tutorial")]
        let tutorial = crate::gui::tutorial::running();
        #[cfg(not(feature = "tutorial"))]
        let tutorial = false;
        if !tutorial && state.nav.is_none() && crate::client::self_update::take_offer() {
            state.nav = Some(Screen::SelfUpdate);
        }
    }

    #[cfg(all(feature = "mobile_ui", target_arch = "wasm32"))]
    {
        const W: f32 = 90.0;
        if crate::platform::fullscreen::available() {
            let label = if crate::platform::fullscreen::is_fullscreen() {
                crate::lang::tr!("title.exit_fullscreen")
            } else {
                crate::lang::tr!("title.fullscreen")
            };
            if Button::new(ctx.vw - W - 4.0, 4.0, W, WIDGET_HEIGHT, label).draw(p, ctx) {
                crate::platform::fullscreen::toggle();
            }
        }
    }

    {
        let edit_profile = EDIT_PROFILE.get();
        let w = p.atlas.font.width_str(edit_profile) + 2.0 * PROFILE_LABEL_PAD;
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
            edit_profile,
        )
        .draw(p, ctx)
        {
            state.profile.open();
            state.nav = Some(Screen::Accounts);
        }
    }

    if crate::platform::address::EAGLER {
        let eagler_mode = EAGLER_MODE.get();
        let w = p.atlas.font.width_str(eagler_mode);
        p.text_plain(
            eagler_mode,
            (ctx.vw - w - 2.0).floor(),
            ctx.vh - 10.0,
            0xFFAA00,
            true,
        );
    }

    #[cfg(singleplayer)]
    if ctx.hovering(x, first_row_y(ctx.vh), WIDGET_WIDTH_BIG, WIDGET_HEIGHT) {
        crate::gui::worlds::hover_tooltip(p, ctx, SINGLEPLAYER_TIP.get());
    }
}

const DISCLAIMER: Key = Key::of("title.disclaimer");
const CREDITS: Key = Key::of("title.open_credits");

fn credits_link(p: &mut Painter, ctx: &ScreenCtx, x: f32, y: f32) -> bool {
    let credits = CREDITS.get();
    #[cfg(feature = "tutorial")]
    crate::gui::tutorial::mark(
        crate::gui::tutorial::Spot::Credits,
        crate::gui::hud_layout::Rect::new(
            x,
            y - 1.0,
            p.atlas.font.width_str(credits),
            crate::text::LINE_HEIGHT + 1.0,
        ),
    );
    link(p, ctx, credits, 0xFFFFFF, false, x, y)
}

pub(crate) fn link(
    p: &mut Painter,
    ctx: &ScreenCtx,
    text: &str,
    color: u32,
    underline: bool,
    x: f32,
    y: f32,
) -> bool {
    let w = p.atlas.font.width_str(text);
    let (hx, hy, hw, hh) = (x, y - 1.0, w, crate::text::LINE_HEIGHT + 1.0);
    let hovered = ctx.hovering(hx, hy, hw, hh);
    let span = Span {
        text: text.to_owned(),
        style: Style {
            color,
            underline: underline || hovered,
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

const EAGLER_MODE: Key = Key::of("title.eagler_mode");

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

    #[test]
    fn singleplayer_pushes_multiplayer_down_one_row() {
        assert_eq!(first_row_y(240.0), 108.0);
        assert_eq!(last_row_y(240.0), 132.0);
        assert_eq!(last_row_y(240.0) + BOTTOM_ROW_STEP, 168.0);
    }
}
