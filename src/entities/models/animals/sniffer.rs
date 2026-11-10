use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::keyframe;
use crate::entities::state::EntityState;

use super::animations::sniffer as anim;
use crate::util::mth::DEG_TO_RAD;

const WALK_ANIMATION_SPEED_MAX: f32 = 9.0;
const WALK_ANIMATION_SCALE_FACTOR: f32 = 100.0;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let bone = root.child("bone", CubeList::new(), PartPose::offset(0.0, 5.0, 0.0));
    let body = bone.child(
        "body",
        CubeList::new()
            .tex_offs(62, 68)
            .add_box(-12.5, -14.0, -20.0, 25.0, 29.0, 40.0)
            .tex_offs(62, 0)
            .add_box_grow(-12.5, -14.0, -20.0, 25.0, 24.0, 40.0, Grow::all(0.5))
            .tex_offs(87, 68)
            .add_box(-12.5, 12.0, -20.0, 25.0, 0.0, 40.0),
        PartPose::offset(0.0, 0.0, 0.0),
    );
    let head = body.child(
        "head",
        CubeList::new()
            .tex_offs(8, 15)
            .add_box(-6.5, -7.5, -11.5, 13.0, 18.0, 11.0)
            .tex_offs(8, 4)
            .add_box(-6.5, 7.5, -11.5, 13.0, 0.0, 11.0),
        PartPose::offset(0.0, 6.5, -19.48),
    );
    head.part(
        "left_ear",
        (2, 0),
        [0.0, 0.0, -3.0, 1.0, 19.0, 7.0],
        PartPose::offset(6.51, -7.5, -4.51),
    );
    head.part(
        "right_ear",
        (48, 0),
        [-1.0, 0.0, -3.0, 1.0, 19.0, 7.0],
        PartPose::offset(-6.51, -7.5, -4.51),
    );
    head.part(
        "nose",
        (10, 45),
        [-6.5, -2.0, -9.0, 13.0, 2.0, 9.0],
        PartPose::offset(0.0, -4.5, -11.5),
    );
    head.part(
        "lower_beak",
        (10, 57),
        [-6.5, -7.0, -8.0, 13.0, 12.0, 9.0],
        PartPose::offset(0.0, 2.5, -12.5),
    );
    let bone = mesh.root().get("bone");
    for (name, u, v, z) in [
        ("right_front_leg", 32, 87, -15.0),
        ("right_mid_leg", 32, 105, 0.0),
        ("right_hind_leg", 32, 123, 15.0),
    ] {
        bone.part(
            name,
            (u, v),
            [-3.5, -1.0, -4.0, 7.0, 10.0, 8.0],
            PartPose::offset(-7.5, 10.0, z),
        );
    }
    for (name, u, v, z) in [
        ("left_front_leg", 0, 87, -15.0),
        ("left_mid_leg", 0, 105, 0.0),
        ("left_hind_leg", 0, 123, 15.0),
    ] {
        bone.part(
            name,
            (u, v),
            [-3.5, -1.0, -4.0, 7.0, 10.0, 8.0],
            PartPose::offset(7.5, 10.0, z),
        );
    }
    LayerDef::create(mesh, 192, 192)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let bone = root.child("bone", CubeList::new(), PartPose::offset(0.0, 24.0, 0.0));
    let body = bone.child(
        "body",
        CubeList::new()
            .tex_offs(0, 35)
            .add_box_grow(-13.0, -14.0, -0.5, 14.0, 14.0, 20.0, Grow::all(0.25))
            .tex_offs(0, 0)
            .add_box(-13.0, -14.0, -0.5, 14.0, 15.0, 20.0)
            .tex_offs(68, 0)
            .add_box(-13.0, 0.0, -0.5, 14.0, 0.0, 20.0),
        PartPose::offset(6.0, -3.0, -9.5),
    );
    let head = body.child(
        "head",
        CubeList::new()
            .tex_offs(68, 20)
            .add_box(-5.0, -4.25, -7.5, 10.0, 9.0, 9.0)
            .tex_offs(88, 20)
            .add_box(-5.0, 3.75, -7.5, 10.0, 0.0, 9.0),
        PartPose::offset(-6.0, -4.75, 0.0),
    );
    head.part(
        "left_ear",
        (104, 38),
        [0.0, 0.0, -2.0, 1.0, 11.0, 3.0],
        PartPose::offset(5.0, -4.25, -1.5),
    );
    head.part(
        "right_ear",
        (96, 38),
        [-1.0, 0.0, -2.0, 1.0, 11.0, 3.0],
        PartPose::offset(-5.0, -4.25, -1.5),
    );
    head.part(
        "nose",
        (68, 47),
        [-5.0, -3.0, -2.0, 10.0, 3.0, 4.0],
        PartPose::offset(0.0, -1.25, -9.5),
    );
    head.part(
        "lower_beak",
        (68, 38),
        [-5.0, -2.5, -2.0, 10.0, 5.0, 4.0],
        PartPose::offset(0.0, 1.25, -9.5),
    );
    let bone = mesh.root().get("bone");
    for (name, u, v, x, z) in [
        ("right_front_leg", 0, 69, -4.0, -7.0),
        ("right_mid_leg", 0, 78, -4.0, 0.0),
        ("right_hind_leg", 0, 87, -4.0, 7.0),
        ("left_front_leg", 16, 69, 4.0, -7.0),
        ("left_mid_leg", 16, 78, 4.0, 0.0),
        ("left_hind_leg", 16, 87, 4.0, 7.0),
    ] {
        bone.part(
            name,
            (u, v),
            [-2.0, -1.0, -2.0, 4.0, 5.0, 4.0],
            PartPose::offset(x, -4.0, z),
        );
    }
    LayerDef::create(mesh, 128, 128)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st);
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st);
    keyframe::apply_static(&anim::BABY_TRANSFORM, model, parts);
}

const STATE_SEARCHING: u8 = 4;

fn setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    let searching = st.extras.sniffer_state == STATE_SEARCHING;
    let walk = if searching {
        &anim::SNIFFER_SNIFF_SEARCH
    } else {
        &anim::SNIFFER_WALK
    };
    keyframe::apply_walk(
        walk,
        model,
        parts,
        st.walk_pos,
        st.walk_speed,
        WALK_ANIMATION_SPEED_MAX,
        WALK_ANIMATION_SCALE_FACTOR,
    );

    use crate::entities::state::anim_slot::sniffer as slot;
    let states = &st.extras.anim;
    let age = st.age_ticks;
    keyframe::apply_state(&anim::SNIFFER_DIG, model, parts, states[slot::DIGGING], age);
    keyframe::apply_state(
        &anim::SNIFFER_LONGSNIFF,
        model,
        parts,
        states[slot::SNIFFING],
        age,
    );
    keyframe::apply_state(
        &anim::SNIFFER_STAND_UP,
        model,
        parts,
        states[slot::RISING],
        age,
    );
    keyframe::apply_state(
        &anim::SNIFFER_HAPPY,
        model,
        parts,
        states[slot::FEELING_HAPPY],
        age,
    );
    keyframe::apply_state(
        &anim::SNIFFER_SNIFFSNIFF,
        model,
        parts,
        states[slot::SCENTING],
        age,
    );
}
