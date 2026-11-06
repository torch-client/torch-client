use azalea_core::position::BlockPos;

use crate::gui::painter::Painter;
use crate::gui::suggestions;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, TextBox, WIDGET_HEIGHT};
use crate::gui::{Screen, ScreenCtx};
use crate::session::{CommandBlockData, CommandBlockMode as Mode, CommandBlockUpdate, SharedMutex};

const MAX_COMMAND: usize = 32500;

const BOX_W: f32 = 300.0;
const PREVIOUS_W: f32 = 276.0;
const PREVIOUS_Y: f32 = 135.0;
const MODE_Y: f32 = 165.0;
const MODE_W: f32 = 100.0;
const BUTTON_W: f32 = 150.0;
const LABEL: u32 = 0xA0A0A0;
const UNEDITABLE: u32 = 0x8F8F8F;

const SUGGEST: suggestions::Style = suggestions::Style {
    limit: 7,
    line_start_offset: 0,
    anchor_to_bottom: false,
    fill: 0x8000_0000,
    bordered: true,
    options: crate::play::commands::Options::COMMAND_ONLY,
};

const SUGGEST_Y: f32 = 72.0;

pub struct CommandBlockState {
    pos: BlockPos,
    field: suggestions::Field,
    previous: TextBox,
    mode: Mode,
    conditional: bool,
    automatic: bool,
    track_output: bool,
    enabled: bool,
}

impl Default for CommandBlockState {
    fn default() -> CommandBlockState {
        CommandBlockState {
            pos: BlockPos::default(),
            field: suggestions::Field::new(TextBox::bordered(MAX_COMMAND, "")),
            previous: {
                let mut previous = TextBox::bordered(MAX_COMMAND, "");
                previous.color = UNEDITABLE;
                previous
            },
            mode: Mode::Redstone,
            conditional: false,
            automatic: false,
            track_output: true,
            enabled: false,
        }
    }
}

impl CommandBlockState {
    pub fn open(&mut self, pos: BlockPos, mode: Mode, conditional: bool) {
        *self = CommandBlockState {
            pos,
            mode,
            conditional,
            ..CommandBlockState::default()
        };
        self.field.input.focused = true;
        self.field.suggest.allow = true;
        self.field.invalidate();
        self.previous.set_text("-");
    }

    pub fn update_gui(&mut self, data: CommandBlockData) {
        self.field.input.set_text(&data.command);
        self.track_output = data.track_output;
        self.automatic = data.automatic;
        self.update_previous_output(&data.last_output);
        self.enabled = true;
    }

    fn update_previous_output(&mut self, last_output: &str) {
        self.previous
            .set_text(if self.track_output { last_output } else { "-" });
    }

    pub fn hide_suggestions(&mut self) -> bool {
        self.field.hide()
    }

    fn submit(&self) -> CommandBlockUpdate {
        CommandBlockUpdate {
            pos: self.pos,
            command: self.field.input.text.clone(),
            mode: self.mode,
            track_output: self.track_output,
            conditional: self.conditional,
            automatic: self.automatic,
        }
    }
}

fn anchor(ctx: &ScreenCtx) -> suggestions::Anchor {
    suggestions::Anchor {
        text_x: box_x(ctx),
        inner_width: BOX_W - 8.0,
        popup_y: SUGGEST_Y,
        usage_y: SUGGEST_Y,
    }
}

pub(in crate::gui) fn center(ctx: &ScreenCtx) -> f32 {
    (ctx.vw / 2.0).floor()
}

fn box_x(ctx: &ScreenCtx) -> f32 {
    center(ctx) - 150.0
}

