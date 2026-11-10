use std::sync::OnceLock;

use bevy::math::Vec2;

use crate::gui::hud_layout::{ElementId, Transforms};
use crate::gui::painter::Painter;
use crate::gui::screens::HotbarGeom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Jump,
    Sneak,
    Attack,
    Use,
    Inventory,
    Chat,
    Command,
    Pause,
    Camera,
    Debug,
    Drop,
    Offhand,
    #[cfg_attr(not(feature = "module_panel"), allow(dead_code))]
    Mods,
}

#[cfg(feature = "module_panel")]
pub const KEYS: &[Key] = &[
    Key::Jump,
    Key::Sneak,
    Key::Attack,
    Key::Use,
    Key::Inventory,
    Key::Chat,
    Key::Command,
    Key::Pause,
    Key::Camera,
    Key::Debug,
    Key::Drop,
    Key::Offhand,
    Key::Mods,
];
#[cfg(not(feature = "module_panel"))]
pub const KEYS: &[Key] = &[
    Key::Jump,
    Key::Sneak,
    Key::Attack,
    Key::Use,
    Key::Inventory,
    Key::Chat,
    Key::Command,
    Key::Pause,
    Key::Camera,
    Key::Debug,
    Key::Drop,
    Key::Offhand,
];

impl Key {
    pub fn bit(self) -> u16 {
        1 << self as u16
    }

    fn place(self) -> (f32, f32, f32) {
        match self {
            Key::Jump => (0.905, 0.7556, 0.8),
            Key::Sneak => (0.805, 0.8556, 0.6),
            Key::Attack => (0.815, 0.611, 0.6),
            Key::Use => (0.92, 0.533, 0.6),
            Key::Inventory => (0.95, 0.111, 0.55),
            Key::Chat => (0.88, 0.111, 0.55),
            Key::Command => (0.81, 0.111, 0.5),
            Key::Pause => (0.5, 0.0833, 0.45),
            Key::Camera => (0.43, 0.0833, 0.45),
            Key::Debug => (0.57, 0.0833, 0.45),
            Key::Drop => (0.711, 0.951, 0.4),
            Key::Offhand => (0.289, 0.951, 0.4),
            Key::Mods => (0.36, 0.0833, 0.45),
        }
    }

    pub fn element(self) -> ElementId {
        match self {
            Key::Jump | Key::Sneak => ElementId::JumpButton,
            Key::Attack | Key::Use => ElementId::UseButton,
            Key::Inventory | Key::Chat | Key::Command => ElementId::ChatButton,
            Key::Pause | Key::Camera | Key::Debug | Key::Mods => ElementId::ModButton,
            Key::Drop | Key::Offhand => ElementId::Hotbar,
        }
    }

    fn sprite(self) -> usize {
        self as usize
    }
}

const DPAD_SLACK: f32 = 1.1;
const BUTTON_SLACK: f32 = 1.15;
const HOTBAR_GAP: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    vw: f32,
    vh: f32,
    unit: f32,
    hotbar: (f32, f32),
    pub dpad_center: Vec2,
    pub dpad_half: f32,
}

impl Layout {
    pub fn new(vw: f32, vh: f32) -> Self {
        let unit = vw.min(vh) / 9.0;
        let geom = HotbarGeom::new(vw, vh);
        let (left, _, _, _) = geom.slot_rect(0);
        let (x, _, w, _) = geom.slot_rect(8);
        Layout {
            vw,
            vh,
            unit,
            hotbar: (left, x + w),
            dpad_center: Vec2::new(0.12 * vw, 0.733 * vh),
            dpad_half: 1.3 * unit,
        }
    }

    pub fn button(&self, key: Key) -> (Vec2, f32) {
        let (x, y, size) = key.place();
        let r = size * self.unit;
        let mut c = Vec2::new(x * self.vw, y * self.vh);
        let reach = r * BUTTON_SLACK + HOTBAR_GAP;
        match key {
            Key::Drop => c.x = c.x.max(self.hotbar.1 + reach),
            Key::Offhand => c.x = c.x.min(self.hotbar.0 - reach),
            _ => {}
        }
        (c, r)
    }

