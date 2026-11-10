use std::f32::consts::TAU;

use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea_core::direction::Direction;
use azalea_core::hit_result::BlockHitResult;
use azalea_core::position::{BlockPos, Vec3};
use azalea_registry::builtin::BlockKind;

use super::target::{Cells, HORIZONTAL, Me, edge_aim, face_visible, step};
use crate::modules::sent::{Look, look_at, view_vector};
use crate::modules::{MS_PER_TICK, roll, turn};
use crate::util::mth::wrap_degrees;

pub(super) struct WorldCells<'a> {
    pub world: &'a azalea_world::World,
    pub item: BlockKind,
}

impl Cells for WorldCells<'_> {
    fn state(&self, p: BlockPos) -> Option<azalea::block::BlockState> {
        self.world.get_block_state(p)
    }
    fn item(&self) -> BlockKind {
        self.item
    }
}

impl WorldCells<'_> {
    pub fn trace(&self, eye: [f64; 3], look: Look, reach: f64) -> Option<BlockHitResult> {
        let d = view_vector(look);
        let from = Vec3::new(eye[0], eye[1], eye[2]);
        let to = Vec3::new(
            eye[0] + d[0] * reach,
            eye[1] + d[1] * reach,
            eye[2] + d[2] * reach,
        );
        let hit = clip(
            &self.world.chunks,
            ClipContext {
                from,
                to,
                block_shape_type: BlockShapeType::Outline,
                fluid_pick_type: FluidPickType::None,
                held_item: azalea_registry::builtin::ItemKind::Air,
                entity: None,
            },
        );
        let l = hit.location;
        let dist =
            ((l.x - eye[0]).powi(2) + (l.y - eye[1]).powi(2) + (l.z - eye[2]).powi(2)).sqrt();
        (!hit.miss && dist <= reach).then_some(hit)
    }
}

struct Jitter {
    phase: f32,
    speed: f32,
    amp: f32,
    reroll_in: u32,
}

impl Jitter {
    const IDLE: Jitter = Jitter {
        phase: 0.0,
        speed: 0.0,
        amp: 0.0,
        reroll_in: 0,
    };

    fn next(&mut self) -> f32 {
        if self.reroll_in == 0 {
            self.reroll_in = (roll(200.0, 499.0) / MS_PER_TICK).round() as u32;
            self.speed = roll(0.04, 0.16);
            self.amp = roll(0.1, 0.3);
        } else {
            self.reroll_in -= 1;
        }
        self.phase = (self.phase + self.speed) % TAU;
        self.phase.sin() * self.amp + (self.phase * 3.1 + 0.7).sin() * self.amp * 0.25
    }
}

pub(super) struct Aimer {
    sticky: Option<BlockPos>,
    last_support: Option<BlockPos>,
    y_offset: f64,
    jitter: Jitter,
    pub warm: u32,
}

impl Aimer {
    pub const IDLE: Aimer = Aimer {
        sticky: None,
        last_support: None,
        y_offset: 0.0,
        jitter: Jitter::IDLE,
        warm: 0,
    };

