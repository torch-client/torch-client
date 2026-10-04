use crate::session::SharedState;
use crate::util::mth::wrap_degrees;

pub const DEADZONE: f32 = 0.4;

const EASE: f32 = 9.0;

pub fn step(err: f32, rate: f32, dt: f32) -> f32 {
    if err.abs() < DEADZONE {
        return 0.0;
    }
    let cap = rate * dt;
    let k = 1.0 - (-EASE * dt).exp();
    quantise((err * k).clamp(-cap, cap))
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
    aim: [f32; 3],
    rate: f32,
) -> (f32, f32) {
    let partial = crate::renderer::systems::partial_ticks(s);
    let (want_yaw, want_pitch) =
        super::aim_assist::look_at(super::aim_assist::eye(s, partial), aim);
    (
        d.0 - step(wrap_degrees(want_yaw - yaw), rate, dt),
        d.1 - step(want_pitch - pitch, rate, dt),
    )
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
}
