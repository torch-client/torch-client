#![allow(dead_code)]
#![allow(clippy::approx_constant, clippy::eq_op, clippy::excessive_precision)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, DEG_TO_RAD, Interpolation, Keyframe, Target,
};

static IDLE_0_WIND_TOP_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.5, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.5, 0.0, -0.5], Interpolation::Linear),
    Keyframe::new(0.75, [-0.5, 0.0, -0.5], Interpolation::Linear),
    Keyframe::new(1.25, [-0.5, 0.0, 0.5], Interpolation::Linear),
    Keyframe::new(1.75, [0.5, 0.0, 0.5], Interpolation::Linear),
    Keyframe::new(2.0, [0.5, 0.0, 0.0], Interpolation::Linear),
];

static IDLE_1_WIND_MID_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.5, 0.0, -0.5], Interpolation::Linear),
    Keyframe::new(0.5, [-0.5, 0.0, -0.5], Interpolation::Linear),
    Keyframe::new(1.0, [-0.5, 0.0, 0.5], Interpolation::Linear),
    Keyframe::new(1.5, [0.5, 0.0, 0.5], Interpolation::Linear),
    Keyframe::new(2.0, [0.5, 0.0, -0.5], Interpolation::Linear),
];

static IDLE_2_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.0, [0.0, -1.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static IDLE_3_RODS_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0,
        [0.0 * DEG_TO_RAD, 1080.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static IDLE_4_RODS_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.0, [0.0, 1.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static IDLE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "wind_top",
        target: Target::Position,
        keyframes: IDLE_0_WIND_TOP_POSITION,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Position,
        keyframes: IDLE_1_WIND_MID_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: IDLE_2_HEAD_POSITION,
    },
    Channel {
        bone: "rods",
        target: Target::Rotation,
        keyframes: IDLE_3_RODS_ROTATION,
    },
    Channel {
        bone: "rods",
        target: Target::Position,
        keyframes: IDLE_4_RODS_POSITION,
    },
];

pub static IDLE: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.0,
    looping: true,
    channels: IDLE_CHANNELS,
};

static SHOOT_0_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [-12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [-12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static SHOOT_1_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 2.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.7917, [0.0, 1.0, 2.0], Interpolation::Linear),
    Keyframe::new(0.9583, [0.0, 1.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.125, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static SHOOT_2_WIND_BOTTOM_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
    Interpolation::Linear,
)];

static SHOOT_3_WIND_MID_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static SHOOT_4_WIND_MID_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 5.0], Interpolation::Linear),
    Keyframe::new(0.75, [0.0, 0.0, 6.0], Interpolation::Linear),
    Keyframe::new(0.9167, [0.0, 0.0, -2.0], Interpolation::Linear),
    Keyframe::new(1.125, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static SHOOT_5_WIND_TOP_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static SHOOT_6_WIND_TOP_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 3.0], Interpolation::Linear),
    Keyframe::new(0.8333, [0.0, 0.0, 4.0], Interpolation::Linear),
    Keyframe::new(0.9583, [0.0, 0.0, -2.0], Interpolation::Linear),
    Keyframe::new(1.125, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static SHOOT_7_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [-2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static SHOOT_8_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, -3.0, 5.0], Interpolation::Linear),
    Keyframe::new(0.8333, [0.0, -3.0, 6.0], Interpolation::Linear),
    Keyframe::new(0.9583, [0.0, -3.0, -1.0], Interpolation::Linear),
    Keyframe::new(1.125, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static SHOOT_9_RODS_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0,
        [0.0 * DEG_TO_RAD, 360.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static SHOOT_CHANNELS: &[Channel] = &[
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: SHOOT_0_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: SHOOT_1_HEAD_POSITION,
    },
    Channel {
        bone: "wind_bottom",
        target: Target::Rotation,
        keyframes: SHOOT_2_WIND_BOTTOM_ROTATION,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Rotation,
        keyframes: SHOOT_3_WIND_MID_ROTATION,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Position,
        keyframes: SHOOT_4_WIND_MID_POSITION,
    },
    Channel {
        bone: "wind_top",
        target: Target::Rotation,
        keyframes: SHOOT_5_WIND_TOP_ROTATION,
    },
    Channel {
        bone: "wind_top",
        target: Target::Position,
        keyframes: SHOOT_6_WIND_TOP_POSITION,
    },
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: SHOOT_7_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: SHOOT_8_BODY_POSITION,
    },
    Channel {
        bone: "rods",
        target: Target::Rotation,
        keyframes: SHOOT_9_RODS_ROTATION,
    },
];

pub static SHOOT: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.125,
    looping: false,
    channels: SHOOT_CHANNELS,
};

static JUMP_0_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 10.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, -11.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static JUMP_1_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-19.25 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static JUMP_2_WIND_BODY_SCALE: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [1.0 - 1.0, 1.2999999523162842 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
];

static JUMP_3_WIND_BOTTOM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 90.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 360.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static JUMP_4_WIND_BOTTOM_SCALE: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [1.0 - 1.0, 1.100000023841858 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
];