    pub fn idle(&mut self) {
        self.sticky = None;
        self.last_support = None;
        self.warm = 0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        c: &WorldCells,
        me: &Me,
        target: BlockPos,
        airborne: bool,
        y: f64,
        from: Look,
        steering: bool,
        turn: (f32, f32),
        reach: f64,
    ) -> (BlockPos, Look) {
        let [px, py, pz] = me.pos;
        let layer_y = y.floor() as i32 - 1;
        let landing = !me.on_ground && me.vel[1] <= 0.05 && py <= layer_y as f64 + 2.05;

        let mut target = target;
        if !landing
            && let Some(prev) = self.sticky
            && prev != target
        {
            let supported = c.solid(prev.down(1))
                || c.solid(prev.up(1))
                || HORIZONTAL.iter().any(|&d| {
                    let n = step(prev, d);
                    c.solid(n) && face_visible(prev, n, px, pz)
                });
            let (dx, dz) = (prev.x as f64 + 0.5 - px, prev.z as f64 + 0.5 - pz);
            if c.replaceable(prev) && supported && dx * dx + dz * dz < 12.0 {
                target = prev;
            }
        }
        self.sticky = Some(target);

        let reversing = !me.on_ground
            && me.walk_dir().is_some_and(|(mx, mz)| {
                let (tx, tz) = (target.x as f64 + 0.5 - px, target.z as f64 + 0.5 - pz);
                tx * mx + tz * mz < -0.9
            });

        let (edge, support) = edge_aim(c, me, target, airborne, y, self.last_support, reversing);
        self.last_support = support;
        let no_floor = !c.solid(target.down(1));
        let aim = if no_floor {
            edge
        } else {
            self.y_offset =
                (self.y_offset + (roll(0.0, 1.0) as f64 - 0.5) * 0.07).clamp(-0.35, 0.35);
            [edge[0], edge[1] + self.y_offset, edge[2]]
        };

        let eye = me.eye();
        let predicted = match (me.on_ground, me.walk_dir()) {
            (true, dir) => {
                let (dx, dz) = dir.unwrap_or((0.0, 0.0));
                let speed = if dir.is_some() {
                    me.vel[0].hypot(me.vel[2]).min(0.6)
                } else {
                    0.0
                };
                [
                    eye[0] + dx * speed,
                    eye[1] + me.vel[1] * 0.5,
                    eye[2] + dz * speed,
                ]
            }
            (false, _) => eye,
        };

        let hi = turn.1.max(turn.0);
        let mut lo = turn.0.min(hi);
        if no_floor || !me.on_ground {
            lo = lo.max(hi * 0.9);
        }

        let grid = me.moving() && me.on_ground;
        let want = solve_grid(c, aim, predicted, target, me.yaw, grid, reach);

        if me.on_ground
            && !me.jump
            && steering
            && self.parked(c, me, eye, predicted, from, target, reach)
        {
            return (target, from);
        }

        let want = Look {
            yaw: want.yaw,
            pitch: want.pitch + self.jitter.next(),
        };
        (target, turn_toward(from, want, roll(lo, hi)))
    }

    #[allow(clippy::too_many_arguments)]
    fn parked(
        &self,
        c: &WorldCells,
        me: &Me,
        eye: [f64; 3],
        predicted: [f64; 3],
        look: Look,
        target: BlockPos,
        reach: f64,
    ) -> bool {
        let now = park_arm_cell(c, me, eye, look, target.y, reach);
        let next = park_arm_cell(c, me, predicted, look, target.y, reach);
        if now.is_some() && !(next.is_some() && next != now) {
            return true;
        }
        [predicted, eye].into_iter().any(|from| {
            c.trace(from, look, reach).is_some_and(|hit| {
                let filled = step(hit.block_pos, hit.direction) == target;
                (self.last_support == Some(hit.block_pos) && filled)
                    || (hit.block_pos == target.down(1) && hit.direction == Direction::Up)
            })
        })
    }
}

pub(super) fn turn_toward(from: Look, to: Look, budget: f32) -> Look {
    let dy = wrap_degrees(to.yaw - from.yaw);
    let dp = to.pitch - from.pitch;
    let total = dy.abs() + dp.abs();
    let (sy, sp) = if total > budget && total >= 0.05 {
        (budget * dy / total, budget * dp / total)
    } else {
        (dy, dp)
    };
    Look {
        yaw: from.yaw + turn::quantise(sy),
        pitch: (from.pitch + turn::quantise(sp)).clamp(-90.0, 90.0),
    }
}

fn solve_grid(
    c: &WorldCells,
    aim: [f64; 3],
    eye: [f64; 3],
    target: BlockPos,
    base_yaw: f32,
    grid: bool,
    reach: f64,
) -> Look {
    let direct = look_at(eye, aim);
    if !grid {
        return direct;
    }
    let step0 = (wrap_degrees(direct.yaw - base_yaw) / 45.0).round() as i32;
    for ring in 0..=4i32 {
        let signs: &[i32] = if ring == 0 { &[0] } else { &[1, -1] };
        for &ys in signs {
            let yaw = base_yaw + 45.0 * (step0 + ys * ring) as f32;
            for dp in [0, 4, 8, 12] {
                let psigns: &[i32] = if dp == 0 { &[0] } else { &[1, -1] };
                for &ps in psigns {
                    let pitch = (direct.pitch + (ps * dp) as f32).clamp(-90.0, 90.0);
                    let look = Look { yaw, pitch };
                    if c.trace(eye, look, reach)
                        .is_some_and(|h| step(h.block_pos, h.direction) == target)
                    {
                        return look;
                    }
                }
            }
        }
    }
    Look {
        yaw: base_yaw + 45.0 * step0 as f32,
        pitch: direct.pitch,
    }
}

