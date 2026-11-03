use azalea::block::BlockState;
use azalea::physics::collision::BlockWithShape;
use azalea_core::direction::Direction;
use azalea_core::position::BlockPos;
use azalea_registry::builtin::BlockKind;

pub(super) const HORIZONTAL: [Direction; 4] = [
    Direction::North,
    Direction::South,
    Direction::East,
    Direction::West,
];

const ALL: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub(super) trait Cells {
    fn state(&self, p: BlockPos) -> Option<BlockState>;
    fn item(&self) -> BlockKind;

    fn solid(&self, p: BlockPos) -> bool {
        self.state(p).is_some_and(|s| {
            s.is_collision_shape_full() && !crate::play::placement::has_use_interaction(s)
        })
    }

    fn replaceable(&self, p: BlockPos) -> bool {
        self.state(p)
            .is_some_and(|s| crate::play::placement::replaceable(s, self.item()) == Some(true))
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Me {
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub on_ground: bool,
    pub eye_height: f64,
    pub forward: f32,
    pub strafe: f32,
    pub yaw: f32,
    pub jump: bool,
}

impl Me {
    pub fn eye(&self) -> [f64; 3] {
        [self.pos[0], self.pos[1] + self.eye_height, self.pos[2]]
    }

    pub fn column(&self) -> (i32, i32) {
        (self.pos[0].floor() as i32, self.pos[2].floor() as i32)
    }

    pub fn moving(&self) -> bool {
        self.forward != 0.0 || self.strafe != 0.0
    }

    pub fn walk_dir(&self) -> Option<(f64, f64)> {
        if !self.moving() {
            return None;
        }
        let r = (self.yaw as f64).to_radians();
        let (s, f) = (self.strafe as f64, self.forward as f64);
        let x = s * r.cos() - f * r.sin();
        let z = f * r.cos() + s * r.sin();
        let len = (x * x + z * z).sqrt();
        (len >= 0.01).then(|| (x / len, z / len))
    }
}

pub(super) fn step(p: BlockPos, d: Direction) -> BlockPos {
    p.offset_with_direction(d)
}

fn has_support_neighbor(c: &impl Cells, p: BlockPos) -> bool {
    ALL.iter().any(|&d| c.solid(step(p, d)))
}

pub(super) fn has_h_support(c: &impl Cells, p: BlockPos) -> bool {
    HORIZONTAL.iter().any(|&d| c.solid(step(p, d)))
}

pub(super) fn face_visible(pos: BlockPos, neighbor: BlockPos, x: f64, z: f64) -> bool {
    if neighbor.z == pos.z - 1 {
        return z > (neighbor.z + 1) as f64;
    }
    if neighbor.z == pos.z + 1 {
        return z < neighbor.z as f64;
    }
    if neighbor.x == pos.x + 1 {
        return x < neighbor.x as f64;
    }
    if neighbor.x == pos.x - 1 {
        return x > (neighbor.x + 1) as f64;
    }
    true
}

pub(super) fn visible_h_support(c: &impl Cells, p: BlockPos, x: f64, z: f64) -> bool {
    HORIZONTAL.iter().any(|&d| {
        let n = step(p, d);
        c.solid(n) && face_visible(p, n, x, z)
    })
}

pub(super) fn place_position(
    c: &impl Cells,
    me: &Me,
    airborne: bool,
    y: f64,
    expand: f64,
) -> Option<BlockPos> {
    let [px, py, pz] = me.pos;
    let feet = if airborne { y.floor() } else { py.floor() } as i32;
    let below_y = feet - 1;
    let (bx, bz) = me.column();
    let below = BlockPos::new(bx, below_y, bz);

    if !me.on_ground {
        if c.replaceable(below) && has_support_neighbor(c, below) {
            return Some(below);
        }
        let at_feet = BlockPos::new(bx, below_y + 1, bz);
        if py < below_y as f64 + 1.05 && c.replaceable(at_feet) && has_support_neighbor(c, at_feet)
        {
            return Some(at_feet);
        }
        if let Some(cell) = knockback_catch(c, px, pz, below_y) {
            return Some(cell);
        }
    }

    if let Some(cell) = place_in_move_dir(c, me, below_y)
        && (me.on_ground || has_h_support(c, cell))
    {
        return Some(cell);
    }

    if c.replaceable(below) && has_support_neighbor(c, below) {
        return Some(below);
    }

    let fx = px - px.floor();
    let fz = pz - pz.floor();
    let west = fx < expand;
    let east = fx > 1.0 - expand;
    let north = fz < expand;
    let south = fz > 1.0 - expand;
    let off = |dx: i32, dz: i32| BlockPos::new(bx + dx, below_y, bz + dz);
    let candidates = [
        west.then(|| off(-1, 0)),
        east.then(|| off(1, 0)),
        north.then(|| off(0, -1)),
        south.then(|| off(0, 1)),
        (west && north).then(|| off(-1, -1)),
        (east && north).then(|| off(1, -1)),
        (west && south).then(|| off(-1, 1)),
        (east && south).then(|| off(1, 1)),
    ];
    for cell in candidates.into_iter().flatten() {
        if c.replaceable(cell) && has_support_neighbor(c, cell) && c.solid(cell.down(1)) {
            return Some(cell);
        }
    }

    let (dx, dz) = me.walk_dir()?;
    for d in [1.0, 2.0] {
        let ahead = BlockPos::new(
            (px + dx * d).floor() as i32,
            below_y,
            (pz + dz * d).floor() as i32,
        );
        if c.replaceable(ahead) && (me.on_ground || has_h_support(c, ahead)) {
            return Some(ahead);
        }
    }
    None
}

pub(super) fn telly_position(c: &impl Cells, me: &Me, y: f64) -> Option<BlockPos> {
    let below_y = y.floor() as i32 - 1;
    let (bx, bz) = me.column();
    let below = BlockPos::new(bx, below_y, bz);
    if c.replaceable(below) && has_support_neighbor(c, below) {
        return Some(below);
    }
    place_in_move_dir(c, me, below_y).filter(|&p| has_h_support(c, p))
}

pub(super) fn knockback_catch(c: &impl Cells, x: f64, z: f64, y: i32) -> Option<BlockPos> {
    let (bx, bz) = (x.floor() as i32, z.floor() as i32);
    for supported_pass in [true, false] {
        let mut best: Option<(f64, BlockPos)> = None;
        for dx in -6..=6 {
            for dz in -6..=6 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let cell = BlockPos::new(bx + dx, y, bz + dz);
                if !c.replaceable(cell) {
                    continue;
                }
                let supported = c.solid(cell.down(1)) || visible_h_support(c, cell, x, z);
                let placeable = supported || c.solid(cell.up(1));
                if supported == supported_pass && placeable {
                    let (ox, oz) = (cell.x as f64 + 0.5 - x, cell.z as f64 + 0.5 - z);
                    let dist = ox * ox + oz * oz;
                    if best.is_none_or(|(b, _)| dist < b) {
                        best = Some((dist, cell));
                    }
                }
            }
        }
        if let Some((_, cell)) = best {
            return Some(cell);
        }
    }
    None
}

pub(super) fn place_in_move_dir(c: &impl Cells, me: &Me, y: i32) -> Option<BlockPos> {
    let (dx, dz) = me.walk_dir()?;
    let [px, _, pz] = me.pos;
    let ahead = BlockPos::new((px + dx).floor() as i32, y, (pz + dz).floor() as i32);
    if c.replaceable(ahead) && has_support_neighbor(c, ahead) {
        return Some(ahead);
    }
    if !(dx.abs() > 0.1 && dz.abs() > 0.1) {
        return None;
    }
    let (bx, bz) = me.column();
    let (sx, sz) = (if dx > 0.0 { 1 } else { -1 }, if dz > 0.0 { 1 } else { -1 });
    let side_z = BlockPos::new(bx, y, bz + sz);
    let side_x = BlockPos::new(bx + sx, y, bz);
    let dist_x = if dx > 0.0 {
        (bx + 1) as f64 - px
    } else {
        px - bx as f64
    };
    let dist_z = if dz > 0.0 {
        (bz + 1) as f64 - pz
    } else {
        pz - bz as f64
    };
    let (first, second) = if dist_x > dist_z {
        (side_z, side_x)
    } else {
        (side_x, side_z)
    };
    let open = |p: BlockPos| c.replaceable(p) && has_support_neighbor(c, p);
    for p in [first, second] {
        if open(p) && c.solid(p.down(1)) {
            return Some(p);
        }
    }
    for p in [first, second] {
        if open(p) {
            return Some(p);
        }
    }
    let (fx, fz) = (px - bx as f64, pz - bz as f64);
    let back_z = BlockPos::new(bx, y, bz - sz);
    let back_x = BlockPos::new(bx - sx, y, bz);
    let (b1, b2) = if fx < fz {
        (back_x, back_z)
    } else {
        (back_z, back_x)
    };
    [b1, b2].into_iter().find(|&p| open(p))
}

fn direction_between(from: BlockPos, to: BlockPos) -> Option<Direction> {
    match (to.x - from.x, to.y - from.y, to.z - from.z) {
        (1, 0, 0) => Some(Direction::East),
        (-1, 0, 0) => Some(Direction::West),
        (0, 1, 0) => Some(Direction::Up),
        (0, -1, 0) => Some(Direction::Down),
        (0, 0, 1) => Some(Direction::South),
        (0, 0, -1) => Some(Direction::North),
        _ => None,
    }
}

fn inset(lo: i32, v: f64) -> f64 {
    v.clamp(lo as f64 + 0.2, lo as f64 + 0.8)
}

pub(super) fn edge_aim(
    c: &impl Cells,
    me: &Me,
    pos: BlockPos,
    airborne: bool,
    y: f64,
    last_support: Option<BlockPos>,
    reversing: bool,
) -> ([f64; 3], Option<BlockPos>) {
    let [px, py, pz] = me.pos;
    let feet = if airborne { y.floor() } else { py.floor() } as i32;
    let below_y = feet - 1;
    let (bx, bz) = me.column();
    let below = BlockPos::new(bx, below_y, bz);
    let centre = |p: BlockPos| [p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5];

    if !c.solid(pos.down(1)) {
        let mut support: Option<(Direction, BlockPos)> = None;
        if !reversing {
            if let Some(last) = last_support {
                support = HORIZONTAL
                    .iter()
                    .map(|&d| (d, step(pos, d)))
                    .find(|&(_, n)| n == last && c.solid(n) && face_visible(pos, n, px, pz));
            }
            if support.is_none() {
                let mut best = -1.0;
                for &d in &HORIZONTAL {
                    let n = step(pos, d);
                    if c.solid(n) && face_visible(pos, n, px, pz) {
                        let (ox, oz) = (n.x as f64 + 0.5 - px, n.z as f64 + 0.5 - pz);
                        let dist = ox * ox + oz * oz;
                        if dist > best {
                            best = dist;
                            support = Some((d, n));
                        }
                    }
                }
            }
        }
        if let Some((d, s)) = support {
            let mut a = [s.x as f64 + 0.5, s.y as f64 + 0.7, s.z as f64 + 0.5];
            match d.opposite() {
                Direction::North => a[2] = s.z as f64,
                Direction::South => a[2] = (s.z + 1) as f64,
                Direction::East => a[0] = (s.x + 1) as f64,
                Direction::West => a[0] = s.x as f64,
                _ => {}
            }
            if matches!(d, Direction::East | Direction::West) {
                a[2] = inset(s.z, pz);
            } else {
                a[0] = inset(s.x, px);
            }
            return (a, Some(s));
        }
        if c.solid(pos.up(1)) {
            return (
                [pos.x as f64 + 0.5, (pos.y + 1) as f64, pos.z as f64 + 0.5],
                None,
            );
        }
        if let Some(last) = last_support
            && c.solid(last)
        {
            let (ox, oz) = (px - (last.x as f64 + 0.5), pz - (last.z as f64 + 0.5));
            let ay = pos.y as f64 + 0.7;
            if ox.abs() >= oz.abs() {
                if ox > 0.0 && px > (last.x + 1) as f64 {
                    return ([(last.x + 1) as f64, ay, inset(last.z, pz)], Some(last));
                }
                if ox < 0.0 && px < last.x as f64 {
                    return ([last.x as f64, ay, inset(last.z, pz)], Some(last));
                }
            } else {
                if oz > 0.0 && pz > (last.z + 1) as f64 {
                    return ([inset(last.x, px), ay, (last.z + 1) as f64], Some(last));
                }
                if oz < 0.0 && pz < last.z as f64 {
                    return ([inset(last.x, px), ay, last.z as f64], Some(last));
                }
            }
        }
    }

    if below == pos {
        return (centre(pos), None);
    }
    let base = pos.down(1);
    if pos.y == below_y + 1 && c.solid(base) {
        return (
            [
                base.x as f64 + 0.5,
                (base.y + 1) as f64,
                base.z as f64 + 0.5,
            ],
            None,
        );
    }
    let Some(dir) = direction_between(below, pos) else {
        return (centre(pos), None);
    };
    if !c.solid(base) {
        return (centre(pos), None);
    }

    let eye_y = py + me.eye_height;
    let below_top = (below.y + 1) as f64;
    let base_top = (base.y + 1) as f64;
    let t = (eye_y - below_top) / (eye_y - base_top);
    let mut a = [base.x as f64 + 0.5, base_top, base.z as f64 + 0.5];
    let over = |edge: f64, p: f64| (edge - (1.0 - t) * p) / t;
    match dir {
        Direction::North => a[2] = inset(base.z, a[2].min(over(below.z as f64 - 0.01, pz))),
        Direction::South => a[2] = inset(base.z, a[2].max(over((below.z + 1) as f64 + 0.01, pz))),
        Direction::East => a[0] = inset(base.x, a[0].max(over((below.x + 1) as f64 + 0.01, px))),
        Direction::West => a[0] = inset(base.x, a[0].min(over(below.x as f64 - 0.01, px))),
        _ => {}
    }
    (a, None)
}

pub(super) fn is_scaffold_block(kind: BlockKind) -> bool {
    if matches!(
        kind,
        BlockKind::Air
            | BlockKind::Tnt
            | BlockKind::Sand
            | BlockKind::RedSand
            | BlockKind::Gravel
            | BlockKind::SuspiciousSand
            | BlockKind::SuspiciousGravel
    ) {
        return false;
    }
    if kind.to_str().ends_with("_concrete_powder") {
        return false;
    }
    BlockState::from(kind).is_collision_shape_full()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct Grid(HashSet<(i32, i32, i32)>);

    impl Cells for Grid {
        fn state(&self, p: BlockPos) -> Option<BlockState> {
            Some(if self.0.contains(&(p.x, p.y, p.z)) {
                BlockState::from(BlockKind::Stone)
            } else {
                BlockState::AIR
            })
        }
        fn item(&self) -> BlockKind {
            BlockKind::Stone
        }
    }

    fn me(pos: [f64; 3], forward: f32) -> Me {
        Me {
            pos,
            vel: [0.0; 3],
            on_ground: true,
            eye_height: 1.62,
            forward,
            strafe: 0.0,
            yaw: 0.0,
            jump: false,
        }
    }

    #[test]
    fn walking_off_the_end_fills_the_cell_ahead() {
        let grid = Grid((-5..=0).map(|z| (0, 63, z)).collect());
        let p = me([0.5, 64.0, 0.8], 1.0);
        assert_eq!(
            place_position(&grid, &p, false, 64.0, 0.0),
            Some(BlockPos::new(0, 63, 1))
        );
    }

    #[test]
    fn expand_zero_offers_no_side_cells() {
        let mut set: HashSet<_> = [(0, 63, 0)].into();
        for (x, z) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            set.insert((x, 62, z));
        }
        let grid = Grid(set);
        let p = me([0.5, 64.0, 0.5], 0.0);
        assert_eq!(place_position(&grid, &p, false, 64.0, 0.0), None);
        let p = me([0.2, 64.0, 0.5], 0.0);
        assert_eq!(
            place_position(&grid, &p, false, 64.0, 0.3),
            Some(BlockPos::new(-1, 63, 0))
        );
    }

    #[test]
    fn a_cell_with_nothing_to_click_is_not_chosen() {
        let grid = Grid(HashSet::new());
        let mut p = me([0.5, 70.0, 0.5], 0.0);
        p.on_ground = false;
        assert_eq!(place_position(&grid, &p, false, 70.0, 0.0), None);
    }

    #[test]
    fn the_side_aim_is_on_the_supports_face() {
        let grid = Grid([(0, 63, 0)].into());
        let p = me([0.5, 64.0, 1.4], 1.0);
        let (aim, support) = edge_aim(&grid, &p, BlockPos::new(0, 63, 1), false, 64.0, None, false);
        assert_eq!(support, Some(BlockPos::new(0, 63, 0)));
        assert!((aim[2] - 1.0).abs() < 1e-9, "{aim:?}");
        assert!((aim[1] - 63.7).abs() < 1e-9, "{aim:?}");
    }

    #[test]
    fn falling_and_explosive_blocks_are_not_scaffold_blocks() {
        assert!(is_scaffold_block(BlockKind::Stone));
        assert!(is_scaffold_block(BlockKind::OakPlanks));
        assert!(!is_scaffold_block(BlockKind::Sand));
        assert!(!is_scaffold_block(BlockKind::RedConcretePowder));
        assert!(!is_scaffold_block(BlockKind::Tnt));
        assert!(!is_scaffold_block(BlockKind::OakSlab));
        assert!(!is_scaffold_block(BlockKind::Cobweb));
    }
}
