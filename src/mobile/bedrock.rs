use std::sync::OnceLock;

use bevy::math::Vec2;

#[cfg(feature = "asset_download")]
use super::art::{self, Tile};
use crate::gui::hud_layout::{ElementId, Transforms};
use crate::gui::painter::Painter;
use crate::gui::screens::HotbarGeom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scheme {
    Dpad,
    Joystick,
}

impl Scheme {
    pub const ALL: [Scheme; 2] = [Scheme::Dpad, Scheme::Joystick];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Jump,
    Sneak,
    Sprint,
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
    Key::Sprint,
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
    Key::Sprint,
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

    fn place(self, scheme: Scheme) -> Option<(f32, f32, f32)> {
        let (dpad, joystick) = match self {
            Key::Jump => ((0.90, 0.78, 0.14), (0.92, 0.78, 0.16)),
            Key::Sneak => ((MOVE.0, MOVE.1, 0.10), (0.81, 0.86, 0.115)),
            Key::Sprint => return (scheme == Scheme::Joystick).then_some((0.72, 0.78, 0.105)),
            Key::Attack => return Some((0.80, 0.64, 0.16)),
            Key::Use => return Some((0.92, 0.52, 0.16)),
            Key::Inventory => return Some((0.73, 0.95, 0.075)),
            Key::Offhand => return Some((0.80, 0.96, 0.075)),
            Key::Drop => return Some((0.87, 0.96, 0.075)),
            Key::Chat | Key::Command | Key::Pause | Key::Camera | Key::Debug | Key::Mods => {
                return Some((0.5, TOP_Y, TOP_SIDE));
            }
        };
        Some(match scheme {
            Scheme::Dpad => dpad,
            Scheme::Joystick => joystick,
        })
    }

    fn slot(self) -> Option<i8> {
        match self {
            Key::Command if cfg!(feature = "module_panel") => Some(-3),
            Key::Command => Some(1),
            Key::Debug => Some(-2),
            Key::Chat => Some(-1),
            Key::Pause => Some(0),
            Key::Mods => Some(1),
            Key::Camera => Some(2),
            _ => None,
        }
    }

    pub fn element(self, scheme: Scheme) -> ElementId {
        match self {
            Key::Sneak if scheme == Scheme::Dpad => ElementId::Pad,
            Key::Jump | Key::Sneak | Key::Sprint => ElementId::JumpButton,
            Key::Attack | Key::Use => ElementId::UseButton,
            Key::Chat | Key::Command | Key::Pause | Key::Camera | Key::Debug | Key::Mods => {
                ElementId::ChatButton
            }
            Key::Inventory | Key::Drop | Key::Offhand => ElementId::Hotbar,
        }
    }

    fn sprite(self) -> usize {
        self as usize
    }
}

const MOVE: (f32, f32, f32) = (0.13, 0.76, 0.32);
const TOP_Y: f32 = 0.055;
const TOP_SIDE: f32 = 0.075;
const TOP_PITCH: f32 = 0.04;
const TOP_SPREAD: f32 = 1.1;
const HOTBAR_GAP: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Square {
    pub center: Vec2,
    pub side: f32,
}

