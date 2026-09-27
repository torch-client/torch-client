use crate::gui::painter::Painter;

pub struct Accent {
    pub name: &'static str,
    pub base: u32,
    pub hi: u32,
}

pub static ACCENTS: &[Accent] = &[
    Accent {
        name: "Emerald",
        base: 0x2F_A8_78,
        hi: 0x38_BE_8B,
    },
    Accent {
        name: "Sky",
        base: 0x3B_8F_E0,
        hi: 0x4F_A3_F2,
    },
    Accent {
        name: "Amber",
        base: 0xD9_A2_38,
        hi: 0xF2_BA_52,
    },
    Accent {
        name: "Rose",
        base: 0xDB_57_75,
        hi: 0xF2_70_8C,
    },
    Accent {
        name: "Violet",
        base: 0x86_5C_EF,
        hi: 0x9E_74_F5,
    },
    Accent {
        name: "Steel",
        base: 0x62_72_8C,
        hi: 0x7A_8A_A3,
    },
];

pub struct Surface {
    pub name: &'static str,
    pub base: u32,
    pub light: bool,
}

pub static SURFACES: &[Surface] = &[
    Surface {
        name: "Ink",
        base: 0x0E_0E_0E,
        light: false,
    },
    Surface {
        name: "Graphite",
        base: 0x17_19_1C,
        light: false,
    },
    Surface {
        name: "Midnight",
        base: 0x0D_13_20,
        light: false,
    },
    Surface {
        name: "Moss",
        base: 0x11_18_15,
        light: false,
    },
    Surface {
        name: "Plum",
        base: 0x17_10_1B,
        light: false,
    },
    Surface {
        name: "Ash",
        base: 0x21_21_23,
        light: false,
    },
    Surface {
        name: "Paper",
        base: 0xEC_E7_DF,
        light: true,
    },
    Surface {
        name: "Mist",
        base: 0xDF_E4_EA,
        light: true,
    },
];

pub const fn lighten(rgb: u32, amount: u32) -> u32 {
    let mut out = 0;
    let mut shift = 0;
    while shift <= 16 {
        let c = (rgb >> shift) & 0xFF;
        let c = if c + amount > 0xFF { 0xFF } else { c + amount };
        out |= c << shift;
        shift += 8;
    }
    out
}

pub const fn darken(rgb: u32, amount: u32) -> u32 {
    let mut out = 0;
    let mut shift = 0;
    while shift <= 16 {
        let c = (rgb >> shift) & 0xFF;
        let c = if c < amount { 0 } else { c - amount };
        out |= c << shift;
        shift += 8;
    }
    out
}

pub const ROW: u32 = 0x00_00_00_00;
pub const ROW_HOVER: u32 = 0x22_FF_FF_FF;
pub const TRACK: u32 = 0xFF_33_33_33;
pub const KNOB: u32 = 0xFF_F2_F2_F2;

pub const TEXT_TITLE: u32 = 0xFF_FF_FF;
pub const TEXT: u32 = 0xC0_C0_C0;
pub const TEXT_DIM: u32 = 0x77_77_77;
pub const TEXT_TITLE_L: u32 = 0x14_14_16;
pub const TEXT_L: u32 = 0x36_36_3A;
pub const TEXT_DIM_L: u32 = 0x86_86_8C;

pub const ROW_HOVER_L: u32 = 0x1F_00_00_00;
pub const TRACK_L: u32 = 0xFF_C4_C0_B8;
pub const TEXT_ON: u32 = 0xF2_FF_F8;

pub fn chevron(p: &mut Painter, x: f32, y: f32, w: f32, down: bool, argb: u32) {
    let rows = 3.0_f32;
    let step = w * 0.5 / rows;
    for i in 0..rows as u32 {
        let i = i as f32;
        let row = if down { i } else { rows - 1.0 - i };
        p.fill(x + i * step, y + row, w - 2.0 * i * step, 1.0, argb);
    }
}

pub fn revert(p: &mut Painter, x: f32, y: f32, argb: u32) {
    p.fill(x, y, 1.0, 7.0, argb);
    for i in 0..4 {
        let i = i as f32;
        p.fill(x + 2.0 + i, y + 3.0 - i, 1.0, 1.0 + 2.0 * i, argb);
    }
}

pub fn dots(p: &mut Painter, x: f32, y: f32, argb: u32) {
    for i in 0..3 {
        p.fill(x, y + i as f32 * 2.5, 1.0, 1.0, argb);
    }
}
