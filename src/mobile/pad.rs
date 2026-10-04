use bevy::math::Vec2;

use crate::gui::hud_layout::{ElementId, Transforms};
use crate::gui::screens::HotbarGeom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    SneakToggle,
    Forward,
    SprintToggle,
    Left,
    Jump,
    Right,
    Inventory,
    Back,
    Pause,
}

impl Cell {
    pub const ALL: [Cell; 9] = [
        Cell::SneakToggle,
        Cell::Forward,
        Cell::SprintToggle,
        Cell::Left,
        Cell::Jump,
        Cell::Right,
        Cell::Inventory,
        Cell::Back,
        Cell::Pause,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Cell::SneakToggle => "SNK",
            Cell::Forward => "^",
            Cell::SprintToggle => "SPR",
            Cell::Left => "<",
            Cell::Jump => "JMP",
            Cell::Right => ">",
            Cell::Inventory => "INV",
            Cell::Back => "v",
            Cell::Pause => "ESC",
        }
    }
}

pub use crate::gui::TouchMovement as Movement;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
    Pad(Cell),
    Stick,
    Use,
    Hotbar(u8),
    Chat,
    Look,
    LookOnly,
}

#[derive(Clone, Copy, Debug)]
pub struct NoAttackZone {
    pub lower: (Vec2, f32),
    pub upper: (Vec2, f32),
}

impl NoAttackZone {
    fn around(use_button: Rect) -> Self {
        let centre = use_button.center();
        let r = use_button.w * NO_ATTACK_RADIUS;
        NoAttackZone {
            lower: (centre, r),
            upper: (
                centre - Vec2::new(0.0, use_button.h * NO_ATTACK_UPPER_OFFSET),
                r * NO_ATTACK_UPPER_WEIGHT,
            ),
        }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        [self.lower, self.upper]
            .iter()
            .any(|&(c, r)| p.distance(c) <= r)
    }
}

const NO_ATTACK_RADIUS: f32 = 1.0;
const NO_ATTACK_UPPER_OFFSET: f32 = 1.5;
const NO_ATTACK_UPPER_WEIGHT: f32 = 0.75;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Stick {
    pub center: Vec2,
    pub radius: f32,
    pub travel: f32,
}

impl Stick {
    pub fn knob(&self, at: Vec2) -> Vec2 {
        let offset = at - self.center;
        let len = offset.length();
        if len <= self.radius || len == 0.0 {
            self.center + offset
        } else {
            self.center + offset / len * self.radius
        }
    }

    pub fn direction(&self, at: Vec2) -> Option<(i8, i8)> {
        let offset = at - self.center;
        let len = offset.length();
        if len < self.travel * STICK_DEADZONE {
            return None;
        }
        let unit = offset / len;
        const SECTOR: f32 = 0.382_683_43;
        let sign = |v: f32| {
            if v > SECTOR {
                1
            } else if v < -SECTOR {
                -1
            } else {
                0
            }
        };
        Some((sign(unit.x), sign(unit.y)))
    }
}

pub fn top_controls_bottom(vw: f32, vh: f32) -> f32 {
    let close = Layout::new(vw, vh, 0.0, Movement::Buttons).close_button;
    close.y + close.h
}

pub struct Layout {
    pub cell: f32,
    pub pad: Vec2,
    pub movement: Movement,
    pub stick: Option<Stick>,
    pub jump_button: Option<Rect>,
    pub round_cells: Option<[(Cell, Vec2); 4]>,
    pub use_button: Rect,
    pub close_button: Rect,
    pub texting_button: Rect,
    pub no_attack: NoAttackZone,
    hotbar: HotbarGeom,
    vh: f32,
}

const CELL_FRACTION: f32 = 0.16;
const CELL_MIN: f32 = 18.0;
const CELL_MAX: f32 = 44.0;
const MARGIN_FRACTION: f32 = 0.04;
const HOTBAR_H: f32 = 22.0;
const USE_SCALE: f32 = 1.5;
const STICK_RADIUS: f32 = 1.2;
const STICK_TRAVEL_ROUND: f32 = STICK_RADIUS;
const STICK_TRAVEL_PAD: f32 = 1.0;
const STICK_DEADZONE: f32 = 0.3;
const ROUND_CELL_OFFSET: f32 = 1.25;

