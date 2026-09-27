#![allow(clippy::approx_constant, dead_code)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, DEG_TO_RAD, Interpolation, Keyframe, Target,
};

const fn deg(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x * DEG_TO_RAD, y * DEG_TO_RAD, z * DEG_TO_RAD]
}

const fn pos(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x, -y, z]
}

static CAMEL_WALK_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 2.5), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(0.0, 0.0, -2.5), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, 2.5), Interpolation::CatmullRom),
];

static CAMEL_WALK_C1: &[Keyframe] = &[
    Keyframe::new(0.0, deg(2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(-2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, deg(-2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(2.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(-22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(22.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, pos(0.0, 4.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(-22.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.2083, pos(0.0, 4.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-20.4, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.375, deg(-22.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-20.4, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_WALK_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -0.21, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0833, pos(0.0, 4.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.375, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, -0.21, 0.0), Interpolation::Linear),
];

static CAMEL_WALK_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.625, deg(-22.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(22.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, pos(0.0, 4.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.625, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, deg(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, deg(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_WALK_C12: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        deg(15.94102, -8.42106, 20.94102),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.75,
        deg(15.94102, 8.42106, -20.94102),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.5,
        deg(15.94102, -8.42106, 20.94102),
        Interpolation::CatmullRom,
    ),
];

static CAMEL_WALK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "root",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C0,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C1,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_WALK_C3,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C4,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_WALK_C5,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_WALK_C7,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C8,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_WALK_C9,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C10,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C11,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_WALK_C12,
    },
];

pub static CAMEL_WALK: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.5,
    looping: true,
    channels: CAMEL_WALK_CHANNELS,
};

static CAMEL_SIT_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, deg(24.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.0, 0.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.8, pos(0.0, -6.0, 1.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, -19.9, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, -2.0, 11.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, -2.0, 11.0), Interpolation::Linear),
    Keyframe::new(1.7, pos(0.0, -8.4, 11.4), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, -20.6, 12.0), Interpolation::Linear),
];

static CAMEL_SIT_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, -2.0, 11.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, -2.0, 11.0), Interpolation::Linear),
    Keyframe::new(1.7, pos(0.0, -8.4, 11.4), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, -20.6, 12.0), Interpolation::Linear),
];

static CAMEL_SIT_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, deg(-15.0, -3.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(-65.0, -9.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-90.0, -15.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.7, pos(1.0, -0.62, 0.25), Interpolation::Linear),
    Keyframe::new(1.9, pos(0.5, -11.25, 2.5), Interpolation::Linear),
    Keyframe::new(2.0, pos(1.0, -20.5, 5.0), Interpolation::Linear),
];

static CAMEL_SIT_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, deg(-15.0, 3.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(-65.0, 9.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-90.0, 15.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.7, pos(-1.0, -0.62, 0.25), Interpolation::Linear),
    Keyframe::new(1.9, pos(-0.5, -11.25, 2.5), Interpolation::Linear),
    Keyframe::new(2.0, pos(-1.0, -20.5, 5.0), Interpolation::Linear),
];

static CAMEL_SIT_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(-27.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-21.25, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(80.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(50.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_SIT_C1,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_SIT_C3,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_C4,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_SIT_C5,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_SIT_C7,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_C8,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_SIT_C9,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_C10,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_C11,
    },
];

pub static CAMEL_SIT: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.0,
    looping: false,
    channels: CAMEL_SIT_CHANNELS,
};

static CAMEL_SIT_POSE_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -19.9, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, -19.9, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -20.6, 12.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, -20.6, 12.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -20.6, 12.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, -20.6, 12.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, -15.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-90.0, -15.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(1.0, -20.5, 5.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(1.0, -20.5, 5.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 15.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-90.0, 15.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-1.0, -20.5, 5.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(-1.0, -20.5, 5.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(50.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_SIT_POSE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_POSE_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_SIT_POSE_C1,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_POSE_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_SIT_POSE_C3,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_POSE_C4,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_SIT_POSE_C5,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_POSE_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_SIT_POSE_C7,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_POSE_C8,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_SIT_POSE_C9,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_POSE_C10,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_SIT_POSE_C11,
    },
];

pub static CAMEL_SIT_POSE: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.0,
    looping: false,
    channels: CAMEL_SIT_POSE_CHANNELS,
};

static CAMEL_STANDUP_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(-17.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.8, deg(-17.83, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.3, deg(-5.83, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -19.9, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, pos(0.0, -19.9, -3.0), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.0, -12.76, -4.0), Interpolation::CatmullRom),
    Keyframe::new(1.8, pos(0.0, -10.1, -4.0), Interpolation::CatmullRom),
    Keyframe::new(2.3, pos(0.0, -2.9, -2.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-49.06, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, deg(-22.5, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, deg(-25.0, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -20.6, 12.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, -20.6, 8.0), Interpolation::Linear),
    Keyframe::new(1.1, pos(0.0, -7.14, 4.42), Interpolation::Linear),
    Keyframe::new(1.8, pos(0.0, -1.27, -1.33), Interpolation::Linear),
    Keyframe::new(2.3, pos(0.0, -1.27, -0.33), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-49.06, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, deg(-22.5, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, deg(-25.0, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -20.6, 12.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, -20.6, 8.0), Interpolation::Linear),
    Keyframe::new(1.1, pos(0.0, -7.14, 4.42), Interpolation::Linear),
    Keyframe::new(1.8, pos(0.0, -1.27, -1.33), Interpolation::Linear),
    Keyframe::new(2.3, pos(0.0, -1.27, -0.33), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, -15.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3, deg(-90.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(-90.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-60.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.2, deg(30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(1.0, -20.5, 5.0), Interpolation::Linear),
    Keyframe::new(0.3, pos(-2.0, -20.5, 3.0), Interpolation::Linear),
    Keyframe::new(0.6, pos(-2.0, -20.5, 3.0), Interpolation::Linear),
    Keyframe::new(1.1, pos(-2.0, -10.5, 2.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(-2.0, -0.4, -3.9), Interpolation::Linear),
    Keyframe::new(1.9, pos(-2.0, -4.3, -9.8), Interpolation::Linear),
    Keyframe::new(2.2, pos(-1.0, -2.5, -5.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 15.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3, deg(-90.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(-90.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-60.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.2, deg(30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-1.0, -20.5, 5.0), Interpolation::Linear),
    Keyframe::new(0.3, pos(2.0, -20.5, 3.0), Interpolation::Linear),
    Keyframe::new(0.6, pos(2.0, -20.5, 3.0), Interpolation::Linear),
    Keyframe::new(1.1, pos(2.0, -10.5, 2.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(2.0, -0.4, -3.9), Interpolation::Linear),
    Keyframe::new(1.9, pos(2.0, -4.3, -9.8), Interpolation::Linear),
    Keyframe::new(2.2, pos(1.0, -2.5, -5.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.8, deg(55.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(65.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.4, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(50.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, deg(55.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(55.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(17.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(5.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_STANDUP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: CAMEL_STANDUP_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_STANDUP_C1,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_STANDUP_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_STANDUP_C3,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_STANDUP_C4,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_STANDUP_C5,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_STANDUP_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_STANDUP_C7,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_STANDUP_C8,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_STANDUP_C9,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_STANDUP_C10,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_STANDUP_C11,
    },
];

pub static CAMEL_STANDUP: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.6,
    looping: false,
    channels: CAMEL_STANDUP_CHANNELS,
};

static CAMEL_DASH_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(5.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_DASH_C1: &[Keyframe] = &[
    Keyframe::new(0.0, deg(67.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(112.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(67.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(112.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(67.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_DASH_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(10.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(10.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(10.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_DASH_C3: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        deg(44.97272, 1.76749, -1.76833),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.125, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.25,
        deg(44.97272, 1.76749, -1.76833),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.375, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.5,
        deg(44.97272, 1.76749, -1.76833),
        Interpolation::CatmullRom,
    ),
];

static CAMEL_DASH_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.125,
        deg(44.97272, -1.76749, 1.76833),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.25, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.375,
        deg(44.97272, -1.76749, 1.76833),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.5, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_DASH_C5: &[Keyframe] = &[
    Keyframe::new(0.0, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_DASH_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.125, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, deg(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(-45.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_DASH_C7: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, -67.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, -67.5, 0.0), Interpolation::Linear),
];

static CAMEL_DASH_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 67.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 67.5, 0.0), Interpolation::Linear),
];

static CAMEL_DASH_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C0,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C1,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C3,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C4,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C5,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C6,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C7,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_DASH_C8,
    },
];

pub static CAMEL_DASH: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: true,
    channels: CAMEL_DASH_CHANNELS,
};

static CAMEL_IDLE_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(5.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        1.0,
        deg(4.98107, 0.43523, -4.98107),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.0,
        deg(4.9872, -0.29424, 3.36745),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(4.0, deg(5.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_IDLE_C1: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.0, deg(-2.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(4.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static CAMEL_IDLE_C2: &[Keyframe] = &[
    Keyframe::new(2.5, deg(0.0, 0.0, -45.0), Interpolation::CatmullRom),
    Keyframe::new(2.625, deg(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(2.75, deg(0.0, 0.0, -45.0), Interpolation::CatmullRom),
    Keyframe::new(2.875, deg(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(3.0, deg(0.0, 0.0, -45.0), Interpolation::CatmullRom),
];

static CAMEL_IDLE_C3: &[Keyframe] = &[
    Keyframe::new(2.5, deg(0.0, 0.0, 45.0), Interpolation::CatmullRom),
    Keyframe::new(2.625, deg(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(2.75, deg(0.0, 0.0, 45.0), Interpolation::CatmullRom),
    Keyframe::new(2.875, deg(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(3.0, deg(0.0, 0.0, 45.0), Interpolation::CatmullRom),
];

static CAMEL_IDLE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_IDLE_C0,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_IDLE_C1,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_IDLE_C2,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_IDLE_C3,
    },
];

pub static CAMEL_IDLE: AnimationDefinition = AnimationDefinition {
    length_seconds: 4.0,
    looping: false,
    channels: CAMEL_IDLE_CHANNELS,
};
