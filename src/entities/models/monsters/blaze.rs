#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -4.0, -4.0, 8.0, 8.0, 8.0),
        PartPose::ZERO,
    );
    let rod = CubeList::new()
        .tex_offs(0, 16)
        .add_box(0.0, 0.0, 0.0, 2.0, 8.0, 2.0);

    let mut angle = 0.0f32;
    for i in 0..4 {
        let x = angle.cos() * 9.0;
        let y = -2.0 + ((i * 2) as f32 * 0.25).cos();
        let z = angle.sin() * 9.0;
        root.child(&format!("part{i}"), rod.clone(), PartPose::offset(x, y, z));
        angle += 1.0;
    }
    angle = 0.7853982;
    for i in 4..8 {
        let x = angle.cos() * 7.0;
        let y = 2.0 + ((i * 2) as f32 * 0.25).cos();
        let z = angle.sin() * 7.0;
        root.child(&format!("part{i}"), rod.clone(), PartPose::offset(x, y, z));
        angle += 1.0;
    }
    angle = 0.47123894;
    for i in 8..12 {
        let x = angle.cos() * 5.0;
        let y = 11.0 + (i as f32 * 1.5 * 0.5).cos();
        let z = angle.sin() * 5.0;
        root.child(&format!("part{i}"), rod.clone(), PartPose::offset(x, y, z));
        angle += 1.0;
    }
    LayerDef::create(mesh, 64, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let age = st.age_ticks;

    let mut angle = age * 3.1415927 * -0.1;
    for i in 0..4 {
        let id = model.id(&format!("part{i}"));
        parts[id].y = -2.0 + (((i * 2) as f32 + age) * 0.25).cos();
        parts[id].x = angle.cos() * 9.0;
        parts[id].z = angle.sin() * 9.0;
        angle += 1.0;
    }
    angle = 0.7853982 + age * 3.1415927 * 0.03;
    for i in 4..8 {
        let id = model.id(&format!("part{i}"));
        parts[id].y = 2.0 + (((i * 2) as f32 + age) * 0.25).cos();
        parts[id].x = angle.cos() * 7.0;
        parts[id].z = angle.sin() * 7.0;
        angle += 1.0;
    }
    angle = 0.47123894 + age * 3.1415927 * -0.05;
    for i in 8..12 {
        let id = model.id(&format!("part{i}"));
        parts[id].y = 11.0 + ((i as f32 * 1.5 + age) * 0.5).cos();
        parts[id].x = angle.cos() * 5.0;
        parts[id].z = angle.sin() * 5.0;
        angle += 1.0;
    }

    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
}
