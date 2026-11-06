pub mod partial;
pub mod storage;

use std::{
    collections::HashMap,
    fmt::Debug,
    io,
    io::{Cursor, Write},
    sync::atomic::{AtomicU64, Ordering},
};

use azalea_block::block_state::BlockState;
use azalea_buf::{AzBuf, BufReadError};
use azalea_core::{
    heightmap_kind::HeightmapKind,
    position::{ChunkBiomePos, ChunkBlockPos, ChunkSectionBiomePos, ChunkSectionBlockPos},
};
use azalea_registry::data::Biome;
use tracing::warn;

use crate::{heightmap::Heightmap, palette::PalettedContainer};

const SECTION_HEIGHT: u32 = 16;

#[derive(Debug)]
pub struct Chunk {
    pub sections: Box<[Section]>,
    pub heightmaps: HashMap<HeightmapKind, Heightmap>,
    version: u64,
}

static NEXT_VERSION: AtomicU64 = AtomicU64::new(1);

fn next_version() -> u64 {
    NEXT_VERSION.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Section {
    pub block_count: u16,
    pub fluid_count: u16,
    pub states: PalettedContainer<BlockState>,
    pub biomes: PalettedContainer<Biome>,
}

pub fn calculate_chunk_storage_range(view_distance: u32) -> u32 {
    u32::max(view_distance, 2) + 3
}

impl Default for Chunk {
    fn default() -> Self {
        Chunk {
            sections: vec![Section::default(); (384 / 16) as usize].into(),
            heightmaps: HashMap::new(),
            version: next_version(),
        }
    }
}

impl Chunk {
    pub fn read_with_dimension_height(
        buf: &mut Cursor<&[u8]>,
        dimension_height: u32,
        min_y: i32,
        heightmaps_data: &[(HeightmapKind, Box<[u64]>)],
    ) -> Result<Self, BufReadError> {
        let section_count = dimension_height / SECTION_HEIGHT;
        let mut sections = Vec::with_capacity(section_count as usize);
        for _ in 0..section_count {
            let section = Section::azalea_read(buf)?;
            sections.push(section);
        }
        let sections = sections.into_boxed_slice();

        let mut heightmaps = HashMap::new();
        for (kind, data) in heightmaps_data {
            let data = data.clone();
            let heightmap = Heightmap::new(*kind, dimension_height, min_y, data);
            heightmaps.insert(*kind, heightmap);
        }

        Ok(Chunk {
            sections,
            heightmaps,
            version: next_version(),
        })
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn get_block_state(&self, pos: &ChunkBlockPos, min_y: i32) -> Option<BlockState> {
        get_block_state_from_sections(&self.sections, pos, min_y)
    }

    #[must_use = "Use Chunk::set_block_state instead if you don't need the previous state"]
    pub fn get_and_set_block_state(
        &mut self,
        pos: &ChunkBlockPos,
        state: BlockState,
        min_y: i32,
    ) -> BlockState {
        let section_index = section_index(pos.y, min_y);
        let Some(section) = self.sections.get_mut(section_index as usize) else {
            warn!(
                "Tried to get and set block state {state:?} at out-of-bounds relative chunk position {pos:?}",
            );
            return BlockState::AIR;
        };
        let chunk_section_pos = ChunkSectionBlockPos::from(pos);
        let previous_state = section.get_and_set_block_state(chunk_section_pos, state);
        self.version = next_version();

        for heightmap in self.heightmaps.values_mut() {
            heightmap.update(pos, state, &self.sections);
        }

        previous_state
    }

    pub fn set_block_state(&mut self, pos: &ChunkBlockPos, state: BlockState, min_y: i32) {
        let section_index = section_index(pos.y, min_y);
        let Some(section) = self.sections.get_mut(section_index as usize) else {
            warn!(
                "Tried to set block state {state:?} at out-of-bounds relative chunk position {pos:?}",
            );
            return;
        };
        let chunk_section_pos = ChunkSectionBlockPos::from(pos);
        section.get_and_set_block_state(chunk_section_pos, state);
        self.version = next_version();

        for heightmap in self.heightmaps.values_mut() {
            heightmap.update(pos, state, &self.sections);
        }
    }

    pub fn get_biome(&self, pos: ChunkBiomePos, min_y: i32) -> Option<Biome> {
        if pos.y < min_y {
            return None;
        }
        let section_index = section_index(pos.y, min_y);
        let Some(section) = self.sections.get(section_index as usize) else {
            warn!("Tried to get biome at out-of-bounds relative chunk position {pos:?}",);
            return None;
        };
        let chunk_section_pos = ChunkSectionBiomePos::from(pos);
        Some(section.get_biome(chunk_section_pos))
    }
}

#[inline]
pub fn get_block_state_from_sections(
    sections: &[Section],
    pos: &ChunkBlockPos,
    min_y: i32,
) -> Option<BlockState> {
    if pos.y < min_y {
        return None;
    }
    let section_index = section_index(pos.y, min_y) as usize;
    if section_index >= sections.len() {
        return None;
    };
    let section = &sections[section_index];

    let chunk_section_pos = ChunkSectionBlockPos::from(pos);
    Some(section.get_block_state(chunk_section_pos))
}

impl AzBuf for Section {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let block_count = u16::azalea_read(buf)?;
        let fluid_count = u16::azalea_read(buf)?;

        let states = PalettedContainer::<BlockState>::read(buf)?;
        let biomes = PalettedContainer::<Biome>::read(buf)?;
        Ok(Section {
            block_count,
            fluid_count,
            states,
            biomes,
        })
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.block_count.azalea_write(buf)?;
        self.fluid_count.azalea_write(buf)?;
        self.states.write(buf)?;
        self.biomes.write(buf)?;
        Ok(())
    }
}

impl Section {
    pub fn get_block_state(&self, pos: ChunkSectionBlockPos) -> BlockState {
        self.states.get(pos)
    }
    pub fn get_and_set_block_state(
        &mut self,
        pos: ChunkSectionBlockPos,
        state: BlockState,
    ) -> BlockState {
        let previous_state = self.states.get_and_set(pos, state);

        if previous_state.is_air() && !state.is_air() {
            self.block_count += 1;
        } else if !previous_state.is_air() && state.is_air() {
            self.block_count -= 1;
        }

        previous_state
    }

    pub fn get_biome(&self, pos: ChunkSectionBiomePos) -> Biome {
        self.biomes.get(pos)
    }
    pub fn set_biome(&mut self, pos: ChunkSectionBiomePos, biome: Biome) {
        self.biomes.set(pos, biome);
    }
    pub fn get_and_set_biome(&mut self, pos: ChunkSectionBiomePos, biome: Biome) -> Biome {
        self.biomes.get_and_set(pos, biome)
    }
}

#[inline]
pub fn section_index(y: i32, min_y: i32) -> u32 {
    if y < min_y {
        #[cfg(debug_assertions)]
        tracing::warn!("y ({y}) must be at least {min_y}");
        #[cfg(not(debug_assertions))]
        tracing::trace!("y ({y}) must be at least {min_y}")
    };
    let min_section_index = min_y >> 4;
    ((y >> 4) - min_section_index) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BitStorage,
        palette::{PalletedContainerKind, SectionPos},
    };

    #[test]
    fn test_section_index() {
        assert_eq!(section_index(0, 0), 0);
        assert_eq!(section_index(128, 0), 8);
        assert_eq!(section_index(127, 0), 7);
        assert_eq!(section_index(0, -64), 4);
        assert_eq!(section_index(-64, -64), 0);
        assert_eq!(section_index(-49, -64), 0);
        assert_eq!(section_index(-48, -64), 1);
        assert_eq!(section_index(128, -64), 12);
    }

    #[test]
    fn serialize_and_deserialize_section() {
        let mut states = PalettedContainer::new();

        states.set(
            SectionPos::new(1, 2, 3),
            BlockState::try_from(BlockState::MAX_STATE).unwrap(),
        );
        states.set(
            SectionPos::new(4, 5, 6),
            BlockState::try_from(BlockState::MAX_STATE).unwrap(),
        );
        let biomes = PalettedContainer::new();
        let section = Section {
            block_count: 2,
            fluid_count: 0,
            states,
            biomes,
        };

        let mut buf = Vec::new();
        section.azalea_write(&mut buf).unwrap();

        let mut cur = Cursor::new(buf.as_slice());
        let deserialized_section = Section::azalea_read(&mut cur).unwrap();
        assert_eq!(cur.position(), buf.len() as u64);

        assert_eq!(section, deserialized_section);
    }

    fn global_section(bits: u8, first: u32) -> Vec<u8> {
        let mut storage = BitStorage::new(bits as usize, 4096, None).unwrap();
        for i in 0..4096 {
            storage.set(i, 1);
        }
        storage.set(0, first as u64);

        let mut bytes = Vec::new();
        4096_u16.azalea_write(&mut bytes).unwrap();
        0_u16.azalea_write(&mut bytes).unwrap();
        bits.azalea_write(&mut bytes).unwrap();
        for word in &storage.data {
            word.azalea_write(&mut bytes).unwrap();
        }
        bytes.extend([0, 0]);
        bytes
    }

    fn assert_next_section_aligned(mut bytes: Vec<u8>) -> Section {
        let expected_next = BlockState::try_from(2_u32).unwrap();
        bytes.extend([16, 0, 0, 0, 0, 2, 0, 0]);

        let mut cur = Cursor::new(bytes.as_slice());
        let section = Section::azalea_read(&mut cur).unwrap();
        let next = Section::azalea_read(&mut cur).unwrap();

        assert_eq!(cur.position(), bytes.len() as u64);
        assert_eq!(next.states.get_at_index(0), expected_next);
        section
    }

    #[test]
    fn global_section_with_unknown_id_survives() {
        let unknown = u32::from(BlockState::MAX_STATE) + 1;
        let native = BlockState::storage_bits(9);

        let section = assert_next_section_aligned(global_section(native, unknown));

        assert_eq!(section.states.get_at_index(0), BlockState::stand_in());
        assert_eq!(
            section.states.get_at_index(1),
            BlockState::try_from(1_u32).unwrap()
        );

        let mut unpacked = vec![BlockState::AIR; 4096];
        section.states.unpack_into(&mut unpacked);
        assert_eq!(unpacked[0], BlockState::stand_in());
        assert_eq!(unpacked[1], BlockState::try_from(1_u32).unwrap());
    }

    #[test]
    fn wide_global_sections_decode_aligned() {
        for (bits, id) in [(16, 65_535), (17, 131_071), (32, u32::MAX)] {
            let section = assert_next_section_aligned(global_section(bits, id));

            assert_eq!(section.states.bits_per_entry, bits);
            assert_eq!(section.states.get_at_index(0), BlockState::stand_in());
            assert_eq!(
                section.states.get_at_index(4095),
                BlockState::try_from(1_u32).unwrap()
            );
        }
    }

    #[test]
    fn wide_global_section_round_trips_its_raw_ids() {
        let bytes = global_section(17, 131_071);
        let section = Section::azalea_read(&mut Cursor::new(bytes.as_slice())).unwrap();

        let mut written = Vec::new();
        section.azalea_write(&mut written).unwrap();

        assert_eq!(written, bytes);
    }

    #[test]
    fn block_section_wider_than_a_u32_fails() {
        let bytes = [0, 0, 0, 0, 33];
        assert!(Section::azalea_read(&mut Cursor::new(&bytes[..])).is_err());
    }

    #[test]
    fn narrow_global_byte_is_stored_at_the_native_width() {
        let native = BlockState::storage_bits(9);
        assert_eq!(BlockState::storage_bits(native - 1), native);
        assert_eq!(BlockState::storage_bits(native), native);
        assert_eq!(BlockState::storage_bits(native + 1), native + 1);
    }
}
