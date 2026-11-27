use std::sync::LazyLock;

use num_traits::Float;

pub const WALK_ONE_BLOCK_COST: f32 = 20. / 4.317;
pub const SPRINT_ONE_BLOCK_COST: f32 = 20. / 5.612;
pub const WALK_OFF_BLOCK_COST: f32 = WALK_ONE_BLOCK_COST * 0.8;
pub const SPRINT_MULTIPLIER: f32 = SPRINT_ONE_BLOCK_COST / WALK_ONE_BLOCK_COST;
pub const JUMP_PENALTY: f32 = 2.;
pub const ENTER_WATER_PENALTY: f32 = 3.;
pub const CENTER_AFTER_FALL_COST: f32 = WALK_ONE_BLOCK_COST - WALK_OFF_BLOCK_COST;
pub const WALK_ONE_IN_WATER_COST: f32 = 20. / 1.960;
pub const SNEAK_ONE_BLOCK_COST: f32 = 20. / 1.3;

const WATER_ACCEL: f32 = 0.02 * 0.98;
const WATER_DRAG: f32 = 0.8;
const SPRINTING_WATER_DRAG: f32 = 0.9;
const VERTICAL_WATER_DRAG: f32 = 0.8;
const WATER_JUMP: f32 = 0.04;
const WATER_SINK: f32 = 0.08 / 16.;
const WATER_CURRENT: f32 = 0.014;

pub const SWIM_ONE_BLOCK_COST: f32 = (1. - WATER_DRAG) / WATER_ACCEL;
pub const SWIM_SPRINT_ONE_BLOCK_COST: f32 = (1. - SPRINTING_WATER_DRAG) / WATER_ACCEL;
pub const SWIM_AGAINST_CURRENT_COST: f32 = (1. - WATER_DRAG) / (WATER_ACCEL - WATER_CURRENT);
pub const SWIM_SPRINT_AGAINST_CURRENT_COST: f32 =
    (1. - SPRINTING_WATER_DRAG) / (WATER_ACCEL - WATER_CURRENT);
pub const SWIM_UP_ONE_BLOCK_COST: f32 = 1.
    / ((VERTICAL_WATER_DRAG * WATER_JUMP - WATER_SINK) / (1. - VERTICAL_WATER_DRAG) + WATER_JUMP);
pub const SWIM_UP_AGAINST_CURRENT_COST: f32 = 1.
    / ((VERTICAL_WATER_DRAG * (WATER_JUMP - WATER_CURRENT) - WATER_SINK)
        / (1. - VERTICAL_WATER_DRAG)
        + (WATER_JUMP - WATER_CURRENT));
pub const SINK_ONE_BLOCK_COST: f32 = (1. - VERTICAL_WATER_DRAG) / WATER_SINK;
pub const SUBMERGED_PENALTY: f32 = 6.;

pub const SOURCE_SURFACE: f32 = 8. / 9.;
pub const FLOWING_SURFACE: f32 = 1. / 9.;

pub fn fall_step_at(blocks: u32) -> f32 {
    match FALL_N_BLOCKS_COST.get(blocks as usize) {
        Some(&ticks) => velocity(ticks as u32 + 1),
        None => 3.92,
    }
}

pub fn water_landing_is_safe(drop: u32, depth: u32, top_is_source: bool) -> bool {
    if depth == 0 {
        return false;
    }
    let surface = if top_is_source {
        SOURCE_SURFACE
    } else {
        FLOWING_SURFACE
    };
    fall_step_at(drop) < (depth - 1) as f32 + surface
}

pub const COST_HEURISTIC: f32 = 3.563;

pub const BLOCK_BREAK_ADDITIONAL_PENALTY: f32 = 2.;

pub static FALL_1_25_BLOCKS_COST: LazyLock<f32> = LazyLock::new(|| distance_to_ticks(1.25));
pub static FALL_0_25_BLOCKS_COST: LazyLock<f32> = LazyLock::new(|| distance_to_ticks(0.25));
pub static JUMP_ONE_BLOCK_COST: LazyLock<f32> =
    LazyLock::new(|| *FALL_1_25_BLOCKS_COST - *FALL_0_25_BLOCKS_COST);

