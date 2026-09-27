#![allow(dead_code)]
#![allow(clippy::approx_constant, clippy::eq_op, clippy::excessive_precision)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, DEG_TO_RAD, Interpolation, Keyframe, Target,
};

static CREAKING_WALK_0_UPPER_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [
            26.8802 * DEG_TO_RAD,
            -23.399 * DEG_TO_RAD,
            -9.0616 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [
            -2.2093 * DEG_TO_RAD,
            5.9119 * DEG_TO_RAD,
            0.0675 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            23.0778 * DEG_TO_RAD,
            14.2906 * DEG_TO_RAD,
            4.6066 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.875,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [
            26.8802 * DEG_TO_RAD,
            -23.399 * DEG_TO_RAD,
            -9.0616 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
];

static CREAKING_WALK_1_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0417,
        [-17.5 * DEG_TO_RAD, -62.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0833,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4583,
        [0.0 * DEG_TO_RAD, 15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0833,
        [
            -37.1532 * DEG_TO_RAD,
            81.1131 * DEG_TO_RAD,
            -28.3621 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_WALK_2_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [-32.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.875,
        [12.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_WALK_3_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [-25.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [-9.0923 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7917,
        [
            -15.137 * DEG_TO_RAD,
            -66.7758 * DEG_TO_RAD,
            13.9603 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [-9.0923 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_WALK_4_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [
            49.8924 * DEG_TO_RAD,
            -3.8282 * DEG_TO_RAD,
            3.2187 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [17.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [
            -56.5613 * DEG_TO_RAD,
            -12.2403 * DEG_TO_RAD,
            -8.7374 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_WALK_5_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 2.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, -0.1846, 0.5979], Interpolation::Linear),
    Keyframe::new(0.375, [0.0, 0.0665, -2.2177], Interpolation::Linear),
    Keyframe::new(0.5, [0.0, -1.3563, -4.3474], Interpolation::Linear),
    Keyframe::new(0.625, [0.0, -0.1047, -1.6556], Interpolation::Linear),
    Keyframe::new(0.9167, [0.0, 0.0, -1.0], Interpolation::Linear),
    Keyframe::new(1.125, [0.0, 0.0, 2.0], Interpolation::Linear),
];

static CREAKING_WALK_6_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [
            25.5305 * DEG_TO_RAD,
            11.3125 * DEG_TO_RAD,
            5.3525 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [
            -49.5628 * DEG_TO_RAD,
            7.3556 * DEG_TO_RAD,
            6.7933 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [55.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [
            25.5305 * DEG_TO_RAD,
            11.3125 * DEG_TO_RAD,
            5.3525 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
];

static CREAKING_WALK_7_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, -0.9674, -3.6578], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.2979, -0.9411], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.3, -0.94], Interpolation::Linear),
    Keyframe::new(0.4583, [0.0, 0.3, 1.06], Interpolation::Linear),
    Keyframe::new(1.125, [0.0, -0.9674, -3.6578], Interpolation::Linear),
];

static CREAKING_WALK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "upper_body",
        target: Target::Rotation,
        keyframes: CREAKING_WALK_0_UPPER_BODY_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CREAKING_WALK_1_HEAD_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: CREAKING_WALK_2_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: CREAKING_WALK_3_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: CREAKING_WALK_4_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: CREAKING_WALK_5_LEFT_LEG_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: CREAKING_WALK_6_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: CREAKING_WALK_7_RIGHT_LEG_POSITION,
    },
];

pub static CREAKING_WALK: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.125,
    looping: true,
    channels: CREAKING_WALK_CHANNELS,
};

static CREAKING_ATTACK_0_UPPER_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0833,
        [0.0 * DEG_TO_RAD, 45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [-115.0 * DEG_TO_RAD, 67.5 * DEG_TO_RAD, -90.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [0.0 * DEG_TO_RAD, 45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_ATTACK_1_UPPER_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.0833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 2.7716, -1.1481], Interpolation::Linear),
    Keyframe::new(0.375, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.7083, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_ATTACK_2_UPPER_BODY_SCALE: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
];

static CREAKING_ATTACK_3_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [0.0 * DEG_TO_RAD, -45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [-11.25 * DEG_TO_RAD, -45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -117.3939 * DEG_TO_RAD,
            76.6331 * DEG_TO_RAD,
            -130.1483 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [-45.0 * DEG_TO_RAD, -45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [60.0 * DEG_TO_RAD, -45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [60.0 * DEG_TO_RAD, -45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, -45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_ATTACK_4_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5, [0.3827, -0.5133, -0.7682], Interpolation::Linear),
    Keyframe::new(0.5833, [0.3827, -0.5133, -0.7682], Interpolation::Linear),
    Keyframe::new(0.625, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.7083, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_ATTACK_5_HEAD_SCALE: &[Keyframe] = &[
    Keyframe::new(
        0.1667,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [1.0 - 1.0, 1.2999999523162842 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
];

static CREAKING_ATTACK_6_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4583,
        [55.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_ATTACK_7_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.625, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.7083, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_ATTACK_8_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_ATTACK_9_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.1667, [0.0, 0.0, -2.0], Interpolation::Linear),
    Keyframe::new(0.625, [0.0, 0.0, -2.0], Interpolation::Linear),
    Keyframe::new(0.7083, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_ATTACK_10_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [0.0 * DEG_TO_RAD, 45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_ATTACK_11_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.1667, [0.7071, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.625, [0.7071, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.7083, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_ATTACK_12_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [
            10.3453 * DEG_TO_RAD,
            14.7669 * DEG_TO_RAD,
            2.664 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4583,
        [57.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_ATTACK_13_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.7083, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_ATTACK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "upper_body",
        target: Target::Rotation,
        keyframes: CREAKING_ATTACK_0_UPPER_BODY_ROTATION,
    },
    Channel {
        bone: "upper_body",
        target: Target::Position,
        keyframes: CREAKING_ATTACK_1_UPPER_BODY_POSITION,
    },
    Channel {
        bone: "upper_body",
        target: Target::Scale,
        keyframes: CREAKING_ATTACK_2_UPPER_BODY_SCALE,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CREAKING_ATTACK_3_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: CREAKING_ATTACK_4_HEAD_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Scale,
        keyframes: CREAKING_ATTACK_5_HEAD_SCALE,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: CREAKING_ATTACK_6_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: CREAKING_ATTACK_7_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: CREAKING_ATTACK_8_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: CREAKING_ATTACK_9_LEFT_LEG_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: CREAKING_ATTACK_10_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: CREAKING_ATTACK_11_RIGHT_LEG_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: CREAKING_ATTACK_12_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: CREAKING_ATTACK_13_LEFT_ARM_POSITION,
    },
];

pub static CREAKING_ATTACK: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.7083,
    looping: true,
    channels: CREAKING_ATTACK_CHANNELS,
};

static CREAKING_INVULNERABLE_0_UPPER_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0833,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_INVULNERABLE_1_UPPER_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.0833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_INVULNERABLE_2_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0833,
        [17.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_INVULNERABLE_3_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_INVULNERABLE_4_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0833,
        [20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_INVULNERABLE_5_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_INVULNERABLE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "upper_body",
        target: Target::Rotation,
        keyframes: CREAKING_INVULNERABLE_0_UPPER_BODY_ROTATION,
    },
    Channel {
        bone: "upper_body",
        target: Target::Position,
        keyframes: CREAKING_INVULNERABLE_1_UPPER_BODY_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: CREAKING_INVULNERABLE_2_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: CREAKING_INVULNERABLE_3_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: CREAKING_INVULNERABLE_4_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: CREAKING_INVULNERABLE_5_LEFT_ARM_POSITION,
    },
];

pub static CREAKING_INVULNERABLE: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.2917,
    looping: false,
    channels: CREAKING_INVULNERABLE_CHANNELS,
};

static CREAKING_DEATH_0_UPPER_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0833,
        [-40.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [16.25 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.6667,
        [
            29.0814 * DEG_TO_RAD,
            62.5516 * DEG_TO_RAD,
            26.5771 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [12.2115 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0,
        [10.25 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [-47.64 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [21.96 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [
            17.3266 * DEG_TO_RAD,
            7.9022 * DEG_TO_RAD,
            -0.1381 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
];

static CREAKING_DEATH_1_UPPER_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.0833, [0.0, -0.557, 1.2659], Interpolation::Linear),
    Keyframe::new(0.1667, [0.0, 2.0889, -0.3493], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_DEATH_2_UPPER_BODY_SCALE: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0833,
        [1.0 - 1.0, 1.100000023841858 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [1.0 - 1.0, 0.8999999761581421 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
];

static CREAKING_DEATH_3_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5417,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [
            -12.1479 * DEG_TO_RAD,
            -34.3927 * DEG_TO_RAD,
            6.9326 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_DEATH_4_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_DEATH_5_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [-4.4444 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.875,
        [
            -26.7402 * DEG_TO_RAD,
            -78.831 * DEG_TO_RAD,
            26.3025 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9583,
        [-5.5556 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_DEATH_6_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_DEATH_7_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.0833,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [5.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [
            -67.4168 * DEG_TO_RAD,
            -12.9552 * DEG_TO_RAD,
            -8.0231 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.6667,
        [8.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0,
        [
            10.773 * DEG_TO_RAD,
            -29.5608 * DEG_TO_RAD,
            -5.3627 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            12.9625 * DEG_TO_RAD,
            39.2735 * DEG_TO_RAD,
            8.2901 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.9167,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static CREAKING_DEATH_8_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static CREAKING_DEATH_CHANNELS: &[Channel] = &[
    Channel {
        bone: "upper_body",
        target: Target::Rotation,
        keyframes: CREAKING_DEATH_0_UPPER_BODY_ROTATION,
    },
    Channel {
        bone: "upper_body",
        target: Target::Position,
        keyframes: CREAKING_DEATH_1_UPPER_BODY_POSITION,
    },
    Channel {
        bone: "upper_body",
        target: Target::Scale,
        keyframes: CREAKING_DEATH_2_UPPER_BODY_SCALE,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: CREAKING_DEATH_3_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: CREAKING_DEATH_4_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: CREAKING_DEATH_5_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: CREAKING_DEATH_6_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: CREAKING_DEATH_7_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: CREAKING_DEATH_8_HEAD_POSITION,
    },
];

pub static CREAKING_DEATH: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.25,
    looping: false,
    channels: CREAKING_DEATH_CHANNELS,
};
