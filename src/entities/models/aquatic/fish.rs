#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, CubeList, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::models::aquatic::scaling;
use crate::entities::state::EntityState;

pub fn cod_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-1.0, -2.0, 0.0, 2.0, 4.0, 7.0),
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.child(
        "head",
        CubeList::new()
            .tex_offs(11, 0)
            .add_box(-1.0, -2.0, -3.0, 2.0, 4.0, 3.0),
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.child(
        "nose",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-1.0, -2.0, -1.0, 2.0, 3.0, 1.0),
        PartPose::offset(0.0, 22.0, -3.0),
    );
    root.child(
        "right_fin",
        CubeList::new()
            .tex_offs(22, 1)
            .add_box(-2.0, 0.0, -1.0, 2.0, 0.0, 2.0),
        PartPose::offset_rotation(-1.0, 23.0, 0.0, 0.0, 0.0, -0.785_398_2),
    );
    root.child(
        "left_fin",
        CubeList::new()
            .tex_offs(22, 4)
            .add_box(0.0, 0.0, -1.0, 2.0, 0.0, 2.0),
        PartPose::offset_rotation(1.0, 23.0, 0.0, 0.0, 0.0, 0.785_398_2),
    );
    root.child(
        "tail_fin",
        CubeList::new()
            .tex_offs(22, 3)
            .add_box(0.0, -2.0, 0.0, 0.0, 4.0, 4.0),
        PartPose::offset(0.0, 22.0, 7.0),
    );
    root.child(
        "top_fin",
        CubeList::new()
            .tex_offs(20, -6)
            .add_box(0.0, -1.0, -1.0, 0.0, 1.0, 6.0),
        PartPose::offset(0.0, 20.0, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn cod_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let amplitude = if st.is_in_water { 1.0 } else { 1.5 };
    parts[model.id("tail_fin")].y_rot = -amplitude * 0.45 * (0.6 * st.age_ticks).sin();
}

pub fn salmon_mesh() -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body_front",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-1.5, -2.5, 0.0, 3.0, 5.0, 8.0),
        PartPose::offset(0.0, 20.0, -7.2),
    );
    root.child(
        "body_back",
        CubeList::new()
            .tex_offs(0, 13)
            .add_box(-1.5, -2.5, 0.0, 3.0, 5.0, 8.0),
        PartPose::offset(0.0, 20.0, 0.800_000_2),
    );
    root.child(
        "head",
        CubeList::new()
            .tex_offs(22, 0)
            .add_box(-1.0, -2.0, -3.0, 2.0, 4.0, 3.0),
        PartPose::offset(0.0, 20.0, -7.2),
    );
    root.get("body_back").child(
        "back_fin",
        CubeList::new()
            .tex_offs(20, 10)
            .add_box(0.0, -2.5, 0.0, 0.0, 5.0, 6.0),
        PartPose::offset(0.0, 0.0, 8.0),
    );
    root.get("body_front").child(
        "top_front_fin",
        CubeList::new()
            .tex_offs(2, 1)
            .add_box(0.0, 0.0, 0.0, 0.0, 2.0, 3.0),
        PartPose::offset(0.0, -4.5, 5.0),
    );
    root.get("body_back").child(
        "top_back_fin",
        CubeList::new()
            .tex_offs(0, 2)
            .add_box(0.0, 0.0, 0.0, 0.0, 2.0, 4.0),
        PartPose::offset(0.0, -4.5, -1.0),
    );
    root.child(
        "right_fin",
        CubeList::new()
            .tex_offs(-4, 0)
            .add_box(-2.0, 0.0, 0.0, 2.0, 0.0, 2.0),
        PartPose::offset_rotation(-1.5, 21.5, -7.2, 0.0, 0.0, -0.785_398_2),
    );
    root.child(
        "left_fin",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(0.0, 0.0, 0.0, 2.0, 0.0, 2.0),
        PartPose::offset_rotation(1.5, 21.5, -7.2, 0.0, 0.0, 0.785_398_2),
    );
    mesh
}

pub fn salmon_layer() -> LayerDef {
    LayerDef::create(salmon_mesh(), 32, 32)
}

pub fn salmon_small_layer() -> LayerDef {
    LayerDef::create(scaling(salmon_mesh(), 0.5), 32, 32)
}

