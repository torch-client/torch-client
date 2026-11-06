use azalea_core::position::BlockPos;
use azalea_protocol::packets::game::s_set_structure_block::{
    BytePosition, Flags, Mirror, Rotation, ServerboundSetStructureBlock, StructureMode, UpdateType,
};

use super::{
    DONE_X, cycle, decimal_format, done_cancel, field_label, field_label_right, hidden, on_off,
    parse_float, parse_int, parse_long, send, step, title,
};
use crate::blockentities::game_master::{DEFAULT_INTEGRITY, MIRROR_VALUES, StructureEdit};
use crate::gui::painter::Painter;
use crate::gui::render::EditKey;
use crate::gui::tooltip::label;
use crate::gui::widgets::{Button, TextBox, WIDGET_HEIGHT};
use crate::gui::{Screen, ScreenCtx};
use crate::session::{GameMasterPacket, SharedMutex};

const MAX_NAME: usize = 128;
const MAX_NUMBER: usize = 15;
const MAX_SEED: usize = 31;
const MAX_DATA: usize = 128;

const COLUMN_X: [f32; 3] = [-152.0, -72.0, 8.0];
const NAME_W: f32 = 300.0;
const NUMBER_W: f32 = 80.0;
const DATA_W: f32 = 240.0;
const NAME_Y: f32 = 40.0;
const POSITION_Y: f32 = 80.0;
const SIZE_Y: f32 = 120.0;

const LABEL_X: f32 = -153.0;
const LABEL_RIGHT: f32 = 154.0;
const NAME_LABEL_Y: f32 = 30.0;
const POSITION_LABEL_Y: f32 = 70.0;
const SIZE_LABEL_Y: f32 = 110.0;
const INCLUDE_LABEL_Y: f32 = 150.0;
const MODE_INFO_Y: f32 = 174.0;

const SIDE_X: f32 = 104.0;
const SIDE_W: f32 = 50.0;
const SHOW_Y: f32 = 80.0;
const DETECT_Y: f32 = 120.0;
const INCLUDE_Y: f32 = 160.0;
const ROW_Y: f32 = 185.0;
const MODE_W: f32 = 50.0;
const MIRROR_X: f32 = -20.0;
const SMALL_W: f32 = 40.0;
const ROTATION_X: [f32; 4] = [-102.0, -61.0, 21.0, 62.0];

static DEFAULT_MODES: [StructureMode; 3] = [
    StructureMode::Save,
    StructureMode::Load,
    StructureMode::Corner,
];
static ALL_MODES: [StructureMode; 4] = [
    StructureMode::Save,
    StructureMode::Load,
    StructureMode::Corner,
    StructureMode::Data,
];

pub struct State {
    pos: BlockPos,
    mode: StructureMode,
    mode_index: i32,
    mirror: Mirror,
    rotation: Rotation,
    include_entities: bool,
    strict: bool,
    show_air: bool,
    show_bounding_box: bool,
    name: TextBox,
    position: [TextBox; 3],
    size: [TextBox; 3],
    integrity: TextBox,
    seed: TextBox,
    data: TextBox,
}

impl Default for State {
    fn default() -> State {
        let number = || TextBox::bordered(MAX_NUMBER, "");
        State {
            pos: BlockPos::default(),
            mode: StructureMode::Data,
            mode_index: -1,
            mirror: Mirror::None,
            rotation: Rotation::None,
            include_entities: false,
            strict: false,
            show_air: false,
            show_bounding_box: true,
            name: TextBox::bordered(MAX_NAME, ""),
            position: [number(), number(), number()],
            size: [number(), number(), number()],
            integrity: number(),
            seed: TextBox::bordered(MAX_SEED, ""),
            data: TextBox::bordered(MAX_DATA, ""),
        }
    }
}

enum Action {
    Done,
    Cancel,
    Send(UpdateType),
}

