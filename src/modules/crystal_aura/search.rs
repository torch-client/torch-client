use azalea::ecs::entity::Entity as AzEntity;
use azalea::entity::metadata::PlayerAbsorption;
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea_core::direction::Direction;
use azalea_core::hit_result::BlockHitResult;
use azalea_core::position::{BlockPos, Vec3};
use azalea_registry::builtin::{BlockKind, EntityKind};
use azalea_world::World;

use super::Me;
use crate::modules::sent::{Look, box_distance, view_vector};
use crate::util::combat::{self, CRYSTAL_POWER, Defense, seen_percent};

const FACE_MIN: f32 = 1.5;

#[derive(Clone, Copy)]
pub(super) struct Body {
    pub feet: [f64; 3],
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Body {
    pub fn of(b: &azalea_core::aabb::Aabb) -> Body {
        let (min, max) = ([b.min.x, b.min.y, b.min.z], [b.max.x, b.max.y, b.max.z]);
        Body {
            feet: [(min[0] + max[0]) * 0.5, min[1], (min[2] + max[2]) * 0.5],
            min,
            max,
        }
    }
}

pub(super) struct Target {
    body: Body,
    defense: Defense,
    min: f32,
}

#[derive(Clone, Copy)]
pub(super) struct Crystal {
    pub id: i32,
    pub entity: AzEntity,
    pub body: Body,
}

impl Crystal {
    pub fn cell(&self) -> BlockPos {
        let f = self.body.feet;
        BlockPos::new(
            f[0].floor() as i32,
            f[1].floor() as i32,
            f[2].floor() as i32,
        )
    }
}

pub(super) struct Rules {
    pub target_range: f64,
    pub min_damage: f32,
    pub max_self: f32,
    pub anti_suicide: bool,
    pub face: Option<(f32, f32)>,
    pub difficulty: u8,
}

pub(super) struct Scene {
    pub targets: Vec<Target>,
    pub crystals: Vec<Crystal>,
    boxes: Vec<(i32, [f64; 3], [f64; 3])>,
}

impl Scene {
    pub fn gather(bot: &azalea::Client, me: &Me, rules: &Rules, reach: f64) -> Scene {
        let mut targets = Vec::new();
        let mut crystals = Vec::new();
        let mut boxes = Vec::new();
        let near = reach + 4.0;
        crate::modules::nearby::each(bot, |e| {
            let body = Body::of(&e.physics.bounding_box);
            let from_eye = box_distance(me.eye, body.min, body.max);
            if from_eye <= near {
                boxes.push((e.id, body.min, body.max));
            }
            if e.kind == EntityKind::EndCrystal {
                if from_eye <= near {
                    crystals.push(Crystal {
                        id: e.id,
                        entity: e.entity,
                        body,
                    });
                }
                return;
            }
            let Some(health) = e.health.filter(|&h| h > 0.0) else {
                return;
            };
            if !crate::modules::entities::kind_enabled(e.kind)
                || distance(body.feet, me.body.feet) > rules.target_range
            {
                return;
            }
            let absorption = e
                .world
                .get::<PlayerAbsorption>(e.entity)
                .map_or(0.0, |a| a.0);
            targets.push((e.id, body, health + absorption));
        });

        let equipment = crate::client::tracking::equipment().lock().unwrap();
        let targets = targets
            .into_iter()
            .map(|(id, body, health)| {
                let worn = equipment.get(&id).map(|e| e.worn).unwrap_or_default();
                let face = rules.face.is_some_and(|(at_health, at_durability)| {
                    health <= at_health
                        || worn
                            .iter()
                            .any(|w| w.present && w.durability * 100.0 <= at_durability)
                });
                Target {
                    body,
                    defense: Defense::of(&worn, None),
                    min: if face {
                        rules.min_damage.min(FACE_MIN)
                    } else {
                        rules.min_damage
                    },
                }
            })
            .collect();
        Scene {
            targets,
            crystals,
            boxes,
        }
    }

