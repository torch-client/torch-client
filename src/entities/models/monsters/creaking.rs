use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe;
use crate::entities::models::monsters::creaking_anim;
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

type Keep = fn(&str) -> bool;

fn base_mesh(keep: Keep) -> MeshDef {
    let cubes = |name: &str, list: CubeList| if keep(name) { list } else { CubeList::new() };

    let mut mesh = MeshDef::new();
    let root = mesh
        .root()
        .child("root", CubeList::new(), PartPose::offset(0.0, 24.0, 0.0));
    let upper_body = root.child(
        "upper_body",
        CubeList::new(),
        PartPose::offset(-1.0, -19.0, 0.0),
    );
    upper_body.child(
        "head",
        cubes(
            "head",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-3.0, -10.0, -3.0, 6.0, 10.0, 6.0)
                .tex_offs(28, 31)
                .add_box(-3.0, -13.0, -3.0, 6.0, 3.0, 6.0)
                .tex_offs(12, 40)
                .add_box(3.0, -13.0, 0.0, 9.0, 14.0, 0.0)
                .tex_offs(34, 12)
                .add_box(-12.0, -14.0, 0.0, 9.0, 14.0, 0.0),
        ),
        PartPose::offset(-3.0, -11.0, 0.0),
    );
    upper_body.child(
        "body",
        cubes(
            "body",
            CubeList::new()
                .tex_offs(0, 16)
                .add_box(0.0, -3.0, -3.0, 6.0, 13.0, 5.0)
                .tex_offs(24, 0)
                .add_box(-6.0, -4.0, -3.0, 6.0, 7.0, 5.0),
        ),
        PartPose::offset(0.0, -7.0, 1.0),
    );
    upper_body.child(
        "right_arm",
        cubes(
            "right_arm",
            CubeList::new()
                .tex_offs(22, 13)
                .add_box(-2.0, -1.5, -1.5, 3.0, 21.0, 3.0)
                .tex_offs(46, 0)
                .add_box(-2.0, 19.5, -1.5, 3.0, 4.0, 3.0),
        ),
        PartPose::offset(-7.0, -9.5, 1.5),
    );
    upper_body.child(
        "left_arm",
        cubes(
            "left_arm",
            CubeList::new()
                .tex_offs(30, 40)
                .add_box(0.0, -1.0, -1.5, 3.0, 16.0, 3.0)
                .tex_offs(52, 12)
                .add_box(0.0, -5.0, -1.5, 3.0, 4.0, 3.0)
                .tex_offs(52, 19)
                .add_box(0.0, 15.0, -1.5, 3.0, 4.0, 3.0),
        ),
        PartPose::offset(6.0, -9.0, 0.5),
    );
    root.child(
        "left_leg",
        cubes(
            "left_leg",
            CubeList::new()
                .tex_offs(42, 40)
                .add_box(-1.5, 0.0, -1.5, 3.0, 16.0, 3.0)
                .tex_offs(45, 55)
                .add_box(-1.5, 15.7, -4.5, 5.0, 0.0, 9.0),
        ),
        PartPose::offset(1.5, -16.0, 0.5),
    );
    root.child(
        "right_leg",
        cubes(
            "right_leg",
            CubeList::new()
                .tex_offs(0, 34)
                .add_box(-3.0, -1.5, -1.5, 3.0, 19.0, 3.0)
                .tex_offs(45, 46)
                .add_box(-5.0, 17.2, -4.5, 5.0, 0.0, 9.0)
                .tex_offs(12, 34)
                .add_box(-3.0, -4.5, -1.5, 3.0, 3.0, 3.0),
        ),
        PartPose::offset(-1.0, -17.5, 0.5),
    );
    mesh
}

pub fn layer() -> LayerDef {
    LayerDef::create(base_mesh(|_| true), 64, 64)
}

pub fn eyes_layer() -> LayerDef {
    LayerDef::create(base_mesh(|name| name == "head"), 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    if st.extras.can_move {
        keyframe::apply_walk(
            &creaking_anim::CREAKING_WALK,
            model,
            parts,
            st.walk_pos,
            st.walk_speed,
            1.0,
            1.0,
        );
    }

    if st.attack_time > 0.0 {
        let animation = &creaking_anim::CREAKING_ATTACK;
        keyframe::apply(
            animation,
            model,
            parts,
            st.attack_time * animation.length_seconds,
            1.0,
        );
    }
    if st.death_time > 0.0 || st.extras.tearing_down {
        keyframe::apply(
            &creaking_anim::CREAKING_DEATH,
            model,
            parts,
            st.death_time / 20.0,
            1.0,
        );
    }
}