impl State {
    pub fn open(&mut self, pos: BlockPos, edit: StructureEdit) {
        *self = State {
            pos,
            mode: edit.mode,
            mode_index: DEFAULT_MODES
                .iter()
                .position(|m| *m == edit.mode)
                .map_or(-1, |i| i as i32),
            mirror: edit.mirror,
            rotation: edit.rotation,
            include_entities: !edit.ignore_entities,
            strict: edit.strict,
            show_air: edit.show_air,
            show_bounding_box: edit.show_bounding_box,
            ..State::default()
        };
        self.name.set_text(&edit.name);
        for i in 0..3 {
            self.position[i].set_text(&edit.offset[i].to_string());
            self.size[i].set_text(&edit.size[i].to_string());
        }
        self.integrity.set_text(&decimal_format(edit.integrity));
        self.seed.set_text(&edit.seed.to_string());
        self.data.set_text(&edit.metadata);
        if self.mode != StructureMode::Data {
            self.name.focus();
        }
    }

    fn packet(&self, update_type: UpdateType) -> GameMasterPacket {
        let byte = |b: &TextBox| parse_int(&b.text, 0) as i8 as u8;
        let bytes = |boxes: &[TextBox; 3]| BytePosition {
            x: byte(&boxes[0]),
            y: byte(&boxes[1]),
            z: byte(&boxes[2]),
        };
        GameMasterPacket::Structure(ServerboundSetStructureBlock {
            pos: self.pos,
            update_type,
            mode: self.mode,
            name: self.name.text.clone(),
            offset: bytes(&self.position),
            size: bytes(&self.size),
            mirror: self.mirror,
            rotation: self.rotation,
            data: self.data.text.clone(),
            integrity: parse_float(&self.integrity.text, DEFAULT_INTEGRITY),
            seed: parse_long(&self.seed.text) as u64,
            flags: Flags {
                ignore_entities: !self.include_entities,
                show_air: self.show_air,
                show_bounding_box: self.show_bounding_box,
                strict: self.strict,
            },
        })
    }
}

fn valid_name_char(name: &str, c: char, cursor: usize) -> bool {
    let colon = name.chars().position(|n| n == ':');
    let slash = name.chars().position(|n| n == '/');
    match c {
        ':' => slash.is_none_or(|s| cursor <= s) && colon.is_none(),
        '/' => colon.is_none_or(|n| cursor > n),
        _ => matches!(c, '_' | '-' | 'a'..='z' | '0'..='9' | '.'),
    }
}

fn mode_label(mode: StructureMode) -> &'static str {
    match mode {
        StructureMode::Save => label!("structure_block.mode.save"),
        StructureMode::Load => label!("structure_block.mode.load"),
        StructureMode::Corner => label!("structure_block.mode.corner"),
        StructureMode::Data => label!("structure_block.mode.data"),
    }
}

fn mode_info(mode: StructureMode) -> &'static str {
    match mode {
        StructureMode::Save => label!("structure_block.mode_info.save"),
        StructureMode::Load => label!("structure_block.mode_info.load"),
        StructureMode::Corner => label!("structure_block.mode_info.corner"),
        StructureMode::Data => label!("structure_block.mode_info.data"),
    }
}

fn mirror_label(mirror: Mirror) -> &'static str {
    match mirror {
        Mirror::None => label!("mirror.none"),
        Mirror::LeftRight => label!("mirror.left_right"),
        Mirror::FrontBack => label!("mirror.front_back"),
    }
}

