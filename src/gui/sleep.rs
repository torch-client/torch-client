use crate::gui::ScreenCtx;
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::session::SharedMutex;
use std::sync::Arc;

const BUTTON_WIDTH: f32 = 200.0;
const BUTTON_BOTTOM_OFFSET: f32 = 40.0;

pub fn draw(p: &mut Painter, ctx: &ScreenCtx, shared: &Arc<SharedMutex>) {
    let x = (ctx.vw / 2.0 - BUTTON_WIDTH / 2.0).floor();
    let y = (ctx.vh - BUTTON_BOTTOM_OFFSET).floor();

    if Button::new(
        x,
        y,
        BUTTON_WIDTH,
        WIDGET_HEIGHT,
        label!("multiplayer.stopSleeping"),
    )
    .draw(p, ctx)
    {
        shared.lock().unwrap().leave_bed_requested = true;
    }
}
