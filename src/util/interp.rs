use crate::util::mth::rot_lerp_f64;

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

pub const DEFAULT_INTERPOLATION_STEPS: i32 = 3;

pub struct Target<'a> {
    pub pos: &'a mut [f64; 3],
    pub y_rot: &'a mut f32,
    pub x_rot: &'a mut f32,
}

#[derive(Clone, Debug)]
pub struct Interpolation {
    length: i32,
    steps: i32,
    position: [f64; 3],
    y_rot: f32,
    x_rot: f32,
    previous_tick_position: Option<[f64; 3]>,
    previous_tick_rot: Option<(f32, f32)>,
}

impl Interpolation {
    pub fn new() -> Interpolation {
        Interpolation::with_length(DEFAULT_INTERPOLATION_STEPS)
    }

    pub fn with_length(length: i32) -> Interpolation {
        Interpolation {
            length,
            steps: 0,
            position: [0.0; 3],
            y_rot: 0.0,
            x_rot: 0.0,
            previous_tick_position: None,
            previous_tick_rot: None,
        }
    }

    pub fn position(&self, entity: [f64; 3]) -> [f64; 3] {
        if self.steps > 0 {
            self.position
        } else {
            entity
        }
    }

    pub fn y_rot(&self, entity: f32) -> f32 {
        if self.steps > 0 { self.y_rot } else { entity }
    }

    pub fn x_rot(&self, entity: f32) -> f32 {
        if self.steps > 0 { self.x_rot } else { entity }
    }

    pub fn has_active_interpolation(&self) -> bool {
        self.steps > 0
    }

    #[allow(
        dead_code,
        reason = "settable length for the display entities, which have no RenderSpec yet"
    )]
    pub fn set_interpolation_length(&mut self, length: i32) {
        self.length = length;
    }

    pub fn interpolate_to(
        &mut self,
        entity: Target<'_>,
        position: [f64; 3],
        y_rot: f32,
        x_rot: f32,
    ) {
        if self.length == 0 {
            *entity.pos = position;
            *entity.y_rot = y_rot;
            *entity.x_rot = x_rot;
            self.cancel();
        } else if !self.has_active_interpolation()
            || self.y_rot(*entity.y_rot) != y_rot
            || self.x_rot(*entity.x_rot) != x_rot
            || self.position(*entity.pos) != position
        {
            self.steps = self.length;
            self.position = position;
            self.y_rot = y_rot;
            self.x_rot = x_rot;
            self.previous_tick_position = Some(*entity.pos);
            self.previous_tick_rot = Some((*entity.x_rot, *entity.y_rot));
        }
    }

    pub fn interpolate(&mut self, entity: Target<'_>) {
        if !self.has_active_interpolation() {
            self.cancel();
            return;
        }

        let alpha = 1.0 / self.steps as f64;

        if let Some(previous) = self.previous_tick_position {
            let delta = [
                entity.pos[0] - previous[0],
                entity.pos[1] - previous[1],
                entity.pos[2] - previous[2],
            ];
            self.position = [
                self.position[0] + delta[0],
                self.position[1] + delta[1],
                self.position[2] + delta[2],
            ];
        }

        if let Some((previous_x, previous_y)) = self.previous_tick_rot {
            self.y_rot += *entity.y_rot - previous_y;
            self.x_rot += *entity.x_rot - previous_x;
        }

        let new_position = [
            lerp(alpha, entity.pos[0], self.position[0]),
            lerp(alpha, entity.pos[1], self.position[1]),
            lerp(alpha, entity.pos[2], self.position[2]),
        ];
        let new_y_rot = rot_lerp_f64(alpha, *entity.y_rot as f64, self.y_rot as f64) as f32;
        let new_x_rot = lerp(alpha, *entity.x_rot as f64, self.x_rot as f64) as f32;

        *entity.pos = new_position;
        *entity.y_rot = new_y_rot;
        *entity.x_rot = new_x_rot;
        self.steps -= 1;
        self.previous_tick_position = Some(new_position);
        self.previous_tick_rot = Some((*entity.x_rot, *entity.y_rot));
    }

    pub fn cancel(&mut self) {
        self.steps = 0;
        self.previous_tick_position = None;
        self.previous_tick_rot = None;
    }
}

