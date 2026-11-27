use azalea_client::WalkDirection;
use azalea_core::direction::CardinalDirection;

use super::{Edge, ExecuteCtx, IsReachedCtx, MoveData, MovesCtx, default_is_reached};
use crate::pathfinder::{
    astar,
    costs::{JUMP_ONE_BLOCK_COST, JUMP_PENALTY, SNEAK_ONE_BLOCK_COST},
    placing::PlaceRules,
    player_pos_to_block_pos,
    positions::RelBlockPos,
};

const UNPLACED_FLOOR_PENALTY: f32 = 0.1;

pub fn place_move(ctx: &mut MovesCtx, node: RelBlockPos) {
    let Some(rules) = ctx.placing else {
        return;
    };
    if ctx.world.is_block_water(node) {
        return;
    }
    let Some(floor_penalty) = floor_support(ctx, rules, node.down(1)) else {
        return;
    };
    pillar_move(ctx, rules, node, floor_penalty);
    bridge_move(ctx, rules, node, floor_penalty);
}

fn floor_support(ctx: &MovesCtx, rules: &dyn PlaceRules, floor: RelBlockPos) -> Option<f32> {
    if ctx.world.is_block_solid(floor) {
        return Some(0.);
    }
    if ctx.world.is_block_passable(floor)
        && rules
            .place_cost(floor.apply(ctx.world.origin()))
            .is_finite()
    {
        return Some(UNPLACED_FLOOR_PENALTY);
    }
    None
}

fn pillar_move(ctx: &mut MovesCtx, rules: &dyn PlaceRules, pos: RelBlockPos, floor_penalty: f32) {
    if !ctx.world.is_block_passable(pos) {
        return;
    }
    let place_cost = rules.place_cost(pos.apply(ctx.world.origin()));
    if !place_cost.is_finite() {
        return;
    }
    let head_cost = ctx
        .world
        .cost_for_breaking_block(pos.up(2), ctx.mining_cache);
    if head_cost == f32::INFINITY {
        return;
    }

    let cost = *JUMP_ONE_BLOCK_COST + JUMP_PENALTY + place_cost + floor_penalty + head_cost;

    ctx.edges.push(Edge {
        movement: astar::Movement {
            target: pos.up(1),
            data: MoveData {
                execute: &execute_pillar_move,
                is_reached: &pillar_is_reached,
            },
        },
        cost,
    });
}

const PILLAR_CENTRE_TOLERANCE: f64 = 0.2;

fn execute_pillar_move(mut ctx: ExecuteCtx) {
    let base = ctx.target.down(1);

    if ctx.mine_while_at_start(ctx.target.up(1)) {
        return;
    }

    let centre = base.center();
    let (dx, dz) = (centre.x - ctx.position.x, centre.z - ctx.position.z);
    let off_centre = (dx * dx + dz * dz).sqrt();
    if off_centre > PILLAR_CENTRE_TOLERANCE {
        ctx.walk_world(dx, dz);
    }

    let on_ground = ctx.physics.on_ground();
    let in_column = player_pos_to_block_pos(ctx.position) == base;
    if ctx.place(base) && on_ground && in_column && off_centre <= PILLAR_CENTRE_TOLERANCE {
        ctx.jump();
    }
}

#[must_use]
pub fn pillar_is_reached(
    IsReachedCtx {
        target,
        position,
        physics,
        ..
    }: IsReachedCtx,
) -> bool {
    player_pos_to_block_pos(position) == target && physics.on_ground()
}

fn bridge_move(ctx: &mut MovesCtx, rules: &dyn PlaceRules, pos: RelBlockPos, floor_penalty: f32) {
    for dir in CardinalDirection::iter() {
        let dest = pos + RelBlockPos::new(dir.x(), 0, dir.z());
        let floor = dest.down(1);

        if ctx.world.is_block_standable(floor) {
            continue;
        }
        if !ctx.world.is_block_passable(floor) && !ctx.world.is_block_water(floor) {
            continue;
        }
        if ctx.world.is_block_passable(floor) && ctx.world.lands_in_danger(dest, false) {
            continue;
        }
        let place_cost = rules.place_cost(floor.apply(ctx.world.origin()));
        if !place_cost.is_finite() {
            continue;
        }
        let pass_cost = ctx.world.cost_for_passing(dest, ctx.mining_cache);
        if pass_cost == f32::INFINITY {
            continue;
        }

        let cost = SNEAK_ONE_BLOCK_COST + place_cost + floor_penalty + pass_cost;

        ctx.edges.push(Edge {
            movement: astar::Movement {
                target: dest,
                data: MoveData {
                    execute: &execute_bridge_move,
                    is_reached: &bridge_is_reached,
                },
            },
            cost,
        });
    }
}

const BRIDGE_REACHED_PAST_FACE: f64 = 0.35;

#[must_use]
pub fn bridge_is_reached(ctx: IsReachedCtx) -> bool {
    let (start, target, position) = (ctx.start, ctx.target, ctx.position);
    let (ax, az) = (
        (target.x - start.x).signum() as f64,
        (target.z - start.z).signum() as f64,
    );
    let start_centre = start.center();
    let past_face = (position.x - (start_centre.x + 0.5 * ax)) * ax
        + (position.z - (start_centre.z + 0.5 * az)) * az;
    past_face > BRIDGE_REACHED_PAST_FACE && default_is_reached(ctx)
}

fn execute_bridge_move(mut ctx: ExecuteCtx) {
    let target = ctx.target;

    if ctx.mine_while_at_start(target.up(1)) {
        return;
    }
    if ctx.mine_while_at_start(target) {
        return;
    }

    if ctx.place(target.down(1)) {
        ctx.sneak();
        let target_centre = target.center();
        ctx.walk_world(
            target_centre.x - ctx.position.x,
            target_centre.z - ctx.position.z,
        );
        return;
    }

    ctx.look_at(target.center());
    ctx.walk(WalkDirection::Forward);
}
