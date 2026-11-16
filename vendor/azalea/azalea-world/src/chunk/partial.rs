use std::{
    fmt::{self, Debug},
    sync::Arc,
};

use azalea_block::BlockState;
use azalea_core::{
    heightmap_kind::HeightmapKind,
    position::{BlockPos, ChunkBlockPos, ChunkPos},
};
use nohash_hasher::IntMap;
use parking_lot::RwLock;
use tracing::{debug, trace};

use crate::{
    Chunk,
    chunk::{ChunkDecodeFailure, storage::ChunkStorage},
};

pub struct PartialChunkStorage {
    view_center: ChunkPos,
    pub(crate) chunk_radius: u32,
    chunks: IntMap<ChunkPos, Arc<RwLock<Chunk>>>,
    generation: u64,
}

static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl PartialChunkStorage {
    pub fn new(chunk_radius: u32) -> Self {
        PartialChunkStorage {
            view_center: ChunkPos::new(0, 0),
            chunk_radius,
            chunks: IntMap::default(),
            generation: NEXT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn update_view_center(&mut self, view_center: ChunkPos) {
        self.view_center = view_center;
    }

    pub fn update_view_radius(&mut self, chunk_radius: u32) {
        self.chunk_radius = chunk_radius;
    }

    pub fn view_center(&self) -> ChunkPos {
        self.view_center
    }

    pub fn view_range(&self) -> u32 {
        self.chunk_radius * 2 + 1
    }

    pub fn chunk_radius(&self) -> u32 {
        self.chunk_radius
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
        data: &[u8],
        heightmaps: &[(HeightmapKind, Box<[u64]>)],
        chunk_storage: &mut ChunkStorage,
        biome_registry_len: usize,
    ) -> Option<ChunkDecodeFailure> {
        debug!("Replacing chunk at {:?}", pos);
        let (chunk, failure) = Chunk::read_lenient(
            data,
            chunk_storage.height(),
            chunk_storage.min_y(),
            heightmaps,
            biome_registry_len,
        );
        self.set(pos, Some(chunk), chunk_storage);
        trace!("Loaded chunk {pos:?}");
        failure
    }

    pub fn limited_get(&self, pos: &ChunkPos) -> Option<&Arc<RwLock<Chunk>>> {
        self.chunks.get(pos)
    }

    pub fn set(&mut self, pos: &ChunkPos, chunk: Option<Chunk>, chunk_storage: &mut ChunkStorage) {
        let new_chunk = chunk.map(|c| chunk_storage.upsert(*pos, c));
        self.limited_set(pos, new_chunk);
    }

    pub fn limited_set(&mut self, pos: &ChunkPos, chunk: Option<Arc<RwLock<Chunk>>>) {
        match chunk {
            Some(chunk) => {
                self.chunks.insert(*pos, chunk);
            }
            None => {
                self.chunks.remove(pos);
            }
        }
    }

    pub fn forget_held(&mut self, pos: &ChunkPos) {
        self.chunks.remove(pos);
    }

    pub fn retain(&mut self, mut keep: impl FnMut(ChunkPos) -> bool) -> Vec<ChunkPos> {
        let mut dropped = Vec::new();
        self.chunks.retain(|pos, _| {
            let kept = keep(*pos);
            if !kept {
                dropped.push(*pos);
            }
            kept
        });
        dropped
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn loaded(&self) -> impl Iterator<Item = (ChunkPos, &Arc<RwLock<Chunk>>)> {
        self.chunks.iter().map(|(pos, chunk)| (*pos, chunk))
    }
}

impl Debug for PartialChunkStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PartialChunkStorage")
            .field("view_center", &self.view_center)
            .field("chunk_radius", &self.chunk_radius)
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

    fn chunk() -> Option<Arc<RwLock<Chunk>>> {
        Some(Arc::new(RwLock::new(Chunk::default())))
    }

    #[test]
    fn a_column_outside_the_window_is_kept() {
        let mut storage = PartialChunkStorage::new(5);
        storage.update_view_center(ChunkPos::new(0, 0));
        let far = ChunkPos::new(500, -300);
        storage.limited_set(&far, chunk());
        assert!(!storage.in_range(&far));
        assert!(storage.limited_get(&far).is_some());
    }

    #[test]
    fn moving_or_narrowing_the_window_drops_nothing() {
        let mut storage = PartialChunkStorage::new(8);
        for x in -8..=8 {
            for z in -8..=8 {
                storage.limited_set(&ChunkPos::new(x, z), chunk());
            }
        }
        storage.update_view_center(ChunkPos::new(100, 100));
        storage.update_view_radius(3);
        assert_eq!(storage.len(), 17 * 17);
    }

    #[test]
    fn retain_reports_what_it_dropped() {
        let mut storage = PartialChunkStorage::new(8);
        for x in -2..=2 {
            storage.limited_set(&ChunkPos::new(x, 0), chunk());
        }
        let mut dropped = storage.retain(|pos| pos.x >= 0);
        dropped.sort_by_key(|p| p.x);
        assert_eq!(dropped, vec![ChunkPos::new(-2, 0), ChunkPos::new(-1, 0)]);
        assert_eq!(storage.len(), 3);
    }
}
