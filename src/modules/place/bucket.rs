use azalea::Client;
use azalea::entity::inventory::Inventory;
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea_core::game_type::GameMode;
use azalea_core::hit_result::BlockHitResult;
use azalea_core::position::{BlockPos, Vec3};
use azalea_inventory::ItemStack;
use azalea_registry::builtin::{BlockKind, ItemKind};

use super::{
    Click, DIRECTIONS, Fired, Want, candidates_with, distance, face_axis, face_plane, face_points,
    meets,
};
use crate::modules::scaffold::aim::WorldCells;
use crate::modules::sent::{self, Look, look_at, view_vector};
use crate::play::placement::{has_use_interaction, predict_bucket, predict_bucket_fill};

const SOURCE_TOP: f64 = 0.85;

pub fn pour_candidates(
    world: &azalea_world::World,
    eye: [f64; 3],
    reach: f64,
    item: ItemKind,
    want: &Want,
    visible: bool,
) -> Vec<Click> {
    let held = ItemStack::new(item, 1);
    let min_y = world.chunks.min_y();
    let height = world.chunks.height();
    let predict = |hit: &BlockHitResult, _look: Look, _sneak: bool| {
        predict_bucket(&held, hit, min_y, height, |at| world.get_block_state(at))
    };
    candidates_with(world, eye, reach, want, visible, false, &predict)
        .into_iter()
        .map(|k| Click { sneak: false, ..k })
        .collect()
}

pub fn drain_candidates(
    world: &azalea_world::World,
    eye: [f64; 3],
    reach: f64,
    cell: BlockPos,
) -> Vec<Click> {
    let lo = [f64::from(cell.x), f64::from(cell.y), f64::from(cell.z)];
    let hi = [lo[0] + 1.0, lo[1] + SOURCE_TOP, lo[2] + 1.0];
    let mut found: Vec<(f64, Click)> = Vec::new();
    for face in DIRECTIONS {
        let (axis, positive) = face_axis(face);
        let plane = face_plane(lo, hi, face);
        if (positive && eye[axis] <= plane) || (!positive && eye[axis] >= plane) {
            continue;
        }
        for p in face_points(lo, hi, face) {
            let d = distance(eye, Vec3::new(p[0], p[1], p[2]));
            if d > reach {
                continue;
            }
            let look = look_at(eye, p);
            match source_trace(world, eye, look, reach) {
                Some(hit) if hit.block_pos == cell => found.push((
                    d,
                    Click {
                        hit,
                        look,
                        sneak: false,
                    },
                )),
                _ => {}
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, c)| c).collect()
}

pub fn source_trace(
    world: &azalea_world::World,
    eye: [f64; 3],
    look: Look,
    reach: f64,
) -> Option<BlockHitResult> {
    let d = view_vector(look);
    let from = Vec3::new(eye[0], eye[1], eye[2]);
    let to = Vec3::new(
        eye[0] + d[0] * reach,
        eye[1] + d[1] * reach,
        eye[2] + d[2] * reach,
    );
    let hit = clip(
        &world.chunks,
        ClipContext {
            from,
            to,
            block_shape_type: BlockShapeType::Outline,
            fluid_pick_type: FluidPickType::SourceOnly,
            held_item: ItemKind::Bucket,
            entity: None,
        },
    );
    (!hit.miss && distance(eye, hit.location) <= reach).then_some(hit)
}

pub fn fire_pour(bot: &Client, want: &Want, reach: f64) -> Fired {
    let Some((eye, look)) = sent::sent_view(bot) else {
        return Fired::NotAimed;
    };
    let Some(held) = held_item(bot) else {
        return Fired::Declined;
    };
    let Ok(world) = bot.world() else {
        return Fired::NotAimed;
    };
    let crosshair = {
        let w = world.read();
        let tracer = WorldCells {
            world: &w,
            item: BlockKind::Air,
        };
        let Some(hit) = tracer.trace(eye, look, reach) else {
            return Fired::NotAimed;
        };
        match predict_bucket(&held, &hit, w.chunks.min_y(), w.chunks.height(), |at| {
            w.get_block_state(at)
        }) {
            Some((pos, state)) if meets(want, pos, state) => {}
            Some(_) => return Fired::NotAimed,
            None => return Fired::Declined,
        }
        if w.get_block_state(hit.block_pos)
            .is_some_and(has_use_interaction)
        {
            return Fired::Declined;
        }
        hit
    };
    send(bot, look, Some(&crosshair))
}

pub fn fire_drain(bot: &Client, cell: BlockPos, reach: f64) -> Fired {
    let Some((eye, look)) = sent::sent_view(bot) else {
        return Fired::NotAimed;
    };
    let Some(held) = held_item(bot) else {
        return Fired::Declined;
    };
    let creative = bot
        .component::<GameMode>()
        .is_ok_and(|gm| *gm == GameMode::Creative);
    let Ok(world) = bot.world() else {
        return Fired::NotAimed;
    };
    let crosshair = {
        let w = world.read();
        let Some(hit) = source_trace(&w, eye, look, reach) else {
            return Fired::NotAimed;
        };
        match predict_bucket_fill(&held, &hit, creative, |at| w.get_block_state(at)) {
            Some((pos, _, _)) if pos == cell => {}
            Some(_) => return Fired::NotAimed,
            None => {
                return if hit.block_pos == cell {
                    Fired::Declined
                } else {
                    Fired::NotAimed
                };
            }
        }
        let tracer = WorldCells {
            world: &w,
            item: BlockKind::Air,
        };
        let behind = tracer.trace(eye, look, reach);
        if behind.as_ref().is_some_and(|b| {
            w.get_block_state(b.block_pos)
                .is_some_and(has_use_interaction)
        }) {
            return Fired::Declined;
        }
        behind
    };
    send(bot, look, crosshair.as_ref())
}

fn send(bot: &Client, look: Look, crosshair: Option<&BlockHitResult>) -> Fired {
    use azalea_protocol::packets::game::ServerboundUseItem;
    use azalea_protocol::packets::game::s_use_item_on::ServerboundUseItemOn;

    let main = crate::session::InteractionHand::Main;
    if let Some(hit) = crosshair {
        let Some(seq) = next_sequence(bot) else {
            return Fired::Declined;
        };
        crate::client::outbox::send(
            bot,
            ServerboundUseItemOn {
                hand: main.packet(),
                block_hit: hit.into(),
                seq,
            },
        );
    }
    let Some(seq) = next_sequence(bot) else {
        return Fired::Declined;
    };
    crate::client::outbox::send(
        bot,
        ServerboundUseItem {
            hand: main.packet(),
            seq,
            y_rot: look.yaw,
            x_rot: look.pitch,
        },
    );
    Fired::Placed
}

fn next_sequence(bot: &Client) -> Option<u32> {
    use azalea::interact::BlockStatePredictionHandler;

    let mut ecs = bot.ecs.write();
    let mut handler = ecs.get_mut::<BlockStatePredictionHandler>(bot.entity)?;
    Some(handler.start_predicting())
}

fn held_item(bot: &Client) -> Option<ItemStack> {
    bot.component::<Inventory>()
        .ok()
        .map(|inv| inv.held_item().clone())
}
