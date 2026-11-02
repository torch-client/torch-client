use super::Tower;
use super::target::Me;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub(super) struct Act {
    pub jump: Option<bool>,
    pub vy: Option<f64>,
}

pub(super) struct State {
    ticks: u32,
    jumped: bool,
    landed: bool,
    rising: bool,
}

impl State {
    pub const IDLE: State = State {
        ticks: 0,
        jumped: false,
        landed: false,
        rising: false,
    };

    pub fn step(&mut self, mode: Tower, me: &Me, locked_y: &mut f64) -> Act {
        if mode == Tower::Off {
            return Act::default();
        }
        if !me.jump {
            *self = State::IDLE;
            return Act::default();
        }
        if !me.on_ground {
            if mode == Tower::Motion && self.rising && me.pos[1] > *locked_y + 1.0 {
                self.rising = false;
                return Act {
                    jump: Some(false),
                    vy: Some(-0.15),
                };
            }
            return Act {
                jump: Some(false),
                vy: None,
            };
        }
        *locked_y = me.pos[1];
        match mode {
            Tower::Off => Act::default(),
            Tower::Jump => Act {
                jump: Some(true),
                vy: None,
            },
            Tower::Motion => {
                self.rising = true;
                Act {
                    jump: Some(false),
                    vy: Some(0.42),
                }
            }
            Tower::Step => {
                let mut jump = false;
                if self.ticks == 0 {
                    jump = true;
                    self.jumped = true;
                    self.landed = false;
                }
                if self.jumped && !self.landed {
                    self.landed = true;
                    self.ticks = 0;
                } else {
                    self.ticks += 1;
                }
                Act {
                    jump: Some(jump),
                    vy: None,
                }
            }
            Tower::Smooth => match self.ticks {
                0 => {
                    self.ticks = 1;
                    Act {
                        jump: Some(false),
                        vy: Some(-0.07),
                    }
                }
                1 => {
                    self.ticks = 2;
                    Act {
                        jump: Some(false),
                        vy: None,
                    }
                }
                _ => {
                    self.ticks = 0;
                    Act {
                        jump: Some(true),
                        vy: None,
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me(y: f64, on_ground: bool, jump: bool) -> Me {
        Me {
            pos: [0.5, y, 0.5],
            vel: [0.0; 3],
            on_ground,
            eye_height: 1.62,
            forward: 0.0,
            strafe: 0.0,
            yaw: 0.0,
            jump,
        }
    }

    #[test]
    fn smooth_jumps_every_third_ground_tick() {
        let mut s = State::IDLE;
        let mut y = 64.0;
        let jumps: Vec<_> = (0..6)
            .map(|_| s.step(Tower::Smooth, &me(64.0, true, true), &mut y).jump)
            .collect();
        assert_eq!(
            jumps,
            [
                Some(false),
                Some(false),
                Some(true),
                Some(false),
                Some(false),
                Some(true)
            ]
        );
    }

    #[test]
    fn motion_launches_then_pulls_down_past_a_block() {
        let mut s = State::IDLE;
        let mut y = 0.0;
        assert_eq!(
            s.step(Tower::Motion, &me(64.0, true, true), &mut y).vy,
            Some(0.42)
        );
        assert_eq!(y, 64.0);
        assert_eq!(
            s.step(Tower::Motion, &me(64.8, false, true), &mut y).vy,
            None
        );
        assert_eq!(
            s.step(Tower::Motion, &me(65.1, false, true), &mut y).vy,
            Some(-0.15)
        );
        assert_eq!(
            s.step(Tower::Motion, &me(65.2, false, true), &mut y).vy,
            None,
            "once a launch"
        );
    }

    #[test]
    fn letting_go_of_jump_hands_it_back() {
        let mut s = State::IDLE;
        let mut y = 0.0;
        assert_eq!(
            s.step(Tower::Jump, &me(64.0, true, false), &mut y),
            Act::default()
        );
        assert_eq!(
            s.step(Tower::Off, &me(64.0, true, true), &mut y),
            Act::default()
        );
    }
}