fn park_arm_cell(
    c: &WorldCells,
    me: &Me,
    eye: [f64; 3],
    look: Look,
    target_y: i32,
    reach: f64,
) -> Option<BlockPos> {
    let hit = c.trace(eye, look, reach)?;
    acceptable(c, me, step(hit.block_pos, hit.direction), target_y).then_some(hit.block_pos)
}

pub(super) fn acceptable(c: &impl Cells, me: &Me, pos: BlockPos, target_y: i32) -> bool {
    let [px, py, pz] = me.pos;
    let (bx, bz) = me.column();
    let layer_ok = if me.on_ground {
        pos.y == target_y
    } else if !c.replaceable(BlockPos::new(bx, target_y, bz)) {
        pos.y == target_y
    } else if me.vel[1] > 0.05 || py > target_y as f64 + 2.05 {
        pos.y == target_y
    } else {
        let min = target_y.min(py.floor() as i32);
        (min..=target_y).contains(&pos.y)
    };
    if !layer_ok || !c.replaceable(pos) {
        return false;
    }
    let (dx, dz) = (pos.x as f64 + 0.5 - px, pos.z as f64 + 0.5 - pz);
    let max = if me.on_ground { 6.25 } else { 20.25 };
    dx * dx + dz * dz <= max
}

pub(super) fn face_click(hit: &BlockHitResult) -> BlockHitResult {
    let p = hit.block_pos;
    let inner = |v: f64| v.clamp(0.02, 0.98);
    let mut r = [
        hit.location.x - p.x as f64,
        hit.location.y - p.y as f64,
        hit.location.z - p.z as f64,
    ];
    match hit.direction {
        Direction::Up | Direction::Down => {
            r = [
                inner(r[0]),
                if hit.direction == Direction::Up {
                    1.0
                } else {
                    0.0
                },
                inner(r[2]),
            ];
        }
        Direction::North | Direction::South => {
            r = [
                inner(r[0]),
                inner(r[1]),
                if hit.direction == Direction::South {
                    1.0
                } else {
                    0.0
                },
            ];
        }
        Direction::East | Direction::West => {
            r = [
                if hit.direction == Direction::East {
                    1.0
                } else {
                    0.0
                },
                inner(r[1]),
                inner(r[2]),
            ];
        }
    }
    BlockHitResult {
        location: Vec3::new(p.x as f64 + r[0], p.y as f64 + r[1], p.z as f64 + r[2]),
        ..hit.clone()
    }
}

pub(super) fn click(
    c: &WorldCells,
    me: &Me,
    eye: [f64; 3],
    look: Look,
    target_y: i32,
    reach: f64,
) -> Option<BlockHitResult> {
    let hit = c.trace(eye, look, reach)?;
    let place = step(hit.block_pos, hit.direction);
    (acceptable(c, me, place, target_y) && c.solid(hit.block_pos)).then(|| face_click(&hit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_is_capped_and_split() {
        let from = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        let next = turn_toward(
            from,
            Look {
                yaw: 90.0,
                pitch: 30.0,
            },
            40.0,
        );
        assert!((next.yaw - 30.0).abs() < 0.2, "{next:?}");
        assert!((next.pitch - 10.0).abs() < 0.2, "{next:?}");
        let near = turn_toward(
            from,
            Look {
                yaw: 5.0,
                pitch: -3.0,
            },
            40.0,
        );
        assert!(
            (near.yaw - 5.0).abs() < 0.2 && (near.pitch + 3.0).abs() < 0.2,
            "{near:?}"
        );
    }

    #[test]
    fn a_turn_takes_the_short_way_and_does_not_wrap() {
        let from = Look {
            yaw: 170.0,
            pitch: 0.0,
        };
        let next = turn_toward(
            from,
            Look {
                yaw: -170.0,
                pitch: 0.0,
            },
            40.0,
        );
        assert!((next.yaw - 190.0).abs() < 0.2, "{next:?}");
    }

    #[test]
    fn a_click_lands_inside_its_face() {
        let hit = BlockHitResult {
            location: Vec3::new(3.999, 64.5, -1.0),
            miss: false,
            direction: Direction::East,
            block_pos: BlockPos::new(3, 64, -2),
            inside: false,
            world_border: false,
        };
        let c = face_click(&hit);
        assert_eq!(c.location.x, 4.0);
        assert!((c.location.y - 64.5).abs() < 1e-9);
        assert!(
            (c.location.z - (-2.0 + 0.98)).abs() < 1e-9,
            "{:?}",
            c.location
        );
    }
}
