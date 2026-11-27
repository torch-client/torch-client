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
    position::{
        ChunkBiomePos, ChunkBlockPos, ChunkPos, ChunkSectionBiomePos, ChunkSectionBlockPos,
    },
};
use azalea_registry::data::Biome;
use tracing::warn;

use crate::{
    heightmap::Heightmap,
    palette::{
        GlobalBits, PaletteKind, PalettedContainer, PalletedContainerKind,
        block_state_registry_bits, registry_bits,
    },
};

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
        Ok(Chunk {
            sections: sections.into_boxed_slice(),
            heightmaps: read_heightmaps(heightmaps_data, dimension_height, min_y),
            version: next_version(),
        })
    }

    pub fn read_lenient(
        data: &[u8],
        dimension_height: u32,
        min_y: i32,
        heightmaps_data: &[(HeightmapKind, Box<[u64]>)],
        biome_registry_len: usize,
    ) -> (Self, Option<ChunkDecodeFailure>) {
        let section_count = (dimension_height / SECTION_HEIGHT) as usize;
        let vanilla = SectionWidths {
            blocks: GlobalBits::Registry(block_state_registry_bits()),
            biomes: biome_width(biome_registry_len),
        };
        let first = decode_sections(data, section_count, vanilla);
        let clean = |pass: &DecodePass| pass.failure.is_none() && pass.consumed == data.len();
        let mut chosen = first;
        if !clean(&chosen) && chosen.ambiguous {
            let as_sent = SectionWidths {
                blocks: GlobalBits::AsSent,
                biomes: GlobalBits::AsSent,
            };
            let second = decode_sections(data, section_count, as_sent);
            if clean(&second) || second.decoded > chosen.decoded {
                chosen = second;
                if let Some(failure) = chosen.failure.as_mut() {
                    failure.read_as_sent = true;
                }
            }
        }
        let DecodePass {
            mut sections,
            failure,
            ..
        } = chosen;
        sections.resize_with(section_count, Section::default);
        (
            Chunk {
                sections: sections.into_boxed_slice(),
                heightmaps: read_heightmaps(heightmaps_data, dimension_height, min_y),
                version: next_version(),
            },
            failure,
        )
    }

    pub fn replace_biomes(
        &mut self,
        data: &[u8],
        biome_registry_len: usize,
    ) -> Result<(), ChunkDecodeFailure> {
        let section_count = self.sections.len();
        let first = decode_biomes(data, section_count, biome_width(biome_registry_len));
        let clean = |pass: &BiomePass| pass.failure.is_none() && pass.consumed == data.len();
        let mut chosen = first;
        if !clean(&chosen) && chosen.ambiguous {
            let second = decode_biomes(data, section_count, GlobalBits::AsSent);
            if clean(&second) || second.biomes.len() > chosen.biomes.len() {
                chosen = second;
                if let Some(failure) = chosen.failure.as_mut() {
                    failure.read_as_sent = true;
                }
            }
        }
        if let Some(failure) = chosen.failure {
            return Err(failure);
        }
        for (section, biomes) in self.sections.iter_mut().zip(chosen.biomes) {
            section.biomes = biomes;
        }
        self.version = next_version();
        Ok(())
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

fn read_heightmaps(
    heightmaps_data: &[(HeightmapKind, Box<[u64]>)],
    dimension_height: u32,
    min_y: i32,
) -> HashMap<HeightmapKind, Heightmap> {
    let mut heightmaps = HashMap::new();
    for (kind, data) in heightmaps_data {
        let heightmap = Heightmap::new(*kind, dimension_height, min_y, data.clone());
        heightmaps.insert(*kind, heightmap);
    }
    heightmaps
}

#[derive(Clone, Copy, Debug)]
struct SectionWidths {
    blocks: GlobalBits,
    biomes: GlobalBits,
}

#[derive(Clone, Debug)]
pub struct ChunkDecodeFailure {
    pub section: usize,
    pub sections: usize,
    pub part: &'static str,
    pub sent_bits: Option<u8>,
    pub palette: Option<PaletteKind>,
    pub read_bits: Option<u8>,
    pub read_as_sent: bool,
    pub error: String,
}

pub const BIOME_UPDATE: &str = "biome update";

fn biome_width(biome_registry_len: usize) -> GlobalBits {
    if biome_registry_len == 0 {
        GlobalBits::AsSent
    } else {
        GlobalBits::Registry(registry_bits(biome_registry_len))
    }
}

fn global_mismatch<S: PalletedContainerKind>(c: &PalettedContainer<S>, sent: u8) -> bool {
    matches!(c.palette, crate::palette::Palette::Global) && c.bits_per_entry != sent
}

fn width_for(
    kind: &Option<PaletteKind>,
    sent: Option<u8>,
    global: GlobalBits,
    stored: fn(u8) -> u8,
) -> Option<u8> {
    let sent = sent?;
    Some(match (kind, global) {
        (Some(PaletteKind::Global), GlobalBits::Registry(bits)) => bits,
        (Some(PaletteKind::Global), GlobalBits::AsSent) => sent,
        _ => stored(sent),
    })
}

struct BiomePass {
    biomes: Vec<PalettedContainer<Biome>>,
    failure: Option<ChunkDecodeFailure>,
    consumed: usize,
    ambiguous: bool,
}

fn decode_biomes(data: &[u8], section_count: usize, global: GlobalBits) -> BiomePass {
    let mut buf = Cursor::new(data);
    let mut pass = BiomePass {
        biomes: Vec::with_capacity(section_count),
        failure: None,
        consumed: 0,
        ambiguous: false,
    };
    for section in 0..section_count {
        match PalettedContainer::<Biome>::read_with(&mut buf, global) {
            Ok((biomes, sent)) => {
                pass.ambiguous |= global_mismatch(&biomes, sent);
                pass.biomes.push(biomes);
            }
            Err(e) => {
                let read_bits = width_for(&e.kind, e.sent_bits, global, Biome::storage_bits);
                pass.ambiguous |= e.kind == Some(PaletteKind::Global) && read_bits != e.sent_bits;
                pass.failure = Some(ChunkDecodeFailure {
                    section,
                    sections: section_count,
                    part: BIOME_UPDATE,
                    sent_bits: e.sent_bits,
                    palette: e.kind,
                    read_bits,
                    read_as_sent: false,
                    error: e.error.to_string(),
                });
                break;
            }
        }
    }
    pass.consumed = buf.position() as usize;
    pass
}

struct DecodePass {
    sections: Vec<Section>,
    decoded: usize,
    failure: Option<ChunkDecodeFailure>,
    consumed: usize,
    ambiguous: bool,
}

fn decode_sections(data: &[u8], section_count: usize, widths: SectionWidths) -> DecodePass {
    let mut buf = Cursor::new(data);
    let mut pass = DecodePass {
        sections: Vec::with_capacity(section_count),
        decoded: 0,
        failure: None,
        consumed: 0,
        ambiguous: false,
    };
    for index in 0..section_count {
        let fail = |part: &'static str,
                    sent_bits: Option<u8>,
                    palette: Option<PaletteKind>,
                    read_bits: Option<u8>,
                    error: String| ChunkDecodeFailure {
            section: index,
            sections: section_count,
            part,
            sent_bits,
            palette,
            read_bits,
            read_as_sent: false,
            error,
        };
        let counts = u16::azalea_read(&mut buf).and_then(|b| Ok((b, u16::azalea_read(&mut buf)?)));
        let (block_count, fluid_count) = match counts {
            Ok(counts) => counts,
            Err(e) => {
                pass.failure = Some(fail("header", None, None, None, e.to_string()));
                break;
            }
        };
        let states = match PalettedContainer::<BlockState>::read_with(&mut buf, widths.blocks) {
            Ok((states, sent)) => {
                pass.ambiguous |= global_mismatch(&states, sent);
                states
            }
            Err(e) => {
                let read_bits =
                    width_for(&e.kind, e.sent_bits, widths.blocks, BlockState::storage_bits);
                pass.ambiguous |= e.kind == Some(PaletteKind::Global) && read_bits != e.sent_bits;
                let error = e.error.to_string();
                pass.failure = Some(fail("block states", e.sent_bits, e.kind, read_bits, error));
                break;
            }
        };
        match PalettedContainer::<Biome>::read_with(&mut buf, widths.biomes) {
            Ok((biomes, sent)) => {
                pass.ambiguous |= global_mismatch(&biomes, sent);
                pass.sections.push(Section {
                    block_count,
                    fluid_count,
                    states,
                    biomes,
                });
                pass.decoded += 1;
            }
            Err(e) => {
                let read_bits =
                    width_for(&e.kind, e.sent_bits, widths.biomes, Biome::storage_bits);
                pass.ambiguous |= e.kind == Some(PaletteKind::Global) && read_bits != e.sent_bits;
                let error = e.error.to_string();
                pass.failure = Some(fail("biomes", e.sent_bits, e.kind, read_bits, error));
                pass.sections.push(Section {
                    block_count,
                    fluid_count,
                    states,
                    biomes: PalettedContainer::new(),
                });
                break;
            }
        }
    }
    pass.consumed = buf.position() as usize;
    pass
}

pub fn biome_registry_len(registries: &azalea_core::registry_holder::RegistryHolder) -> usize {
    use azalea_registry::DataRegistry;
    let id = azalea_registry::identifier::Identifier::from(Biome::NAME);
    registries.extra.get(&id).map_or(0, |registry| registry.map.len())
}

pub type DecodeFailureHook = fn(ChunkPos, &ChunkDecodeFailure);

static DECODE_FAILURE_HOOK: std::sync::OnceLock<DecodeFailureHook> = std::sync::OnceLock::new();

pub fn set_decode_failure_hook(hook: DecodeFailureHook) {
    let _ = DECODE_FAILURE_HOOK.set(hook);
}

pub fn report_decode_failure(pos: ChunkPos, failure: &ChunkDecodeFailure) {
    match DECODE_FAILURE_HOOK.get() {
        Some(hook) => hook(pos, failure),
        None if failure.part == BIOME_UPDATE => warn!(
            "column {},{}: a biome update would not decode at section {} of {} ({:?} bits sent, \
             read at {:?}, {:?} palette): {}; the column keeps the biomes it had",
            pos.x,
            pos.z,
            failure.section,
            failure.sections,
            failure.sent_bits,
            failure.read_bits,
            failure.palette,
            failure.error
        ),
        None => warn!(
            "column {},{}: section {} of {} would not decode ({}, {:?} bits sent, read at {:?}, \
             {:?} palette): {}; the rest of the column is air",
            pos.x,
            pos.z,
            failure.section,
            failure.sections,
            failure.part,
            failure.sent_bits,
            failure.read_bits,
            failure.palette,
            failure.error
        ),
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
    fn replace_biomes_is_all_or_nothing() {
        use azalea_registry::DataRegistry;

        let biome_of = |chunk: &Chunk, section: usize| {
            chunk.sections[section].biomes.get_at_index(0).protocol_id()
        };
        let mut chunk = Chunk::default();
        let version = chunk.version();
        let mut bytes = [0, 5].repeat(chunk.sections.len());
        bytes.push(9);

        chunk.replace_biomes(&bytes, 65).unwrap();
        assert!((0..chunk.sections.len()).all(|i| biome_of(&chunk, i) == 5));
        assert_ne!(chunk.version(), version);

        let version = chunk.version();
        let failure = chunk.replace_biomes(&[0, 6], 65).unwrap_err();
        assert_eq!((failure.section, failure.part), (1, BIOME_UPDATE));
        assert_eq!(biome_of(&chunk, 0), 5);
        assert_eq!(chunk.version(), version);
    }

    #[test]
    fn narrow_global_byte_is_stored_at_the_native_width() {
        let native = BlockState::storage_bits(9);
        assert_eq!(BlockState::storage_bits(native - 1), native);
        assert_eq!(BlockState::storage_bits(native), native);
        assert_eq!(BlockState::storage_bits(native + 1), native + 1);
    }
}
