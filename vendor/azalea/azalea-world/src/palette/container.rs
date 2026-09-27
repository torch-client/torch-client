use std::{
    fmt::Debug,
    io::{self, Cursor, Write},
};

use azalea_block::BlockState;
use azalea_buf::{AzBuf, BufReadError};
use azalea_core::position::{ChunkSectionBiomePos, ChunkSectionBlockPos};
use azalea_registry::data::Biome;
use tracing::{debug, warn};

use super::{Palette, PaletteKind};
use crate::BitStorage;

#[derive(Clone, Debug, PartialEq)]
pub struct PalettedContainer<S: PalletedContainerKind> {
    pub bits_per_entry: u8,
    pub palette: Palette<S>,
    pub storage: BitStorage,
}

pub trait PalletedContainerKind:
    Copy + Clone + Debug + Default + PartialEq + TryFrom<u32> + Into<u32>
{
    type SectionPos: SectionPos;

    fn size_bits() -> usize;

    fn size() -> usize {
        1 << (Self::size_bits() * 3)
    }

    fn bits_per_entry_to_palette_kind(bits_per_entry: u8) -> PaletteKind;
}
impl PalletedContainerKind for BlockState {
    type SectionPos = ChunkSectionBlockPos;

    fn size_bits() -> usize {
        4
    }

    fn bits_per_entry_to_palette_kind(bits_per_entry: u8) -> PaletteKind {
        match bits_per_entry {
            0 => PaletteKind::SingleValue,
            1..=4 => PaletteKind::Linear,
            5..=8 => PaletteKind::Hashmap,
            _ => PaletteKind::Global,
        }
    }
}
impl PalletedContainerKind for Biome {
    type SectionPos = ChunkSectionBiomePos;

    fn size_bits() -> usize {
        2
    }

    fn bits_per_entry_to_palette_kind(bits_per_entry: u8) -> PaletteKind {
        match bits_per_entry {
            0 => PaletteKind::SingleValue,
            1..=3 => PaletteKind::Linear,
            _ => PaletteKind::Global,
        }
    }
}

pub trait SectionPos {
    fn coords(&self) -> (usize, usize, usize);
    fn new(x: usize, y: usize, z: usize) -> Self;
}
impl SectionPos for ChunkSectionBlockPos {
    fn coords(&self) -> (usize, usize, usize) {
        (self.x as usize, self.y as usize, self.z as usize)
    }

    fn new(x: usize, y: usize, z: usize) -> Self {
        ChunkSectionBlockPos {
            x: x as u8,
            y: y as u8,
            z: z as u8,
        }
    }
}
impl SectionPos for ChunkSectionBiomePos {
    fn coords(&self) -> (usize, usize, usize) {
        (self.x as usize, self.y as usize, self.z as usize)
    }

    fn new(x: usize, y: usize, z: usize) -> Self {
        ChunkSectionBiomePos {
            x: x as u8,
            y: y as u8,
            z: z as u8,
        }
    }
}

impl<S: PalletedContainerKind> PalettedContainer<S> {
    pub fn new() -> Self {
        let palette = Palette::SingleValue(S::default());
        let size = S::size();
        let storage = BitStorage::new(0, size, Some(Box::new([]))).unwrap();

        PalettedContainer {
            bits_per_entry: 0,
            palette,
            storage,
        }
    }

    pub fn read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let bits_per_entry = u8::azalea_read(buf)?;
        let palette_type = S::bits_per_entry_to_palette_kind(bits_per_entry);
        let palette = palette_type.read(buf)?;
        let size = S::size();

        let mut storage = match BitStorage::new(
            bits_per_entry as usize,
            size,
            if bits_per_entry == 0 {
                Some(Box::new([]))
            } else {
                None
            },
        ) {
            Ok(storage) => storage,
            Err(e) => {
                warn!("Failed to create bit storage: {:?}", e);
                return Err(BufReadError::Custom(
                    "Failed to create bit storage".to_owned(),
                ));
            }
        };

        for i in 0..storage.data.len() {
            storage.data[i] = u64::azalea_read(buf)?;
        }

