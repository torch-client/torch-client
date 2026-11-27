use std::collections::{HashMap, HashSet};

use azalea_core::position::BlockPos;

use crate::modules::place::Click;
use crate::modules::sent::{Look, ray_hits, view_vector};

pub(super) const EYE_DRIFT: f64 = 0.3;

pub(super) struct Sight {
    feet: BlockPos,
    eye: [f64; 3],
    clicks: HashMap<BlockPos, Vec<Click>>,
    tried: HashSet<BlockPos>,
    air: HashSet<BlockPos>,
}

impl Sight {
    pub(super) fn new(feet: BlockPos) -> Sight {
        Sight {
            feet,
            eye: [f64::NAN; 3],
            clicks: HashMap::new(),
            tried: HashSet::new(),
            air: HashSet::new(),
        }
    }

    pub(super) fn forget(&mut self, p: BlockPos) {
        self.clicks.remove(&p);
        self.tried.remove(&p);
        self.air.remove(&p);
    }

    pub(super) fn reset(&mut self) {
        self.clicks.clear();
        self.tried.clear();
        self.air.clear();
    }

    pub(super) fn is_air(&self, p: BlockPos) -> bool {
        self.air.contains(&p)
    }

    pub(super) fn solved_air(&mut self, p: BlockPos, clicks: Vec<Click>) {
        self.solved(p, clicks);
        if self.clicks.contains_key(&p) {
            self.air.insert(p);
        }
    }

