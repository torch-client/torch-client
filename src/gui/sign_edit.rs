use azalea_core::position::BlockPos;

use crate::blockentities::feed::{BlockEntityData, INHERIT_COLOR, SignFace};
use crate::blockentities::text::{self, SignMetrics};
use crate::gui::painter::Painter;
use crate::gui::render::{EditKey, GuiInput};
use crate::gui::tooltip::label;
use crate::gui::widgets::{self, Button, TextBox, WIDGET_HEIGHT, WIDGET_WIDTH_BIG};
use crate::gui::{Screen, ScreenCtx};
use crate::platform::time::Instant;
use crate::session::{SharedMutex, SignEditKind, SignEditRequest, SignUpdateRequest};
use crate::text::{Span, Style};

pub(crate) const SIGN_TEXTURE_WIDTH: u32 = 64;

pub(crate) const BOARD_UV: [u32; 4] = [2, 2, 24, 12];

pub(crate) const STICK_UV: [u32; 4] = [2, 16, 2, 14];

pub(crate) fn hanging_sprite(wood: &str) -> String {
    format!("sign_edit/hanging/{wood}")
}

pub(crate) fn board_sprite(wood: &str) -> String {
    format!("sign_edit/board/{wood}")
}

pub(crate) fn stick_sprite(wood: &str) -> String {
    format!("sign_edit/stick/{wood}")
}

const MAX_CHARS: usize = 128;

const MODEL_SCALE: f32 = 62.500_004;

const MODEL_BOTTOM: f32 = 168.0;

const MODEL_LIFT: f32 = -0.75;

enum Background {
    Standing {
        board: String,
        stick: Option<String>,
    },
    Hanging(String),
}

pub struct SignPreview {
    pub pos: [i32; 3],
    front: bool,
    lines: [Option<String>; 4],
    pub generation: u64,
    until_rev: Option<u64>,
}

impl SignPreview {
    pub fn applies(&self, rev: u64) -> bool {
        self.until_rev.is_none_or(|until| until == rev)
    }

    pub fn apply(&self, data: &BlockEntityData) -> BlockEntityData {
        let mut sign = match data {
            BlockEntityData::Sign(sign) => sign.clone(),
            _ => Default::default(),
        };
        let face = if self.front {
            &mut sign.front
        } else {
            &mut sign.back
        };
        for (line, edit) in face.lines.iter_mut().zip(&self.lines) {
            let Some(edit) = edit else { continue };
            line.clear();
            if !edit.is_empty() {
                line.push(Span {
                    text: edit.clone(),
                    style: Style::colored(INHERIT_COLOR),
                });
            }
        }
        BlockEntityData::Sign(sign)
    }
}

pub struct SignEditState {
    pos: BlockPos,
    front: bool,
    kind: SignEditKind,
    metrics: SignMetrics,
    background: Background,
    face: SignFace,
    opening: [String; 4],
    lines: [String; 4],
    line: usize,
    field: TextBox,
    opened_at: Instant,
    preview: Option<SignPreview>,
    next_gen: u64,
}

impl Default for SignEditState {
    fn default() -> SignEditState {
        SignEditState {
            pos: BlockPos::default(),
            front: true,
            kind: SignEditKind::Standing { wall: false },
            metrics: text::SignKind::Standing.metrics(),
            background: Background::Hanging(String::new()),
            face: SignFace::default(),
            opening: Default::default(),
            lines: Default::default(),
            line: 0,
            field: TextBox::new(MAX_CHARS, ""),
            opened_at: Instant::now(),
            preview: None,
            next_gen: 0,
        }
    }
}

impl SignEditState {
    pub fn open(&mut self, req: SignEditRequest) {
        self.pos = req.pos;
        self.front = req.front;
        self.kind = req.kind;
        self.metrics = req.kind.sign_kind().metrics();
        self.background = match req.kind {
            SignEditKind::Standing { wall } => Background::Standing {
                board: board_sprite(req.wood),
                stick: (!wall).then(|| stick_sprite(req.wood)),
            },
            SignEditKind::Hanging => Background::Hanging(hanging_sprite(req.wood)),
        };
        self.opening = std::array::from_fn(|i| {
            req.face.lines[i]
                .iter()
                .map(|span| span.text.as_str())
                .collect()
        });
        self.lines = self.opening.clone();
        self.face = req.face;
        self.line = 0;
        self.field = TextBox::new(MAX_CHARS, "");
        self.field.max_width = Some(self.metrics.max_width);
        self.field.strip_formatting = true;
        self.field.set_text(&self.lines[0]);
        self.field.focused = true;
        self.opened_at = Instant::now();
        self.preview = None;
    }

    pub fn close(&mut self, rev: Option<u64>) -> SignUpdateRequest {
        match rev {
            Some(rev) => {
                if let Some(preview) = &mut self.preview {
                    preview.until_rev = Some(rev);
                }
            }
            None => self.preview = None,
        }
        SignUpdateRequest {
            pos: self.pos,
            front: self.front,
            lines: self.lines.clone(),
        }
    }

    pub fn key(&self) -> [i32; 3] {
        [self.pos.x, self.pos.y, self.pos.z]
    }

