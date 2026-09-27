use azalea_registry::builtin::EntityKind;

pub trait Level {
    fn collide(&self, min: [f64; 3], max: [f64; 3], movement: [f64; 3]) -> [f64; 3];

    fn friction(&self, pos: [i32; 3]) -> f32;

    fn speed_factor(&self, pos: [i32; 3]) -> (f32, bool);
}

pub struct Void;

impl Level for Void {
    fn collide(&self, _min: [f64; 3], _max: [f64; 3], movement: [f64; 3]) -> [f64; 3] {
        movement
    }

    fn friction(&self, _pos: [i32; 3]) -> f32 {
        0.6
    }

    fn speed_factor(&self, _pos: [i32; 3]) -> (f32, bool) {
        (1.0, false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Server,
    Item,
    FallingBlock,
    Tnt,
    Arrow,
    Thrown,
    Fireball,
}

impl Motion {
    pub fn for_kind(kind: EntityKind) -> Motion {
        match kind {
            EntityKind::Item => Motion::Item,
            EntityKind::FallingBlock => Motion::FallingBlock,
            EntityKind::Tnt => Motion::Tnt,
            EntityKind::Arrow | EntityKind::SpectralArrow | EntityKind::Trident => Motion::Arrow,
            EntityKind::Snowball
            | EntityKind::Egg
            | EntityKind::EnderPearl
            | EntityKind::ExperienceBottle
            | EntityKind::SplashPotion
            | EntityKind::LingeringPotion => Motion::Thrown,
            EntityKind::Fireball
            | EntityKind::SmallFireball
            | EntityKind::DragonFireball
            | EntityKind::WitherSkull => Motion::Fireball,
            _ => Motion::Server,
        }
    }

    fn gravity(self) -> f64 {
        match self {
            Motion::Item | Motion::FallingBlock | Motion::Tnt => 0.04,
            Motion::Arrow => 0.05,
            Motion::Thrown => 0.03,
            Motion::Fireball | Motion::Server => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Body {
    pub velocity: [f64; 3],
    pub on_ground: bool,
    pub in_ground: bool,
}

pub struct Shape {
    pub width: f64,
    pub height: f64,
    pub in_water: bool,
    pub in_lava: bool,
    pub ticks: u32,
    pub id: i32,
}

fn resting(body: &Body, shape: &Shape) -> bool {
    const STILL_SPEED_SQR: f64 = 9.999_999_747_378_752e-6;

    let horizontal_sqr = body.velocity[0] * body.velocity[0] + body.velocity[2] * body.velocity[2];

    body.on_ground
        && horizontal_sqr <= STILL_SPEED_SQR
        && (shape.ticks as i32).wrapping_add(shape.id) % 4 != 0
}

fn bounding_box(pos: [f64; 3], shape: &Shape) -> ([f64; 3], [f64; 3]) {
    let half = shape.width / 2.0;
    (
        [pos[0] - half, pos[1], pos[2] - half],
        [pos[0] + half, pos[1] + shape.height, pos[2] + half],
    )
}

fn block_below(pos: [f64; 3], offset: f64) -> [i32; 3] {
    [
        pos[0].floor() as i32,
        (pos[1] - offset).floor() as i32,
        pos[2].floor() as i32,
    ]
}

fn move_self(
    pos: [f64; 3],
    body: &mut Body,
    shape: &Shape,
    level: &dyn Level,
    below_offset: f64,
) -> [f64; 3] {
    let delta = body.velocity;
    let (min, max) = bounding_box(pos, shape);
    let movement = level.collide(min, max, delta);

    let moved_sqr =
        movement[0] * movement[0] + movement[1] * movement[1] + movement[2] * movement[2];
    let delta_sqr = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
    let pos = if moved_sqr > 1.0e-7 || delta_sqr - moved_sqr < 1.0e-7 {
        [
            pos[0] + movement[0],
            pos[1] + movement[1],
            pos[2] + movement[2],
        ]
    } else {
        pos
    };

    let x_collision = !equal(delta[0], movement[0]);
    let z_collision = !equal(delta[2], movement[2]);
    let horizontal_collision = x_collision || z_collision;
    if delta[1].abs() > 0.0 {
        let vertical_collision = delta[1] != movement[1];
        body.on_ground = vertical_collision && delta[1] < 0.0;
    }
    if horizontal_collision {
        if x_collision {
            body.velocity[0] = 0.0;
        }
        if z_collision {
            body.velocity[2] = 0.0;
        }
    }

    let factor = {
        let (here, answers_for_itself) = level.speed_factor([
            pos[0].floor() as i32,
            pos[1].floor() as i32,
            pos[2].floor() as i32,
        ]);
        if here == 1.0 && !answers_for_itself {
            level.speed_factor(block_below(pos, below_offset)).0
        } else {
            here
        }
    } as f64;
    body.velocity[0] *= factor;
    body.velocity[2] *= factor;

    pos
}

fn equal(a: f64, b: f64) -> bool {
    (b - a).abs() < 1.0e-5
}

pub fn step(
    motion: Motion,
    mut pos: [f64; 3],
    body: &mut Body,
    shape: &Shape,
    level: &dyn Level,
) -> [f64; 3] {
    match motion {
        Motion::Server => pos,

        Motion::Item => {
            if shape.in_water {
                fluid_movement(body, 0.990_000_009_536_743_2);
            } else if shape.in_lava {
                fluid_movement(body, 0.949_999_988_079_071);
            } else {
                body.velocity[1] -= motion.gravity();
            }

            if resting(body, shape) {
                return pos;
            }

            pos = move_self(pos, body, shape, level, 0.999_999);

            let mut friction = 0.98f32;
            if body.on_ground {
                friction = level.friction(block_below(pos, 0.999_999)) * 0.98;
            }
            body.velocity[0] *= friction as f64;
            body.velocity[1] *= 0.98;
            body.velocity[2] *= friction as f64;
            if body.on_ground && body.velocity[1] < 0.0 {
                body.velocity[1] *= -0.5;
            }
            pos
        }

        Motion::FallingBlock | Motion::Tnt => {
            body.velocity[1] -= motion.gravity();

            if resting(body, shape) {
                return pos;
            }

            pos = move_self(pos, body, shape, level, 0.500_001);
            for axis in &mut body.velocity {
                *axis *= 0.98;
            }
            if motion == Motion::Tnt && body.on_ground {
                body.velocity[0] *= 0.7;
                body.velocity[1] *= -0.5;
                body.velocity[2] *= 0.7;
            }
            pos
        }

        Motion::Arrow => {
            if body.in_ground {
                return pos;
            }
            let inertia = if shape.in_water { 0.6 } else { 0.99 };
            for axis in &mut body.velocity {
                *axis *= inertia;
            }
            let (min, max) = bounding_box(pos, shape);
            let movement = level.collide(min, max, body.velocity);
            let hit = (0..3).any(|i| !equal(movement[i], body.velocity[i]));
            for i in 0..3 {
                pos[i] += movement[i];
            }
            if hit {
                body.velocity = [0.0; 3];
                body.in_ground = true;
            } else {
                body.velocity[1] -= motion.gravity();
            }
            pos
        }

        Motion::Thrown | Motion::Fireball => {
            let inertia = match (motion, shape.in_water) {
                (Motion::Thrown, true) => 0.8,
                (Motion::Thrown, false) => 0.99,
                (_, true) => 0.8,
                (_, false) => 0.95,
            };
            body.velocity[1] -= motion.gravity();
            for axis in &mut body.velocity {
                *axis *= inertia;
            }
            let (min, max) = bounding_box(pos, shape);
            let movement = level.collide(min, max, body.velocity);
            for i in 0..3 {
                pos[i] += movement[i];
            }
            pos
        }
    }
}

fn fluid_movement(body: &mut Body, multiplier: f64) {
    body.velocity[0] *= multiplier;
    if body.velocity[1] < 0.059_999_998_658_895_49 {
        body.velocity[1] += 5.0e-4;
    }
    body.velocity[2] *= multiplier;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Floor;

    impl Level for Floor {
        fn collide(&self, min: [f64; 3], _max: [f64; 3], movement: [f64; 3]) -> [f64; 3] {
            let mut out = movement;
            if movement[1] < 0.0 && min[1] + movement[1] < 0.0 {
                out[1] = -min[1];
            }
            out
        }

        fn friction(&self, _pos: [i32; 3]) -> f32 {
            0.6
        }

        fn speed_factor(&self, _pos: [i32; 3]) -> (f32, bool) {
            (1.0, false)
        }
    }

    fn item_shape() -> Shape {
        Shape {
            width: 0.25,
            height: 0.25,
            in_water: false,
            in_lava: false,
            ticks: 1,
            id: 1,
        }
    }

    #[test]
    fn an_item_falls_at_vanillas_rate() {
        let mut body = Body::default();
        let pos = step(
            Motion::Item,
            [0.0, 10.0, 0.0],
            &mut body,
            &item_shape(),
            &Void,
        );
        assert!((pos[1] - 9.96).abs() < 1e-9, "{}", pos[1]);
        assert!(
            (body.velocity[1] + 0.04 * 0.98).abs() < 1e-9,
            "{}",
            body.velocity[1]
        );
    }

    #[test]
    fn an_item_lands_and_stays_on_the_floor() {
        let mut body = Body::default();
        let mut pos = [0.0, 3.0, 0.0];
        let mut shape = item_shape();
        for tick in 0..200u32 {
            shape.ticks = tick;
            pos = step(Motion::Item, pos, &mut body, &shape, &Floor);
        }
        assert!(body.on_ground);
        assert!(pos[1].abs() < 1e-6, "{}", pos[1]);
    }

    #[test]
    fn an_item_keeps_the_velocity_it_spawned_with() {
        let mut body = Body {
            velocity: [0.2, 0.2, 0.0],
            ..Default::default()
        };
        let mut pos = [0.0, 10.0, 0.0];
        for _ in 0..5 {
            pos = step(Motion::Item, pos, &mut body, &item_shape(), &Void);
        }
        assert!(pos[0] > 0.5, "{}", pos[0]);
        assert!(pos[1] > 10.0, "{}", pos[1]);
    }

    #[test]
    fn a_falling_block_accelerates_downward() {
        let mut body = Body::default();
        let shape = Shape {
            width: 0.98,
            height: 0.98,
            in_water: false,
            in_lava: false,
            ticks: 0,
            id: 0,
        };
        let mut pos = [0.0, 10.0, 0.0];
        let mut last = 0.0;
        for _ in 0..5 {
            let before = pos[1];
            pos = step(Motion::FallingBlock, pos, &mut body, &shape, &Void);
            let fell = before - pos[1];
            assert!(fell > last, "{fell} did not exceed {last}");
            last = fell;
        }
    }

    #[test]
    fn a_stuck_arrow_does_not_move() {
        let mut body = Body {
            velocity: [1.0, 0.0, 0.0],
            in_ground: true,
            ..Default::default()
        };
        let shape = Shape {
            width: 0.5,
            height: 0.5,
            in_water: false,
            in_lava: false,
            ticks: 0,
            id: 0,
        };
        let pos = step(Motion::Arrow, [0.0, 10.0, 0.0], &mut body, &shape, &Void);
        assert_eq!(pos, [0.0, 10.0, 0.0]);
        assert_eq!(body.velocity, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_fireball_flies_flat() {
        let mut body = Body {
            velocity: [1.0, 0.0, 0.0],
            ..Default::default()
        };
        let shape = Shape {
            width: 1.0,
            height: 1.0,
            in_water: false,
            in_lava: false,
            ticks: 0,
            id: 0,
        };
        let pos = step(Motion::Fireball, [0.0, 10.0, 0.0], &mut body, &shape, &Void);
        assert_eq!(pos[1], 10.0);
        assert!((pos[0] - 0.95).abs() < 1e-9, "{}", pos[0]);
    }
}
