use crate::entities::geom::{BakedModel, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

pub fn outer_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().part(
        "cube",
        (0, 0),
        [-4.0, 16.0, -4.0, 8.0, 8.0, 8.0],
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn inner_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.part(
        "cube",
        (0, 16),
        [-3.0, 17.0, -3.0, 6.0, 6.0, 6.0],
        PartPose::ZERO,
    );
    root.part(
        "right_eye",
        (32, 0),
        [-3.25, 18.0, -3.5, 2.0, 2.0, 2.0],
        PartPose::ZERO,
    );
    root.part(
        "left_eye",
        (32, 4),
        [1.25, 18.0, -3.5, 2.0, 2.0, 2.0],
        PartPose::ZERO,
    );
    root.part(
        "mouth",
        (32, 8),
        [0.0, 21.0, -3.5, 1.0, 1.0, 1.0],
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn setup_anim(_model: &BakedModel, _parts: &mut [PartState], _st: &EntityState) {}

const SEGMENT_COUNT: usize = 8;

const CUBE_NAMES: [&str; SEGMENT_COUNT] = [
    "cube0", "cube1", "cube2", "cube3", "cube4", "cube5", "cube6", "cube7",
];

pub fn magma_cube_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    for i in 0..SEGMENT_COUNT {
        let mut u = 0;
        let mut v = 0;
        if i > 0 && i < 4 {
            v += 9 * i as i32;
        } else if i > 3 {
            u = 32;
            v += 9 * i as i32 - 36;
        }
        root.part(
            &format!("cube{i}"),
            (u, v),
            [-4.0, (16 + i) as f32, -4.0, 8.0, 1.0, 8.0],
            PartPose::ZERO,
        );
    }
    root.part(
        "inside_cube",
        (24, 40),
        [-2.0, 18.0, -2.0, 4.0, 4.0, 4.0],
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn magma_cube_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let squish = st.extras.squish.max(0.0);
    for i in 0..SEGMENT_COUNT {
        let id = model.id(CUBE_NAMES[i]);
        parts[id].y = -((4 - i as i32) as f32) * squish * 1.7;
    }
}
