use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

fn triangle_wave(value: f32, period: f32) -> f32 {
    ((value % period - period * 0.5).abs() - period * 0.25) / (period * 0.25)
}

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -12.0, -5.5, 8.0, 10.0, 8.0)
            .tex_offs(24, 0)
            .add_box(-1.0, -5.0, -7.5, 2.0, 4.0, 2.0),
        PartPose::offset(0.0, -7.0, -2.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 40)
            .add_box(-9.0, -2.0, -6.0, 18.0, 12.0, 11.0)
            .tex_offs(0, 70)
            .add_box_grow(-4.5, 10.0, -3.0, 9.0, 5.0, 6.0, Grow::all(0.5)),
        PartPose::offset(0.0, -7.0, 0.0),
    );
    root.child(
        "right_arm",
        CubeList::new()
            .tex_offs(60, 21)
            .add_box(-13.0, -2.5, -3.0, 4.0, 30.0, 6.0),
        PartPose::offset(0.0, -7.0, 0.0),
    );
    root.child(
        "left_arm",
        CubeList::new()
            .tex_offs(60, 58)
            .add_box(9.0, -2.5, -3.0, 4.0, 30.0, 6.0),
        PartPose::offset(0.0, -7.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(37, 0)
            .add_box(-3.5, -3.0, -3.0, 6.0, 16.0, 5.0),
        PartPose::offset(-4.0, 11.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(60, 0)
            .mirror()
            .add_box(-3.5, -3.0, -3.0, 6.0, 16.0, 5.0),
        PartPose::offset(5.0, 11.0, 0.0),
    );
    LayerDef::create(mesh, 128, 128)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    let right_arm = model.id("right_arm");
    let left_arm = model.id("left_arm");
    let right_leg = model.id("right_leg");
    let left_leg = model.id("left_leg");

    let attack = st.extras.attack_ticks;
    let speed = st.walk_speed;
    let pos = st.walk_pos;
    if attack > 0.0 {
        parts[right_arm].x_rot = -2.0 + 1.5 * triangle_wave(attack, 10.0);
        parts[left_arm].x_rot = -2.0 + 1.5 * triangle_wave(attack, 10.0);
    } else if st.extras.offer_flower_ticks > 0 {
        parts[right_arm].x_rot =
            -0.8 + 0.025 * triangle_wave(st.extras.offer_flower_ticks as f32, 70.0);
        parts[left_arm].x_rot = 0.0;
    } else {
        parts[right_arm].x_rot = (-0.2 + 1.5 * triangle_wave(pos, 13.0)) * speed;
        parts[left_arm].x_rot = (-0.2 - 1.5 * triangle_wave(pos, 13.0)) * speed;
    }

    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    parts[right_leg].x_rot = -1.5 * triangle_wave(pos, 13.0) * speed;
    parts[left_leg].x_rot = 1.5 * triangle_wave(pos, 13.0) * speed;
    parts[right_leg].y_rot = 0.0;
    parts[left_leg].y_rot = 0.0;
}
