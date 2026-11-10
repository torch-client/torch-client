use std::f32::consts::{FRAC_PI_2, PI};

use crate::blocks::direction::{Direction, Faces};
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
    chest_double(Direction::East, 0.0, 0.0)
}

pub fn chest_right() -> LayerDef {
    chest_double(Direction::West, 1.0, 15.0)
}

fn chest_double(side: Direction, x: f32, lock_x: f32) -> LayerDef {
    let visible = Faces::of(&[
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        side,
    ]);
    let mut mesh = MeshDef::new();
    {
        let root = mesh.root();
        root.child(
            "bottom",
            CubeList::new()
                .tex_offs(0, 19)
                .add_box_faces(x, 0.0, 1.0, 15.0, 10.0, 14.0, visible),
            PartPose::ZERO,
        );
        root.child(
            "lid",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_faces(x, 0.0, 0.0, 15.0, 5.0, 14.0, visible),
            PartPose::offset(0.0, 9.0, 1.0),
        );
        root.child(
            "lock",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box_faces(lock_x, -2.0, 14.0, 1.0, 4.0, 1.0, visible),
            PartPose::offset(0.0, 9.0, 1.0),
        );
    }
    LayerDef::create(mesh, 64, 64)
}

pub fn conduit_shell() -> LayerDef {
    LayerDef::single_box("shell", (0, 0), [-3.0, -3.0, -3.0, 6.0, 6.0, 6.0], (32, 16))
}

pub fn conduit_cage() -> LayerDef {
    LayerDef::single_box("shell", (0, 0), [-4.0, -4.0, -4.0, 8.0, 8.0, 8.0], (32, 16))
}

pub fn conduit_wind() -> LayerDef {
    LayerDef::single_box(
        "wind",
        (0, 0),
        [-8.0, -8.0, -8.0, 16.0, 16.0, 16.0],
        (64, 32),
    )
}

