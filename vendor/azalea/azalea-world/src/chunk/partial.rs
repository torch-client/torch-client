use std::{
    fmt::{self, Debug},
    io::Cursor,
    sync::Arc,
};

use azalea_block::BlockState;
use azalea_buf::BufReadError;
use azalea_core::{
    heightmap_kind::HeightmapKind,
    position::{BlockPos, ChunkBlockPos, ChunkPos},
};
use parking_lot::RwLock;
use tracing::{debug, trace, warn};

use crate::{Chunk, chunk::storage::ChunkStorage};

pub struct PartialChunkStorage {
    view_center: ChunkPos,
    pub(crate) chunk_radius: u32,
    view_range: u32,
    chunks: Box<[Option<LoadedChunk>]>,
}

#[derive(Clone)]
struct LoadedChunk {
    pos: ChunkPos,
    chunk: Arc<RwLock<Chunk>>,
}

impl PartialChunkStorage {
    pub fn new(chunk_radius: u32) -> Self {
        let view_range = chunk_radius * 2 + 1;
        PartialChunkStorage {
            view_center: ChunkPos::new(0, 0),
            chunk_radius,
            view_range,
            chunks: vec![None; (view_range * view_range) as usize].into(),
        }
    }

    pub fn update_view_center(&mut self, view_center: ChunkPos) {
        self.view_center = view_center;
    }

    pub fn update_view_radius(&mut self, chunk_radius: u32) {
        if chunk_radius == self.chunk_radius {
            return;
        }
        let mut resized = Self::new(chunk_radius);
        resized.view_center = self.view_center;
        for slot in self.chunks.iter().flatten() {
            if resized.in_range(&slot.pos) {
                let index = resized.index_from_chunk_pos(&slot.pos);
                resized.chunks[index] = Some(slot.clone());
            }
        }
        *self = resized;
    }

    pub fn view_center(&self) -> ChunkPos {
        self.view_center
    }

    pub fn view_range(&self) -> u32 {
        self.view_range
    }

    pub fn chunk_radius(&self) -> u32 {
        self.chunk_radius
    }

    pub fn index_from_chunk_pos(&self, chunk_pos: &ChunkPos) -> usize {
        let view_range = self.view_range as i32;

        let x = i32::rem_euclid(chunk_pos.x, view_range) * view_range;
        let z = i32::rem_euclid(chunk_pos.z, view_range);
        (x + z) as usize
    }

    pub fn chunk_pos_from_index(&self, index: usize) -> ChunkPos {
        let view_range = self.view_range as i32;
        let offset_x = index as i32 / view_range;
        let offset_z = index as i32 % view_range;

        fn in_window(center: i32, residue: i32, view_range: i32) -> i32 {
            let start = center - (view_range - 1) / 2;
            start + (residue - start).rem_euclid(view_range)
        }

        ChunkPos::new(
            in_window(self.view_center.x, offset_x, view_range),
            in_window(self.view_center.z, offset_z, view_range),
        )
    }

    pub fn in_range(&self, chunk_pos: &ChunkPos) -> bool {
        in_range_for_view_center_and_radius(chunk_pos, self.view_center, self.chunk_radius)
    }

    pub fn set_block_state(
        &self,
        pos: BlockPos,
        state: BlockState,
        chunk_storage: &ChunkStorage,
    ) -> Option<BlockState> {
        if pos.y < chunk_storage.min_y()
            || pos.y >= (chunk_storage.min_y() + chunk_storage.height() as i32)
        {
            return None;
        }
        let chunk_pos = ChunkPos::from(pos);
        let chunk_lock = chunk_storage.get(&chunk_pos)?;
        let old = {
            let mut chunk = chunk_lock.write();
            chunk.get_and_set_block_state(&ChunkBlockPos::from(pos), state, chunk_storage.min_y())
        };
        crate::block_change::notify(pos, old, state);
        Some(old)
    }

    pub fn replace_with_packet_data(
        &mut self,
        pos: &ChunkPos,
        data: &mut Cursor<&[u8]>,
        heightmaps: &[(HeightmapKind, Box<[u64]>)],
        chunk_storage: &mut ChunkStorage,
    ) -> Result<(), BufReadError> {
        debug!("Replacing chunk at {:?}", pos);
        if !self.in_range(pos) {
            warn!("Ignoring chunk since it's not in the view range: {pos:?}");
            return Ok(());
        }

        let chunk = Chunk::read_with_dimension_height(
            data,
            chunk_storage.height(),
            chunk_storage.min_y(),
            heightmaps,
        )?;

        self.set(pos, Some(chunk), chunk_storage);
        trace!("Loaded chunk {pos:?}");

        Ok(())
    }

    pub fn limited_get(&self, pos: &ChunkPos) -> Option<&Arc<RwLock<Chunk>>> {
        if !self.in_range(pos) {
            warn!(
                "Chunk at {:?} is not in the render distance (center: {:?}, {} chunks)",
                pos, self.view_center, self.chunk_radius,
            );
            return None;
        }

        let slot = self.chunks[self.index_from_chunk_pos(pos)].as_ref()?;
        (slot.pos == *pos).then_some(&slot.chunk)
    }

    pub fn set(&mut self, pos: &ChunkPos, chunk: Option<Chunk>, chunk_storage: &mut ChunkStorage) {
        let new_chunk = chunk.map(|c| chunk_storage.upsert(*pos, c));
        self.limited_set(pos, new_chunk);
    }

