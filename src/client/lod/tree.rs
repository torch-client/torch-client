use std::collections::HashSet;

use super::curve::Curve;
use super::pos::{MAX_DETAIL, SectionPos};
use super::store::{Store, nearest_distance};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RealArea {
    pub eye: [f64; 3],
    pub radius: f64,
    pub min_y: i32,
    pub max_y: i32,
}

impl RealArea {
    fn covers(self, pos: SectionPos) -> bool {
        let x0 = f64::from(pos.min_block_x());
        let z0 = f64::from(pos.min_block_z());
        let w = f64::from(pos.block_width());
        let far = |lo: f64, c: f64| (lo - c).abs().max((lo + w - c).abs());
        let (dx, dz) = (far(x0, self.eye[0]), far(z0, self.eye[2]));
        (dx * dx + dz * dz).sqrt() <= self.radius
            && self.eye[1] - f64::from(self.min_y) <= self.radius
            && f64::from(self.max_y) - self.eye[1] <= self.radius
    }
}

pub fn select(
    store: &Store,
    eye: (f64, f64),
    radius: f64,
    real: RealArea,
    curve: &Curve,
) -> HashSet<SectionPos> {
    let mut wanted = HashSet::new();
    let lo = SectionPos::containing(
        MAX_DETAIL,
        (eye.0 - radius).floor() as i32,
        (eye.1 - radius).floor() as i32,
    );
    let hi = SectionPos::containing(
        MAX_DETAIL,
        (eye.0 + radius).ceil() as i32,
        (eye.1 + radius).ceil() as i32,
    );
    for x in lo.x..=hi.x {
        for z in lo.z..=hi.z {
            visit(
                store,
                SectionPos::new(MAX_DETAIL, x, z),
                eye,
                radius,
                real,
                curve,
                &mut wanted,
            );
        }
    }
    wanted
}

fn visit(
    store: &Store,
    pos: SectionPos,
    eye: (f64, f64),
    radius: f64,
    real: RealArea,
    curve: &Curve,
    wanted: &mut HashSet<SectionPos>,
) {
    if nearest_distance(pos, eye) > radius || real.covers(pos) || !store.has_data(pos) {
        return;
    }
    let (cx, cz) = pos.center_block();
    let distance = ((cx - eye.0).powi(2) + (cz - eye.1).powi(2)).sqrt();
    let expected = curve.detail_at(distance);
    if pos.detail > expected {
        for i in 0..4 {
            visit(store, pos.child(i), eye, radius, real, curve, wanted);
        }
    } else {
        wanted.insert(pos);
    }
}