pub fn conduit_eye() -> LayerDef {
    use crate::entities::geom::Grow;
    let mut mesh = MeshDef::new();
    mesh.root().child(
        "eye",
        CubeList::new().tex_offs(0, 0).add_box_grow(
            -4.0,
            -4.0,
            0.0,
            8.0,
            8.0,
            0.0,
            Grow::all(0.01),
        ),
        PartPose::ZERO,
    );
    LayerDef::create(mesh, 16, 16)
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

pub fn bell() -> LayerDef {
    let mut mesh = MeshDef::new();
    mesh.root()
        .child(
            "bell_body",
            CubeList::new()
                .tex_offs(0, 0)
                .add_box(-3.0, -6.0, -3.0, 6.0, 7.0, 6.0),
            PartPose::offset(8.0, 12.0, 8.0),
        )
        .child(
            "bell_base",
            CubeList::new()
                .tex_offs(0, 13)
                .add_box(4.0, 4.0, 4.0, 8.0, 2.0, 8.0),
            PartPose::offset(-8.0, -12.0, -8.0),
        );
    LayerDef::create(mesh, 32, 32)
}

pub fn decorated_pot_base() -> LayerDef {
    let plane = CubeList::new()
        .tex_offs(-14, 13)
        .add_box(0.0, 0.0, 0.0, 14.0, 0.0, 14.0);
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    root.child(
        "neck",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box_grow(
                4.0,
                17.0,
                4.0,
                8.0,
                3.0,
                8.0,
                crate::entities::geom::Grow::all(-0.1),
            )
            .tex_offs(0, 5)
            .add_box_grow(
                5.0,
                20.0,
                5.0,
                6.0,
                1.0,
                6.0,
                crate::entities::geom::Grow::all(0.2),
            ),
        PartPose::offset_rotation(0.0, 37.0, 16.0, PI, 0.0, 0.0),
    );
    root.child("top", plane.clone(), PartPose::offset(1.0, 16.0, 1.0));
    root.child("bottom", plane, PartPose::offset(1.0, 0.0, 1.0));
    LayerDef::create(mesh, 32, 32)
}

pub fn decorated_pot_side(side: usize) -> LayerDef {
    let (name, pose) = match side {
        0 => (
            "back",
            PartPose::offset_rotation(15.0, 16.0, 1.0, 0.0, 0.0, PI),
        ),
        1 => (
            "left",
            PartPose::offset_rotation(1.0, 16.0, 1.0, 0.0, -FRAC_PI_2, PI),
        ),
        2 => (
            "right",
            PartPose::offset_rotation(15.0, 16.0, 15.0, 0.0, FRAC_PI_2, PI),
        ),
        _ => (
            "front",
            PartPose::offset_rotation(1.0, 16.0, 15.0, PI, 0.0, 0.0),
        ),
    };
    let mut mesh = MeshDef::new();
    mesh.root().child(
        name,
        CubeList::new().tex_offs(1, 0).add_box_faces(
            0.0,
            0.0,
            0.0,
            14.0,
            16.0,
            0.0,
            Faces::of(&[Direction::North]),
        ),
        pose,
    );
    LayerDef::create(mesh, 16, 16)
}

#[allow(clippy::approx_constant)]
pub fn copper_golem_running() -> LayerDef {
    use crate::entities::geom::Grow;
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child("body", CubeList::new(), PartPose::offset(-1.064, -5.0, 0.0));
    body.child(
        "body_r1",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box(-4.02, -6.116, -3.5, 8.0, 6.0, 6.0),
        PartPose::offset_rotation(1.1, 0.1, 0.7, 0.1204, -0.0064, -0.0779),
    );
    body.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -5.1, -5.0, 8.0, 5.0, 10.0)
            .tex_offs(56, 0)
            .add_box(-1.02, -2.1, -6.0, 2.0, 3.0, 2.0)
            .tex_offs(37, 8)
            .add_box_grow(-1.02, -9.1, -1.0, 2.0, 4.0, 2.0, Grow::all(-0.015))
            .tex_offs(37, 0)
            .add_box_grow(-2.0, -13.1, -2.0, 4.0, 4.0, 4.0, Grow::all(-0.015)),
        PartPose::offset(0.7, -5.6, -1.8),
    );
    body.child(
        "right_arm",
        CubeList::new(),
        PartPose::offset(-4.0, -6.0, 0.0),
    )
    .child(
        "right_arm_r1",
        CubeList::new()
            .tex_offs(36, 16)
            .add_box(-3.052, -1.11, -2.036, 3.0, 10.0, 4.0),
        PartPose::offset_rotation(0.7, -0.248, -1.62, 1.0036, 0.0, 0.0),
    );
    body.child(
        "left_arm",
        CubeList::new(),
        PartPose::offset(4.0, -6.0, 0.0),
    )
    .child(
        "left_arm_r1",
        CubeList::new()
            .tex_offs(50, 16)
            .add_box(0.032, -1.1, -2.0, 3.0, 10.0, 4.0),
        PartPose::offset_rotation(0.732, 0.0, 0.0, -0.8715, -0.0535, -0.0449),
    );
    root.child(
        "right_leg",
        CubeList::new(),
        PartPose::offset(-3.064, -5.0, 0.0),
    )
    .child(
        "right_leg_r1",
        CubeList::new()
            .tex_offs(0, 27)
            .add_box(-1.856, -0.1, -1.09, 4.0, 5.0, 4.0),
        PartPose::offset_rotation(1.048, 0.0, -0.9, -0.8727, 0.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new(),
        PartPose::offset(0.936, -5.0, 0.0),
    )
    .child(
        "left_leg_r1",
        CubeList::new()
            .tex_offs(16, 27)
            .add_box(-2.088, -0.1, -2.0, 4.0, 5.0, 4.0),
        PartPose::offset_rotation(1.0, 0.0, 0.0, 0.7854, 0.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

#[allow(clippy::approx_constant)]
pub fn copper_golem_sitting() -> LayerDef {
    use crate::entities::geom::Grow;
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(3, 19)
            .add_box(-3.0, -4.0, -4.525, 6.0, 1.0, 6.0)
            .tex_offs(0, 15)
            .add_box(-4.0, -3.0, -3.525, 8.0, 6.0, 6.0),
        PartPose::offset(0.0, -3.0, 2.325),
    );
    body.child(
        "body_r1",
        CubeList::new()
            .tex_offs(3, 18)
            .add_box(-4.0, -3.0, -2.2, 8.0, 6.0, 3.0),
        PartPose::offset_rotation(0.0, -1.0, -4.325, 0.0, 0.0, -3.1416),
    );
    body.child(
        "head",
        CubeList::new()
            .tex_offs(37, 8)
            .add_box_grow(-1.0, -7.0, -3.3, 2.0, 4.0, 2.0, Grow::all(-0.015))
            .tex_offs(37, 0)
            .add_box_grow(-2.0, -11.0, -4.3, 4.0, 4.0, 4.0, Grow::all(-0.015))
            .tex_offs(0, 0)
            .add_box(-4.0, -3.0, -7.325, 8.0, 5.0, 10.0)
            .tex_offs(56, 0)
            .add_box(-1.0, 0.0, -8.325, 2.0, 3.0, 2.0),
        PartPose::offset(0.0, -6.0, -0.2),
    );
    body.child(
        "right_arm",
        CubeList::new(),
        PartPose::offset_rotation(-4.0, -5.6, -1.8, 0.4363, 0.0, 0.0),
    )
    .child(
        "right_arm_r1",
        CubeList::new()
            .tex_offs(36, 16)
            .add_box(-3.075, -0.9733, -1.9966, 3.0, 10.0, 4.0),
        PartPose::offset_rotation(0.0, 0.0893, 0.1198, -1.0472, 0.0, 0.0),
    );
    body.child(
        "left_arm",
        CubeList::new(),
        PartPose::offset_rotation(4.0, -5.6, -1.7, 0.4363, 0.0, 0.0),
    )
    .child(
        "left_arm_r1",
        CubeList::new()
            .tex_offs(50, 16)
            .add_box(0.075, -1.0443, -1.8997, 3.0, 10.0, 4.0),
        PartPose::offset_rotation(0.0, -0.0015, -0.0808, -1.0472, 0.0, 0.0),
    );
    root.child(
        "right_leg",
        CubeList::new(),
        PartPose::offset(-2.1, -2.1, -2.075),
    )
    .child(
        "right_leg_r1",
        CubeList::new()
            .tex_offs(0, 27)
            .add_box(-2.0, 0.975, 0.0, 4.0, 5.0, 4.0),
        PartPose::offset_rotation(0.05, -1.9, 1.075, -1.5708, 0.0, 0.0),
    );
    root.child(
        "left_leg",
        CubeList::new(),
        PartPose::offset(2.0, -2.0, -2.075),
    )
    .child(
        "left_leg_r1",
        CubeList::new()
            .tex_offs(16, 27)
            .add_box(-2.0, 0.975, 0.0, 4.0, 5.0, 4.0),
        PartPose::offset_rotation(0.05, -2.0, 1.075, -1.5708, 0.0, 0.0),
    );
    LayerDef::create(mesh, 64, 64)
}

#[allow(clippy::approx_constant)]
pub fn copper_golem_star() -> LayerDef {
    use crate::entities::geom::Grow;
    let mut mesh = MeshDef::new();
    let root = mesh.root();
    let body = root.child(
        "body",
        CubeList::new()
            .tex_offs(0, 15)
            .add_box(-4.0, -6.0, -3.0, 8.0, 6.0, 6.0),
        PartPose::offset(0.0, -5.0, 0.0),
    );
    body.child(
        "head",
        CubeList::new()
            .tex_offs(0, 0)
            .add_box(-4.0, -5.0, -5.0, 8.0, 5.0, 10.0)
            .tex_offs(56, 0)
            .add_box(-1.0, -2.0, -6.0, 2.0, 3.0, 2.0)
            .tex_offs(37, 8)
            .add_box_grow(-1.0, -9.0, -1.0, 2.0, 4.0, 2.0, Grow::all(-0.015))
            .tex_offs(37, 0)
            .add_box_grow(-2.0, -13.0, -2.0, 4.0, 4.0, 4.0, Grow::all(-0.015)),
        PartPose::offset(0.0, -6.0, 0.0),
    );
    let right_arm = body.child(
        "right_arm",
        CubeList::new(),
        PartPose::offset(-4.0, -6.0, 0.0),
    );
    right_arm.child(
        "right_arm_r1",
        CubeList::new()
            .tex_offs(36, 16)
            .add_box(-1.5, -5.0, -2.0, 3.0, 10.0, 4.0),
        PartPose::offset_rotation(1.0, 1.0, 0.0, 0.0, 0.0, 1.9199),
    );
    right_arm.child(
        "rightItem",
        CubeList::new(),
        PartPose::offset(-1.0, 7.4, -1.0),
    );
    body.child(
        "left_arm",
        CubeList::new(),
        PartPose::offset(4.0, -6.0, 0.0),
    )
    .child(
        "left_arm_r1",
        CubeList::new()
            .tex_offs(50, 16)
            .add_box(-1.5, -5.0, -2.0, 3.0, 10.0, 4.0),
        PartPose::offset_rotation(-1.0, 1.0, 0.0, 0.0, 0.0, -1.9199),
    );
    root.child(
        "right_leg",
        CubeList::new(),
        PartPose::offset(-3.0, -5.0, 0.0),
    )
    .child(
        "right_leg_r1",
        CubeList::new()
            .tex_offs(0, 27)
            .add_box(-2.0, -2.5, -2.0, 4.0, 5.0, 4.0),
        PartPose::offset_rotation(0.35, 2.0, 0.01, 0.0, 0.0, 0.2618),
    );
    root.child(
        "left_leg",
        CubeList::new(),
        PartPose::offset(1.0, -5.0, 0.0),
    )
    .child(
        "left_leg_r1",
        CubeList::new()
            .tex_offs(16, 27)
            .add_box(-2.0, -2.5, -2.0, 4.0, 5.0, 4.0),
        PartPose::offset_rotation(1.65, 2.0, 0.0, 0.0, 0.0, -0.2618),
    );
    LayerDef::create(mesh, 64, 64)
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
