use azalea::Client;
use azalea::movement::LastSentLookDirection;

use super::sent::Look;
use crate::session::SharedState;
use crate::util::mth::wrap_degrees;

const TICK: f32 = super::MS_PER_TICK / 1000.0;

const RATE_JITTER: f32 = 0.2;

#[derive(Clone, Copy, PartialEq)]
pub enum Shape {
    Eased,
    Capped,
}

impl Shape {
    fn step(self) -> fn(f32, f32, f32) -> f32 {
        match self {
            Shape::Eased => step,
            Shape::Capped => step_capped,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SilentTurn {
    out: Option<Look>,
}

impl SilentTurn {
    pub const IDLE: SilentTurn = SilentTurn { out: None };

    pub fn drop_out(&mut self) {
        self.out = None;
    }

    pub fn next(
        &mut self,
        bot: &Client,
        camera: Look,
        want: Option<Look>,
        rate: f32,
        shape: Shape,
    ) -> Option<Look> {
        let target = match want {
            Some(w) => w,
            None if self.out.is_some() => camera,
            None => return None,
        };
        let from = match self.out {
            Some(remembered) => bot
                .component::<LastSentLookDirection>()
                .map_or(remembered, |l| Look {
                    yaw: l.y_rot,
                    pitch: l.x_rot,
                }),
            None => camera,
        };
        let rate = rate * super::roll(1.0 - RATE_JITTER, 1.0);
        let step = shape.step();
        let next = Look {
            yaw: from.yaw + step(wrap_degrees(target.yaw - from.yaw), rate, TICK),
            pitch: (from.pitch + step(target.pitch - from.pitch, rate, TICK)).clamp(-90.0, 90.0),
        };
        if want.is_none()
            && wrap_degrees(next.yaw - camera.yaw).abs() < DEADZONE
            && (next.pitch - camera.pitch).abs() < DEADZONE
        {
            self.out = None;
            return None;
        }
        self.out = Some(next);
        Some(next)
    }
}

pub fn walk_keys(flags: u8, camera_yaw: f32, sent_yaw: f32, fix: bool) -> u8 {
    if fix {
        super::sent::move_fix(flags, camera_yaw, sent_yaw)
    } else {
        flags
    }
}

pub const DEADZONE: f32 = 0.4;

const EASE: f32 = 9.0;

pub fn step(err: f32, rate: f32, dt: f32) -> f32 {
    if err.abs() < DEADZONE {
        return 0.0;
    }
    let cap = rate * dt;
    quantise((err * ease(dt)).clamp(-cap, cap))
}

pub fn ease(dt: f32) -> f32 {
    1.0 - (-EASE * dt).exp()
}

pub fn step_capped(err: f32, rate: f32, dt: f32) -> f32 {
    if err.abs() < DEADZONE {
        return 0.0;
    }
    let cap = rate * dt;
    quantise(err.clamp(-cap, cap))
}

pub fn quantise(deg: f32) -> f32 {
    let grid = crate::renderer::input::LOOK_SCALE;
    (deg / grid).round() * grid
}

pub fn look_toward(
    s: &SharedState,
    yaw: f32,
    pitch: f32,
    dt: f32,
    d: (f32, f32),
    aim: [f64; 3],
    rate: f32,
) -> (f32, f32) {
    look_toward_by(s, yaw, pitch, dt, d, aim, rate, Shape::Eased)
}

#[allow(clippy::too_many_arguments)]
pub fn look_toward_by(
    s: &SharedState,
    yaw: f32,
    pitch: f32,
    dt: f32,
    d: (f32, f32),
    aim: [f64; 3],
    rate: f32,
    shape: Shape,
) -> (f32, f32) {
    let step = shape.step();
    let partial = crate::renderer::systems::partial_ticks(s);
    let (want_yaw, want_pitch) =
        super::aim_assist::look_at(super::aim_assist::eye(s, partial), aim);
    (
        d.0 - step(wrap_degrees(want_yaw - yaw), rate, dt),
        d.1 - step(want_pitch - pitch, rate, dt),
    )
}

pub fn body_step(s: &mut SharedState, aim: [f64; 3], rate: f32) {
    const TICK: f32 = 0.05;
    let (want_yaw, want_pitch) = super::aim_assist::look_at(super::aim_assist::eye(s, 1.0), aim);
    let (yaw, pitch) = (s.camera_yaw, s.camera_pitch);
    s.camera_yaw = yaw + step(wrap_degrees(want_yaw - yaw), rate, TICK);
    s.camera_pitch = (pitch + step(want_pitch - pitch, rate, TICK)).clamp(-89.0, 89.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_lands_on_the_pointer_grid() {
        let grid = crate::renderer::input::LOOK_SCALE;
        for deg in [0.0, 0.01, 0.37, -2.4, 17.9] {
            let q = quantise(deg);
            assert!(
                (q / grid - (q / grid).round()).abs() < 1e-4,
                "{deg} quantised to {q}, which is not a multiple of {grid}"
            );
            assert!((q - deg).abs() <= grid * 0.5 + 1e-6);
        }
    }

    #[test]
    fn a_step_is_capped_eased_and_stops_short() {
        let tick = 0.05;
        assert!((step(170.0, 900.0, tick) - 45.0).abs() <= 0.15);
        assert!((step(-170.0, 900.0, tick) + 45.0).abs() <= 0.15);
        let s = step(10.0, 900.0, tick);
        assert!(s > 0.0 && s < 10.0, "{s}");
        assert_eq!(step(DEADZONE * 0.5, 900.0, tick), 0.0);
    }

    #[test]
    fn a_capped_step_takes_the_whole_error_under_the_cap() {
        let tick = 0.05;
        assert!((step_capped(170.0, 900.0, tick) - 45.0).abs() <= 0.15);
        assert!((step_capped(10.0, 900.0, tick) - 10.0).abs() <= 0.15);
        assert!((step_capped(-10.0, 900.0, tick) + 10.0).abs() <= 0.15);
        assert_eq!(step_capped(DEADZONE * 0.5, 900.0, tick), 0.0);
    }
}
