use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

const SPEED: f32 = 1.5;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 32)
            .add_box(-2.0, 0.0, -2.0, 4.0, 16.0, 4.0),
        PartPose::offset(-4.0, 8.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 55)
            .add_box(-2.0, 0.0, -2.0, 4.0, 16.0, 4.0),
        PartPose::offset(4.0, 8.0, 0.0),
    );
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -6.0, -8.0, 16.0, 14.0, 16.0),
        PartPose::offset(0.0, 1.0, 0.0),
    );
    body.child(
        "right_bottom_bristle",
        CubeList::new()
            .tex_offs(16, 65)
            .add_box_mirror(-12.0, 0.0, 0.0, 12.0, 0.0, 16.0, true),
        PartPose::offset_rotation(-8.0, 4.0, -8.0, 0.0, 0.0, -1.2217305),
    );
    body.child(
        "right_middle_bristle",
        CubeList::new()
            .tex_offs(16, 49)
            .add_box_mirror(-12.0, 0.0, 0.0, 12.0, 0.0, 16.0, true),
        PartPose::offset_rotation(-8.0, -1.0, -8.0, 0.0, 0.0, -1.134464),
    );
    body.child(
        "right_top_bristle",
        CubeList::new()
            .tex_offs(16, 33)
            .add_box_mirror(-12.0, 0.0, 0.0, 12.0, 0.0, 16.0, true),
        PartPose::offset_rotation(-8.0, -5.0, -8.0, 0.0, 0.0, -0.87266463),
    );
    body.child(
        "left_top_bristle",
        CubeList::new()
            .tex_offs(16, 33)
            .add_box(0.0, 0.0, 0.0, 12.0, 0.0, 16.0),
        PartPose::offset_rotation(8.0, -6.0, -8.0, 0.0, 0.0, 0.87266463),
    );
    body.child(
        "left_middle_bristle",
        CubeList::new()
            .tex_offs(16, 49)
            .add_box(0.0, 0.0, 0.0, 12.0, 0.0, 16.0),
        PartPose::offset_rotation(8.0, -2.0, -8.0, 0.0, 0.0, 1.134464),
    );
    body.child(
        "left_bottom_bristle",
        CubeList::new()
            .tex_offs(16, 65)
            .add_box(0.0, 0.0, 0.0, 12.0, 0.0, 16.0),
        PartPose::offset_rotation(8.0, 3.0, -8.0, 0.0, 0.0, 1.2217305),
    );
    LayerDef::create(mesh, 64, 128)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.5, -3.75, -4.0, 7.0, 7.0, 8.0),
        PartPose::offset(0.0, 16.75, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 24)
            .add_box(-1.0, 0.0, -1.0, 2.0, 4.0, 2.0),
        PartPose::offset(-1.5, 20.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(8, 24)
            .add_box(-1.0, 0.0, -1.0, 2.0, 4.0, 2.0),
        PartPose::offset(1.5, 20.0, 0.0),
    );
    let body = root.get("body");
    body.child(
        "bristle0",
        CubeList::new()
            .tex_offs(0, 21)
            .add_box(-3.5, -2.5, 0.0, 7.0, 3.0, 0.0),
        PartPose::offset(0.0, -4.25, 2.0),
    );
    body.child(
        "bristle1",
        CubeList::new()
            .tex_offs(0, 18)
            .add_box(-3.5, -2.5, 0.0, 7.0, 3.0, 0.0),
        PartPose::offset(0.0, -4.25, 0.0),
    );
    body.child(
        "bristle2",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box(-3.5, -2.5, 0.0, 7.0, 3.0, 0.0),
        PartPose::offset(0.0, -4.25, -2.0),
    );
    LayerDef::create(mesh, 32, 32)
}

fn animate_bristle(
    parts: &mut [PartState],
    age_ticks: f32,
    bristle_flow: f32,
    first: usize,
    second: usize,
    third: usize,
    axis: fn(&mut PartState, f32),
) {
    axis(&mut parts[first], bristle_flow * 0.6);
    axis(&mut parts[second], bristle_flow * 1.2);
    axis(&mut parts[third], bristle_flow * 1.3);
    axis(&mut parts[first], 0.1 * (age_ticks * 0.4).sin());
    axis(&mut parts[second], 0.1 * (age_ticks * 0.2).sin());
    axis(&mut parts[third], 0.05 * (age_ticks * -0.4).sin());
}

