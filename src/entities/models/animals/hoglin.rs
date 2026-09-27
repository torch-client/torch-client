use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

const DEFAULT_HEAD_X_ROT: f32 = 0.87266463;

const ATTACK_HEAD_X_ROT_END: f32 = -0.34906584;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(1, 1)
            .add_box(-8.0, -7.0, -13.0, 16.0, 14.0, 26.0),
        PartPose::offset(0.0, 7.0, 0.0),
    );
    body.child(
        "mane",
        CubeList::new().tex_offs(90, 33).add_box_grow(
            0.0,
            0.0,
            -9.0,
            0.0,
            10.0,
            19.0,
            Grow::all(0.001),
        ),
        PartPose::offset(0.0, -14.0, -7.0),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(61, 1)
            .add_box(-7.0, -3.0, -19.0, 14.0, 6.0, 19.0),
        PartPose::offset_rotation(0.0, 2.0, -12.0, DEFAULT_HEAD_X_ROT, 0.0, 0.0),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(1, 1)
            .add_box(-6.0, -1.0, -2.0, 6.0, 1.0, 4.0),
        PartPose::offset_rotation(-6.0, -2.0, -3.0, 0.0, 0.0, -0.6981317),
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(1, 6)
            .add_box(0.0, -1.0, -2.0, 6.0, 1.0, 4.0),
        PartPose::offset_rotation(6.0, -2.0, -3.0, 0.0, 0.0, 0.6981317),
    );
    head.child(
        "right_horn",
        CubeList::new()
            .tex_offs(10, 13)
            .add_box(-1.0, -11.0, -1.0, 2.0, 11.0, 2.0),
        PartPose::offset(-7.0, 2.0, -12.0),
    );
    head.child(
        "left_horn",
        CubeList::new()
            .tex_offs(1, 13)
            .add_box(-1.0, -11.0, -1.0, 2.0, 11.0, 2.0),
        PartPose::offset(7.0, 2.0, -12.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(66, 42)
            .add_box(-3.0, 0.0, -3.0, 6.0, 14.0, 6.0),
        PartPose::offset(-4.0, 10.0, -8.5),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(41, 42)
            .add_box(-3.0, 0.0, -3.0, 6.0, 14.0, 6.0),
        PartPose::offset(4.0, 10.0, -8.5),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(21, 45)
            .add_box(-2.5, 0.0, -2.5, 5.0, 11.0, 5.0),
        PartPose::offset(-5.0, 13.0, 10.0),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(0, 45)
            .add_box(-2.5, 0.0, -2.5, 5.0, 11.0, 5.0),
        PartPose::offset(5.0, 13.0, 10.0),
    );
    LayerDef::create(mesh, 128, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-5.0, -2.2605, -10.547, 10.0, 4.0, 12.0)
            .tex_offs(44, 29)
            .add_box(-7.0, -4.0981, -8.4879, 2.0, 5.0, 2.0)
            .tex_offs(52, 29)
            .add_box(5.0, -4.0981, -8.4879, 2.0, 5.0, 2.0),
        PartPose::offset_rotation(0.0, 13.0, -7.0, 0.8727, 0.0, 0.0),
    );
    head.child(
        "right_ear",
        CubeList::new()
            .tex_offs(32, 5)
            .add_box(-5.1, -0.5, -2.0, 6.0, 1.0, 4.0),
        PartPose::offset_rotation(-5.0, -1.0, -1.5, 0.0, 0.0, -0.8727),
    );
    head.child(
        "left_ear",
        CubeList::new()
            .tex_offs(32, 0)
            .mirror()
            .add_box(-0.9, -0.5, -2.0, 6.0, 1.0, 4.0)
            .mirror_if(false),
        PartPose::offset_rotation(5.0, -1.0, -1.5, 0.0, 0.0, 0.8727),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box_grow(-4.0, -14.0, -7.0, 8.0, 8.0, 14.0, Grow::all(0.02))
            .tex_offs(24, 39)
            .add_box_grow(0.0, -18.0, -8.0, 0.0, 6.0, 11.0, Grow::all(0.02)),
        PartPose::offset(0.0, 24.0, 0.0),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(0, 47)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset(-2.5, 18.0, 4.5),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(12, 47)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset(2.5, 18.0, 4.5),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(0, 38)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset(-2.5, 18.0, -4.5),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(12, 38)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset(2.5, 18.0, -4.5),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, false);
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    setup(model, parts, st, true);
}

fn setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState, baby: bool) {
    let speed = st.walk_speed;
    let pos = st.walk_pos;
    let head = model.id("head");
    let right_ear = model.id("right_ear");
    let left_ear = model.id("left_ear");
    let right_front = model.id("right_front_leg");
    let left_front = model.id("left_front_leg");
    let right_hind = model.id("right_hind_leg");
    let left_hind = model.id("left_hind_leg");

    parts[right_ear].z_rot = -0.6981317 - speed * pos.sin();
    parts[left_ear].z_rot = 0.6981317 + speed * pos.sin();
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;

    let remaining = st.extras.attack_animation_remaining_ticks;
    let headbutt = 1.0 - (10 - 2 * remaining).abs() as f32 / 10.0;
    parts[head].x_rot =
        DEFAULT_HEAD_X_ROT + headbutt * (ATTACK_HEAD_X_ROT_END - DEFAULT_HEAD_X_ROT);
    if baby {
        parts[head].y += headbutt * 2.5;
    }

    parts[right_front].x_rot = pos.cos() * 1.2 * speed;
    parts[left_front].x_rot = (pos + std::f32::consts::PI).cos() * 1.2 * speed;
    parts[right_hind].x_rot = parts[left_front].x_rot;
    parts[left_hind].x_rot = parts[right_front].x_rot;
}
