mod components;
pub mod effect_events;
pub mod indexing;

use std::collections::HashSet;

use azalea_block::{BlockState, BlockTrait, fluid_state::FluidKind, properties};
use azalea_core::{
    aabb::Aabb,
    entity_id::MinecraftEntityId,
    game_type::GameMode,
    position::{BlockPos, ChunkPos, Vec3},
    tick::GameTick,
};
use azalea_registry::{
    builtin::{BlockKind, EntityKind},
    tags,
};
use azalea_world::{ChunkStorage, WorldName, Worlds};
use bevy_app::{App, Plugin, PostUpdate, Update};
use bevy_ecs::prelude::*;
pub use components::*;
use derive_more::{Deref, DerefMut};
use indexing::EntityUuidIndex;
use tracing::debug;

use crate::{
    FluidOnEyes, LookDirection, Physics, Pose, Position, SnifferStateKind,
    dimensions::{
        EntityDimensions, RemoteScale, ShulkerPeekAmount, baby_dimensions, calculate_dimensions,
        shulker_bounding_box,
    },
    metadata::{self, Health, Player},
    plugin::effect_events::{handle_add_effect, handle_remove_effects},
};

#[derive(Clone, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum EntityUpdateSystems {
    Index,
    Deindex,
}

pub struct EntityPlugin;
impl Plugin for EntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            indexing::remove_despawned_entities_from_indexes.in_set(EntityUpdateSystems::Deindex),
        )
        .add_systems(
            Update,
            (
                (
                    indexing::update_entity_chunk_positions,
                    indexing::insert_entity_chunk_position,
                )
                    .chain()
                    .in_set(EntityUpdateSystems::Index),
                (
                    debug_new_entity,
                    add_dead,
                    clamp_look_direction,
                    update_on_climbable,
                    (update_dimensions, update_bounding_box).chain(),
                    update_crouching,
                ),
            ),
        )
        .add_systems(GameTick,
            (update_in_loaded_chunk, update_fluid_on_eyes, tick_shulker_peek),
        )
        .add_observer(handle_add_effect)
        .add_observer(handle_remove_effects)
        .init_resource::<EntityUuidIndex>();
    }
}

fn debug_new_entity(query: Query<(Entity, Option<&LocalEntity>), Added<MinecraftEntityId>>) {
    for (entity, local) in query.iter() {
        if local.is_some() {
            debug!("new local entity: {:?}", entity);
        } else {
            debug!("new entity: {:?}", entity);
        }
    }
}

pub fn add_dead(mut commands: Commands, query: Query<(Entity, &Health), Changed<Health>>) {
    for (entity, health) in query.iter() {
        if **health <= 0.0 {
            commands.entity(entity).insert(Dead);
        }
    }
}

pub fn update_fluid_on_eyes(
    mut query: Query<
        (&mut FluidOnEyes, &Position, &EntityDimensions, &WorldName),
        With<metadata::AbstractLiving>,
    >,
    worlds: Res<Worlds>,
) {
    query
        .par_iter_mut()
        .for_each(|(mut fluid_on_eyes, position, dimensions, world_name)| {
            let Some(world) = worlds.get(world_name) else {
                return;
            };

            let adjusted_eye_y = position.y + (dimensions.eye_height as f64) - 0.1111111119389534;
            let eye_block_pos = BlockPos::from(position.with_y(adjusted_eye_y));
            let fluid_at_eye = world
                .read()
                .get_fluid_state(eye_block_pos)
                .unwrap_or_default();
            let fluid_cutoff_y = (eye_block_pos.y as f32 + fluid_at_eye.height()) as f64;
            if fluid_cutoff_y > adjusted_eye_y {
                **fluid_on_eyes = fluid_at_eye.kind;
            } else {
                **fluid_on_eyes = FluidKind::Empty;
            }
        });
}

pub fn update_on_climbable(
    mut query: Query<
        (
            &mut OnClimbable,
            &Position,
            &WorldName,
            &GameMode,
            Option<&crate::metadata::FallFlying>,
        ),
        With<LocalEntity>,
    >,
    worlds: Res<Worlds>,
) {
    for (mut on_climbable, position, world_name, &game_mode, fall_flying) in query.iter_mut() {
        if game_mode == GameMode::Spectator {
            if **on_climbable {
                **on_climbable = false;
            }
            continue;
        }

        let Some(world) = worlds.get(world_name) else {
            continue;
        };

        let world = world.read();
        **on_climbable = climbable_at(&world, BlockPos::from(position), fall_flying.is_some_and(|f| **f));
    }
}