    pub(super) fn refresh(
        &mut self,
        feet: BlockPos,
        eye: [f64; 3],
        foreign: &[BlockPos],
        radius: i32,
    ) -> bool {
        let near = |p: &BlockPos| {
            (p.x - feet.x).abs() <= radius
                && (p.y - feet.y).abs() <= radius
                && (p.z - feet.z).abs() <= radius
        };
        let drift = (0..3)
            .map(|i| (eye[i] - self.eye[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if feet != self.feet || drift.is_nan() {
            self.feet = feet;
            self.eye = eye;
            self.reset();
            return true;
        }
        if drift > EYE_DRIFT {
            self.eye = eye;
            self.clicks.clear();
            self.air.clear();
        }
        for q in foreign.iter().filter(|q| near(q)) {
            self.changed(*q);
        }
        false
    }

    pub(super) fn changed(&mut self, q: BlockPos) {
        self.forget(q);
        for n in neighbours(q) {
            self.forget(n);
        }
        let eye = self.eye;
        let e = [
            eye[0].floor() as i32,
            eye[1].floor() as i32,
            eye[2].floor() as i32,
        ];
        self.tried.retain(|c| {
            let c = [c.x, c.y, c.z];
            let q = [q.x, q.y, q.z];
            !(0..3).all(|i| q[i] >= c[i].min(e[i]) - 1 && q[i] <= c[i].max(e[i]) + 1)
        });
        let air = &self.air;
        self.clicks.retain(|cell, list| {
            if air.contains(cell) {
                return true;
            }
            list.retain(|k| !crosses(eye, point(k), q));
            !list.is_empty()
        });
    }

    pub(super) fn tried(&self, p: BlockPos) -> bool {
        self.tried.contains(&p)
    }

    pub(super) fn known(&self, p: BlockPos) -> bool {
        self.tried.contains(&p) || self.clicks.contains_key(&p)
    }

    pub(super) fn clicks(&self, p: BlockPos) -> Option<&[Click]> {
        self.clicks.get(&p).map(Vec::as_slice)
    }

    pub(super) fn fail(&mut self, p: BlockPos) {
        self.clicks.remove(&p);
        self.air.remove(&p);
        self.tried.insert(p);
    }

    pub(super) fn solved(&mut self, p: BlockPos, clicks: Vec<Click>) {
        self.air.remove(&p);
        if clicks.is_empty() {
            self.fail(p);
        } else {
            self.tried.remove(&p);
            self.clicks.insert(p, clicks);
        }
    }

    pub(super) fn placed(&mut self, p: BlockPos, eye: [f64; 3]) {
        self.clicks.remove(&p);
        self.tried.remove(&p);
        self.air.remove(&p);
        for n in neighbours(p) {
            self.clicks.remove(&n);
            self.tried.remove(&n);
            self.air.remove(&n);
        }
        let air = &self.air;
        self.clicks.retain(|cell, list| {
            if air.contains(cell) {
                return true;
            }
            list.retain(|k| !crosses(eye, point(k), p));
            !list.is_empty()
        });
    }
}

fn neighbours(p: BlockPos) -> [BlockPos; 6] {
    [
        BlockPos::new(p.x + 1, p.y, p.z),
        BlockPos::new(p.x - 1, p.y, p.z),
        BlockPos::new(p.x, p.y + 1, p.z),
        BlockPos::new(p.x, p.y - 1, p.z),
        BlockPos::new(p.x, p.y, p.z + 1),
        BlockPos::new(p.x, p.y, p.z - 1),
    ]
}

pub(super) fn point(k: &Click) -> [f64; 3] {
    [k.hit.location.x, k.hit.location.y, k.hit.location.z]
}

pub(super) fn crosses(a: [f64; 3], b: [f64; 3], p: BlockPos) -> bool {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if len < 1e-9 {
        return false;
    }
    let dir = [d[0] / len, d[1] / len, d[2] / len];
    const E: f64 = 1e-6;
    let lo = [f64::from(p.x) + E, f64::from(p.y) + E, f64::from(p.z) + E];
    let hi = [
        f64::from(p.x) + 1.0 - E,
        f64::from(p.y) + 1.0 - E,
        f64::from(p.z) + 1.0 - E,
    ];
    ray_hits(a, dir, len, lo, hi)
}

pub(super) fn covers(p: BlockPos, eye: [f64; 3], others: &[Click]) -> bool {
    !others.is_empty() && others.iter().all(|k| crosses(eye, point(k), p))
}

pub(super) fn angle(a: Look, b: Look) -> f32 {
    let (u, v) = (view_vector(a), view_vector(b));
    let dot = (u[0] * v[0] + u[1] * v[1] + u[2] * v[2]).clamp(-1.0, 1.0);
    dot.acos().to_degrees() as f32
}

pub(super) fn least_turn(clicks: &[Click], from: Look) -> Option<(&Click, f32)> {
    clicks
        .iter()
        .map(|k| (k, angle(from, k.look)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Choice {
    pub(super) y: i32,
    pub(super) air: bool,
    pub(super) covers: bool,
    pub(super) held: bool,
    pub(super) angle: f32,
}

pub(super) fn pick(choices: &[Choice]) -> Option<usize> {
    choices
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.y.cmp(&b.y)
                .then(a.air.cmp(&b.air))
                .then(a.covers.cmp(&b.covers))
                .then(b.held.cmp(&a.held))
                .then(a.angle.total_cmp(&b.angle))
        })
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use azalea_core::direction::Direction;
    use azalea_core::hit_result::BlockHitResult;
    use azalea_core::position::Vec3;

    use super::*;
    use crate::modules::sent::look_at;

    fn floor_click(eye: [f64; 3], cell: BlockPos) -> Click {
        let p = [
            f64::from(cell.x) + 0.5,
            f64::from(cell.y),
            f64::from(cell.z) + 0.5,
        ];
        Click {
            hit: BlockHitResult {
                location: Vec3::new(p[0], p[1], p[2]),
                miss: false,
                direction: Direction::Up,
                block_pos: BlockPos::new(cell.x, cell.y - 1, cell.z),
                inside: false,
                world_border: false,
            },
            look: look_at(eye, p),
            sneak: false,
        }
    }

    #[test]
    fn the_stand_position_resets_on_moves_and_foreign_changes_nearby() {
        let feet = BlockPos::new(0, 64, 0);
        let eye = [0.5, 65.62, 0.5];
        let mut sight = Sight::new(feet);
        assert!(sight.refresh(feet, eye, &[], 5));
        sight.fail(BlockPos::new(1, 64, 0));
        assert!(!sight.refresh(feet, eye, &[BlockPos::new(20, 64, 0)], 5));
        assert!(sight.tried(BlockPos::new(1, 64, 0)));
        assert!(!sight.refresh(feet, [0.6, 65.7, 0.4], &[], 5));
        assert!(sight.tried(BlockPos::new(1, 64, 0)));
        assert!(!sight.refresh(feet, eye, &[BlockPos::new(3, 66, -5)], 5));
        assert!(sight.tried(BlockPos::new(1, 64, 0)));
        assert!(!sight.refresh(feet, eye, &[BlockPos::new(2, 64, 0)], 5));
        assert!(!sight.tried(BlockPos::new(1, 64, 0)));
        sight.fail(BlockPos::new(1, 64, 0));
        sight.solved(
            BlockPos::new(-1, 64, 0),
            vec![floor_click(eye, BlockPos::new(-1, 64, 0))],
        );
        assert!(!sight.refresh(feet, [0.9, 65.62, 0.9], &[], 5));
        assert!(sight.tried(BlockPos::new(1, 64, 0)));
        assert!(!sight.known(BlockPos::new(-1, 64, 0)));
        sight.fail(BlockPos::new(1, 64, 0));
        assert!(sight.refresh(BlockPos::new(1, 64, 1), eye, &[], 5));
        assert!(!sight.known(BlockPos::new(1, 64, 0)));
    }

    #[test]
    fn an_own_placement_drops_the_clicks_it_hides_and_frees_its_neighbours() {
        let eye = [0.5, 101.62, 0.5];
        let near = BlockPos::new(0, 100, 2);
        let far = BlockPos::new(0, 100, 3);
        let beside = BlockPos::new(1, 100, 2);
        let mut sight = Sight::new(BlockPos::new(0, 100, 0));
        sight.solved(far, vec![floor_click(eye, far)]);
        sight.fail(beside);
        sight.placed(near, eye);
        assert!(!sight.known(far));
        assert!(!sight.known(beside));
    }

    #[test]
    fn a_nearer_floor_block_hides_the_far_row_and_not_the_other_way() {
        let eye = [0.5, 101.62, 0.5];
        let near = BlockPos::new(0, 100, 2);
        let far = BlockPos::new(0, 100, 3);
        assert!(covers(near, eye, &[floor_click(eye, far)]));
        assert!(!covers(far, eye, &[floor_click(eye, near)]));
        assert!(!crosses(eye, [0.5, 100.0, 2.5], BlockPos::new(0, 99, 2)));
    }

    #[test]
    fn the_floor_is_laid_from_the_far_row_back() {
        let eye = [0.5, 101.62, 0.5];
        let look = Look {
            yaw: 0.0,
            pitch: 60.0,
        };
        let near = BlockPos::new(0, 100, 2);
        let far = BlockPos::new(0, 100, 3);
        let (kn, kf) = (floor_click(eye, near), floor_click(eye, far));
        let choices = [
            Choice {
                y: 100,
                air: false,
                covers: covers(near, eye, std::slice::from_ref(&kf)),
                held: true,
                angle: angle(look, kn.look),
            },
            Choice {
                y: 100,
                air: false,
                covers: covers(far, eye, std::slice::from_ref(&kn)),
                held: true,
                angle: angle(look, kf.look),
            },
        ];
        assert!(choices[0].angle < choices[1].angle);
        assert_eq!(pick(&choices), Some(1));
    }

    #[test]
    fn a_row_is_swept_in_order() {
        let eye = [0.5, 101.62, 0.5];
        let row: Vec<BlockPos> = [3, -1, 1, 0, 2]
            .iter()
            .map(|&x| BlockPos::new(x, 100, 3))
            .collect();
        let mut left: Vec<Click> = row.iter().map(|c| floor_click(eye, *c)).collect();
        let mut look = look_at(eye, [-1.5, 100.0, 3.5]);
        let mut order = Vec::new();
        while !left.is_empty() {
            let choices: Vec<Choice> = left
                .iter()
                .map(|k| Choice {
                    y: 100,
                    air: false,
                    covers: false,
                    held: true,
                    angle: angle(look, k.look),
                })
                .collect();
            let i = pick(&choices).unwrap();
            let k = left.remove(i);
            look = k.look;
            order.push(k.hit.block_pos.x);
        }
        assert_eq!(order, vec![-1, 0, 1, 2, 3]);
    }

    #[test]
    fn the_item_changes_only_when_the_held_one_has_no_cell_left() {
        let choices = [
            Choice {
                y: 64,
                air: false,
                covers: false,
                held: false,
                angle: 5.0,
            },
            Choice {
                y: 64,
                air: false,
                covers: false,
                held: true,
                angle: 40.0,
            },
        ];
        assert_eq!(pick(&choices), Some(1));
        let other = [Choice {
            y: 64,
            air: false,
            covers: false,
            held: false,
            angle: 5.0,
        }];
        assert_eq!(pick(&other), Some(0));
        let layers = [
            Choice {
                y: 65,
                air: false,
                covers: false,
                held: true,
                angle: 1.0,
            },
            Choice {
                y: 64,
                air: false,
                covers: false,
                held: false,
                angle: 90.0,
            },
        ];
        assert_eq!(pick(&layers), Some(1));
        let air = [
            Choice {
                y: 64,
                air: true,
                covers: false,
                held: true,
                angle: 1.0,
            },
            Choice {
                y: 64,
                air: false,
                covers: true,
                held: false,
                angle: 90.0,
            },
        ];
        assert_eq!(pick(&air), Some(1));
    }
}
