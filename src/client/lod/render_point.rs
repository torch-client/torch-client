pub const EMPTY: u64 = 0;

pub const MAX_WORLD_Y_SIZE: i32 = 2031 + 4064;

const ALPHA_DOWNSIZE_SHIFT: u32 = 4;
const MATERIAL_SHIFT: u32 = 60;
const COLOR_SHIFT: u32 = 32;
const BLUE_SHIFT: u32 = COLOR_SHIFT;
const GREEN_SHIFT: u32 = BLUE_SHIFT + 8;
const RED_SHIFT: u32 = GREEN_SHIFT + 8;
const ALPHA_SHIFT: u32 = RED_SHIFT + 8;
const MAX_Y_SHIFT: u32 = 20;
const MIN_Y_SHIFT: u32 = 8;
const BLOCK_LIGHT_SHIFT: u32 = 4;
const SKY_LIGHT_SHIFT: u32 = 0;

const ALPHA_MASK: u64 = 0xF;
const CHANNEL_MASK: u64 = 0xFF;
const COLOR_MASK: u64 = 0xFF_FFFF;
const Y_MASK: u64 = 0xFFF;
const MIN_MAX_Y_MASK: u64 = 0xFF_FFFF;
const LIGHT_MASK: u64 = 0xF;
const MATERIAL_MASK: u64 = 0xF;

#[inline]
pub fn create(
    max_y: i32,
    min_y: i32,
    argb: u32,
    sky_light: u8,
    block_light: u8,
    material: u8,
) -> u64 {
    let a = (argb >> 24) & 0xFF;
    let r = (argb >> 16) & 0xFF;
    let g = (argb >> 8) & 0xFF;
    let b = argb & 0xFF;
    ((a >> ALPHA_DOWNSIZE_SHIFT) as u64) << ALPHA_SHIFT
        | (r as u64 & CHANNEL_MASK) << RED_SHIFT
        | (g as u64 & CHANNEL_MASK) << GREEN_SHIFT
        | (b as u64 & CHANNEL_MASK) << BLUE_SHIFT
        | (max_y as u64 & Y_MASK) << MAX_Y_SHIFT
        | (min_y as u64 & Y_MASK) << MIN_Y_SHIFT
        | (block_light as u64 & LIGHT_MASK) << BLOCK_LIGHT_SHIFT
        | (sky_light as u64 & LIGHT_MASK) << SKY_LIGHT_SHIFT
        | (material as u64 & MATERIAL_MASK) << MATERIAL_SHIFT
}

#[inline]
pub fn y_max(p: u64) -> i32 {
    ((p >> MAX_Y_SHIFT) & Y_MASK) as i32
}

#[inline]
pub fn y_min(p: u64) -> i32 {
    ((p >> MIN_Y_SHIFT) & Y_MASK) as i32
}

#[inline]
pub fn set_y_min(p: u64, min_y: i32) -> u64 {
    (p & !(Y_MASK << MIN_Y_SHIFT)) | (min_y as u64 & Y_MASK) << MIN_Y_SHIFT
}

#[inline]
pub fn alpha(p: u64) -> u8 {
    ((((p >> ALPHA_SHIFT) & ALPHA_MASK) << ALPHA_DOWNSIZE_SHIFT) | 0b1111) as u8
}

#[inline]
pub fn red(p: u64) -> u8 {
    ((p >> RED_SHIFT) & CHANNEL_MASK) as u8
}

#[inline]
pub fn green(p: u64) -> u8 {
    ((p >> GREEN_SHIFT) & CHANNEL_MASK) as u8
}

#[inline]
pub fn blue(p: u64) -> u8 {
    ((p >> BLUE_SHIFT) & CHANNEL_MASK) as u8
}

#[inline]
pub fn sky_light(p: u64) -> u8 {
    ((p >> SKY_LIGHT_SHIFT) & LIGHT_MASK) as u8
}

#[inline]
pub fn block_light(p: u64) -> u8 {
    ((p >> BLOCK_LIGHT_SHIFT) & LIGHT_MASK) as u8
}

#[inline]
pub fn material(p: u64) -> u8 {
    ((p >> MATERIAL_SHIFT) & MATERIAL_MASK) as u8
}

#[inline]
pub fn has_zero_height(p: u64) -> bool {
    (p >> MIN_Y_SHIFT) & MIN_MAX_Y_MASK == 0
}

#[inline]
pub fn exists(p: u64) -> bool {
    p != EMPTY
}

#[inline]
pub fn color(p: u64) -> u32 {
    let a = alpha(p) as u32;
    (((p >> COLOR_SHIFT) & COLOR_MASK) as u32) | a << 24
}

#[inline]
pub fn is_fluid(p: u64) -> bool {
    let m = material(p);
    m == super::blocks::Material::Water as u8 || m == super::blocks::Material::Lava as u8
}

#[inline]
pub fn is_same_fluid_on_top(p: u64, above: u64) -> bool {
    exists(above) && y_min(above) == y_max(p) && material(above) == material(p)
}

#[inline]
pub fn is_surface_lowered(p: u64, above: u64) -> bool {
    is_fluid(p) && !is_same_fluid_on_top(p, above)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_round_trip() {
        let p = create(300, 12, 0xFF80_4020, 15, 7, 12);
        assert_eq!(y_max(p), 300);
        assert_eq!(y_min(p), 12);
        assert_eq!((red(p), green(p), blue(p)), (0x80, 0x40, 0x20));
        assert_eq!(alpha(p), 0xFF);
        assert_eq!(sky_light(p), 15);
        assert_eq!(block_light(p), 7);
        assert_eq!(material(p), 12);
        assert!(is_fluid(p));
        assert!(!has_zero_height(p));
    }
}
