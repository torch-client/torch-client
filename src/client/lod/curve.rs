use crate::gui::LodQuality;

use super::pos::MAX_DETAIL;

#[derive(Clone, Copy, Debug)]
pub struct Curve {
    unit: f64,
    log_base: f64,
}

impl Curve {
    pub fn new(quality: LodQuality) -> Self {
        let (base, unit) = quality.curve();
        Self {
            unit: f64::from(unit) * 16.0,
            log_base: base.ln(),
        }
    }

    pub fn detail_at(&self, block_distance: f64) -> u8 {
        let detail = ((block_distance / self.unit).ln() / self.log_base) as i32;
        detail.clamp(0, MAX_DETAIL as i32) as u8
    }
}

const MEDIUM_VERTICAL: [usize; 11] = [6, 6, 6, 4, 4, 4, 4, 4, 4, 4, 1];

pub fn max_vertical_slices(detail: u8) -> usize {
    MEDIUM_VERTICAL[(detail as usize).min(MEDIUM_VERTICAL.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medium_steps_where_the_plan_says() {
        let c = Curve::new(LodQuality::Medium);
        assert_eq!(c.detail_at(0.0), 0);
        assert_eq!(c.detail_at(383.0), 0);
        assert_eq!(c.detail_at(385.0), 1);
        assert_eq!(c.detail_at(767.0), 1);
        assert_eq!(c.detail_at(769.0), 2);
        assert_eq!(c.detail_at(3073.0), 4);
        assert_eq!(c.detail_at(1.0e9), MAX_DETAIL);
    }
}
