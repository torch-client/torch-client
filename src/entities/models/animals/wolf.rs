use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

fn adult_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child("head", CubeList::new(), PartPose::offset(-1.0, 13.5, -7.0));
    head.child(
        "real_head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-2.0, -3.0, -2.0, 6.0, 6.0, 4.0, g)
            .tex_offs(16, 14)
            .add_box_grow(-2.0, -5.0, 0.0, 2.0, 2.0, 1.0, g)
            .tex_offs(16, 14)
            .add_box_grow(2.0, -5.0, 0.0, 2.0, 2.0, 1.0, g)
            .tex_offs(0, 10)
            .add_box_grow(-0.5, -0.001, -5.0, 3.0, 3.0, 4.0, g),
        PartPose::ZERO,
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(18, 14)
            .add_box_grow(-3.0, -2.0, -3.0, 6.0, 9.0, 6.0, g),
        PartPose::offset_rotation(0.0, 14.0, 2.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    root.child(
        "upper_body",
        CubeList::new()
            .tex_offs(21, 0)
            .add_box_grow(-3.0, -3.0, -3.0, 8.0, 6.0, 7.0, g),
        PartPose::offset_rotation(-1.0, 14.0, -3.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    let left_leg = CubeList::new()
        .tex_offs(0, 18)
        .add_box_grow(0.0, 0.0, -1.0, 2.0, 8.0, 2.0, g);
    let right_leg = CubeList::new()
        .mirror()
        .tex_offs(0, 18)
        .add_box_grow(0.0, 0.0, -1.0, 2.0, 8.0, 2.0, g);
    root.child(
        "right_hind_leg",
        right_leg.clone(),
        PartPose::offset(-2.5, 16.0, 7.0),
    );
    root.child(
        "left_hind_leg",
        left_leg.clone(),
        PartPose::offset(0.5, 16.0, 7.0),
    );
    root.child(
        "right_front_leg",
        right_leg,
        PartPose::offset(-2.5, 16.0, -4.0),
    );
    root.child(
        "left_front_leg",
        left_leg,
        PartPose::offset(0.5, 16.0, -4.0),
    );
    let tail = root.child(
        "tail",
        CubeList::new(),
        PartPose::offset_rotation(-1.0, 12.0, 8.0, 0.62831855, 0.0, 0.0),
    );
    tail.child(
        "real_tail",
        CubeList::new()
            .tex_offs(9, 18)
            .add_box_grow(0.0, 0.0, -1.0, 2.0, 8.0, 2.0, g),
        PartPose::ZERO,
    );
    mesh
}

pub fn layer() -> LayerDef {
    LayerDef::create(adult_mesh(Grow::NONE), 64, 32)
}

pub fn armor_layer() -> LayerDef {
    LayerDef::create(adult_mesh(Grow::all(0.2)), 64, 32)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 12)
            .add_box_grow(-2.99, -3.25, -3.0, 6.0, 5.0, 5.0, Grow::all(0.025))
            .tex_offs(17, 12)
            .add_box(-1.5, -0.24, -5.0, 3.0, 2.0, 2.0),
        PartPose::offset(0.0, 18.25, -4.0),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(0, 5)
            .add_box(-1.0, -1.0, -0.5, 2.0, 2.0, 1.0),
        PartPose::offset(-2.0, -4.25, -0.5),
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(20, 5)
            .add_box(-1.0, -1.0, -0.5, 2.0, 2.0, 1.0),
        PartPose::offset(2.0, -4.25, -0.5),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.0, -2.0, -4.0, 6.0, 4.0, 8.0),
        PartPose::offset(0.0, 19.0, 0.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(0, 22)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(-1.5, 21.0, 3.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(8, 22)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(1.5, 21.0, 3.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(-1.5, 21.0, -3.0),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(20, 0)
            .add_box(-1.0, 0.0, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(1.5, 21.0, -3.0),
    );
    let tail = root.child(
        "tail",
        CubeList::new(),
        PartPose::offset_rotation(0.0, 19.0, 3.0, -0.5236, 0.0, 0.0),
    );
    tail.child(
        "tail_r1",
        CubeList::new()
            .tex_offs(22, 16)
            .add_box(-1.0, -5.7, -1.0, 2.0, 6.0, 2.0),
        PartPose::offset_rotation(0.0, -0.6, 0.2, -3.1, 0.0, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

fn body_roll_angle(st: &EntityState, offset: f32) -> f32 {
    let progress = ((st.extras.shake_anim + offset) / 1.8).clamp(0.0, 1.0);
    (progress * std::f32::consts::PI).sin()
        * (progress * std::f32::consts::PI * 11.0).sin()
        * 0.15
        * std::f32::consts::PI
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let upper_body = model.id("upper_body");
    let real_head = model.id("real_head");
    let real_tail = model.id("real_tail");
    common(model, parts, st, |parts, st| {
        parts[upper_body].y += 2.0;
        parts[upper_body].x_rot = 1.2566371;
        parts[upper_body].y_rot = 0.0;
        let _ = st;
    });
    parts[real_head].z_rot = st.extras.head_roll_angle + body_roll_angle(st, 0.0);
    parts[upper_body].z_rot = body_roll_angle(st, -0.08);
    parts[real_tail].z_rot = body_roll_angle(st, -0.2);
    tail_of_setup(model, parts, st);
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let body = model.id("body");
    let head = model.id("head");
    let tail = model.id("tail");
    common(model, parts, st, |parts, st| {
        parts[body].x_rot -= 1.0;
        let _ = st;
    });
    parts[head].z_rot = st.extras.head_roll_angle + body_roll_angle(st, 0.0);
    parts[tail].z_rot = body_roll_angle(st, -0.2);
    tail_of_setup(model, parts, st);
}

fn common(
    model: &BakedModel,
    parts: &mut [PartState],
    st: &EntityState,
    extra_sitting: impl FnOnce(&mut [PartState], &EntityState),
) {
    let pos = st.walk_pos;
    let speed = st.walk_speed;
    let tail = model.id("tail");
    let body = model.id("body");
    let right_hind = model.id("right_hind_leg");
    let left_hind = model.id("left_hind_leg");
    let right_front = model.id("right_front_leg");
    let left_front = model.id("left_front_leg");

    if st.extras.anger_ticks > 0 {
        parts[tail].y_rot = 0.0;
    } else {
        parts[tail].y_rot = (pos * 0.6662).cos() * 1.4 * speed;
    }

    if st.extras.sitting {
        let age_scale = st.age_scale;
        parts[body].y += 4.0 * age_scale;
        parts[body].z -= 2.0 * age_scale;
        parts[body].x_rot = 0.7853982;
        parts[tail].y += 9.0 * age_scale;
        parts[tail].z -= 2.0 * age_scale;
        parts[right_hind].y += 6.7 * age_scale;
        parts[right_hind].z -= 5.0 * age_scale;
        parts[right_hind].x_rot = 4.712389;
        parts[left_hind].y += 6.7 * age_scale;
        parts[left_hind].z -= 5.0 * age_scale;
        parts[left_hind].x_rot = 4.712389;
        parts[right_front].x_rot = 5.811947;
        parts[right_front].x += 0.01 * age_scale;
        parts[right_front].y += 1.0 * age_scale;
        parts[left_front].x_rot = 5.811947;
        parts[left_front].x -= 0.01 * age_scale;
        parts[left_front].y += 1.0 * age_scale;
        extra_sitting(parts, st);
    } else {
        parts[right_hind].x_rot = (pos * 0.6662).cos() * 1.4 * speed;
        parts[left_hind].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
        parts[right_front].x_rot = (pos * 0.6662 + std::f32::consts::PI).cos() * 1.4 * speed;
        parts[left_front].x_rot = (pos * 0.6662).cos() * 1.4 * speed;
    }

    parts[body].z_rot = body_roll_angle(st, -0.16);
}

fn tail_of_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    let tail = model.id("tail");
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[tail].x_rot = st.extras.tail_angle;
}
