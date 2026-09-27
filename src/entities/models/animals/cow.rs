use crate::entities::geom::{CubeList, Grow, LayerDef, MeshDef, PartPose};

fn base_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -4.0, -6.0, 8.0, 8.0, 6.0)
            .tex_offs(1, 33)
            .add_box(-3.0, 1.0, -7.0, 6.0, 3.0, 1.0)
            .tex_offs(22, 0)
            .add_box(-5.0, -5.0, -5.0, 1.0, 3.0, 1.0)
            .tex_offs(22, 0)
            .add_box(4.0, -5.0, -5.0, 1.0, 3.0, 1.0),
        PartPose::offset(0.0, 4.0, -8.0),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(18, 4)
            .add_box(-6.0, -10.0, -7.0, 12.0, 18.0, 10.0)
            .tex_offs(52, 0)
            .add_box(-2.0, 2.0, -8.0, 4.0, 6.0, 1.0),
        PartPose::offset_rotation(0.0, 5.0, 2.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    let left_leg = CubeList::new()
        .mirror()
        .tex_offs(0, 16)
        .add_box(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0);
    let right_leg = CubeList::new()
        .tex_offs(0, 16)
        .add_box(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0);
    root.child(
        "right_hind_leg",
        right_leg.clone(),
        PartPose::offset(-4.0, 12.0, 7.0),
    );
    root.child(
        "left_hind_leg",
        left_leg.clone(),
        PartPose::offset(4.0, 12.0, 7.0),
    );
    root.child(
        "right_front_leg",
        right_leg,
        PartPose::offset(-4.0, 12.0, -5.0),
    );
    root.child(
        "left_front_leg",
        left_leg,
        PartPose::offset(4.0, 12.0, -5.0),
    );
    mesh
}

pub fn layer() -> LayerDef {
    LayerDef::create(base_mesh(), 64, 64)
}

pub fn cold_layer() -> LayerDef {
    let mut mesh = base_mesh();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(20, 32)
            .add_box_grow(-6.0, -10.0, -7.0, 12.0, 18.0, 10.0, Grow::all(0.5))
            .tex_offs(18, 4)
            .add_box(-6.0, -10.0, -7.0, 12.0, 18.0, 10.0)
            .tex_offs(52, 0)
            .add_box(-2.0, 2.0, -8.0, 4.0, 6.0, 1.0),
        PartPose::offset_rotation(0.0, 5.0, 2.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0),
    );
    let head = root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -4.0, -6.0, 8.0, 8.0, 6.0)
            .tex_offs(9, 33)
            .add_box(-3.0, 1.0, -7.0, 6.0, 3.0, 1.0),
        PartPose::offset(0.0, 4.0, -8.0),
    );
    head.child(
        "right_horn",
        CubeList::new()
            .tex_offs(0, 40)
            .add_box(-1.5, -4.5, -0.5, 2.0, 6.0, 2.0),
        PartPose::offset_rotation(-4.5, -2.5, -3.5, 1.5708, 0.0, 0.0),
    );
    head.child(
        "left_horn",
        CubeList::new()
            .tex_offs(0, 32)
            .add_box(-1.5, -3.0, -0.5, 2.0, 6.0, 2.0),
        PartPose::offset_rotation(5.5, -2.5, -5.0, 1.5708, 0.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn warm_layer() -> LayerDef {
    let mut mesh = base_mesh();
    mesh.root().child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -4.0, -6.0, 8.0, 8.0, 6.0)
            .tex_offs(1, 33)
            .add_box(-3.0, 1.0, -7.0, 6.0, 3.0, 1.0)
            .tex_offs(27, 0)
            .add_box(-8.0, -3.0, -5.0, 4.0, 2.0, 2.0)
            .tex_offs(39, 0)
            .add_box(-8.0, -5.0, -5.0, 2.0, 2.0, 2.0)
            .tex_offs(27, 0)
            .mirror()
            .add_box(4.0, -3.0, -5.0, 4.0, 2.0, 2.0)
            .mirror_if(false)
            .tex_offs(39, 0)
            .mirror()
            .add_box(6.0, -5.0, -5.0, 2.0, 2.0, 2.0)
            .mirror_if(false),
        PartPose::offset(0.0, 4.0, -8.0),
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn baby_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "head",
        CubeList::new()
            .tex_offs(0, 18)
            .add_box(-3.0, -4.569, -4.8333, 6.0, 6.0, 5.0)
            .tex_offs(8, 29)
            .add_box(3.0, -5.569, -3.8333, 1.0, 2.0, 1.0)
            .tex_offs(4, 29)
            .mirror()
            .add_box(-4.0, -5.569, -3.8333, 1.0, 2.0, 1.0)
            .mirror_if(false)
            .tex_offs(12, 29)
            .add_box(-2.0, -1.569, -5.8333, 4.0, 3.0, 1.0),
        PartPose::offset(0.0, 13.569, -5.1667),
    );
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-7.0, -7.0, -1.0, 8.0, 6.0, 12.0),
        PartPose::offset(3.0, 19.0, -5.0),
    );
    root.child(
        "right_front_leg",
        CubeList::new()
            .tex_offs(22, 18)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset(-2.5, 18.0, -3.5),
    );
    root.child(
        "left_front_leg",
        CubeList::new()
            .tex_offs(34, 18)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset(2.5, 18.0, -3.5),
    );
    root.child(
        "right_hind_leg",
        CubeList::new()
            .tex_offs(22, 27)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset(-2.5, 18.0, 3.5),
    );
    root.child(
        "left_hind_leg",
        CubeList::new()
            .tex_offs(34, 27)
            .add_box(-1.5, 0.0, -1.5, 3.0, 6.0, 3.0),
        PartPose::offset(2.5, 18.0, 3.5),
    );
    LayerDef::create(mesh, 64, 64)
}