pub fn draw(
    p: &mut Painter,
    state: &mut State,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
) -> Option<Screen> {
    let cx = super::center(ctx);
    let mut action = None;
    let mut pressed = false;

    let (done, cancel) = done_cancel(p, ctx, true);
    if done {
        action = Some(Action::Done);
    }
    if cancel {
        action = Some(Action::Cancel);
    }

    let mode = state.mode;
    let save = mode == StructureMode::Save;
    let load = mode == StructureMode::Load;

    for (shown, key, update) in [
        (
            save,
            label!("structure_block.button.save"),
            UpdateType::SaveArea,
        ),
        (
            load,
            label!("structure_block.button.load"),
            UpdateType::LoadArea,
        ),
    ] {
        if shown {
            if Button::new(cx + SIDE_X, ROW_Y, SIDE_W, WIDGET_HEIGHT, key).draw(p, ctx) {
                action = Some(Action::Send(update));
            }
        } else {
            hidden();
        }
    }

    let mode_button = Button::new(cx + DONE_X, ROW_Y, MODE_W, WIDGET_HEIGHT, mode_label(mode));
    let delta = cycle(p, ctx, &mode_button);
    if delta != 0 {
        pressed = true;
        let list: &[StructureMode] = if ctx.input.alt {
            &ALL_MODES
        } else {
            &DEFAULT_MODES
        };
        state.mode_index = (state.mode_index + delta).rem_euclid(list.len() as i32);
        state.mode = list[state.mode_index as usize];
    }
    let mode = state.mode;
    let save = mode == StructureMode::Save;
    let load = mode == StructureMode::Load;

    if save {
        let detect = Button::new(
            cx + SIDE_X,
            DETECT_Y,
            SIDE_W,
            WIDGET_HEIGHT,
            label!("structure_block.button.detect_size"),
        );
        if detect.draw(p, ctx) {
            action = Some(Action::Send(UpdateType::ScanArea));
        }
    } else {
        hidden();
    }

    if save || load {
        let include = Button::new(
            cx + SIDE_X,
            INCLUDE_Y,
            SIDE_W,
            WIDGET_HEIGHT,
            on_off(state.include_entities),
        );
        if cycle(p, ctx, &include) != 0 {
            pressed = true;
            state.include_entities = !state.include_entities;
        }
    } else {
        hidden();
    }

    if load {
        let strict = Button::new(
            cx + SIDE_X,
            DETECT_Y,
            SIDE_W,
            WIDGET_HEIGHT,
            on_off(state.strict),
        );
        if cycle(p, ctx, &strict) != 0 {
            pressed = true;
            state.strict = !state.strict;
        }
        let mirror = Button::new(
            cx + MIRROR_X,
            ROW_Y,
            SMALL_W,
            WIDGET_HEIGHT,
            mirror_label(state.mirror),
        );
        let delta = cycle(p, ctx, &mirror);
        if delta != 0 {
            pressed = true;
            state.mirror = step(&MIRROR_VALUES, state.mirror, delta);
        }
    } else {
        hidden();
        hidden();
    }

    if save {
        let air = Button::new(
            cx + SIDE_X,
            SHOW_Y,
            SIDE_W,
            WIDGET_HEIGHT,
            on_off(state.show_air),
        );
        if cycle(p, ctx, &air) != 0 {
            pressed = true;
            state.show_air = !state.show_air;
        }
    } else {
        hidden();
    }

    if load {
        let bbox = Button::new(
            cx + SIDE_X,
            SHOW_Y,
            SIDE_W,
            WIDGET_HEIGHT,
            on_off(state.show_bounding_box),
        );
        if cycle(p, ctx, &bbox) != 0 {
            pressed = true;
            state.show_bounding_box = !state.show_bounding_box;
        }
        const LABELS: [&str; 4] = ["0", "90", "180", "270"];
        for (i, rotation) in crate::blockentities::game_master::ROTATION_VALUES
            .into_iter()
            .enumerate()
        {
            let button = Button {
                active: state.rotation != rotation,
                ..Button::new(cx + ROTATION_X[i], ROW_Y, SMALL_W, WIDGET_HEIGHT, LABELS[i])
            };
            if button.draw(p, ctx) {
                pressed = true;
                state.rotation = rotation;
            }
        }
    } else {
        for _ in 0..5 {
            hidden();
        }
    }

    if mode != StructureMode::Data {
        state.name.update_with(
            p,
            ctx,
            cx + COLUMN_X[0],
            NAME_Y,
            NAME_W,
            WIDGET_HEIGHT,
            |field, key| match key {
                EditKey::Char(c) => !valid_name_char(&field.text, c, field.cursor),
                _ => false,
            },
        );
    } else {
        hidden();
    }
    for (row, y, shown) in [
        (&mut state.position, POSITION_Y, save || load),
        (&mut state.size, SIZE_Y, save),
    ] {
        for (field, x) in row.iter_mut().zip(COLUMN_X) {
            if shown {
                field.update(p, ctx, cx + x, y, NUMBER_W, WIDGET_HEIGHT);
            } else {
                hidden();
            }
        }
    }
    if load {
        state
            .integrity
            .update(p, ctx, cx + COLUMN_X[0], SIZE_Y, NUMBER_W, WIDGET_HEIGHT);
        state
            .seed
            .update(p, ctx, cx + COLUMN_X[1], SIZE_Y, NUMBER_W, WIDGET_HEIGHT);
    } else {
        hidden();
        hidden();
    }
    if mode == StructureMode::Data {
        state
            .data
            .update(p, ctx, cx + COLUMN_X[0], SIZE_Y, DATA_W, WIDGET_HEIGHT);
    } else {
        hidden();
    }

    title(p, label!("block.minecraft.structure_block"), ctx);
    let (left, right) = (cx + LABEL_X, cx + LABEL_RIGHT);
    if mode != StructureMode::Data {
        field_label(
            p,
            label!("structure_block.structure_name"),
            left,
            NAME_LABEL_Y,
        );
    }
    if matches!(mode, StructureMode::Load | StructureMode::Save) {
        field_label(
            p,
            label!("structure_block.position"),
            left,
            POSITION_LABEL_Y,
        );
        field_label_right(
            p,
            label!("structure_block.include_entities"),
            right,
            INCLUDE_LABEL_Y,
        );
    }
    if mode == StructureMode::Save {
        field_label(p, label!("structure_block.size"), left, SIZE_LABEL_Y);
        field_label_right(
            p,
            label!("structure_block.detect_size"),
            right,
            SIZE_LABEL_Y,
        );
        field_label_right(
            p,
            label!("structure_block.show_air"),
            right,
            POSITION_LABEL_Y,
        );
    }
    if mode == StructureMode::Load {
        field_label(p, label!("structure_block.integrity"), left, SIZE_LABEL_Y);
        field_label_right(p, label!("structure_block.strict"), right, SIZE_LABEL_Y);
        field_label_right(
            p,
            label!("structure_block.show_boundingbox"),
            right,
            POSITION_LABEL_Y,
        );
    }
    if mode == StructureMode::Data {
        field_label(p, label!("structure_block.custom_data"), left, SIZE_LABEL_Y);
    }
    field_label(p, mode_info(mode), left, MODE_INFO_Y);

    if action.is_none() && !pressed && ctx.input.enter {
        action = Some(Action::Done);
    }
    match action? {
        Action::Done => send(shared, state.packet(UpdateType::UpdateData)),
        Action::Cancel => {}
        Action::Send(update) => send(shared, state.packet(update)),
    }
    Some(Screen::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_filter() {
        assert!(valid_name_char("", 'a', 0));
        assert!(!valid_name_char("", 'A', 0));
        assert!(valid_name_char("mod", ':', 3));
        assert!(!valid_name_char("mod:x", ':', 5));
        assert!(!valid_name_char("a/b", ':', 3));
        assert!(valid_name_char("a/b", ':', 1));
        assert!(valid_name_char("mod:x", '/', 5));
        assert!(!valid_name_char("mod:x", '/', 2));
        assert!(valid_name_char("x", '/', 1));
    }
}
