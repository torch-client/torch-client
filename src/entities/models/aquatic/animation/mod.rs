use crate::entities::keyframe::DEG_TO_RAD;

pub mod baby_axolotl;
pub mod bat;
pub mod frog;
pub mod nautilus;

const fn degree_vec(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x * DEG_TO_RAD, y * DEG_TO_RAD, z * DEG_TO_RAD]
}

const fn pos_vec(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x, -y, z]
}

const fn scale_vec(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x - 1.0, y - 1.0, z - 1.0]
}
