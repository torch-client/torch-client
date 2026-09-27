use std::f32::consts::{FRAC_PI_2, PI};

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

use super::humanoid::{self, DEG_TO_RAD};

pub const VILLAGER_LIKE_SCALE: f32 = 0.937_5;

pub fn villager_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -10.0, -4.0, 8.0, 10.0, 8.0),
        PartPose::ZERO,
    );
    head.child(
        "hat",
        CubeList::new().tex_offs(32, 0).add_box_grow(
            -4.0,
            -10.0,
            -4.0,
            8.0,
            10.0,
            8.0,
            Grow::all(0.51),
        ),
        PartPose::ZERO,
    )
    .child(
        "hat_rim",
        CubeList::new()
            .tex_offs(30, 47)
            .add_box(-8.0, -8.0, -6.0, 16.0, 16.0, 1.0),
        PartPose::rotation(-FRAC_PI_2, 0.0, 0.0),
    );
    head.child(
        "nose",
        CubeList::new()
            .tex_offs(24, 0)
            .add_box(-1.0, -1.0, -6.0, 2.0, 4.0, 2.0),
        PartPose::offset(0.0, -2.0, 0.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(16, 20)
            .add_box(-4.0, 0.0, -3.0, 8.0, 12.0, 6.0),
        PartPose::ZERO,
    )
    .child(
        "jacket",
        CubeList::new().tex_offs(0, 38).add_box_grow(
            -4.0,
            0.0,
            -3.0,
            8.0,
            20.0,
            6.0,
            Grow::all(0.5),
        ),
        PartPose::ZERO,
    );
    root.child(
        "arms",
        CubeList::new()
            .tex_offs(44, 22)
            .add_box(-8.0, -2.0, -2.0, 4.0, 8.0, 4.0)
            .tex_offs(44, 22)
            .add_box_mirror(4.0, -2.0, -2.0, 4.0, 8.0, 4.0, true)
            .tex_offs(40, 38)
            .add_box(-4.0, 2.0, -2.0, 8.0, 4.0, 4.0),
        PartPose::offset_rotation(0.0, 3.0, -1.0, -0.75, 0.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 22)
            .add_box(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0),
        PartPose::offset(-2.0, 12.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 22)
            .mirror()
            .add_box(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0),
        PartPose::offset(2.0, 12.0, 0.0),
    );
    mesh
}

pub fn villager_layer() -> LayerDef {
    LayerDef::create(
        humanoid::scaling(villager_mesh(), VILLAGER_LIKE_SCALE),
        64,
        64,
    )
}

pub fn villager_no_hat_layer() -> LayerDef {
    let mut mesh = villager_mesh();
    humanoid::clear_cubes(
        mesh.root(),
        &["head", "head/hat", "head/hat/hat_rim", "head/nose"],
    );
    LayerDef::create(humanoid::scaling(mesh, VILLAGER_LIKE_SCALE), 64, 64)
}

