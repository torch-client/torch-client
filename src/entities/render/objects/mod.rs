use bevy::math::{Mat4, Vec3};
use bevy::prelude::Transform;

use crate::entities::MODEL_Y_OFFSET;
use crate::entities::registry::Registry;

pub mod blocks;
pub mod boat;
pub mod frame;
pub mod minecart;
pub mod misc;
pub mod projectile;
pub mod thrown;

pub fn register(registry: &mut Registry) {
    boat::register(registry);
    minecart::register(registry);
    projectile::register(registry);
    misc::register(registry);
    blocks::register(registry);
    frame::register(registry);
    thrown::register(registry);
}

pub(crate) fn mirror() -> Mat4 {
    Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
}

pub(crate) fn hook_for(after: Mat4) -> Transform {
    let origin_inverse = Mat4::from_translation(Vec3::new(0.0, -MODEL_Y_OFFSET, 0.0));
    Transform::from_matrix(mirror() * after * origin_inverse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_cancels_the_fixed_chain_nodes() {
        let hook = hook_for(Mat4::IDENTITY);
        let chain = mirror()
            * hook.to_matrix()
            * Mat4::from_translation(Vec3::new(0.0, MODEL_Y_OFFSET, 0.0));
        let moved = chain.transform_point3(Vec3::new(1.0, 2.0, 3.0));
        assert!(
            (moved - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5,
            "{moved:?}"
        );
    }

    #[test]
    fn hook_reproduces_a_mirror_and_a_quarter_turn() {
        let after = mirror() * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let hook = hook_for(after);
        let chain = mirror()
            * hook.to_matrix()
            * Mat4::from_translation(Vec3::new(0.0, MODEL_Y_OFFSET, 0.0));
        let point = Vec3::new(1.0, 2.0, 3.0);
        let want = after.transform_point3(point);
        let got = chain.transform_point3(point);
        assert!((want - got).length() < 1e-5, "{want:?} vs {got:?}");
    }
}
