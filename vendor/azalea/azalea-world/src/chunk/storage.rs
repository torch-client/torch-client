use std::{
    any::Any,
    collections::hash_map::Entry,
    fmt::{self, Debug},
    ops::{Deref, DerefMut},
    sync::{Arc, Weak},
};

use azalea_block::{BlockState, fluid_state::FluidState};
use azalea_core::position::{BlockPos, ChunkBiomePos, ChunkBlockPos, ChunkPos};
use azalea_registry::data::Biome;
use nohash_hasher::IntMap;
use parking_lot::RwLock;

use crate::Chunk;

pub struct ChunkStorage(pub Box<dyn ChunkStorageTrait>);

pub trait ChunkStorageTrait: Send + Sync + Any {
    fn min_y(&self) -> i32;
    fn height(&self) -> u32;
    #[must_use]
    fn get(&self, pos: &ChunkPos) -> Option<Arc<RwLock<Chunk>>>;
    #[must_use]
    fn upsert(&mut self, pos: ChunkPos, chunk: Chunk) -> Arc<RwLock<Chunk>>;
    fn chunks(&self) -> Box<[&ChunkPos]>;
    fn clone_box(&self) -> Box<dyn ChunkStorageTrait>;
    fn compact_dead(&mut self) {}

    fn get_block_state(&self, pos: BlockPos) -> Option<BlockState> {
        let chunk = self.get(&ChunkPos::from(pos))?;
        let chunk = chunk.read();
        chunk.get_block_state(&ChunkBlockPos::from(pos), self.min_y())
    }
    fn set_block_state(&self, pos: BlockPos, state: BlockState) -> Option<BlockState> {
        if pos.y < self.min_y() || pos.y >= (self.min_y() + self.height() as i32) {
            return None;
        }
        let chunk = self.get(&ChunkPos::from(pos))?;
        let old = {
            let mut chunk = chunk.write();
            chunk.get_and_set_block_state(&ChunkBlockPos::from(pos), state, self.min_y())
        };
        crate::block_change::notify(pos, old, state);
        Some(old)
    }
    fn get_fluid_state(&self, pos: BlockPos) -> Option<FluidState> {
        let block_state = self.get_block_state(pos)?;
        Some(FluidState::from(block_state))
    }
    fn get_biome(&self, pos: BlockPos) -> Option<Biome> {
        let chunk = self.get(&ChunkPos::from(pos))?;
        let chunk = chunk.read();
        chunk.get_biome(ChunkBiomePos::from(pos), self.min_y())
    }
}
impl ChunkStorage {
    pub fn new(height: u32, min_y: i32) -> Self {
        Self(Box::new(WeakChunkStorage::new(height, min_y)))
    }

    pub fn new_with(inner: Box<dyn ChunkStorageTrait>) -> Self {
        Self(inner)
    }
}

impl Deref for ChunkStorage {
    type Target = dyn ChunkStorageTrait;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
impl DerefMut for ChunkStorage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}
impl Clone for ChunkStorage {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

#[derive(Clone, Debug)]
pub struct WeakChunkStorage {
    pub height: u32,
    pub min_y: i32,
    pub map: IntMap<ChunkPos, Weak<RwLock<Chunk>>>,
}

impl WeakChunkStorage {
    pub fn new(height: u32, min_y: i32) -> Self {
        WeakChunkStorage {
            height,
            min_y,
            map: IntMap::default(),
        }
    }
}
impl ChunkStorageTrait for WeakChunkStorage {
    fn min_y(&self) -> i32 {
        self.min_y
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn get(&self, pos: &ChunkPos) -> Option<Arc<RwLock<Chunk>>> {
        self.map.get(pos).and_then(|chunk| chunk.upgrade())
    }

    fn upsert(&mut self, pos: ChunkPos, chunk: Chunk) -> Arc<RwLock<Chunk>> {
        match self.map.entry(pos) {
            Entry::Occupied(mut e) => {
                if let Some(existing) = e.get_mut().upgrade() {
                    *existing.write() = chunk;
                    existing
                } else {
                    let arc = Arc::new(RwLock::new(chunk));
                    e.insert(Arc::downgrade(&arc));
                    arc
                }
            }
            Entry::Vacant(e) => {
                let arc = Arc::new(RwLock::new(chunk));
                e.insert(Arc::downgrade(&arc));
                arc
            }
        }
    }

    fn chunks(&self) -> Box<[&ChunkPos]> {
        self.map.keys().collect::<Vec<_>>().into_boxed_slice()
    }

    fn clone_box(&self) -> Box<dyn ChunkStorageTrait> {
        Box::new(self.clone())
    }

    fn compact_dead(&mut self) {
        self.map.retain(|_, chunk| chunk.strong_count() > 0);
    }
}

impl Default for WeakChunkStorage {
    fn default() -> Self {
        Self::new(384, -64)
    }
}
impl Default for ChunkStorage {
    fn default() -> Self {
        Self::new(384, -64)
    }
}

impl Debug for ChunkStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkStorage")
            .field("min_y", &self.0.min_y())
            .field("height", &self.0.height())
            .field("chunk_count", &self.0.chunks().len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use azalea_core::position::{BlockPos, ChunkPos};

    use crate::{
        Chunk,
        chunk::{partial::PartialChunkStorage, storage::ChunkStorage},
    };

    #[test]
    fn test_out_of_bounds_y() {
        let mut chunk_storage = ChunkStorage::default();
        let mut partial_chunk_storage = PartialChunkStorage::default();
        partial_chunk_storage.set(
            &ChunkPos { x: 0, z: 0 },
            Some(Chunk::default()),
            &mut chunk_storage,
        );
        assert!(
            chunk_storage
                .get_block_state(BlockPos { x: 0, y: 319, z: 0 })
                .is_some()
        );
        assert!(
            chunk_storage
                .get_block_state(BlockPos { x: 0, y: 320, z: 0 })
                .is_none()
        );
        assert!(
            chunk_storage
                .get_block_state(BlockPos { x: 0, y: 338, z: 0 })
                .is_none()
        );
        assert!(
            chunk_storage
                .get_block_state(BlockPos { x: 0, y: -64, z: 0 })
                .is_some()
        );
        assert!(
            chunk_storage
                .get_block_state(BlockPos { x: 0, y: -65, z: 0 })
                .is_none()
        );
    }
}
