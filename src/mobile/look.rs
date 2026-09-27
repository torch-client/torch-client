use bevy::math::Vec2;

#[derive(Clone, Copy, Debug)]
pub struct MobileConfig {
    pub drag_threshold: f32,
    pub hold_secs: f32,
    pub look_deg_per_unit: f32,
    pub double_tap_secs: f32,
    pub double_tap_slop: f32,
    pub safe_inset: f32,
}

impl Default for MobileConfig {
    fn default() -> Self {
        MobileConfig {
            drag_threshold: 4.0,
            hold_secs: 0.25,
            look_deg_per_unit: 0.45,
            double_tap_secs: 7.0 / 20.0,
            double_tap_slop: 12.0,
            safe_inset: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LookTrack {
    pub start: Vec2,
    pub last: Vec2,
    pub start_time: f32,
    pub max_dist: f32,
}

impl LookTrack {
    pub fn new(pos: Vec2, now: f32) -> Self {
        LookTrack {
            start: pos,
            last: pos,
            start_time: now,
            max_dist: 0.0,
        }
    }

    pub fn advance(&mut self, pos: Vec2) -> Vec2 {
        let delta = pos - self.last;
        self.last = pos;
        self.max_dist = self.max_dist.max(pos.distance(self.start));
        delta
    }

    pub fn attacking(&self, now: f32, cfg: &MobileConfig) -> bool {
        now - self.start_time >= cfg.hold_secs
    }

    pub fn progress(&self, now: f32, cfg: &MobileConfig) -> f32 {
        ((now - self.start_time) / cfg.hold_secs).clamp(0.0, 1.0)
    }

    pub fn is_tap(&self, now: f32, cfg: &MobileConfig) -> bool {
        !self.attacking(now, cfg) && self.max_dist <= cfg.drag_threshold
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LastTap {
    pub pos: Vec2,
    pub time: f32,
}

impl LastTap {
    pub fn doubles(&self, pos: Vec2, now: f32, cfg: &MobileConfig) -> bool {
        now - self.time <= cfg.double_tap_secs && pos.distance(self.pos) <= cfg.double_tap_slop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> MobileConfig {
        MobileConfig::default()
    }

    #[test]
    fn a_prompt_second_tap_nearby_doubles() {
        let c = cfg();
        let first = LastTap {
            pos: Vec2::new(100.0, 100.0),
            time: 1.0,
        };
        assert!(first.doubles(Vec2::new(103.0, 102.0), 1.0 + c.double_tap_secs, &c));
    }

    #[test]
    fn a_late_or_distant_second_tap_does_not_double() {
        let c = cfg();
        let first = LastTap {
            pos: Vec2::new(100.0, 100.0),
            time: 1.0,
        };
        assert!(!first.doubles(Vec2::new(100.0, 100.0), 1.0 + c.double_tap_secs + 0.01, &c));
        assert!(!first.doubles(Vec2::new(100.0 + c.double_tap_slop + 1.0, 100.0), 1.05, &c));
    }

    #[test]
    fn small_movement_then_release_is_a_tap() {
        let c = cfg();
        let mut t = LookTrack::new(Vec2::new(100.0, 100.0), 0.0);
        t.advance(Vec2::new(101.0, 100.0));
        t.advance(Vec2::new(102.0, 100.0));
        assert!(t.is_tap(0.1, &c));
        assert!(!t.attacking(0.1, &c));
    }

    #[test]
    fn a_quick_drag_is_not_a_tap() {
        let c = cfg();
        let mut t = LookTrack::new(Vec2::new(100.0, 100.0), 0.0);
        t.advance(Vec2::new(120.0, 100.0));
        assert!(!t.is_tap(0.05, &c));
        t.advance(Vec2::new(100.0, 100.0));
        assert!(!t.is_tap(0.1, &c));
    }

    #[test]
    fn holding_attacks_whether_or_not_it_moved() {
        let c = cfg();
        let still = LookTrack::new(Vec2::new(100.0, 100.0), 1.0);
        assert!(!still.attacking(1.0 + c.hold_secs - 0.01, &c));
        assert!(still.attacking(1.0 + c.hold_secs, &c));

        let mut dragging = LookTrack::new(Vec2::new(100.0, 100.0), 1.0);
        for i in 1..20 {
            dragging.advance(Vec2::new(100.0 + i as f32 * 6.0, 100.0));
        }
        assert!(dragging.max_dist > c.drag_threshold);
        assert!(dragging.attacking(1.0 + c.hold_secs, &c));
    }

    #[test]
    fn a_release_after_attacking_is_not_a_tap() {
        let c = cfg();
        let t = LookTrack::new(Vec2::new(100.0, 100.0), 0.0);
        assert!(t.attacking(c.hold_secs, &c));
        assert!(!t.is_tap(c.hold_secs, &c));
    }

    #[test]
    fn progress_fills_over_the_hold_window() {
        let c = cfg();
        let t = LookTrack::new(Vec2::ZERO, 2.0);
        assert_eq!(t.progress(2.0, &c), 0.0);
        assert_eq!(t.progress(2.0 + c.hold_secs / 2.0, &c), 0.5);
        assert_eq!(t.progress(2.0 + c.hold_secs, &c), 1.0);
        assert_eq!(t.progress(60.0, &c), 1.0);
    }

    #[test]
    fn advance_reports_frame_delta_and_tracks_peak_distance() {
        let mut t = LookTrack::new(Vec2::ZERO, 0.0);
        assert_eq!(t.advance(Vec2::new(3.0, 0.0)), Vec2::new(3.0, 0.0));
        assert_eq!(t.advance(Vec2::new(3.0, 4.0)), Vec2::new(0.0, 4.0));
        assert_eq!(t.max_dist, 5.0);
        t.advance(Vec2::ZERO);
        assert_eq!(t.max_dist, 5.0);
    }
}