impl Layout {
    pub fn new(vw: f32, vh: f32, safe_inset: f32, movement: Movement) -> Self {
        let short = vw.min(vh);
        let cell = (short * CELL_FRACTION).clamp(CELL_MIN, CELL_MAX);
        let margin = short * MARGIN_FRACTION + safe_inset;
        let bottom = vh - HOTBAR_H - margin;
        let use_size = cell * USE_SCALE;
        let pad = Vec2::new(margin, bottom - cell * 3.0);
        let center = pad + Vec2::splat(cell * 1.5);
        let use_button = Rect {
            x: vw - margin - use_size,
            y: bottom - use_size,
            w: use_size,
            h: use_size,
        };

        let stick = match movement {
            Movement::Buttons => None,
            Movement::Joystick => Some(Stick {
                center,
                radius: cell * STICK_RADIUS,
                travel: cell * STICK_TRAVEL_ROUND,
            }),
            Movement::PadStick => Some(Stick {
                center,
                radius: cell * 0.5,
                travel: cell * STICK_TRAVEL_PAD,
            }),
        };

        let jump_button = movement.has_stick().then(|| Rect {
            x: use_button.x - cell * 1.2,
            y: bottom - cell,
            w: cell,
            h: cell,
        });

        let round_cells = (movement == Movement::Joystick).then(|| {
            let d = cell * ROUND_CELL_OFFSET;
            [
                (Cell::SneakToggle, center + Vec2::new(-d, -d)),
                (Cell::SprintToggle, center + Vec2::new(d, -d)),
                (Cell::Inventory, center + Vec2::new(-d, d)),
                (Cell::Pause, center + Vec2::new(d, d)),
            ]
        });

        Layout {
            cell,
            pad,
            movement,
            stick,
            jump_button,
            round_cells,
            use_button,
            close_button: Rect {
                x: vw - margin - cell,
                y: margin,
                w: cell,
                h: cell,
            },
            texting_button: Rect {
                x: vw - margin - cell * 2.0 - margin,
                y: margin,
                w: cell,
                h: cell,
            },
            no_attack: NoAttackZone::around(use_button),
            hotbar: HotbarGeom::new(vw, vh),
            vh,
        }
    }

    pub fn cell_rect(&self, i: usize) -> Rect {
        Rect {
            x: self.pad.x + (i % 3) as f32 * self.cell,
            y: self.pad.y + (i / 3) as f32 * self.cell,
            w: self.cell,
            h: self.cell,
        }
    }

    pub fn hotbar_rect(&self, i: usize) -> Rect {
        let (x, y, w, _) = self.hotbar.slot_rect(i);
        Rect {
            x,
            y,
            w,
            h: self.vh - y,
        }
    }

    #[cfg(test)]
    pub fn hit(&self, p: Vec2) -> Control {
        self.hit_in(p, &Transforms::IDENTITY)
    }

    pub fn hit_in(&self, screen: Vec2, xf: &Transforms) -> Control {
        if let Some(jump) = self.jump_button
            && jump.contains(xf.to_local(ElementId::JumpButton, screen))
        {
            return Control::Pad(Cell::Jump);
        }
        let p = xf.to_local(ElementId::Pad, screen);
        if let Some(round) = self.round_cells {
            for (cell, at) in round {
                if p.distance(at) <= self.cell * 0.5 {
                    return Control::Pad(cell);
                }
            }
        }
        if let Some(stick) = self.stick
            && p.distance(stick.center) <= stick.radius
        {
            return Control::Stick;
        }
        if self.movement == Movement::Buttons {
            for (i, cell) in Cell::ALL.iter().enumerate() {
                if self.cell_rect(i).contains(p) {
                    return Control::Pad(*cell);
                }
            }
        } else if self.movement == Movement::PadStick {
            for (i, cell) in Cell::ALL.iter().enumerate() {
                if matches!(
                    cell,
                    Cell::SneakToggle | Cell::SprintToggle | Cell::Inventory | Cell::Pause
                ) && self.cell_rect(i).contains(p)
                {
                    return Control::Pad(*cell);
                }
            }
        }
        if self
            .use_button
            .contains(xf.to_local(ElementId::UseButton, screen))
        {
            return Control::Use;
        }
        if self
            .close_button
            .contains(xf.to_local(ElementId::ChatButton, screen))
        {
            return Control::Chat;
        }
        let p = xf.to_local(ElementId::Hotbar, screen);
        for i in (0..9).filter(|_| xf.drawn(ElementId::Hotbar)) {
            if self.hotbar_rect(i).contains(p) {
                return Control::Hotbar(i as u8);
            }
        }
        if self
            .no_attack
            .contains(xf.to_local(ElementId::UseButton, screen))
        {
            return Control::LookOnly;
        }
        Control::Look
    }
}

#[derive(Default)]
pub struct PadState {
    pub sneak: bool,
    pub sprint: bool,
    pub auto_walk: bool,
    last_forward: Option<f32>,
    last_jump: Option<f32>,
}