    fn weigh(&self, w: &World, me: &Me, rules: &Rules, center: [f64; 3], beat: f32) -> Option<f32> {
        let hurt = |body: &Body, def: &Defense, exposure: f32| {
            combat::explosion(
                CRYSTAL_POWER,
                center,
                body.feet,
                exposure,
                def,
                rules.difficulty,
            )
        };
        let safe = |d: f32| d <= rules.max_self && !(rules.anti_suicide && d >= me.health);
        if !safe(hurt(&me.body, &me.defense, 1.0))
            && !safe(hurt(
                &me.body,
                &me.defense,
                seen_percent(&w.chunks, center, me.body.min, me.body.max),
            ))
        {
            return None;
        }
        let mut best: Option<f32> = None;
        for t in &self.targets {
            let bound = hurt(&t.body, &t.defense, 1.0);
            if bound < t.min || bound <= beat || best.is_some_and(|b| bound <= b) {
                continue;
            }
            let d = hurt(
                &t.body,
                &t.defense,
                seen_percent(&w.chunks, center, t.body.min, t.body.max),
            );
            if d >= t.min && d > beat && best.is_none_or(|b| d > b) {
                best = Some(d);
            }
        }
        best
    }

    pub fn best_crystal(
        &self,
        w: &World,
        me: &Me,
        rules: &Rules,
        reach: f64,
        walls: bool,
        skip: impl Fn(i32) -> bool,
    ) -> Option<(Crystal, Option<[f64; 3]>)> {
        let mut best: Option<(f32, Crystal, Option<[f64; 3]>)> = None;
        for c in &self.crystals {
            if skip(c.id) || box_distance(me.eye, c.body.min, c.body.max) > reach {
                continue;
            }
            let aim = aim_into(me.eye, c.body.min, c.body.max);
            if !walls && aim.is_some_and(|p| blocked(w, me.eye, p)) {
                continue;
            }
            let beat = best.as_ref().map_or(0.0, |b| b.0);
            if let Some(d) = self.weigh(w, me, rules, c.body.feet, beat) {
                best = Some((d, *c, aim));
            }
        }
        best.map(|(_, c, aim)| (c, aim))
    }

    pub fn best_base(
        &self,
        w: &World,
        me: &Me,
        rules: &Rules,
        reach: f64,
        walls: bool,
        gone: impl Fn(i32) -> bool,
    ) -> Option<(BlockPos, [f64; 3], f32)> {
        let r = reach.ceil() as i32 + 1;
        let at = BlockPos::new(
            me.eye[0].floor() as i32,
            me.eye[1].floor() as i32,
            me.eye[2].floor() as i32,
        );
        let mut best: Option<(BlockPos, [f64; 3], f32)> = None;
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    let base = BlockPos::new(at.x + dx, at.y + dy, at.z + dz);
                    let top = [
                        base.x as f64 + 0.5,
                        base.y as f64 + 1.0,
                        base.z as f64 + 0.5,
                    ];
                    if distance(me.eye, top) > reach {
                        continue;
                    }
                    let Some(state) = w.get_block_state(base) else {
                        continue;
                    };
                    if !matches!(
                        BlockKind::from(state),
                        BlockKind::Obsidian | BlockKind::Bedrock
                    ) {
                        continue;
                    }
                    if !w.get_block_state(base.up(1)).is_some_and(|s| s.is_air()) {
                        continue;
                    }
                    if self.obstructed(me, base, &gone) {
                        continue;
                    }
                    let aim = if walls {
                        Some(top)
                    } else {
                        face_point(w, me.eye, base, reach)
                    };
                    let Some(aim) = aim else {
                        continue;
                    };
                    let beat = best.as_ref().map_or(0.0, |b| b.2);
                    if let Some(d) = self.weigh(w, me, rules, top, beat) {
                        best = Some((base, aim, d));
                    }
                }
            }
        }
        best
    }

    fn obstructed(&self, me: &Me, base: BlockPos, gone: &impl Fn(i32) -> bool) -> bool {
        let lo = [base.x as f64, base.y as f64 + 1.0, base.z as f64];
        let hi = [lo[0] + 1.0, lo[1] + 2.0, lo[2] + 1.0];
        let hits = |min: [f64; 3], max: [f64; 3]| (0..3).all(|i| lo[i] < max[i] && hi[i] > min[i]);
        hits(me.body.min, me.body.max)
            || self
                .boxes
                .iter()
                .any(|&(id, min, max)| hits(min, max) && !gone(id))
    }
}