    pub fn preview(&self) -> Option<&SignPreview> {
        self.preview.as_ref()
    }

    pub fn handle_input(&mut self, input: &GuiInput, font: &crate::text::Font) {
        if input.left_click {
            crate::platform::keyboard::note_tap(true);
        }
        let SignEditState {
            field, lines, line, ..
        } = self;
        let mut switched = false;
        let changed = field.handle_edits_validated(input, font, |field, key| {
            let step = match key {
                EditKey::Up => 3,
                EditKey::Down | EditKey::Enter => 1,
                _ => return false,
            };
            lines[*line].clone_from(&field.text);
            *line = (*line + step) & 3;
            field.set_text(&lines[*line]);
            field.push_keyboard();
            switched = true;
            true
        });
        if changed || switched {
            lines[*line].clone_from(&field.text);
        }
        if changed {
            self.refresh_preview();
        }
    }

    fn refresh_preview(&mut self) {
        let edits: [Option<String>; 4] = std::array::from_fn(|i| {
            (self.lines[i] != self.opening[i]).then(|| self.lines[i].clone())
        });
        if edits.iter().all(Option::is_none) {
            self.preview = None;
            return;
        }
        self.next_gen += 1;
        self.preview = Some(SignPreview {
            pos: self.key(),
            front: self.front,
            lines: edits,
            generation: self.next_gen,
            until_rev: None,
        });
    }
}

fn model_rect(cx: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> (f32, f32, f32, f32) {
    let unit = MODEL_SCALE / 16.0;
    let top = MODEL_BOTTOM + (MODEL_LIFT * 16.0 + y0) * unit;
    (cx + x0 * unit, top, (x1 - x0) * unit, (y1 - y0) * unit)
}

pub fn draw(
    p: &mut Painter,
    state: &mut SignEditState,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
) -> Option<Screen> {
    if shared.lock().unwrap().session.sign_edit_open_pos != Some(state.pos) {
        return Some(Screen::None);
    }

    let (title, offset_y, text_scale) = match state.kind {
        SignEditKind::Standing { .. } => (label!("sign.edit"), 90.0, 0.976_562_8),
        SignEditKind::Hanging => (label!("hanging_sign.edit"), 125.0, 1.0),
    };
    widgets::draw_title(p, ctx.vw, 40.0, title);

    let cx = ctx.vw / 2.0;
    match &state.background {
        Background::Standing { board, stick } => {
            let cx = cx.floor();
            let (x, y, w, h) = model_rect(cx, -12.0, -14.0, 12.0, -2.0);
            p.sprite(board, x, y, w, h);
            if let Some(stick) = stick {
                let (x, y, w, h) = model_rect(cx, -1.0, -2.0, 1.0, 12.0);
                p.sprite(stick, x, y, w, h);
            }
        }
        Background::Hanging(sprite) => {
            const SCALE: f32 = 4.5;
            p.sprite(
                sprite,
                cx - 8.0 * SCALE,
                offset_y - 13.0 - 8.0 * SCALE,
                16.0 * SCALE,
                16.0 * SCALE,
            );
        }
    }

    state.handle_input(ctx.input, &p.atlas.font);

    let color = if state.face.glowing {
        state.face.color
    } else {
        text::dark_color(&state.face)
    };
    let lh = state.metrics.line_height;
    let midpoint = 4.0 * lh / 2.0;
    let show_cursor = (state.opened_at.elapsed().as_millis() / 300) % 2 == 0;
    let font = &p.atlas.font;
    let width = |s: &str| font.width_str(s).ceil() as i32;
    p.scaled(text_scale, cx, offset_y, |p| {
        for (i, line) in state.lines.iter().enumerate() {
            let half = width(line) / 2;
            let y = i as f32 * lh - midpoint;
            p.text_str(line, -half as f32, y, color, false);
            if i != state.line {
                continue;
            }
            let chars = line.chars().count();
            let prefix_width = |n: usize| {
                let end = line.char_indices().nth(n).map_or(line.len(), |(b, _)| b);
                (width(&line[..end]) - half) as f32
            };
            let cursor = state.field.cursor.min(chars);
            let cursor_x = prefix_width(cursor);
            if show_cursor {
                if cursor >= chars {
                    widgets::draw_append_caret(p, cursor_x, y, color, false);
                } else {
                    widgets::draw_insert_caret(p, cursor_x, y, lh, color);
                }
            }
            let anchor = state.field.anchor.min(chars);
            if anchor != cursor {
                let (lo, hi) = (cursor.min(anchor), cursor.max(anchor));
                let selected: String = line.chars().skip(lo).take(hi - lo).collect();
                widgets::draw_selection(
                    p,
                    &selected,
                    prefix_width(lo),
                    prefix_width(hi),
                    y,
                    y,
                    y + lh,
                );
            }
        }
    });

    let x = ((ctx.vw - WIDGET_WIDTH_BIG) / 2.0).floor();
    let base = (ctx.vh / 4.0).floor();
    let done = Button::new(
        x,
        base + 144.0,
        WIDGET_WIDTH_BIG,
        WIDGET_HEIGHT,
        label!("gui.done"),
    );
    if done.draw(p, ctx) {
        return Some(Screen::None);
    }
    None
}
