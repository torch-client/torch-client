use std::sync::Arc;

use crate::util::mth::{lerp_f64, rot_lerp};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Step {
    pub pos: [f64; 3],
    pub movement: [f64; 3],
    pub y_rot: f32,
    pub x_rot: f32,
    pub weight: f32,
}

impl Step {
    pub(crate) const ZERO: Step = Step {
        pos: [0.0; 3],
        movement: [0.0; 3],
        y_rot: 0.0,
        x_rot: 0.0,
        weight: 0.0,
    };
}

#[derive(Clone, Debug)]
pub(crate) struct LerpView {
    pub steps: Arc<[Step]>,
    pub total_weight: f64,
    pub lerp_delay: i32,
    pub old: Step,
}

impl LerpView {
    fn idle() -> LerpView {
        LerpView {
            steps: Arc::from([]),
            total_weight: 0.0,
            lerp_delay: 0,
            old: Step::ZERO,
        }
    }

    pub(crate) fn active(&self) -> bool {
        !self.steps.is_empty()
    }

    pub(crate) fn sample(&self, partial: f32) -> Option<Step> {
        if !self.active() {
            return None;
        }
        let alpha = ((3 - self.lerp_delay) as f32 + partial) / 3.0;
        let mut count_up = 0.0f32;
        let mut in_step = 1.0f32;
        let mut index = self.steps.len() - 1;
        for (i, step) in self.steps.iter().enumerate() {
            let weight = step.weight;
            if weight <= 0.0 {
                continue;
            }
            count_up += weight;
            if f64::from(count_up) >= self.total_weight * f64::from(alpha) {
                let before = count_up - weight;
                in_step = ((f64::from(alpha) * self.total_weight - f64::from(before))
                    / f64::from(weight)) as f32;
                index = i;
                break;
            }
        }
        let current = self.steps[index];
        let previous = if index > 0 {
            self.steps[index - 1]
        } else {
            self.old
        };
        let lerp3 =
            |a: [f64; 3], b: [f64; 3]| std::array::from_fn(|i| lerp_f64(in_step, a[i], b[i]));
        Some(Step {
            pos: lerp3(previous.pos, current.pos),
            movement: lerp3(previous.movement, current.movement),
            y_rot: rot_lerp(in_step, previous.y_rot, current.y_rot),
            x_rot: rot_lerp(in_step, previous.x_rot, current.x_rot),
            weight: 0.0,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CartRide {
    pub view: LerpView,
    pub pos_old: [f64; 3],
    pub pos: [f64; 3],
}

impl CartRide {
    pub(crate) fn passenger_offset(&self, partial: f32) -> Option<[f64; 3]> {
        let at = self.view.sample(partial)?.pos;
        Some(std::array::from_fn(|i| {
            at[i] - lerp_f64(partial, self.pos_old[i], self.pos[i])
        }))
    }
}

const MAX_QUEUED: usize = 512;

#[derive(Clone, Debug)]
pub(crate) struct CartLerp {
    queued: Vec<Step>,
    view: LerpView,
}

impl Default for CartLerp {
    fn default() -> Self {
        CartLerp {
            queued: Vec::new(),
            view: LerpView::idle(),
        }
    }
}

impl CartLerp {
    pub(crate) fn push(&mut self, steps: impl IntoIterator<Item = Step>) {
        self.queued.extend(steps);
        let over = self.queued.len().saturating_sub(MAX_QUEUED);
        self.queued.drain(..over);
    }

    pub(crate) fn view(&self) -> &LerpView {
        &self.view
    }

    pub(crate) fn tick(&mut self, entity: Step) -> Option<Step> {
        self.view.lerp_delay = self.view.lerp_delay.saturating_sub(1);
        if self.view.lerp_delay <= 0 {
            self.view.old = Step {
                weight: 0.0,
                ..entity
            };
            if self.queued.is_empty() {
                if self.view.active() {
                    self.view.steps = Arc::from([]);
                }
            } else {
                self.view.total_weight = self.queued.iter().map(|s| f64::from(s.weight)).sum();
                self.view.steps = Arc::from(std::mem::take(&mut self.queued));
                self.view.lerp_delay = if self.view.total_weight == 0.0 { 0 } else { 3 };
            }
        }
        self.view.sample(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, weight: f32) -> Step {
        Step {
            pos: [x, 0.0, 0.0],
            weight,
            ..Step::ZERO
        }
    }

    #[test]
    fn idle_cart_is_left_alone() {
        let mut cart = CartLerp::default();
        assert_eq!(cart.tick(at(5.0, 0.0)), None);
        assert!(!cart.view().active());
    }

    #[test]
    fn one_step_is_walked_over_three_ticks() {
        let mut cart = CartLerp::default();
        cart.push([at(3.0, 1.0)]);
        let x = |s: Option<Step>| s.unwrap().pos[0];
        assert!((x(cart.tick(at(0.0, 0.0))) - 1.0).abs() < 1.0e-6);
        assert!((x(cart.tick(at(1.0, 0.0))) - 2.0).abs() < 1.0e-6);
        assert!((x(cart.tick(at(2.0, 0.0))) - 3.0).abs() < 1.0e-6);
        assert_eq!(cart.tick(at(3.0, 0.0)), None);
    }

    #[test]
    fn steps_are_walked_by_weight() {
        let mut cart = CartLerp::default();
        cart.push([at(3.0, 3.0), at(4.0, 1.0)]);
        cart.tick(at(0.0, 0.0));
        let view = cart.view().clone();
        let p = view.sample(0.75).unwrap();
        assert!((p.pos[0] - 1.0).abs() < 1.0e-5, "{p:?}");
    }

    #[test]
    fn zero_weight_packet_snaps_to_its_last_step() {
        let mut cart = CartLerp::default();
        cart.push([at(7.0, 0.0)]);
        let s = cart.tick(at(0.0, 0.0)).unwrap();
        assert_eq!(s.pos[0], 7.0);
        assert_eq!(cart.view().lerp_delay, 0);
    }
}
