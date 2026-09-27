use std::f32::consts::{FRAC_PI_2, PI};

use crate::direction::{Direction, Faces};
use crate::entities::geom::{CubeList, LayerDef, MeshDef, PartPose};

pub fn chest_single() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "bottom",
            CubeList::new()
                .tex_offs(0, 19)
                .add_box(1.0, 0.0, 1.0, 14.0, 10.0, 14.0),
            PartPose::ZERO,
        );
        root.child(
            "lid",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(1.0, 0.0, 0.0, 14.0, 5.0, 14.0),
            PartPose::offset(0.0, 9.0, 1.0),
        );
        root.child(
            "lock",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(7.0, -2.0, 14.0, 2.0, 4.0, 1.0),
            PartPose::offset(0.0, 9.0, 1.0),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn chest_left() -> LayerDef {
    let visible = Faces::of(&[
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::East,
    ]);
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "bottom",
            CubeList::new()
                .tex_offs(0, 19)
                .add_box_faces(0.0, 0.0, 1.0, 15.0, 10.0, 14.0, visible),
            PartPose::ZERO,
        );
        root.child(
            "lid",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_faces(0.0, 0.0, 0.0, 15.0, 5.0, 14.0, visible),
            PartPose::offset(0.0, 9.0, 1.0),
        );
        root.child(
            "lock",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_faces(0.0, -2.0, 14.0, 1.0, 4.0, 1.0, visible),
            PartPose::offset(0.0, 9.0, 1.0),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn chest_right() -> LayerDef {
    let visible = Faces::of(&[
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
    ]);
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "bottom",
            CubeList::new()
                .tex_offs(0, 19)
                .add_box_faces(1.0, 0.0, 1.0, 15.0, 10.0, 14.0, visible),
            PartPose::ZERO,
        );
        root.child(
            "lid",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_faces(1.0, 0.0, 0.0, 15.0, 5.0, 14.0, visible),
            PartPose::offset(0.0, 9.0, 1.0),
        );
        root.child(
            "lock",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_faces(15.0, -2.0, 14.0, 1.0, 4.0, 1.0, visible),
            PartPose::offset(0.0, 9.0, 1.0),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn conduit_shell() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root().child(
        "shell",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-3.0, -3.0, -3.0, 6.0, 6.0, 6.0),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 32, 16)
}

pub fn standing_sign() -> LayerDef {
    sign_layer(true)
}

pub fn wall_sign() -> LayerDef {
    sign_layer(false)
}

fn sign_layer(standing: bool) -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "sign",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-12.0, -14.0, -1.0, 24.0, 12.0, 2.0),
            PartPose::ZERO,
        );
        if standing {
            root.child(
                "stick",
                CubeList::new()
                    .tex_offs(0, 14)
                    .add_box(-1.0, -2.0, -1.0, 2.0, 14.0, 2.0),
                PartPose::ZERO,
            );
        }
    }
    LayerDef::create(mesh, 64, 32)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HangingAttachment {
    Ceiling,
    CeilingMiddle,
    Wall,
}