pub fn salmon_large_layer() -> LayerDef {
    LayerDef::create(scaling(salmon_mesh(), 1.5), 32, 32)
}

pub fn salmon_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let (amplitude, angle) = if st.is_in_water {
        (1.0, 1.0)
    } else {
        (1.3, 1.7)
    };
    parts[model.id("body_back")].y_rot = -amplitude * 0.25 * (angle * 0.6 * st.age_ticks).sin();
}

fn tropical_small_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(-1.0, -1.5, -3.0, 2.0, 3.0, 6.0, g),
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.child(
        "tail",
        CubeList::new()
            .tex_offs(22, -6)
            .add_box_grow(0.0, -1.5, 0.0, 0.0, 3.0, 6.0, g),
        PartPose::offset(0.0, 22.0, 3.0),
    );
    root.child(
        "right_fin",
        CubeList::new()
            .tex_offs(2, 16)
            .add_box_grow(-2.0, -1.0, 0.0, 2.0, 2.0, 0.0, g),
        PartPose::offset_rotation(-1.0, 22.5, 0.0, 0.0, 0.785_398_2, 0.0),
    );
    root.child(
        "left_fin",
        CubeList::new()
            .tex_offs(2, 12)
            .add_box_grow(0.0, -1.0, 0.0, 2.0, 2.0, 0.0, g),
        PartPose::offset_rotation(1.0, 22.5, 0.0, 0.0, -0.785_398_2, 0.0),
    );
    root.child(
        "top_fin",
        CubeList::new()
            .tex_offs(10, -5)
            .add_box_grow(0.0, -3.0, 0.0, 0.0, 3.0, 6.0, g),
        PartPose::offset(0.0, 20.5, -3.0),
    );
    mesh
}

fn tropical_large_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 20)
            .add_box_grow(-1.0, -3.0, -3.0, 2.0, 6.0, 6.0, g),
        PartPose::offset(0.0, 19.0, 0.0),
    );
    root.child(
        "tail",
        CubeList::new()
            .tex_offs(21, 16)
            .add_box_grow(0.0, -3.0, 0.0, 0.0, 6.0, 5.0, g),
        PartPose::offset(0.0, 19.0, 3.0),
    );
    root.child(
        "right_fin",
        CubeList::new()
            .tex_offs(2, 16)
            .add_box_grow(-2.0, 0.0, 0.0, 2.0, 2.0, 0.0, g),
        PartPose::offset_rotation(-1.0, 20.0, 0.0, 0.0, 0.785_398_2, 0.0),
    );
    root.child(
        "left_fin",
        CubeList::new()
            .tex_offs(2, 12)
            .add_box_grow(0.0, 0.0, 0.0, 2.0, 2.0, 0.0, g),
        PartPose::offset_rotation(1.0, 20.0, 0.0, 0.0, -0.785_398_2, 0.0),
    );
    root.child(
        "top_fin",
        CubeList::new()
            .tex_offs(20, 11)
            .add_box_grow(0.0, -4.0, 0.0, 0.0, 4.0, 6.0, g),
        PartPose::offset(0.0, 16.0, -3.0),
    );
    root.child(
        "bottom_fin",
        CubeList::new()
            .tex_offs(20, 21)
            .add_box_grow(0.0, 0.0, 0.0, 0.0, 4.0, 6.0, g),
        PartPose::offset(0.0, 22.0, -3.0),
    );
    mesh
}

const FISH_PATTERN_DEFORMATION: Grow = Grow::all(0.008);

pub fn tropical_small_layer() -> LayerDef {
    LayerDef::create(tropical_small_mesh(Grow::NONE), 32, 32)
}

pub fn tropical_small_pattern_layer() -> LayerDef {
    LayerDef::create(tropical_small_mesh(FISH_PATTERN_DEFORMATION), 32, 32)
}

pub fn tropical_large_layer() -> LayerDef {
    LayerDef::create(tropical_large_mesh(Grow::NONE), 32, 32)
}

pub fn tropical_large_pattern_layer() -> LayerDef {
    LayerDef::create(tropical_large_mesh(FISH_PATTERN_DEFORMATION), 32, 32)
}

pub fn tropical_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let amplitude = if st.is_in_water { 1.0 } else { 1.5 };
    parts[model.id("tail")].y_rot = -amplitude * 0.45 * (0.6 * st.age_ticks).sin();
}

