use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, WIDGET_HEIGHT};
use crate::gui::{GuiState, Screen, ScreenCtx};
use azalea::loading::LevelLoadingReason;

const TEXT_ABOVE_CENTRE: f32 = 50.0;
const BUTTON_W: f32 = 98.0;
const BUTTON_GAP: f32 = 8.0;
const BUTTON_BOTTOM: f32 = 28.0;
const END_PORTAL_FILL: u32 = 0xFF00_0000;
const TICKS_PER_SECOND: f32 = 20.0;

#[derive(Default)]
pub struct State {
    pub shown_id: u64,
    pub reason: LevelLoadingReason,
    pub opened_at: Option<crate::platform::time::Instant>,
    pub connecting: bool,
    pub transferring: bool,
}

pub const SHOWN_AT_MOST: std::time::Duration = std::time::Duration::from_secs(3);

impl State {
    pub fn open(&mut self, id: u64, reason: LevelLoadingReason) {
        self.shown_id = id;
        self.reason = reason;
        self.opened_at = Some(crate::platform::time::Instant::now());
        self.connecting = false;
    }

    pub fn begin_connect(&mut self) {
        self.connecting = true;
        self.transferring = false;
        self.reason = LevelLoadingReason::Other;
        self.opened_at = None;
    }

    pub fn begin_transfer(&mut self) {
        self.begin_connect();
        self.transferring = true;
    }

    pub fn overdue(&self) -> bool {
        self.opened_at
            .is_some_and(|at| at.elapsed() >= SHOWN_AT_MOST)
    }
}

pub fn draw(p: &mut Painter, state: &mut GuiState, ctx: &ScreenCtx) -> Option<Screen> {
    match state.level_loading.reason {
        LevelLoadingReason::NetherPortal => {
            let frame = (ctx.input.time * TICKS_PER_SECOND) as u64;
            crate::gui::screen_effects::portal_sprite(p, frame, 0xFFFF_FFFF, ctx.vw, ctx.vh);
        }
        LevelLoadingReason::EndPortal => p.fill(0.0, 0.0, ctx.vw, ctx.vh, END_PORTAL_FILL),
        LevelLoadingReason::Other => crate::gui::menu::background(p, ctx.vw, ctx.vh),
    }

    let x_center = (ctx.vw / 2.0).floor();
    let text_top = (ctx.vh / 2.0).floor() - TEXT_ABOVE_CENTRE;
    let text = if !state.level_loading.connecting {
        label!("multiplayer.downloadingTerrain")
    } else if state.level_loading.transferring {
        label!("connect.transferring")
    } else {
        label!("connect.connecting")
    };
    p.text_centered(text, x_center, text_top, 0xFFFFFF, true);

    let y = ctx.vh - BUTTON_BOTTOM;
    let left = (x_center - BUTTON_W - BUTTON_GAP / 2.0).floor();
    let hide = Button::new(
        left,
        y,
        BUTTON_W,
        WIDGET_HEIGHT,
        label!("menu.returnToGame"),
    )
    .draw(p, ctx);
    let disconnect = Button::new(
        left + BUTTON_W + BUTTON_GAP,
        y,
        BUTTON_W,
        WIDGET_HEIGHT,
        label!("menu.disconnect"),
    )
    .draw(p, ctx);

    if disconnect {
        state.disconnect = true;
        state.level_loading.connecting = false;
        return Some(Screen::Multiplayer);
    }
    let hidden = hide || ctx.input.escape;
    if hidden {
        state.level_loading.connecting = false;
    }
    hidden.then_some(Screen::None)
}
