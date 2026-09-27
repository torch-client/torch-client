#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe;
use crate::entities::models::monsters::breeze_anim;
use crate::entities::state::{EntityState, Pose};

type Keep = fn(&str) -> bool;

fn base_mesh(keep: Keep) -> MeshDef {
    let cubes = |name: &str, list: CubeList| if keep(name) { list } else { CubeList::new() };

    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child("body", CubeList::new(), PartPose::offset(0.0, 0.0, 0.0));
    let rods = body.child("rods", CubeList::new(), PartPose::offset(0.0, 8.0, 0.0));
    let rod = || {
        CubeList::new()
            .tex_offs(0, 17)
            .add_box(-1.0, 0.0, -3.0, 2.0, 8.0, 2.0)
    };
    rods.child(
        "rod_1",
        cubes("rod_1", rod()),
        PartPose::offset_rotation(2.5981, -3.0, 1.5, -2.7489, -1.0472, 3.1416),
    );
    rods.child(
        "rod_2",
        cubes("rod_2", rod()),
        PartPose::offset_rotation(-2.5981, -3.0, 1.5, -2.7489, 1.0472, 3.1416),
    );
    rods.child(
        "rod_3",
        cubes("rod_3", rod()),
        PartPose::offset_rotation(0.0, -3.0, -3.0, 0.3927, 0.0, 0.0),
    );
    let head_cubes = || {
        CubeList::new()
            .tex_offs(4, 24)
            .add_box(-5.0, -5.0, -4.2, 10.0, 3.0, 4.0)
            .tex_offs(0, 0)
            .add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0)
    };
    let head = body.child(
        "head",
        cubes("head", head_cubes()),
        PartPose::offset(0.0, 4.0, 0.0),
    );
    head.child(
        "eyes",
        cubes("eyes", head_cubes()),
        PartPose::offset(0.0, 0.0, 0.0),
    );

    let wind_body = root.child(
        "wind_body",
        CubeList::new(),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    let wind_bottom = wind_body.child(
        "wind_bottom",
        cubes(
            "wind_bottom",
            CubeList::new()
                .tex_offs(1, 83)
                .add_box(-2.5, -7.0, -2.5, 5.0, 7.0, 5.0),
        ),
        PartPose::offset(0.0, 24.0, 0.0),
    );
    let wind_mid = wind_bottom.child(
        "wind_mid",
        cubes(
            "wind_mid",
            CubeList::new()
                .tex_offs(74, 28)
                .add_box(-6.0, -6.0, -6.0, 12.0, 6.0, 12.0)
                .tex_offs(78, 32)
                .add_box(-4.0, -6.0, -4.0, 8.0, 6.0, 8.0)
                .tex_offs(49, 71)
                .add_box(-2.5, -6.0, -2.5, 5.0, 6.0, 5.0),
        ),
        PartPose::offset(0.0, -7.0, 0.0),
    );
    wind_mid.child(
        "wind_top",
        cubes(
            "wind_top",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-9.0, -8.0, -9.0, 18.0, 8.0, 18.0)
                .tex_offs(6, 6)
                .add_box(-6.0, -8.0, -6.0, 12.0, 8.0, 12.0)
                .tex_offs(105, 57)
                .add_box(-2.5, -8.0, -2.5, 5.0, 8.0, 5.0),
        ),
        PartPose::offset(0.0, -6.0, 0.0),
    );
    mesh
}

pub fn layer() -> LayerDef {
    LayerDef::create(
        base_mesh(|name| matches!(name, "head" | "eyes" | "rod_1" | "rod_2" | "rod_3")),
        32,
        32,
    )
}

pub fn wind_layer() -> LayerDef {
    LayerDef::create(
        base_mesh(|name| matches!(name, "wind_bottom" | "wind_mid" | "wind_top")),
        128,
        128,
    )
}

pub fn eyes_layer() -> LayerDef {
    LayerDef::create(base_mesh(|name| name == "eyes"), 32, 32)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let seconds = st.age_ticks / 20.0;
    keyframe::apply(&breeze_anim::IDLE, model, parts, seconds, 1.0);

    let one_shot = match st.pose {
        Pose::Shooting => Some(&breeze_anim::SHOOT),
        Pose::Inhaling => Some(&breeze_anim::INHALE),
        Pose::Sliding => Some(&breeze_anim::SLIDE),
        Pose::LongJumping => Some(&breeze_anim::JUMP),
        _ => None,
    };
    if let Some(animation) = one_shot {
        keyframe::apply(
            animation,
            model,
            parts,
            seconds % animation.length_seconds,
            1.0,
        );
    }
}
