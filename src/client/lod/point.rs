pub const EMPTY: u64 = 0;

const ID_WIDTH: u32 = 32;
const HEIGHT_WIDTH: u32 = 12;
const MIN_Y_WIDTH: u32 = 12;
const SKY_LIGHT_WIDTH: u32 = 4;

const ID_OFFSET: u32 = 0;
const HEIGHT_OFFSET: u32 = ID_OFFSET + ID_WIDTH;
const MIN_Y_OFFSET: u32 = HEIGHT_OFFSET + HEIGHT_WIDTH;
const SKY_LIGHT_OFFSET: u32 = MIN_Y_OFFSET + MIN_Y_WIDTH;
const BLOCK_LIGHT_OFFSET: u32 = SKY_LIGHT_OFFSET + SKY_LIGHT_WIDTH;

const ID_MASK: u64 = 0x7FFF_FFFF;
const HEIGHT_MASK: u64 = (1 << HEIGHT_WIDTH) - 1;
const MIN_Y_MASK: u64 = (1 << MIN_Y_WIDTH) - 1;
const LIGHT_MASK: u64 = 0xF;

#[inline]
pub fn encode(id: u32, height: i32, min_y: i32, block_light: u8, sky_light: u8) -> u64 {
    (id as u64 & ID_MASK) << ID_OFFSET
        | (height as u64 & HEIGHT_MASK) << HEIGHT_OFFSET
        | (min_y as u64 & MIN_Y_MASK) << MIN_Y_OFFSET
        | (block_light as u64 & LIGHT_MASK) << BLOCK_LIGHT_OFFSET
        | (sky_light as u64 & LIGHT_MASK) << SKY_LIGHT_OFFSET
}

#[inline]
pub fn id(p: u64) -> u32 {
    ((p >> ID_OFFSET) & ID_MASK) as u32
}

#[inline]
pub fn height(p: u64) -> i32 {
    ((p >> HEIGHT_OFFSET) & HEIGHT_MASK) as i32
}

#[inline]
pub fn bottom_y(p: u64) -> i32 {
    ((p >> MIN_Y_OFFSET) & MIN_Y_MASK) as i32
}

#[inline]
pub fn sky_light(p: u64) -> u8 {
    ((p >> SKY_LIGHT_OFFSET) & LIGHT_MASK) as u8
}

#[inline]
pub fn block_light(p: u64) -> u8 {
    ((p >> BLOCK_LIGHT_OFFSET) & LIGHT_MASK) as u8
}

#[inline]
pub fn pack_id(state: u16, biome: u8) -> u32 {
    state as u32 | (biome as u32) << 16
}

#[inline]
pub fn id_state(id: u32) -> u16 {
    (id & 0xFFFF) as u16
}

#[inline]
pub fn id_biome(id: u32) -> u8 {
    ((id >> 16) & 0xFF) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_round_trip() {
        let p = encode(pack_id(1234, 7), 100, 64, 3, 15);
        assert_eq!(id_state(id(p)), 1234);
        assert_eq!(id_biome(id(p)), 7);
        assert_eq!(height(p), 100);
        assert_eq!(bottom_y(p), 64);
        assert_eq!(block_light(p), 3);
        assert_eq!(sky_light(p), 15);
    }
}
