use azalea_core::position::BlockPos;

use super::lanes::{self, ARRIVE, Axis, Cell};

pub(super) const PICKUP: f64 = 0.3 + 1.0 + 0.125;

const ACROSS: f64 = PICKUP - ARRIVE;

pub(super) const Y_BAND: i32 = 1;

pub(super) const MIN_BAND: usize = 3;

pub(super) const DETOUR: f64 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Item {
    pub(super) id: i32,
    pub(super) pos: [f64; 3],
    pub(super) block: BlockPos,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Pass {
    pub(super) from: BlockPos,
    pub(super) to: BlockPos,
    pub(super) items: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Waypoint {
    pub(super) at: BlockPos,
    pub(super) ids: Vec<i32>,
}

#[derive(Debug, Default)]
pub(super) struct Cleanup {
    pub(super) passes: Vec<Pass>,
    pub(super) off_floor: usize,
    pub(super) wet: usize,
    pub(super) no_line: usize,
    pub(super) strays_kept: usize,
    pub(super) strays_left: usize,
}

fn cross_of(axis: Axis, p: [f64; 3]) -> f64 {
    match axis {
        Axis::X => p[2],
        Axis::Z => p[0],
    }
}

fn along_of(axis: Axis, p: [f64; 3]) -> f64 {
    match axis {
        Axis::X => p[0],
        Axis::Z => p[2],
    }
}

fn centre(b: BlockPos) -> [f64; 3] {
    [f64::from(b.x) + 0.5, f64::from(b.y), f64::from(b.z) + 0.5]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn segment_d2(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
    let len2 = dx * dx + dz * dz;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[2] - a[2]) * dz) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - dx * t).powi(2) + (p[2] - a[2] - dz * t).powi(2)
}

fn standable(c: Cell) -> bool {
    matches!(c, Cell::Free | Cell::Crop)
}

pub(super) fn on_floor(y: i32, floors: &[i32]) -> bool {
    floors.iter().any(|f| (y - f).abs() <= Y_BAND)
}

fn end_cell(
    axis: Axis,
    cross: i32,
    y: i32,
    a: i32,
    dir: i32,
    cell: &impl Fn(BlockPos) -> Cell,
) -> BlockPos {
    [a, a + dir, a - dir]
        .into_iter()
        .map(|a| axis.at(a, cross, y))
        .find(|p| standable(cell(*p)))
        .unwrap_or(axis.at(a, cross, y))
}

pub(super) fn bands(
    items: &[Item],
    axis: Axis,
    cell: &impl Fn(BlockPos) -> Cell,
) -> (Vec<Pass>, Vec<Item>, usize) {
    let mut sorted = items.to_vec();
    sorted.sort_by(|a, b| {
        a.block
            .y
            .cmp(&b.block.y)
            .then(cross_of(axis, a.pos).total_cmp(&cross_of(axis, b.pos)))
    });
    let (mut passes, mut strays, mut no_line) = (Vec::new(), Vec::new(), 0);
    let mut i = 0;
    while i < sorted.len() {
        let y = sorted[i].block.y;
        let c0 = cross_of(axis, sorted[i].pos);
        let hi = (c0 + ACROSS - 0.5).floor() as i32;
        let lo = (c0 - ACROSS - 0.5).ceil() as i32;
        let mut found = None;
        for line in (lo..=hi).rev() {
            let top = f64::from(line) + 0.5 + ACROSS;
            let mut j = i;
            while j < sorted.len() && sorted[j].block.y == y && cross_of(axis, sorted[j].pos) <= top
            {
                j += 1;
            }
            let along = sorted[i..j].iter().map(|m| along_of(axis, m.pos));
            let min = along.clone().fold(f64::INFINITY, f64::min).floor() as i32;
            let max = along.fold(f64::NEG_INFINITY, f64::max).floor() as i32;
            if lanes::line(axis, line, y, min, max, cell).is_none() {
                continue;
            }
            let pass = Pass {
                from: end_cell(axis, line, y, min, -1, cell),
                to: end_cell(axis, line, y, max, 1, cell),
                items: sorted[i..j].iter().map(|m| m.id).collect(),
            };
            found = Some((j, pass));
            break;
        }
        match found {
            Some((j, pass)) => {
                if pass.items.len() >= MIN_BAND {
                    passes.push(pass);
                } else {
                    strays.extend_from_slice(&sorted[i..j]);
                }
                i = j;
            }
            None => {
                no_line += 1;
                i += 1;
            }
        }
    }
    (passes, strays, no_line)
}

