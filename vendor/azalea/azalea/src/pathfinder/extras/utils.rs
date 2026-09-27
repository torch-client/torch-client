use azalea_core::position::{BlockPos, Vec3};
use azalea_entity::direction_looking_at;
use azalea_world::ChunkStorage;

pub fn get_reachable_blocks_around_player(
    player_position: BlockPos,
    chunk_storage: &ChunkStorage,
) -> Vec<BlockPos> {
    let mut blocks = Vec::new();

    for x in -6..=6 {
        for y in -5..=7 {
            for z in -6..=6 {
                let block_pos = player_position + BlockPos::new(x, y, z);
                let block_state = chunk_storage
                    .get_block_state(&block_pos)
                    .unwrap_or_default();

                if block_state.is_air() {
                    continue;
                }

                if can_reach_block(chunk_storage, player_position, block_pos) {
                    blocks.push(block_pos);
                }
            }
        }
    }

    blocks
}

pub fn pick_closest_block(position: BlockPos, blocks: &[BlockPos]) -> Option<BlockPos> {
    let mut closest_block_pos = None;
    let mut closest_distance = i32::MAX;
    for block_pos in &blocks[1..] {
        if block_pos.y < position.y {
            continue;
        }
        let distance = block_pos.distance_squared_to(&position);
        if distance < closest_distance {
            closest_block_pos = Some(*block_pos);
            closest_distance = distance;
        }
    }

    if closest_block_pos.is_none() {
        for block_pos in blocks {
            let distance = block_pos.distance_squared_to(&position);
            if distance < closest_distance {
                closest_block_pos = Some(*block_pos);
                closest_distance = distance;
            }
        }
    }

    closest_block_pos
}

pub fn get_hit_result_while_looking_at(
    chunk_storage: &ChunkStorage,
    player_position: BlockPos,
    look_target: BlockPos,
) -> BlockPos {
    let eye_position = Vec3 {
        x: player_position.x as f64 + 0.5,
        y: player_position.y as f64 + 1.53,
        z: player_position.z as f64 + 0.5,
    };
    get_hit_result_while_looking_at_with_eye_position(chunk_storage, eye_position, look_target)
}

pub fn can_reach_block(
    chunk_storage: &ChunkStorage,
    player_position: BlockPos,
    look_target: BlockPos,
) -> bool {
    let hit_result = get_hit_result_while_looking_at(chunk_storage, player_position, look_target);
    hit_result == look_target
}

pub fn get_hit_result_while_looking_at_with_eye_position(
    chunk_storage: &azalea_world::ChunkStorage,
    eye_position: Vec3,
    look_target: BlockPos,
) -> BlockPos {
    let look_direction = direction_looking_at(&eye_position, &look_target.center());
    let block_hit_result =
        azalea_client::interact::pick(&look_direction, &eye_position, chunk_storage, 4.5);
    block_hit_result.block_pos
}

#[cfg(test)]
mod tests {
    use azalea_core::position::ChunkPos;
    use azalea_world::{Chunk, PartialWorld};

    use super::*;

    #[test]
    fn test_cannot_reach_block_through_wall_when_y_is_negative() {
        let mut partial_world = PartialWorld::default();
        let mut world = ChunkStorage::default();
        partial_world
            .chunks
            .set(&ChunkPos { x: 0, z: 0 }, Some(Chunk::default()), &mut world);

        let set_solid_block_at = |x, y, z| {
            partial_world.chunks.set_block_state(
                &BlockPos::new(x, y, z),
                BlockKind::Stone.into(),
                &world,
            );
        };

        let y_offset = -8;

        set_solid_block_at(1, y_offset, 0);
        set_solid_block_at(1, y_offset + 1, 0);
        set_solid_block_at(0, y_offset, 1);
        set_solid_block_at(0, y_offset + 1, 1);
        set_solid_block_at(1, y_offset, 1);

        let player_position = BlockPos::new(0, y_offset, 0);
        let look_target = BlockPos::new(1, y_offset, 1);

        assert!(!can_reach_block(&world, player_position, look_target));
    }
}
