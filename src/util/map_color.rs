const MATERIAL_COLORS: [u32; 62] = [
    0x000000, 0x7FB238, 0xF7E9A3, 0xC7C7C7, 0xFF0000, 0xA0A0FF, 0xA7A7A7, 0x007C00, 0xFFFFFF,
    0xA4A8B8, 0x976D4D, 0x707070, 0x4040FF, 0x8F7748, 0xFFFCF5, 0xD87F33, 0xB24CD8, 0x6699D8,
    0xE5E533, 0x7FCC19, 0xF27FA5, 0x4C4C4C, 0x999999, 0x4C7F99, 0x7F3FB2, 0x334CB2, 0x664C33,
    0x667F33, 0x993333, 0x191919, 0xFAEE4D, 0x5CDBD5, 0x4A80FF, 0x00D93A, 0x815631, 0x700200,
    0xD1B1A1, 0x9F5224, 0x95576C, 0x706C8A, 0xBA8524, 0x677535, 0xA04D4E, 0x392923, 0x876B62,
    0x575C5C, 0x7A4958, 0x4C3E5C, 0x4C3223, 0x4C522A, 0x8E3C2E, 0x251610, 0xBD3031, 0x943F61,
    0x5C191D, 0x167E86, 0x3A8E8C, 0x562C3E, 0x14B485, 0x646464, 0xD8AF93, 0x7FA796,
];

const BRIGHTNESS: [u32; 4] = [180, 220, 255, 135];

pub fn rgba_from_packed_id(packed: u8) -> [u8; 4] {
    let material = (packed >> 2) as usize;
    if material == 0 {
        return [0, 0, 0, 0];
    }
    let Some(color) = MATERIAL_COLORS.get(material) else {
        return [0, 0, 0, 0];
    };
    let scale = BRIGHTNESS[(packed & 3) as usize];
    let channel = |shift: u32| (((color >> shift) & 0xff) * scale / 255).min(255) as u8;
    [channel(16), channel(8), channel(0), 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_matches_vanilla() {
        assert_eq!(MATERIAL_COLORS.len(), 62);
        assert_eq!(MATERIAL_COLORS[1], 8368696);
        assert_eq!(MATERIAL_COLORS[61], 8365974);
        assert_eq!(BRIGHTNESS, [180, 220, 255, 135]);
    }

    #[test]
    fn grass_scales_by_brightness() {
        assert_eq!(rgba_from_packed_id(4), [89, 125, 39, 255]);
        assert_eq!(rgba_from_packed_id(5), [109, 153, 48, 255]);
        assert_eq!(rgba_from_packed_id(6), [127, 178, 56, 255]);
        assert_eq!(rgba_from_packed_id(7), [67, 94, 29, 255]);
    }

    #[test]
    fn unexplored_is_transparent() {
        for shade in 0..4 {
            assert_eq!(rgba_from_packed_id(shade), [0, 0, 0, 0]);
        }
        assert_eq!(rgba_from_packed_id(63 << 2), [0, 0, 0, 0]);
    }
}
