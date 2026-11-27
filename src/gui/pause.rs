use crate::gui::painter::Painter;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx};

mod tr {
    use crate::gui::tooltip::label;

    macro_rules! keys {
        ($($name:ident = $key:literal),* $(,)?) => {
            $(pub fn $name() -> &'static str { label!($key) })*
        };
    }

    keys! {
        title = "menu.game",
        return_to_game = "menu.returnToGame",
        advancements = "gui.advancements",
        statistics = "gui.stats",
        options = "menu.options",
        report_bugs = "menu.reportBugs",
        disconnect = "menu.disconnect",
    }
}

const BUTTON_WIDTH_FULL: f32 = 204.0;

const BUTTON_WIDTH_HALF: f32 = 98.0;

const BUTTON_PADDING: f32 = 4.0;

const MENU_PADDING_TOP: f32 = 50.0;

const TITLE_Y: f32 = 40.0;

const COLUMN_W: f32 = BUTTON_WIDTH_HALF + 2.0 * BUTTON_PADDING;

const GRID_W: f32 = 2.0 * COLUMN_W;

const BUTTON_TOPS: [f32; 4] = [
    MENU_PADDING_TOP,
    MENU_PADDING_TOP + WIDGET_HEIGHT + BUTTON_PADDING,
    MENU_PADDING_TOP + 2.0 * (WIDGET_HEIGHT + BUTTON_PADDING),
    MENU_PADDING_TOP + 3.0 * (WIDGET_HEIGHT + BUTTON_PADDING),
];

const GRID_H: f32 = BUTTON_TOPS[3] + WIDGET_HEIGHT;

fn grid_origin(vw: f32, vh: f32) -> (f32, f32) {
    (
        ((vw - GRID_W) * 0.5).floor(),
        ((vh - GRID_H) * 0.25).floor(),
    )
}

#[cfg(feature = "mobile_ui")]
pub fn side_strip_right(vw: f32, vh: f32) -> f32 {
    grid_origin(vw, vh).0
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) {
    let (gx, gy) = grid_origin(ctx.vw, ctx.vh);

    p.text_centered(tr::title(), ctx.vw / 2.0, TITLE_Y, 0xFFFFFF, true);

    let mut button = |row: usize, column: usize, width: f32, label: &'static str| {
        Button::new(
            gx + column as f32 * COLUMN_W + BUTTON_PADDING,
            gy + BUTTON_TOPS[row],
            width,
            WIDGET_HEIGHT,
            label,
        )
        .draw(p, ctx)
    };

    if button(0, 0, BUTTON_WIDTH_FULL, tr::return_to_game()) {
        state.nav = Some(Screen::None);
    }
    if button(1, 0, BUTTON_WIDTH_HALF, tr::advancements()) {
        state.advancements.parent = Screen::Pause;
        state.nav = Some(Screen::Advancements);
    }
    if button(1, 1, BUTTON_WIDTH_HALF, tr::statistics()) {
        state.nav = Some(Screen::Statistics);
    }
    if button(2, 0, BUTTON_WIDTH_HALF, tr::options()) {
        state.options_parent = Screen::Pause;
        state.nav = Some(Screen::Options);
    }
    if button(2, 1, BUTTON_WIDTH_HALF, tr::report_bugs()) {
        state.credits_parent = Screen::Pause;
        state.nav = Some(Screen::Credits);
    }
    if button(3, 0, BUTTON_WIDTH_FULL, tr::disconnect()) {
        state.disconnect = true;
        state.nav = Some(Screen::Multiplayer);
        #[cfg(all(target_arch = "wasm32", feature = "screen_record"))]
        crate::platform::record::stop();
    }
    #[cfg(all(target_arch = "wasm32", feature = "screen_record"))]
    record_button(p, ctx, gx, gy);
}

#[cfg(all(target_arch = "wasm32", feature = "screen_record"))]
const RECORD_ICON: f32 = 10.0;

#[cfg(all(target_arch = "wasm32", feature = "screen_record"))]
fn record_button(p: &mut Painter, ctx: &ScreenCtx, gx: f32, gy: f32) {
    use crate::platform::record::{self, Status};
    use crate::text::Span;

    let x = gx + BUTTON_PADDING + BUTTON_WIDTH_FULL + BUTTON_PADDING;
    let y = gy + BUTTON_TOPS[3];
    let size = WIDGET_HEIGHT;
    let status = record::status();

    let mut button = Button::new(x, y, size, size, "");
    button.active = !matches!(status, Status::Saving);
    let pressed = button.draw(p, ctx);

    let ix = x + ((size - RECORD_ICON) / 2.0).floor();
    let iy = y + ((size - RECORD_ICON) / 2.0).floor();
    match status {
        Status::Idle => record_dot(p, ix, iy, 0xFFE02424),
        Status::Recording { .. } => p.fill(
            ix + 1.0,
            iy + 1.0,
            RECORD_ICON - 2.0,
            RECORD_ICON - 2.0,
            0xFFE02424,
        ),
        Status::Saving => record_dot(p, ix, iy, 0xFF707070),
    }

    if pressed {
        match status {
            Status::Idle => record::start(),
            Status::Recording { .. } => record::stop(),
            Status::Saving => {}
        }
    }
    if let Some(e) = record::last_error() {
        p.text_centered(
            &e,
            ctx.vw / 2.0,
            y + WIDGET_HEIGHT + BUTTON_PADDING,
            0xFF5555,
            true,
        );
    }

    if ctx.hovering(x, y, size, size)
        && let Some(m) = ctx.mouse()
    {
        let tip = match status {
            Status::Idle => crate::lang::tr!("gui.pause.record_start").to_string(),
            Status::Recording { secs } => {
                crate::lang::trf!(
                    "gui.pause.record_stop",
                    format!("{}:{:02}", secs / 60, secs % 60)
                )
            }
            Status::Saving => crate::lang::tr!("gui.pause.record_saving").to_string(),
        };
        crate::gui::tooltip::draw_lines(
            p,
            &[vec![Span::colored(tip, 0xFFFFFF)]],
            m.x,
            m.y,
            ctx.vw,
            ctx.vh,
        );
    }
}

#[cfg(all(target_arch = "wasm32", feature = "screen_record"))]
fn record_dot(p: &mut Painter, x: f32, y: f32, argb: u32) {
    let r = RECORD_ICON / 2.0;
    for row in 0..RECORD_ICON as u32 {
        let dy = row as f32 + 0.5 - r;
        let half = (r * r - dy * dy).sqrt().round();
        p.fill(x + r - half, y + row as f32, 2.0 * half, 1.0, argb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_matches_the_java_layout() {
        assert_eq!(BUTTON_TOPS, [50.0, 74.0, 98.0, 122.0]);
        assert_eq!(GRID_H, 142.0);
        assert_eq!(GRID_W, 212.0);
        assert_eq!(
            BUTTON_PADDING + BUTTON_WIDTH_FULL,
            COLUMN_W + BUTTON_PADDING + BUTTON_WIDTH_HALF
        );
    }

    #[test]
    fn grid_is_placed_a_quarter_down() {
        let (gx, gy) = grid_origin(854.0, 480.0);
        assert_eq!(gx, 321.0);
        assert_eq!(gx + BUTTON_PADDING + BUTTON_WIDTH_FULL / 2.0, 427.0);
        assert_eq!(gy, ((480.0 - 142.0) * 0.25f32).floor());
    }
}