    pub fn button_at(&self, screen: Vec2, xf: &Transforms) -> Option<Key> {
        KEYS.iter().copied().find(|&key| {
            if key.element() == ElementId::Hotbar && !xf.drawn(ElementId::Hotbar) {
                return false;
            }
            let (c, r) = self.button(key);
            xf.to_local(key.element(), screen).distance(c) <= r * BUTTON_SLACK
        })
    }

    pub fn dpad_cell(&self, p: Vec2) -> Option<(i8, i8)> {
        let half = self.dpad_half;
        let d = p - self.dpad_center;
        if d.x.abs() > half * DPAD_SLACK || d.y.abs() > half * DPAD_SLACK {
            return None;
        }
        let cell = |v: f32| {
            if v < -half / 3.0 {
                -1
            } else if v > half / 3.0 {
                1
            } else {
                0
            }
        };
        Some((cell(d.x), cell(d.y)))
    }

    pub fn dpad_arrows(&self) -> [((i8, i8), Vec2, f32); 4] {
        let third = self.dpad_half * 2.0 / 3.0;
        let at = |cell: (i8, i8)| {
            let offset = Vec2::new(f32::from(cell.0), f32::from(cell.1)) * third;
            (cell, self.dpad_center + offset, third * 0.46)
        };
        [at((0, -1)), at((-1, 0)), at((1, 0)), at((0, 1))]
    }

    pub fn close(&self) -> (Vec2, f32) {
        (
            Vec2::new(self.vw - 0.8 * self.unit, 0.8 * self.unit),
            0.5 * self.unit,
        )
    }

    pub fn top_bottom(&self) -> f32 {
        KEYS.iter()
            .map(|&k| self.button(k))
            .filter(|(c, _)| c.y < self.vh / 2.0)
            .map(|(c, r)| c.y + r)
            .fold(0.0, f32::max)
    }
}

pub fn draw_group(p: &mut Painter, l: &Layout, element: ElementId, held: impl Fn(Key) -> bool) {
    for &key in KEYS.iter().filter(|k| k.element() == element) {
        let (c, r) = l.button(key);
        blit(p, c, r, sprite(key.sprite(), held(key)));
    }
}

pub fn draw_dpad(p: &mut Painter, l: &Layout, held: Option<(i8, i8)>) {
    for (i, (cell, c, half)) in l.dpad_arrows().into_iter().enumerate() {
        let on = held == Some(cell);
        blit(p, c, half, sprite(ARROWS + i, on));
    }
}

fn blit(p: &mut Painter, c: Vec2, half: f32, rects: &[Block]) {
    let side = (2.0 * half).round().max(1.0);
    let (x0, y0) = ((c.x - side / 2.0).round(), (c.y - side / 2.0).round());
    let px = side / T as f32;
    for r in rects {
        p.fill(
            x0 + f32::from(r.x) * px,
            y0 + f32::from(r.y) * px,
            f32::from(r.w) * px,
            f32::from(r.h) * px,
            r.argb,
        );
    }
}

struct Block {
    x: u8,
    y: u8,
    w: u8,
    h: u8,
    argb: u32,
}

const T: usize = 24;
const ARROWS: usize = 13;
const SPRITES: usize = ARROWS + 4;

fn sprite(i: usize, pressed: bool) -> &'static [Block] {
    static CACHE: OnceLock<Vec<Vec<Block>>> = OnceLock::new();
    let all = CACHE.get_or_init(|| {
        (0..SPRITES * 2)
            .map(|j| blocks(&tile(j / 2, j % 2 == 1), face_argb(j % 2 == 1)))
            .collect()
    });
    &all[i * 2 + usize::from(pressed)]
}

fn argb(a: u8, g: u8) -> u32 {
    u32::from(a) << 24 | u32::from(g) << 16 | u32::from(g) << 8 | u32::from(g)
}