impl PadState {
    pub fn press(&mut self, cell: Cell, now: f32, double_tap_secs: f32) {
        match cell {
            Cell::SneakToggle => self.sneak = !self.sneak,
            Cell::SprintToggle => self.sprint = !self.sprint,
            Cell::Forward => {
                let doubled = self
                    .last_forward
                    .is_some_and(|t| now - t <= double_tap_secs);
                if self.auto_walk {
                    self.auto_walk = false;
                    self.last_forward = None;
                } else if doubled {
                    self.auto_walk = true;
                    self.last_forward = None;
                } else {
                    self.last_forward = Some(now);
                }
            }
            _ => {}
        }
    }

    pub fn jump_double_tap(&mut self, now: f32, double_tap_secs: f32) -> bool {
        let doubled = self.last_jump.is_some_and(|t| now - t <= double_tap_secs);
        self.last_jump = if doubled { None } else { Some(now) };
        doubled
    }

    pub fn latched(&self, cell: Cell) -> bool {
        match cell {
            Cell::SneakToggle => self.sneak,
            Cell::SprintToggle => self.sprint,
            Cell::Forward => self.auto_walk,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VW: f32 = 427.0;
    const VH: f32 = 240.0;

    fn layout() -> Layout {
        Layout::new(VW, VH, 0.0, Movement::Buttons)
    }

    #[test]
    fn every_cell_hits_its_own_control() {
        let l = layout();
        for (i, cell) in Cell::ALL.iter().enumerate() {
            let r = l.cell_rect(i);
            let centre = Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0);
            assert_eq!(l.hit(centre), Control::Pad(*cell), "cell {i}");
        }
    }

    #[test]
    fn cell_boundaries_are_half_open() {
        let l = layout();
        let r = l.cell_rect(4);
        assert_eq!(l.hit(Vec2::new(r.x, r.y)), Control::Pad(Cell::Jump));
        assert_eq!(
            l.hit(Vec2::new(r.x + r.w, r.y)),
            Control::Pad(Cell::SprintToggle)
        );
        assert_eq!(l.hit(Vec2::new(r.x, r.y + r.h)), Control::Pad(Cell::Back));
        assert_eq!(
            l.hit(Vec2::new(r.x - 0.01, r.y)),
            Control::Pad(Cell::Forward)
        );
    }

    #[test]
    fn outside_every_control_is_a_look_gesture() {
        let l = layout();
        assert_eq!(l.hit(Vec2::new(VW / 2.0, VH / 3.0)), Control::Look);
        assert_eq!(
            l.hit(Vec2::new(l.pad.x + 1.0, l.pad.y - 1.0)),
            Control::Look
        );
        assert_eq!(
            l.hit(Vec2::new(l.pad.x + l.cell * 3.0 + 1.0, l.pad.y + 1.0)),
            Control::Look
        );
    }

    #[test]
    fn the_teardrop_round_use_looks_without_attacking() {
        for movement in [Movement::Buttons, Movement::Joystick, Movement::PadStick] {
            let l = Layout::new(VW, VH, 0.0, movement);
            let u = l.use_button;
            let c = u.center();
            assert_eq!(l.hit(Vec2::new(c.x, u.y - 2.0)), Control::LookOnly);
            let (upper, r) = l.no_attack.upper;
            assert_eq!(l.hit(upper), Control::LookOnly);
            assert_eq!(l.hit(upper - Vec2::new(0.0, r - 1.0)), Control::LookOnly);
            assert_eq!(l.hit(upper - Vec2::new(0.0, r + 1.0)), Control::Look);
            assert_eq!(l.hit(Vec2::new(VW / 2.0, VH / 3.0)), Control::Look);
            assert_eq!(l.hit(u.center()), Control::Use);
            if let Some(j) = l.jump_button {
                assert_eq!(l.hit(j.center()), Control::Pad(Cell::Jump));
            }
        }
    }

    #[test]
    fn use_button_and_hotbar_are_hittable() {
        let l = layout();
        let u = l.use_button;
        assert_eq!(
            l.hit(Vec2::new(u.x + u.w / 2.0, u.y + u.h / 2.0)),
            Control::Use
        );
        for i in 0..9u8 {
            let r = l.hotbar_rect(i as usize);
            assert_eq!(
                l.hit(Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)),
                Control::Hotbar(i)
            );
            assert_eq!(
                l.hit(Vec2::new(r.x + r.w / 2.0, VH - 0.5)),
                Control::Hotbar(i)
            );
        }
    }

