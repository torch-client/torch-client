use std::f32::consts::SQRT_2;

use azalea_client::{SprintDirection, WalkDirection};
use azalea_core::direction::CardinalDirection;

use super::{Edge, ExecuteCtx, IsReachedCtx, MoveData, MovesCtx, default_is_reached};
use crate::pathfinder::{
    astar,
    costs::{
        ENTER_WATER_PENALTY, SINK_ONE_BLOCK_COST, SUBMERGED_PENALTY, SWIM_AGAINST_CURRENT_COST,
        SWIM_ONE_BLOCK_COST, SWIM_SPRINT_AGAINST_CURRENT_COST, SWIM_SPRINT_ONE_BLOCK_COST,
        SWIM_UP_AGAINST_CURRENT_COST, SWIM_UP_ONE_BLOCK_COST,
    },
    player_pos_to_block_pos,
    positions::RelBlockPos,
    safety::SUBMERGED_MAX_DEPTH,
    world::{is_block_state_passable, is_block_state_water},
};

const SIDES: [(i16, i16); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

pub fn swim_move(ctx: &mut MovesCtx, node: RelBlockPos) {
    if ctx.world.is_block_water(node) {
        swim_level(ctx, node);
        swim_diagonal(ctx, node);
        swim_vertical(ctx, node);
        swim_ashore(ctx, node);
    } else if ctx.world.is_block_standable(node.down(1)) {
        wade_in(ctx, node);
    }
}

fn is_water_node(ctx: &MovesCtx, pos: RelBlockPos) -> bool {
    if !ctx.world.is_block_water(pos) {
        return false;
    }
    let head = pos.up(1);
    ctx.world.is_block_passable(head) || (ctx.world.is_block_water(head) && near_air(ctx, pos))
}

fn near_air(ctx: &MovesCtx, pos: RelBlockPos) -> bool {
    let head = pos.up(1);
    column_reaches_air(ctx, head)
        || SIDES
            .iter()
            .any(|&(dx, dz)| column_reaches_air(ctx, RelBlockPos::new(head.x + dx, head.y, head.z + dz)))
}

fn column_reaches_air(ctx: &MovesCtx, from: RelBlockPos) -> bool {
    let mut cell = from;
    for _ in 0..=SUBMERGED_MAX_DEPTH {
        if ctx.world.is_block_passable(cell) {
            return true;
        }
        if !ctx.world.is_block_water(cell) {
            return false;
        }
        cell = cell.up(1);
    }
    false
}

fn submerged(ctx: &MovesCtx, pos: RelBlockPos) -> bool {
    !ctx.world.is_block_passable(pos.up(1))
}

fn level_cost(ctx: &MovesCtx, from: RelBlockPos, to: RelBlockPos) -> f32 {
    let under = submerged(ctx, from) || submerged(ctx, to);
    let flowing = ctx.world.is_block_flowing(from) || ctx.world.is_block_flowing(to);
    match (under, flowing) {
        (false, false) => SWIM_ONE_BLOCK_COST,
        (false, true) => SWIM_AGAINST_CURRENT_COST,
        (true, false) => SWIM_SPRINT_ONE_BLOCK_COST + SUBMERGED_PENALTY,
        (true, true) => SWIM_SPRINT_AGAINST_CURRENT_COST + SUBMERGED_PENALTY,
    }
}

fn push(ctx: &mut MovesCtx, target: RelBlockPos, cost: f32, data: MoveData) {
    ctx.edges.push(Edge {
        movement: astar::Movement { target, data },
        cost,
    });
}

fn swim_level(ctx: &mut MovesCtx, node: RelBlockPos) {
    for dir in CardinalDirection::iter() {
        let to = node + RelBlockPos::new(dir.x(), 0, dir.z());
        if !is_water_node(ctx, to) {
            continue;
        }
        let cost = level_cost(ctx, node, to);
        push(
            ctx,
            to,
            cost,
            MoveData {
                execute: &execute_swim,
                is_reached: &default_is_reached,
            },
        );
    }
}

fn swim_diagonal(ctx: &mut MovesCtx, node: RelBlockPos) {
    for dir in CardinalDirection::iter() {
        let right = dir.right();
        let to = node + RelBlockPos::new(dir.x() + right.x(), 0, dir.z() + right.z());
        if !is_water_node(ctx, to) {
            continue;
        }
        let sides = [
            node + RelBlockPos::new(dir.x(), 0, dir.z()),
            node + RelBlockPos::new(right.x(), 0, right.z()),
        ];
        if !sides
            .iter()
            .all(|s| ctx.world.is_block_open(*s) && ctx.world.is_block_open(s.up(1)))
        {
            continue;
        }
        let cost = level_cost(ctx, node, to).mul_add(SQRT_2, 0.001);
        push(
            ctx,
            to,
            cost,
            MoveData {
                execute: &execute_swim,
                is_reached: &default_is_reached,
            },
        );
    }
}

fn swim_vertical(ctx: &mut MovesCtx, node: RelBlockPos) {
    let up = node.up(1);
    if is_water_node(ctx, up) {
        let mut cost = if ctx.world.is_block_flowing(node) || ctx.world.is_block_flowing(up) {
            SWIM_UP_AGAINST_CURRENT_COST
        } else {
            SWIM_UP_ONE_BLOCK_COST
        };
        cost += SUBMERGED_PENALTY;
        push(
            ctx,
            up,
            cost,
            MoveData {
                execute: &execute_swim_up,
                is_reached: &default_is_reached,
            },
        );
    }

    let down = node.down(1);
    if is_water_node(ctx, down) {
        let cost = SINK_ONE_BLOCK_COST + SUBMERGED_PENALTY;
        push(
            ctx,
            down,
            cost,
            MoveData {
                execute: &execute_sink,
                is_reached: &sink_is_reached,
            },
        );
    }
}

fn swim_ashore(ctx: &mut MovesCtx, node: RelBlockPos) {
    if submerged(ctx, node) {
        return;
    }
    let base = if ctx.world.is_block_flowing(node) {
        SWIM_AGAINST_CURRENT_COST
    } else {
        SWIM_ONE_BLOCK_COST
    };
    for dir in CardinalDirection::iter() {
        let to = node + RelBlockPos::new(dir.x(), 0, dir.z());
        if !ctx.world.is_standable(to) {
            continue;
        }
        push(
            ctx,
            to,
            base,
            MoveData {
                execute: &execute_swim_ashore,
                is_reached: &default_is_reached,
            },
        );
    }
}

fn wade_in(ctx: &mut MovesCtx, node: RelBlockPos) {
    for dir in CardinalDirection::iter() {
        let to = node + RelBlockPos::new(dir.x(), 0, dir.z());
        if !is_water_node(ctx, to) {
            continue;
        }
        let cost = ENTER_WATER_PENALTY + level_cost(ctx, node, to);
        push(
            ctx,
            to,
            cost,
            MoveData {
                execute: &execute_swim,
                is_reached: &default_is_reached,
            },
        );
    }
}

const CENTRE_TOLERANCE: f64 = 0.2;

fn centre(ctx: &mut ExecuteCtx) {
    let c = ctx.target.center();
    let (dx, dz) = (c.x - ctx.position.x, c.z - ctx.position.z);
    if dx * dx + dz * dz > CENTRE_TOLERANCE * CENTRE_TOLERANCE {
        ctx.walk_world(dx, dz);
    } else {
        ctx.walk(WalkDirection::None);
    }
}

fn execute_swim(mut ctx: ExecuteCtx) {
    let target = ctx.target;
    let head = ctx.get_block_state(target.up(1));
    let surface = is_block_state_passable(head) && !is_block_state_water(head);
    ctx.look_at(target.center());
    if surface {
        ctx.walk(WalkDirection::Forward);
        ctx.jump();
    } else {
        ctx.sprint(SprintDirection::Forward);
    }
}

fn execute_swim_up(mut ctx: ExecuteCtx) {
    centre(&mut ctx);
    ctx.jump();
}

fn execute_sink(mut ctx: ExecuteCtx) {
    centre(&mut ctx);
    ctx.sneak();
}

fn execute_swim_ashore(mut ctx: ExecuteCtx) {
    let target = ctx.target;
    ctx.look_at(target.center());
    ctx.walk(WalkDirection::Forward);
    ctx.jump_if_in_water();
}

#[must_use]
pub fn sink_is_reached(
    IsReachedCtx {
        position, target, ..
    }: IsReachedCtx,
) -> bool {
    let feet = player_pos_to_block_pos(position);
    feet.x == target.x && feet.z == target.z && feet.y <= target.y
}
