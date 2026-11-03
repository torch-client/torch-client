#![allow(clippy::approx_constant, dead_code)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, Interpolation, Keyframe, Target, degree_vec as deg,
    pos_vec as pos, scale_vec as scale,
};

static CAMEL_BABY_WALK_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, -2.5), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, 2.5), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C1: &[Keyframe] = &[
    Keyframe::new(0.0, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, deg(-2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, deg(-2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(2.5, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C2: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, pos(0.0, 0.0, 0.1), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C3: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-22.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(-22.5, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C4: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.075, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.075, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.2083, pos(0.075, 4.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(0.075, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C5: &[Keyframe] = &[
    Keyframe::new(0.0, deg(22.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(-22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(22.5, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C6: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-0.1, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, pos(-0.1, 4.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, pos(-0.1, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(-0.1, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C7: &[Keyframe] = &[
    Keyframe::new(0.0, deg(22.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, deg(-9.49, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5833, deg(-17.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.2083, deg(7.38, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(22.5, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C8: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-0.1, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos(-0.1, 5.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5833, pos(-0.1, 0.0, -0.1), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(-0.1, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_WALK_C9: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-15.83, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(-7.38, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.25, deg(-21.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(-15.83, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_WALK_C10: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.1, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6667, pos(0.1, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, pos(0.1, 4.0, 0.17), Interpolation::CatmullRom),
    Keyframe::new(1.2083, pos(0.1, 0.0, -0.11), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(0.1, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, deg(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, deg(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, deg(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, deg(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C13: &[Keyframe] = &[
    Keyframe::new(0.0, deg(15.94, -8.42, 20.94), Interpolation::Linear),
    Keyframe::new(0.75, deg(15.94, 8.42, -20.94), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(15.94, -8.42, 20.94), Interpolation::Linear),
];

static CAMEL_BABY_WALK_C14: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -0.6, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, pos(0.0, -0.6, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_WALK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "root",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C0,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C1,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: CAMEL_BABY_WALK_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C3,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_WALK_C4,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C5,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_WALK_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C7,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_WALK_C8,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C9,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_WALK_C10,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C11,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C12,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_WALK_C13,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_BABY_WALK_C14,
    },
];

pub static CAMEL_BABY_WALK: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.5,
    looping: true,
    channels: CAMEL_BABY_WALK_CHANNELS,
};

static CAMEL_BABY_DASH_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(5.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_DASH_C1: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, -2.8, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_DASH_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(67.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(112.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(67.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(112.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(67.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_DASH_C3: &[Keyframe] = &[
    Keyframe::new(0.0, deg(5.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(5.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(5.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_DASH_C4: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 0.0, 1.0), Interpolation::Linear),
];

static CAMEL_BABY_DASH_C5: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        deg(44.9727, 1.7675, -1.7683),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.125, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.25,
        deg(44.9727, 1.7675, -1.7683),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.375, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.5,
        deg(44.9727, 1.7675, -1.7683),
        Interpolation::CatmullRom,
    ),
];

static CAMEL_BABY_DASH_C6: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.05, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_DASH_C7: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.125,
        deg(44.9727, -1.7675, 1.7683),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.25, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.375,
        deg(44.9727, -1.7675, 1.7683),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.5, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_DASH_C8: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(-0.05, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_DASH_C9: &[Keyframe] = &[
    Keyframe::new(0.0, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_DASH_C10: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(-0.05, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_DASH_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_DASH_C12: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.05, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_DASH_C13: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, -67.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, -67.5, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_DASH_C14: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 67.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 67.5, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_DASH_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_BABY_DASH_C1,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C2,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C3,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: CAMEL_BABY_DASH_C4,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C5,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_DASH_C6,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C7,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_DASH_C8,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C9,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_DASH_C10,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C11,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_DASH_C12,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C13,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_DASH_C14,
    },
];

pub static CAMEL_BABY_DASH: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: true,
    channels: CAMEL_BABY_DASH_CHANNELS,
};

static CAMEL_BABY_IDLE_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(5.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(4.9811, 0.4352, -4.9811), Interpolation::CatmullRom),
    Keyframe::new(3.0, deg(4.9872, -0.2942, 3.3674), Interpolation::CatmullRom),
    Keyframe::new(4.0, deg(5.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_IDLE_C1: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.0, deg(-2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(4.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_IDLE_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, -45.0), Interpolation::CatmullRom),
    Keyframe::new(2.5, deg(0.0, 0.0, -45.0), Interpolation::CatmullRom),
    Keyframe::new(2.625, deg(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(2.75, deg(0.0, 0.0, -45.0), Interpolation::CatmullRom),
    Keyframe::new(2.875, deg(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(3.0, deg(0.0, 0.0, -45.0), Interpolation::CatmullRom),
    Keyframe::new(4.0, deg(0.0, 0.0, -45.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_IDLE_C3: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 45.0), Interpolation::CatmullRom),
    Keyframe::new(2.5, deg(0.0, 0.0, 45.0), Interpolation::CatmullRom),
    Keyframe::new(2.625, deg(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(2.75, deg(0.0, 0.0, 45.0), Interpolation::CatmullRom),
    Keyframe::new(2.875, deg(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(3.0, deg(0.0, 0.0, 45.0), Interpolation::CatmullRom),
    Keyframe::new(4.0, deg(0.0, 0.0, 45.0), Interpolation::CatmullRom),
];

static CAMEL_BABY_IDLE_C4: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, -0.1, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_IDLE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_IDLE_C0,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_IDLE_C1,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_IDLE_C2,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_IDLE_C3,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_BABY_IDLE_C4,
    },
];

pub static CAMEL_BABY_IDLE: AnimationDefinition = AnimationDefinition {
    length_seconds: 4.0,
    looping: true,
    channels: CAMEL_BABY_IDLE_CHANNELS,
};

static CAMEL_BABY_SIT_POSE_C0: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(2.5, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C1: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, -13.25, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C2: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(-90.0, 10.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C3: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.1, -11.0, 6.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C4: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(-90.0, -10.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C5: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, -11.0, 6.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C6: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(-90.0, -11.88, -0.54),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C7: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(1.4, -10.95, 1.95),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C8: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(-90.0, 11.89, 0.54),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C9: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(-1.33, -11.0, 1.95),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C10: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(-4.75, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C11: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 0.0, 1.22),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C12: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(52.5, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C13: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C14: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(0.0, 0.0, -22.5),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C15: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C16: &[Keyframe] = &[Keyframe::new(
    0.0,
    deg(0.0, 0.0, 15.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C17: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C18: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_C19: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_POSE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C1,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C3,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C4,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C5,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C7,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C8,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C9,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C10,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C11,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C12,
    },
    Channel {
        bone: "tail",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C13,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C14,
    },
    Channel {
        bone: "left_ear",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C15,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_POSE_C16,
    },
    Channel {
        bone: "right_ear",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C17,
    },
    Channel {
        bone: "root",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_POSE_C18,
    },
    Channel {
        bone: "root",
        target: Target::Scale,
        keyframes: CAMEL_BABY_SIT_POSE_C19,
    },
];

pub static CAMEL_BABY_SIT_POSE: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.0,
    looping: true,
    channels: CAMEL_BABY_SIT_POSE_CHANNELS,
};