pub fn draw(
    p: &mut Painter,
    state: &mut CommandBlockState,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
) -> Option<Screen> {
    let (data, block_gone) = {
        let mut s = shared.lock().unwrap();
        (
            s.session.command_block_data.take(),
            s.session.command_block_pos.is_none(),
        )
    };
    if let Some(data) = data {
        state.update_gui(data);
    }
    if block_gone {
        return Some(Screen::None);
    }

    let x = box_x(ctx);

    let consumed = {
        let font = &p.atlas.font;
        state
            .field
            .handle_input(ctx.input, font, SUGGEST, anchor(ctx))
    };
    if !consumed.click {
        state
            .field
            .input
            .handle_mouse(ctx.input, &p.atlas.font, x, 50.0, BOX_W, WIDGET_HEIGHT);
    }
    state.field.handle_edit(ctx.input);
    state
        .field
        .refresh(&p.atlas.font, shared, SUGGEST, anchor(ctx));

    p.text_centered(
        label!("advMode.setCommand"),
        ctx.vw / 2.0,
        20.0,
        0xFFFFFF,
        true,
    );
    p.text_plain(label!("advMode.command"), x + 1.0, 40.0, LABEL, true);
    if state.field.input.suggestion != state.field.suggest.ghost {
        state.field.input.suggestion = state.field.suggest.ghost.clone();
    }
    state.field.input.draw(p, x, 50.0, BOX_W, WIDGET_HEIGHT);

    if !state.previous.text.is_empty() {
        p.text_plain(
            label!("advMode.previousOutput"),
            x + 1.0,
            PREVIOUS_Y - 10.0,
            LABEL,
            true,
        );
        state
            .previous
            .draw(p, x, PREVIOUS_Y, PREVIOUS_W, WIDGET_HEIGHT);
    }

    let output = Button {
        active: state.enabled,
        ..Button::new(
            x + BOX_W - WIDGET_HEIGHT,
            PREVIOUS_Y,
            WIDGET_HEIGHT,
            WIDGET_HEIGHT,
            if state.track_output { "O" } else { "X" },
        )
    };
    if output.draw(p, ctx) && !consumed.click {
        state.track_output = !state.track_output;
        let last = state.previous.text.clone();
        state.update_previous_output(&last);
    }

    let mode = Button {
        active: state.enabled,
        ..Button::new(
            center(ctx) - 154.0,
            MODE_Y,
            MODE_W,
            WIDGET_HEIGHT,
            match state.mode {
                Mode::Sequence => label!("advMode.mode.sequence"),
                Mode::Auto => label!("advMode.mode.auto"),
                Mode::Redstone => label!("advMode.mode.redstone"),
            },
        )
    };
    if mode.draw(p, ctx) && !consumed.click {
        state.mode = match state.mode {
            Mode::Sequence => Mode::Auto,
            Mode::Auto => Mode::Redstone,
            Mode::Redstone => Mode::Sequence,
        };
    }
    let conditional = Button {
        active: state.enabled,
        ..Button::new(
            center(ctx) - 50.0,
            MODE_Y,
            MODE_W,
            WIDGET_HEIGHT,
            if state.conditional {
                label!("advMode.mode.conditional")
            } else {
                label!("advMode.mode.unconditional")
            },
        )
    };
    if conditional.draw(p, ctx) && !consumed.click {
        state.conditional = !state.conditional;
    }
    let automatic = Button {
        active: state.enabled,
        ..Button::new(
            center(ctx) + 54.0,
            MODE_Y,
            MODE_W,
            WIDGET_HEIGHT,
            if state.automatic {
                label!("advMode.mode.autoexec.bat")
            } else {
                label!("advMode.mode.redstoneTriggered")
            },
        )
    };
    if automatic.draw(p, ctx) && !consumed.click {
        state.automatic = !state.automatic;
    }

    let button_y = (ctx.vh / 4.0).floor() + 132.0;
    let done = Button {
        active: state.enabled,
        ..Button::new(
            center(ctx) - 154.0,
            button_y,
            BUTTON_W,
            WIDGET_HEIGHT,
            label!("gui.done"),
        )
    };
    let cancel = Button::new(
        center(ctx) + 4.0,
        button_y,
        BUTTON_W,
        WIDGET_HEIGHT,
        label!("gui.cancel"),
    );
    let confirmed = ctx.input.enter && state.enabled;
    let done_pressed = (done.draw(p, ctx) && !consumed.click) || confirmed;
    let cancelled = cancel.draw(p, ctx) && !consumed.click;

    suggestions::draw(p, &mut state.field, ctx, SUGGEST, anchor(ctx));

    if done_pressed {
        shared.lock().unwrap().session.command_block_update = Some(state.submit());
        return Some(Screen::None);
    }
    cancelled.then_some(Screen::None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::render::GuiInput;

    #[test]
    fn slashless_line_completes() {
        let shared = std::sync::Arc::new(SharedMutex::default());
        shared.lock().unwrap().session.command_tree =
            Some(std::sync::Arc::new(crate::gui::chat::tests::preview_tree()));
        let (font, _) = crate::text::Font::load(&crate::assets_root());
        let input = GuiInput::default();
        let ctx = ScreenCtx {
            input: &input,
            vw: 427.0,
            vh: 240.0,
        };
        let mut state = CommandBlockState::default();
        state.open(BlockPos::new(0, 64, 0), Mode::Redstone, false);
        state.field.input.set_text("g");
        state.field.suggest.allow = true;
        state.field.refresh(&font, &shared, SUGGEST, anchor(&ctx));
        assert!(
            state.field.suggest.list.is_some(),
            "a slashless line should still complete"
        );
    }
}
