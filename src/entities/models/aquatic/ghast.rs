#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::models::aquatic::scaling;
use crate::entities::state::EntityState;

fn tentacle_name(i: usize) -> String {
    format!("tentacle{i}")
}

const GHAST_TENTACLE_LENGTHS: [f32; 9] = [8.0, 13.0, 9.0, 11.0, 11.0, 10.0, 12.0, 9.0, 12.0];

pub fn ghast_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -8.0, -8.0, 16.0, 16.0, 16.0),
        PartPose::offset(0.0, 17.6, 0.0),
    );
    for (i, len) in GHAST_TENTACLE_LENGTHS.iter().enumerate() {
        let xo = (((i % 3) as f32 - (i / 3 % 2) as f32 * 0.5 + 0.25) / 2.0 * 2.0 - 1.0) * 5.0;
        let yo = ((i / 3) as f32 / 2.0 * 2.0 - 1.0) * 5.0;
        root.child(
            &tentacle_name(i),
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-1.0, 0.0, -1.0, 2.0, *len, 2.0),
            PartPose::offset(xo, 24.6, yo),
        );
    }
    LayerDef::create(scaling(mesh, 4.5), 64, 32)
}

pub fn animate_tentacles(
    model: &BakedModel,
    parts: &mut [PartState],
    st: &EntityState,
    count: usize,
) {
    for i in 0..count {
        parts[model.id(&tentacle_name(i))].x_rot =
            0.2 * (st.age_ticks * 0.3 + i as f32).sin() + 0.4;
    }
}

pub fn ghast_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    animate_tentacles(model, parts, st, 9);
}

const BABY_SCALE: f32 = 0.2375;

fn happy_ghast_mesh(is_baby: bool, deformation: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let body = mesh.root().child(
        "body",
        CubeList::new().tex_offs(0, 0).add_box_grow(
            -8.0,
            -8.0,
            -8.0,
            16.0,
            16.0,
            16.0,
            deformation,
        ),
        PartPose::offset(0.0, 16.0, 0.0),
    );
    if is_baby {
        body.child(
            "inner_body",
            CubeList::new().tex_offs(0, 32).add_box_grow(
                -8.0,
                -16.0,
                -8.0,
                16.0,
                16.0,
                16.0,
                deformation.extend(-0.5),
            ),
            PartPose::offset(0.0, 8.0, 0.0),
        );
    }
    const TENTACLES: [(f32, f32, f32, f32); 9] = [
        (2.0, 5.0, -3.75, -5.0),
        (2.0, 7.0, 1.25, -5.0),
        (2.0, 4.0, 6.25, -5.0),
        (2.0, 5.0, -6.25, 0.0),
        (2.0, 5.0, -1.25, 0.0),
        (2.0, 7.0, 3.75, 0.0),
        (2.0, 8.0, -3.75, 5.0),
        (2.0, 8.0, 1.25, 5.0),
        (2.0, 5.0, 6.25, 5.0),
    ];
    for (i, (w, h, x, z)) in TENTACLES.iter().enumerate() {
        body.child(
            &tentacle_name(i),
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_grow(-1.0, 0.0, -1.0, *w, *h, 2.0, deformation),
            PartPose::offset(*x, 7.0, *z),
        );
    }
    scaling(mesh, 4.0)
}

pub fn happy_ghast_layer() -> LayerDef {
    LayerDef::create(happy_ghast_mesh(false, Grow::NONE), 64, 64)
}

pub fn happy_ghast_baby_layer() -> LayerDef {
    LayerDef::create(
        scaling(happy_ghast_mesh(true, Grow::NONE), BABY_SCALE),
        64,
        64,
    )
}

pub fn happy_ghast_ropes_layer() -> LayerDef {
    LayerDef::create(happy_ghast_mesh(false, Grow::all(0.2)), 64, 64)
}

pub fn happy_ghast_baby_ropes_layer() -> LayerDef {
    LayerDef::create(
        scaling(happy_ghast_mesh(true, Grow::all(0.2)), BABY_SCALE),
        64,
        64,
    )
}

const BODY_SQUEEZE: f32 = 0.9375;

pub fn happy_ghast_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    if st.extras.body_armor.is_some() {
        let body = model.id("body");
        parts[body].x_scale = BODY_SQUEEZE;
        parts[body].y_scale = BODY_SQUEEZE;
        parts[body].z_scale = BODY_SQUEEZE;
    }
    animate_tentacles(model, parts, st, 9);
}

fn harness_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "harness",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -16.0, -8.0, 16.0, 16.0, 16.0),
        PartPose::offset(0.0, 24.0, 0.0),
    );
    root.child(
        "goggles",
        CubeList::new().tex_offs(0, 32).add_box_grow(
            -8.0,
            -2.5,
            -2.5,
            16.0,
            5.0,
            5.0,
            Grow::all(0.15),
        ),
        PartPose::offset(0.0, 14.0, -5.5),
    );
    scaling(mesh, 4.0)
}

pub fn harness_layer() -> LayerDef {
    LayerDef::create(harness_mesh(), 64, 64)
}

pub fn baby_harness_layer() -> LayerDef {
    LayerDef::create(
        scaling(scaling(harness_mesh(), BABY_SCALE), BABY_SCALE),
        64,
        64,
    )
}

const GOGGLES_Y_OFFSET: f32 = 14.0;

pub fn harness_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let goggles = model.id("goggles");
    if st.extras.is_ridden {
        parts[goggles].x_rot = 0.0;
        parts[goggles].y = GOGGLES_Y_OFFSET;
    } else {
        parts[goggles].x_rot = -0.7854;
        parts[goggles].y = 9.0;
    }
}
