use crate::gui::ScreenCtx;
use crate::gui::painter::Painter;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::session::SharedMutex;
use std::sync::Arc;

const LEAVE_BED: &str = "Leave Bed";

const BUTTON_WIDTH: f32 = 150.0;

pub fn draw(p: &mut Painter, ctx: &ScreenCtx, shared: &Arc<SharedMutex>) {
    let x = ((ctx.vw - BUTTON_WIDTH) / 2.0).floor();
    let y = (ctx.vh * 0.75).floor();

    if Button::new(x, y, BUTTON_WIDTH, WIDGET_HEIGHT, LEAVE_BED).draw(p, ctx) {
        shared.lock().unwrap().leave_bed_requested = true;
    }
}
