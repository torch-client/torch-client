use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

pub fn outer_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().child(
        "cube",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, 16.0, -4.0, 8.0, 8.0, 8.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn inner_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "cube",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box(-3.0, 17.0, -3.0, 6.0, 6.0, 6.0),
        PartPose::ZERO,
    );
    root.child(
        "right_eye",
        CubeList::new()
            .tex_offs(32, 0)
            .add_box(-3.25, 18.0, -3.5, 2.0, 2.0, 2.0),
        PartPose::ZERO,
    );
    root.child(
        "left_eye",
        CubeList::new()
            .tex_offs(32, 4)
            .add_box(1.25, 18.0, -3.5, 2.0, 2.0, 2.0),
        PartPose::ZERO,
    );
    root.child(
        "mouth",
        CubeList::new()
            .tex_offs(32, 8)
            .add_box(0.0, 21.0, -3.5, 1.0, 1.0, 1.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn setup_anim(_model: &BakedModel, _parts: &mut [PartState], _st: &EntityState) {}

const SEGMENT_COUNT: usize = 8;

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
        root.child(
            &format!("cube{i}"),
            CubeList::new()
                .tex_offs(u, v)
                .add_box(-4.0, (16 + i) as f32, -4.0, 8.0, 1.0, 8.0),
            PartPose::ZERO,
        );
    }
    root.child(
        "inside_cube",
        CubeList::new()
            .tex_offs(24, 40)
            .add_box(-2.0, 18.0, -2.0, 4.0, 4.0, 4.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn magma_cube_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let squish = st.extras.squish.max(0.0);
    for i in 0..SEGMENT_COUNT {
        let id = model.id(&format!("cube{i}"));
        parts[id].y = -((4 - i as i32) as f32) * squish * 1.7;
    }
}
