#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

fn shell_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "lid",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -16.0, -8.0, 16.0, 12.0, 16.0),
        PartPose::offset(0.0, 24.0, 0.0),
    );
    root.child(
        "base",
        CubeList::new()
            .tex_offs(0, 28)
            .add_box(-8.0, -8.0, -8.0, 16.0, 8.0, 16.0),
        PartPose::offset(0.0, 24.0, 0.0),
    );
    mesh
}

pub fn box_layer() -> LayerDef {
    LayerDef::create(shell_mesh(), 64, 64)
}

pub fn layer() -> LayerDef {
    let mut mesh = shell_mesh();
    mesh.root().child(
        "head",
        CubeList::new()
            .tex_offs(0, 52)
            .add_box(-3.0, 0.0, -3.0, 6.0, 6.0, 6.0),
        PartPose::offset(0.0, 12.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let peek = st.extras.peek;
    let bs = (0.5 + peek) * 3.1415927;
    let q = -1.0 + bs.sin();
    let mut extra = 0.0;
    if bs > 3.1415927 {
        extra = (st.age_ticks * 0.1).sin() * 0.7;
    }

    let lid = model.id("lid");
    parts[lid].set_pos(0.0, 16.0 + bs.sin() * 8.0 + extra, 0.0);
    parts[lid].y_rot = if peek > 0.3 {
        q * q * q * q * 3.1415927 * 0.125
    } else {
        0.0
    };

    let head = model.id("head");
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = (st.y_rot - 180.0) * DEG_TO_RAD;
}