pub static FALL_N_BLOCKS_COST: LazyLock<[f32; 4097]> = LazyLock::new(|| {

    let mut fall_n_blocks_cost = [0.; 4097];
    let mut last_distance_blocks = 0;
    let mut current_distance = 0.;
    let mut tick_count = 0;

    'outer: loop {
        let fall_distance_per_tick = velocity(tick_count);

        let current_distance_blocks = (current_distance + fall_distance_per_tick) as usize;
        if current_distance_blocks > last_distance_blocks {
            for blocks in (last_distance_blocks + 1)..=current_distance_blocks {
                if blocks == fall_n_blocks_cost.len() {
                    break 'outer;
                }

                fall_n_blocks_cost[blocks] =
                    tick_count as f32 + (blocks as f32 - current_distance) / fall_distance_per_tick;
            }

            last_distance_blocks = current_distance_blocks;
        }

        current_distance += fall_distance_per_tick;
        tick_count += 1;
    }

    fall_n_blocks_cost
});

fn velocity(ticks: u32) -> f32 {
    (0.98.powi(ticks as i32) - 1.) * -3.92
}

fn distance_to_ticks(distance: f32) -> f32 {
    if distance == 0. {
        return 0.;
    }
    let mut tick_count = 0;
    let mut remaining_distance = distance;
    loop {
        let fall_distance_per_tick = velocity(tick_count);
        if fall_distance_per_tick >= remaining_distance {
            return (tick_count as f32) + (remaining_distance / fall_distance_per_tick);
        }
        remaining_distance -= fall_distance_per_tick;
        tick_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use crate::pathfinder::costs::{FALL_N_BLOCKS_COST, distance_to_ticks};

    #[test]
    fn test_fall_n_blocks_cost() {
        for i in 0..4 {
            let a = FALL_N_BLOCKS_COST[i];
            let b = distance_to_ticks(i as f32);
            assert!((a - b).abs() < 0.1, "{a} != {b}");
        }
    }

    fn settle(push: f32, drag: f32, after: f32) -> f32 {
        let mut v = 0f32;
        let mut moved = 0.;
        for _ in 0..500 {
            moved = v + push;
            v = moved * drag - after;
        }
        moved
    }

    #[test]
    fn swim_costs_match_the_water_physics() {
        use super::*;
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(close(1. / settle(0.02 * 0.98, 0.8, 0.), SWIM_ONE_BLOCK_COST));
        assert!(close(SWIM_ONE_BLOCK_COST, 10.204));
        assert!(close(SWIM_ONE_BLOCK_COST, WALK_ONE_IN_WATER_COST));
        assert!(close(1. / settle(0.02 * 0.98, 0.9, 0.), SWIM_SPRINT_ONE_BLOCK_COST));
        assert!(close(SWIM_SPRINT_ONE_BLOCK_COST, 5.102));
        assert!(close(1. / settle(0.0196 - 0.014, 0.8, 0.), SWIM_AGAINST_CURRENT_COST));
        assert!(close(SWIM_AGAINST_CURRENT_COST, 35.714));
        assert!(close(1. / settle(0.0196 - 0.014, 0.9, 0.), SWIM_SPRINT_AGAINST_CURRENT_COST));
        assert!(close(1. / settle(0.04, 0.8, 0.005), SWIM_UP_ONE_BLOCK_COST));
        assert!(close(SWIM_UP_ONE_BLOCK_COST, 5.714));
        assert!(close(1. / settle(0.04 - 0.014, 0.8, 0.005), SWIM_UP_AGAINST_CURRENT_COST));
        assert!(close(SWIM_UP_AGAINST_CURRENT_COST, 9.524));
        assert!(close(1. / -settle(0., 0.8, 0.005), SINK_ONE_BLOCK_COST));
        assert!(close(SINK_ONE_BLOCK_COST, 40.));
        assert!(SWIM_SPRINT_ONE_BLOCK_COST + SUBMERGED_PENALTY > SWIM_ONE_BLOCK_COST);
    }

    #[test]
    fn water_landing_needs_depth_for_the_impact_speed() {
        use super::*;
        assert!(water_landing_is_safe(1, 1, true));
        assert!(water_landing_is_safe(4, 1, true));
        assert!(!water_landing_is_safe(5, 1, true));
        assert!(!water_landing_is_safe(10, 1, true));
        assert!(water_landing_is_safe(10, 2, true));
        assert!(!water_landing_is_safe(10, 1, false));
        assert!(water_landing_is_safe(10, 3, false));
        assert!(!water_landing_is_safe(5, 0, true));
        for b in 1..64 {
            assert!(fall_step_at(b + 1) >= fall_step_at(b));
        }
    }
}