pub fn hanging_sign(attachment: HangingAttachment) -> LayerDef {
    const CHAIN_ROT: f32 = 0.785_398_2;

    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "board",
            CubeList::new()
                .tex_offs(0, 12)
                .add_box(-7.0, 0.0, -1.0, 14.0, 10.0, 2.0),
            PartPose::ZERO,
        );
        if attachment == HangingAttachment::Wall {
            root.child(
                "plank",
                CubeList::new()
                    .tex_offs(0, 0)
                    .add_box(-8.0, -6.0, -2.0, 16.0, 2.0, 4.0),
                PartPose::ZERO,
            );
        }
        if matches!(
            attachment,
            HangingAttachment::Wall | HangingAttachment::Ceiling
        ) {
            let chains = root.child("normalChains", CubeList::new(), PartPose::ZERO);
            chains.child(
                "chainL1",
                CubeList::new()
                    .tex_offs(0, 6)
                    .add_box(-1.5, 0.0, 0.0, 3.0, 6.0, 0.0),
                PartPose::offset_rotation(-5.0, -6.0, 0.0, 0.0, -CHAIN_ROT, 0.0),
            );
            chains.child(
                "chainL2",
                CubeList::new()
                    .tex_offs(6, 6)
                    .add_box(-1.5, 0.0, 0.0, 3.0, 6.0, 0.0),
                PartPose::offset_rotation(-5.0, -6.0, 0.0, 0.0, CHAIN_ROT, 0.0),
            );
            chains.child(
                "chainR1",
                CubeList::new()
                    .tex_offs(0, 6)
                    .add_box(-1.5, 0.0, 0.0, 3.0, 6.0, 0.0),
                PartPose::offset_rotation(5.0, -6.0, 0.0, 0.0, -CHAIN_ROT, 0.0),
            );
            chains.child(
                "chainR2",
                CubeList::new()
                    .tex_offs(6, 6)
                    .add_box(-1.5, 0.0, 0.0, 3.0, 6.0, 0.0),
                PartPose::offset_rotation(5.0, -6.0, 0.0, 0.0, CHAIN_ROT, 0.0),
            );
        }
        if attachment == HangingAttachment::CeilingMiddle {
            root.child(
                "vChains",
                CubeList::new()
                    .tex_offs(14, 6)
                    .add_box(-6.0, -6.0, 0.0, 12.0, 6.0, 0.0),
                PartPose::ZERO,
            );
        }
    }
    LayerDef::create(mesh, 64, 32)
}

pub fn banner_body_standing() -> LayerDef {
    banner_body(true)
}

pub fn banner_body_wall() -> LayerDef {
    banner_body(false)
}

fn banner_body(standing: bool) -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        if standing {
            root.child(
                "pole",
                CubeList::new()
                    .tex_offs(44, 0)
                    .add_box(-1.0, -42.0, -1.0, 2.0, 42.0, 2.0),
                PartPose::ZERO,
            );
        }
        root.child(
            "bar",
            CubeList::new().tex_offs(0, 42).add_box(
                -10.0,
                if standing { -44.0 } else { -20.5 },
                if standing { -1.0 } else { 9.5 },
                20.0,
                2.0,
                2.0,
            ),
            PartPose::ZERO,
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn banner_flag_standing() -> LayerDef {
    banner_flag(true)
}

pub fn banner_flag_wall() -> LayerDef {
    banner_flag(false)
}

fn banner_flag(standing: bool) -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        mesh.root().child(
            "flag",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-10.0, 0.0, -2.0, 20.0, 40.0, 1.0),
            PartPose::offset(
                0.0,
                if standing { -44.0 } else { -20.5 },
                if standing { 0.0 } else { 10.5 },
            ),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn bed_head() -> LayerDef {
    let visible_body = Faces::of(&[
        Direction::Down,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ]);
    let visible_legs = Faces::of(&[
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ]);
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "main",
            CubeList::new().tex_offs(0, 0).add_box_faces(
                0.0,
                0.0,
                0.0,
                16.0,
                16.0,
                6.0,
                visible_body,
            ),
            PartPose::ZERO,
        );
        root.child(
            "left_leg",
            CubeList::new().tex_offs(50, 6).add_box_faces(
                0.0,
                6.0,
                0.0,
                3.0,
                3.0,
                3.0,
                visible_legs,
            ),
            PartPose::rotation(FRAC_PI_2, 0.0, FRAC_PI_2),
        );
        root.child(
            "right_leg",
            CubeList::new().tex_offs(50, 18).add_box_faces(
                -16.0,
                6.0,
                0.0,
                3.0,
                3.0,
                3.0,
                visible_legs,
            ),
            PartPose::rotation(FRAC_PI_2, 0.0, PI),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn bed_foot() -> LayerDef {
    let visible_body = Faces::of(&[
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ]);
    let visible_legs = Faces::of(&[
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ]);
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "main",
            CubeList::new().tex_offs(0, 22).add_box_faces(
                0.0,
                0.0,
                0.0,
                16.0,
                16.0,
                6.0,
                visible_body,
            ),
            PartPose::ZERO,
        );
        root.child(
            "left_leg",
            CubeList::new().tex_offs(50, 0).add_box_faces(
                0.0,
                6.0,
                -16.0,
                3.0,
                3.0,
                3.0,
                visible_legs,
            ),
            PartPose::rotation(FRAC_PI_2, 0.0, 0.0),
        );
        root.child(
            "right_leg",
            CubeList::new().tex_offs(50, 12).add_box_faces(
                -16.0,
                6.0,
                -16.0,
                3.0,
                3.0,
                3.0,
                visible_legs,
            ),
            PartPose::rotation(FRAC_PI_2, 0.0, 1.5 * PI),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn skull_mob_head() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "head",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0),
            PartPose::ZERO,
        );
    }
    LayerDef::create(mesh, 64, 32)
}

