use crate::util::javarandom::JavaRandom;

const FLASH_INTERVAL: i64 = 600;
const MAX_OFFSET: i64 = 200;
const MIN_DURATION: i64 = 100;
const MAX_DURATION: i64 = 380;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Flash {
    offset: i64,
    duration: i64,
    x_angle: f32,
    y_angle: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct EndFlash {
    pub(crate) intensity: f32,
    pub(crate) previous: f32,
    pub(crate) x_angle: f32,
    pub(crate) y_angle: f32,
}

fn between_inclusive(random: &mut JavaRandom, min: i64, max_inclusive: i64) -> i64 {
    i64::from(random.next_int((max_inclusive - min + 1) as u32)) + min
}

fn flash(clock_time: i64) -> Flash {
    let mut random = JavaRandom::new(clock_time.div_euclid(FLASH_INTERVAL));
    random.next_f32();
    let offset = between_inclusive(&mut random, 0, MAX_OFFSET);
    let duration = between_inclusive(&mut random, MIN_DURATION, MAX_DURATION.min(FLASH_INTERVAL - offset));
    let x_angle = random.next_f32() * 70.0 - 60.0;
    let y_angle = random.next_f32() * 360.0 - 180.0;
    Flash { offset, duration, x_angle, y_angle }
}

fn intensity(clock_time: i64) -> f32 {
    let flash = flash(clock_time);
    let within = clock_time.rem_euclid(FLASH_INTERVAL);
    if within >= flash.offset && within <= flash.offset + flash.duration {
        ((within - flash.offset) as f32 * std::f32::consts::PI / flash.duration as f32).sin()
    } else {
        0.0
    }
}

pub(crate) fn at(clock_ticks: f64) -> EndFlash {
    let tick = clock_ticks.floor() as i64;
    let partial = (clock_ticks - clock_ticks.floor()) as f32;
    let (old, new) = (intensity(tick - 1), intensity(tick));
    let flash = flash(tick);
    EndFlash {
        intensity: old + (new - old) * partial,
        previous: old,
        x_angle: flash.x_angle,
        y_angle: flash.y_angle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intensity_is_half_a_sine_over_the_flash() {
        let f = flash(1234);
        let start = (1234 / FLASH_INTERVAL) * FLASH_INTERVAL + f.offset;
        assert_eq!(intensity(start - 1), 0.0);
        assert!(intensity(start).abs() < 1e-6);
        assert!(intensity(start + f.duration / 2) > 0.99);
        assert_eq!(intensity(start + f.duration + 1), 0.0);
        assert!((MIN_DURATION..=MAX_DURATION).contains(&f.duration));
        assert!((0..=MAX_OFFSET).contains(&f.offset));
        assert!((-60.0..10.0).contains(&f.x_angle) && (-180.0..180.0).contains(&f.y_angle));
    }

    #[test]
    fn an_interval_has_one_flash() {
        assert_eq!(flash(600), flash(1199));
        assert_ne!(flash(600), flash(1200));
    }
}
