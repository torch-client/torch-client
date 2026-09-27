use crate::gui::painter::Painter;
use crate::gui::widgets::{self, Button, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx};

const TITLE: &str = "Game Menu";

const RETURN_TO_GAME: &str = "Back to Game";

const OPTIONS: &str = "Options...";

const DISCONNECT: &str = "Disconnect";

const BUTTON_WIDTH_FULL: f32 = 204.0;

const BUTTON_PADDING: f32 = 4.0;

const MENU_PADDING_TOP: f32 = 50.0;

const TITLE_Y: f32 = 40.0;

const GRID_W: f32 = BUTTON_WIDTH_FULL + 2.0 * BUTTON_PADDING;

const BUTTON_TOPS: [f32; 3] = [
    MENU_PADDING_TOP,
    MENU_PADDING_TOP + WIDGET_HEIGHT + BUTTON_PADDING,
    MENU_PADDING_TOP + 2.0 * WIDGET_HEIGHT + 2.0 * BUTTON_PADDING,
];

const GRID_H: f32 = BUTTON_TOPS[2] + WIDGET_HEIGHT;

fn grid_origin(vw: f32, vh: f32) -> (f32, f32) {
    (
        ((vw - GRID_W) * 0.5).floor(),
        ((vh - GRID_H) * 0.25).floor(),
    )
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let (gx, gy) = grid_origin(ctx.vw, ctx.vh);
    let x = gx + BUTTON_PADDING;

    widgets::draw_title(p, ctx.vw, TITLE_Y, TITLE);

    let mut button = |i: usize, label: &'static str| {
        Button::new(
            x,
            gy + BUTTON_TOPS[i],
            BUTTON_WIDTH_FULL,
            WIDGET_HEIGHT,
            label,
        )
        .draw(p, ctx)
    };

    if button(0, RETURN_TO_GAME) {
        state.nav = Some(Screen::None);
    }
    if button(1, OPTIONS) {
        state.options_parent = Screen::Pause;
        state.nav = Some(Screen::Options);
    }
    if button(2, DISCONNECT) {
        state.disconnect = true;
        state.nav = Some(Screen::Title);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_matches_the_java_layout() {
        assert_eq!(BUTTON_TOPS, [50.0, 74.0, 98.0]);
        assert_eq!(GRID_H, 118.0);
        assert_eq!(GRID_W, 212.0);
    }

    #[test]
    fn grid_is_placed_a_quarter_down() {
        let (gx, gy) = grid_origin(854.0, 480.0);
        assert_eq!(gx, 321.0);
        assert_eq!(gx + BUTTON_PADDING + BUTTON_WIDTH_FULL / 2.0, 427.0);
        assert_eq!(gy, ((480.0 - 118.0) * 0.25f32).floor());
    }
}