    pub fn limited_set(&mut self, pos: &ChunkPos, chunk: Option<Arc<RwLock<Chunk>>>) {
        if !self.in_range(pos) {
            return;
        }
        let index = self.index_from_chunk_pos(pos);
        match chunk {
            Some(chunk) => self.chunks[index] = Some(LoadedChunk { pos: *pos, chunk }),
            None => {
                if self.chunks[index].as_ref().is_some_and(|s| s.pos == *pos) {
                    self.chunks[index] = None;
                }
            }
        }
    }

    pub fn loaded(&self) -> impl Iterator<Item = (ChunkPos, &Arc<RwLock<Chunk>>)> {
        self.chunks
            .iter()
            .flatten()
            .map(|slot| (slot.pos, &slot.chunk))
    }
}

impl Debug for PartialChunkStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartialChunkStorage")
            .field("view_center", &self.view_center)
            .field("chunk_radius", &self.chunk_radius)
            .field("view_range", &self.view_range)
            .field("chunks", &format_args!("{} items", self.chunks.len()))
            .finish()
    }
}

impl Default for PartialChunkStorage {
    fn default() -> Self {
        Self::new(8)
    }
}

pub fn in_range_for_view_center_and_radius(
    chunk_pos: &ChunkPos,
    view_center: ChunkPos,
    chunk_radius: u32,
) -> bool {
    (chunk_pos.x - view_center.x).unsigned_abs() <= chunk_radius
        && (chunk_pos.z - view_center.z).unsigned_abs() <= chunk_radius
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use azalea_core::position::ChunkPos;
    use parking_lot::RwLock;

    use crate::{Chunk, chunk::partial::PartialChunkStorage};

    #[test]
    fn index_round_trips_over_the_whole_window() {
        for center in [
            ChunkPos::new(0, 0),
            ChunkPos::new(-1, 7),
            ChunkPos::new(23, -46),
            ChunkPos::new(-8, -8),
        ] {
            let mut storage = PartialChunkStorage::new(5);
            storage.update_view_center(center);
            let radius = 5i32;
            for x in center.x - radius..=center.x + radius {
                for z in center.z - radius..=center.z + radius {
                    let pos = ChunkPos::new(x, z);
                    let index = storage.index_from_chunk_pos(&pos);
                    assert_eq!(
                        storage.chunk_pos_from_index(index),
                        pos,
                        "centre {center:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_slot_is_only_read_for_the_column_it_was_filled_for() {
        let mut storage = PartialChunkStorage::new(5);
        let view_range = storage.view_range() as i32;
        storage.update_view_center(ChunkPos::new(0, 0));

        let stored = ChunkPos::new(2, 0);
        let chunk = Arc::new(RwLock::new(Chunk::default()));
        storage.limited_set(&stored, Some(chunk));
        assert!(storage.limited_get(&stored).is_some());

        let aliased = ChunkPos::new(2 + view_range, 0);
        assert_eq!(
            storage.index_from_chunk_pos(&aliased),
            storage.index_from_chunk_pos(&stored)
        );
        storage.update_view_center(aliased);
        assert!(storage.limited_get(&aliased).is_none());
    }

    #[test]
    fn forgetting_a_column_leaves_its_slots_neighbour_alone() {
        let mut storage = PartialChunkStorage::new(5);
        let view_range = storage.view_range() as i32;
        let here = ChunkPos::new(0, 0);
        let away = ChunkPos::new(view_range, 0);

        storage.update_view_center(here);
        storage.limited_set(&here, Some(Arc::new(RwLock::new(Chunk::default()))));
        storage.limited_set(&away, None);
        assert!(storage.limited_get(&here).is_some());

        storage.limited_set(&here, None);
        assert!(storage.limited_get(&here).is_none());
    }

    #[test]
    fn widening_the_window_keeps_every_chunk_where_it_belongs() {
        let mut storage = PartialChunkStorage::new(5);
        storage.update_view_center(ChunkPos::new(0, 0));
        for x in -5..=5 {
            for z in -5..=5 {
                storage.limited_set(
                    &ChunkPos::new(x, z),
                    Some(Arc::new(RwLock::new(Chunk::default()))),
                );
            }
        }

        storage.update_view_radius(11);
        assert_eq!(storage.chunk_radius(), 11);
        for x in -5..=5 {
            for z in -5..=5 {
                let pos = ChunkPos::new(x, z);
                assert!(storage.limited_get(&pos).is_some(), "lost {pos:?}");
            }
        }
    }

    #[test]
    fn narrowing_the_window_drops_only_what_no_longer_fits() {
        let mut storage = PartialChunkStorage::new(8);
        storage.update_view_center(ChunkPos::new(0, 0));
        for x in -8..=8 {
            for z in -8..=8 {
                storage.limited_set(
                    &ChunkPos::new(x, z),
                    Some(Arc::new(RwLock::new(Chunk::default()))),
                );
            }
        }

        storage.update_view_radius(3);
        for x in -3..=3 {
            for z in -3..=3 {
                let pos = ChunkPos::new(x, z);
                assert!(storage.limited_get(&pos).is_some(), "lost {pos:?}");
            }
        }
        assert_eq!(storage.loaded().count(), 7 * 7);
    }

    #[test]
    fn test_chunk_pos_from_index() {
        let mut partial_chunk_storage = PartialChunkStorage::new(5);
        partial_chunk_storage.update_view_center(ChunkPos::new(0, -1));
        assert_eq!(
            partial_chunk_storage.chunk_pos_from_index(
                partial_chunk_storage.index_from_chunk_pos(&ChunkPos::new(2, -1))
            ),
            ChunkPos::new(2, -1),
        );
    }
}