static JUMP_5_WIND_MID_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 180.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static JUMP_6_WIND_MID_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 6.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, -2.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static JUMP_7_WIND_TOP_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 90.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static JUMP_8_WIND_TOP_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 5.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, -2.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static JUMP_9_RODS_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 360.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static JUMP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: JUMP_0_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: JUMP_1_HEAD_ROTATION,
    },
    Channel {
        bone: "wind_body",
        target: Target::Scale,
        keyframes: JUMP_2_WIND_BODY_SCALE,
    },
    Channel {
        bone: "wind_bottom",
        target: Target::Rotation,
        keyframes: JUMP_3_WIND_BOTTOM_ROTATION,
    },
    Channel {
        bone: "wind_bottom",
        target: Target::Scale,
        keyframes: JUMP_4_WIND_BOTTOM_SCALE,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Rotation,
        keyframes: JUMP_5_WIND_MID_ROTATION,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Position,
        keyframes: JUMP_6_WIND_MID_POSITION,
    },
    Channel {
        bone: "wind_top",
        target: Target::Rotation,
        keyframes: JUMP_7_WIND_TOP_ROTATION,
    },
    Channel {
        bone: "wind_top",
        target: Target::Position,
        keyframes: JUMP_8_WIND_TOP_POSITION,
    },
    Channel {
        bone: "rods",
        target: Target::Rotation,
        keyframes: JUMP_9_RODS_ROTATION,
    },
];

pub static JUMP: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: false,
    channels: JUMP_CHANNELS,
};

static INHALE_0_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5, [0.0, 10.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.625, [0.0, 10.0, 0.0], Interpolation::Linear),
];

static INHALE_1_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static INHALE_2_WIND_BODY_SCALE: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
];

static INHALE_3_WIND_BOTTOM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 90.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static INHALE_4_WIND_BOTTOM_SCALE: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
];

static INHALE_5_WIND_MID_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static INHALE_6_WIND_MID_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5, [0.0, 6.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.625, [0.0, 6.0, 0.0], Interpolation::Linear),
];

static INHALE_7_WIND_TOP_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static INHALE_8_WIND_TOP_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5, [0.0, 5.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.625, [0.0, 5.0, 0.0], Interpolation::Linear),
];

static INHALE_9_RODS_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 360.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static INHALE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: INHALE_0_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: INHALE_1_HEAD_ROTATION,
    },
    Channel {
        bone: "wind_body",
        target: Target::Scale,
        keyframes: INHALE_2_WIND_BODY_SCALE,
    },
    Channel {
        bone: "wind_bottom",
        target: Target::Rotation,
        keyframes: INHALE_3_WIND_BOTTOM_ROTATION,
    },
    Channel {
        bone: "wind_bottom",
        target: Target::Scale,
        keyframes: INHALE_4_WIND_BOTTOM_SCALE,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Rotation,
        keyframes: INHALE_5_WIND_MID_ROTATION,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Position,
        keyframes: INHALE_6_WIND_MID_POSITION,
    },
    Channel {
        bone: "wind_top",
        target: Target::Rotation,
        keyframes: INHALE_7_WIND_TOP_ROTATION,
    },
    Channel {
        bone: "wind_top",
        target: Target::Position,
        keyframes: INHALE_8_WIND_TOP_POSITION,
    },
    Channel {
        bone: "rods",
        target: Target::Rotation,
        keyframes: INHALE_9_RODS_ROTATION,
    },
];

pub static INHALE: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.0,
    looping: false,
    channels: INHALE_CHANNELS,
};

static SLIDE_0_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2, [0.0, 0.0, -6.0], Interpolation::Linear),
];

static SLIDE_1_WIND_MID_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2, [0.0, 0.0, -3.0], Interpolation::Linear),
];

static SLIDE_2_WIND_TOP_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2, [0.0, 0.0, -2.0], Interpolation::Linear),
];

static SLIDE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: SLIDE_0_BODY_POSITION,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Position,
        keyframes: SLIDE_1_WIND_MID_POSITION,
    },
    Channel {
        bone: "wind_top",
        target: Target::Position,
        keyframes: SLIDE_2_WIND_TOP_POSITION,
    },
];

pub static SLIDE: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.2,
    looping: false,
    channels: SLIDE_CHANNELS,
};

static SLIDE_BACK_0_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, -6.0], Interpolation::Linear),
    Keyframe::new(0.1, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static SLIDE_BACK_1_WIND_MID_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, -3.0], Interpolation::Linear),
    Keyframe::new(0.1, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static SLIDE_BACK_2_WIND_TOP_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, -2.0], Interpolation::Linear),
    Keyframe::new(0.1, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static SLIDE_BACK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: SLIDE_BACK_0_BODY_POSITION,
    },
    Channel {
        bone: "wind_mid",
        target: Target::Position,
        keyframes: SLIDE_BACK_1_WIND_MID_POSITION,
    },
    Channel {
        bone: "wind_top",
        target: Target::Position,
        keyframes: SLIDE_BACK_2_WIND_TOP_POSITION,
    },
];

pub static SLIDE_BACK: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.1,
    looping: false,
    channels: SLIDE_BACK_CHANNELS,
};