pub fn baby_villager_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let arms = root.child("arms", CubeList::new(), PartPose::offset(0.0, 17.5, 0.0));
    arms.child(
        "right_hand",
        CubeList::new()
            .tex_offs(36, 15)
            .add_box(-1.0, -2.4925, -1.8401, 2.0, 4.0, 2.0)
            .tex_offs(16, 15)
            .add_box(5.0, -2.4925, -1.8401, 2.0, 4.0, 2.0),
        PartPose::offset_rotation(-3.0, 1.4025, -0.9599, -1.047_2, 0.0, 0.0),
    );
    arms.child(
        "middlearm_r1",
        CubeList::new()
            .tex_offs(24, 17)
            .add_box(-2.0, -0.9924, -0.9825, 4.0, 2.0, 2.0),
        PartPose::offset_rotation(0.0, 0.9024, -1.8175, -1.047_2, 0.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(8, 23)
            .add_box(-1.0, -0.5, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(-1.0, 21.5, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 23)
            .add_box(-1.0, -0.5, -1.0, 2.0, 3.0, 2.0),
        PartPose::offset(1.0, 21.5, 0.0),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -8.0, -3.5, 8.0, 8.0, 7.0),
        PartPose::offset(0.0, 16.0, 0.0),
    );
    head.child(
        "hat",
        CubeList::new().tex_offs(0, 30).add_box_grow(
            -4.0,
            -4.0,
            -3.5,
            8.0,
            8.0,
            7.0,
            Grow::all(0.3),
        ),
        PartPose::offset(0.0, -4.0, 0.0),
    );
    head.child(
        "hat_rim",
        CubeList::new()
            .tex_offs(0, 45)
            .add_box(-7.0, -0.5, -6.0, 14.0, 1.0, 12.0),
        PartPose::offset(0.0, -4.5, 0.0),
    );
    head.child(
        "nose",
        CubeList::new()
            .tex_offs(23, 0)
            .add_box(-1.0, 0.0, -0.5, 2.0, 2.0, 1.0),
        PartPose::offset(0.0, -2.0, -4.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box(-2.0, -2.75, -1.5, 4.0, 5.0, 3.0),
        PartPose::offset(0.0, 18.75, 0.0),
    );
    root.child(
        "bb_main",
        CubeList::new().tex_offs(16, 21).add_box_grow(
            -2.5,
            -8.0,
            -1.5,
            4.0,
            6.0,
            3.0,
            Grow::all(0.2),
        ),
        PartPose::offset(0.5, 24.0, 0.0),
    );
    mesh
}

pub fn baby_villager_layer() -> LayerDef {
    LayerDef::create(baby_villager_mesh(), 64, 64)
}

pub fn baby_villager_no_hat_layer() -> LayerDef {
    let mut mesh = baby_villager_mesh();
    humanoid::clear_cubes(
        mesh.root(),
        &["head", "head/hat", "head/hat_rim", "head/nose"],
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    if st.extras.unhappy_counter > 0 {
        parts[head].z_rot = 0.3 * (0.45 * st.age_ticks).sin();
        parts[head].x_rot = 0.4;
    } else {
        parts[head].z_rot = 0.0;
    }

    let right_leg = model.id("right_leg");
    let left_leg = model.id("left_leg");
    parts[right_leg].x_rot = (st.walk_pos * 0.6662).cos() * 1.4 * st.walk_speed * 0.5;
    parts[left_leg].x_rot = (st.walk_pos * 0.6662 + PI).cos() * 1.4 * st.walk_speed * 0.5;
    parts[right_leg].y_rot = 0.0;
    parts[left_leg].y_rot = 0.0;
}

pub fn witch_layer() -> LayerDef {
    let mut mesh = villager_mesh();
    {
        let root = mesh.root();
        let head = root.child(
            "head",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-4.0, -10.0, -4.0, 8.0, 10.0, 8.0),
            PartPose::ZERO,
        );
        let hat = head.child(
            "hat",
            CubeList::new()
                .tex_offs(0, 64)
                .add_box(0.0, 0.0, 0.0, 10.0, 2.0, 10.0),
            PartPose::offset(-5.0, -10.031_25, -5.0),
        );
        let hat2 = hat.child(
            "hat2",
            CubeList::new()
                .tex_offs(0, 76)
                .add_box(0.0, 0.0, 0.0, 7.0, 4.0, 7.0),
            PartPose::offset_rotation(1.75, -4.0, 2.0, -0.052_359_88, 0.0, 0.026_179_94),
        );
        let hat3 = hat2.child(
            "hat3",
            CubeList::new()
                .tex_offs(0, 87)
                .add_box(0.0, 0.0, 0.0, 4.0, 4.0, 4.0),
            PartPose::offset_rotation(1.75, -4.0, 2.0, -0.104_719_76, 0.0, 0.052_359_88),
        );
        hat3.child(
            "hat4",
            CubeList::new().tex_offs(0, 95).add_box_grow(
                0.0,
                0.0,
                0.0,
                1.0,
                2.0,
                1.0,
                Grow::all(0.25),
            ),
            PartPose::offset_rotation(1.75, -2.0, 2.0, -0.209_439_52, 0.0, 0.104_719_76),
        );
    }
    mesh.root().get("head/nose").child(
        "mole",
        CubeList::new().tex_offs(0, 0).add_box_grow(
            0.0,
            3.0,
            -6.75,
            1.0,
            1.0,
            1.0,
            Grow::all(-0.25),
        ),
        PartPose::offset(0.0, -2.0, 0.0),
    );
    LayerDef::create(humanoid::scaling(mesh, VILLAGER_LIKE_SCALE), 64, 128)
}

pub fn witch_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].y_rot = st.y_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
    let right_leg = model.id("right_leg");
    let left_leg = model.id("left_leg");
    parts[right_leg].x_rot = (st.walk_pos * 0.6662).cos() * 1.4 * st.walk_speed * 0.5;
    parts[left_leg].x_rot = (st.walk_pos * 0.6662 + PI).cos() * 1.4 * st.walk_speed * 0.5;

    let speed = 0.01 * (st.id.rem_euclid(10)) as f32;
    let nose = model.id("nose");
    parts[nose].x_rot = (st.age_ticks * speed).sin() * 4.5 * DEG_TO_RAD;
    parts[nose].z_rot = (st.age_ticks * speed).cos() * 2.5 * DEG_TO_RAD;
    if !st.extras.main_hand.is_empty() {
        parts[nose].set_pos(0.0, 1.0, -1.5);
        parts[nose].x_rot = -0.9;
    }
}
