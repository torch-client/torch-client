pub(crate) const GRAVITY: f64 = 0.08;

pub(crate) const DRAG: f64 = 0.98_f32 as f64;

pub(crate) const AIR_FRICTION: f64 = 0.91_f32 as f64;

pub(crate) fn step_y(y: f64, vy: f64) -> (f64, f64) {
    (y + vy, (vy - GRAVITY) * DRAG)
}

pub(crate) fn step(pos: [f64; 3], vel: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let (y, vy) = step_y(pos[1], vel[1]);
    (
        [pos[0] + vel[0], y, pos[2] + vel[2]],
        [vel[0] * AIR_FRICTION, vy, vel[2] * AIR_FRICTION],
    )
}

pub(crate) fn airborne_path(
    feet: [f64; 3],
    vel: [f64; 3],
    floor: Option<f64>,
    max: usize,
) -> Vec<[f64; 3]> {
    let landed = |p: [f64; 3]| floor.is_some_and(|f| p[1] <= f);
    let mut out = Vec::with_capacity(max.min(64));
    let (mut p, mut v) = (feet, vel);
    while out.len() < max && !landed(p) {
        out.push(p);
        (p, v) = step(p, v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speed_after(n: usize) -> f64 {
        let mut vy = 0.0;
        for _ in 0..n {
            vy = step_y(0.0, vy).1;
        }
        -vy
    }

    #[test]
    fn a_fall_from_rest_matches_vanilla() {
        assert!((speed_after(1000) - 3.92).abs() < 1e-3);
        let (mut y, mut vy) = (0.0, 0.0);
        let mut ticks = 0;
        while y > -1.0 {
            (y, vy) = step_y(y, vy);
            ticks += 1;
        }
        assert_eq!(ticks, 6);
    }

    #[test]
    fn the_path_ends_before_the_floor() {
        let path = airborne_path([0.0, 74.0, 0.0], [0.0, -0.5, 0.0], Some(64.0), 100);
        assert_eq!(path[0], [0.0, 74.0, 0.0]);
        assert!(path.iter().all(|p| p[1] > 64.0));
        let (mut p, mut v) = (path[0], [0.0, -0.5, 0.0]);
        for _ in 0..path.len() {
            (p, v) = step(p, v);
        }
        let next = p;
        assert!(next[1] <= 64.0, "one more step lands");
        assert_eq!(airborne_path([0.0, 70.0, 0.0], [0.0; 3], None, 7).len(), 7);
    }

    #[test]
    fn horizontal_speed_decays_in_the_air() {
        let (_, v) = step([0.0; 3], [1.0, 0.0, -1.0]);
        assert!((v[0] - 0.91).abs() < 1e-6 && (v[2] + 0.91).abs() < 1e-6);
    }
}
