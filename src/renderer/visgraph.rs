pub use crate::direction::Direction;
use crate::direction::Faces;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct VisibilitySet(u64);

impl VisibilitySet {
    pub fn none() -> Self {
        VisibilitySet(0)
    }

    pub fn all() -> Self {
        VisibilitySet((1u64 << 36) - 1)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn bits(self) -> u64 {
        self.0
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_bits(bits: u64) -> Self {
        VisibilitySet(bits & ((1u64 << 36) - 1))
    }

    #[inline]
    fn bit(a: Direction, b: Direction) -> u64 {
        1u64 << (a.index() + b.index() * 6)
    }

    pub fn visible_between(self, a: Direction, b: Direction) -> bool {
        self.0 & Self::bit(a, b) != 0
    }

    #[allow(
        dead_code,
        reason = "the camera's own section takes this branch once occlusion walks from inside it"
    )]
    pub fn any_to(self, to: Direction) -> bool {
        Direction::ALL
            .iter()
            .any(|&from| self.visible_between(from, to))
    }

    fn add(&mut self, faces: Faces) {
        for a in Direction::ALL.into_iter().filter(|&d| faces.contains(d)) {
            for b in Direction::ALL.into_iter().filter(|&d| faces.contains(d)) {
                self.0 |= Self::bit(a, b);
            }
        }
    }
}

#[inline]
fn cell_index(x: u8, y: u8, z: u8) -> u16 {
    x as u16 | (z as u16) << 4 | (y as u16) << 8
}

const DX: i32 = 1;
const DZ: i32 = 16;
const DY: i32 = 256;

struct CellBits([u64; 64]);

impl CellBits {
    fn new() -> Self {
        CellBits([0; 64])
    }

    #[inline]
    fn get(&self, i: u16) -> bool {
        self.0[(i >> 6) as usize] & (1u64 << (i & 63)) != 0
    }

    #[inline]
    fn set(&mut self, i: u16) {
        self.0[(i >> 6) as usize] |= 1u64 << (i & 63);
    }
}

fn push_edges(index: u16, faces: &mut Faces) {
    let x = index & 15;
    if x == 0 {
        faces.insert(Direction::West);
    } else if x == 15 {
        faces.insert(Direction::East);
    }

    let y = (index >> 8) & 15;
    if y == 0 {
        faces.insert(Direction::Down);
    } else if y == 15 {
        faces.insert(Direction::Up);
    }

    let z = (index >> 4) & 15;
    if z == 0 {
        faces.insert(Direction::North);
    } else if z == 15 {
        faces.insert(Direction::South);
    }
}

fn neighbor(index: u16, face: Direction) -> Option<u16> {
    let index = index as i32;
    let out = match face {
        Direction::Down => {
            if (index >> 8) & 15 == 0 {
                return None;
            }
            index - DY
        }
        Direction::Up => {
            if (index >> 8) & 15 == 15 {
                return None;
            }
            index + DY
        }
        Direction::North => {
            if (index >> 4) & 15 == 0 {
                return None;
            }
            index - DZ
        }
        Direction::South => {
            if (index >> 4) & 15 == 15 {
                return None;
            }
            index + DZ
        }
        Direction::West => {
            if index & 15 == 0 {
                return None;
            }
            index - DX
        }
        Direction::East => {
            if index & 15 == 15 {
                return None;
            }
            index + DX
        }
    };
    Some(out as u16)
}

pub fn resolve<F: Fn(u8, u8, u8) -> bool>(is_opaque: F) -> VisibilitySet {
    let mut bits = CellBits::new();
    let mut empty: u32 = 4096;

    for y in 0..16u8 {
        for z in 0..16u8 {
            for x in 0..16u8 {
                if is_opaque(x, y, z) {
                    bits.set(cell_index(x, y, z));
                    empty -= 1;
                }
            }
        }
    }

    let mut result = VisibilitySet::none();

    if 4096 - empty < 256 {
        return VisibilitySet::all();
    }
    if empty == 0 {
        return VisibilitySet::none();
    }

    let mut queue: Vec<u16> = Vec::with_capacity(4096);

    for x in 0..16u8 {
        for y in 0..16u8 {
            for z in 0..16u8 {
                if !(x == 0 || x == 15 || y == 0 || y == 15 || z == 0 || z == 15) {
                    continue;
                }
                let start = cell_index(x, y, z);
                if bits.get(start) {
                    continue;
                }

                let mut faces = Faces(0);
                queue.clear();
                queue.push(start);
                bits.set(start);

                let mut head = 0usize;
                while head < queue.len() {
                    let index = queue[head];
                    head += 1;
                    push_edges(index, &mut faces);
                    for &face in Direction::ALL.iter() {
                        if let Some(n) = neighbor(index, face) {
                            if !bits.get(n) {
                                bits.set(n);
                                queue.push(n);
                            }
                        }
                    }
                }

                result.add(faces);
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_faces() -> Vec<(Direction, Direction)> {
        let mut pairs = Vec::new();
        for &a in Direction::ALL.iter() {
            for &b in Direction::ALL.iter() {
                pairs.push((a, b));
            }
        }
        pairs
    }

    #[test]
    fn all_air_connects_everything() {
        let set = resolve(|_, _, _| false);
        for (a, b) in all_faces() {
            assert!(set.visible_between(a, b), "{:?} <-> {:?}", a, b);
        }
    }

    #[test]
    fn fully_opaque_connects_nothing() {
        let set = resolve(|_, _, _| true);
        for (a, b) in all_faces() {
            assert!(!set.visible_between(a, b), "{:?} <-> {:?}", a, b);
        }
    }

    #[test]
    fn straight_tunnel_along_x_connects_only_west_east() {
        let set = resolve(|_, y, z| !(y == 8 && z == 8));
        assert!(set.visible_between(Direction::West, Direction::East));
        assert!(!set.visible_between(Direction::Up, Direction::Down));
        assert!(!set.visible_between(Direction::North, Direction::South));
        assert!(!set.visible_between(Direction::West, Direction::Up));
    }

    #[test]
    fn straight_tunnel_along_y_connects_only_up_down() {
        let set = resolve(|x, _, z| !(x == 8 && z == 8));
        assert!(set.visible_between(Direction::Up, Direction::Down));
        assert!(!set.visible_between(Direction::West, Direction::East));
        assert!(!set.visible_between(Direction::North, Direction::South));
    }

    #[test]
    fn l_shaped_tunnel_connects_west_and_up_only() {
        let is_open =
            |x: u8, y: u8, z: u8| (z == 8 && y == 8 && x <= 8) || (z == 8 && x == 8 && y >= 8);
        let set = resolve(|x, y, z| !is_open(x, y, z));
        assert!(set.visible_between(Direction::West, Direction::Up));
        assert!(!set.visible_between(Direction::West, Direction::East));
        assert!(!set.visible_between(Direction::West, Direction::Down));
        assert!(!set.visible_between(Direction::Up, Direction::Down));
    }

    #[test]
    fn few_opaque_cells_shortcut_returns_all() {
        let set = resolve(|x, y, z| x == 8 && !(y == 15 && z == 15));
        for (a, b) in all_faces() {
            assert!(set.visible_between(a, b), "{:?} <-> {:?}", a, b);
        }
    }

    #[test]
    fn visible_between_is_symmetric() {
        let set = resolve(|x, y, z| !(y == 8 && z == 8) && !(x == 8 && z == 8));
        for (a, b) in all_faces() {
            assert_eq!(set.visible_between(a, b), set.visible_between(b, a));
        }
    }
}
