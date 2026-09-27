use crate::entities::geom::{BakedModel, CubeList, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;

pub fn layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "bottom",
        CubeList::new()
            .tex_offs(0, 10)
            .add_box(-10.0, -8.0, -1.0, 20.0, 16.0, 2.0),
        PartPose::offset_rotation(0.0, 4.0, 0.0, 1.570_796_4, 0.0, 0.0),
    );
    root.child(
        "front",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -9.0, -1.0, 16.0, 8.0, 2.0),
        PartPose::offset_rotation(-9.0, 4.0, 0.0, 0.0, 4.712_389, 0.0),
    );
    root.child(
        "back",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -9.0, -1.0, 16.0, 8.0, 2.0),
        PartPose::offset_rotation(9.0, 4.0, 0.0, 0.0, 1.570_796_4, 0.0),
    );
    root.child(
        "left",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -9.0, -1.0, 16.0, 8.0, 2.0),
        PartPose::offset_rotation(0.0, 4.0, -7.0, 0.0, 3.141_592_7, 0.0),
    );
    root.child(
        "right",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-8.0, -9.0, -1.0, 16.0, 8.0, 2.0),
        PartPose::offset(0.0, 4.0, 7.0),
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn setup_anim(_model: &BakedModel, _parts: &mut [PartState], _st: &EntityState) {}
