#![allow(clippy::approx_constant)]

use crate::entities::geom::{
    BakedModel, CubeList, Grow, LayerDef, MeshDef, PartDef, PartPose, PartState,
};
use crate::entities::state::EntityState;

fn tentacle_name(i: usize) -> &'static str {
    const NAMES: [&str; 8] = [
        "tentacle0",
        "tentacle1",
        "tentacle2",
        "tentacle3",
        "tentacle4",
        "tentacle5",
        "tentacle6",
        "tentacle7",
    ];
    NAMES[i]
}

fn tentacle_ring(root: &mut PartDef, cubes: CubeList, radius: f32, y: f32) {
    for i in 0..8 {
        let angle = i as f64 * std::f64::consts::PI * 2.0 / 8.0;
        let x = angle.cos() as f32 * radius;
        let z = angle.sin() as f32 * radius;
        let y_rot =
            (i as f64 * std::f64::consts::PI * -2.0 / 8.0 + std::f64::consts::FRAC_PI_2) as f32;
        root.child(
            tentacle_name(i),
            cubes.clone(),
            PartPose::offset_rotation(x, y, z, 0.0, y_rot, 0.0),
        );
    }
}

pub fn squid_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let g = Grow::all(0.02);
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-6.0, -8.0, -6.0, 12.0, 16.0, 12.0, g),
        PartPose::offset(0.0, 8.0, 0.0),
    );
    let tentacle = CubeList::new()
        .tex_offs(48, 0)
        .add_box(-1.0, 0.0, -1.0, 2.0, 18.0, 2.0);
    tentacle_ring(root, tentacle, 5.0, 15.0);
    LayerDef::create(mesh, 64, 32)
}

pub fn baby_squid_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -5.0, -4.0, 8.0, 10.0, 8.0),
        PartPose::offset(0.0, 13.0, 0.0),
    );
    let tentacle = CubeList::new()
        .tex_offs(0, 18)
        .add_box(-1.0, -0.5, -1.0, 2.0, 6.0, 2.0);
    tentacle_ring(root, tentacle, 3.0, 18.5);
    LayerDef::create(mesh, 32, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    for i in 0..8 {
        parts[model.id(tentacle_name(i))].x_rot = st.extras.tentacle_angle;
    }
}
