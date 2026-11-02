use azalea::Client;
use azalea::entity::LastSentPosition;
use azalea::entity::dimensions::EntityDimensions;
use azalea::movement::LastSentLookDirection;

use crate::session::{MOVE_BACK, MOVE_FORWARD, MOVE_LEFT, MOVE_RIGHT};
use crate::util::mth::wrap_degrees;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub yaw: f32,
    pub pitch: f32,
}

pub fn view_vector(l: Look) -> [f64; 3] {
    let (y, p) = ((l.yaw as f64).to_radians(), (l.pitch as f64).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

pub fn look_at(eye: [f64; 3], to: [f64; 3]) -> Look {
    let (dx, dy, dz) = (to[0] - eye[0], to[1] - eye[1], to[2] - eye[2]);
    let horizontal = (dx * dx + dz * dz).sqrt();
    Look {
        yaw: (-dx).atan2(dz).to_degrees() as f32,
        pitch: (-dy).atan2(horizontal).to_degrees() as f32,
    }
}

pub fn sent_view(bot: &Client) -> Option<([f64; 3], Look)> {
    let pos = azalea::Vec3::from(&*bot.component::<LastSentPosition>().ok()?);
    let eye_height = bot.component::<EntityDimensions>().ok()?.eye_height as f64;
    let look = {
        let l = bot.component::<LastSentLookDirection>().ok()?;
        Look {
            yaw: l.y_rot,
            pitch: l.x_rot,
        }
    };
    Some(([pos.x, pos.y + eye_height, pos.z], look))
}

pub fn sent_ray(bot: &Client) -> Option<([f64; 3], [f64; 3])> {
    sent_view(bot).map(|(eye, look)| (eye, view_vector(look)))
}

pub const DIRS: [u8; 8] = [
    MOVE_FORWARD,
    MOVE_FORWARD | MOVE_RIGHT,
    MOVE_RIGHT,
    MOVE_BACK | MOVE_RIGHT,
    MOVE_BACK,
    MOVE_BACK | MOVE_LEFT,
    MOVE_LEFT,
    MOVE_FORWARD | MOVE_LEFT,
];

pub const WALK_KEYS: u8 = MOVE_FORWARD | MOVE_BACK | MOVE_LEFT | MOVE_RIGHT;

pub fn move_fix(flags: u8, camera_yaw: f32, sent_yaw: f32) -> u8 {
    let Some(i) = DIRS.iter().position(|&d| d == flags & WALK_KEYS) else {
        return flags;
    };
    let heading = wrap_degrees(i as f32 * 45.0 + camera_yaw - sent_yaw);
    let j = ((heading / 45.0).round() as i32).rem_euclid(8) as usize;
    flags & !WALK_KEYS | DIRS[j]
}

pub fn keys_toward(dx: f64, dz: f64, yaw: f32) -> Option<u8> {
    if dx * dx + dz * dz < 1e-8 {
        return None;
    }
    let heading = ((-dx).atan2(dz).to_degrees() as f32) - yaw;
    let i = ((wrap_degrees(heading) / 45.0).round() as i32).rem_euclid(8) as usize;
    Some(DIRS[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looking_at_a_point_faces_it() {
        for (yaw, pitch) in [(0.0, 0.0), (90.0, 30.0), (-135.0, -45.0), (179.0, 80.0)] {
            let v = view_vector(Look { yaw, pitch });
            let eye = [1.0, 64.0, -2.0];
            let to = [
                eye[0] + v[0] * 5.0,
                eye[1] + v[1] * 5.0,
                eye[2] + v[2] * 5.0,
            ];
            let got = look_at(eye, to);
            assert!(wrap_degrees(got.yaw - yaw).abs() < 1e-3, "{yaw} -> {got:?}");
            assert!((got.pitch - pitch).abs() < 1e-3, "{pitch} -> {got:?}");
        }
    }

    #[test]
    fn the_view_vector_matches_vanilla() {
        let close = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9);
        assert!(close(
            view_vector(Look {
                yaw: 0.0,
                pitch: 0.0
            }),
            [0.0, 0.0, 1.0]
        ));
        assert!(close(
            view_vector(Look {
                yaw: 90.0,
                pitch: 0.0
            }),
            [-1.0, 0.0, 0.0]
        ));
        assert!(close(
            view_vector(Look {
                yaw: 0.0,
                pitch: 90.0
            }),
            [0.0, -1.0, 0.0]
        ));
    }

    #[test]
    fn move_fix_keeps_the_heading() {
        assert_eq!(move_fix(MOVE_FORWARD, 0.0, 90.0), MOVE_LEFT);
        assert_eq!(move_fix(MOVE_FORWARD, 0.0, 0.0), MOVE_FORWARD);
        assert_eq!(move_fix(MOVE_FORWARD, 0.0, 180.0), MOVE_BACK);
        assert_eq!(
            move_fix(MOVE_RIGHT, 30.0, 30.0 + 45.0),
            MOVE_FORWARD | MOVE_RIGHT
        );
        let jump = crate::session::MOVE_JUMP;
        assert_eq!(move_fix(MOVE_FORWARD | jump, 0.0, 90.0), MOVE_LEFT | jump);
        assert_eq!(move_fix(jump, 0.0, 90.0), jump);
        assert_eq!(
            move_fix(MOVE_FORWARD | MOVE_BACK, 0.0, 90.0),
            MOVE_FORWARD | MOVE_BACK
        );
    }

    #[test]
    fn keys_toward_a_direction() {
        assert_eq!(keys_toward(0.0, 1.0, 0.0), Some(MOVE_FORWARD));
        assert_eq!(keys_toward(0.0, -1.0, 0.0), Some(MOVE_BACK));
        assert_eq!(keys_toward(-1.0, 0.0, 0.0), Some(MOVE_RIGHT));
        assert_eq!(keys_toward(1.0, 0.0, 0.0), Some(MOVE_LEFT));
        assert_eq!(keys_toward(-1.0, 1.0, 0.0), Some(MOVE_FORWARD | MOVE_RIGHT));
        assert_eq!(keys_toward(0.0, 0.0, 0.0), None);
    }
}