fn tile(i: usize, pressed: bool) -> [u32; T * T] {
    let mut px = plate(pressed);
    let rows: Vec<Vec<bool>> = if i < ARROWS {
        bitmap(ICONS[i])
    } else {
        arrow(i - ARROWS)
    };
    let h = rows.len();
    let w = rows[0].len();
    let ox = (T - w) / 2;
    let oy = (T - h) / 2 + usize::from(pressed);
    let on = |x: i32, y: i32, rows: &Vec<Vec<bool>>| {
        let (x, y) = (x - ox as i32, y - oy as i32);
        x >= 0 && y >= 0 && (y as usize) < h && (x as usize) < w && rows[y as usize][x as usize]
    };
    for y in 0..T as i32 {
        for x in 0..T as i32 {
            if on(x, y, &rows) {
                continue;
            }
            let near = (-1..=1).any(|dy| (-1..=1).any(|dx| on(x + dx, y + dy, &rows)));
            if near {
                px[y as usize * T + x as usize] = ICON_EDGE;
            }
        }
    }
    for y in 0..T as i32 {
        for x in 0..T as i32 {
            if on(x, y, &rows) {
                px[y as usize * T + x as usize] = ICON;
            }
        }
    }
    px
}

const OUTLINE: u32 = 0xF510_1010;
const ICON: u32 = 0xFFFF_FFFF;
const ICON_EDGE: u32 = 0xFF26_2626;

fn face_argb(pressed: bool) -> u32 {
    argb(226, if pressed { 118 } else { 150 })
}

fn plate(pressed: bool) -> [u32; T * T] {
    let (light, dark): ([u8; 2], [u8; 2]) = if pressed {
        ([78, 92], [176, 158])
    } else {
        ([222, 196], [92, 112])
    };
    const BEVEL_ALPHA: u8 = 246;
    let last = T - 1;
    let mut px = [0u32; T * T];
    for y in 0..T {
        for x in 0..T {
            let i = y * T + x;
            if (x == 0 || x == last) && (y == 0 || y == last) {
                continue;
            }
            let ring = x.min(y).min(last - x).min(last - y);
            if ring == 0 {
                px[i] = OUTLINE;
                continue;
            }
            if (ring == 1 || ring == 2) && (x == 1 || x == last - 1) && (y == 1 || y == last - 1) {
                px[i] = OUTLINE;
                continue;
            }
            if ring <= 2 {
                let shade = if x + y < last {
                    light[ring - 1]
                } else {
                    dark[ring - 1]
                };
                px[i] = argb(BEVEL_ALPHA, shade);
            } else {
                px[i] = face_argb(pressed);
            }
        }
    }
    px
}

fn blocks(px: &[u32; T * T], face: u32) -> Vec<Block> {
    const INNER: usize = 3;
    let inner =
        |x: usize, y: usize| (INNER..T - INNER).contains(&x) && (INNER..T - INNER).contains(&y);
    let mut out = vec![Block {
        x: INNER as u8,
        y: INNER as u8,
        w: (T - 2 * INNER) as u8,
        h: (T - 2 * INNER) as u8,
        argb: face,
    }];
    let mut open: Vec<usize> = Vec::new();
    for y in 0..T {
        let mut next_open = Vec::new();
        let mut x = 0;
        while x < T {
            let c = px[y * T + x];
            let start = x;
            while x < T && px[y * T + x] == c && !(inner(x, y) && c == face) {
                x += 1;
            }
            if x == start {
                x += 1;
                continue;
            }
            if c >> 24 == 0 {
                continue;
            }
            let (bx, bw) = (start as u8, (x - start) as u8);
            let above = open
                .iter()
                .copied()
                .find(|&i| out[i].x == bx && out[i].w == bw && out[i].argb == c);
            match above {
                Some(i) => {
                    out[i].h += 1;
                    next_open.push(i);
                }
                None => {
                    out.push(Block {
                        x: bx,
                        y: y as u8,
                        w: bw,
                        h: 1,
                        argb: c,
                    });
                    next_open.push(out.len() - 1);
                }
            }
        }
        open = next_open;
    }
    out
}

