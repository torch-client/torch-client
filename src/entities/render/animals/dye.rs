use crate::entities::state::EntityState;

pub const TEXTURE_DIFFUSE: [u32; 16] = [
    16383998, 16351261, 13061821, 3847130, 16701501, 8439583, 15961002, 4673362, 10329495, 1481884,
    8991416, 3949738, 8606770, 6192150, 11546150, 1908001,
];

pub fn tint(rgb: u32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        1.0,
    ]
}

pub fn dye_tint(ordinal: u8) -> [f32; 4] {
    tint(TEXTURE_DIFFUSE[(ordinal as usize) % 16])
}

fn sheep_color(ordinal: usize) -> u32 {
    if ordinal == 0 {
        return 0xE6_E6_E6;
    }
    let src = TEXTURE_DIFFUSE[ordinal % 16];
    let scale = |c: u32| ((c as f32 * 0.75).floor() as u32).min(255);
    (scale((src >> 16) & 0xFF) << 16) | (scale((src >> 8) & 0xFF) << 8) | scale(src & 0xFF)
}

const SHEEP_COLOR_DURATION: i32 = 25;

pub fn sheep_wool_tint(st: &EntityState) -> [f32; 4] {
    if !st.extras.magic_name_jeb {
        return tint(sheep_color(st.extras.wool_color as usize));
    }
    let tick_count = st.age_ticks.floor() as i32;
    let value = tick_count / SHEEP_COLOR_DURATION;
    let c1 = value.rem_euclid(16) as usize;
    let c2 = (value + 1).rem_euclid(16) as usize;
    let sub_step = ((tick_count % SHEEP_COLOR_DURATION) as f32 + st.age_ticks.fract())
        / SHEEP_COLOR_DURATION as f32;
    let a = sheep_color(c1);
    let b = sheep_color(c2);
    let lerp = |shift: u32| {
        let from = ((a >> shift) & 0xFF) as f32;
        let to = ((b >> shift) & 0xFF) as f32;
        (from + sub_step * (to - from)).round() as u32
    };
    tint((lerp(16) << 16) | (lerp(8) << 8) | lerp(0))
}
