use azalea_core::position::BlockPos;

use crate::gui::op_blocks::{cycle, step};
use crate::gui::painter::Painter;
use crate::gui::suggestions;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, TextBox, WIDGET_HEIGHT};
use crate::gui::{Screen, ScreenCtx};
use crate::session::{
    CommandBlockData, CommandBlockMode as Mode, CommandBlockOpen, CommandBlockUpdate, SharedMutex,
};

const MAX_COMMAND: usize = 32500;

const BOX_W: f32 = 300.0;
const PREVIOUS_W: f32 = 276.0;
const PREVIOUS_Y: f32 = 135.0;
const MINECART_PREVIOUS_Y: f32 = 150.0;
const MODES: [Mode; 3] = [Mode::Sequence, Mode::Auto, Mode::Redstone];
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

#[derive(Clone, Copy)]
enum Target {
    Block(BlockPos),
    Minecart(i32),
}

pub struct CommandBlockState {
    target: Target,
    field: suggestions::Field,
    previous: TextBox,
    last_output: String,
    mode: Mode,
    conditional: bool,
    automatic: bool,
    track_output: bool,
    enabled: bool,
}

impl Default for CommandBlockState {
    fn default() -> CommandBlockState {
        CommandBlockState {
            target: Target::Block(BlockPos::default()),
            field: suggestions::Field::new(TextBox::bordered(MAX_COMMAND, "")),
            previous: {
                let mut previous = TextBox::bordered(MAX_COMMAND, "");
                previous.color = UNEDITABLE;
                previous.browser_keyboard = false;
                previous
            },
            last_output: String::new(),
            mode: Mode::Redstone,
            conditional: false,
            automatic: false,
            track_output: true,
            enabled: false,
        }
    }
}

impl CommandBlockState {
    pub fn open(&mut self, open: CommandBlockOpen) {
        *self = CommandBlockState::default();
        match open {
            CommandBlockOpen::Block {
                pos,
                mode,
                conditional,
            } => {
                self.target = Target::Block(pos);
                self.mode = mode;
                self.conditional = conditional;
            }
            CommandBlockOpen::Minecart { id, data } => {
                self.target = Target::Minecart(id);
                self.update_gui(data);
            }
        }
        self.field.input.focused = true;
        self.field.suggest.allow = true;
        self.field.invalidate();
    }

    pub fn update_gui(&mut self, data: CommandBlockData) {
        self.field.input.set_text(&data.command);
        self.track_output = data.track_output;
        self.automatic = data.automatic;
        self.last_output = data.last_output;
        self.update_previous_output();
        self.enabled = true;
    }

    fn update_previous_output(&mut self) {
        self.previous.set_text(if self.track_output {
            self.last_output.as_str()
        } else {
            "-"
        });
    }

    pub fn hide_suggestions(&mut self) -> bool {
        self.field.hide()
    }

    fn submit(&self) -> CommandBlockUpdate {
        let command = self.field.input.text.clone();
        match self.target {
            Target::Block(pos) => CommandBlockUpdate::Block {
                pos,
                command,
                mode: self.mode,
                track_output: self.track_output,
                conditional: self.conditional,
                automatic: self.automatic,
            },
            Target::Minecart(id) => CommandBlockUpdate::Minecart {
                id,
                command,
                track_output: self.track_output,
            },
        }
    }

    fn previous_y(&self) -> f32 {
        match self.target {
            Target::Block(_) => PREVIOUS_Y,
            Target::Minecart(_) => MINECART_PREVIOUS_Y,
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
            match state.target {
                Target::Block(_) => s.session.command_block_pos.is_none(),
                Target::Minecart(_) => s.session.command_minecart.is_none(),
            },
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
        if !state.previous.text.is_empty() {
            let y = state.previous_y();
            state
                .previous
                .handle_mouse(ctx.input, &p.atlas.font, x, y, PREVIOUS_W, WIDGET_HEIGHT);
            if ctx.input.left_click {
                if ctx.hovering(x, y, PREVIOUS_W, WIDGET_HEIGHT) {
                    state.previous.focused = true;
                    state.field.input.focused = false;
                } else if ctx.hovering(x, 50.0, BOX_W, WIDGET_HEIGHT) {
                    state.previous.focused = false;
                    state.field.input.focused = true;
                }
            }
        }
    }
    state.previous.handle_read_only(ctx.input);
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
    state.field.input.draw_formatted(
        p,
        x,
        50.0,
        BOX_W,
        WIDGET_HEIGHT,
        &state.field.suggest.info.spans,
    );

    let previous_y = state.previous_y();
    if !state.previous.text.is_empty() {
        p.text_plain(
            label!("advMode.previousOutput"),
            x + 1.0,
            previous_y - 10.0,
            LABEL,
            true,
        );
        state
            .previous
            .draw(p, x, previous_y, PREVIOUS_W, WIDGET_HEIGHT);
    }

    let output = Button {
        active: state.enabled,
        ..Button::new(
            x + BOX_W - WIDGET_HEIGHT,
            previous_y,
            WIDGET_HEIGHT,
            WIDGET_HEIGHT,
            if state.track_output { "O" } else { "X" },
        )
    };
    let taken = consumed.click || consumed.scroll;
    if cycle(p, ctx, &output) != 0 && !taken {
        state.track_output = !state.track_output;
        state.update_previous_output();
    }

    if matches!(state.target, Target::Block(_)) {
        mode_buttons(p, state, ctx, taken);
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

fn mode_buttons(p: &mut Painter, state: &mut CommandBlockState, ctx: &ScreenCtx, taken: bool) {
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
    let delta = cycle(p, ctx, &mode);
    if delta != 0 && !taken {
        state.mode = step(&MODES, state.mode, delta);
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
    if cycle(p, ctx, &conditional) != 0 && !taken {
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
    if cycle(p, ctx, &automatic) != 0 && !taken {
        state.automatic = !state.automatic;
    }
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
        state.open(CommandBlockOpen::Block {
            pos: BlockPos::new(0, 64, 0),
            mode: Mode::Redstone,
            conditional: false,
        });
        state.field.input.set_text("g");
        state.field.suggest.allow = true;
        state.field.refresh(&font, &shared, SUGGEST, anchor(&ctx));
        assert!(
            state.field.suggest.list.is_some(),
            "a slashless line should still complete"
        );
    }

    #[test]
    fn toggling_track_output_keeps_the_last_output() {
        let mut state = CommandBlockState::default();
        state.open(CommandBlockOpen::Minecart {
            id: 7,
            data: CommandBlockData {
                command: "say hi".to_string(),
                track_output: true,
                last_output: "[00:00:00] hi".to_string(),
                automatic: false,
            },
        });
        state.track_output = false;
        state.update_previous_output();
        assert_eq!(state.previous.text, "-");
        state.track_output = true;
        state.update_previous_output();
        assert_eq!(state.previous.text, "[00:00:00] hi");
    }
}
