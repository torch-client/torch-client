use crate::entities::geom::{CubeList, Grow, LayerDef, MeshDef, PartPose};

use super::humanoid;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Head,
    Chest,
    Legs,
    Feet,
}

impl Slot {
    fn cleared(self) -> &'static [&'static str] {
        match self {
            Slot::Head => &["body", "right_arm", "left_arm", "right_leg", "left_leg"],
            Slot::Chest => &["head", "head/hat", "right_leg", "left_leg"],
            Slot::Legs => &["head", "head/hat", "right_arm", "left_arm"],
            Slot::Feet => &["head", "head/hat", "body", "right_arm", "left_arm"],
        }
    }

    fn deformation(self, inner: Grow, outer: Grow) -> Grow {
        match self {
            Slot::Legs => inner,
            _ => outer,
        }
    }
}

pub const INNER_ARMOR_DEFORMATION: Grow = Grow::all(0.5);

pub const OUTER_ARMOR_DEFORMATION: Grow = Grow::all(1.0);

pub const PIGLIN_OUTER_ARMOR_DEFORMATION: Grow = Grow::all(1.02);

fn base_armor_mesh(g: Grow) -> MeshDef {
    let mut mesh = humanoid::create_mesh(g, 0.0);
    let leg = g.extend(humanoid::LEGGINGS_OVERLAY_SCALE);
    let root = mesh.root();
    root.child(
        "right_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .add_box_grow(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, leg),
        PartPose::offset(-1.9, 12.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new()
            .tex_offs(0, 16)
            .mirror()
            .add_box_grow(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, leg),
        PartPose::offset(1.9, 12.0, 0.0),
    );
    mesh
}

pub fn humanoid_armor_layer(slot: Slot, inner: Grow, outer: Grow) -> LayerDef {
    let mut mesh = base_armor_mesh(slot.deformation(inner, outer));
    humanoid::clear_cubes(mesh.root(), slot.cleared());
    LayerDef::create(mesh, 64, 32)
}

pub fn zombie_villager_armor_layer(slot: Slot, inner: Grow, outer: Grow) -> LayerDef {
    let g = slot.deformation(inner, outer);
    let mut mesh = humanoid::create_mesh(g, 0.0);
    {
        let root = mesh.root();
        root.child(
            "head",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_grow(-4.0, -10.0, -4.0, 8.0, 8.0, 8.0, g),
            PartPose::ZERO,
        );
        root.child(
            "body",
            CubeList::new().tex_offs(16, 16).add_box_grow(
                -4.0,
                0.0,
                -2.0,
                8.0,
                12.0,
                4.0,
                g.extend(0.1),
            ),
            PartPose::ZERO,
        );
        root.child(
            "right_leg",
            CubeList::new().tex_offs(0, 16).add_box_grow(
                -2.0,
                0.0,
                -2.0,
                4.0,
                12.0,
                4.0,
                g.extend(0.1),
            ),
            PartPose::offset(-2.0, 12.0, 0.0),
        );
        root.child(
            "left_leg",
            CubeList::new().tex_offs(0, 16).mirror().add_box_grow(
                -2.0,
                0.0,
                -2.0,
                4.0,
                12.0,
                4.0,
                g.extend(0.1),
            ),
            PartPose::offset(2.0, 12.0, 0.0),
        );
        root.get("head/hat")
            .child("hat_rim", CubeList::new(), PartPose::ZERO);
    }
    humanoid::clear_cubes(mesh.root(), slot.cleared());
    LayerDef::create(mesh, 64, 32)
}

pub fn armor_stand_armor_layer(slot: Slot, inner: Grow, outer: Grow) -> LayerDef {
    let g = slot.deformation(inner, outer);
    let mut mesh = humanoid::create_mesh(g, 0.0);
    {
        let root = mesh.root();
        let head = root.child(
            "head",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_grow(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0, g),
            PartPose::offset(0.0, 1.0, 0.0),
        );
        head.child(
            "hat",
            CubeList::new().tex_offs(32, 0).add_box_grow(
                -4.0,
                -8.0,
                -4.0,
                8.0,
                8.0,
                8.0,
                g.extend(humanoid::HAT_OVERLAY_SCALE),
            ),
            PartPose::ZERO,
        );
        let leg = g.extend(humanoid::LEGGINGS_OVERLAY_SCALE);
        root.child(
            "right_leg",
            CubeList::new()
                .tex_offs(0, 16)
                .add_box_grow(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, leg),
            PartPose::offset(-1.9, 11.0, 0.0),
        );
        root.child(
            "left_leg",
            CubeList::new()
                .tex_offs(0, 16)
                .mirror()
                .add_box_grow(-2.0, 0.0, -2.0, 4.0, 12.0, 4.0, leg),
            PartPose::offset(1.9, 11.0, 0.0),
        );
    }
    humanoid::clear_cubes(mesh.root(), slot.cleared());
    LayerDef::create(mesh, 64, 32)
}

fn scaled(layer: LayerDef, factor: f32) -> LayerDef {
    LayerDef {
        mesh: humanoid::scaling(layer.mesh, factor),
        ..layer
    }
}

fn baby(layer: LayerDef) -> LayerDef {
    let mesh = humanoid::baby_transform(
        layer.mesh,
        humanoid::BabyTransform::HUMANOID,
        &[
            "head",
            "body",
            "right_arm",
            "left_arm",
            "right_leg",
            "left_leg",
        ],
        &["head"],
    );
    LayerDef { mesh, ..layer }
}

fn humanoid_set(slot: Slot) -> LayerDef {
    humanoid_armor_layer(slot, INNER_ARMOR_DEFORMATION, OUTER_ARMOR_DEFORMATION)
}

pub fn humanoid_helmet() -> LayerDef {
    humanoid_set(Slot::Head)
}
pub fn humanoid_chestplate() -> LayerDef {
    humanoid_set(Slot::Chest)
}
pub fn humanoid_leggings() -> LayerDef {
    humanoid_set(Slot::Legs)
}
pub fn humanoid_boots() -> LayerDef {
    humanoid_set(Slot::Feet)
}

pub fn husk_helmet() -> LayerDef {
    scaled(humanoid_set(Slot::Head), 1.0625)
}
pub fn husk_chestplate() -> LayerDef {
    scaled(humanoid_set(Slot::Chest), 1.0625)
}
pub fn husk_leggings() -> LayerDef {
    scaled(humanoid_set(Slot::Legs), 1.0625)
}
pub fn husk_boots() -> LayerDef {
    scaled(humanoid_set(Slot::Feet), 1.0625)
}

pub fn giant_helmet() -> LayerDef {
    scaled(humanoid_set(Slot::Head), 6.0)
}
pub fn giant_chestplate() -> LayerDef {
    scaled(humanoid_set(Slot::Chest), 6.0)
}
pub fn giant_leggings() -> LayerDef {
    scaled(humanoid_set(Slot::Legs), 6.0)
}
pub fn giant_boots() -> LayerDef {
    scaled(humanoid_set(Slot::Feet), 6.0)
}

fn piglin_set(slot: Slot) -> LayerDef {
    humanoid_armor_layer(
        slot,
        INNER_ARMOR_DEFORMATION,
        PIGLIN_OUTER_ARMOR_DEFORMATION,
    )
}

pub fn piglin_helmet() -> LayerDef {
    piglin_set(Slot::Head)
}
pub fn piglin_chestplate() -> LayerDef {
    piglin_set(Slot::Chest)
}
pub fn piglin_leggings() -> LayerDef {
    piglin_set(Slot::Legs)
}
pub fn piglin_boots() -> LayerDef {
    piglin_set(Slot::Feet)
}

fn zombie_villager_set(slot: Slot) -> LayerDef {
    zombie_villager_armor_layer(slot, INNER_ARMOR_DEFORMATION, OUTER_ARMOR_DEFORMATION)
}

pub fn zombie_villager_helmet() -> LayerDef {
    zombie_villager_set(Slot::Head)
}
pub fn zombie_villager_chestplate() -> LayerDef {
    zombie_villager_set(Slot::Chest)
}
pub fn zombie_villager_leggings() -> LayerDef {
    zombie_villager_set(Slot::Legs)
}
pub fn zombie_villager_boots() -> LayerDef {
    zombie_villager_set(Slot::Feet)
}

fn armor_stand_set(slot: Slot) -> LayerDef {
    armor_stand_armor_layer(slot, INNER_ARMOR_DEFORMATION, OUTER_ARMOR_DEFORMATION)
}

pub fn armor_stand_helmet() -> LayerDef {
    armor_stand_set(Slot::Head)
}
pub fn armor_stand_chestplate() -> LayerDef {
    armor_stand_set(Slot::Chest)
}
pub fn armor_stand_leggings() -> LayerDef {
    armor_stand_set(Slot::Legs)
}
pub fn armor_stand_boots() -> LayerDef {
    armor_stand_set(Slot::Feet)
}

pub fn armor_stand_small_helmet() -> LayerDef {
    baby(armor_stand_set(Slot::Head))
}
pub fn armor_stand_small_chestplate() -> LayerDef {
    baby(armor_stand_set(Slot::Chest))
}
pub fn armor_stand_small_leggings() -> LayerDef {
    baby(armor_stand_set(Slot::Legs))
}
pub fn armor_stand_small_boots() -> LayerDef {
    baby(armor_stand_set(Slot::Feet))
}
