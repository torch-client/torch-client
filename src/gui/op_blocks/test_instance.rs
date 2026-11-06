use azalea_core::position::{BlockPos, Vec3i};
use azalea_protocol::packets::game::s_set_structure_block::Rotation;
use azalea_protocol::packets::game::s_test_instance_block_action::{
    Action, ServerboundTestInstanceBlockAction, TestInstanceBlockEntityData,
    TestInstanceBlockEntityStatus,
};
use azalea_registry::identifier::Identifier;

use super::{RED, cycle, field_label, on_off, parse_int, send, step, title};
use crate::blockentities::game_master::{
    MAX_SIZE, ROTATION_VALUES, TestInstanceEdit, parse_identifier,
};
use crate::gui::configphase::{INNER_PADDING, text_area};
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{Button, TextBox, WIDGET_HEIGHT};
use crate::gui::{Screen, ScreenCtx};
use crate::session::{GameMasterPacket, SharedMutex, TestInstanceStatus};
use crate::text::Span;

const WIDTH: f32 = 316.0;
const BUTTON_PADDING: f32 = 8.0;
const LEFT_X: i32 = -158;
const MAX_ID: usize = 128;
const MAX_NUMBER: usize = 15;
const ID_LABEL_Y: f32 = 30.0;
const ID_Y: f32 = 40.0;
const INFO_Y: f32 = 70.0;
const INFO_H: f32 = 8.0 * 9.0;
const SIZE_ROW_Y: f32 = 160.0;
const SIZE_ROW_COUNT: i32 = 5;
const SIZE_LABEL_Y: f32 = 150.0;
const ACTION_ROW_Y: f32 = 185.0;
const ACTION_ROW_COUNT: i32 = 2;
const BOTTOM_ROW_Y: f32 = 210.0;
const BOTTOM_ROW_COUNT: i32 = 3;
const MIN_SIZE: i32 = 1;

pub struct State {
    pos: BlockPos,
    id: TextBox,
    size: [TextBox; 3],
    rotation: Rotation,
    include_entities: bool,
    info: Vec<Span>,
    lines: Option<Vec<Vec<Span>>>,
    scroll: f32,
    scroll_drag: Option<f32>,
    seen_id: String,
}

impl Default for State {
    fn default() -> State {
        let number = || TextBox::bordered(MAX_NUMBER, "");
        State {
            pos: BlockPos::default(),
            id: TextBox::bordered(MAX_ID, ""),
            size: [number(), number(), number()],
            rotation: Rotation::None,
            include_entities: true,
            info: Vec::new(),
            lines: None,
            scroll: 0.0,
            scroll_drag: None,
            seen_id: String::new(),
        }
    }
}

fn exact_widget_size(count: i32) -> f32 {
    (WIDTH - (count - 1) as f32 * BUTTON_PADDING) / count as f32
}

fn widget_size(count: i32) -> f32 {
    exact_widget_size(count) as i32 as f32
}

fn widget_x(cx: f32, index: i32, count: i32) -> f32 {
    let left = cx as i32 + LEFT_X;
    (left as f32 + index as f32 * (BUTTON_PADDING + exact_widget_size(count))) as i32 as f32
}

fn rotation_label(rotation: Rotation) -> &'static str {
    match rotation {
        Rotation::None => "0",
        Rotation::Clockwise90 => "90",
        Rotation::Clockwise180 => "180",
        Rotation::Counterclockwise90 => "270",
    }
}

fn parse_size(s: &str) -> i32 {
    parse_int(s, MIN_SIZE).clamp(MIN_SIZE, MAX_SIZE)
}

impl State {
    pub fn open(&mut self, pos: BlockPos, edit: TestInstanceEdit, shared: &SharedMutex) {
        *self = State {
            pos,
            rotation: edit.rotation,
            include_entities: !edit.ignore_entities,
            ..State::default()
        };
        if let Some(test) = &edit.test {
            self.id.set_text(test);
        }
        self.seen_id = self.id.text.clone();
        self.set_size(edit.size);
        self.update_test_info(Action::Init, shared);
    }

    fn set_size(&mut self, size: [i32; 3]) {
        for (field, n) in self.size.iter_mut().zip(size) {
            field.set_text(&n.to_string());
        }
    }

    fn update_test_info(&mut self, action: Action, shared: &SharedMutex) {
        if !self.send(action, shared) {
            self.info = vec![Span::colored(
                label!("test_instance.description.invalid_id"),
                RED,
            )];
            self.lines = None;
        }
    }

    fn send(&self, action: Action, shared: &SharedMutex) -> bool {
        let test = parse_identifier(&self.id.text).map(Identifier::new);
        let valid = test.is_some();
        let [x, y, z] = [0, 1, 2].map(|i| parse_size(&self.size[i].text));
        send(
            shared,
            GameMasterPacket::TestInstance(ServerboundTestInstanceBlockAction {
                pos: self.pos,
                action,
                data: TestInstanceBlockEntityData {
                    test,
                    size: Vec3i { x, y, z },
                    rotation: self.rotation,
                    ignore_entities: !self.include_entities,
                    status: TestInstanceBlockEntityStatus::Cleared,
                    error_message: None,
                },
            }),
        );
        valid
    }

