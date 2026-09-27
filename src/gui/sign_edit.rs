use azalea_core::position::BlockPos;

use crate::gui::painter::Painter;
use crate::gui::render::GuiInput;
use crate::gui::widgets::{self, Button, TextBox, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx};
use crate::session::SignUpdateRequest;

const MAX_LINE_WIDTH: f32 = 90.0;

const LINE_HEIGHT: f32 = 10.0;

const MAX_CHARS: usize = 128;

pub struct SignEditState {
    pos: BlockPos,
    front: bool,
    lines: [String; 4],
    line: usize,
    field: TextBox,
}

impl Default for SignEditState {
    fn default() -> SignEditState {
        SignEditState {
            pos: BlockPos::default(),
            front: true,
            lines: Default::default(),
            line: 0,
            field: TextBox::new(MAX_CHARS, ""),
        }
    }
}

impl SignEditState {
    pub fn open(&mut self, pos: BlockPos, front: bool, lines: [String; 4]) {
        self.pos = pos;
        self.front = front;
        self.lines = lines;
        self.line = 0;
        self.field = TextBox::new(MAX_CHARS, "");
        self.field.max_width = Some(MAX_LINE_WIDTH);
        self.field.strip_formatting = true;
        self.field.set_text(&self.lines[0]);
        self.field.focused = true;
    }

    pub fn submit(&self) -> SignUpdateRequest {
        SignUpdateRequest {
            pos: self.pos,
            front: self.front,
            lines: self.lines.clone(),
        }
    }

    fn switch_line(&mut self, line: usize) {
        self.line = line & 3;
        self.field.set_text(&self.lines[self.line]);
    }

    pub fn handle_input(&mut self, input: &GuiInput, font: &crate::text::Font) {
        if input.up_arrow {
            self.switch_line(self.line.wrapping_sub(1));
            return;
        }
        if input.down_arrow || input.enter {
            self.switch_line(self.line + 1);
            return;
        }
        self.field.handle_input_validated(input, font);
        self.lines[self.line] = self.field.text.clone();
    }
}

pub fn draw(p: &mut Painter, state: &mut SignEditState, ctx: &ScreenCtx) -> Option<Screen> {
    const TITLE: &str = "Edit sign message";
    const TITLE_Y: f32 = 40.0;
    widgets::draw_title(p, ctx.vw, TITLE_Y, TITLE);

    let mid = ctx.vh / 2.0;
    let top = mid - 2.0 * LINE_HEIGHT;
    const PADDING: f32 = 6.0;
    let (px, py) = (
        ((ctx.vw - WIDGET_WIDTH_BIG) / 2.0).floor() - PADDING,
        top - PADDING,
    );
    let (pw, ph) = (
        WIDGET_WIDTH_BIG + 2.0 * PADDING,
        4.0 * LINE_HEIGHT + 2.0 * PADDING,
    );
    p.fill(px - 1.0, py - 1.0, pw + 2.0, ph + 2.0, 0xFFA0_A0A0);
    p.fill(px, py, pw, ph, 0xFF00_0000);

    state.handle_input(ctx.input, &p.atlas.font);
    for (i, line) in state.lines.iter().enumerate() {
        let text = if i == state.line {
            &state.field.text
        } else {
            line
        };
        let w = p.atlas.font.width_str(text);
        let y = top + i as f32 * LINE_HEIGHT;
        p.text_str(text, ((ctx.vw - w) / 2.0).floor(), y, 0xE0E0E0, false);
    }

    let x = ((ctx.vw - WIDGET_WIDTH_BIG) / 2.0).floor();
    let base = (ctx.vh / 4.0).floor();
    let done = Button::new(x, base + 144.0, WIDGET_WIDTH_BIG, WIDGET_HEIGHT, "Done");
    if done.draw(p, ctx) {
        return Some(Screen::None);
    }
    None
}
