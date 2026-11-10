use std::{borrow::Cow, collections::HashSet, sync::LazyLock};

use azalea_block::{
    BlockState,
    fluid_state::{FluidKind, FluidState},
};
use azalea_core::{
    aabb::Aabb,
    direction::{Axis, Direction},
    hit_result::BlockHitResult,
    math::{self, EPSILON, lerp},
    position::{BlockPos, Vec3},
};
use azalea_registry::{
    builtin::{BlockKind, ItemKind},
    tags,
};
use azalea_world::ChunkStorage;

use crate::collision::{BLOCK_SHAPE, BlockWithShape, EMPTY_SHAPE, Shapes, VoxelShape, box_shape};

#[derive(Clone, Debug)]
pub struct ClipContext {
    pub from: Vec3,
    pub to: Vec3,
    pub block_shape_type: BlockShapeType,
    pub fluid_pick_type: FluidPickType,
    pub held_item: ItemKind,
    pub entity: Option<ClipEntity>,
}

#[derive(Clone, Copy, Debug)]
pub struct ClipEntity {
    pub bottom: f64,
    pub descending: bool,
}
impl ClipContext {
    pub fn block_shape(&self, block_state: BlockState, pos: BlockPos) -> Cow<'static, VoxelShape> {
        match self.block_shape_type {
            BlockShapeType::Collider => block_state.collision_shape(pos),
            BlockShapeType::Outline => match holding_outline(block_state, self.held_item) {
                Some(shape) => Cow::Borrowed(shape),
                None => block_state.outline_shape(pos),
            },
            BlockShapeType::Visual => visual_shape(block_state, pos, self.entity),
            BlockShapeType::FallDamageResetting => {
                if tags::blocks::FALL_DAMAGE_RESETTING.contains(&BlockKind::from(block_state)) {
                    block_state.collision_shape(pos)
                } else {
                    Cow::Borrowed(&EMPTY_SHAPE)
                }
            }
        }
    }

    pub fn fluid_shape(
        &self,
        fluid_state: FluidState,
        world: &ChunkStorage,
        pos: BlockPos,
    ) -> &VoxelShape {
        if self.fluid_pick_type.can_pick(&fluid_state) {
            crate::collision::fluid_shape(&fluid_state, world, pos)
        } else {
            &EMPTY_SHAPE
        }
    }
}

pub fn holding_outline(block_state: BlockState, held_item: ItemKind) -> Option<&'static VoxelShape> {
    let full = match BlockKind::from(block_state) {
        BlockKind::Scaffolding => held_item == ItemKind::Scaffolding,
        BlockKind::Light => held_item == ItemKind::Light,
        _ => false,
    };
    full.then_some(&*BLOCK_SHAPE)
}

static SNOW_LAYER_SHAPES: LazyLock<[VoxelShape; 9]> =
    LazyLock::new(|| std::array::from_fn(|layers| box_shape(0.0, 0.0, 0.0, 1.0, layers as f64 / 8.0, 1.0)));

pub fn visual_shape(
    block_state: BlockState,
    pos: BlockPos,
    entity: Option<ClipEntity>,
) -> Cow<'static, VoxelShape> {
    use azalea_block::properties::Layers;

    let kind = BlockKind::from(block_state);
    match kind {
        BlockKind::SoulSand | BlockKind::Mud => return Cow::Borrowed(&*BLOCK_SHAPE),
        BlockKind::PowderSnow => return Cow::Borrowed(&*EMPTY_SHAPE),
        BlockKind::Snow => {
            let layers = block_state.property::<Layers>().map_or(1, |l| l as usize + 1);
            return Cow::Borrowed(&SNOW_LAYER_SHAPES[layers]);
        }
        BlockKind::Scaffolding => {
            let context = crate::collision::world_collisions::PowderSnowCollision {
                has_entity: entity.is_some(),
                entity_bottom: entity.map_or(0.0, |e| e.bottom),
                descending: entity.is_some_and(|e| e.descending),
                ..Default::default()
            };
            return Cow::Borrowed(
                crate::collision::world_collisions::scaffolding_shape(&context, block_state, pos)
                    .unwrap_or(&*EMPTY_SHAPE),
            );
        }
        _ => {}
    }
    if tags::blocks::FENCES.contains(&kind) {
        return block_state.outline_shape(pos);
    }
    let id = kind.to_str();
    let transparent = matches!(kind, BlockKind::Glass | BlockKind::TintedGlass)
        || id.ends_with("_stained_glass")
        || id.ends_with("copper_grate");
    let bars = kind == BlockKind::IronBars || id.ends_with("copper_bars") || id.ends_with("glass_pane");
    if transparent || bars {
        return Cow::Borrowed(&*EMPTY_SHAPE);
    }
    block_state.collision_shape(pos)
}

