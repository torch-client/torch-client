use azalea_block::BlockState;
use azalea_core::position::ChunkSectionBlockPos;

use super::*;

#[test]
fn test_resize_0_bits_to_1() {
    let mut palette_container = PalettedContainer::<BlockState>::new();

    assert_eq!(palette_container.bits_per_entry, 0);
    assert_eq!(palette_container.get_at_index(0), BlockState::AIR);
    assert_eq!(
        PaletteKind::from(&palette_container.palette),
        PaletteKind::SingleValue
    );
    let block_state_1 = BlockState::try_from(1_u32).unwrap();
    palette_container.set_at_index(0, block_state_1);
    assert_eq!(palette_container.get_at_index(0), block_state_1);
    assert_eq!(
        PaletteKind::from(&palette_container.palette),
        PaletteKind::Linear
    );
}

#[test]
fn test_resize_8_bits_to_global_keeps_every_state() {
    let mut palette_container = PalettedContainer::<BlockState>::new();
    let state = |i: usize| BlockState::try_from(20_000 + i as u32).unwrap();

    for i in 0..257 {
        palette_container.set_at_index(i, state(i));
    }

    assert_eq!(
        PaletteKind::from(&palette_container.palette),
        PaletteKind::Global
    );
    assert_eq!(
        palette_container.bits_per_entry,
        BlockState::storage_bits(9)
    );
    for i in 0..257 {
        assert_eq!(palette_container.get_at_index(i), state(i));
    }
}

#[test]
fn test_unknown_palette_entry_reads_as_stand_in() {
    use std::io::Cursor;

    use azalea_buf::{AzBuf, AzBufVar};

    let unknown = u32::from(BlockState::MAX_STATE) + 1;
    let mut bytes = Vec::new();
    2_u8.azalea_write(&mut bytes).unwrap();
    vec![1_u32, unknown].azalea_write_var(&mut bytes).unwrap();
    for _ in 0..256 {
        0x1010_1010_1010_1010_u64.azalea_write(&mut bytes).unwrap();
    }

    let mut cursor = Cursor::new(bytes.as_slice());
    let container = PalettedContainer::<BlockState>::read(&mut cursor).unwrap();

    assert_eq!(cursor.position(), bytes.len() as u64);
    assert_eq!(
        container.get_at_index(0),
        BlockState::try_from(1_u32).unwrap()
    );
    assert_eq!(container.get_at_index(1), BlockState::stand_in());
    assert!(azalea_registry::unknown::snapshot().blocks >= 1);
}

#[test]
fn test_unknown_biome_keeps_its_default() {
    use azalea_registry::data::Biome;

    assert_eq!(
        <Biome as PalletedContainerKind>::from_network(u32::MAX),
        Biome::try_from(u32::MAX).unwrap_or_default()
    );
}

#[test]
fn test_resize_0_bits_to_5() {
    let mut palette_container = PalettedContainer::<BlockState>::new();

    let set = |pc: &mut PalettedContainer<BlockState>, i, v: u32| {
        pc.set_at_index(i, BlockState::try_from(v).unwrap());
    };

    set(&mut palette_container, 0, 0);
    assert_eq!(palette_container.bits_per_entry, 0);

    set(&mut palette_container, 1, 1);
    assert_eq!(palette_container.bits_per_entry, 4);

    set(&mut palette_container, 2, 2);
    assert_eq!(palette_container.bits_per_entry, 4);
    set(&mut palette_container, 3, 3);

    set(&mut palette_container, 4, 4);
    assert_eq!(palette_container.bits_per_entry, 4);
    set(&mut palette_container, 5, 5);
    set(&mut palette_container, 6, 6);
    set(&mut palette_container, 7, 7);

    set(&mut palette_container, 8, 8);
    assert_eq!(palette_container.bits_per_entry, 4);
    set(&mut palette_container, 9, 9);
    set(&mut palette_container, 10, 10);
    set(&mut palette_container, 11, 11);
    set(&mut palette_container, 12, 12);
    set(&mut palette_container, 13, 13);
    set(&mut palette_container, 14, 14);
    set(&mut palette_container, 15, 15);
    assert_eq!(palette_container.bits_per_entry, 4);

    set(&mut palette_container, 16, 16);
    assert_eq!(palette_container.bits_per_entry, 5);
}

#[test]
fn test_read_narrow_block_container_at_four_bits() {
    let mut bytes = vec![2, 2, 0, 1];
    let mut packed = 0u64;
    for slot in 0..16 {
        packed |= (slot as u64 & 1) << (slot * 4);
    }
    for _ in 0..256 {
        bytes.extend_from_slice(&packed.to_be_bytes());
    }
    bytes.push(0xAB);

    let mut cur = Cursor::new(&bytes[..]);
    let container = PalettedContainer::<BlockState>::read(&mut cur).unwrap();
    assert_eq!(container.bits_per_entry, 4);
    assert_eq!(container.get_at_index(0), BlockState::AIR);
    assert_eq!(container.get_at_index(1), BlockState::try_from(1_u32).unwrap());
    assert_eq!(cur.position() as usize, bytes.len() - 1, "stopped at the next field");
}

#[test]
fn test_coords_from_index() {
    let palette_container = PalettedContainer::<BlockState>::new();

    for x in 0..15 {
        for y in 0..15 {
            for z in 0..15 {
                assert_eq!(
                    palette_container.pos_from_index(
                        palette_container.index_from_pos(ChunkSectionBlockPos::new(x, y, z))
                    ),
                    ChunkSectionBlockPos::new(x, y, z)
                );
            }
        }
    }
}
