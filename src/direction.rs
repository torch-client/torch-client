#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Direction {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    #[inline]
    pub const fn offset(self) -> (i32, i32, i32) {
        match self {
            Direction::Down => (0, -1, 0),
            Direction::Up => (0, 1, 0),
            Direction::North => (0, 0, -1),
            Direction::South => (0, 0, 1),
            Direction::West => (-1, 0, 0),
            Direction::East => (1, 0, 0),
        }
    }

    #[inline]
    pub const fn normal(self) -> [f32; 3] {
        let (x, y, z) = self.offset();
        [x as f32, y as f32, z as f32]
    }

    #[inline]
    pub const fn opposite(self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(i: u8) -> Direction {
        match i {
            0 => Direction::Down,
            1 => Direction::Up,
            2 => Direction::North,
            3 => Direction::South,
            4 => Direction::West,
            _ => Direction::East,
        }
    }

    #[inline]
    pub const fn to_y_rot(self) -> f32 {
        match self {
            Direction::West => 90.0,
            Direction::North => 180.0,
            Direction::East => 270.0,
            _ => 0.0,
        }
    }

    #[inline]
    pub const fn step_x(self) -> f32 {
        self.offset().0 as f32
    }

    #[allow(dead_code, reason = "completes getStepX/Y/Z; the other two are used")]
    #[inline]
    pub const fn step_y(self) -> f32 {
        self.offset().1 as f32
    }

    #[inline]
    pub const fn step_z(self) -> f32 {
        self.offset().2 as f32
    }

    #[inline]
    pub const fn mirrored(self) -> Direction {
        match self {
            Direction::West => Direction::East,
            Direction::East => Direction::West,
            other => other,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Faces(pub u8);

impl Faces {
    pub const ALL: Faces = Faces(0b11_1111);

    pub const fn of(faces: &[Direction]) -> Faces {
        let mut bits = 0u8;
        let mut i = 0;
        while i < faces.len() {
            bits |= 1 << (faces[i] as u8);
            i += 1;
        }
        Faces(bits)
    }

    pub const fn contains(self, face: Direction) -> bool {
        self.0 & (1 << (face as u8)) != 0
    }

    #[inline]
    pub fn insert(&mut self, face: Direction) {
        self.0 |= 1 << (face as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ordinals_are_mojangs() {
        assert_eq!(Direction::Down.index(), 0);
        assert_eq!(Direction::Up.index(), 1);
        assert_eq!(Direction::North.index(), 2);
        assert_eq!(Direction::South.index(), 3);
        assert_eq!(Direction::West.index(), 4);
        assert_eq!(Direction::East.index(), 5);
        for (i, d) in Direction::ALL.iter().enumerate() {
            assert_eq!(d.index(), i);
            assert_eq!(Direction::from_index(i as u8), *d);
        }
    }

    #[test]
    fn opposites_pair_up_and_offsets_negate() {
        for d in Direction::ALL {
            assert_eq!(d.opposite().opposite(), d);
            let (x, y, z) = d.offset();
            let (ox, oy, oz) = d.opposite().offset();
            assert_eq!((x + ox, y + oy, z + oz), (0, 0, 0));
        }
    }

    #[test]
    fn mirroring_only_flips_x() {
        assert_eq!(Direction::West.mirrored(), Direction::East);
        assert_eq!(Direction::East.mirrored(), Direction::West);
        for d in [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
        ] {
            assert_eq!(d.mirrored(), d);
        }
    }

    #[test]
    fn the_face_set_packs_by_ordinal() {
        let set = Faces::of(&[Direction::Up, Direction::East]);
        assert!(set.contains(Direction::Up));
        assert!(set.contains(Direction::East));
        assert!(!set.contains(Direction::Down));
        for d in Direction::ALL {
            assert!(Faces::ALL.contains(d));
        }
    }
}
