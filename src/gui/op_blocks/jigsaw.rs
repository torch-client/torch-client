use azalea_core::position::BlockPos;
use azalea_protocol::packets::game::s_jigsaw_generate::ServerboundJigsawGenerate;
use azalea_protocol::packets::game::s_set_jigsaw_block::ServerboundSetJigsawBlock;
use azalea_registry::identifier::Identifier;

use super::{cycle, done_cancel, field_label, hidden, on_off, parse_int, send, step};
use crate::blockentities::game_master::{JigsawEdit, Joint, parse_identifier};
use crate::gui::options::generic_value;
use crate::gui::painter::Painter;
use crate::gui::tooltip::{label, translate};
use crate::gui::widgets::{Button, Slider, TextBox, WIDGET_HEIGHT};
use crate::gui::{Screen, ScreenCtx};
use crate::session::{GameMasterPacket, SharedMutex};
use crate::text::Span;

const MAX_ID: usize = 128;
const MAX_FINAL_STATE: usize = 256;
const MAX_PRIORITY: usize = 3;

const LEFT_X: f32 = -153.0;
const WIDE_W: f32 = 300.0;
const WIDE_Y: [f32; 4] = [20.0, 55.0, 90.0, 125.0];
const LABEL_ABOVE: f32 = 10.0;
const PRIORITY_Y: f32 = 160.0;
const PLACEMENT_X: f32 = -50.0;
const PRIORITY_W: f32 = 98.0;
const JOINT_X: f32 = 54.0;
const JOINT_LABEL_X: f32 = 53.0;
const ROW_Y: f32 = 185.0;
const ROW_W: f32 = 100.0;
const LEVELS_X: f32 = -154.0;
const KEEP_X: f32 = -50.0;
const GENERATE_X: f32 = 54.0;
const MAX_LEVELS: i32 = 20;
const LEVELS_SLIDER_ID: u32 = 0x3000;
const TOOLTIP_W: f32 = 170.0;

pub struct State {
    pos: BlockPos,
    pool: TextBox,
    name: TextBox,
    target: TextBox,
    final_state: TextBox,
    selection_priority: TextBox,
    placement_priority: TextBox,
    joint: Joint,
    vertical: bool,
    levels: i32,
    keep_jigsaws: bool,
}

impl Default for State {
    fn default() -> State {
        State {
            pos: BlockPos::default(),
            pool: TextBox::bordered(MAX_ID, ""),
            name: TextBox::bordered(MAX_ID, ""),
            target: TextBox::bordered(MAX_ID, ""),
            final_state: TextBox::bordered(MAX_FINAL_STATE, ""),
            selection_priority: TextBox::bordered(MAX_PRIORITY, ""),
            placement_priority: TextBox::bordered(MAX_PRIORITY, ""),
            joint: Joint::Rollable,
            vertical: false,
            levels: 0,
            keep_jigsaws: true,
        }
    }
}

impl State {
    pub fn open(&mut self, pos: BlockPos, edit: JigsawEdit) {
        *self = State {
            pos,
            joint: edit.joint,
            vertical: edit.vertical,
            ..State::default()
        };
        self.pool.set_text(&edit.pool);
        self.name.set_text(&edit.name);
        self.target.set_text(&edit.target);
        self.final_state.set_text(&edit.final_state);
        self.selection_priority
            .set_text(&edit.selection_priority.to_string());
        self.placement_priority
            .set_text(&edit.placement_priority.to_string());
        self.pool.focus();
    }

    fn packet(&self) -> Option<ServerboundSetJigsawBlock> {
        let id = |b: &TextBox| parse_identifier(&b.text).map(Identifier::new);
        Some(ServerboundSetJigsawBlock {
            pos: self.pos,
            name: id(&self.name)?,
            target: id(&self.target)?,
            pool: id(&self.pool)?,
            final_state: self.final_state.text.clone(),
            joint: self.joint.name().to_owned(),
            selection_priority: parse_int(&self.selection_priority.text, 0),
            placement_priority: parse_int(&self.placement_priority.text, 0),
        })
    }
}

fn joint_label(joint: Joint) -> &'static str {
    match joint {
        Joint::Rollable => label!("jigsaw_block.joint.rollable"),
        Joint::Aligned => label!("jigsaw_block.joint.aligned"),
    }
}

