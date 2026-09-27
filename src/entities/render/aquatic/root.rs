use bevy::prelude::*;

use crate::entities::{RootPose, setup_rotations};

pub fn then_rotate(pose: &mut RootPose, q: Quat) {
    pose.extra_rotation *= q;
}

pub fn then_translate(pose: &mut RootPose, v: Vec3) {
    pose.extra_offset += pose.extra_rotation * v;
}

pub fn fish_root(
    st: &crate::entities::EntityState,
    amplitude: f32,
    angle: f32,
    beached_shove: Vec3,
) -> RootPose {
    let mut pose = setup_rotations(st, 90.0);
    let body_z_rot = amplitude * 4.3 * (angle * 0.6 * st.age_ticks).sin();
    then_rotate(&mut pose, Quat::from_rotation_y(body_z_rot.to_radians()));
    if !st.is_in_water {
        then_translate(&mut pose, beached_shove);
        then_rotate(
            &mut pose,
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        );
    }
    pose
}
