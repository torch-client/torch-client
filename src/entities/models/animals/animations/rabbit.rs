#![allow(clippy::approx_constant, dead_code)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, Interpolation, Keyframe, Target, degree_vec as deg,
    pos_vec as pos, scale_vec as scale,
};

static HOP_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, deg(4.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2917, deg(32.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4167, deg(33.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5833, deg(18.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, deg(-4.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2917, deg(-32.17, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, deg(-34.58, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5833, deg(-20.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(125.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2917, deg(125.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, deg(120.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, deg(95.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5417, deg(42.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-0.17, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, deg(-0.17, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, deg(25.25, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2917, deg(-65.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, deg(-67.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.625, deg(-1.25, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.749, deg(-1.25, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, pos(0.0, 0.5, 0.6), Interpolation::Linear),
    Keyframe::new(0.4167, pos(0.0, 0.9, 0.4), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2083, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.3333, deg(0.0, 0.0, -17.5), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(0.0, 0.0, -17.5), Interpolation::CatmullRom),
    Keyframe::new(0.5833, deg(0.0, 0.0, -2.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static HOP_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2083, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.3333, deg(0.0, 0.0, 20.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(0.0, 0.0, 20.0), Interpolation::CatmullRom),
    Keyframe::new(0.5833, deg(0.0, 0.0, 2.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static HOP_C11: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(-48.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5417, deg(-41.24, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C13: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-0.025, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2083, pos(-0.025, -0.2, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, pos(-0.02, -0.3, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, pos(-0.025, 0.0, 0.0), Interpolation::CatmullRom),
];

static HOP_C14: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(7.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(-31.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(-35.33, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static HOP_C15: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.025, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2083, pos(0.025, -0.3, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, pos(0.02, -0.23, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, pos(0.025, 0.0, 0.0), Interpolation::CatmullRom),
];

static HOP_C16: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, -2.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(0.0, -2.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, deg(47.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4167, deg(47.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, -2.5, 0.0), Interpolation::Linear),
];

static HOP_C17: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C18: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, deg(47.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4167, deg(47.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C19: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C20: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, deg(-25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, deg(27.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_C21: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static HOP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: HOP_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: HOP_C1,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: HOP_C2,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: HOP_C3,
    },
    Channel {
        bone: "backlegs",
        target: Target::Rotation,
        keyframes: HOP_C4,
    },
    Channel {
        bone: "backlegs",
        target: Target::Position,
        keyframes: HOP_C5,
    },
    Channel {
        bone: "frontlegs",
        target: Target::Rotation,
        keyframes: HOP_C6,
    },
    Channel {
        bone: "frontlegs",
        target: Target::Position,
        keyframes: HOP_C7,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: HOP_C8,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: HOP_C9,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: HOP_C10,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: HOP_C11,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: HOP_C12,
    },
    Channel {
        bone: "left_ear",
        target: Target::Position,
        keyframes: HOP_C13,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: HOP_C14,
    },
    Channel {
        bone: "right_ear",
        target: Target::Position,
        keyframes: HOP_C15,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: HOP_C16,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: HOP_C17,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: HOP_C18,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: HOP_C19,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: HOP_C20,
    },
    Channel {
        bone: "tail",
        target: Target::Position,
        keyframes: HOP_C21,
    },
];

pub static HOP: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.75,
    looping: true,
    channels: HOP_CHANNELS,
};

static IDLE_HEAD_TILT_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(-62.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(-57.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7083, deg(-57.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.875, deg(-57.0, 12.0, 2.0), Interpolation::Linear),
    Keyframe::new(0.9583, deg(-57.0, 5.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.5417, deg(-57.0, 5.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.7083, deg(-56.5, -16.0, -3.0), Interpolation::Linear),
    Keyframe::new(1.7917, deg(-57.0, -7.5, -1.5), Interpolation::Linear),
    Keyframe::new(2.5417, deg(-57.0, -7.5, -1.5), Interpolation::Linear),
    Keyframe::new(2.7083, deg(-57.0, 11.0, 2.0), Interpolation::Linear),
    Keyframe::new(2.7917, deg(-57.0, 5.0, 1.0), Interpolation::Linear),
    Keyframe::new(2.9583, deg(-57.0, 5.0, 1.0), Interpolation::Linear),
    Keyframe::new(3.1667, deg(-57.0, 5.0, 1.0), Interpolation::Linear),
    Keyframe::new(3.25, deg(-57.5, 5.0, 1.0), Interpolation::Linear),
    Keyframe::new(3.4583, deg(-60.0, 5.0, 0.5), Interpolation::Linear),
    Keyframe::new(3.5833, deg(-60.0, 5.0, 0.5), Interpolation::Linear),
    Keyframe::new(3.6667, deg(-39.5, 3.5, 0.5), Interpolation::Linear),
    Keyframe::new(3.8333, deg(10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.9167, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.0, 0.7, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.8333, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.9167, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C2: &[Keyframe] = &[
    Keyframe::new(0.0, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(3.8333, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(3.9167, scale(1.0, 1.0, 1.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C3: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.0833, deg(33.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2083, deg(54.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2917, deg(55.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(55.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7083, deg(55.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.8333, deg(55.0, -3.5, -19.5), Interpolation::CatmullRom),
    Keyframe::new(0.9167, deg(55.0, -6.0, 7.5), Interpolation::CatmullRom),
    Keyframe::new(1.0417, deg(55.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5417, deg(55.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.625, deg(55.0, 2.5, 12.5), Interpolation::CatmullRom),
    Keyframe::new(1.7083, deg(55.0, 4.5, 25.5), Interpolation::CatmullRom),
    Keyframe::new(1.7917, deg(55.0, 19.5, -16.0), Interpolation::CatmullRom),
    Keyframe::new(1.875, deg(55.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5417, deg(54.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.5833, deg(54.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.8333, deg(-46.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(3.9167, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_HEAD_TILT_C4: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.8333, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.9167, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0417, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5417, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.8333, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.9167, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C5: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, deg(54.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9167, deg(40.0, 20.5, -11.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(39.5, 11.5, -6.0), Interpolation::CatmullRom),
    Keyframe::new(1.2917, deg(39.3, 11.5, -6.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(39.5, 11.5, -6.0), Interpolation::Linear),
    Keyframe::new(1.9167, deg(36.0, -28.5, 16.5), Interpolation::Linear),
    Keyframe::new(2.0, deg(38.0, -18.5, 10.0), Interpolation::Linear),
    Keyframe::new(2.625, deg(38.0, -18.5, 10.0), Interpolation::Linear),
    Keyframe::new(2.7083, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.7917, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.9583, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.1667, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.25, deg(42.5, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(3.5833, deg(45.5, -0.5, 0.0), Interpolation::Linear),
    Keyframe::new(3.8333, deg(-20.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(3.9167, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C6: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(0.3333, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(0.5833, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.75, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(2.7083, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(3.5833, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(3.8333, pos(0.0, -1.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(3.9167, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C7: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2917, deg(19.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9167, deg(-15.0, 0.0, 7.5), Interpolation::Linear),
    Keyframe::new(1.0, deg(-7.5, 0.0, -5.0), Interpolation::Linear),
    Keyframe::new(1.0833, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.6667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(-3.0, 1.5, -17.0), Interpolation::Linear),
    Keyframe::new(1.8333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9167, deg(-48.75, 0.0, 5.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-65.0, 2.5, 0.5), Interpolation::Linear),
    Keyframe::new(2.0833, deg(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.1667, deg(-40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5417, deg(-40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.625, deg(11.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.7083, deg(23.25, 0.0, 3.75), Interpolation::Linear),
    Keyframe::new(2.75, deg(25.5, 2.0, 6.0), Interpolation::Linear),
    Keyframe::new(2.8333, deg(10.0, -0.5, 0.75), Interpolation::Linear),
    Keyframe::new(3.625, deg(10.0, -0.5, 0.75), Interpolation::Linear),
    Keyframe::new(3.7083, deg(-25.0, 0.25, -1.0), Interpolation::Linear),
    Keyframe::new(3.7917, deg(-19.0, 0.0, -1.0), Interpolation::Linear),
    Keyframe::new(3.8333, deg(2.5, 0.2, -0.5), Interpolation::Linear),
    Keyframe::new(3.9167, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C8: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, pos(-0.125, -0.6553, 0.1589), Interpolation::Linear),
    Keyframe::new(0.3333, pos(-0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(-0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(
        0.9167,
        pos(-0.0502, -0.1915, -0.0518),
        Interpolation::Linear,
    ),
    Keyframe::new(1.0, pos(-0.2423, -0.1025, 0.0261), Interpolation::Linear),
    Keyframe::new(1.0833, pos(-0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.6667, pos(-0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, pos(-0.3, -0.3, 0.0), Interpolation::Linear),
    Keyframe::new(1.7917, pos(-0.095, -0.15, 0.0), Interpolation::Linear),
    Keyframe::new(1.8333, pos(-0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(-0.025, -0.575, 0.0), Interpolation::Linear),
    Keyframe::new(2.0833, pos(-0.025, -0.575, 0.0), Interpolation::Linear),
    Keyframe::new(2.1667, pos(-0.025, -0.575, 0.0), Interpolation::Linear),
    Keyframe::new(2.5417, pos(-0.065, -0.575, 0.0), Interpolation::Linear),
    Keyframe::new(2.625, pos(-0.025, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(2.75, pos(-0.025, -0.25, 0.25), Interpolation::Linear),
    Keyframe::new(2.7917, pos(-0.025, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(2.8333, pos(-0.025, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(3.625, pos(-0.025, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(3.9167, pos(-0.025, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C9: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(
        0.9167,
        deg(-22.3423, -2.5587, 5.4476),
        Interpolation::Linear,
    ),
    Keyframe::new(1.0, deg(-11.17, -1.28, -21.03), Interpolation::Linear),
    Keyframe::new(
        1.0833,
        deg(-4.1101, 3.0703, -18.1443),
        Interpolation::Linear,
    ),
    Keyframe::new(1.125, deg(-0.0844, 3.027, -9.5338), Interpolation::Linear),
    Keyframe::new(1.5417, deg(-0.0844, 3.027, -9.5338), Interpolation::Linear),
    Keyframe::new(1.6667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(-1.5, 1.0, -14.5), Interpolation::Linear),
    Keyframe::new(1.8333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9167, deg(-45.0, 0.0, 5.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-60.0, 1.0, 5.0), Interpolation::Linear),
    Keyframe::new(2.0833, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5417, deg(-50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.625, deg(16.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.7083, deg(25.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.7917, deg(5.25, 0.5, -2.0), Interpolation::Linear),
    Keyframe::new(2.8333, deg(8.5, 1.0, -4.0), Interpolation::Linear),
    Keyframe::new(3.625, deg(8.5, 1.0, -4.0), Interpolation::Linear),
    Keyframe::new(3.7083, deg(-37.5, 0.25, -1.5), Interpolation::Linear),
    Keyframe::new(3.7917, deg(-23.5, 0.0, -1.0), Interpolation::Linear),
    Keyframe::new(3.8333, deg(10.0, 0.25, -0.5), Interpolation::Linear),
    Keyframe::new(3.9167, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C10: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.07, -0.59, 0.14), Interpolation::Linear),
    Keyframe::new(0.2083, pos(0.075, -0.4587, 0.1769), Interpolation::Linear),
    Keyframe::new(0.3333, pos(0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.875, pos(0.13, -0.15, 0.0), Interpolation::Linear),
    Keyframe::new(0.9167, pos(0.125, -0.2, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0182, -0.4258, 0.0581), Interpolation::Linear),
    Keyframe::new(1.0833, pos(0.02, -0.36, 0.02), Interpolation::Linear),
    Keyframe::new(1.125, pos(0.025, -0.175, 0.0), Interpolation::Linear),
    Keyframe::new(1.5417, pos(0.025, -0.175, 0.0), Interpolation::Linear),
    Keyframe::new(1.6667, pos(0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, pos(0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8333, pos(0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.1, -0.55, 0.0), Interpolation::Linear),
    Keyframe::new(2.0833, pos(0.025, -0.5, 0.0), Interpolation::Linear),
    Keyframe::new(2.5417, pos(0.025, -0.5, 0.0), Interpolation::Linear),
    Keyframe::new(2.5833, pos(0.025, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(2.7083, pos(0.025, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(2.8333, pos(0.025, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(3.625, pos(0.025, -0.25, 0.0), Interpolation::Linear),
    Keyframe::new(3.9167, pos(0.025, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(60.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4167, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7083, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.8333, deg(38.5, 15.0, -9.5), Interpolation::Linear),
    Keyframe::new(0.9167, deg(39.5, 7.0, -4.5), Interpolation::Linear),
    Keyframe::new(1.0, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.6667, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(36.5, 28.5, -16.0), Interpolation::Linear),
    Keyframe::new(1.8333, deg(38.5, 20.0, -10.5), Interpolation::Linear),
    Keyframe::new(3.5, deg(38.5, 20.0, -10.5), Interpolation::Linear),
    Keyframe::new(3.5833, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.75, deg(12.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.8333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.9167, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C12: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(0.4167, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(0.7083, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(1.6667, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(3.5833, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(3.75, pos(0.0, 0.5, 0.5), Interpolation::Linear),
    Keyframe::new(3.8333, pos(0.0, 1.0, -0.5), Interpolation::Linear),
    Keyframe::new(3.9167, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C13: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, deg(70.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7083, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.8333, deg(40.0, 9.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.9167, deg(39.5, 5.0, -4.5), Interpolation::Linear),
    Keyframe::new(1.0, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.6667, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(37.5, -24.5, 13.0), Interpolation::Linear),
    Keyframe::new(1.8333, deg(39.5, -15.5, 8.0), Interpolation::Linear),
    Keyframe::new(3.5, deg(39.5, -15.5, 8.0), Interpolation::Linear),
    Keyframe::new(3.5833, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.75, deg(10.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.8333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.9167, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C14: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, pos(0.0, 0.8, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(0.7083, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(1.6667, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(3.5833, pos(0.0, 0.6, 0.0), Interpolation::Linear),
    Keyframe::new(3.75, pos(0.0, 0.5, 0.0), Interpolation::Linear),
    Keyframe::new(3.8333, pos(0.0, 1.0, -0.5), Interpolation::Linear),
    Keyframe::new(3.9167, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_C15: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2917, pos(0.0, 0.1981, 0.0278), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(0.0, 0.2, 0.03), Interpolation::CatmullRom),
    Keyframe::new(1.8333, pos(-0.0261, 0.005, -0.0061), Interpolation::Linear),
    Keyframe::new(3.625, pos(-0.03, 0.01, -0.01), Interpolation::Linear),
    Keyframe::new(3.9167, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_HEAD_TILT_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: IDLE_HEAD_TILT_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: IDLE_HEAD_TILT_C1,
    },
    Channel {
        bone: "body",
        target: Target::Scale,
        keyframes: IDLE_HEAD_TILT_C2,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: IDLE_HEAD_TILT_C3,
    },
    Channel {
        bone: "tail",
        target: Target::Position,
        keyframes: IDLE_HEAD_TILT_C4,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: IDLE_HEAD_TILT_C5,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: IDLE_HEAD_TILT_C6,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: IDLE_HEAD_TILT_C7,
    },
    Channel {
        bone: "left_ear",
        target: Target::Position,
        keyframes: IDLE_HEAD_TILT_C8,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: IDLE_HEAD_TILT_C9,
    },
    Channel {
        bone: "right_ear",
        target: Target::Position,
        keyframes: IDLE_HEAD_TILT_C10,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: IDLE_HEAD_TILT_C11,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: IDLE_HEAD_TILT_C12,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: IDLE_HEAD_TILT_C13,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: IDLE_HEAD_TILT_C14,
    },
    Channel {
        bone: "frontlegs",
        target: Target::Position,
        keyframes: IDLE_HEAD_TILT_C15,
    },
];

pub static IDLE_HEAD_TILT: AnimationDefinition = AnimationDefinition {
    length_seconds: 4.0,
    looping: false,
    channels: IDLE_HEAD_TILT_CHANNELS,
};