pub fn climbable_at(world: &azalea_world::World, block_pos: BlockPos, fall_flying: bool) -> bool {
    let block_state_at_feet = world.get_block_state(block_pos).unwrap_or_default();
    let block_at_feet = Box::<dyn BlockTrait>::from(block_state_at_feet);
    let registry_block_at_feet = block_at_feet.as_block_kind();

    if fall_flying && tags::blocks::CAN_GLIDE_THROUGH.contains(&registry_block_at_feet) {
        return false;
    }

    tags::blocks::CLIMBABLE.contains(&registry_block_at_feet)
        || (tags::blocks::TRAPDOORS.contains(&registry_block_at_feet)
            && is_trapdoor_usable_as_ladder(block_state_at_feet, block_pos, world))
}

fn is_trapdoor_usable_as_ladder(
    block_state: BlockState,
    block_pos: BlockPos,
    world: &azalea_world::World,
) -> bool {
    if !block_state
        .property::<properties::Open>()
        .unwrap_or_default()
    {
        return false;
    }

    let block_below = world.get_block_state(block_pos.down(1)).unwrap_or_default();
    let registry_block_below = block_below.as_block_kind();
    if registry_block_below != BlockKind::Ladder {
        return false;
    }
    let ladder_facing = block_below
        .property::<properties::FacingCardinal>()
        .expect("ladder block must have facing property");
    let trapdoor_facing = block_state
        .property::<properties::FacingCardinal>()
        .expect("trapdoor block must have facing property");
    if ladder_facing != trapdoor_facing {
        return false;
    }

    true
}

#[derive(Clone, Component, Deref, DerefMut)]
pub struct LoadedBy(pub HashSet<Entity>);

pub fn clamp_look_direction(mut query: Query<&mut LookDirection>) {
    for mut look_direction in &mut query {
        *look_direction = look_direction.clamped();
    }
}

#[allow(clippy::type_complexity)]
pub fn update_bounding_box(
    mut query: Query<
        (
            &mut Physics,
            &Position,
            &EntityDimensions,
            &EntityKindComponent,
            Option<&metadata::AttachFace>,
            Option<&ShulkerPeekAmount>,
            Option<&RemoteScale>,
        ),
        Or<(
            Changed<Position>,
            Changed<EntityDimensions>,
            Changed<metadata::AttachFace>,
            Changed<ShulkerPeekAmount>,
        )>,
    >,
) {
    for (mut physics, position, dimensions, kind, attach_face, peek, remote_scale) in
        query.iter_mut()
    {
        physics.bounding_box = if let Some(face) = attach_face {
            shulker_bounding_box(
                remote_scale.map_or(1.0, |s| s.0),
                face.0,
                peek.map_or(0.0, |p| p.0),
                **position,
            )
        } else if matches!(**kind, EntityKind::WindCharge | EntityKind::BreezeWindCharge) {
            let half = (dimensions.width / 2.0) as f64;
            let y = position.y - 0.15f32 as f64;
            Aabb {
                min: Vec3::new(position.x - half, y, position.z - half),
                max: Vec3::new(position.x + half, y + dimensions.height as f64, position.z + half),
            }
        } else {
            dimensions.make_bounding_box(**position)
        };
    }
}

