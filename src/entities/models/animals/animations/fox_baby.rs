#![allow(clippy::approx_constant, dead_code)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, Interpolation, Keyframe, Target, degree_vec as deg,
    pos_vec as pos, scale_vec as scale,
};

static FOX_BABY_WALK_C0: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C1: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C2: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C3: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, -1.025, 0.0),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-35.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-35.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static FOX_BABY_WALK_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.05, 0.6, -0.02), Interpolation::Linear),
    Keyframe::new(0.25, pos(0.05, 0.6, -0.02), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.05, 0.6, -0.02), Interpolation::Linear),
];

static FOX_BABY_WALK_C6: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale(1.0, 1.149999976158142, 1.0),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C7: &[Keyframe] = &[
    Keyframe::new(0.0, deg(35.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(35.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static FOX_BABY_WALK_C8: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-0.05, 0.6, -0.02), Interpolation::Linear),
    Keyframe::new(0.25, pos(-0.05, 0.6, -0.02), Interpolation::Linear),
    Keyframe::new(0.5, pos(-0.05, 0.6, -0.02), Interpolation::Linear),
];

static FOX_BABY_WALK_C9: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale(1.0, 1.149999976158142, 1.0),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(35.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(35.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static FOX_BABY_WALK_C11: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.05, 0.6, -0.4),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C12: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale(1.0, 1.149999976158142, 1.0),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C13: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-35.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-35.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static FOX_BABY_WALK_C14: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(-0.05, 0.6, -0.4),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C15: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale(1.0, 1.149999976158142, 1.0),
    Interpolation::Linear,
)];

static FOX_BABY_WALK_C16: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(-2.5, 0.0, 0.0),
    Interpolation::CatmullRom,
)];

static FOX_BABY_WALK_C17: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, -0.05, 0.0),
    Interpolation::CatmullRom,
)];

static FOX_BABY_WALK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: FOX_BABY_WALK_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: FOX_BABY_WALK_C1,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: FOX_BABY_WALK_C2,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: FOX_BABY_WALK_C3,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: FOX_BABY_WALK_C4,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: FOX_BABY_WALK_C5,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Scale,
        keyframes: FOX_BABY_WALK_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: FOX_BABY_WALK_C7,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: FOX_BABY_WALK_C8,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Scale,
        keyframes: FOX_BABY_WALK_C9,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: FOX_BABY_WALK_C10,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: FOX_BABY_WALK_C11,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Scale,
        keyframes: FOX_BABY_WALK_C12,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: FOX_BABY_WALK_C13,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: FOX_BABY_WALK_C14,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Scale,
        keyframes: FOX_BABY_WALK_C15,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: FOX_BABY_WALK_C16,
    },
    Channel {
        bone: "tail",
        target: Target::Position,
        keyframes: FOX_BABY_WALK_C17,
    },
];

pub static FOX_BABY_WALK: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: true,
    channels: FOX_BABY_WALK_CHANNELS,
};
