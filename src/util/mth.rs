use std::f32::consts::{PI, TAU};

pub const DEG_TO_RAD: f32 = 0.017_453_292;

pub const RAD_TO_DEG: f32 = 57.295_776;

pub fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

pub fn lerp_f64(t: f32, a: f64, b: f64) -> f64 {
    a + t as f64 * (b - a)
}

pub fn wrap_degrees_f64(mut degrees: f64) -> f64 {
    degrees %= 360.0;
    if degrees >= 180.0 {
        degrees -= 360.0;
    }
    if degrees < -180.0 {
        degrees += 360.0;
    }
    degrees
}

pub fn rot_lerp_f64(t: f64, a: f64, b: f64) -> f64 {
    a + t * wrap_degrees_f64(b - a)
}

pub fn wrap_degrees(mut degrees: f32) -> f32 {
    degrees %= 360.0;
    if degrees >= 180.0 {
        degrees -= 360.0;
    }
    if degrees < -180.0 {
        degrees += 360.0;
    }
    degrees
}

pub fn rot_lerp(t: f32, from: f32, to: f32) -> f32 {
    from + t * wrap_degrees(to - from)
}

pub fn rot_lerp_rad(t: f32, from: f32, to: f32) -> f32 {
    let mut diff = to - from;
    while diff < -PI {
        diff += TAU;
    }
    while diff >= PI {
        diff -= TAU;
    }
    from + t * diff
}

pub fn rot_lerp_capped(from: f32, to: f32, max: f32) -> f32 {
    from + wrap_degrees(to - from).clamp(-max, max)
}

pub fn progress(x: f32, start: f32, end: f32) -> f32 {
    ((x - start) / (end - start)).clamp(0.0, 1.0)
}

pub mod ease {
    use std::f32::consts::PI;

    pub fn in_quad(x: f32) -> f32 {
        x * x
    }

    pub fn out_quart(x: f32) -> f32 {
        let inv = 1.0 - x;
        1.0 - (inv * inv) * (inv * inv)
    }

    pub fn in_out_sine(x: f32) -> f32 {
        -((PI * x).cos() - 1.0) / 2.0
    }

    pub fn in_out_expo(x: f32) -> f32 {
        if x < 0.5 {
            if x == 0.0 {
                0.0
            } else {
                2.0f32.powf(20.0 * x - 10.0) / 2.0
            }
        } else if x == 1.0 {
            1.0
        } else {
            (2.0 - 2.0f32.powf(-20.0 * x + 10.0)) / 2.0
        }
    }

    pub fn out_back(x: f32) -> f32 {
        const C1: f32 = 1.701_58;
        const C3: f32 = 2.701_58;
        1.0 + C3 * (x - 1.0).powi(3) + C1 * (x - 1.0).powi(2)
    }
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn srgb_byte_to_linear(c: u8) -> f32 {
    srgb_to_linear(c as f32 / 255.0)
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::ease::*;
    use super::*;

    #[test]
    fn the_two_conversion_constants_round_trip() {
        assert!((DEG_TO_RAD * RAD_TO_DEG - 1.0).abs() < 1e-6);
        assert!((180.0 * DEG_TO_RAD - PI).abs() < 1e-5);
    }

    #[test]
    fn wrapping_is_half_open_at_the_top() {
        assert_eq!(wrap_degrees(0.0), 0.0);
        assert_eq!(wrap_degrees(180.0), -180.0);
        assert_eq!(wrap_degrees(-180.0), -180.0);
        assert!((wrap_degrees(370.0) - 10.0).abs() < 1e-4);
        assert!((wrap_degrees(-370.0) + 10.0).abs() < 1e-4);
    }

    #[test]
    fn rotation_lerps_take_the_short_way() {
        assert!((rot_lerp(0.5, 350.0, 10.0) - 360.0).abs() < 1e-4);
        let from = 350.0 * DEG_TO_RAD;
        let to = 10.0 * DEG_TO_RAD;
        assert!((rot_lerp_rad(0.5, from, to) - 360.0 * DEG_TO_RAD).abs() < 1e-4);
    }

    #[test]
    fn the_capped_lerp_never_oversteps() {
        assert!((rot_lerp_capped(0.0, 90.0, 10.0) - 10.0).abs() < 1e-4);
        assert!((rot_lerp_capped(0.0, -90.0, 10.0) + 10.0).abs() < 1e-4);
        assert!((rot_lerp_capped(0.0, 5.0, 10.0) - 5.0).abs() < 1e-4);
    }

    #[test]
    fn progress_clamps_outside_its_window() {
        assert_eq!(progress(5.0, 10.0, 20.0), 0.0);
        assert_eq!(progress(25.0, 10.0, 20.0), 1.0);
        assert!((progress(15.0, 10.0, 20.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn every_easing_runs_from_zero_to_one() {
        for f in [
            in_quad as fn(f32) -> f32,
            out_quart,
            in_out_sine,
            in_out_expo,
            out_back,
        ] {
            assert!(f(0.0).abs() < 1e-6);
            assert!((f(1.0) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn out_back_overshoots_before_it_settles() {
        assert!(out_back(0.7) > 1.0);
    }
}