pub fn pufferfish_small_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 27)
            .add_box(-1.5, -2.0, -1.5, 3.0, 2.0, 3.0),
        PartPose::offset(0.0, 23.0, 0.0),
    );
    root.child(
        "right_eye",
        CubeList::new()
            .tex_offs(24, 6)
            .add_box(-1.5, 0.0, -1.5, 1.0, 1.0, 1.0),
        PartPose::offset(0.0, 20.0, 0.0),
    );
    root.child(
        "left_eye",
        CubeList::new()
            .tex_offs(28, 6)
            .add_box(0.5, 0.0, -1.5, 1.0, 1.0, 1.0),
        PartPose::offset(0.0, 20.0, 0.0),
    );
    root.child(
        "back_fin",
        CubeList::new()
            .tex_offs(-3, 0)
            .add_box(-1.5, 0.0, 0.0, 3.0, 0.0, 3.0),
        PartPose::offset(0.0, 22.0, 1.5),
    );
    root.child(
        "right_fin",
        CubeList::new()
            .tex_offs(25, 0)
            .add_box(-1.0, 0.0, 0.0, 1.0, 0.0, 2.0),
        PartPose::offset(-1.5, 22.0, -1.5),
    );
    root.child(
        "left_fin",
        CubeList::new()
            .tex_offs(25, 0)
            .add_box(0.0, 0.0, 0.0, 1.0, 0.0, 2.0),
        PartPose::offset(1.5, 22.0, -1.5),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn pufferfish_small_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let sway = 0.4 * (st.age_ticks * 0.2).sin();
    parts[model.id("right_fin")].z_rot = -0.2 + sway;
    parts[model.id("left_fin")].z_rot = 0.2 - sway;
}