static CAULDRON_INSIDE: LazyLock<VoxelShape> =
    LazyLock::new(|| box_shape(0.125, 0.25, 0.125, 0.875, 1.0, 0.875));

static HOPPER_INTERACTION: LazyLock<[VoxelShape; 5]> = LazyLock::new(|| {
    let inside = || box_shape(0.125, 0.6875, 0.125, 0.875, 1.0, 0.875);
    let spout = |x0: f64, z0: f64, x1: f64, z1: f64| {
        Shapes::or(inside(), box_shape(x0, 0.5, z0, x1, 0.625, z1))
    };
    [
        inside(),
        spout(0.375, 0.0, 0.625, 0.25),
        spout(0.375, 0.75, 0.625, 1.0),
        spout(0.0, 0.375, 0.25, 0.625),
        spout(0.75, 0.375, 1.0, 0.625),
    ]
});

pub fn interaction_shape(block_state: BlockState) -> &'static VoxelShape {
    use azalea_block::properties::Facing;

    match BlockKind::from(block_state) {
        BlockKind::Scaffolding | BlockKind::Composter => &*BLOCK_SHAPE,
        BlockKind::Cauldron
        | BlockKind::WaterCauldron
        | BlockKind::LavaCauldron
        | BlockKind::PowderSnowCauldron => &*CAULDRON_INSIDE,
        BlockKind::Hopper => {
            let i = match block_state.property::<Facing>().unwrap_or(Facing::Down) {
                Facing::Down => 0,
                Facing::North => 1,
                Facing::South => 2,
                Facing::West => 3,
                Facing::East => 4,
            };
            &HOPPER_INTERACTION[i]
        }
        _ => &*EMPTY_SHAPE,
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BlockShapeType {
    Collider,
    Outline,
    Visual,
    FallDamageResetting,
}
#[derive(Clone, Copy, Debug)]
pub enum FluidPickType {
    None,
    SourceOnly,
    Any,
    Water,
}
impl FluidPickType {
    pub fn can_pick(&self, fluid_state: &FluidState) -> bool {
        match self {
            Self::None => false,
            Self::SourceOnly => fluid_state.amount == 8,
            Self::Any => fluid_state.kind != FluidKind::Empty,
            Self::Water => fluid_state.kind == FluidKind::Water,
        }
    }
}

pub fn clip(chunk_storage: &ChunkStorage, context: ClipContext) -> BlockHitResult {
    traverse_blocks(
        context.from,
        context.to,
        context,
        |ctx, block_pos| {
            let block_state = chunk_storage.get_block_state(block_pos).unwrap_or_default();
            let fluid_state = FluidState::from(block_state);

            let block_shape = ctx.block_shape(block_state, block_pos);
            let interaction_clip = clip_with_interaction_override(
                ctx.from,
                ctx.to,
                block_pos,
                &block_shape,
                block_state,
            );
            let fluid_shape = ctx.fluid_shape(fluid_state, chunk_storage, block_pos);
            let fluid_clip = fluid_shape.clip(ctx.from, ctx.to, block_pos);

            let distance_to_interaction = interaction_clip
                .as_ref()
                .map(|hit| ctx.from.distance_squared_to(hit.location))
                .unwrap_or(f64::MAX);
            let distance_to_fluid = fluid_clip
                .as_ref()
                .map(|hit| ctx.from.distance_squared_to(hit.location))
                .unwrap_or(f64::MAX);

            if distance_to_interaction <= distance_to_fluid {
                interaction_clip
            } else {
                fluid_clip
            }
        },
        |context| {
            let vec = context.from - context.to;
            BlockHitResult::miss(context.to, Direction::nearest(vec))
        },
    )
}

fn clip_with_interaction_override(
    from: Vec3,
    to: Vec3,
    block_pos: BlockPos,
    block_shape: &VoxelShape,
    block_state: BlockState,
) -> Option<BlockHitResult> {
    let block_hit_result = block_shape.clip(from, to, block_pos);

    if let Some(block_hit_result) = block_hit_result {
        let interaction_shape = interaction_shape(block_state);
        let interaction_hit_result = interaction_shape.clip(from, to, block_pos);
        if let Some(interaction_hit_result) = interaction_hit_result
            && interaction_hit_result.location.distance_squared_to(from)
                < block_hit_result.location.distance_squared_to(from)
        {
            return Some(block_hit_result.with_direction(interaction_hit_result.direction));
        }

        Some(block_hit_result)
    } else {
        None
    }
}

pub fn traverse_blocks<C, T>(
    from: Vec3,
    to: Vec3,
    context: C,
    get_hit_result: impl Fn(&C, BlockPos) -> Option<T>,
    get_miss_result: impl Fn(&C) -> T,
) -> T {
    if from == to {
        return get_miss_result(&context);
    }

    let right_after_end = Vec3 {
        x: lerp(-EPSILON, to.x, from.x),
        y: lerp(-EPSILON, to.y, from.y),
        z: lerp(-EPSILON, to.z, from.z),
    };

    let right_before_start = Vec3 {
        x: lerp(-EPSILON, from.x, to.x),
        y: lerp(-EPSILON, from.y, to.y),
        z: lerp(-EPSILON, from.z, to.z),
    };

    let mut current_block = BlockPos::from(right_before_start);
    if let Some(data) = get_hit_result(&context, current_block) {
        return data;
    }

    let vec = right_after_end - right_before_start;

    let vec_sign = Vec3 {
        x: math::sign(vec.x),
        y: math::sign(vec.y),
        z: math::sign(vec.z),
    };

    #[rustfmt::skip]
    let percentage_step = Vec3 {
        x: if vec_sign.x == 0. { f64::MAX } else { vec_sign.x / vec.x },
        y: if vec_sign.y == 0. { f64::MAX } else { vec_sign.y / vec.y },
        z: if vec_sign.z == 0. { f64::MAX } else { vec_sign.z / vec.z },
    };

    let mut percentage = Vec3 {
        x: percentage_step.x
            * if vec_sign.x > 0. {
                1. - math::fract(right_before_start.x)
            } else {
                math::fract(right_before_start.x)
            },
        y: percentage_step.y
            * if vec_sign.y > 0. {
                1. - math::fract(right_before_start.y)
            } else {
                math::fract(right_before_start.y)
            },
        z: percentage_step.z
            * if vec_sign.z > 0. {
                1. - math::fract(right_before_start.z)
            } else {
                math::fract(right_before_start.z)
            },
    };

    loop {
        if percentage.x > 1. && percentage.y > 1. && percentage.z > 1. {
            return get_miss_result(&context);
        }

        if percentage.x < percentage.y {
            if percentage.x < percentage.z {
                current_block.x += vec_sign.x as i32;
                percentage.x += percentage_step.x;
            } else {
                current_block.z += vec_sign.z as i32;
                percentage.z += percentage_step.z;
            }
        } else if percentage.y < percentage.z {
            current_block.y += vec_sign.y as i32;
            percentage.y += percentage_step.y;
        } else {
            current_block.z += vec_sign.z as i32;
            percentage.z += percentage_step.z;
        }

        if let Some(data) = get_hit_result(&context, current_block) {
            return data;
        }
    }
}

pub fn box_traverse_blocks(from: Vec3, to: Vec3, aabb: &Aabb) -> HashSet<BlockPos> {
    let delta = to - from;
    let traversed_blocks = BlockPos::between_closed_aabb(aabb);
    if delta.length_squared() < (0.99999_f32 * 0.99999) as f64 {
        return traversed_blocks.into_iter().collect();
    }

    let mut traversed_and_collided_blocks = HashSet::new();
    let target_min_pos = aabb.min;
    let from_min_pos = target_min_pos - delta;
    add_collisions_along_travel(
        &mut traversed_and_collided_blocks,
        from_min_pos,
        target_min_pos,
        *aabb,
    );
    traversed_and_collided_blocks.extend(traversed_blocks);
    traversed_and_collided_blocks
}

pub fn add_collisions_along_travel(
    collisions: &mut HashSet<BlockPos>,
    from: Vec3,
    to: Vec3,
    aabb: Aabb,
) {
    let delta = to - from;
    let mut min_x = from.x.floor() as i32;
    let mut min_y = from.y.floor() as i32;
    let mut min_z = from.z.floor() as i32;
    let direction_x = math::sign_as_int(delta.x);
    let direction_y = math::sign_as_int(delta.y);
    let direction_z = math::sign_as_int(delta.z);
    let step_x = if direction_x == 0 {
        f64::MAX
    } else {
        direction_x as f64 / delta.x
    };
    let step_y = if direction_y == 0 {
        f64::MAX
    } else {
        direction_y as f64 / delta.y
    };
    let step_z = if direction_z == 0 {
        f64::MAX
    } else {
        direction_z as f64 / delta.z
    };
    let mut cur_x = step_x
        * if direction_x > 0 {
            1. - math::fract(from.x)
        } else {
            math::fract(from.x)
        };
    let mut cur_y = step_y
        * if direction_y > 0 {
            1. - math::fract(from.y)
        } else {
            math::fract(from.y)
        };
    let mut cur_z = step_z
        * if direction_z > 0 {
            1. - math::fract(from.z)
        } else {
            math::fract(from.z)
        };
    let mut step_count = 0;

    while cur_x <= 1. || cur_y <= 1. || cur_z <= 1. {
        if cur_x < cur_y {
            if cur_x < cur_z {
                min_x += direction_x;
                cur_x += step_x;
            } else {
                min_z += direction_z;
                cur_z += step_z;
            }
        } else if cur_y < cur_z {
            min_y += direction_y;
            cur_y += step_y;
        } else {
            min_z += direction_z;
            cur_z += step_z;
        }

        if step_count > 16 {
            break;
        }
        step_count += 1;

        let Some(clip_location) = Aabb::clip_with_from_and_to(
            Vec3::new(min_x as f64, min_y as f64, min_z as f64),
            Vec3::new((min_x + 1) as f64, (min_y + 1) as f64, (min_z + 1) as f64),
            from,
            to,
        ) else {
            continue;
        };

        let initial_max_x = clip_location
            .x
            .clamp(min_x as f64 + 1.0E-5, min_x as f64 + 1.0 - 1.0E-5);
        let initial_max_y = clip_location
            .y
            .clamp(min_y as f64 + 1.0E-5, min_y as f64 + 1.0 - 1.0E-5);
        let initial_max_z = clip_location
            .z
            .clamp(min_z as f64 + 1.0E-5, min_z as f64 + 1.0 - 1.0E-5);
        let max_x = (initial_max_x + aabb.get_size(Axis::X)).floor() as i32;
        let max_y = (initial_max_y + aabb.get_size(Axis::Y)).floor() as i32;
        let max_z = (initial_max_z + aabb.get_size(Axis::Z)).floor() as i32;

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    collisions.insert(BlockPos::new(x, y, z));
                }
            }
        }
    }
}