pub fn tick_shulker_peek(
    mut commands: Commands,
    mut query: Query<(Entity, &metadata::Peek, Option<&mut ShulkerPeekAmount>)>,
) {
    for (entity, peek, current) in &mut query {
        let target = peek.0 as f32 * 0.01;
        let now = current.as_ref().map_or(0.0, |c| c.0);
        if now == target {
            continue;
        }
        let clamp = |v: f32, lo: f32, hi: f32| if v < lo { lo } else if v > hi { hi } else { v };
        let next = if now > target {
            clamp(now - 0.05, target, 1.0)
        } else {
            clamp(now + 0.05, 0.0, target)
        };
        match current {
            Some(mut c) => c.0 = next,
            None => {
                commands.entity(entity).insert(ShulkerPeekAmount(next));
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn update_dimensions(
    mut query: Query<
        (
            &mut EntityDimensions,
            &EntityKindComponent,
            &Pose,
            Option<&crate::Attributes>,
            Option<&RemoteScale>,
            (
                Option<&metadata::AbstractAgeableBaby>,
                Option<&metadata::ZombieBaby>,
                Option<&metadata::ZoglinBaby>,
                Option<&metadata::PiglinBaby>,
            ),
            (
                Option<&metadata::SlimeSize>,
                Option<&metadata::MagmaCubeSize>,
                Option<&metadata::SulfurCubeSize>,
                Option<&metadata::PhantomSize>,
            ),
            (
                Option<&metadata::Small>,
                Option<&metadata::ArmorStandMarker>,
                Option<&metadata::SnifferState>,
                Option<&metadata::PuffState>,
            ),
            (
                Option<&metadata::AttachFace>,
                Option<&ShulkerPeekAmount>,
                Option<&metadata::SalmonKind>,
            ),
            (
                Option<&metadata::InteractionWidth>,
                Option<&metadata::InteractionHeight>,
                Option<&metadata::Radius>,
            ),
        ),
        Or<(
            Or<(
                Changed<EntityKindComponent>,
                Changed<Pose>,
                Changed<crate::Attributes>,
                Changed<RemoteScale>,
                Changed<metadata::AbstractAgeableBaby>,
                Changed<metadata::ZombieBaby>,
                Changed<metadata::ZoglinBaby>,
                Changed<metadata::PiglinBaby>,
                Changed<metadata::InteractionWidth>,
                Changed<metadata::InteractionHeight>,
                Changed<metadata::Radius>,
            )>,
            Or<(
                Changed<metadata::SlimeSize>,
                Changed<metadata::MagmaCubeSize>,
                Changed<metadata::SulfurCubeSize>,
                Changed<metadata::PhantomSize>,
                Changed<metadata::Small>,
                Changed<metadata::ArmorStandMarker>,
                Changed<metadata::SnifferState>,
                Changed<metadata::PuffState>,
                Changed<metadata::AttachFace>,
                Changed<ShulkerPeekAmount>,
                Changed<metadata::SalmonKind>,
            )>,
        )>,
    >,
) {
    for (
        mut dimensions,
        kind,
        pose,
        attributes,
        remote_scale,
        babies,
        sizes,
        specials,
        shulker,
        server_sized,
    ) in query.iter_mut()
    {
        let kind = **kind;
        let scale = remote_scale
            .map(|s| s.0)
            .or_else(|| attributes.map(|a| a.scale.calculate() as f32))
            .unwrap_or(1.0);
        let baby = babies.0.is_some_and(|b| b.0)
            || babies.1.is_some_and(|b| b.0)
            || babies.2.is_some_and(|b| b.0)
            || babies.3.is_some_and(|b| b.0);

        let default = if *pose == Pose::Sleeping {
            calculate_dimensions(kind, *pose)
        } else if let Some(size) = sizes.0.map(|s| s.0).or(sizes.1.map(|s| s.0)).or(sizes.2.map(|s| s.0)) {
            calculate_dimensions(kind, *pose).scale(size.clamp(1, 127) as f32)
        } else if kind == EntityKind::Interaction {
            EntityDimensions::new(
                server_sized.0.map_or(1.0, |w| w.0),
                server_sized.1.map_or(1.0, |h| h.0),
            )
        } else if kind == EntityKind::AreaEffectCloud {
            EntityDimensions::new(server_sized.2.map_or(3.0, |r| r.0) * 2.0, 0.5)
        } else if kind == EntityKind::ArmorStand {
            if specials.1.is_some_and(|m| m.0) {
                EntityDimensions::new(0.0, 0.0).fixed()
            } else if specials.0.is_some_and(|s| s.0) {
                EntityDimensions::from(kind).scale(0.5).eye_height(0.9875)
            } else {
                EntityDimensions::from(kind)
            }
        } else if kind == EntityKind::Sniffer
            && specials.2.is_some_and(|s| s.0 == SnifferStateKind::Digging)
        {
            let adult = EntityDimensions::from(kind);
            EntityDimensions::new(adult.width, adult.height - 0.4)
                .eye_height(0.81)
                .scale(if baby { 0.5 } else { 1.0 })
        } else if matches!(kind, EntityKind::Camel | EntityKind::CamelHusk)
            && (baby || *pose == Pose::Sitting)
        {
            let adult = EntityDimensions::from(kind);
            match (*pose == Pose::Sitting, baby) {
                (true, true) => EntityDimensions::new(0.95, 0.425).eye_height(0.41),
                (true, false) => {
                    EntityDimensions::new(adult.width, adult.height - 1.43).eye_height(0.845)
                }
                (false, _) => EntityDimensions::new(0.95, 1.4).eye_height(1.38),
            }
        } else if baby {
            let dims = baby_dimensions(kind);
            if kind == EntityKind::Goat && *pose == Pose::LongJumping {
                dims.scale(0.7)
            } else {
                dims
            }
        } else if let Some(size) = sizes.3 {
            calculate_dimensions(kind, *pose).scale(1.0 + 0.15 * size.0 as f32)
        } else if kind == EntityKind::Goat && *pose == Pose::LongJumping {
            calculate_dimensions(kind, *pose).scale(0.7)
        } else if kind == EntityKind::Warden && matches!(*pose, Pose::Digging | Pose::Emerging) {
            EntityDimensions::new(calculate_dimensions(kind, *pose).width, 1.0).fixed()
        } else if kind == EntityKind::Salmon {
            let factor = match shulker.2.map_or(1, |k| k.0) {
                i32::MIN..=0 => 0.5,
                1 => 1.0,
                _ => 1.5,
            };
            calculate_dimensions(kind, *pose).scale(factor)
        } else if kind == EntityKind::Pufferfish {
            let factor = match specials.3.map_or(0, |p| p.0) {
                0 => 0.5,
                1 => 0.7,
                _ => 1.0,
            };
            calculate_dimensions(kind, *pose).scale(factor)
        } else if kind == EntityKind::Shulker
            && shulker.0.is_none_or(|f| f.0 == azalea_core::direction::Direction::Down)
            && shulker.1.is_some_and(|p| p.0 > 0.0)
        {
            let peek = shulker.1.map_or(0.0, |p| p.0);
            calculate_dimensions(kind, *pose).scale_xy(1.0, 1.0 + peek)
        } else {
            calculate_dimensions(kind, *pose)
        };
        *dimensions = default.scale(scale);
    }
}

#[allow(clippy::type_complexity)]
pub fn update_crouching(
    query: Query<(&mut Crouching, &Pose), (Without<LocalEntity>, With<Player>)>,
) {
    for (mut crouching, pose) in query {
        let new_crouching = *pose == Pose::Crouching;
        if **crouching != new_crouching {
            **crouching = new_crouching;
        }
    }
}

#[derive(Clone, Component, Copy, Debug)]
pub struct InLoadedChunk;

pub fn update_in_loaded_chunk(
    mut commands: bevy_ecs::system::Commands,
    query: Query<(Entity, &WorldName, &Position, Option<&InLoadedChunk>)>,
    worlds: Res<Worlds>,
) {
    for (entity, world_name, position, last_in_loaded_chunk) in &query {
        let player_chunk_pos = ChunkPos::from(position);
        let Some(world_lock) = worlds.get(world_name) else {
            commands.entity(entity).remove::<InLoadedChunk>();
            continue;
        };

        let in_loaded_chunk = world_lock.read().chunks.get(&player_chunk_pos).is_some();
        if in_loaded_chunk {
            if last_in_loaded_chunk.is_none() {
                commands.entity(entity).insert(InLoadedChunk);
            }
        } else {
            if last_in_loaded_chunk.is_some() {
                commands.entity(entity).remove::<InLoadedChunk>();
            }
        }
    }
}

pub fn on_pos_legacy(chunk_storage: &ChunkStorage, position: Position) -> BlockPos {
    on_pos(0.2, chunk_storage, position)
}

pub fn on_pos(offset: f32, chunk_storage: &ChunkStorage, pos: Position) -> BlockPos {
    let x = pos.x.floor() as i32;
    let y = (pos.y - offset as f64).floor() as i32;
    let z = pos.z.floor() as i32;
    let pos = BlockPos { x, y, z };

    let block_pos = pos.down(1);
    let block_state = chunk_storage.get_block_state(block_pos);
    if block_state == Some(BlockState::AIR) {
        let block_pos_below = block_pos.down(1);
        let block_state_below = chunk_storage.get_block_state(block_pos_below);
        if let Some(_block_state_below) = block_state_below {
        }
    }

    pos
}

#[cfg(test)]
mod tests {
    use azalea_block::{
        blocks::{Ladder, OakTrapdoor},
        properties::{FacingCardinal, TopBottom},
    };
    use azalea_core::position::{BlockPos, ChunkPos};
    use azalea_registry::builtin::BlockKind;
    use azalea_world::{Chunk, ChunkStorage, PartialWorld, World};

    use super::is_trapdoor_usable_as_ladder;

    #[test]
    fn test_is_trapdoor_useable_as_ladder() {
        let mut partial_world = PartialWorld::default();
        let mut chunks = ChunkStorage::default();
        partial_world.chunks.set(
            &ChunkPos { x: 0, z: 0 },
            Some(Chunk::default()),
            &mut chunks,
        );
        partial_world.chunks.set_block_state(
            BlockPos::new(0, 0, 0),
            BlockKind::Stone.into(),
            &chunks,
        );

        let ladder = Ladder {
            facing: FacingCardinal::East,
            waterlogged: false,
        };
        partial_world
            .chunks
            .set_block_state(BlockPos::new(0, 0, 0), ladder.into(), &chunks);

        let trapdoor = OakTrapdoor {
            facing: FacingCardinal::East,
            half: TopBottom::Bottom,
            open: true,
            powered: false,
            waterlogged: false,
        };
        partial_world
            .chunks
            .set_block_state(BlockPos::new(0, 1, 0), trapdoor.into(), &chunks);

        let world = World::from(chunks);
        let trapdoor_matches_ladder = is_trapdoor_usable_as_ladder(
            world
                .get_block_state(BlockPos::new(0, 1, 0))
                .unwrap_or_default(),
            BlockPos::new(0, 1, 0),
            &world,
        );

        assert!(trapdoor_matches_ladder);
    }
}