fn add_z_rot(part: &mut PartState, rotation: f32) {
    part.z_rot += rotation;
}

fn add_x_rot(part: &mut PartState, rotation: f32) {
    part.x_rot += rotation;
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let (pos, speed) = common(model, parts, st);
    let body = model.id("body");
    let left_leg = model.id("left_leg");
    let right_leg = model.id("right_leg");
    let rbb = model.id("right_bottom_bristle");
    let rmb = model.id("right_middle_bristle");
    let rtb = model.id("right_top_bristle");
    let ltb = model.id("left_top_bristle");
    let lmb = model.id("left_middle_bristle");
    let lbb = model.id("left_bottom_bristle");

    parts[rbb].z_rot = -1.2217305;
    parts[rmb].z_rot = -1.134464;
    parts[rtb].z_rot = -0.87266463;
    parts[ltb].z_rot = 0.87266463;
    parts[lmb].z_rot = 1.134464;
    parts[lbb].z_rot = 1.2217305;

    let bristle_flow = (pos * SPEED + std::f32::consts::PI).cos() * speed;
    animate_bristle(parts, st.age_ticks, bristle_flow, rtb, rmb, rbb, add_z_rot);
    animate_bristle(parts, st.age_ticks, bristle_flow, ltb, lmb, lbb, add_z_rot);

    parts[body].y = 2.0;
    parts[body].y -= 2.0 * (pos * SPEED).cos() * 2.0 * speed;
    parts[left_leg].y = 8.0 + 2.0 * (pos * SPEED * 0.5 + std::f32::consts::PI).sin() * 2.0 * speed;
    parts[right_leg].y = 8.0 + 2.0 * (pos * SPEED * 0.5).sin() * 2.0 * speed;
}

pub fn baby_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let (pos, speed) = common(model, parts, st);
    let body = model.id("body");
    let left_leg = model.id("left_leg");
    let right_leg = model.id("right_leg");
    let front = model.id("bristle2");
    let middle = model.id("bristle1");
    let bottom = model.id("bristle0");

    parts[body].y = 17.25;
    parts[body].y -= 1.0 * (pos * SPEED).cos() * 2.0 * speed;
    parts[left_leg].y = 20.0 + 2.0 * (pos * SPEED * 0.5 + std::f32::consts::PI).sin() * 2.0 * speed;
    parts[right_leg].y = 20.0 + 2.0 * (pos * SPEED * 0.5).sin() * 2.0 * speed;
    let bristle_flow = (pos * SPEED + std::f32::consts::PI).cos() * speed;
    animate_bristle(
        parts,
        st.age_ticks,
        bristle_flow,
        front,
        middle,
        bottom,
        add_x_rot,
    );
}

fn common(model: &BakedModel, parts: &mut [PartState], st: &EntityState) -> (f32, f32) {
    let pos = st.walk_pos;
    let speed = st.walk_speed.min(0.25);
    let body = model.id("body");
    let left_leg = model.id("left_leg");
    let right_leg = model.id("right_leg");

    if !st.extras.ridden {
        parts[body].x_rot = st.x_rot * DEG_TO_RAD;
        parts[body].y_rot = st.y_rot * DEG_TO_RAD;
    } else {
        parts[body].x_rot = 0.0;
        parts[body].y_rot = 0.0;
    }

    parts[body].z_rot = 0.1 * (pos * SPEED).sin() * 4.0 * speed;
    parts[left_leg].x_rot = (pos * SPEED * 0.5).sin() * 2.0 * speed;
    parts[right_leg].x_rot = (pos * SPEED * 0.5 + std::f32::consts::PI).sin() * 2.0 * speed;
    parts[left_leg].z_rot = 0.17453292 * (pos * SPEED * 0.5).cos() * speed;
    parts[right_leg].z_rot = 0.17453292 * (pos * SPEED * 0.5 + std::f32::consts::PI).cos() * speed;
    (pos, speed)
}
