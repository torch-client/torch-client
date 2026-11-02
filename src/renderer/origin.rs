use bevy::math::{DVec3, IVec3, Vec3};
use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;

pub const RECENTRE_DISTANCE: f64 = 1024.0;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, ExtractResource)]
pub struct RenderOrigin {
    x: i32,
    z: i32,
}

impl RenderOrigin {
    pub fn follow(&mut self, eye: DVec3) {
        if (eye.x - f64::from(self.x)).abs() <= RECENTRE_DISTANCE
            && (eye.z - f64::from(self.z)).abs() <= RECENTRE_DISTANCE
        {
            return;
        }
        let snap = |v: f64| (v / 16.0).floor() as i32 * 16;
        *self = Self {
            x: snap(eye.x),
            z: snap(eye.z),
        };
    }

    pub fn block(self) -> IVec3 {
        IVec3::new(self.x, 0, self.z)
    }

    pub fn world(self) -> DVec3 {
        DVec3::new(f64::from(self.x), 0.0, f64::from(self.z))
    }

    pub fn to_render(self, p: DVec3) -> Vec3 {
        (p - self.world()).as_vec3()
    }

    pub fn to_render_array(self, p: [f64; 3]) -> Vec3 {
        self.to_render(DVec3::from_array(p))
    }

    pub fn block_to_render(self, b: impl Into<IVec3>) -> Vec3 {
        (b.into() - self.block()).as_vec3()
    }

    pub fn block_centre_to_render(self, b: impl Into<IVec3>) -> Vec3 {
        self.block_to_render(b) + Vec3::splat(0.5)
    }

    pub fn to_world(self, v: Vec3) -> DVec3 {
        v.as_dvec3() + self.world()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_put_inside_the_recentre_distance() {
        let mut origin = RenderOrigin::default();
        let before = origin.block();
        origin.follow(DVec3::new(1000.0, 64.0, -1000.0));
        assert_eq!(origin.block(), before);
    }

    #[test]
    fn recentres_on_the_eye_section() {
        let mut origin = RenderOrigin::default();
        let before = origin.block();
        origin.follow(DVec3::new(30_000_000.7, 64.0, -2_000.2));
        assert_ne!(origin.block(), before);
        assert_eq!(origin.block(), IVec3::new(30_000_000, 0, -2_000 - 16));
    }

    #[test]
    fn far_out_positions_keep_their_fraction() {
        let mut origin = RenderOrigin::default();
        let eye = DVec3::new(29_999_999.3125, 70.5, -29_999_999.6875);
        origin.follow(eye);
        let rel = origin.to_render(eye);
        assert_eq!(rel, Vec3::new(15.3125, 70.5, 0.3125));
        assert_eq!(origin.to_world(rel), eye);
    }

    #[test]
    fn block_corners_are_exact() {
        let mut origin = RenderOrigin::default();
        origin.follow(DVec3::new(16_777_217.0, 0.0, 0.0));
        assert_eq!(
            origin.block_to_render([16_777_219, -64, 5]),
            Vec3::new(3.0, -64.0, 5.0)
        );
        assert_eq!(
            origin.block_centre_to_render([16_777_219, -64, 5]),
            Vec3::new(3.5, -63.5, 5.5)
        );
    }
}
