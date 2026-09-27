use crate::entities::geom::{
    BakedModel, CubeList, LayerDef, MeshDef, PartDef, PartPose, PartState,
};
use crate::entities::state::EntityState;

fn boat_common_parts(root: &mut PartDef) {
    root.child(
        "bottom",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-14.0, -9.0, -3.0, 28.0, 16.0, 3.0),
        PartPose::offset_rotation(0.0, 3.0, 1.0, 1.570_796_4, 0.0, 0.0),
    );
    root.child(
        "back",
        CubeList::new()
            .tex_offs(0, 19)
            .add_box(-13.0, -7.0, -1.0, 18.0, 6.0, 2.0),
        PartPose::offset_rotation(-15.0, 4.0, 4.0, 0.0, 4.712_389, 0.0),
    );
    root.child(
        "front",
        CubeList::new()
            .tex_offs(0, 27)
            .add_box(-8.0, -7.0, -1.0, 16.0, 6.0, 2.0),
        PartPose::offset_rotation(15.0, 4.0, 0.0, 0.0, 1.570_796_4, 0.0),
    );
    root.child(
        "right",
        CubeList::new()
            .tex_offs(0, 35)
            .add_box(-14.0, -7.0, -1.0, 28.0, 6.0, 2.0),
        PartPose::offset_rotation(0.0, 4.0, -9.0, 0.0, 3.141_592_7, 0.0),
    );
    root.child(
        "left",
        CubeList::new()
            .tex_offs(0, 43)
            .add_box(-14.0, -7.0, -1.0, 28.0, 6.0, 2.0),
        PartPose::offset(0.0, 4.0, 9.0),
    );
    root.child(
        "left_paddle",
        CubeList::new()
            .tex_offs(62, 0)
            .add_box(-1.0, 0.0, -5.0, 2.0, 2.0, 18.0)
            .add_box(-1.001, -3.0, 8.0, 1.0, 6.0, 7.0),
        PartPose::offset_rotation(3.0, -5.0, 9.0, 0.0, 0.0, 0.196_349_55),
    );
    root.child(
        "right_paddle",
        CubeList::new()
            .tex_offs(62, 20)
            .add_box(-1.0, 0.0, -5.0, 2.0, 2.0, 18.0)
            .add_box(0.001, -3.0, 8.0, 1.0, 6.0, 7.0),
        PartPose::offset_rotation(3.0, -5.0, -9.0, 0.0, 3.141_592_7, 0.196_349_55),
    );
}

fn raft_common_parts(root: &mut PartDef) {
    root.child(
        "bottom",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-14.0, -11.0, -4.0, 28.0, 20.0, 4.0)
            .tex_offs(0, 0)
            .add_box(-14.0, -9.0, -8.0, 28.0, 16.0, 4.0),
        PartPose::offset_rotation(0.0, -2.1, 1.0, 1.5708, 0.0, 0.0),
    );
    root.child(
        "left_paddle",
        CubeList::new()
            .tex_offs(0, 24)
            .add_box(-1.0, 0.0, -5.0, 2.0, 2.0, 18.0)
            .add_box(-1.001, -3.0, 8.0, 1.0, 6.0, 7.0),
        PartPose::offset_rotation(3.0, -4.0, 9.0, 0.0, 0.0, 0.196_349_55),
    );
    root.child(
        "right_paddle",
        CubeList::new()
            .tex_offs(40, 24)
            .add_box(-1.0, 0.0, -5.0, 2.0, 2.0, 18.0)
            .add_box(0.001, -3.0, 8.0, 1.0, 6.0, 7.0),
        PartPose::offset_rotation(3.0, -4.0, -9.0, 0.0, 3.141_592_7, 0.196_349_55),
    );
}

fn add_chest_parts(root: &mut PartDef, bottom_y: f32, lid_y: f32, lock_y: f32) {
    root.child(
        "chest_bottom",
        CubeList::new()
            .tex_offs(0, 76)
            .add_box(0.0, 0.0, 0.0, 12.0, 8.0, 12.0),
        PartPose::offset_rotation(-2.0, bottom_y, -6.0, 0.0, -1.570_796_4, 0.0),
    );
    root.child(
        "chest_lid",
        CubeList::new()
            .tex_offs(0, 59)
            .add_box(0.0, 0.0, 0.0, 12.0, 4.0, 12.0),
        PartPose::offset_rotation(-2.0, lid_y, -6.0, 0.0, -1.570_796_4, 0.0),
    );
    root.child(
        "chest_lock",
        CubeList::new()
            .tex_offs(0, 59)
            .add_box(0.0, 0.0, 0.0, 2.0, 4.0, 1.0),
        PartPose::offset_rotation(-1.0, lock_y, -1.0, 0.0, -1.570_796_4, 0.0),
    );
}

pub fn boat_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    boat_common_parts(mesh.root());
    LayerDef::create(mesh, 128, 64)
}

pub fn chest_boat_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    boat_common_parts(mesh.root());
    add_chest_parts(mesh.root(), -5.0, -9.0, -6.0);
    LayerDef::create(mesh, 128, 128)
}

pub fn raft_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    raft_common_parts(mesh.root());
    LayerDef::create(mesh, 128, 64)
}

pub fn chest_raft_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    raft_common_parts(mesh.root());
    add_chest_parts(mesh.root(), -10.1, -14.1, -11.1);
    LayerDef::create(mesh, 128, 128)
}

#[allow(
    dead_code,
    reason = "the water patch cannot be drawn as a textured layer; see the doc comment"
)]
pub fn water_patch_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().child(
        "water_patch",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-14.0, -9.0, -3.0, 28.0, 16.0, 3.0),
        PartPose::offset_rotation(0.0, -3.0, 1.0, 1.570_796_4, 0.0, 0.0),
    );
    LayerDef::create(mesh, 0, 0)
}

fn clamped_lerp(factor: f32, min: f32, max: f32) -> f32 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        min + factor * (max - min)
    }
}

fn animate_paddle(time: f32, side: i32, paddle: &mut PartState) {
    paddle.x_rot = clamped_lerp(((-time).sin() + 1.0) / 2.0, -1.047_197_6, -0.261_799_4);
    paddle.y_rot = clamped_lerp(((-time + 1.0).sin() + 1.0) / 2.0, -0.785_398_2, 0.785_398_2);
    if side == 1 {
        paddle.y_rot = 3.141_592_7 - paddle.y_rot;
    }
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let left = model.id("left_paddle");
    let right = model.id("right_paddle");
    animate_paddle(st.extras.paddle_left, 0, &mut parts[left]);
    animate_paddle(st.extras.paddle_right, 1, &mut parts[right]);
}
