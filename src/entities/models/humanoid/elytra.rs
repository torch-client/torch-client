use crate::entities::geom::{CubeList, Grow, LayerDef, MeshDef, PartPose};

pub const DEFAULT_X_ROT: f32 = 0.261_799_4;
pub const DEFAULT_Z_ROT: f32 = -0.261_799_4;

pub fn layer() -> LayerDef {
    LayerDef::create(offset(mesh()), 64, 32)
}

pub fn baby_layer() -> LayerDef {
    LayerDef::create(offset(mesh().transformed(|pose| pose.scaled(0.5))), 64, 32)
}

fn offset(mesh: MeshDef) -> MeshDef {
    mesh.transformed(|pose| pose.translated(0.0, 0.0, 2.0))
}

pub fn tick_angles(angles: &mut [f32; 3], fall_flying: bool, crouching: bool, delta: [f64; 3]) {
    const FLYING_X: [f32; 2] = [DEFAULT_X_ROT, 0.349_065_84];
    const FLYING_Z: [f32; 2] = [DEFAULT_Z_ROT, -1.570_796_4];
    const CROUCH: [f32; 3] = [0.698_131_7, 0.087_266_46, -0.785_398_2];

    let target = if fall_flying {
        let ratio = if delta[1] < 0.0 {
            let len = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
            let down = if len > 0.0 { -delta[1] / len } else { 0.0 };
            1.0 - (down.powf(1.5) as f32)
        } else {
            1.0
        };
        let lerp = |from: f32, to: f32| from + (to - from) * ratio;
        [
            lerp(FLYING_X[0], FLYING_X[1]),
            0.0,
            lerp(FLYING_Z[0], FLYING_Z[1]),
        ]
    } else if crouching {
        CROUCH
    } else {
        [DEFAULT_X_ROT, 0.0, DEFAULT_Z_ROT]
    };

    for axis in 0..3 {
        angles[axis] += (target[axis] - angles[axis]) * 0.3;
    }
}

pub fn wing_angles(angles: [f32; 3], crouching: bool) -> ([f32; 4], [f32; 4]) {
    let y = if crouching { 3.0 } else { 0.0 };
    let [x_rot, y_rot, z_rot] = angles;
    ([y, x_rot, y_rot, z_rot], [y, x_rot, -y_rot, -z_rot])
}

fn mesh() -> MeshDef {
    const WIND: Grow = Grow::all(1.0);

    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "left_wing",
        CubeList::new()
            .tex_offs(22, 0)
            .add_box_grow(-10.0, 0.0, 0.0, 10.0, 20.0, 2.0, WIND),
        PartPose::offset_rotation(5.0, 0.0, 0.0, DEFAULT_X_ROT, 0.0, DEFAULT_Z_ROT),
    );
    root.child(
        "right_wing",
        CubeList::new()
            .tex_offs(22, 0)
            .mirror()
            .add_box_grow(0.0, 0.0, 0.0, 10.0, 20.0, 2.0, WIND),
        PartPose::offset_rotation(-5.0, 0.0, 0.0, DEFAULT_X_ROT, 0.0, -DEFAULT_Z_ROT),
    );
    mesh
}
