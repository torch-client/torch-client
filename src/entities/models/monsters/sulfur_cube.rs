use crate::entities::geom::{LayerDef, MeshDef, PartPose};

pub fn outer_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().part(
        "cube",
        (0, 0),
        [-9.0, -9.0, -9.0, 18.0, 18.0, 18.0],
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 128, 128)
}

pub fn inner_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().part(
        "cube",
        (0, 36),
        [-8.0, -8.0, -8.0, 16.0, 16.0, 16.0],
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 128, 128)
}

pub fn small_outer_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().part(
        "cube",
        (0, 0),
        [-5.0, -5.0, -5.0, 10.0, 10.0, 10.0],
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn small_inner_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().part(
        "cube",
        (0, 20),
        [-4.0, -4.0, -4.0, 8.0, 8.0, 8.0],
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 64)
}