pub fn skull_humanoid_head() -> LayerDef {
    use crate::entities::geom::Grow;

    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        let head = root.child(
            "head",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0),
            PartPose::ZERO,
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
                Grow(0.25, 0.25, 0.25),
            ),
            PartPose::ZERO,
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn piglin_head() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        let head = root.child(
            "head",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-5.0, -8.0, -4.0, 10.0, 8.0, 8.0)
                .tex_offs(31, 1)
                .add_box(-2.0, -4.0, -5.0, 4.0, 4.0, 1.0)
                .tex_offs(2, 4)
                .add_box(2.0, -2.0, -5.0, 1.0, 2.0, 1.0)
                .tex_offs(2, 0)
                .add_box(-3.0, -2.0, -5.0, 1.0, 2.0, 1.0),
            PartPose::ZERO,
        );
        head.child(
            "left_ear",
            CubeList::new()
                .tex_offs(51, 6)
                .add_box(0.0, 0.0, -2.0, 1.0, 5.0, 4.0),
            PartPose::offset_rotation(4.5, -6.0, 0.0, 0.0, 0.0, -0.523_598_8),
        );
        head.child(
            "right_ear",
            CubeList::new()
                .tex_offs(39, 6)
                .add_box(-1.0, 0.0, -2.0, 1.0, 5.0, 4.0),
            PartPose::offset_rotation(-4.5, -6.0, 0.0, 0.0, 0.0, 0.523_598_8),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn dragon_head() -> LayerDef {
    use crate::entities::geom::Grow;

    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        let head = root.child(
            "head",
            CubeList::new()
                .add_box_at(-6.0, -1.0, -24.0, 12.0, 5.0, 16.0, Grow::NONE, 176, 44)
                .add_box_at(-8.0, -8.0, -10.0, 16.0, 16.0, 16.0, Grow::NONE, 112, 30)
                .mirror()
                .add_box_at(-5.0, -12.0, -4.0, 2.0, 4.0, 6.0, Grow::NONE, 0, 0)
                .add_box_at(-5.0, -3.0, -22.0, 2.0, 2.0, 4.0, Grow::NONE, 112, 0)
                .mirror_if(false)
                .add_box_at(3.0, -12.0, -4.0, 2.0, 4.0, 6.0, Grow::NONE, 0, 0)
                .add_box_at(3.0, -3.0, -22.0, 2.0, 2.0, 4.0, Grow::NONE, 112, 0),
            PartPose::offset(0.0, -7.986_666, 0.0).scaled(0.75),
        );
        head.child(
            "jaw",
            CubeList::new()
                .tex_offs(176, 65)
                .add_box(-6.0, 0.0, -16.0, 12.0, 4.0, 16.0),
            PartPose::offset(0.0, 4.0, -8.0),
        );
    }
    LayerDef::create(mesh, 256, 256)
}

