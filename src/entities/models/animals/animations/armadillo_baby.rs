#![allow(clippy::approx_constant, dead_code)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, Interpolation, Keyframe, Target, degree_vec as deg,
    pos_vec as pos,
};

static ARMADILLO_BABY_ROLL_UP_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 5.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, 6.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 6.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, -1.0, -1.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, 1.0, -3.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 1.0, -3.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, 2.0, -3.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(17.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(-72.5, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, -1.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, 0.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.3, pos(0.0, 0.0, 5.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, 0.0, 5.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, -0.25, 5.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 5.0, -2.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, 8.0, -2.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 8.0, -2.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(1.0, 3.0, -2.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(-45.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 5.0, -2.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, 8.0, -2.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 8.0, -2.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(-1.0, 3.0, -3.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(-27.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, deg(-32.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(-85.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C11: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 5.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(-1.0, 8.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(-1.0, 9.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(-1.0, 2.0, 3.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(-12.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(-85.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C13: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 5.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(1.0, 8.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(1.0, 9.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(1.0, 2.0, 3.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C14: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, deg(-2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.45, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_C15: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 3.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 8.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, 7.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 7.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.45, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_UP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_UP_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_UP_C1,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_UP_C2,
    },
    Channel {
        bone: "tail",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_UP_C3,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_UP_C4,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_UP_C5,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_UP_C6,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_UP_C7,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_UP_C8,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_UP_C9,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_UP_C10,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_UP_C11,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_UP_C12,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_UP_C13,
    },
    Channel {
        bone: "cube",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_UP_C14,
    },
    Channel {
        bone: "cube",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_UP_C15,
    },
];

pub static ARMADILLO_BABY_ROLL_UP: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: false,
    channels: ARMADILLO_BABY_ROLL_UP_CHANNELS,
};

static ARMADILLO_BABY_WALK_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(0.0, 0.0, 4.6), Interpolation::CatmullRom),
    Keyframe::new(0.3, deg(0.0, 0.0, 6.81), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.7, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.95, deg(0.0, 0.0, -4.6), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(0.0, 0.0, -6.89), Interpolation::CatmullRom),
    Keyframe::new(1.25, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.45, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_WALK_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, pos(0.0, -0.2, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.7, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.95, pos(0.0, -0.2, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.25, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.45, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_WALK_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-9.17, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(-8.24, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C3: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(-20.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C4: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.0, 1.0, -0.18), Interpolation::Linear),
    Keyframe::new(1.45, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C5: &[Keyframe] = &[
    Keyframe::new(0.0, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.55, deg(-20.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.95, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, deg(50.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C6: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, -0.25), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(0.55, pos(0.0, 1.0, -0.18), Interpolation::Linear),
    Keyframe::new(0.7, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.95, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, pos(0.0, 0.0, -0.25), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C7: &[Keyframe] = &[
    Keyframe::new(0.0, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.55, deg(-20.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.95, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, deg(50.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C8: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, -0.25), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(0.55, pos(0.0, 1.0, -0.18), Interpolation::Linear),
    Keyframe::new(0.7, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.95, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, pos(0.0, 0.0, -0.25), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C9: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(-20.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C10: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.0, 1.0, -0.18), Interpolation::Linear),
    Keyframe::new(1.45, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_WALK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_WALK_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_WALK_C1,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_WALK_C2,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_WALK_C3,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_WALK_C4,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_WALK_C5,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_WALK_C6,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_WALK_C7,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_WALK_C8,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_WALK_C9,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_WALK_C10,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_WALK_C11,
    },
];

pub static ARMADILLO_BABY_WALK: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.4583,
    looping: true,
    channels: ARMADILLO_BABY_WALK_CHANNELS,
};

static ARMADILLO_BABY_ROLL_OUT_C0: &[Keyframe] = &[
    Keyframe::new(0.1, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.65, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.85, deg(-2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.95, deg(-7.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.05, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(7.5, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -1.9, 5.0), Interpolation::Linear),
    Keyframe::new(0.05, pos(0.0, -1.0, 0.2), Interpolation::Linear),
    Keyframe::new(0.1, pos(0.0, 0.0, 0.2), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 2.1, 1.2), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.03, 0.13), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, 0.03, 0.13), Interpolation::Linear),
    Keyframe::new(0.65, pos(0.0, 0.03, 0.13), Interpolation::Linear),
    Keyframe::new(0.7, pos(0.0, 0.1, 0.2), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 3.1, 2.2), Interpolation::Linear),
    Keyframe::new(0.85, pos(0.0, 4.1, 2.2), Interpolation::Linear),
    Keyframe::new(0.9, pos(0.0, 0.1, 0.2), Interpolation::Linear),
    Keyframe::new(0.95, pos(0.0, 0.9, -0.8), Interpolation::Linear),
    Keyframe::new(1.05, pos(0.0, 0.9, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, pos(0.0, 2.6, 0.2), Interpolation::Linear),
    Keyframe::new(1.15, pos(0.0, 2.4, 0.2), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, 0.0, 0.2), Interpolation::Linear),
    Keyframe::new(1.25, pos(0.0, 0.0, 0.2), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.0, 0.0, 0.2), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C2: &[Keyframe] = &[
    Keyframe::new(1.1, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(0.0, 0.0, 30.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C3: &[Keyframe] = &[
    Keyframe::new(1.1, pos(0.0, 3.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, 8.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.3, pos(-1.0, 3.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C4: &[Keyframe] = &[
    Keyframe::new(1.1, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(0.0, 0.0, -30.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C5: &[Keyframe] = &[
    Keyframe::new(1.1, pos(0.0, 3.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, 8.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.3, pos(1.0, 3.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.35, pos(1.0, 3.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.05, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.55, deg(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(-92.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.1, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.3, deg(0.0, 0.0, 30.0), Interpolation::CatmullRom),
    Keyframe::new(1.4, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.45, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_ROLL_OUT_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-1.0, 2.0, 2.0), Interpolation::Linear),
    Keyframe::new(0.05, pos(-1.0, 2.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, pos(0.0, 2.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.55, pos(0.0, 2.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.7, pos(-1.0, 2.0, 2.63), Interpolation::CatmullRom),
    Keyframe::new(1.1, pos(-1.0, 2.0, 2.0), Interpolation::Linear),
    Keyframe::new(1.2, pos(-1.0, 7.0, 2.0), Interpolation::Linear),
    Keyframe::new(1.3, pos(-1.0, 3.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.45, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.05, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.55, deg(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(-87.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.1, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.3, deg(0.0, 0.0, -30.0), Interpolation::CatmullRom),
    Keyframe::new(1.4, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.45, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_ROLL_OUT_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(1.0, 2.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(0.05, pos(1.0, 2.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(0.15, pos(1.0, 2.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, pos(0.0, 2.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.55, pos(0.0, 2.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.7, pos(1.0, 2.0, 1.88), Interpolation::CatmullRom),
    Keyframe::new(0.75, pos(1.0, 2.0, 2.67), Interpolation::CatmullRom),
    Keyframe::new(1.1, pos(1.0, 2.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(1.2, pos(1.0, 8.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(1.25, pos(1.06, 5.06, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.3, pos(1.0, 3.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.4, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.45, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_ROLL_OUT_C10: &[Keyframe] = &[
    Keyframe::new(1.1, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, pos(0.0, 5.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.0, -1.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C12: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, -1.4129, 0.0617), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C13: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C14: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, -1.4129, 0.0617), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C15: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.05, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(-7.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(-17.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.85, deg(-25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.95, deg(12.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.05, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_C16: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.05, pos(0.0, 1.6, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, 0.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 1.2, 0.0), Interpolation::Linear),
    Keyframe::new(0.85, pos(0.0, 1.7, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.95, pos(0.0, 1.3, 0.0), Interpolation::Linear),
    Keyframe::new(1.05, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, 5.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, pos(0.0, 8.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 1.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_ROLL_OUT_CHANNELS: &[Channel] = &[
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C0,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C1,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C2,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C3,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C4,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C5,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C6,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C7,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C8,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C9,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C10,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C11,
    },
    Channel {
        bone: "left_ear",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C12,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C13,
    },
    Channel {
        bone: "right_ear",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C14,
    },
    Channel {
        bone: "cube",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C15,
    },
    Channel {
        bone: "cube",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_ROLL_OUT_C16,
    },
];

pub static ARMADILLO_BABY_ROLL_OUT: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.5,
    looping: false,
    channels: ARMADILLO_BABY_ROLL_OUT_CHANNELS,
};

static ARMADILLO_BABY_PEEK_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-70.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.15, deg(-65.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(-7.5, 0.0, 45.0), Interpolation::CatmullRom),
    Keyframe::new(1.15, deg(-7.5, 0.0, 45.0), Interpolation::Linear),
    Keyframe::new(
        1.3,
        deg(-0.8639, -1.4959, -39.1287),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(1.6, deg(-0.8639, -1.4959, -39.1287), Interpolation::Linear),
    Keyframe::new(1.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, deg(-25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.85, deg(-70.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_PEEK_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -1.25, 4.0), Interpolation::Linear),
    Keyframe::new(0.1, pos(0.0, -1.0, 4.0), Interpolation::Linear),
    Keyframe::new(0.15, pos(0.0, -1.0, 4.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, -1.0, 5.0), Interpolation::Linear),
    Keyframe::new(0.35, pos(0.0, 0.0, 0.2), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, 0.0, 0.2), Interpolation::Linear),
    Keyframe::new(0.45, pos(0.0, 0.05, 0.7), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 1.1, 1.2), Interpolation::Linear),
    Keyframe::new(0.6, pos(0.0, 0.1, 0.8), Interpolation::Linear),
    Keyframe::new(0.7, pos(0.0, 0.1, 0.7), Interpolation::Linear),
    Keyframe::new(1.75, pos(0.0, 0.1, 0.7), Interpolation::Linear),
    Keyframe::new(1.8, pos(0.0, 0.1, 0.2), Interpolation::Linear),
    Keyframe::new(1.9, pos(0.0, -1.2, 3.53), Interpolation::Linear),
    Keyframe::new(1.95, pos(0.0, -1.2, 3.53), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, -1.2, 3.53), Interpolation::Linear),
    Keyframe::new(2.15, pos(0.0, -0.9, 4.2), Interpolation::Linear),
    Keyframe::new(2.3, pos(0.0, -0.9, 3.4), Interpolation::Linear),
    Keyframe::new(2.5, pos(0.0, -1.9, 4.1), Interpolation::Linear),
];

static ARMADILLO_BABY_PEEK_C2: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 3.0, -2.0),
    Interpolation::Linear,
)];

static ARMADILLO_BABY_PEEK_C3: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 3.0, -2.0),
    Interpolation::Linear,
)];

static ARMADILLO_BABY_PEEK_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.85, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.75, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.8, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.95, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_PEEK_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-1.0, 2.0, 2.0), Interpolation::Linear),
    Keyframe::new(0.6, pos(-1.0, 2.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(0.65, pos(-1.0, 2.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.85, pos(0.0, 1.0, -0.5), Interpolation::CatmullRom),
    Keyframe::new(1.0, pos(0.0, 1.0, -0.5), Interpolation::CatmullRom),
    Keyframe::new(1.75, pos(0.0, 1.0, -0.5), Interpolation::CatmullRom),
    Keyframe::new(1.95, pos(0.0, 1.0, 3.0), Interpolation::CatmullRom),
    Keyframe::new(2.0, pos(-1.0, 2.0, 3.0), Interpolation::CatmullRom),
    Keyframe::new(2.15, pos(-1.0, 3.0, 4.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_PEEK_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.65, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.85, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.75, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.8, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.95, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_PEEK_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(1.0, 2.0, 2.0), Interpolation::Linear),
    Keyframe::new(0.65, pos(1.0, 2.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(0.85, pos(0.0, 1.0, -0.4), Interpolation::CatmullRom),
    Keyframe::new(1.0, pos(0.0, 1.0, -0.4), Interpolation::CatmullRom),
    Keyframe::new(1.75, pos(0.0, 1.0, -0.4), Interpolation::CatmullRom),
    Keyframe::new(1.95, pos(1.0, 1.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(2.0, pos(1.0, 2.0, 3.0), Interpolation::CatmullRom),
    Keyframe::new(2.15, pos(1.0, 3.0, 4.0), Interpolation::CatmullRom),
];

static ARMADILLO_BABY_PEEK_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.85, deg(-10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_PEEK_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, -1.13, 1.05), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, -1.4129, 0.0617), Interpolation::Linear),
    Keyframe::new(0.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_PEEK_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.85, deg(-10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_PEEK_C11: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, -1.13, 1.05), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, -1.4129, 0.0617), Interpolation::Linear),
    Keyframe::new(0.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_PEEK_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.35, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-7.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.05, deg(-17.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.15, deg(-25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, deg(12.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_PEEK_C13: &[Keyframe] = &[
    Keyframe::new(0.25, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.35, pos(0.0, 1.6, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 0.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.05, pos(0.0, 1.2, 0.0), Interpolation::Linear),
    Keyframe::new(2.15, pos(0.0, 1.7, 0.0), Interpolation::Linear),
    Keyframe::new(2.25, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, pos(0.0, 1.3, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static ARMADILLO_BABY_PEEK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_PEEK_C0,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_PEEK_C1,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_PEEK_C2,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_PEEK_C3,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_PEEK_C4,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_PEEK_C5,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_PEEK_C6,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_PEEK_C7,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_PEEK_C8,
    },
    Channel {
        bone: "left_ear",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_PEEK_C9,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_PEEK_C10,
    },
    Channel {
        bone: "right_ear",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_PEEK_C11,
    },
    Channel {
        bone: "cube",
        target: Target::Rotation,
        keyframes: ARMADILLO_BABY_PEEK_C12,
    },
    Channel {
        bone: "cube",
        target: Target::Position,
        keyframes: ARMADILLO_BABY_PEEK_C13,
    },
];

pub static ARMADILLO_BABY_PEEK: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.5,
    looping: false,
    channels: ARMADILLO_BABY_PEEK_CHANNELS,
};
