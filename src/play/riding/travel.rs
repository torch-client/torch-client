use azalea::block::fluid_state::FluidKind;
use azalea::entity::{Physics, move_relative};
use azalea::physics::collision::{MoveCtx, move_colliding};
use azalea::physics::travel::{fluid_jump_threshold, travel_ctx};
use azalea::world::World;
use azalea_core::position::{BlockPos, Vec3};

use super::mount::Mount;

pub(super) fn travel(
    mount: Mount,
    ctx: &mut MoveCtx,
    input: Vec3,
    speed: f64,
    flying_speed: f64,
    refuse: bool,
) {
    match mount {
        Mount::HappyGhast => flying(ctx, input, (flying_speed * 5.0 / 3.0) as f32),
        Mount::Nautilus if ctx.physics.is_in_water() => {
            move_relative(ctx.physics, ctx.direction, speed as f32, input);
            let velocity = ctx.physics.velocity;
            move_colliding(ctx, velocity);
            ctx.physics.velocity *= 0.9;
        }
        _ => {
            let mut input = input;
            if refuse && ctx.physics.on_ground() {
                ctx.physics.velocity.x = 0.0;
                ctx.physics.velocity.z = 0.0;
                input.x = 0.0;
                input.z = 0.0;
            }
            let in_water = ctx.physics.is_in_water();
            ctx.physics.x_acceleration = input.x as f32;
            ctx.physics.y_acceleration = input.y as f32;
            ctx.physics.z_acceleration = input.z as f32;
            travel_ctx(ctx);
            if in_water
                && mount.floats_while_ridden()
                && ctx.physics.water_fluid_height > fluid_jump_threshold()
            {
                ctx.physics.velocity.y += 0.03999999910593033;
            }
        }
    }
}

fn flying(ctx: &mut MoveCtx, input: Vec3, speed: f32) {
    let drag = if ctx.physics.is_in_water() {
        0.800000011920929
    } else if ctx.physics.is_in_lava() {
        0.5
    } else {
        0.9100000262260437
    };
    move_relative(ctx.physics, ctx.direction, speed, input);
    let velocity = ctx.physics.velocity;
    move_colliding(ctx, velocity);
    ctx.physics.velocity *= drag;
}

pub(super) fn float_strider(physics: &mut Physics, position: Vec3, world: &World) {
    if !physics.is_in_lava() {
        return;
    }
    let block = BlockPos::from(position);
    let above_slab = position.y > block.y as f64 + 0.5 - 9.999999747378752e-6;
    let lava_above = world
        .get_fluid_state(block.up(1))
        .is_some_and(|fluid| fluid.kind == FluidKind::Lava);
    if above_slab && !lava_above {
        physics.set_on_ground(true);
    } else {
        physics.velocity = physics.velocity * 0.5 + Vec3::new(0.0, 0.05, 0.0);
    }
}
