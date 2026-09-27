use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::state::EntityState;
use crate::util::mth::DEG_TO_RAD;

pub fn arrow_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "back",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(0.0, -2.5, -2.5, 0.0, 5.0, 5.0),
            PartPose::offset_rotation(-11.0, 0.0, 0.0, 0.785_398_2, 0.0, 0.0).with_scale(0.8),
        );
        let cross = CubeList::new().tex_offs(0, 0).add_box_tex_scale(
            -12.0,
            -2.0,
            0.0,
            16.0,
            4.0,
            0.0,
            Grow::NONE,
            1.0,
            0.8,
        );
        root.child(
            "cross_1",
            cross.clone(),
            PartPose::rotation(0.785_398_2, 0.0, 0.0),
        );
        root.child("cross_2", cross, PartPose::rotation(2.356_194_5, 0.0, 0.0));
    }
    LayerDef::create(mesh.transformed(|pose| pose.scaled(0.9)), 32, 32)
}

pub fn arrow_setup_anim(_model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let shake = st.extras.arrow_shake;
    if shake > 0.0 {
        let pow = -(shake * 3.0).sin() * shake;
        parts[0].z_rot += pow * DEG_TO_RAD;
    }
}

pub fn trident_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let pole = mesh.root().child(
            "pole",
            CubeList::new()
                .tex_offs(0, 6)
                .add_box(-0.5, 2.0, -0.5, 1.0, 25.0, 1.0),
            PartPose::ZERO,
        );
        pole.child(
            "base",
            CubeList::new()
                .tex_offs(4, 0)
                .add_box(-1.5, 0.0, -0.5, 3.0, 2.0, 1.0),
            PartPose::ZERO,
        );
        pole.child(
            "left_spike",
            CubeList::new()
                .tex_offs(4, 3)
                .add_box(-2.5, -3.0, -0.5, 1.0, 4.0, 1.0),
            PartPose::ZERO,
        );
        pole.child(
            "middle_spike",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-0.5, -4.0, -0.5, 1.0, 4.0, 1.0),
            PartPose::ZERO,
        );
        pole.child(
            "right_spike",
            CubeList::new()
                .tex_offs(4, 3)
                .mirror()
                .add_box(1.5, -3.0, -0.5, 1.0, 4.0, 1.0),
            PartPose::ZERO,
        );
    }
    LayerDef::create(mesh, 32, 32)
}

pub fn no_anim(_model: &BakedModel, _parts: &mut [PartState], _st: &EntityState) {}

pub fn shulker_bullet_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().child(
        "main",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -4.0, -1.0, 8.0, 8.0, 2.0)
            .tex_offs(0, 10)
            .add_box(-1.0, -4.0, -4.0, 2.0, 8.0, 8.0)
            .tex_offs(20, 0)
            .add_box(-4.0, -1.0, -4.0, 8.0, 2.0, 8.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn shulker_bullet_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let main = model.id("main");
    parts[main].y_rot = st.body_rot * DEG_TO_RAD;
    parts[main].x_rot = st.x_rot * DEG_TO_RAD;
}

pub fn wind_charge_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let bone = mesh
            .root()
            .child("bone", CubeList::new(), PartPose::offset(0.0, 0.0, 0.0));
        bone.child(
            "wind",
            CubeList::new()
                .tex_offs(15, 20)
                .add_box_grow(-4.0, -1.0, -4.0, 8.0, 2.0, 8.0, Grow::all(0.0))
                .tex_offs(0, 9)
                .add_box_grow(-3.0, -2.0, -3.0, 6.0, 4.0, 6.0, Grow::all(0.0)),
            PartPose::offset_rotation(0.0, 0.0, 0.0, 0.0, -0.7854, 0.0),
        );
        bone.child(
            "wind_charge",
            CubeList::new().tex_offs(0, 0).add_box_grow(
                -2.0,
                -2.0,
                -2.0,
                4.0,
                4.0,
                4.0,
                Grow::all(0.0),
            ),
            PartPose::offset(0.0, 0.0, 0.0),
        );
    }
    LayerDef::create(mesh, 64, 32)
}

pub fn wind_charge_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    const ROTATION_SPEED: f32 = 16.0;
    let charge = model.id("wind_charge");
    let wind = model.id("wind");
    parts[charge].y_rot = -st.age_ticks * ROTATION_SPEED * DEG_TO_RAD;
    parts[wind].y_rot = st.age_ticks * ROTATION_SPEED * DEG_TO_RAD;
}

pub fn llama_spit_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().child(
        "main",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, 0.0, 0.0, 2.0, 2.0, 2.0)
            .add_box(0.0, -4.0, 0.0, 2.0, 2.0, 2.0)
            .add_box(0.0, 0.0, -4.0, 2.0, 2.0, 2.0)
            .add_box(0.0, 0.0, 0.0, 2.0, 2.0, 2.0)
            .add_box(2.0, 0.0, 0.0, 2.0, 2.0, 2.0)
            .add_box(0.0, 2.0, 0.0, 2.0, 2.0, 2.0)
            .add_box(0.0, 0.0, 2.0, 2.0, 2.0, 2.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 32)
}

pub fn wither_skull_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().child(
        "head",
        CubeList::new()
            .tex_offs(0, 35)
            .add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 64, 64)
}

pub fn wither_skull_setup_anim(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let head = model.id("head");
    parts[head].y_rot = st.body_rot * DEG_TO_RAD;
    parts[head].x_rot = st.x_rot * DEG_TO_RAD;
}
