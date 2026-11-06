use azalea_core::position::BlockPos;
use azalea_protocol::packets::game::s_set_test_block::{ServerboundSetTestBlock, TestBlockMode};

use super::{DONE_X, cycle, done_cancel, field_label, hidden, send, step, title};
use crate::blockentities::game_master::{TEST_BLOCK_MODE_VALUES, TestBlockEdit};
use crate::gui::painter::Painter;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, TextBox, WIDGET_HEIGHT};
use crate::gui::{Screen, ScreenCtx};
use crate::session::{GameMasterPacket, SharedMutex};

const MAX_MESSAGE: usize = 128;
const MESSAGE_X: f32 = -152.0;
const MESSAGE_Y: f32 = 80.0;
const MESSAGE_W: f32 = 240.0;
const LABEL_X: f32 = -153.0;
const MESSAGE_LABEL_Y: f32 = 70.0;
const MODE_INFO_Y: f32 = 174.0;
const MODE_Y: f32 = 185.0;
const MODE_W: f32 = 50.0;

pub struct State {
    pos: BlockPos,
    mode: TestBlockMode,
    message: TextBox,
}

impl Default for State {
    fn default() -> State {
        State {
            pos: BlockPos::default(),
            mode: TestBlockMode::Fail,
            message: TextBox::bordered(MAX_MESSAGE, ""),
        }
    }
}

impl State {
    pub fn open(&mut self, pos: BlockPos, edit: TestBlockEdit) {
        *self = State {
            pos,
            mode: edit.mode,
            ..State::default()
        };
        self.message.set_text(&edit.message);
        if self.mode != TestBlockMode::Start {
            self.message.focus();
        }
    }
}

fn mode_label(mode: TestBlockMode) -> &'static str {
    match mode {
        TestBlockMode::Start => label!("test_block.mode.start"),
        TestBlockMode::Log => label!("test_block.mode.log"),
        TestBlockMode::Fail => label!("test_block.mode.fail"),
        TestBlockMode::Accept => label!("test_block.mode.accept"),
    }
}

fn mode_info(mode: TestBlockMode) -> &'static str {
    match mode {
        TestBlockMode::Start => label!("test_block.mode_info.start"),
        TestBlockMode::Log => label!("test_block.mode_info.log"),
        TestBlockMode::Fail => label!("test_block.mode_info.fail"),
        TestBlockMode::Accept => label!("test_block.mode_info.accept"),
    }
}

pub fn draw(
    p: &mut Painter,
    state: &mut State,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
) -> Option<Screen> {
    let cx = super::center(ctx);

    if state.mode != TestBlockMode::Start {
        state
            .message
            .update(p, ctx, cx + MESSAGE_X, MESSAGE_Y, MESSAGE_W, WIDGET_HEIGHT);
    } else {
        hidden();
    }

    let mode = Button::new(
        cx + DONE_X,
        MODE_Y,
        MODE_W,
        WIDGET_HEIGHT,
        mode_label(state.mode),
    );
    let delta = cycle(p, ctx, &mode);
    if delta != 0 {
        state.mode = step(&TEST_BLOCK_MODE_VALUES, state.mode, delta);
    }
    let (done, cancelled) = done_cancel(p, ctx, true);

    title(p, label!("block.minecraft.test_block"), ctx);
    if state.mode != TestBlockMode::Start {
        field_label(
            p,
            label!("test_block.message"),
            cx + LABEL_X,
            MESSAGE_LABEL_Y,
        );
    }
    field_label(p, mode_info(state.mode), cx + LABEL_X, MODE_INFO_Y);

    if done {
        send(
            shared,
            GameMasterPacket::Test(ServerboundSetTestBlock {
                position: state.pos,
                mode: state.mode,
                message: state.message.text.clone(),
            }),
        );
        return Some(Screen::None);
    }
    cancelled.then_some(Screen::None)
}