fn walk_length(passes: &[Pass], order: &[(usize, bool)], start: BlockPos, home: BlockPos) -> f64 {
    let mut at = centre(start);
    let mut total = 0.0;
    for &(i, forward) in order {
        let (a, b) = if forward {
            (passes[i].from, passes[i].to)
        } else {
            (passes[i].to, passes[i].from)
        };
        total += distance(at, centre(a)) + distance(centre(a), centre(b));
        at = centre(b);
    }
    total + distance(at, centre(home))
}

fn order_passes(passes: Vec<Pass>, start: BlockPos, home: BlockPos) -> Vec<Pass> {
    let ends: Vec<[BlockPos; 2]> = passes.iter().map(|p| [p.from, p.to]).collect();
    let out = lanes::order_ends(&ends, start);
    let back: Vec<(usize, bool)> = lanes::order_ends(&ends, home)
        .into_iter()
        .rev()
        .map(|(i, f)| (i, !f))
        .collect();
    let order =
        if walk_length(&passes, &back, start, home) < walk_length(&passes, &out, start, home) {
            back
        } else {
            out
        };
    let mut slots: Vec<Option<Pass>> = passes.into_iter().map(Some).collect();
    order
        .into_iter()
        .filter_map(|(i, forward)| {
            let p = slots[i].take()?;
            Some(if forward {
                p
            } else {
                Pass {
                    from: p.to,
                    to: p.from,
                    items: p.items,
                }
            })
        })
        .collect()
}

pub(super) fn plan(
    items: &[Item],
    axis: Axis,
    floors: &[i32],
    start: BlockPos,
    home: BlockPos,
    cell: impl Fn(BlockPos) -> Cell,
) -> Cleanup {
    let mut out = Cleanup::default();
    let mut dry = Vec::with_capacity(items.len());
    for it in items {
        if !on_floor(it.block.y, floors) {
            out.off_floor += 1;
        } else if cell(it.block) == Cell::Water {
            out.wet += 1;
        } else {
            dry.push(*it);
        }
    }
    let (mut passes, strays, no_line) = bands(&dry, axis, &cell);
    out.no_line = no_line;
    let mut lines: Vec<[[f64; 3]; 2]> = passes
        .iter()
        .map(|p| [centre(p.from), centre(p.to)])
        .collect();
    lines.push([centre(start), centre(home)]);
    for s in strays {
        let near = lines
            .iter()
            .any(|[a, b]| segment_d2(s.pos, *a, *b) <= DETOUR * DETOUR);
        if near && standable(cell(s.block)) {
            passes.push(Pass {
                from: s.block,
                to: s.block,
                items: vec![s.id],
            });
            out.strays_kept += 1;
        } else {
            out.strays_left += 1;
        }
    }
    out.passes = order_passes(passes, start, home);
    out
}

