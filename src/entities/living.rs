use crate::util::interp::{Interpolation, Target};
use crate::util::mth::{lerp, lerp_f64, rot_lerp, wrap_degrees};

pub const SWING_DURATION: i32 = 6;

pub const MAX_HEAD_ROTATION: f32 = 50.0;

const LERP_HEAD_STEPS: i32 = 3;

const MAX_INTERPOLATION_DISTANCE_SQR: f64 = 4096.0;

const BODY_ROT_MOVEMENT_THRESHOLD: f32 = 0.002_500_000_2;

fn distance_sqr(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    dx * dx + dy * dy + dz * dz
}

fn unwind(old: &mut f32, new: f32) {
    while new - *old < -180.0 {
        *old -= 360.0;
    }
    while new - *old >= 180.0 {
        *old += 360.0;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WalkAnim {
    speed_old: f32,
    speed: f32,
    position: f32,
    position_scale: f32,
}

impl WalkAnim {
    pub fn new() -> WalkAnim {
        WalkAnim {
            speed_old: 0.0,
            speed: 0.0,
            position: 0.0,
            position_scale: 1.0,
        }
    }

    pub fn update(&mut self, target_speed: f32, factor: f32, position_scale: f32) {
        self.speed_old = self.speed;
        self.speed += (target_speed - self.speed) * factor;
        self.position += self.speed;
        self.position_scale = position_scale;
    }

    pub fn stop(&mut self) {
        self.speed_old = 0.0;
        self.speed = 0.0;
        self.position = 0.0;
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn current_speed(&self) -> f32 {
        self.speed
    }

    pub fn speed(&self, partial: f32) -> f32 {
        lerp(partial, self.speed_old, self.speed).min(1.0)
    }

    pub fn position(&self, partial: f32) -> f32 {
        (self.position - self.speed * (1.0 - partial)) * self.position_scale
    }
}

impl Default for WalkAnim {
    fn default() -> Self {
        WalkAnim::new()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SwimAmount {
    amount: f32,
    old: f32,
}

impl SwimAmount {
    pub const fn new() -> SwimAmount {
        SwimAmount {
            amount: 0.0,
            old: 0.0,
        }
    }

    pub fn tick(&mut self, swimming: bool) {
        self.old = self.amount;
        self.amount = if swimming {
            (self.amount + 0.09).min(1.0)
        } else {
            (self.amount - 0.09).max(0.0)
        };
    }

    pub fn get(&self, partial: f32) -> f32 {
        lerp(partial, self.old, self.amount)
    }
}

#[derive(Clone, Debug, Default)]
pub struct LivingCore {
    pub pos: [f64; 3],
    pub pos_old: [f64; 3],
    pub body_rot: f32,
    pub body_rot_old: f32,
    pub head_rot: f32,
    pub head_rot_old: f32,
    pub y_rot: f32,
    pub y_rot_old: f32,
    pub x_rot: f32,
    pub x_rot_old: f32,
    pub interp: Option<Interpolation>,
    lerp_head_steps: i32,
    lerp_y_head_rot: f32,
    raw_pos: [f64; 3],
    raw_y_rot: f32,
    raw_x_rot: f32,
    raw_head_rot: f32,
    pub walk: WalkAnim,
    swing_time: i32,
    swinging: bool,
    swing_left: bool,
    attack_anim: f32,
    attack_anim_old: f32,
    swing_duration: i32,
    death_time: u32,
    hurt_time: u32,
}

impl LivingCore {
    pub fn start(
        &mut self,
        pos: [f64; 3],
        y_rot: f32,
        x_rot: f32,
        head_rot: f32,
        interp: Option<Interpolation>,
    ) {
        self.interp = interp;
        self.pos = pos;
        self.pos_old = pos;
        self.y_rot = y_rot;
        self.x_rot = x_rot;
        self.body_rot = y_rot;
        self.head_rot = head_rot;
        self.raw_pos = pos;
        self.raw_y_rot = y_rot;
        self.raw_x_rot = x_rot;
        self.raw_head_rot = head_rot;
    }

    pub fn handle_movement_packets(
        &mut self,
        pos: [f64; 3],
        y_rot: f32,
        x_rot: f32,
    ) -> Option<([f64; 3], f32, f32)> {
        if pos == self.raw_pos && y_rot == self.raw_y_rot && x_rot == self.raw_x_rot {
            return None;
        }
        self.raw_pos = pos;
        self.raw_y_rot = y_rot;
        self.raw_x_rot = x_rot;
        let too_big = distance_sqr(self.pos, pos) > MAX_INTERPOLATION_DISTANCE_SQR;
        let Some(mut interp) = self.interp.take() else {
            return Some((pos, y_rot, x_rot));
        };
        if too_big {
            self.pos = pos;
            self.y_rot = y_rot;
            self.x_rot = x_rot;
            self.pos_old = pos;
            self.y_rot_old = y_rot;
            self.x_rot_old = x_rot;
            interp.cancel();
        } else {
            interp.interpolate_to(
                Target {
                    pos: &mut self.pos,
                    y_rot: &mut self.y_rot,
                    x_rot: &mut self.x_rot,
                },
                pos,
                y_rot,
                x_rot,
            );
        }
        self.interp = Some(interp);
        None
    }

    pub fn rotate_head(&mut self, head_rot: f32, lerp: bool) {
        if head_rot == self.raw_head_rot {
            return;
        }
        self.raw_head_rot = head_rot;
        if lerp {
            self.lerp_y_head_rot = head_rot;
            self.lerp_head_steps = LERP_HEAD_STEPS;
        } else {
            self.head_rot = head_rot;
        }
    }

    pub fn latch(&mut self) {
        self.pos_old = self.pos;
        self.y_rot_old = self.y_rot;
        self.x_rot_old = self.x_rot;
        self.body_rot_old = self.body_rot;
        self.head_rot_old = self.head_rot;
    }

    pub fn step_interpolation(&mut self) {
        if let Some(interp) = self.interp.as_mut()
            && interp.has_active_interpolation()
        {
            interp.interpolate(Target {
                pos: &mut self.pos,
                y_rot: &mut self.y_rot,
                x_rot: &mut self.x_rot,
            });
        }
        if self.lerp_head_steps > 0 {
            self.head_rot = rot_lerp(
                1.0 / self.lerp_head_steps as f32,
                self.head_rot,
                self.lerp_y_head_rot,
            );
            self.lerp_head_steps -= 1;
        }
    }

    pub fn tick_counters(&mut self, dead: bool) {
        self.death_time = if dead {
            (self.death_time + 1).min(20)
        } else {
            0
        };
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }
        self.attack_anim_old = self.attack_anim;
        let duration = self.swing_duration();
        if self.swinging {
            self.swing_time += 1;
            if self.swing_time >= duration {
                self.swing_time = 0;
                self.swinging = false;
            }
        } else {
            self.swing_time = 0;
        }
        self.attack_anim = self.swing_time as f32 / duration as f32;
    }

    fn swing_duration(&self) -> i32 {
        if self.swing_duration > 0 {
            self.swing_duration
        } else {
            SWING_DURATION
        }
    }

    pub fn set_swing_duration(&mut self, ticks: u8) {
        self.swing_duration = i32::from(ticks);
    }

    pub fn turn_body(&mut self) {
        let xd = self.pos[0] - self.pos_old[0];
        let zd = self.pos[2] - self.pos_old[2];
        let mut target = self.body_rot;
        if ((xd * xd + zd * zd) as f32) > BODY_ROT_MOVEMENT_THRESHOLD {
            let walk = (zd.atan2(xd) as f32).to_degrees() - 90.0;
            let diff = (wrap_degrees(self.y_rot) - walk).abs();
            target = if (95.0..265.0).contains(&diff) {
                walk - 180.0
            } else {
                walk
            };
        }
        if self.attack_anim > 0.0 {
            target = self.y_rot;
        }

        self.body_rot += wrap_degrees(target - self.body_rot) * 0.3;
        let head_diff = wrap_degrees(self.y_rot - self.body_rot);
        if head_diff.abs() > MAX_HEAD_ROTATION {
            self.body_rot += head_diff - head_diff.signum() * MAX_HEAD_ROTATION;
        }

        unwind(&mut self.y_rot_old, self.y_rot);
        unwind(&mut self.body_rot_old, self.body_rot);
        unwind(&mut self.x_rot_old, self.x_rot);
        unwind(&mut self.head_rot_old, self.head_rot);
    }

    pub fn swing(&mut self, left_arm: bool) {
        if !self.swinging || self.swing_time >= self.swing_duration() / 2 || self.swing_time < 0 {
            self.swing_time = -1;
            self.swinging = true;
            self.swing_left = left_arm;
        }
    }

    pub fn hurt(&mut self) {
        self.hurt_time = 10;
    }

    pub fn damage(&mut self) {
        self.walk.set_speed(1.5);
        self.hurt();
    }

    pub fn swinging(&self) -> bool {
        self.swinging
    }

    pub fn swing_left(&self) -> bool {
        self.swing_left
    }

    pub fn attack_time(&self, partial: f32) -> f32 {
        let mut diff = self.attack_anim - self.attack_anim_old;
        if diff < 0.0 {
            diff += 1.0;
        }
        self.attack_anim_old + diff * partial
    }

    pub fn death_time(&self, partial: f32) -> f32 {
        if self.death_time > 0 {
            self.death_time as f32 + partial
        } else {
            0.0
        }
    }

    pub fn has_red_overlay(&self) -> bool {
        self.hurt_time > 0 || self.death_time > 0
    }

    pub fn body_rot_pair(&self) -> (f32, f32) {
        (self.body_rot_old, self.body_rot)
    }

    pub fn position(&self, partial: f32) -> [f64; 3] {
        std::array::from_fn(|i| lerp_f64(partial, self.pos_old[i], self.pos[i]))
    }
}