impl Square {
    pub fn contains(&self, p: Vec2) -> bool {
        let half = self.side / 2.0;
        (p.x - self.center.x).abs() <= half && (p.y - self.center.y).abs() <= half
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    vw: f32,
    vh: f32,
    pub scheme: Scheme,
    unit: f32,
    pitch: f32,
    hotbar_shift: f32,
}

impl Layout {
    pub fn new(vw: f32, vh: f32, scheme: Scheme) -> Self {
        let geom = HotbarGeom::new(vw, vh);
        let (x, _, w, _) = geom.slot_rect(8);
        let inventory_left = 0.73 * vw - TOP_SIDE * vh / 2.0;
        Layout {
            vw,
            vh,
            scheme,
            unit: vw.min(vh) / 9.0,
            pitch: (TOP_PITCH * vw).max(TOP_SIDE * vh * TOP_SPREAD),
            hotbar_shift: (x + w + HOTBAR_GAP - inventory_left).max(0.0),
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        KEYS.iter()
            .copied()
            .filter(|&k| k.place(self.scheme).is_some())
    }

    pub fn button(&self, key: Key) -> Option<Square> {
        let (x, y, size) = key.place(self.scheme)?;
        let mut center = Vec2::new(x * self.vw, y * self.vh);
        if let Some(slot) = key.slot() {
            center.x = 0.5 * self.vw + f32::from(slot) * self.pitch;
        }
        if key.element(self.scheme) == ElementId::Hotbar {
            center.x += self.hotbar_shift;
        }
        Some(Square {
            center,
            side: size * self.vh,
        })
    }

    pub fn movement(&self) -> Square {
        Square {
            center: Vec2::new(MOVE.0 * self.vw, MOVE.1 * self.vh),
            side: MOVE.2 * self.vh,
        }
    }

    pub fn button_at(&self, screen: Vec2, xf: &Transforms) -> Option<Key> {
        self.keys().find(|&key| {
            let element = key.element(self.scheme);
            if element == ElementId::Hotbar && !xf.drawn(ElementId::Hotbar) {
                return false;
            }
            self.button(key)
                .is_some_and(|sq| sq.contains(xf.to_local(element, screen)))
        })
    }

    pub fn dpad_cell(&self, p: Vec2) -> (i8, i8) {
        let m = self.movement();
        let third = m.side / 6.0;
        let d = p - m.center;
        let cell = |v: f32| {
            if v < -third {
                -1
            } else if v > third {
                1
            } else {
                0
            }
        };
        (cell(d.x), cell(d.y))
    }

    pub fn dpad_hit(&self, p: Vec2) -> Option<(i8, i8)> {
        self.movement().contains(p).then(|| self.dpad_cell(p))
    }

    pub fn dpad_square(&self, cell: (i8, i8)) -> Square {
        let m = self.movement();
        let third = m.side / 3.0;
        Square {
            center: m.center + Vec2::new(f32::from(cell.0), f32::from(cell.1)) * third,
            side: third,
        }
    }

    pub fn dpad_arrows(&self) -> [((i8, i8), Vec2, f32); 4] {
        let at = |cell: (i8, i8)| {
            let sq = self.dpad_square(cell);
            (cell, sq.center, sq.side * 0.46)
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
        self.keys()
            .filter_map(|k| self.button(k))
            .filter(|sq| sq.center.y < self.vh / 2.0)
            .map(|sq| sq.center.y + sq.side / 2.0)
            .fold(0.0, f32::max)
    }
}

#[derive(Clone, Copy, Default)]
pub struct Lit {
    pub art: bool,
    #[cfg_attr(not(feature = "asset_download"), allow(dead_code))]
    pub flying: bool,
    pub keys: u16,
    pub dpad: Option<(i8, i8)>,
    #[cfg_attr(not(feature = "asset_download"), allow(dead_code))]
    pub stick: Vec2,
}

#[cfg(feature = "asset_download")]
const TINT: u32 = 0xA6FF_FFFF;

pub fn art_ready(atlas: &crate::gui::atlas::GuiAtlas) -> bool {
    #[cfg(feature = "asset_download")]
    return art::region(atlas, Tile::Jump, false).is_some();
    #[cfg(not(feature = "asset_download"))]
    {
        let _ = atlas;
        false
    }
}

pub fn draw_group(p: &mut Painter, l: &Layout, element: ElementId, lit: &Lit) {
    for key in l.keys().filter(|k| k.element(l.scheme) == element) {
        let Some(sq) = l.button(key) else { continue };
        let pressed = lit.keys & key.bit() != 0;
        #[cfg(feature = "asset_download")]
        if lit.art {
            draw_tile(p, key_tile(key, l.scheme, lit.flying), pressed, sq);
            continue;
        }
        blit(p, sq.center, sq.side / 2.0, sprite(key.sprite(), pressed));
    }
}

pub fn draw_dpad(p: &mut Painter, l: &Layout, lit: &Lit) {
    #[cfg(feature = "asset_download")]
    if lit.art {
        let (x, y) = lit.dpad.unwrap_or((0, 0));
        for (cell, tile, pressed) in [
            ((0, -1), Tile::Up, y < 0),
            ((-1, 0), Tile::Left, x < 0),
            ((1, 0), Tile::Right, x > 0),
            ((0, 1), Tile::Down, y > 0),
        ] {
            draw_tile(p, tile, pressed, l.dpad_square(cell));
        }
        if y != 0 {
            let (left, right) = if y < 0 {
                (Tile::UpLeft, Tile::UpRight)
            } else {
                (Tile::DownLeft, Tile::DownRight)
            };
            draw_tile(p, left, x < 0, l.dpad_square((-1, y)));
            draw_tile(p, right, x > 0, l.dpad_square((1, y)));
        }
        return;
    }
    for (i, (cell, c, half)) in l.dpad_arrows().into_iter().enumerate() {
        let on = lit.dpad == Some(cell);
        blit(p, c, half, sprite(ARROWS + i, on));
    }
}

pub fn draw_stick(p: &mut Painter, l: &Layout, lit: &Lit) {
    #[cfg(feature = "asset_download")]
    if lit.art {
        let Some((corner, side)) = draw_tile(p, Tile::Frame, false, l.movement()) else {
            return;
        };
        let knob = side * 0.34;
        let top_left = corner + side * (Vec2::splat(0.33) + lit.stick * 0.25);
        draw_tile(
            p,
            Tile::Knob,
            false,
            Square {
                center: top_left + Vec2::splat(knob / 2.0),
                side: knob,
            },
        );
    }
    #[cfg(not(feature = "asset_download"))]
    let _ = (p, l, lit);
}

#[cfg(feature = "asset_download")]
fn key_tile(key: Key, scheme: Scheme, flying: bool) -> Tile {
    match key {
        Key::Jump if flying => Tile::FlyUp,
        Key::Sneak if flying => Tile::FlyDown,
        Key::Jump if scheme == Scheme::Dpad => Tile::JumpDpad,
        Key::Sneak if scheme == Scheme::Dpad => Tile::SneakDpad,
        Key::Jump => Tile::Jump,
        Key::Sneak => Tile::Sneak,
        Key::Sprint => Tile::Sprint,
        Key::Attack => Tile::Attack,
        Key::Use => Tile::Use,
        Key::Inventory => Tile::Inventory,
        Key::Chat => Tile::Chat,
        Key::Command => Tile::Command,
        Key::Pause => Tile::Pause,
        Key::Camera => Tile::Camera,
        Key::Debug => Tile::Debug,
        Key::Drop => Tile::Drop,
        Key::Offhand => Tile::Offhand,
        Key::Mods => Tile::Mods,
    }
}

#[cfg(feature = "asset_download")]
fn draw_tile(p: &mut Painter, tile: Tile, pressed: bool, sq: Square) -> Option<(Vec2, f32)> {
    let r = art::region(p.atlas, tile, pressed)?;
    let device = p.scale.max(f32::EPSILON);
    let px = r.w as f32;
    let side = (sq.side * device / px).round().max(1.0) * px / device;
    let snap = |v: f32| (v * device).round() / device;
    let corner = sq.center - Vec2::splat(side / 2.0);
    let corner = Vec2::new(snap(corner.x), snap(corner.y));
    p.region_tinted(r, corner.x, corner.y, side, side, TINT);
    Some((corner, side))
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
const ARROWS: usize = 14;
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
        "###....###.....",
        ".###....###....",
        "..###....###...",
        "...###....###..",
        "....###....###.",
        ".....###....###",
        ".....###....###",
        "....###....###.",
        "...###....###..",
        "..###....###...",
        ".###....###....",
        "###....###.....",
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

    fn layouts() -> impl Iterator<Item = Layout> {
        SIZES
            .into_iter()
            .flat_map(|(vw, vh)| Scheme::ALL.into_iter().map(move |s| Layout::new(vw, vh, s)))
    }

    fn overlap(a: Square, b: Square) -> bool {
        let reach = (a.side + b.side) / 2.0;
        (a.center.x - b.center.x).abs() < reach && (a.center.y - b.center.y).abs() < reach
    }

    #[test]
    fn dpad_cells_match_their_arrows() {
        for l in layouts() {
            for (cell, c, _) in l.dpad_arrows() {
                assert_eq!(l.dpad_cell(c), cell);
                assert_eq!(l.dpad_square(cell).center, c);
            }
            let m = l.movement();
            assert_eq!(l.dpad_cell(m.center), (0, 0));
            assert_eq!(
                l.dpad_cell(m.center + Vec2::splat(-m.side * 0.45)),
                (-1, -1)
            );
        }
    }

    #[test]
    fn dpad_hit_and_read_agree_inside_the_pad() {
        for l in layouts() {
            let m = l.movement();
            for i in 0..20 {
                for j in 0..20 {
                    let t = (Vec2::new(i as f32, j as f32) + 0.5) / 20.0 - 0.5;
                    let at = m.center + t * m.side;
                    assert_eq!(l.dpad_hit(at), Some(l.dpad_cell(at)), "{at}");
                }
            }
            let past = m.center + Vec2::new(0.0, -m.side);
            assert_eq!(l.dpad_hit(past), None);
            assert_eq!(l.dpad_cell(past), (0, -1));
            let corner = m.center + Vec2::splat(m.side);
            assert_eq!(l.dpad_hit(corner), None);
            assert_eq!(l.dpad_cell(corner), (1, 1));
        }
    }

    #[test]
    fn each_button_answers_at_its_centre() {
        for l in layouts() {
            for key in l.keys() {
                let sq = l.button(key).unwrap();
                assert_eq!(
                    l.button_at(sq.center, &Transforms::IDENTITY),
                    Some(key),
                    "{key:?} {:?}",
                    l.scheme
                );
            }
        }
    }

    #[test]
    fn no_two_controls_overlap() {
        for l in layouts() {
            let squares: Vec<(Key, Square)> = l.keys().map(|k| (k, l.button(k).unwrap())).collect();
            for (i, &(a, sa)) in squares.iter().enumerate() {
                for &(b, sb) in &squares[i + 1..] {
                    assert!(
                        !overlap(sa, sb),
                        "{a:?} overlaps {b:?} in {:?} at {}",
                        l.scheme,
                        l.vw
                    );
                }
                if !(a == Key::Sneak && l.scheme == Scheme::Dpad) {
                    assert!(
                        !overlap(sa, l.movement()),
                        "{a:?} overlaps the pad in {:?} at {}",
                        l.scheme,
                        l.vw
                    );
                }
                let half = sa.side / 2.0;
                assert!(
                    sa.center.x - half >= 0.0 && sa.center.x + half <= l.vw,
                    "{a:?}"
                );
                assert!(
                    sa.center.y - half >= 0.0 && sa.center.y + half <= l.vh,
                    "{a:?}"
                );
            }
        }
    }

    #[test]
    fn only_the_joystick_scheme_has_sprint() {
        assert!(
            Layout::new(427.0, 240.0, Scheme::Dpad)
                .button(Key::Sprint)
                .is_none()
        );
        assert!(
            Layout::new(427.0, 240.0, Scheme::Joystick)
                .button(Key::Sprint)
                .is_some()
        );
    }

    #[test]
    fn sneak_wins_the_dpad_centre() {
        for (vw, vh) in SIZES {
            let l = Layout::new(vw, vh, Scheme::Dpad);
            let c = l.movement().center;
            assert_eq!(l.button_at(c, &Transforms::IDENTITY), Some(Key::Sneak));
            assert_eq!(l.dpad_hit(c), Some((0, 0)));
        }
    }

    #[test]
    fn top_row_sits_at_starlings_shares() {
        let (vw, vh) = (533.0, 240.0);
        for scheme in Scheme::ALL {
            let l = Layout::new(vw, vh, scheme);
            let share = |k: Key| l.button(k).unwrap().center.x / vw;
            let mut places = vec![
                (Key::Debug, 0.42),
                (Key::Chat, 0.46),
                (Key::Pause, 0.50),
                (Key::Camera, 0.58),
            ];
            #[cfg(feature = "module_panel")]
            places.extend([(Key::Mods, 0.54), (Key::Command, 0.38)]);
            #[cfg(not(feature = "module_panel"))]
            places.push((Key::Command, 0.54));
            for (key, x) in places {
                assert!((share(key) - x).abs() < 1e-4, "{key:?} at {}", share(key));
                assert!((l.button(key).unwrap().center.y - TOP_Y * vh).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn hotbar_neighbours_clear_the_hotbar() {
        for l in layouts() {
            let (x, _, w, _) = HotbarGeom::new(l.vw, l.vh).slot_rect(8);
            for key in [Key::Inventory, Key::Offhand, Key::Drop] {
                let sq = l.button(key).unwrap();
                assert!(
                    sq.center.x - sq.side / 2.0 >= x + w + HOTBAR_GAP - 1e-3,
                    "{key:?} at {}",
                    l.vw
                );
            }
        }
        let l = Layout::new(533.0, 240.0, Scheme::Joystick);
        assert_eq!(l.hotbar_shift, 0.0);
        assert_eq!(l.button(Key::Inventory).unwrap().center.x, 0.73 * 533.0);
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