pub fn draw(
    p: &mut Painter,
    state: &mut State,
    ctx: &ScreenCtx,
    shared: &SharedMutex,
) -> Option<Screen> {
    let cx = super::center(ctx);
    let left = cx + LEFT_X;

    let wide = [
        (&mut state.pool, label!("jigsaw_block.pool")),
        (&mut state.name, label!("jigsaw_block.name")),
        (&mut state.target, label!("jigsaw_block.target")),
        (&mut state.final_state, label!("jigsaw_block.final_state")),
    ];
    for ((field, caption), y) in wide.into_iter().zip(WIDE_Y) {
        field.update(p, ctx, left, y, WIDE_W, WIDGET_HEIGHT);
        field_label(p, caption, left, y - LABEL_ABOVE);
    }
    let placement_x = cx + PLACEMENT_X;
    state
        .selection_priority
        .update(p, ctx, left, PRIORITY_Y, PRIORITY_W, WIDGET_HEIGHT);
    state
        .placement_priority
        .update(p, ctx, placement_x, PRIORITY_Y, PRIORITY_W, WIDGET_HEIGHT);
    let priority_label_y = PRIORITY_Y - LABEL_ABOVE;
    field_label(
        p,
        label!("jigsaw_block.selection_priority"),
        left,
        priority_label_y,
    );
    field_label(
        p,
        label!("jigsaw_block.placement_priority"),
        placement_x,
        priority_label_y,
    );

    let valid = [&state.pool, &state.name, &state.target]
        .iter()
        .all(|b| parse_identifier(&b.text).is_some());
    let mut pressed = false;

    if state.vertical {
        let joint = Button::new(
            cx + JOINT_X,
            PRIORITY_Y,
            ROW_W,
            WIDGET_HEIGHT,
            joint_label(state.joint),
        );
        let delta = cycle(p, ctx, &joint);
        if delta != 0 {
            pressed = true;
            state.joint = step(&Joint::VALUES, state.joint, delta);
        }
        field_label(
            p,
            label!("jigsaw_block.joint_label"),
            cx + JOINT_LABEL_X,
            priority_label_y,
        );
    } else {
        hidden();
    }

    let levels = Slider {
        id: LEVELS_SLIDER_ID,
        x: cx + LEVELS_X,
        y: ROW_Y,
        w: ROW_W,
        h: WIDGET_HEIGHT,
        min: 0,
        max: MAX_LEVELS,
        value: state.levels,
        label: |v: i32| translate("jigsaw_block.levels", &[v.to_string()]),
    }
    .draw(p, ctx);
    state.levels = levels;

    let keep_caption = generic_value(
        label!("jigsaw_block.keep_jigsaws"),
        on_off(state.keep_jigsaws),
    );
    let keep = Button::new(cx + KEEP_X, ROW_Y, ROW_W, WIDGET_HEIGHT, &keep_caption);
    if cycle(p, ctx, &keep) != 0 {
        pressed = true;
        state.keep_jigsaws = !state.keep_jigsaws;
    }

    let generate = Button {
        active: valid,
        ..Button::new(
            cx + GENERATE_X,
            ROW_Y,
            ROW_W,
            WIDGET_HEIGHT,
            label!("jigsaw_block.generate"),
        )
    };
    let generated = generate.draw(p, ctx);
    let (done, cancelled) = done_cancel(p, ctx, valid);

    if let Some(m) = ctx.mouse() {
        let tip = if ctx.hovering(left, PRIORITY_Y, PRIORITY_W, WIDGET_HEIGHT) {
            Some(label!("jigsaw_block.selection_priority.tooltip"))
        } else if ctx.hovering(placement_x, PRIORITY_Y, PRIORITY_W, WIDGET_HEIGHT) {
            Some(label!("jigsaw_block.placement_priority.tooltip"))
        } else {
            None
        };
        if let Some(tip) = tip {
            let lines = p.atlas.font.wrap(&[Span::new(tip)], TOOLTIP_W);
            crate::gui::tooltip::draw_lines(p, &lines, m.x, m.y, ctx.vw, ctx.vh);
        }
    }

    let confirmed = ctx.input.enter && valid && !pressed && !generated && !cancelled;
    if generated || done || confirmed {
        if let Some(packet) = state.packet() {
            send(shared, GameMasterPacket::Jigsaw(packet));
        }
        if generated {
            send(
                shared,
                GameMasterPacket::JigsawGenerate(ServerboundJigsawGenerate {
                    pos: state.pos,
                    levels: state.levels as u32,
                    keep_jigsaws: state.keep_jigsaws,
                }),
            );
        }
        return Some(Screen::None);
    }
    cancelled.then_some(Screen::None)
}