pub(super) fn click_on(
    w: &World,
    eye: [f64; 3],
    look: Look,
    base: BlockPos,
    reach: f64,
) -> Option<BlockHitResult> {
    let d = view_vector(look);
    let to = [
        eye[0] + d[0] * reach,
        eye[1] + d[1] * reach,
        eye[2] + d[2] * reach,
    ];
    let hit = trace(w, eye, to);
    (!hit.miss
        && hit.block_pos == base
        && distance(eye, [hit.location.x, hit.location.y, hit.location.z]) <= reach)
        .then_some(hit)
}

pub(super) fn top_click(base: BlockPos) -> BlockHitResult {
    BlockHitResult {
        location: Vec3::new(
            base.x as f64 + 0.5,
            base.y as f64 + 1.0,
            base.z as f64 + 0.5,
        ),
        miss: false,
        direction: Direction::Up,
        block_pos: base,
        inside: false,
        world_border: false,
    }
}

fn face_point(w: &World, eye: [f64; 3], base: BlockPos, reach: f64) -> Option<[f64; 3]> {
    let (x, y, z) = (base.x as f64 + 0.5, base.y as f64, base.z as f64 + 0.5);
    [[x, y + 0.95, z], [x, y + 0.5, z]].into_iter().find(|&p| {
        let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let to = [
            eye[0] + d[0] * 1.2,
            eye[1] + d[1] * 1.2,
            eye[2] + d[2] * 1.2,
        ];
        let hit = trace(w, eye, to);
        !hit.miss
            && hit.block_pos == base
            && distance(eye, [hit.location.x, hit.location.y, hit.location.z]) <= reach
    })
}

fn aim_into(eye: [f64; 3], min: [f64; 3], max: [f64; 3]) -> Option<[f64; 3]> {
    if (0..3).all(|i| eye[i] >= min[i] && eye[i] <= max[i]) {
        return None;
    }
    let core = [
        (min[0] + max[0]) * 0.5,
        min[1] + (max[1] - min[1]) * 0.4,
        (min[2] + max[2]) * 0.5,
    ];
    Some(std::array::from_fn(|i| {
        let near = eye[i].clamp(min[i], max[i]);
        near + (core[i] - near) * 0.1
    }))
}

fn blocked(w: &World, eye: [f64; 3], to: [f64; 3]) -> bool {
    !trace(w, eye, to).miss
}

fn trace(w: &World, from: [f64; 3], to: [f64; 3]) -> BlockHitResult {
    clip(
        &w.chunks,
        ClipContext {
            from: Vec3::new(from[0], from[1], from[2]),
            to: Vec3::new(to[0], to[1], to[2]),
            block_shape_type: BlockShapeType::Outline,
            fluid_pick_type: FluidPickType::None,
            held_item: azalea_registry::builtin::ItemKind::Air,
            entity: None,
        },
    )
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_aim_point_is_inside_the_box_and_none_from_inside() {
        let (min, max) = ([0.0, 65.0, 0.0], [2.0, 67.0, 2.0]);
        assert!(aim_into([1.0, 66.0, 1.0], min, max).is_none());
        let p = aim_into([5.0, 70.0, 1.0], min, max).unwrap();
        assert!((0..3).all(|i| p[i] > min[i] && p[i] < max[i]), "{p:?}");
    }
}