pub fn book() -> LayerDef {
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "left_lid",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-6.0, -5.0, -0.005, 6.0, 10.0, 0.005),
            PartPose::offset(0.0, 0.0, -1.0),
        );
        root.child(
            "right_lid",
            CubeList::new()
                .tex_offs(16, 0)
                .add_box(0.0, -5.0, -0.005, 6.0, 10.0, 0.005),
            PartPose::offset(0.0, 0.0, 1.0),
        );
        root.child(
            "seam",
            CubeList::new()
                .tex_offs(12, 0)
                .add_box(-1.0, -5.0, 0.0, 2.0, 10.0, 0.005),
            PartPose::rotation(0.0, FRAC_PI_2, 0.0),
        );
        root.child(
            "left_pages",
            CubeList::new()
                .tex_offs(0, 10)
                .add_box(0.0, -4.0, -0.99, 5.0, 8.0, 1.0),
            PartPose::ZERO,
        );
        root.child(
            "right_pages",
            CubeList::new()
                .tex_offs(12, 10)
                .add_box(0.0, -4.0, -0.01, 5.0, 8.0, 1.0),
            PartPose::ZERO,
        );
        for page in ["flip_page1", "flip_page2"] {
            root.child(
                page,
                CubeList::new()
                    .tex_offs(24, 10)
                    .add_box(0.0, -4.0, 0.0, 5.0, 8.0, 0.005),
                PartPose::ZERO,
            );
        }
    }
    LayerDef::create(mesh, 64, 32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::geom::bake;

    #[test]
    fn the_chest_hinges_at_the_back_of_the_lid() {
        let baked = bake(&chest_single());
        assert_eq!(baked.parts.len(), 4);
        let lid = &baked.parts[baked.id("lid")];
        let lock = &baked.parts[baked.id("lock")];
        assert_eq!(lid.initial.y, 9.0);
        assert_eq!(lid.initial.z, 1.0);
        assert_eq!(lock.initial.y, lid.initial.y);
        assert_eq!(lock.initial.z, lid.initial.z);
    }

    #[test]
    fn a_double_chest_half_drops_its_inner_face() {
        for layer in [chest_left(), chest_right()] {
            let baked = bake(&layer);
            let bottom = baked.parts[baked.id("bottom")].mesh.as_ref().unwrap();
            assert_eq!(bottom.count_vertices(), 5 * 4);
        }
    }

    #[test]
    fn only_the_standing_sign_has_a_post() {
        assert!(bake(&standing_sign()).find("stick").is_some());
        assert!(bake(&wall_sign()).find("stick").is_none());
    }

    #[test]
    fn the_hanging_sign_layers_carry_their_own_hardware() {
        let wall = bake(&hanging_sign(HangingAttachment::Wall));
        assert!(wall.find("plank").is_some());
        assert!(wall.find("chainL1").is_some());
        assert!(wall.find("vChains").is_none());

        let ceiling = bake(&hanging_sign(HangingAttachment::Ceiling));
        assert!(ceiling.find("plank").is_none());
        assert!(ceiling.find("chainR2").is_some());
        assert!(ceiling.find("vChains").is_none());

        let middle = bake(&hanging_sign(HangingAttachment::CeilingMiddle));
        assert!(middle.find("plank").is_none());
        assert!(middle.find("normalChains").is_none());
        assert!(middle.find("vChains").is_some());
    }

    #[test]
    fn the_humanoid_head_adds_a_hat_the_mob_head_lacks() {
        assert!(bake(&skull_mob_head()).find("hat").is_none());
        assert!(bake(&skull_humanoid_head()).find("hat").is_some());
    }

    #[test]
    fn the_piglin_head_carries_both_ears() {
        let baked = bake(&piglin_head());
        let left = &baked.parts[baked.id("left_ear")];
        let right = &baked.parts[baked.id("right_ear")];
        assert_eq!(left.initial.x, 4.5);
        assert_eq!(right.initial.x, -4.5);
        assert_eq!(left.initial.z_rot, -0.523_598_8);
        assert_eq!(right.initial.z_rot, 0.523_598_8);
    }

    #[test]
    fn the_dragon_jaw_hangs_off_the_head() {
        let baked = bake(&dragon_head());
        let head_id = baked.id("head");
        let jaw_id = baked.id("jaw");
        assert_eq!(baked.parts[jaw_id].parent, Some(head_id));
        assert_eq!(baked.parts[jaw_id].initial.y, 4.0);
        assert_eq!(baked.parts[jaw_id].initial.z, -8.0);
        assert_eq!(baked.parts[head_id].initial.y, -7.986_666 * 0.75);
    }
}