pub(super) fn waypoints(passes: &[Pass], start: BlockPos) -> Vec<Waypoint> {
    let mut out = Vec::with_capacity(passes.len() * 2);
    let mut at = start;
    for p in passes {
        if p.from != at && p.from != p.to {
            out.push(Waypoint {
                at: p.from,
                ids: Vec::new(),
            });
        }
        out.push(Waypoint {
            at: p.to,
            ids: p.items.clone(),
        });
        at = p.to;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i32, x: f64, z: f64) -> Item {
        let pos = [x, 100.0, z];
        Item {
            id,
            pos,
            block: BlockPos::new(x.floor() as i32, 100, z.floor() as i32),
        }
    }

    fn dry(_: BlockPos) -> Cell {
        Cell::Free
    }

    #[test]
    fn the_pickup_box_follows_the_reference() {
        assert!((PICKUP - 1.425).abs() < 1e-9);
        assert!((ACROSS - 1.125).abs() < 1e-9);
    }

    #[test]
    fn interval_cover_uses_the_fewest_bands() {
        let zs = [0.2, 0.9, 1.6, 2.3, 2.4, 3.0, 4.1, 5.0, 5.5, 6.2];
        let items: Vec<Item> = zs
            .iter()
            .enumerate()
            .map(|(i, &z)| item(i as i32, 2.5 + i as f64, z))
            .collect();
        let (passes, strays, no_line) = bands(&items, Axis::X, &dry);
        let lines: Vec<(i32, usize)> = passes.iter().map(|p| (p.from.z, p.items.len())).collect();
        assert_eq!(lines, vec![(0, 3), (2, 3), (4, 3)]);
        assert_eq!((strays.len(), no_line), (1, 0));
        assert_eq!((passes[0].from.x, passes[0].to.x), (2, 4));
    }

    #[test]
    fn a_band_moves_off_water() {
        let water_at_z0 = |p: BlockPos| if p.z == 0 { Cell::Water } else { Cell::Free };
        let all_water = |_: BlockPos| Cell::Water;
        let four: Vec<Item> = (0..4)
            .map(|i| item(i, 2.5 + f64::from(i), 0.3 + 0.4 * f64::from(i)))
            .collect();
        let (passes, strays, no_line) = bands(&four, Axis::X, &all_water);
        assert_eq!((passes.len(), strays.len(), no_line), (0, 0, 4));
        let more: Vec<Item> = (0..6)
            .map(|i| item(i, 2.5 + f64::from(i), 1.2 + 0.1 * f64::from(i)))
            .collect();
        let (passes, _, _) = bands(&more, Axis::X, &water_at_z0);
        assert_eq!(
            passes
                .iter()
                .map(|p| (p.from.z, p.items.len()))
                .collect::<Vec<_>>(),
            vec![(1, 6)]
        );
        let water_at_z1 = |p: BlockPos| if p.z == 1 { Cell::Water } else { Cell::Free };
        let (passes, _, _) = bands(&more, Axis::X, &water_at_z1);
        assert_eq!(
            passes
                .iter()
                .map(|p| (p.from.z, p.items.len()))
                .collect::<Vec<_>>(),
            vec![(0, 5)]
        );
    }

    #[test]
    fn strays_far_from_every_line_are_left() {
        let mut items: Vec<Item> = (0..5)
            .map(|i| item(i, 1.5 + 3.0 * f64::from(i), 0.5))
            .collect();
        items.push(item(10, 6.5, 3.4));
        items.push(item(11, 6.5, 30.5));
        let floors = [100];
        let start = BlockPos::new(0, 100, -10);
        let home = BlockPos::new(0, 100, -20);
        let plan = plan(&items, Axis::X, &floors, start, home, dry);
        assert_eq!((plan.strays_kept, plan.strays_left), (1, 1));
        assert_eq!(plan.passes.len(), 2);
        assert!(plan.passes.iter().any(|p| p.items == vec![10]));
        assert!(!plan.passes.iter().any(|p| p.items.contains(&11)));
    }

    #[test]
    fn items_off_the_floor_or_in_water_are_left() {
        let mut items: Vec<Item> = (0..4).map(|i| item(i, 1.5 + f64::from(i), 0.5)).collect();
        items.push(Item {
            id: 20,
            pos: [2.5, 89.0, 0.5],
            block: BlockPos::new(2, 89, 0),
        });
        items.push(Item {
            id: 21,
            pos: [2.5, 101.0, 0.5],
            block: BlockPos::new(2, 101, 0),
        });
        items.push(item(22, 2.5, 5.5));
        let cell = |p: BlockPos| if p.z == 5 { Cell::Water } else { Cell::Free };
        let plan = plan(
            &items,
            Axis::X,
            &[100],
            BlockPos::new(0, 100, 0),
            BlockPos::new(0, 100, 0),
            cell,
        );
        assert_eq!((plan.off_floor, plan.wet), (1, 1));
        assert!(on_floor(101, &[100]) && on_floor(99, &[100]) && !on_floor(98, &[100]));
        assert!(
            plan.passes
                .iter()
                .all(|p| p.from.y == 100 || p.from.y == 101)
        );
        assert!(
            !plan
                .passes
                .iter()
                .any(|p| p.items.contains(&20) || p.items.contains(&22))
        );
    }

    #[test]
    fn passes_are_ordered_and_routed_end_to_end() {
        let pass = |z: i32, ids: Vec<i32>| Pass {
            from: BlockPos::new(0, 100, z),
            to: BlockPos::new(16, 100, z),
            items: ids,
        };
        let passes = vec![pass(0, vec![1, 2, 3]), pass(4, vec![4, 5, 6])];
        let start = BlockPos::new(17, 100, 5);
        let home = BlockPos::new(17, 100, -2);
        let ordered = order_passes(passes, start, home);
        assert_eq!(
            ordered
                .iter()
                .map(|p| (p.from.z, p.from.x, p.to.x))
                .collect::<Vec<_>>(),
            vec![(4, 16, 0), (0, 0, 16)]
        );
        let w = waypoints(&ordered, start);
        assert_eq!(
            w.iter()
                .map(|w| (w.at.x, w.at.z, w.ids.len()))
                .collect::<Vec<_>>(),
            vec![(16, 4, 0), (0, 4, 3), (0, 0, 0), (16, 0, 3)]
        );
    }
}
