use azalea_core::{
    aabb::Aabb,
    direction::Axis,
    math,
    position::{BlockPos, Vec3},
};
use azalea_entity::{Position, view_vector};
use azalea_registry::builtin::MobEffect;
use azalea_world::World;

use super::{
    BlockWithShape, MoveCtx, VoxelShape, entity_collisions::get_entity_collisions,
    world_collisions::get_block_collisions_for,
};
use crate::block_jump_factor;

pub fn update_auto_jump(ctx: &mut MoveCtx, start: Vec3) {
    let Some(state) = ctx.physics_state else {
        return;
    };
    let position = **ctx.position;
    if !state.auto_jump
        || ctx.physics.auto_jump_time > 0
        || !ctx.physics.on_ground()
        || state.move_vector.length_squared() <= 0.
        || block_jump_factor(ctx.world, Position::new(position)) < 1.
    {
        return;
    }

    let (xa, za) = ((position.x - start.x) as f32, (position.z - start.z) as f32);
    let mut move_diff = Vec3::new(xa as f64, 0., za as f64);
    let move_end = position + move_diff;
    let speed = ctx.attributes.movement_speed.calculate() as f32;
    let mut move_dist_sq = move_diff.length_squared() as f32;
    if move_dist_sq <= 0.001 {
        let input_xa = speed * state.move_vector.x;
        let input_za = speed * state.move_vector.y;
        let y_rot = ctx.direction.y_rot() * 0.017453292;
        let (sin, cos) = (math::sin(y_rot), math::cos(y_rot));
        move_diff = Vec3::new(
            (input_xa * cos - input_za * sin) as f64,
            0.,
            (input_za * cos + input_xa * sin) as f64,
        );
        move_dist_sq = move_diff.length_squared() as f32;
        if move_dist_sq <= 0.001 {
            return;
        }
    }

    let inverted = 1. / move_dist_sq.sqrt();
    let move_dir = move_diff * inverted as f64;
    let forward = view_vector(ctx.direction);
    if ((forward.x * move_dir.x + forward.z * move_dir.z) as f32) < -0.15 {
        return;
    }

    let world = ctx.world;
    let bounds = ctx.physics.bounding_box;
    let width = (bounds.max.x - bounds.min.x) as f32;
    let height = (bounds.max.y - bounds.min.y) as f32;

    let mut ceiling = BlockPos::from(position.up(height as f64));
    if !collision_shape(world, ceiling).is_empty() {
        return;
    }
    ceiling = ceiling.up(1);
    if !collision_shape(world, ceiling).is_empty() {
        return;
    }

    let mut jump_height = 1.2f32;
    if let Some(level) = ctx.active_effects.and_then(|e| e.get_level(MobEffect::JumpBoost)) {
        jump_height += (level + 1) as f32 * 0.75;
    }

    let look_ahead = (speed * 7.).max(1. / inverted);
    let seg_end = move_end + move_dir * look_ahead as f64;
    let top = seg_end.up(height as f64);
    let test_box = Aabb {
        min: position.min(top),
        max: position.max(top),
    }
    .inflate(Vec3::new(width as f64, 0., width as f64));

    let seg_begin = position.up(0.5099999904632568);
    let seg_end = seg_end.up(0.5099999904632568);
    let right = Vec3::new(-move_dir.z, 0., move_dir.x) * (width * 0.5) as f64;
    let (left_begin, left_end) = (seg_begin - right, seg_end - right);
    let (right_begin, right_end) = (seg_begin + right, seg_end + right);

    let mut shapes = get_entity_collisions(
        world,
        &test_box,
        Some(ctx.source_entity),
        ctx.aabb_query,
        ctx.collidable_entity_query,
    );
    shapes.extend(get_block_collisions_for(world, &test_box, ctx.powder_snow()));

    let mut obstacle_height = None;
    for shape_box in shapes.iter().flat_map(|shape| shape.to_aabbs()) {
        if !shape_box.intersects_vec3(left_begin, left_end)
            && !shape_box.intersects_vec3(right_begin, right_end)
        {
            continue;
        }
        let mut obstacle = shape_box.max.y as f32;
        let obstacle_pos = BlockPos::from(shape_box.get_center());
        let mut steps = 1;
        while (steps as f32) < jump_height {
            let above = obstacle_pos.up(steps);
            let shape = collision_shape(world, above);
            if !shape.is_empty() {
                obstacle = (*shape).max(Axis::Y) as f32 + above.y as f32;
                if obstacle as f64 - position.y > jump_height as f64 {
                    return;
                }
            }
            if steps > 1 {
                ceiling = ceiling.up(1);
                if !collision_shape(world, ceiling).is_empty() {
                    return;
                }
            }
            steps += 1;
        }
        obstacle_height = Some(obstacle);
        break;
    }

    if let Some(obstacle) = obstacle_height {
        let y_delta = (obstacle as f64 - position.y) as f32;
        if y_delta > 0.5 && y_delta <= jump_height {
            ctx.physics.auto_jump_time = 1;
        }
    }
}

fn collision_shape(world: &World, pos: BlockPos) -> std::borrow::Cow<'static, VoxelShape> {
    world.get_block_state(pos).unwrap_or_default().collision_shape(pos)
}