impl Default for Interpolation {
    fn default() -> Interpolation {
        Interpolation::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Entity {
        pos: [f64; 3],
        y_rot: f32,
        x_rot: f32,
    }

    impl Entity {
        fn new() -> Entity {
            Entity {
                pos: [0.0; 3],
                y_rot: 0.0,
                x_rot: 0.0,
            }
        }

        fn target(&mut self) -> Target<'_> {
            Target {
                pos: &mut self.pos,
                y_rot: &mut self.y_rot,
                x_rot: &mut self.x_rot,
            }
        }
    }

    #[test]
    fn a_three_step_run_lands_on_the_target() {
        let mut entity = Entity::new();
        let mut interp = Interpolation::new();
        interp.interpolate_to(entity.target(), [3.0, 0.0, 0.0], 0.0, 0.0);

        interp.interpolate(entity.target());
        assert!((entity.pos[0] - 1.0).abs() < 1e-9, "{}", entity.pos[0]);
        interp.interpolate(entity.target());
        assert!((entity.pos[0] - 2.0).abs() < 1e-9, "{}", entity.pos[0]);
        interp.interpolate(entity.target());
        assert!((entity.pos[0] - 3.0).abs() < 1e-9, "{}", entity.pos[0]);
        assert!(!interp.has_active_interpolation());
    }

    #[test]
    fn a_fresh_target_restarts_the_count() {
        let mut entity = Entity::new();
        let mut interp = Interpolation::new();
        interp.interpolate_to(entity.target(), [3.0, 0.0, 0.0], 0.0, 0.0);
        interp.interpolate(entity.target());
        interp.interpolate_to(entity.target(), [7.0, 0.0, 0.0], 0.0, 0.0);
        interp.interpolate(entity.target());
        assert!((entity.pos[0] - 3.0).abs() < 1e-9, "{}", entity.pos[0]);
        interp.interpolate(entity.target());
        interp.interpolate(entity.target());
        assert!((entity.pos[0] - 7.0).abs() < 1e-9, "{}", entity.pos[0]);
    }

    #[test]
    fn re_aiming_at_the_same_target_is_a_no_op() {
        let mut entity = Entity::new();
        let mut interp = Interpolation::new();
        interp.interpolate_to(entity.target(), [3.0, 0.0, 0.0], 0.0, 0.0);
        interp.interpolate(entity.target());
        interp.interpolate_to(entity.target(), [3.0, 0.0, 0.0], 0.0, 0.0);
        interp.interpolate(entity.target());
        interp.interpolate(entity.target());
        assert!((entity.pos[0] - 3.0).abs() < 1e-9, "{}", entity.pos[0]);
    }

    #[test]
    fn the_yaw_wraps_and_the_pitch_does_not() {
        let mut entity = Entity::new();
        entity.y_rot = 350.0;
        entity.x_rot = 30.0;
        let mut interp = Interpolation::new();
        interp.interpolate_to(entity.target(), [0.0; 3], 10.0, 0.0);
        interp.interpolate(entity.target());
        assert!((entity.y_rot - 356.666_66).abs() < 1e-3, "{}", entity.y_rot);
        assert!((entity.x_rot - 20.0).abs() < 1e-3, "{}", entity.x_rot);
    }

    #[test]
    fn a_zero_length_snaps() {
        let mut entity = Entity::new();
        let mut interp = Interpolation::with_length(0);
        interp.interpolate_to(entity.target(), [5.0, 6.0, 7.0], 90.0, 45.0);
        assert_eq!(entity.pos, [5.0, 6.0, 7.0]);
        assert_eq!(entity.y_rot, 90.0);
        assert_eq!(entity.x_rot, 45.0);
        assert!(!interp.has_active_interpolation());
    }

    #[test]
    fn the_accessors_read_the_target_only_while_a_run_is_active() {
        let mut entity = Entity::new();
        let mut interp = Interpolation::new();
        assert_eq!(interp.position(entity.pos), [0.0; 3]);
        interp.interpolate_to(entity.target(), [3.0, 0.0, 0.0], 90.0, 45.0);
        assert_eq!(interp.position(entity.pos), [3.0, 0.0, 0.0]);
        assert_eq!(interp.y_rot(entity.y_rot), 90.0);
        assert_eq!(interp.x_rot(entity.x_rot), 45.0);
        interp.cancel();
        assert_eq!(interp.position(entity.pos), entity.pos);
        assert_eq!(interp.y_rot(entity.y_rot), entity.y_rot);
    }
}
