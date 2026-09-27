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

const fn scale(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x - 1.0, y - 1.0, z - 1.0]
}

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

static CAMEL_BABY_STANDUP_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(-12.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-12.6, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -13.25, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, -13.25, 0.0), Interpolation::Linear),
    Keyframe::new(0.3, pos(0.0, -11.52, -0.85), Interpolation::Linear),
    Keyframe::new(0.9, pos(0.0, -9.4335, -1.6246), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, -9.15, -1.58), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, -8.5, -1.3), Interpolation::Linear),
    Keyframe::new(1.9, pos(0.0, -1.7, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.3, pos(0.0, -0.1, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, deg(-75.21, 9.37, 2.45), Interpolation::Linear),
    Keyframe::new(0.3, deg(-70.21, 9.37, 2.45), Interpolation::Linear),
    Keyframe::new(0.8, deg(-40.55, 8.68, 1.23), Interpolation::Linear),
    Keyframe::new(1.0, deg(-40.55, 8.68, 1.23), Interpolation::Linear),
    Keyframe::new(1.2, deg(-20.45, 7.61, 0.74), Interpolation::Linear),
    Keyframe::new(1.5, deg(-20.45, 7.61, 0.74), Interpolation::Linear),
    Keyframe::new(1.6, deg(-20.45, 7.61, 0.74), Interpolation::Linear),
    Keyframe::new(1.9, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.1, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.1, -11.0, 6.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.1, -8.0, 6.0), Interpolation::Linear),
    Keyframe::new(0.3, pos(-0.12, -7.18, 5.0), Interpolation::Linear),
    Keyframe::new(0.8, pos(-0.9942, -2.3474, 2.2929), Interpolation::Linear),
    Keyframe::new(1.0, pos(-0.9942, -2.3474, 2.2929), Interpolation::Linear),
    Keyframe::new(1.2, pos(-0.72, -0.6, 1.71), Interpolation::Linear),
    Keyframe::new(1.5, pos(-0.72, -0.6, 1.71), Interpolation::Linear),
    Keyframe::new(1.6, pos(-0.72, -0.6, 1.71), Interpolation::Linear),
    Keyframe::new(1.7, pos(-0.21, -0.55, 1.05), Interpolation::Linear),
    Keyframe::new(1.8, pos(-0.21, 0.73, 0.52), Interpolation::Linear),
    Keyframe::new(1.9, pos(0.05, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.1, pos(0.05, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.2, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, deg(-74.2, -9.3, -2.45), Interpolation::Linear),
    Keyframe::new(0.3, deg(-69.2, -9.32, -2.4), Interpolation::Linear),
    Keyframe::new(0.8, deg(-46.75, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-46.75, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(-24.31, -9.2, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-24.31, -9.2, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, deg(-24.31, -9.2, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(-12.15, -4.6, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-12.15, -4.6, 0.0), Interpolation::Linear),
    Keyframe::new(2.2, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -11.0, 6.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, -7.89, 4.96), Interpolation::Linear),
    Keyframe::new(0.3, pos(0.0, -6.89, 4.96), Interpolation::Linear),
    Keyframe::new(0.8, pos(0.65, -2.98, 3.56), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.65, -2.98, 3.56), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.5, -0.69, 2.47), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.5, -0.69, 2.47), Interpolation::Linear),
    Keyframe::new(1.7, pos(0.5, -0.69, 2.47), Interpolation::Linear),
    Keyframe::new(1.9, pos(0.4, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.1, pos(0.2, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.2, pos(-0.025, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, pos(-0.02, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, -12.0, -0.5), Interpolation::Linear),
    Keyframe::new(0.2, deg(-90.0, -12.0, -0.5), Interpolation::Linear),
    Keyframe::new(0.5, deg(-89.79, -17.4, -0.8), Interpolation::Linear),
    Keyframe::new(0.8, deg(-89.38, -11.59, -0.99), Interpolation::Linear),
    Keyframe::new(0.9, deg(-89.09, -7.49, -0.98), Interpolation::Linear),
    Keyframe::new(1.0, deg(-88.5, -4.3, -1.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(-79.5, -4.3, -1.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(-66.11, -4.19, -1.05), Interpolation::Linear),
    Keyframe::new(1.5, deg(-47.51, -4.19, -1.05), Interpolation::Linear),
    Keyframe::new(1.7, deg(-35.11, -4.19, -1.05), Interpolation::Linear),
    Keyframe::new(1.8, deg(-20.63, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.1, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(1.5, -10.949, 1.9507), Interpolation::Linear),
    Keyframe::new(0.2, pos(1.5, -10.949, 1.9507), Interpolation::Linear),
    Keyframe::new(0.5, pos(1.0, -11.0, 1.75), Interpolation::Linear),
    Keyframe::new(0.8, pos(0.16, -11.11, 0.49), Interpolation::Linear),
    Keyframe::new(0.9, pos(-0.81, -10.95, -0.02), Interpolation::Linear),
    Keyframe::new(1.0, pos(-0.77, -10.8, -0.94), Interpolation::Linear),
    Keyframe::new(1.2, pos(-0.77, -8.7, -0.94), Interpolation::Linear),
    Keyframe::new(1.3, pos(-1.6, -6.43, -1.59), Interpolation::Linear),
    Keyframe::new(1.5, pos(-1.36, -3.37, -1.65), Interpolation::Linear),
    Keyframe::new(1.7, pos(-1.2, -1.33, -1.69), Interpolation::Linear),
    Keyframe::new(1.8, pos(-0.6, 2.0, -0.3), Interpolation::Linear),
    Keyframe::new(1.9, pos(-0.2, 1.9, -1.0), Interpolation::Linear),
    Keyframe::new(2.1, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-90.0, 12.0, 0.5), Interpolation::Linear),
    Keyframe::new(0.2, deg(-90.0, 12.0, 0.5), Interpolation::Linear),
    Keyframe::new(0.5, deg(-90.0, 17.5, 1.0), Interpolation::Linear),
    Keyframe::new(0.8, deg(-90.0, 8.5, 0.73), Interpolation::Linear),
    Keyframe::new(0.9, deg(-90.0, 3.0, 0.37), Interpolation::Linear),
    Keyframe::new(1.0, deg(-75.38, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(-56.38, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-30.15, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.6, deg(3.6, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.1, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-1.3, -11.0, 1.95), Interpolation::Linear),
    Keyframe::new(0.2, pos(-1.3, -11.0, 1.95), Interpolation::Linear),
    Keyframe::new(0.5, pos(-0.7, -10.9, 1.8), Interpolation::Linear),
    Keyframe::new(0.8, pos(-0.06, -11.2, 0.21), Interpolation::Linear),
    Keyframe::new(0.9, pos(0.39, -11.2, -0.23), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.24, -8.0, -1.27), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.34, -4.53, -0.8), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.3, -1.88, -0.42), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.25, 0.18, 0.3), Interpolation::Linear),
    Keyframe::new(1.6, pos(0.25, 1.48, -1.75), Interpolation::Linear),
    Keyframe::new(1.7, pos(0.1, 0.0, -0.4), Interpolation::Linear),
    Keyframe::new(1.9, pos(0.1, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.1, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-4.75, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, deg(5.65, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(46.68, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.6, deg(37.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C11: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.0, 0.8368, 1.9553), Interpolation::Linear),
    Keyframe::new(1.8, pos(0.0, -0.95, 1.5), Interpolation::Linear),
    Keyframe::new(2.3, pos(0.0, -0.95, 1.42), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, -0.95, 1.42), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(46.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, deg(46.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3, deg(39.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(40.75, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(47.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.8, deg(49.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(52.75, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(50.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(42.75, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(1.75, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(7.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, deg(6.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(5.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C13: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.8, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.3, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C14: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 15.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 15.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(0.0, 0.0, 15.0), Interpolation::Linear),
    Keyframe::new(1.6, deg(0.0, 0.0, 5.0), Interpolation::Linear),
    Keyframe::new(1.9, deg(0.0, 0.0, -23.75), Interpolation::Linear),
    Keyframe::new(2.2, deg(0.0, 0.0, -23.75), Interpolation::Linear),
    Keyframe::new(2.3, deg(0.0, 0.0, 12.5), Interpolation::Linear),
    Keyframe::new(2.4, deg(0.0, 0.0, 67.5), Interpolation::Linear),
    Keyframe::new(2.5, deg(0.0, 0.0, 45.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, 45.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C15: &[Keyframe] = &[
    Keyframe::new(2.3, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.4, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C16: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, -22.5), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, -22.5), Interpolation::Linear),
    Keyframe::new(1.4, deg(0.0, 0.0, -22.5), Interpolation::Linear),
    Keyframe::new(1.6, deg(0.0, 0.0, -7.5), Interpolation::Linear),
    Keyframe::new(1.9, deg(0.0, 0.0, 22.5), Interpolation::Linear),
    Keyframe::new(2.2, deg(0.0, 0.0, 22.5), Interpolation::Linear),
    Keyframe::new(2.3, deg(0.0, 0.0, -22.5), Interpolation::Linear),
    Keyframe::new(2.4, deg(0.0, 0.0, -55.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(0.0, 0.0, -45.0), Interpolation::Linear),
    Keyframe::new(2.6, deg(0.0, 0.0, -45.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_C17: &[Keyframe] = &[
    Keyframe::new(2.3, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.4, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_STANDUP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C1,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C3,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C4,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C5,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C7,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C8,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C9,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C10,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C11,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C12,
    },
    Channel {
        bone: "tail",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C13,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C14,
    },
    Channel {
        bone: "right_ear",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C15,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_STANDUP_C16,
    },
    Channel {
        bone: "left_ear",
        target: Target::Position,
        keyframes: CAMEL_BABY_STANDUP_C17,
    },
];

pub static CAMEL_BABY_STANDUP: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.6,
    looping: true,
    channels: CAMEL_BABY_STANDUP_CHANNELS,
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

static CAMEL_BABY_SIT_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, deg(9.26, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, deg(18.15, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(30.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.2, deg(21.14, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.6, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(2.5, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -0.1, 0.0), Interpolation::Linear),
    Keyframe::new(0.2, pos(0.0, -1.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, -2.21, 0.04), Interpolation::Linear),
    Keyframe::new(0.6, pos(0.0, -2.21, 0.04), Interpolation::Linear),
    Keyframe::new(1.1, pos(0.0, -5.23, 0.33), Interpolation::Linear),
    Keyframe::new(1.2, pos(0.0, -6.06, 0.17), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.0, -11.7, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.0, -13.35, 0.0), Interpolation::Linear),
    Keyframe::new(1.6, pos(0.0, -13.25, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, pos(0.0, -13.25, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, -13.25, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(-23.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-44.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(-73.25, 6.5, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-90.0, 10.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.125, -0.1, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.125, -0.1, 0.4), Interpolation::Linear),
    Keyframe::new(0.6, pos(0.12, -0.15, 2.5), Interpolation::Linear),
    Keyframe::new(0.7, pos(0.12, -0.6, 4.6), Interpolation::Linear),
    Keyframe::new(0.9, pos(0.12, -0.87, 5.97), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.125, -1.0, 6.9), Interpolation::Linear),
    Keyframe::new(1.1, pos(0.325, -2.6, 7.6), Interpolation::Linear),
    Keyframe::new(1.3, pos(-0.28, -7.45, 8.06), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.1, -11.0, 6.0), Interpolation::Linear),
    Keyframe::new(1.7, pos(0.1, -11.0, 6.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.1, -11.0, 6.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(-2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7, deg(-23.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-45.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(-73.5, -6.5, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-90.0, -10.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-0.125, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(-0.125, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.6, pos(-0.13, -0.18, 2.35), Interpolation::Linear),
    Keyframe::new(0.7, pos(-0.13, -0.57, 4.7), Interpolation::Linear),
    Keyframe::new(0.9, pos(-0.13, -0.86, 6.07), Interpolation::Linear),
    Keyframe::new(1.0, pos(-0.125, -1.0, 6.8), Interpolation::Linear),
    Keyframe::new(1.1, pos(-0.22, -2.7, 7.8), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.19, -7.5, 8.01), Interpolation::Linear),
    Keyframe::new(1.4, pos(0.0, -11.0, 6.0), Interpolation::Linear),
    Keyframe::new(1.7, pos(0.0, -10.9, 6.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, -11.0, 6.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(-10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-31.47, -0.45, 1.29), Interpolation::Linear),
    Keyframe::new(1.2, deg(-49.78, -10.29, -1.15), Interpolation::Linear),
    Keyframe::new(1.3, deg(-79.21, -13.72, -1.53), Interpolation::Linear),
    Keyframe::new(1.4, deg(-90.0, -11.88, -0.54), Interpolation::Linear),
    Keyframe::new(2.0, deg(-90.0, -11.88, -0.54), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-0.1, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(-0.1, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, pos(-0.1, 0.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.1, pos(0.56, -3.49, 2.18), Interpolation::Linear),
    Keyframe::new(1.3, pos(1.22, -8.72, 3.0), Interpolation::Linear),
    Keyframe::new(1.4, pos(1.4, -10.95, 1.95), Interpolation::Linear),
    Keyframe::new(2.0, pos(1.4, -10.95, 1.95), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(-10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-31.13, 0.56, -1.3), Interpolation::Linear),
    Keyframe::new(1.2, deg(-49.8, 10.3, 1.15), Interpolation::Linear),
    Keyframe::new(1.3, deg(-79.21, 13.72, 1.53), Interpolation::Linear),
    Keyframe::new(1.4, deg(-90.0, 11.89, 0.54), Interpolation::Linear),
    Keyframe::new(2.0, deg(-90.0, 11.89, 0.54), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.1, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos(0.1, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, pos(0.1, 0.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.1, pos(0.2, -0.75, 1.5), Interpolation::Linear),
    Keyframe::new(1.2, pos(-0.51, -3.49, 2.18), Interpolation::Linear),
    Keyframe::new(1.3, pos(-1.22, -8.72, 3.1), Interpolation::Linear),
    Keyframe::new(1.4, pos(-1.33, -11.0, 1.95), Interpolation::Linear),
    Keyframe::new(2.0, pos(-1.33, -11.0, 1.95), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, deg(-20.42, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(-31.25, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1, deg(-26.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(-19.25, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(-14.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(-4.75, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.6, deg(-4.75, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-4.75, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C11: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4, pos(0.0, 0.38, 0.44), Interpolation::Linear),
    Keyframe::new(0.9, pos(0.0, 0.35, 0.88), Interpolation::Linear),
    Keyframe::new(1.1, pos(0.0, -0.3, 1.21), Interpolation::Linear),
    Keyframe::new(1.3, pos(0.0, -0.3, 1.22), Interpolation::Linear),
    Keyframe::new(1.6, pos(0.0, 0.0, 1.22), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(52.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.6, deg(62.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, deg(52.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(52.5, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C13: &[Keyframe] = &[
    Keyframe::new(1.4, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.8, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C14: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, -45.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(0.0, 0.0, -45.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(0.0, 0.0, -62.5), Interpolation::Linear),
    Keyframe::new(1.4, deg(0.0, 0.0, -45.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 20.0), Interpolation::Linear),
    Keyframe::new(1.6, deg(0.0, 0.0, -22.5), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, -22.5), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C15: &[Keyframe] = &[Keyframe::new(
    1.2,
    pos(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_C16: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 45.0), Interpolation::Linear),
    Keyframe::new(1.2, deg(0.0, 0.0, 45.0), Interpolation::Linear),
    Keyframe::new(1.3, deg(0.0, 0.0, 60.0), Interpolation::Linear),
    Keyframe::new(1.4, deg(0.0, 0.0, 45.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, -20.0), Interpolation::Linear),
    Keyframe::new(1.6, deg(0.0, 0.0, 15.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 15.0), Interpolation::Linear),
];

static CAMEL_BABY_SIT_C17: &[Keyframe] = &[Keyframe::new(
    1.2,
    pos(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static CAMEL_BABY_SIT_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C1,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C2,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C3,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C4,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C5,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C6,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C7,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C8,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C9,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C10,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C11,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C12,
    },
    Channel {
        bone: "tail",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C13,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C14,
    },
    Channel {
        bone: "left_ear",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C15,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: CAMEL_BABY_SIT_C16,
    },
    Channel {
        bone: "right_ear",
        target: Target::Position,
        keyframes: CAMEL_BABY_SIT_C17,
    },
];

pub static CAMEL_BABY_SIT: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.0,
    looping: true,
    channels: CAMEL_BABY_SIT_CHANNELS,
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
