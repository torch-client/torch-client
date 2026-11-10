use azalea::app::{App, Plugin};
use azalea::block::BlockState;
use azalea::block::fluid_state::FluidKind;
use azalea::block::properties::PotentSulfurState;
use azalea::ecs::prelude::*;
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::{
    Dead, EntityKindComponent, HasClientLoaded, LocalEntity, Physics, PlayerAbilities, Position,
};
use azalea::physics::collision::BlockWithShape;
use azalea::physics::{PhysicsSystems, travel::travel};
use azalea::world::{World, WorldName, Worlds};
use azalea_core::aabb::Aabb;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::game_type::GameMode;
use azalea_core::position::BlockPos;
use azalea_core::tick::GameTick;
use azalea_registry::builtin::BlockKind;
use azalea_registry::tags::entities::NOT_AFFECTED_BY_GEYSERS;

const LAUNCH: f64 = 0.20000000298023224;
const BASE_LAUNCH_SPEED: f64 = 0.30000001192092896;
const MAX_WATER: i32 = 4;
const MAX_REACH: i32 = 6 * MAX_WATER;

pub struct GeyserPlugin;

impl Plugin for GeyserPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(GameTick, launch.in_set(PhysicsSystems).after(travel));
    }
}

#[allow(clippy::type_complexity)]
fn launch(
    mut query: Query<
        (
            &mut Physics,
            &Position,
            &EntityDimensions,
            &PlayerAbilities,
            &WorldName,
            &EntityIdIndex,
            Option<&GameMode>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>, Without<Dead>),
    >,
    mut vehicles: Query<
        (
            &mut Physics,
            &Position,
            &EntityDimensions,
            &EntityKindComponent,
        ),
        Without<LocalEntity>,
    >,
    worlds: Res<Worlds>,
) {
    for (mut physics, position, dimensions, abilities, world_name, id_index, game_mode) in
        &mut query
    {
        if game_mode == Some(&GameMode::Spectator) {
            continue;
        }
        let Some(world_lock) = worlds.get(world_name) else {
            continue;
        };
        let world = world_lock.read();
        let riding = crate::client::tracking::local_vehicle_id().is_some();
        let bounds = dimensions.make_bounding_box(**position);
        lift(&world, &bounds, &mut physics, abilities.flying || riding);

        let Some(driven) = crate::play::riding::driven_pose() else {
            continue;
        };
        let Some(vehicle) = id_index.get_by_minecraft_entity(MinecraftEntityId(driven.id)) else {
            continue;
        };
        let Ok((mut v_physics, v_position, v_dimensions, kind)) = vehicles.get_mut(vehicle) else {
            continue;
        };
        let bounds = v_dimensions.make_bounding_box(**v_position);
        let exempt = NOT_AFFECTED_BY_GEYSERS.contains(&kind.0);
        lift(&world, &bounds, &mut v_physics, exempt);
    }
}

fn lift(world: &World, bounds: &Aabb, physics: &mut Physics, skip_push: bool) {
    for water in geysers_under(world, bounds) {
        if physics.velocity.y > -0.5 && physics.fall_distance > 1.0 {
            physics.fall_distance = 1.0;
        }
        if skip_push {
            continue;
        }
        if physics.velocity.y < BASE_LAUNCH_SPEED + water as f64 * 0.1 {
            physics.velocity.y += LAUNCH;
        }
    }
}

fn geysers_under(world: &World, bounds: &Aabb) -> Vec<i32> {
    let mut out = Vec::new();
    let x0 = bounds.min.x.floor() as i32;
    let x1 = bounds.max.x.floor() as i32;
    let z0 = bounds.min.z.floor() as i32;
    let z1 = bounds.max.z.floor() as i32;
    let y0 = bounds.min.y.floor() as i32 - MAX_REACH - 1;
    let y1 = bounds.max.y.floor() as i32;
    for x in x0..=x1 {
        for z in z0..=z1 {
            for y in y0..=y1 {
                let pos = BlockPos::new(x, y, z);
                let state = state_at(world, pos);
                if state.as_block_kind() != BlockKind::PotentSulfur
                    || !matches!(
                        state.property::<PotentSulfurState>(),
                        Some(PotentSulfurState::Erupting | PotentSulfurState::Continuous)
                    )
                {
                    continue;
                }
                let Some(source) = source_block(world, pos) else {
                    continue;
                };
                let water = source.y - pos.y - 1;
                let height = unobstructed(world, pos.up(1), water);
                if lift_box(pos, height).intersects_aabb(bounds) {
                    out.push(water);
                }
            }
        }
    }
    out
}

fn lift_box(pos: BlockPos, height: i32) -> Aabb {
    let (x, y, z) = (pos.x as f64, (pos.y + 1) as f64, pos.z as f64);
    let reach = (height - 1) as f64;
    let (min_y, max_y) = if reach < 0.0 {
        (y + reach, y + 1.0)
    } else {
        (y, y + 1.0 + reach)
    };
    Aabb {
        min: azalea_core::position::Vec3::new(x, min_y, z),
        max: azalea_core::position::Vec3::new(x + 1.0, max_y, z + 1.0),
    }
}

fn state_at(world: &World, pos: BlockPos) -> BlockState {
    world.get_block_state(pos).unwrap_or_default()
}

fn passable(state: BlockState) -> bool {
    state.is_air() || state.as_block_kind() == BlockKind::Water || state.is_collision_shape_empty()
}

pub(crate) fn source_block(world: &World, origin: BlockPos) -> Option<BlockPos> {
    let max_y = origin.y + MAX_WATER + 1;
    let mut pos = origin.up(1);
    while pos.y <= max_y {
        let state = state_at(world, pos);
        let water_source = world
            .get_fluid_state(pos)
            .is_some_and(|f| f.kind == FluidKind::Water && f.is_source());
        if water_source && passable(state) {
            pos = pos.up(1);
            continue;
        }
        return passable(state).then_some(pos);
    }
    None
}

fn unobstructed(world: &World, from: BlockPos, water: i32) -> i32 {
    let reach = 6 * water;
    (0..reach)
        .find(|&i| !passable(state_at(world, from.up(i))))
        .unwrap_or(reach)
}