        Ok(PalettedContainer {
            bits_per_entry,
            palette,
            storage,
        })
    }

    pub fn index_from_pos(&self, pos: S::SectionPos) -> usize {
        let size_bits = S::size_bits();
        let (x, y, z) = pos.coords();
        (((y << size_bits) | z) << size_bits) | x
    }

    pub fn pos_from_index(&self, index: usize) -> S::SectionPos {
        let size_bits = S::size_bits();
        let mask = (1 << size_bits) - 1;
        S::SectionPos::new(
            index & mask,
            (index >> size_bits >> size_bits) & mask,
            (index >> size_bits) & mask,
        )
    }

    #[inline]
    pub fn get_at_index(&self, index: usize) -> S {
        let paletted_value = self.storage.get(index);
        self.palette.value_for(paletted_value as usize)
    }

    pub fn get(&self, pos: S::SectionPos) -> S {
        self.get_at_index(self.index_from_pos(pos))
    }

    pub fn get_and_set(&mut self, pos: S::SectionPos, value: S) -> S {
        let paletted_value = self.id_for(value);
        let old_paletted_value = self
            .storage
            .get_and_set(self.index_from_pos(pos), paletted_value as u64);
        self.palette.value_for(old_paletted_value as usize)
    }

    pub fn set_at_index(&mut self, index: usize, value: S) {
        let paletted_value = self.id_for(value);
        self.storage.set(index, paletted_value as u64);
    }

    pub fn set(&mut self, pos: S::SectionPos, value: S) {
        self.set_at_index(self.index_from_pos(pos), value);
    }

    fn create_or_reuse_data(&self, bits_per_entry: u8) -> PalettedContainer<S> {
        let new_palette_type = S::bits_per_entry_to_palette_kind(bits_per_entry);

        let old_palette_type = (&self.palette).into();
        if bits_per_entry == self.bits_per_entry && new_palette_type == old_palette_type {
            return self.clone();
        }
        let storage = BitStorage::new(bits_per_entry as usize, S::size(), None).unwrap();

        debug_assert_eq!(storage.size(), S::size());

        let palette = new_palette_type.as_empty_palette();
        PalettedContainer {
            bits_per_entry,
            palette,
            storage,
        }
    }

    fn on_resize(&mut self, bits_per_entry: u8, value: S) -> usize {
        debug!(
            "Resizing PalettedContainer from {} bpe to {bits_per_entry} for {value:?} with palette={:?}",
            self.bits_per_entry, self.palette
        );
        let mut new_data = self.create_or_reuse_data(bits_per_entry);
        new_data.copy_from(&self.palette, &self.storage);
        *self = new_data;
        self.id_for(value)
    }

    fn copy_from(&mut self, palette: &Palette<S>, storage: &BitStorage) {
        for i in 0..storage.size() {
            let value = palette.value_for(storage.get(i) as usize);
            let id = self.id_for(value) as u64;
            self.storage.set(i, id);
        }
    }

    pub fn id_for(&mut self, value: S) -> usize {
        match &mut self.palette {
            Palette::SingleValue(v) => {
                if (*v).into() != value.into() {
                    self.on_resize(1, value)
                } else {
                    0
                }
            }
            Palette::Linear(palette) => {
                if let Some(index) = palette.iter().position(|&v| v.into() == value.into()) {
                    return index;
                }
                let capacity = 2usize.pow(self.bits_per_entry.into());
                if capacity > palette.len() {
                    palette.push(value);
                    palette.len() - 1
                } else {
                    self.on_resize(self.bits_per_entry + 1, value)
                }
            }
            Palette::Hashmap(palette) => {
                if let Some(index) = palette.iter().position(|v| (*v).into() == value.into()) {
                    return index;
                }
                let capacity = 2usize.pow(self.bits_per_entry.into());
                if capacity > palette.len() {
                    palette.push(value);
                    palette.len() - 1
                } else {
                    self.on_resize(self.bits_per_entry + 1, value)
                }
            }
            Palette::Global => value.into() as usize,
        }
    }
}

impl<S: PalletedContainerKind> PalettedContainer<S> {
    pub fn write(&self, buf: &mut impl Write) -> io::Result<()> {
        self.bits_per_entry.azalea_write(buf)?;
        self.palette.write(buf)?;
        for word in &self.storage.data {
            word.azalea_write(buf)?;
        }

        Ok(())
    }
}

impl<S: PalletedContainerKind> Default for PalettedContainer<S> {
    fn default() -> Self {
        Self::new()
    }
}
