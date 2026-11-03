#![allow(clippy::approx_constant)]

use crate::entities::geom::{BakedModel, Grow, LayerDef, MeshDef, PartPose, PartState};
use crate::entities::models::aquatic::scaling;
use crate::entities::state::EntityState;

pub fn cod_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.part(
        "body",
        (0, 0),
        [-1.0, -2.0, 0.0, 2.0, 4.0, 7.0],
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.part(
        "head",
        (11, 0),
        [-1.0, -2.0, -3.0, 2.0, 4.0, 3.0],
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.part(
        "nose",
        (0, 0),
        [-1.0, -2.0, -1.0, 2.0, 3.0, 1.0],
        PartPose::offset(0.0, 22.0, -3.0),
    );
    root.part(
        "right_fin",
        (22, 1),
        [-2.0, 0.0, -1.0, 2.0, 0.0, 2.0],
        PartPose::offset_rotation(-1.0, 23.0, 0.0, 0.0, 0.0, -0.785_398_2),
    );
    root.part(
        "left_fin",
        (22, 4),
        [0.0, 0.0, -1.0, 2.0, 0.0, 2.0],
        PartPose::offset_rotation(1.0, 23.0, 0.0, 0.0, 0.0, 0.785_398_2),
    );
    root.part(
        "tail_fin",
        (22, 3),
        [0.0, -2.0, 0.0, 0.0, 4.0, 4.0],
        PartPose::offset(0.0, 22.0, 7.0),
    );
    root.part(
        "top_fin",
        (20, -6),
        [0.0, -1.0, -1.0, 0.0, 1.0, 6.0],
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
    root.part(
        "body_front",
        (0, 0),
        [-1.5, -2.5, 0.0, 3.0, 5.0, 8.0],
        PartPose::offset(0.0, 20.0, -7.2),
    );
    root.part(
        "body_back",
        (0, 13),
        [-1.5, -2.5, 0.0, 3.0, 5.0, 8.0],
        PartPose::offset(0.0, 20.0, 0.800_000_2),
    );
    root.part(
        "head",
        (22, 0),
        [-1.0, -2.0, -3.0, 2.0, 4.0, 3.0],
        PartPose::offset(0.0, 20.0, -7.2),
    );
    root.get("body_back").part(
        "back_fin",
        (20, 10),
        [0.0, -2.5, 0.0, 0.0, 5.0, 6.0],
        PartPose::offset(0.0, 0.0, 8.0),
    );
    root.get("body_front").part(
        "top_front_fin",
        (2, 1),
        [0.0, 0.0, 0.0, 0.0, 2.0, 3.0],
        PartPose::offset(0.0, -4.5, 5.0),
    );
    root.get("body_back").part(
        "top_back_fin",
        (0, 2),
        [0.0, 0.0, 0.0, 0.0, 2.0, 4.0],
        PartPose::offset(0.0, -4.5, -1.0),
    );
    root.part(
        "right_fin",
        (-4, 0),
        [-2.0, 0.0, 0.0, 2.0, 0.0, 2.0],
        PartPose::offset_rotation(-1.5, 21.5, -7.2, 0.0, 0.0, -0.785_398_2),
    );
    root.part(
        "left_fin",
        (0, 0),
        [0.0, 0.0, 0.0, 2.0, 0.0, 2.0],
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
    root.part_grow(
        "body",
        (0, 0),
        [-1.0, -1.5, -3.0, 2.0, 3.0, 6.0],
        g,
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.part_grow(
        "tail",
        (22, -6),
        [0.0, -1.5, 0.0, 0.0, 3.0, 6.0],
        g,
        PartPose::offset(0.0, 22.0, 3.0),
    );
    root.part_grow(
        "right_fin",
        (2, 16),
        [-2.0, -1.0, 0.0, 2.0, 2.0, 0.0],
        g,
        PartPose::offset_rotation(-1.0, 22.5, 0.0, 0.0, 0.785_398_2, 0.0),
    );
    root.part_grow(
        "left_fin",
        (2, 12),
        [0.0, -1.0, 0.0, 2.0, 2.0, 0.0],
        g,
        PartPose::offset_rotation(1.0, 22.5, 0.0, 0.0, -0.785_398_2, 0.0),
    );
    root.part_grow(
        "top_fin",
        (10, -5),
        [0.0, -3.0, 0.0, 0.0, 3.0, 6.0],
        g,
        PartPose::offset(0.0, 20.5, -3.0),
    );
    mesh
}

fn tropical_large_mesh(g: Grow) -> MeshDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.part_grow(
        "body",
        (0, 20),
        [-1.0, -3.0, -3.0, 2.0, 6.0, 6.0],
        g,
        PartPose::offset(0.0, 19.0, 0.0),
    );
    root.part_grow(
        "tail",
        (21, 16),
        [0.0, -3.0, 0.0, 0.0, 6.0, 5.0],
        g,
        PartPose::offset(0.0, 19.0, 3.0),
    );
    root.part_grow(
        "right_fin",
        (2, 16),
        [-2.0, 0.0, 0.0, 2.0, 2.0, 0.0],
        g,
        PartPose::offset_rotation(-1.0, 20.0, 0.0, 0.0, 0.785_398_2, 0.0),
    );
    root.part_grow(
        "left_fin",
        (2, 12),
        [0.0, 0.0, 0.0, 2.0, 2.0, 0.0],
        g,
        PartPose::offset_rotation(1.0, 20.0, 0.0, 0.0, -0.785_398_2, 0.0),
    );
    root.part_grow(
        "top_fin",
        (20, 11),
        [0.0, -4.0, 0.0, 0.0, 4.0, 6.0],
        g,
        PartPose::offset(0.0, 16.0, -3.0),
    );
    root.part_grow(
        "bottom_fin",
        (20, 21),
        [0.0, 0.0, 0.0, 0.0, 4.0, 6.0],
        g,
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
    root.part(
        "body",
        (0, 27),
        [-1.5, -2.0, -1.5, 3.0, 2.0, 3.0],
        PartPose::offset(0.0, 23.0, 0.0),
    );
    root.part(
        "right_eye",
        (24, 6),
        [-1.5, 0.0, -1.5, 1.0, 1.0, 1.0],
        PartPose::offset(0.0, 20.0, 0.0),
    );
    root.part(
        "left_eye",
        (28, 6),
        [0.5, 0.0, -1.5, 1.0, 1.0, 1.0],
        PartPose::offset(0.0, 20.0, 0.0),
    );
    root.part(
        "back_fin",
        (-3, 0),
        [-1.5, 0.0, 0.0, 3.0, 0.0, 3.0],
        PartPose::offset(0.0, 22.0, 1.5),
    );
    root.part(
        "right_fin",
        (25, 0),
        [-1.0, 0.0, 0.0, 1.0, 0.0, 2.0],
        PartPose::offset(-1.5, 22.0, -1.5),
    );
    root.part(
        "left_fin",
        (25, 0),
        [0.0, 0.0, 0.0, 1.0, 0.0, 2.0],
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
    root.part(
        "body",
        (12, 22),
        [-2.5, -5.0, -2.5, 5.0, 5.0, 5.0],
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.part(
        "right_blue_fin",
        (24, 0),
        [-2.0, 0.0, 0.0, 2.0, 0.0, 2.0],
        PartPose::offset(-2.5, 18.0, -1.5),
    );
    root.part(
        "left_blue_fin",
        (24, 3),
        [0.0, 0.0, 0.0, 2.0, 0.0, 2.0],
        PartPose::offset(2.5, 18.0, -1.5),
    );
    root.part(
        "top_front_fin",
        (19, 17),
        [-2.5, -1.0, 0.0, 5.0, 1.0, 0.0],
        PartPose::offset_rotation(0.0, 17.0, -2.5, 0.785_398_2, 0.0, 0.0),
    );
    root.part(
        "top_back_fin",
        (11, 17),
        [-2.5, -1.0, 0.0, 5.0, 1.0, 0.0],
        PartPose::offset_rotation(0.0, 17.0, 2.5, -0.785_398_2, 0.0, 0.0),
    );
    root.part(
        "right_front_fin",
        (5, 17),
        [-1.0, -5.0, 0.0, 1.0, 5.0, 0.0],
        PartPose::offset_rotation(-2.5, 22.0, -2.5, 0.0, -0.785_398_2, 0.0),
    );
    root.part(
        "right_back_fin",
        (9, 17),
        [-1.0, -5.0, 0.0, 1.0, 5.0, 0.0],
        PartPose::offset_rotation(-2.5, 22.0, 2.5, 0.0, 0.785_398_2, 0.0),
    );
    root.part(
        "left_back_fin",
        (1, 17),
        [0.0, -5.0, 0.0, 1.0, 5.0, 0.0],
        PartPose::offset_rotation(2.5, 22.0, 2.5, 0.0, -0.785_398_2, 0.0),
    );
    root.part(
        "left_front_fin",
        (1, 17),
        [0.0, -5.0, 0.0, 1.0, 5.0, 0.0],
        PartPose::offset_rotation(2.5, 22.0, -2.5, 0.0, 0.785_398_2, 0.0),
    );
    root.part(
        "bottom_back_fin",
        (18, 20),
        [0.0, 0.0, 0.0, 5.0, 1.0, 0.0],
        PartPose::offset_rotation(-2.5, 22.0, 2.5, 0.785_398_2, 0.0, 0.0),
    );
    root.part(
        "bottom_front_fin",
        (17, 19),
        [-2.5, 0.0, 0.0, 5.0, 1.0, 1.0],
        PartPose::offset_rotation(0.0, 22.0, -2.5, -0.785_398_2, 0.0, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn pufferfish_big_layer() -> LayerDef {
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.part(
        "body",
        (0, 0),
        [-4.0, -8.0, -4.0, 8.0, 8.0, 8.0],
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.part(
        "right_blue_fin",
        (24, 0),
        [-2.0, 0.0, -1.0, 2.0, 1.0, 2.0],
        PartPose::offset(-4.0, 15.0, -2.0),
    );
    root.part(
        "left_blue_fin",
        (24, 3),
        [0.0, 0.0, -1.0, 2.0, 1.0, 2.0],
        PartPose::offset(4.0, 15.0, -2.0),
    );
    root.part(
        "top_front_fin",
        (15, 17),
        [-4.0, -1.0, 0.0, 8.0, 1.0, 0.0],
        PartPose::offset_rotation(0.0, 14.0, -4.0, 0.785_398_2, 0.0, 0.0),
    );
    root.part(
        "top_middle_fin",
        (14, 16),
        [-4.0, -1.0, 0.0, 8.0, 1.0, 1.0],
        PartPose::offset(0.0, 14.0, 0.0),
    );
    root.part(
        "top_back_fin",
        (23, 18),
        [-4.0, -1.0, 0.0, 8.0, 1.0, 0.0],
        PartPose::offset_rotation(0.0, 14.0, 4.0, -0.785_398_2, 0.0, 0.0),
    );
    root.part(
        "right_front_fin",
        (5, 17),
        [-1.0, -8.0, 0.0, 1.0, 8.0, 0.0],
        PartPose::offset_rotation(-4.0, 22.0, -4.0, 0.0, -0.785_398_2, 0.0),
    );
    root.part(
        "left_front_fin",
        (1, 17),
        [0.0, -8.0, 0.0, 1.0, 8.0, 0.0],
        PartPose::offset_rotation(4.0, 22.0, -4.0, 0.0, 0.785_398_2, 0.0),
    );
    root.part(
        "bottom_front_fin",
        (15, 20),
        [-4.0, 0.0, 0.0, 8.0, 1.0, 0.0],
        PartPose::offset_rotation(0.0, 22.0, -4.0, -0.785_398_2, 0.0, 0.0),
    );
    root.part(
        "bottom_middle_fin",
        (15, 20),
        [-4.0, 0.0, 0.0, 8.0, 1.0, 0.0],
        PartPose::offset(0.0, 22.0, 0.0),
    );
    root.part(
        "bottom_back_fin",
        (15, 20),
        [-4.0, 0.0, 0.0, 8.0, 1.0, 0.0],
        PartPose::offset_rotation(0.0, 22.0, 4.0, 0.785_398_2, 0.0, 0.0),
    );
    root.part(
        "right_back_fin",
        (9, 17),
        [-1.0, -8.0, 0.0, 1.0, 8.0, 0.0],
        PartPose::offset_rotation(-4.0, 22.0, 4.0, 0.0, 0.785_398_2, 0.0),
    );
    root.part(
        "left_back_fin",
        (9, 17),
        [0.0, -8.0, 0.0, 1.0, 8.0, 0.0],
        PartPose::offset_rotation(4.0, 22.0, 4.0, 0.0, -0.785_398_2, 0.0),
    );
    LayerDef::create(mesh, 32, 32)
}

pub fn pufferfish_blue_fin_setup(model: &BakedModel, parts: &mut [PartState], st: &EntityState) {
    let sway = 0.4 * (st.age_ticks * 0.2).sin();
    parts[model.id("right_blue_fin")].z_rot = -0.2 + sway;
    parts[model.id("left_blue_fin")].z_rot = 0.2 - sway;
}