fn bitmap(art: &[&str]) -> Vec<Vec<bool>> {
    let w = art.iter().map(|r| r.len()).max().unwrap_or(0);
    art.iter()
        .map(|r| (0..w).map(|x| r.as_bytes().get(x) == Some(&b'#')).collect())
        .collect()
}

fn arrow(turn: usize) -> Vec<Vec<bool>> {
    const S: usize = 16;
    let up = bitmap(UP);
    let top = (S - up.len()) / 2;
    let mut square = vec![vec![false; S]; S];
    for (y, row) in up.iter().enumerate() {
        for (x, &b) in row.iter().enumerate() {
            square[y + top][x] = b;
        }
    }
    let mut out = vec![vec![false; S]; S];
    for y in 0..S {
        for x in 0..S {
            let (nx, ny) = match turn {
                1 => (y, S - 1 - x),
                2 => (S - 1 - y, x),
                3 => (S - 1 - x, S - 1 - y),
                _ => (x, y),
            };
            out[ny][nx] = square[y][x];
        }
    }
    out
}

const UP: &[&str] = &[
    ".......##.......",
    "......####......",
    ".....######.....",
    "....########....",
    "...##########...",
    "..############..",
    ".##############.",
    "################",
];

const ICONS: [&[&str]; ARROWS] = [
    &[
        ".......##.......",
        "......####......",
        ".....######.....",
        "....########....",
        "...##########...",
        "..############..",
        ".....######.....",
        ".....######.....",
        ".....######.....",
        ".....######.....",
        "................",
        "..############..",
        "..############..",
    ],
    &[
        "..############..",
        "..############..",
        "................",
        ".....######.....",
        ".....######.....",
        ".....######.....",
        ".....######.....",
        "..############..",
        "...##########...",
        "....########....",
        ".....######.....",
        "......####......",
        ".......##.......",
    ],
    &[
        ".............###",
        "............####",
        "...........####.",
        "..........####..",
        ".........####...",
        "........####....",
        ".......####.....",
        "..##..####......",
        "...######.......",
        "....####........",
        "...######.......",
        "..###..##.......",
        ".###............",
        "###.............",
        "##..............",
    ],
    &[
        "......####......",
        "......####......",
        "......####......",
        "......####......",
        "................",
        "####...##...####",
        "####..####..####",
        "####..####..####",
        "####...##...####",
        "................",
        "......####......",
        "......####......",
        "......####......",
        "......####......",
    ],
    &["###..###..###", "###..###..###", "###..###..###"],
    &[
        ".############.",
        "##############",
        "##..........##",
        "##.########.##",
        "##..........##",
        "##.######...##",
        "##..........##",
        "##############",
        ".#####..######",
        "....###.......",
        "....##........",
        "....#.........",
    ],
    &[
        "..........###",
        ".........###.",
        "........###..",
        ".......###...",
        "......###....",
        ".....###.....",
        "....###......",
        "...###.......",
        "..###........",
        ".###.........",
        "###..........",
    ],
    &[
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
        "..####..####..",
    ],
    &[
        ".....######.....",
        "...##########...",
        ".####......####.",
        "###....##....###",
        "##....####....##",
        "###....##....###",
        ".####......####.",
        "...##########...",
        ".....######.....",
    ],
    &[
        "######.#####.",
        "######.######",
        "##.........##",
        "##.........##",
        "#####...####.",
        "#####...####.",
        "##.........##",
        "##.........##",
        "##.....######",
        "##.....#####.",
    ],
    &[
        "......##########",
        "......##########",
        "..........######",
        ".........#######",
        "........########",
        ".......####.####",
        "......####..####",
        ".....####...####",
        "....####....####",
        "...####.........",
        "..####..........",
        "................",
        "....############",
        "...############.",
        "..############..",
    ],
    &[
        "....##..........",
        "...###..........",
        "..##############",
        ".###############",
        "..##############",
        "...###..........",
        "....##..........",
        "................",
        "..........##....",
        "..........###...",
        "##############..",
        "###############.",
        "##############..",
        "..........###...",
        "..........##....",
    ],
    &[
        "#####..#####",
        "#####..#####",
        "#####..#####",
        "#####..#####",
        "#####..#####",
        "............",
        "............",
        "#####..#####",
        "#####..#####",
        "#####..#####",
        "#####..#####",
        "#####..#####",
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    const SIZES: [(f32, f32); 2] = [(427.0, 240.0), (533.0, 240.0)];

    #[test]
    fn dpad_cells_match_their_arrows() {
        for (vw, vh) in SIZES {
            let l = Layout::new(vw, vh);
            for (cell, c, _) in l.dpad_arrows() {
                assert_eq!(l.dpad_cell(c), Some(cell));
            }
            let half = l.dpad_half;
            assert_eq!(l.dpad_cell(l.dpad_center), Some((0, 0)));
            assert_eq!(
                l.dpad_cell(l.dpad_center + Vec2::splat(-half * 0.9)),
                Some((-1, -1))
            );
            assert_eq!(
                l.dpad_cell(l.dpad_center + Vec2::new(0.0, half * 1.2)),
                None
            );
        }
    }

    #[test]
    fn each_button_answers_at_its_centre() {
        for (vw, vh) in SIZES {
            let l = Layout::new(vw, vh);
            for &key in KEYS {
                let (c, _) = l.button(key);
                assert_eq!(l.button_at(c, &Transforms::IDENTITY), Some(key), "{key:?}");
            }
        }
    }

    #[test]
    fn arrows_point_at_their_cells() {
        for (turn, (dx, dy)) in [(0, (0, -1)), (1, (-1, 0)), (2, (1, 0)), (3, (0, 1))] {
            let a = arrow(turn);
            let lit: Vec<(i32, i32)> = (0..16)
                .flat_map(|y| (0..16).map(move |x| (x, y)))
                .filter(|&(x, y)| a[y as usize][x as usize])
                .collect();
            let n = lit.len() as f32;
            let mx = lit.iter().map(|p| p.0 as f32).sum::<f32>() / n - 7.5;
            let my = lit.iter().map(|p| p.1 as f32).sum::<f32>() / n - 7.5;
            assert!(mx * dx as f32 <= 0.0 && my * dy as f32 <= 0.0, "{turn}");
            assert!(mx.abs() + my.abs() > 0.5, "{turn} is centred");
        }
    }

    #[test]
    fn blocks_redraw_the_sprite() {
        for i in 0..SPRITES {
            for pressed in [false, true] {
                let t = tile(i, pressed);
                let b = blocks(&t, face_argb(pressed));
                let mut out = [0u32; T * T];
                let mut count = [0u8; T * T];
                for (n, r) in b.iter().enumerate() {
                    for y in r.y..r.y + r.h {
                        for x in r.x..r.x + r.w {
                            let k = usize::from(y) * T + usize::from(x);
                            out[k] = r.argb;
                            if n > 0 {
                                count[k] += 1;
                            }
                        }
                    }
                }
                assert_eq!(out, t, "sprite {i} pressed {pressed}");
                assert!(count.iter().all(|&c| c <= 1), "sprite {i} overlaps itself");
                assert!(b.len() < 200, "sprite {i}: {} blocks", b.len());
            }
        }
    }

    #[test]
    fn drop_and_offhand_clear_the_hotbar() {
        for (vw, vh) in SIZES {
            let l = Layout::new(vw, vh);
            let (d, dr) = l.button(Key::Drop);
            let (o, or) = l.button(Key::Offhand);
            assert!(d.x - dr * BUTTON_SLACK >= l.hotbar.1);
            assert!(o.x + or * BUTTON_SLACK <= l.hotbar.0);
        }
        let l = Layout::new(533.0, 240.0);
        assert_eq!(l.button(Key::Drop).0.x, 0.711 * 533.0);
    }

    #[test]
    fn every_sprite_has_an_outline_and_a_face() {
        for i in 0..SPRITES {
            for pressed in [false, true] {
                let t = tile(i, pressed);
                assert_eq!(t[0], 0, "corner cut");
                assert_eq!(t[1], OUTLINE);
                assert!(t.contains(&ICON), "sprite {i} has no icon");
            }
        }
    }
}