    fn set_status(&mut self, status: TestInstanceStatus) {
        let mut info = Vec::new();
        if let Some(error) = status.error {
            let template = translate("test_instance.description.failed", &["%s".to_owned()]);
            let (before, after) = template.split_once("%s").unwrap_or((template.as_str(), ""));
            info.push(Span::new(before));
            info.extend(error);
            info.push(Span::new(after));
            info.push(Span::new("\n\n"));
        }
        info.extend(status.status);
        self.info = info;
        self.lines = None;
        if let Some(size) = status.size {
            self.set_size(size);
        }
    }
}

pub fn draw(
    p: &mut Painter,
    state: &mut State,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
) -> Option<Screen> {
    let cx = super::center(ctx);
    let left = widget_x(cx, 0, 1);

    let replies = std::mem::take(&mut shared.lock().unwrap().session.test_instance_status);
    for reply in replies {
        state.set_status(reply);
    }

    state.id.update(p, ctx, left, ID_Y, WIDTH, WIDGET_HEIGHT);
    if state.id.text != state.seen_id {
        state.seen_id.clone_from(&state.id.text);
        state.update_test_info(Action::Query, shared);
    }

    let info = &state.info;
    let lines = state
        .lines
        .get_or_insert_with(|| p.atlas.font.wrap(info, WIDTH - 2.0 * INNER_PADDING));
    text_area(
        p,
        ctx,
        lines,
        left,
        INFO_Y,
        WIDTH,
        INFO_H,
        &mut state.scroll,
        &mut state.scroll_drag,
    );

    let size_w = widget_size(SIZE_ROW_COUNT);
    for (i, field) in state.size.iter_mut().enumerate() {
        field.update(
            p,
            ctx,
            widget_x(cx, i as i32, SIZE_ROW_COUNT),
            SIZE_ROW_Y,
            size_w,
            WIDGET_HEIGHT,
        );
    }
    let rotation_x = widget_x(cx, 3, SIZE_ROW_COUNT);
    let rotation = Button::new(
        rotation_x,
        SIZE_ROW_Y,
        size_w,
        WIDGET_HEIGHT,
        rotation_label(state.rotation),
    );
    let delta = cycle(p, ctx, &rotation);
    if delta != 0 {
        state.rotation = step(&ROTATION_VALUES, state.rotation, delta);
    }
    let entities_x = widget_x(cx, 4, SIZE_ROW_COUNT);
    let entities = Button::new(
        entities_x,
        SIZE_ROW_Y,
        size_w,
        WIDGET_HEIGHT,
        on_off(state.include_entities),
    );
    if cycle(p, ctx, &entities) != 0 {
        state.include_entities = !state.include_entities;
    }

    let can_save = state.rotation == Rotation::None && parse_identifier(&state.id.text).is_some();
    let action_w = widget_size(ACTION_ROW_COUNT);
    let reset = Button::new(
        widget_x(cx, 0, ACTION_ROW_COUNT),
        ACTION_ROW_Y,
        action_w,
        WIDGET_HEIGHT,
        label!("test_instance.action.reset"),
    )
    .draw(p, ctx);
    let save = Button {
        active: can_save,
        ..Button::new(
            widget_x(cx, 1, ACTION_ROW_COUNT),
            ACTION_ROW_Y,
            action_w,
            WIDGET_HEIGHT,
            label!("test_instance.action.save"),
        )
    }
    .draw(p, ctx);

    let bottom_w = widget_size(BOTTOM_ROW_COUNT);
    let bottom = |i: i32, text: &'static str| {
        Button::new(
            widget_x(cx, i, BOTTOM_ROW_COUNT),
            BOTTOM_ROW_Y,
            bottom_w,
            WIDGET_HEIGHT,
            text,
        )
    };
    let run = bottom(0, label!("test_instance.action.run")).draw(p, ctx);
    let done = bottom(1, label!("gui.done")).draw(p, ctx);
    let cancelled = bottom(2, label!("gui.cancel")).draw(p, ctx);

    title(p, label!("block.minecraft.test_instance_block"), ctx);
    field_label(p, label!("test_instance_block.test_id"), left, ID_LABEL_Y);
    field_label(p, label!("test_instance_block.size"), left, SIZE_LABEL_Y);
    field_label(
        p,
        label!("test_instance_block.rotation"),
        rotation_x,
        SIZE_LABEL_Y,
    );
    field_label(
        p,
        label!("test_instance_block.entities"),
        entities_x,
        SIZE_LABEL_Y,
    );

    let action = if reset {
        Action::Reset
    } else if save {
        Action::Save
    } else if run {
        Action::Run
    } else if done {
        Action::Set
    } else {
        return cancelled.then_some(Screen::None);
    };
    state.send(action, shared);
    Some(Screen::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_divide_the_column() {
        assert_eq!(widget_size(5), 56.0);
        assert_eq!(widget_size(2), 154.0);
        assert_eq!(widget_size(3), 100.0);
        assert_eq!(widget_x(213.0, 0, 5), 55.0);
        assert_eq!(widget_x(213.0, 4, 5), 314.0);
        assert_eq!(widget_x(213.0, 1, 2), 217.0);
        assert_eq!(widget_x(213.0, 2, 3), 271.0);
    }

    #[test]
    fn sizes_clamp() {
        assert_eq!(parse_size("0"), 1);
        assert_eq!(parse_size("99"), 48);
        assert_eq!(parse_size("x"), 1);
        assert_eq!(parse_size("12"), 12);
    }
}
