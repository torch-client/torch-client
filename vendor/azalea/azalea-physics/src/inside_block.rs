use azalea_block::{BlockState, fluid_state::FluidState, properties};
use azalea_core::position::{BlockPos, Vec3};
use azalea_entity::{ActiveEffects, Physics, PlayerAbilities};
use azalea_registry::builtin::{BlockKind, MobEffect};
use azalea_world::World;

use crate::collision::BlockWithShape;

const COBWEB: Vec3 = Vec3 {
    x: 0.25,
    y: 0.05000000074505806,
    z: 0.25,
};
const COBWEB_WEAVING: Vec3 = Vec3 {
    x: 0.5,
    y: 0.25,
    z: 0.5,
};
const POWDER_SNOW: Vec3 = Vec3 {
    x: 0.8999999761581421,
    y: 1.5,
    z: 0.8999999761581421,
};
const SWEET_BERRY_BUSH: Vec3 = Vec3 {
    x: 0.800000011920929,
    y: 0.75,
    z: 0.800000011920929,
};

pub(crate) struct InsideBlockCtx<'a> {
    pub active_effects: &'a ActiveEffects,
    pub abilities: Option<&'a PlayerAbilities>,
    pub position: Vec3,
    pub width: f32,
}

impl InsideBlockCtx<'_> {
    fn make_stuck(&self, physics: &mut Physics, multiplier: Vec3) {
        if self.abilities.is_some_and(|abilities| abilities.flying) {
            return;
        }
        physics.make_stuck_in_block(multiplier);
    }
}

pub(crate) fn handle_entity_inside_block(
    world: &World,
    block: BlockState,
    block_pos: BlockPos,
    physics: &mut Physics,
    ctx: &InsideBlockCtx,
) {
    match BlockKind::from(block) {
        BlockKind::Cobweb => {
            let multiplier = if ctx.active_effects.get(MobEffect::Weaving).is_some() {
                COBWEB_WEAVING
            } else {
                COBWEB
            };
            ctx.make_stuck(physics, multiplier);
        }
        BlockKind::PowderSnow => {
            let in_block_state = world
                .get_block_state(BlockPos::from(ctx.position))
                .unwrap_or_default();
            if BlockKind::from(in_block_state) == BlockKind::PowderSnow {
                ctx.make_stuck(physics, POWDER_SNOW);
            }
        }
        BlockKind::SweetBerryBush => ctx.make_stuck(physics, SWEET_BERRY_BUSH),
        BlockKind::HoneyBlock => {
            if is_sliding_down(block_pos, physics, ctx) {
                do_slide_movement(physics);
            }
        }
        BlockKind::BubbleColumn => bubble_column(world, block, block_pos, physics),
        _ => {}
    }
}

fn old_delta_y(delta_y: f64) -> f64 {
    delta_y / 0.9800000190734863 + 0.08
}

fn new_delta_y(delta_y: f64) -> f64 {
    (delta_y - 0.08) * 0.9800000190734863
}

fn is_sliding_down(block_pos: BlockPos, physics: &Physics, ctx: &InsideBlockCtx) -> bool {
    if physics.on_ground() {
        return false;
    }
    if ctx.position.y > f64::from(block_pos.y) + 0.9375 - 1.0e-7 {
        return false;
    }
    if old_delta_y(physics.velocity.y) >= -0.08 {
        return false;
    }

    let dx = f64::abs(f64::from(block_pos.x) + 0.5 - ctx.position.x);
    let dz = f64::abs(f64::from(block_pos.z) + 0.5 - ctx.position.z);
    let overlap = 0.4375 + f64::from(ctx.width / 2.0);
    dx + 1.0e-7 > overlap || dz + 1.0e-7 > overlap
}

fn do_slide_movement(physics: &mut Physics) {
    let old_y = old_delta_y(physics.velocity.y);
    let velocity = physics.velocity;
    physics.velocity = if old_y < -0.13 {
        let horizontal = -0.05 / old_y;
        Vec3 {
            x: velocity.x * horizontal,
            y: new_delta_y(-0.05),
            z: velocity.z * horizontal,
        }
    } else {
        Vec3 {
            y: new_delta_y(-0.05),
            ..velocity
        }
    };
    physics.reset_fall_distance();
}

fn bubble_column(world: &World, block: BlockState, block_pos: BlockPos, physics: &mut Physics) {
    let block_above = world.get_block_state(block_pos.up(1)).unwrap_or_default();
    let is_block_above_empty =
        block_above.is_collision_shape_empty() && FluidState::from(block_above).is_empty();
    let drag_down = block
        .property::<properties::Drag>()
        .expect("drag property should always be present on bubble columns");
    let velocity = &mut physics.velocity;

    if is_block_above_empty {
        velocity.y = if drag_down {
            f64::max(-0.9, velocity.y - 0.03)
        } else {
            f64::min(1.8, velocity.y + 0.1)
        };
    } else {
        velocity.y = if drag_down {
            f64::max(-0.3, velocity.y - 0.03)
        } else {
            f64::min(0.7, velocity.y + 0.06)
        };
        physics.reset_fall_distance();
    }
}