pub fn pufferfish_mid_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(12, 22)
            .add_box(-2.5, -5.0, -2.5, 5.0, 5.0, 5.0),
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.child(
        "right_blue_fin",
        CubeList::new()
            .tex_offs(24, 0)
            .add_box(-2.0, 0.0, 0.0, 2.0, 0.0, 2.0),
        PartPose::offset(-2.5, 18.0, -1.5),
    );
    root.child(
        "left_blue_fin",
        CubeList::new()
            .tex_offs(24, 3)
            .add_box(0.0, 0.0, 0.0, 2.0, 0.0, 2.0),
        PartPose::offset(2.5, 18.0, -1.5),
    );
    root.child(
        "top_front_fin",
        CubeList::new()
            .tex_offs(19, 17)
            .add_box(-2.5, -1.0, 0.0, 5.0, 1.0, 0.0),
        PartPose::offset_rotation(0.0, 17.0, -2.5, 0.785_398_2, 0.0, 0.0),
    );
    root.child(
        "top_back_fin",
        CubeList::new()
            .tex_offs(11, 17)
            .add_box(-2.5, -1.0, 0.0, 5.0, 1.0, 0.0),
        PartPose::offset_rotation(0.0, 17.0, 2.5, -0.785_398_2, 0.0, 0.0),
    );
    root.child(
        "right_front_fin",
        CubeList::new()
            .tex_offs(5, 17)
            .add_box(-1.0, -5.0, 0.0, 1.0, 5.0, 0.0),
        PartPose::offset_rotation(-2.5, 22.0, -2.5, 0.0, -0.785_398_2, 0.0),
    );
    root.child(
        "right_back_fin",
        CubeList::new()
            .tex_offs(9, 17)
            .add_box(-1.0, -5.0, 0.0, 1.0, 5.0, 0.0),
        PartPose::offset_rotation(-2.5, 22.0, 2.5, 0.0, 0.785_398_2, 0.0),
    );
    root.child(
        "left_back_fin",
        CubeList::new()
            .tex_offs(1, 17)
            .add_box(0.0, -5.0, 0.0, 1.0, 5.0, 0.0),
        PartPose::offset_rotation(2.5, 22.0, 2.5, 0.0, -0.785_398_2, 0.0),
    );
    root.child(
        "left_front_fin",
        CubeList::new()
            .tex_offs(1, 17)
            .add_box(0.0, -5.0, 0.0, 1.0, 5.0, 0.0),
        PartPose::offset_rotation(2.5, 22.0, -2.5, 0.0, 0.785_398_2, 0.0),
    );
    root.child(
        "bottom_back_fin",
        CubeList::new()
            .tex_offs(18, 20)
            .add_box(0.0, 0.0, 0.0, 5.0, 1.0, 0.0),
        PartPose::offset_rotation(-2.5, 22.0, 2.5, 0.785_398_2, 0.0, 0.0),
    );
    root.child(
        "bottom_front_fin",
        CubeList::new()
            .tex_offs(17, 19)
            .add_box(-2.5, 0.0, 0.0, 5.0, 1.0, 1.0),
        PartPose::offset_rotation(0.0, 22.0, -2.5, -0.785_398_2, 0.0, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn pufferfish_big_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0),
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.child(
        "right_blue_fin",
        CubeList::new()
            .tex_offs(24, 0)
            .add_box(-2.0, 0.0, -1.0, 2.0, 1.0, 2.0),
        PartPose::offset(-4.0, 15.0, -2.0),
    );
    root.child(
        "left_blue_fin",
        CubeList::new()
            .tex_offs(24, 3)
            .add_box(0.0, 0.0, -1.0, 2.0, 1.0, 2.0),
        PartPose::offset(4.0, 15.0, -2.0),
    );
    root.child(
        "top_front_fin",
        CubeList::new()
            .tex_offs(15, 17)
            .add_box(-4.0, -1.0, 0.0, 8.0, 1.0, 0.0),
        PartPose::offset_rotation(0.0, 14.0, -4.0, 0.785_398_2, 0.0, 0.0),
    );
    root.child(
        "top_middle_fin",
        CubeList::new()
            .tex_offs(14, 16)
            .add_box(-4.0, -1.0, 0.0, 8.0, 1.0, 1.0),
        PartPose::offset(0.0, 14.0, 0.0),
    );
    root.child(
        "top_back_fin",
        CubeList::new()
            .tex_offs(23, 18)
            .add_box(-4.0, -1.0, 0.0, 8.0, 1.0, 0.0),
        PartPose::offset_rotation(0.0, 14.0, 4.0, -0.785_398_2, 0.0, 0.0),
    );
    root.child(
        "right_front_fin",
        CubeList::new()
            .tex_offs(5, 17)
            .add_box(-1.0, -8.0, 0.0, 1.0, 8.0, 0.0),
        PartPose::offset_rotation(-4.0, 22.0, -4.0, 0.0, -0.785_398_2, 0.0),
    );
    root.child(
        "left_front_fin",
        CubeList::new()
            .tex_offs(1, 17)
            .add_box(0.0, -8.0, 0.0, 1.0, 8.0, 0.0),
        PartPose::offset_rotation(4.0, 22.0, -4.0, 0.0, 0.785_398_2, 0.0),
    );
    root.child(
        "bottom_front_fin",
        CubeList::new()
            .tex_offs(15, 20)
            .add_box(-4.0, 0.0, 0.0, 8.0, 1.0, 0.0),
        PartPose::offset_rotation(0.0, 22.0, -4.0, -0.785_398_2, 0.0, 0.0),
    );
    root.child(
        "bottom_middle_fin",
        CubeList::new()
            .tex_offs(15, 20)
            .add_box(-4.0, 0.0, 0.0, 8.0, 1.0, 0.0),
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.child(
        "bottom_back_fin",
        CubeList::new()
            .tex_offs(15, 20)
            .add_box(-4.0, 0.0, 0.0, 8.0, 1.0, 0.0),
        PartPose::offset_rotation(0.0, 22.0, 4.0, 0.785_398_2, 0.0, 0.0),
    );
    root.child(
        "right_back_fin",
        CubeList::new()
            .tex_offs(9, 17)
            .add_box(-1.0, -8.0, 0.0, 1.0, 8.0, 0.0),
        PartPose::offset_rotation(-4.0, 22.0, 4.0, 0.0, 0.785_398_2, 0.0),
    );
    root.child(
        "left_back_fin",
        CubeList::new()
            .tex_offs(9, 17)
            .add_box(0.0, -8.0, 0.0, 1.0, 8.0, 0.0),
        PartPose::offset_rotation(4.0, 22.0, 4.0, 0.0, -0.785_398_2, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn pufferfish_blue_fin_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let sway = 0.4 * (st.age_ticks * 0.2).sin();
    parts[model.id("right_blue_fin")].z_rot = -0.2 + sway;
    parts[model.id("left_blue_fin")].z_rot = 0.2 - sway;
}
