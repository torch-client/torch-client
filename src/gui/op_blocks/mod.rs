pub mod jigsaw;
pub mod structure;
pub mod test;
pub mod test_instance;

use crate::gui::command_block::center;
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::gui::{Screen, ScreenCtx, focus};
use crate::session::{GameMasterOpen, GameMasterPacket, SharedMutex};

const LABEL: u32 = 0xA0A0A0;
const TITLE: u32 = crate::text::WHITE;
const TITLE_Y: f32 = 10.0;
const DONE_Y: f32 = 210.0;
const DONE_X: f32 = -154.0;
const CANCEL_X: f32 = 4.0;
const DONE_W: f32 = 150.0;
const RED: u32 = crate::text::FORMAT_COLORS[0xC];

#[derive(Default)]
pub struct OpBlocksState {
    pub structure: structure::State,
    pub jigsaw: jigsaw::State,
    pub test: test::State,
    pub test_instance: test_instance::State,
}

impl OpBlocksState {
    pub fn open(&mut self, request: GameMasterOpen, shared: &SharedMutex) -> Screen {
        match request {
            GameMasterOpen::Structure(pos, edit) => {
                self.structure.open(pos, edit);
                Screen::StructureBlock
            }
            GameMasterOpen::Jigsaw(pos, edit) => {
                self.jigsaw.open(pos, edit);
                Screen::JigsawBlock
            }
            GameMasterOpen::Test(pos, edit) => {
                self.test.open(pos, edit);
                Screen::TestBlock
            }
            GameMasterOpen::TestInstance(pos, edit) => {
                self.test_instance.open(pos, edit, shared);
                Screen::TestInstanceBlock
            }
        }
    }
}

fn title(p: &mut Painter, text: &str, ctx: &ScreenCtx) {
    let w = p.atlas.font.width_str(text);
    p.text_plain(text, center(ctx) - (w / 2.0).floor(), TITLE_Y, TITLE, true);
}

fn field_label(p: &mut Painter, text: &str, x: f32, y: f32) {
    p.text_plain(text, x, y, LABEL, true);
}

fn field_label_right(p: &mut Painter, text: &str, right: f32, y: f32) {
    let w = p.atlas.font.width_str(text);
    field_label(p, text, right - w, y);
}

fn hidden() {
    focus::next(false);
}

fn cycle(p: &mut Painter, ctx: &ScreenCtx, button: &Button) -> i32 {
    if button.draw(p, ctx) {
        return if ctx.input.shift { -1 } else { 1 };
    }
    if button.active
        && ctx.input.scroll != 0.0
        && ctx.hovering(button.x, button.y, button.w, button.h)
    {
        return if ctx.input.scroll > 0.0 { -1 } else { 1 };
    }
    0
}

fn step<T: Copy + PartialEq>(values: &[T], current: T, delta: i32) -> T {
    let index = values.iter().position(|v| *v == current).unwrap_or(0) as i32;
    values[(index + delta).rem_euclid(values.len() as i32) as usize]
}

fn on_off(value: bool) -> &'static str {
    if value {
        label!("options.on")
    } else {
        label!("options.off")
    }
}

fn done_cancel(p: &mut Painter, ctx: &ScreenCtx, done_active: bool) -> (bool, bool) {
    let cx = center(ctx);
    let done = Button {
        active: done_active,
        ..Button::new(
            cx + DONE_X,
            DONE_Y,
            DONE_W,
            WIDGET_HEIGHT,
            label!("gui.done"),
        )
    };
    let done = done.draw(p, ctx);
    let cancel = Button::new(
        cx + CANCEL_X,
        DONE_Y,
        DONE_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    )
    .draw(p, ctx);
    (done, cancel)
}

fn send(shared: &SharedMutex, packet: GameMasterPacket) {
    shared.lock().unwrap().session.game_master_send.push(packet);
}

fn parse_int(s: &str, fallback: i32) -> i32 {
    s.parse().unwrap_or(fallback)
}

fn parse_long(s: &str) -> i64 {
    s.parse().unwrap_or(0)
}

fn parse_float(s: &str, fallback: f32) -> f32 {
    let s = s.trim();
    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
    if unsigned == "NaN" || unsigned == "Infinity" {
        return s.parse().unwrap_or(fallback);
    }
    let body = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(s);
    if !body
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
    {
        return fallback;
    }
    body.parse().unwrap_or(fallback)
}

fn decimal_format(value: f32) -> String {
    let value = f64::from(value);
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        let sign = if value > 0.0 { "" } else { "-" };
        return format!("{sign}\u{221e}");
    }
    let mut out = format!("{value:.4}");
    while out.ends_with('0') && !out.ends_with(".0") {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_formats_like_decimal_format() {
        assert_eq!(decimal_format(1.0), "1.0");
        assert_eq!(decimal_format(0.5), "0.5");
        assert_eq!(decimal_format(0.25), "0.25");
        assert_eq!(decimal_format(0.123_456), "0.1235");
        assert_eq!(decimal_format(0.0), "0.0");
    }

    #[test]
    fn java_number_parsing() {
        assert_eq!(parse_int("+12", 0), 12);
        assert_eq!(parse_int(" 12", 0), 0);
        assert_eq!(parse_float(" 0.5f ", 1.0), 0.5);
        assert_eq!(parse_float("inf", 1.0), 1.0);
        assert_eq!(parse_float("", 1.0), 1.0);
        assert_eq!(parse_long("-3"), -3);
    }

    #[test]
    fn cycling_wraps_both_ways() {
        assert_eq!(step(&[1, 2, 3], 1, -1), 3);
        assert_eq!(step(&[1, 2, 3], 3, 1), 1);
    }
}