    #[test]
    fn pad_clears_the_hotbar_at_every_size() {
        for (vw, vh) in [
            (427.0, 240.0),
            (320.0, 240.0),
            (640.0, 360.0),
            (240.0, 427.0),
            (1920.0, 1080.0),
        ] {
            let l = Layout::new(vw, vh, 0.0, Movement::Buttons);
            let bottom_row = l.cell_rect(6);
            let (hx, hy, hw, _) = HotbarGeom::new(vw, vh).slot_rect(0);
            let pad_right = l.pad.x + l.cell * 3.0;
            let vertical_clear = bottom_row.y + bottom_row.h <= hy;
            let horizontal_clear = pad_right <= hx || l.pad.x >= hx + hw * 9.0;
            assert!(
                vertical_clear || horizontal_clear,
                "pad overlaps the hotbar at {vw}x{vh}"
            );
            assert!(l.use_button.x + l.use_button.w <= vw);
            assert!(l.pad.y >= 0.0);
        }
    }

    #[test]
    fn toggles_latch_and_unlatch() {
        let mut pad = PadState::default();
        assert!(!pad.sneak && !pad.sprint);
        pad.press(Cell::SneakToggle, 0.0, 0.35);
        assert!(pad.sneak && !pad.sprint);
        pad.press(Cell::SneakToggle, 1.0, 0.35);
        assert!(!pad.sneak);
        pad.press(Cell::SprintToggle, 2.0, 0.35);
        pad.press(Cell::SprintToggle, 3.0, 0.35);
        pad.press(Cell::SprintToggle, 4.0, 0.35);
        assert!(pad.sprint);
        assert!(pad.latched(Cell::SprintToggle) && !pad.latched(Cell::SneakToggle));
        pad.press(Cell::Jump, 5.0, 0.35);
        assert!(!pad.sneak && pad.sprint && !pad.auto_walk);
    }

    #[test]
    fn double_tap_forward_latches_auto_walk() {
        let mut pad = PadState::default();
        pad.press(Cell::Forward, 0.0, 0.35);
        assert!(!pad.auto_walk);
        pad.press(Cell::Forward, 0.3, 0.35);
        assert!(pad.auto_walk);
        assert!(pad.latched(Cell::Forward));

        let mut slow = PadState::default();
        for i in 0..5 {
            slow.press(Cell::Forward, i as f32, 0.35);
            assert!(!slow.auto_walk, "press {i} should not latch");
        }
    }

    #[test]
    fn any_forward_tap_cancels_auto_walk() {
        let mut pad = PadState::default();
        pad.press(Cell::Forward, 0.0, 0.35);
        pad.press(Cell::Forward, 0.2, 0.35);
        assert!(pad.auto_walk);
        pad.press(Cell::Forward, 9.0, 0.35);
        assert!(!pad.auto_walk);

        pad.press(Cell::Forward, 20.0, 0.35);
        pad.press(Cell::Forward, 20.2, 0.35);
        assert!(pad.auto_walk);
        pad.press(Cell::Forward, 20.4, 0.35);
        assert!(!pad.auto_walk);
        pad.press(Cell::Forward, 20.6, 0.35);
        assert!(
            !pad.auto_walk,
            "the pair that stopped it must not restart it"
        );
    }

    #[test]
    fn jump_double_tap_fires_only_on_the_second_close_press() {
        let mut pad = PadState::default();
        assert!(!pad.jump_double_tap(0.0, 0.35));
        assert!(pad.jump_double_tap(0.3, 0.35));

        let mut slow = PadState::default();
        for i in 0..5 {
            assert!(
                !slow.jump_double_tap(i as f32, 0.35),
                "press {i} should not double"
            );
        }
    }

    #[test]
    fn texting_button_sits_left_of_close() {
        for (vw, vh) in [(VW, VH), (VH, VW)] {
            let l = Layout::new(vw, vh, 0.0, Movement::Buttons);
            let (s, c) = (l.texting_button, l.close_button);
            assert!(s.x + s.w < c.x);
            assert_eq!((s.y, s.h), (c.y, c.h));
            assert!(s.x > 0.0);
        }
    }

    #[test]
    fn close_button_sits_clear_of_the_pad_and_the_hotbar() {
        for (vw, vh) in [(427.0, 240.0), (320.0, 240.0), (240.0, 427.0)] {
            let l = Layout::new(vw, vh, 0.0, Movement::Buttons);
            let c = l.close_button;
            assert_eq!(c.w, l.cell);
            assert!(c.x + c.w <= vw && c.y >= 0.0);
            assert!(c.y + c.h <= l.pad.y);
            assert!(c.x >= l.pad.x + l.cell * 3.0);
        }
    }

    #[test]
    fn close_button_square_opens_chat_when_no_screen_is_open() {
        let l = layout();
        let c = l.close_button;
        assert_eq!(l.hit(Vec2::new(c.x + 1.0, c.y + 1.0)), Control::Chat);
    }
}
