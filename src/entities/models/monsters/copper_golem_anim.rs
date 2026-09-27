#![allow(dead_code)]
#![allow(clippy::approx_constant, clippy::eq_op, clippy::excessive_precision)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, DEG_TO_RAD, Interpolation, Keyframe, Target,
};

static COPPER_GOLEM_WALK_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [10.0 * DEG_TO_RAD, 15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.2083,
        [10.0 * DEG_TO_RAD, -1.87 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [10.0 * DEG_TO_RAD, -15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.625,
        [10.0 * DEG_TO_RAD, -0.82 * DEG_TO_RAD, 10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [10.0 * DEG_TO_RAD, 15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_1_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.2083,
        [-10.0 * DEG_TO_RAD, 1.87 * DEG_TO_RAD, 10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.625,
        [-10.0 * DEG_TO_RAD, 0.82 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_2_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [70.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [-80.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [70.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_3_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [-80.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [70.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [-80.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_4_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [-60.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [60.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [-60.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_5_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [60.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [-60.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [60.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_0_BODY_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_1_HEAD_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_2_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_3_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_4_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_5_LEFT_LEG_ROTATION,
    },
];

pub static COPPER_GOLEM_WALK: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.8333,
    looping: true,
    channels: COPPER_GOLEM_WALK_CHANNELS,
};

static COPPER_GOLEM_IDLE_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, -35.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, -35.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 35.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [0.0 * DEG_TO_RAD, 35.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [0.0 * DEG_TO_RAD, 35.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_IDLE_1_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5,
        [0.0 * DEG_TO_RAD, 300.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [0.0 * DEG_TO_RAD, 300.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.75,
        [-25.0 * DEG_TO_RAD, 300.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [-25.0 * DEG_TO_RAD, 300.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 360.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.5,
        [0.0 * DEG_TO_RAD, 360.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_IDLE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_IDLE_0_BODY_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_IDLE_1_HEAD_ROTATION,
    },
];

pub static COPPER_GOLEM_IDLE: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.5,
    looping: false,
    channels: COPPER_GOLEM_IDLE_CHANNELS,
};

static COPPER_GOLEM_WALK_ITEM_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [10.0 * DEG_TO_RAD, 7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.2083,
        [10.0 * DEG_TO_RAD, -1.87 * DEG_TO_RAD, -5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [10.0 * DEG_TO_RAD, -7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.625,
        [10.0 * DEG_TO_RAD, -0.82 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [10.0 * DEG_TO_RAD, 7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_ITEM_1_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.2083,
        [-10.0 * DEG_TO_RAD, 1.87 * DEG_TO_RAD, 10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.625,
        [-10.0 * DEG_TO_RAD, 0.82 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_ITEM_2_RIGHT_ARM_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    [
        -59.78638 * DEG_TO_RAD,
        -6.49053 * DEG_TO_RAD,
        -3.76613 * DEG_TO_RAD,
    ],
    Interpolation::Linear,
)];

static COPPER_GOLEM_WALK_ITEM_3_LEFT_ARM_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    [
        -59.78638 * DEG_TO_RAD,
        6.49053 * DEG_TO_RAD,
        3.76613 * DEG_TO_RAD,
    ],
    Interpolation::Linear,
)];

static COPPER_GOLEM_WALK_ITEM_4_LEFT_ARM_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    [-0.21129, 0.0212, -0.07004],
    Interpolation::Linear,
)];

static COPPER_GOLEM_WALK_ITEM_5_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [-30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [-30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_ITEM_6_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.4167,
        [-30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.8333,
        [30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static COPPER_GOLEM_WALK_ITEM_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_ITEM_0_BODY_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_ITEM_1_HEAD_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_ITEM_2_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_ITEM_3_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_WALK_ITEM_4_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_ITEM_5_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_WALK_ITEM_6_LEFT_LEG_ROTATION,
    },
];

pub static COPPER_GOLEM_WALK_ITEM: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.8333,
    looping: true,
    channels: COPPER_GOLEM_WALK_ITEM_CHANNELS,
};

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [18.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [24.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [
            12.40525 * DEG_TO_RAD,
            -0.00044 * DEG_TO_RAD,
            0.00829 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [
            12.40525 * DEG_TO_RAD,
            -0.00044 * DEG_TO_RAD,
            0.00829 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [
            13.92716 * DEG_TO_RAD,
            26.80536 * DEG_TO_RAD,
            6.38918 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.625,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [21.43 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [21.43 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.125,
        [
            12.40725 * DEG_TO_RAD,
            0.0 * DEG_TO_RAD,
            0.00783 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.375,
        [-7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4167,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.625,
        [15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.6667,
        [17.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, -0.5, 0.0], Interpolation::Linear),
    Keyframe::new(1.625, [0.0, -0.4, 0.0], Interpolation::Linear),
    Keyframe::new(
        1.6667,
        [-0.01805, -0.88303, -0.09783],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [-0.01805, -0.88303, -0.09783],
        Interpolation::Linear,
    ),
    Keyframe::new(1.8333, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(2.0417, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.375, [0.0, 0.0, -1.0], Interpolation::Linear),
    Keyframe::new(2.4583, [0.0, 0.0, -1.0], Interpolation::Linear),
    Keyframe::new(2.5417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_2_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [-20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [-2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.6667,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.1667,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [0.0 * DEG_TO_RAD, 27.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.4583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5,
        [0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.625,
        [0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            9.73588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7083,
        [
            9.73588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [
            10.15255 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            17.86088 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.875,
        [
            17.23588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.9167,
        [
            17.23588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            17.23588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0833,
        [-0.26 * DEG_TO_RAD, -1.93 * DEG_TO_RAD, -3.73 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.6667,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_3_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.1667, [0.0, 0.15451, 0.47553], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.0417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.4583, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.625, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(
        1.6667,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(1.875, [-0.22438, -0.82319, -1.27252], Interpolation::Linear),
    Keyframe::new(
        1.9167,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(2.0833, [-0.39, -0.52, -2.21], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.4583, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.6667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [
            -7.38733 * DEG_TO_RAD,
            1.29876 * DEG_TO_RAD,
            9.91615 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -7.38733 * DEG_TO_RAD,
            1.29876 * DEG_TO_RAD,
            9.91615 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 32.5 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [
            -34.55418 * DEG_TO_RAD,
            11.73507 * DEG_TO_RAD,
            36.8361 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4583,
        [
            -117.82767 * DEG_TO_RAD,
            2.94538 * DEG_TO_RAD,
            0.22703 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            -97.7902 * DEG_TO_RAD,
            0.73403 * DEG_TO_RAD,
            1.39387 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [-92.79 * DEG_TO_RAD, 0.73 * DEG_TO_RAD, 1.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [-92.79 * DEG_TO_RAD, 0.73 * DEG_TO_RAD, 1.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [
            -95.83405 * DEG_TO_RAD,
            33.18639 * DEG_TO_RAD,
            -0.40081 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5,
        [-95.83 * DEG_TO_RAD, 33.19 * DEG_TO_RAD, -0.4 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [
            -44.60123 * DEG_TO_RAD,
            10.14454 * DEG_TO_RAD,
            8.66307 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            -4.31506 * DEG_TO_RAD,
            6.54961 * DEG_TO_RAD,
            13.21388 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0833,
        [
            -4.31506 * DEG_TO_RAD,
            6.54961 * DEG_TO_RAD,
            13.21388 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.125,
        [
            -4.31506 * DEG_TO_RAD,
            6.54961 * DEG_TO_RAD,
            13.21388 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2083,
        [
            -113.7629 * DEG_TO_RAD,
            21.38835 * DEG_TO_RAD,
            15.48184 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [-113.76 * DEG_TO_RAD, 21.39 * DEG_TO_RAD, 15.48 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [
            -39.99304 * DEG_TO_RAD,
            7.3511 * DEG_TO_RAD,
            14.05666 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.625,
        [
            68.07913 * DEG_TO_RAD,
            -3.61348 * DEG_TO_RAD,
            1.39182 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.6667,
        [
            193.16708 * DEG_TO_RAD,
            -1.90441 * DEG_TO_RAD,
            -0.43495 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [
            250.66708 * DEG_TO_RAD,
            -1.90441 * DEG_TO_RAD,
            -0.43495 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7917,
        [
            264.19006 * DEG_TO_RAD,
            -4.41519 * DEG_TO_RAD,
            -0.66792 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.8333,
        [
            270.53693 * DEG_TO_RAD,
            16.03493 * DEG_TO_RAD,
            0.47968 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [
            319.31668 * DEG_TO_RAD,
            17.97846 * DEG_TO_RAD,
            1.34328 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9167,
        [0.0 * DEG_TO_RAD, -5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.375, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.75, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.8333, [0.25358, 0.20153, 2.21248], Interpolation::Linear),
    Keyframe::new(1.5, [0.25, 0.2, 2.21], Interpolation::Linear),
    Keyframe::new(1.5417, [-0.79739, 0.10573, 1.70592], Interpolation::Linear),
    Keyframe::new(1.5833, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(1.8333, [-0.51052, 0.38088, 0.79745], Interpolation::Linear),
    Keyframe::new(2.0833, [-0.51052, 0.38088, 0.79745], Interpolation::Linear),
    Keyframe::new(2.125, [-0.51052, 0.38088, 0.79745], Interpolation::Linear),
    Keyframe::new(2.4583, [-0.51, 0.38, 0.8], Interpolation::Linear),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_6_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [25.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -37.5 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [
            -21.59341 * DEG_TO_RAD,
            -12.60837 * DEG_TO_RAD,
            -45.69252 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -120.7755 * DEG_TO_RAD,
            -5.21988 * DEG_TO_RAD,
            -2.02064 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [
            -98.27419 * DEG_TO_RAD,
            -1.79323 * DEG_TO_RAD,
            -1.15048 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [-93.27 * DEG_TO_RAD, -1.79 * DEG_TO_RAD, -1.15 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            -93.27419 * DEG_TO_RAD,
            -1.79323 * DEG_TO_RAD,
            -1.15048 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.1667,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [-95.75 * DEG_TO_RAD, -2.42 * DEG_TO_RAD, 5.97 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [
            -98.4029 * DEG_TO_RAD,
            -17.39503 * DEG_TO_RAD,
            6.85104 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [
            -101.24523 * DEG_TO_RAD,
            -29.87096 * DEG_TO_RAD,
            7.69993 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [-101.25 * DEG_TO_RAD, -29.87 * DEG_TO_RAD, 7.7 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [
            -88.58526 * DEG_TO_RAD,
            -17.10045 * DEG_TO_RAD,
            11.7676 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [-88.59 * DEG_TO_RAD, -17.1 * DEG_TO_RAD, 11.77 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [
            -46.59531 * DEG_TO_RAD,
            -16.13694 * DEG_TO_RAD,
            -3.85578 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_7_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.6667, [-0.00677, 0.76064, 3.19059], Interpolation::Linear),
    Keyframe::new(2.4583, [0.0512, 0.76176, 3.12882], Interpolation::Linear),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_8_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.125,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.375,
        [2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5833,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.625,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_9_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.375, [0.0, -0.0909, -0.10834], Interpolation::Linear),
    Keyframe::new(2.4583, [0.0, -0.09, -0.11], Interpolation::Linear),
    Keyframe::new(2.5417, [0.0, -0.09, -0.11], Interpolation::Linear),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_10_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.125,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.375,
        [2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5833,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.625,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_11_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.375, [0.0, -0.0909, -0.10834], Interpolation::Linear),
    Keyframe::new(2.4583, [0.0, -0.09, -0.11], Interpolation::Linear),
    Keyframe::new(2.5417, [0.0, -0.09, -0.11], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_1_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_2_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_3_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_6_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_7_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_8_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_9_RIGHT_LEG_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_10_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_11_LEFT_LEG_POSITION,
    },
];

pub static COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.0,
    looping: true,
    channels: COPPER_GOLEM_CHEST_INTERACTION_ITEM_DROP_CHANNELS,
};

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [18.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [24.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [
            12.40525 * DEG_TO_RAD,
            -0.00044 * DEG_TO_RAD,
            0.00829 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [
            12.40525 * DEG_TO_RAD,
            -0.00044 * DEG_TO_RAD,
            0.00829 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [
            13.92716 * DEG_TO_RAD,
            26.80536 * DEG_TO_RAD,
            6.38918 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.625,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [21.43 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [21.43 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [
            12.40725 * DEG_TO_RAD,
            0.0 * DEG_TO_RAD,
            0.00783 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [-2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, -0.5, 0.0], Interpolation::Linear),
    Keyframe::new(1.625, [0.0, -0.4, 0.0], Interpolation::Linear),
    Keyframe::new(
        1.6667,
        [-0.01805, -0.88303, -0.09783],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [-0.01805, -0.88303, -0.09783],
        Interpolation::Linear,
    ),
    Keyframe::new(1.8333, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(2.0417, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(2.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_2_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [-20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [-2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.6667,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.1667,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [0.0 * DEG_TO_RAD, 27.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.4583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5,
        [0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.625,
        [0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            9.73588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7083,
        [
            9.73588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [
            10.15255 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            17.86088 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.875,
        [
            17.23588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.9167,
        [
            17.23588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            17.23588 * DEG_TO_RAD,
            -1.93433 * DEG_TO_RAD,
            -3.73384 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0833,
        [-0.26 * DEG_TO_RAD, -1.93 * DEG_TO_RAD, -3.73 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [1.25 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.6667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, -360.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_3_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.1667, [0.0, 0.15451, 0.47553], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.0417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.4583, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.625, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(
        1.6667,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(1.875, [-0.22438, -0.82319, -1.27252], Interpolation::Linear),
    Keyframe::new(
        1.9167,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(2.0833, [-0.39, -0.52, -2.21], Interpolation::Linear),
    Keyframe::new(2.25, [0.0, 0.0, 0.5], Interpolation::Linear),
    Keyframe::new(2.2917, [0.0, 0.01091, -0.02988], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0, 0.01, -0.03], Interpolation::Linear),
    Keyframe::new(2.6667, [0.0, 0.01, -0.03], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.01, -0.03], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [
            -7.38733 * DEG_TO_RAD,
            1.29876 * DEG_TO_RAD,
            9.91615 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -7.38733 * DEG_TO_RAD,
            1.29876 * DEG_TO_RAD,
            9.91615 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 32.5 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [
            -34.55418 * DEG_TO_RAD,
            11.73507 * DEG_TO_RAD,
            36.8361 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4583,
        [
            -117.82767 * DEG_TO_RAD,
            2.94538 * DEG_TO_RAD,
            0.22703 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            -97.7902 * DEG_TO_RAD,
            0.73403 * DEG_TO_RAD,
            1.39387 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [-92.79 * DEG_TO_RAD, 0.73 * DEG_TO_RAD, 1.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [-92.79 * DEG_TO_RAD, 0.73 * DEG_TO_RAD, 1.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [
            -95.83405 * DEG_TO_RAD,
            33.18639 * DEG_TO_RAD,
            -0.40081 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5,
        [-95.83 * DEG_TO_RAD, 33.19 * DEG_TO_RAD, -0.4 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [
            -44.60123 * DEG_TO_RAD,
            10.14454 * DEG_TO_RAD,
            8.66307 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            -4.31506 * DEG_TO_RAD,
            6.54961 * DEG_TO_RAD,
            13.21388 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0833,
        [
            -4.31506 * DEG_TO_RAD,
            6.54961 * DEG_TO_RAD,
            13.21388 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [
            -4.31506 * DEG_TO_RAD,
            6.54961 * DEG_TO_RAD,
            13.21388 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [
            -6.53898 * DEG_TO_RAD,
            13.96898 * DEG_TO_RAD,
            14.34786 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [
            3.50393 * DEG_TO_RAD,
            -4.70737 * DEG_TO_RAD,
            8.3608 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [
            3.50393 * DEG_TO_RAD,
            -4.70737 * DEG_TO_RAD,
            8.3608 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [
            3.90089 * DEG_TO_RAD,
            -4.3843 * DEG_TO_RAD,
            3.35549 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [3.9 * DEG_TO_RAD, -4.38 * DEG_TO_RAD, 3.36 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9167,
        [3.9 * DEG_TO_RAD, -4.38 * DEG_TO_RAD, 3.36 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [
            3.90089 * DEG_TO_RAD,
            -4.3843 * DEG_TO_RAD,
            3.35549 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.375, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.75, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.8333, [0.25358, 0.20153, 2.21248], Interpolation::Linear),
    Keyframe::new(1.5, [0.25, 0.2, 2.21], Interpolation::Linear),
    Keyframe::new(1.5417, [-0.79739, 0.10573, 1.70592], Interpolation::Linear),
    Keyframe::new(1.5833, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(1.8333, [-0.51052, 0.38088, 0.79745], Interpolation::Linear),
    Keyframe::new(2.0833, [-0.51052, 0.38088, 0.79745], Interpolation::Linear),
    Keyframe::new(2.1667, [-0.51052, 0.38088, 0.79745], Interpolation::Linear),
    Keyframe::new(2.25, [-0.46, 0.34, 0.72], Interpolation::Linear),
    Keyframe::new(2.3333, [-0.46, -0.1159, -0.30086], Interpolation::Linear),
    Keyframe::new(2.4583, [-0.46, -0.1159, -0.30086], Interpolation::Linear),
    Keyframe::new(2.5, [-0.46, -0.1159, -0.30086], Interpolation::Linear),
    Keyframe::new(2.5417, [-0.46, 0.88, -0.3], Interpolation::Linear),
    Keyframe::new(2.9167, [-0.46, 0.88, -0.3], Interpolation::Linear),
    Keyframe::new(2.9583, [-0.46, -0.1159, -0.30086], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_6_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [25.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -37.5 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [
            -21.59341 * DEG_TO_RAD,
            -12.60837 * DEG_TO_RAD,
            -45.69252 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -120.7755 * DEG_TO_RAD,
            -5.21988 * DEG_TO_RAD,
            -2.02064 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [
            -98.27419 * DEG_TO_RAD,
            -1.79323 * DEG_TO_RAD,
            -1.15048 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [-93.27 * DEG_TO_RAD, -1.79 * DEG_TO_RAD, -1.15 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            -93.27419 * DEG_TO_RAD,
            -1.79323 * DEG_TO_RAD,
            -1.15048 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.1667,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [-95.75 * DEG_TO_RAD, -2.42 * DEG_TO_RAD, 5.97 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [
            -98.4029 * DEG_TO_RAD,
            -17.39503 * DEG_TO_RAD,
            6.85104 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [
            -101.24523 * DEG_TO_RAD,
            -29.87096 * DEG_TO_RAD,
            7.69993 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [-101.25 * DEG_TO_RAD, -29.87 * DEG_TO_RAD, 7.7 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [
            2.47864 * DEG_TO_RAD,
            -0.32621 * DEG_TO_RAD,
            -12.50706 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [
            2.47864 * DEG_TO_RAD,
            -0.32621 * DEG_TO_RAD,
            -12.50706 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [
            2.41492 * DEG_TO_RAD,
            -0.64686 * DEG_TO_RAD,
            -5.01363 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [2.41 * DEG_TO_RAD, -0.65 * DEG_TO_RAD, -5.01 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9167,
        [2.41 * DEG_TO_RAD, -0.65 * DEG_TO_RAD, -5.01 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [
            2.41492 * DEG_TO_RAD,
            -0.64686 * DEG_TO_RAD,
            -5.01363 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_7_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.6667, [-0.00677, 0.76064, 3.19059], Interpolation::Linear),
    Keyframe::new(1.8333, [-0.00677, 0.76064, 3.19059], Interpolation::Linear),
    Keyframe::new(2.0417, [-0.00677, 0.76064, 3.19059], Interpolation::Linear),
    Keyframe::new(2.25, [0.03, 0.76, 0.45], Interpolation::Linear),
    Keyframe::new(2.3333, [0.03, 0.28229, -0.07133], Interpolation::Linear),
    Keyframe::new(2.4583, [0.03, 0.28229, -0.07133], Interpolation::Linear),
    Keyframe::new(2.5, [0.03, 0.28229, -0.07133], Interpolation::Linear),
    Keyframe::new(2.5417, [0.03, 1.28, -0.07], Interpolation::Linear),
    Keyframe::new(2.9167, [0.03, 1.28, -0.07], Interpolation::Linear),
    Keyframe::new(2.9583, [0.03, 0.28229, -0.07133], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_8_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_9_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_10_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_11_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_12_LEFT_LEG_SCALE: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [1.0 - 1.0, 1.0 - 1.0, 1.0 - 1.0],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_1_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_2_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_3_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_6_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_7_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_8_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_9_RIGHT_LEG_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_10_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_11_LEFT_LEG_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Scale,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_12_LEFT_LEG_SCALE,
    },
];

pub static COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.0,
    looping: true,
    channels: COPPER_GOLEM_CHEST_INTERACTION_ITEM_NODROP_CHANNELS,
};

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [18.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [24.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [
            12.40525 * DEG_TO_RAD,
            -0.00044 * DEG_TO_RAD,
            0.00829 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [
            12.40525 * DEG_TO_RAD,
            -0.00044 * DEG_TO_RAD,
            0.00829 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [
            13.92716 * DEG_TO_RAD,
            26.80536 * DEG_TO_RAD,
            6.38918 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.625,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [21.43 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [21.43 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.125,
        [
            12.40725 * DEG_TO_RAD,
            0.0 * DEG_TO_RAD,
            0.00783 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.375,
        [15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4167,
        [17.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.625,
        [
            24.14867 * DEG_TO_RAD,
            -20.70481 * DEG_TO_RAD,
            -9.00717 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [
            24.14867 * DEG_TO_RAD,
            -20.70481 * DEG_TO_RAD,
            -9.00717 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.75,
        [22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, -0.5, 0.0], Interpolation::Linear),
    Keyframe::new(1.625, [0.0, -0.4, 0.0], Interpolation::Linear),
    Keyframe::new(
        1.6667,
        [-0.01805, -0.88303, -0.09783],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [-0.01805, -0.88303, -0.09783],
        Interpolation::Linear,
    ),
    Keyframe::new(1.8333, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(2.0417, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.625, [0.0, -0.46194, -0.19134], Interpolation::Linear),
    Keyframe::new(2.7083, [0.0, -0.46194, -0.19134], Interpolation::Linear),
    Keyframe::new(2.7917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_2_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [-20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [-2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.6667,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.1667,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [0.0 * DEG_TO_RAD, 27.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.4583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.625,
        [0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            10.16381 * DEG_TO_RAD,
            -16.71134 * DEG_TO_RAD,
            -6.35306 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            10.16381 * DEG_TO_RAD,
            -16.71134 * DEG_TO_RAD,
            -6.35306 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.9167,
        [
            10.16381 * DEG_TO_RAD,
            -16.71134 * DEG_TO_RAD,
            -6.35306 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.9583,
        [
            10.16381 * DEG_TO_RAD,
            -16.71134 * DEG_TO_RAD,
            -6.35306 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0,
        [
            5.16381 * DEG_TO_RAD,
            -16.71134 * DEG_TO_RAD,
            -6.35306 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            0.16381 * DEG_TO_RAD,
            -16.71134 * DEG_TO_RAD,
            -6.35306 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0833,
        [
            0.15732 * DEG_TO_RAD,
            -4.21139 * DEG_TO_RAD,
            -6.31751 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.125,
        [
            0.07901 * DEG_TO_RAD,
            5.3943 * DEG_TO_RAD,
            -3.15187 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [0.0 * DEG_TO_RAD, 7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [
            4.53867 * DEG_TO_RAD,
            7.47675 * DEG_TO_RAD,
            0.59181 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [
            -2.53852 * DEG_TO_RAD,
            9.99038 * DEG_TO_RAD,
            -0.44067 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4167,
        [
            -12.68664 * DEG_TO_RAD,
            9.76061 * DEG_TO_RAD,
            -2.18558 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.625,
        [
            -15.19938 * DEG_TO_RAD,
            22.36971 * DEG_TO_RAD,
            -3.52259 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.6667,
        [
            -3.02173 * DEG_TO_RAD,
            22.37156 * DEG_TO_RAD,
            -2.41802 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [
            -0.52173 * DEG_TO_RAD,
            22.37156 * DEG_TO_RAD,
            -2.41802 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.75,
        [
            -12.40598 * DEG_TO_RAD,
            -0.4674 * DEG_TO_RAD,
            -1.79838 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.8333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_3_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.1667, [0.0, 0.15451, 0.47553], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.0417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.4583, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.625, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(
        1.6667,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.9167,
        [-0.22438, -0.82319, -1.27252],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.9583,
        [-0.52521, -0.96725, -0.32978],
        Interpolation::Linear,
    ),
    Keyframe::new(2.0, [-0.52521, -0.96725, -0.32978], Interpolation::Linear),
    Keyframe::new(2.0417, [-0.5345, -1.16541, -0.37206], Interpolation::Linear),
    Keyframe::new(2.0833, [-0.5345, -1.16541, -0.37206], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.625, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.8333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [
            -7.38733 * DEG_TO_RAD,
            1.29876 * DEG_TO_RAD,
            9.91615 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -7.38733 * DEG_TO_RAD,
            1.29876 * DEG_TO_RAD,
            9.91615 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 32.5 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [
            -34.55418 * DEG_TO_RAD,
            11.73507 * DEG_TO_RAD,
            36.8361 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4583,
        [
            -82.47403 * DEG_TO_RAD,
            17.82361 * DEG_TO_RAD,
            2.17224 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [
            -85.08388 * DEG_TO_RAD,
            14.26971 * DEG_TO_RAD,
            1.99595 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            -85.16266 * DEG_TO_RAD,
            13.19102 * DEG_TO_RAD,
            2.43976 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [-92.79 * DEG_TO_RAD, 0.73 * DEG_TO_RAD, 1.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [-92.79 * DEG_TO_RAD, 0.73 * DEG_TO_RAD, 1.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [
            -95.83405 * DEG_TO_RAD,
            33.18639 * DEG_TO_RAD,
            -0.40081 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [-95.83 * DEG_TO_RAD, 33.19 * DEG_TO_RAD, -0.4 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [-98.33 * DEG_TO_RAD, 33.19 * DEG_TO_RAD, -0.4 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5417,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [
            -84.12204 * DEG_TO_RAD,
            8.95753 * DEG_TO_RAD,
            14.11779 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2083,
        [
            -84.12204 * DEG_TO_RAD,
            8.95753 * DEG_TO_RAD,
            14.11779 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [
            -93.6065 * DEG_TO_RAD,
            13.90544 * DEG_TO_RAD,
            15.98524 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [
            -124.48661 * DEG_TO_RAD,
            66.29146 * DEG_TO_RAD,
            -7.28605 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.375,
        [
            -129.4866 * DEG_TO_RAD,
            66.29146 * DEG_TO_RAD,
            -7.28605 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4167,
        [
            -108.91607 * DEG_TO_RAD,
            1.79762 * DEG_TO_RAD,
            20.93924 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [
            -102.18303 * DEG_TO_RAD,
            4.35881 * DEG_TO_RAD,
            17.40962 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [
            -98.33642 * DEG_TO_RAD,
            -0.70114 * DEG_TO_RAD,
            4.09322 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.625,
        [
            -98.39385 * DEG_TO_RAD,
            6.71929 * DEG_TO_RAD,
            3.00137 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.6667,
        [
            -98.33981 * DEG_TO_RAD,
            1.77244 * DEG_TO_RAD,
            3.7307 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [
            -100.70987 * DEG_TO_RAD,
            3.48829 * DEG_TO_RAD,
            7.1138 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.75,
        [-97.95 * DEG_TO_RAD, 6.92 * DEG_TO_RAD, 13.88 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7917,
        [-87.95 * DEG_TO_RAD, 6.92 * DEG_TO_RAD, 13.88 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.8333,
        [-97.95 * DEG_TO_RAD, 6.92 * DEG_TO_RAD, 13.88 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [-102.95 * DEG_TO_RAD, 6.92 * DEG_TO_RAD, 13.88 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9167,
        [-76.475 * DEG_TO_RAD, 3.46 * DEG_TO_RAD, 6.94 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [-26.475 * DEG_TO_RAD, 3.46 * DEG_TO_RAD, 6.94 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.375, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.75, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.8333, [0.25358, 0.20153, 2.21248], Interpolation::Linear),
    Keyframe::new(1.25, [0.25, 0.2, 2.21], Interpolation::Linear),
    Keyframe::new(1.2917, [0.25, 0.2, 2.21], Interpolation::Linear),
    Keyframe::new(1.5417, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(1.6667, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(1.8333, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(2.0, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(2.0417, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(2.25, [-0.51, 0.38, 0.8], Interpolation::Linear),
    Keyframe::new(2.3333, [-0.51, 0.38, 0.8], Interpolation::Linear),
    Keyframe::new(2.375, [-0.51, 0.38, 0.8], Interpolation::Linear),
    Keyframe::new(2.4167, [-2.14094, -0.69619, 1.23422], Interpolation::Linear),
    Keyframe::new(2.4583, [-0.97932, -0.38244, 0.12884], Interpolation::Linear),
    Keyframe::new(2.5417, [-1.55232, -1.79904, 0.37956], Interpolation::Linear),
    Keyframe::new(2.625, [-1.53125, -1.64598, 1.41168], Interpolation::Linear),
    Keyframe::new(2.6667, [-1.57256, -1.05375, 1.32469], Interpolation::Linear),
    Keyframe::new(2.75, [-1.33, -0.16, 1.02], Interpolation::Linear),
    Keyframe::new(2.7917, [-1.33, -0.16, 1.02], Interpolation::Linear),
    Keyframe::new(2.8333, [-1.33, -0.16, 1.02], Interpolation::Linear),
    Keyframe::new(2.875, [-1.33, -0.16, 1.02], Interpolation::Linear),
    Keyframe::new(2.9167, [-0.5748, -0.38848, 1.45646], Interpolation::Linear),
    Keyframe::new(2.9583, [-0.67, -0.08, 0.51], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_6_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [25.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -37.5 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [
            -21.59341 * DEG_TO_RAD,
            -12.60837 * DEG_TO_RAD,
            -45.69252 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -120.7755 * DEG_TO_RAD,
            -5.21988 * DEG_TO_RAD,
            -2.02064 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [
            -98.27419 * DEG_TO_RAD,
            -1.79323 * DEG_TO_RAD,
            -1.15048 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [-93.27 * DEG_TO_RAD, -1.79 * DEG_TO_RAD, -1.15 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            -93.27419 * DEG_TO_RAD,
            -1.79323 * DEG_TO_RAD,
            -1.15048 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.1667,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [-95.75 * DEG_TO_RAD, -2.42 * DEG_TO_RAD, 5.97 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [
            -98.4029 * DEG_TO_RAD,
            -17.39503 * DEG_TO_RAD,
            6.85104 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [
            -101.24523 * DEG_TO_RAD,
            -29.87096 * DEG_TO_RAD,
            7.69993 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [-101.25 * DEG_TO_RAD, -29.87 * DEG_TO_RAD, 7.7 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.9583,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2083,
        [
            -88.58526 * DEG_TO_RAD,
            -17.10045 * DEG_TO_RAD,
            11.7676 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [-88.59 * DEG_TO_RAD, -17.1 * DEG_TO_RAD, 11.77 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4167,
        [
            -46.59531 * DEG_TO_RAD,
            -16.13694 * DEG_TO_RAD,
            -3.85578 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [
            -24.5317 * DEG_TO_RAD,
            -19.0214 * DEG_TO_RAD,
            -13.70805 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.8333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_7_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.6667, [-0.00677, 0.76064, 3.19059], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0512, 0.76176, 3.12882], Interpolation::Linear),
    Keyframe::new(2.8333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_8_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7083,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.7917,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.8333,
        [5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_9_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.7083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.7917, [0.0, -0.09, -0.11], Interpolation::Linear),
    Keyframe::new(2.8333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_10_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4167,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.625,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_11_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.0417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.4583, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.625, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_1_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_2_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_3_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_6_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_7_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_8_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_9_RIGHT_LEG_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_10_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_11_LEFT_LEG_POSITION,
    },
];

pub static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.0,
    looping: true,
    channels: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_GET_CHANNELS,
};

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [18.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [24.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.9167,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [
            14.72765 * DEG_TO_RAD,
            -31.63886 * DEG_TO_RAD,
            -7.85085 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        [
            12.40525 * DEG_TO_RAD,
            -0.00044 * DEG_TO_RAD,
            0.00829 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [
            12.40525 * DEG_TO_RAD,
            -0.00044 * DEG_TO_RAD,
            0.00829 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [
            13.92716 * DEG_TO_RAD,
            26.80536 * DEG_TO_RAD,
            6.38918 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.625,
        [13.93 * DEG_TO_RAD, 26.81 * DEG_TO_RAD, 6.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7083,
        [
            12.40725 * DEG_TO_RAD,
            0.00444 * DEG_TO_RAD,
            0.00783 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            12.40725 * DEG_TO_RAD,
            0.00444 * DEG_TO_RAD,
            0.00783 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.125,
        [
            12.40725 * DEG_TO_RAD,
            0.0 * DEG_TO_RAD,
            0.00783 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.25,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.6667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, -0.6, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, -0.5, 0.0], Interpolation::Linear),
    Keyframe::new(1.625, [0.0, -0.4, 0.0], Interpolation::Linear),
    Keyframe::new(1.7083, [0.0, -0.34, 0.0], Interpolation::Linear),
    Keyframe::new(2.0417, [0.0, -0.34, 0.0], Interpolation::Linear),
    Keyframe::new(2.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.4583, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.6667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_2_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [-20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [-2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.6667,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.0417,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.1667,
        [0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [0.0 * DEG_TO_RAD, 27.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.4167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.4583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.625,
        [0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7083,
        [0.57 * DEG_TO_RAD, -1.25 * DEG_TO_RAD, 0.07 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.75,
        [
            0.89798 * DEG_TO_RAD,
            -18.12465 * DEG_TO_RAD,
            -0.16276 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.7917,
        [
            1.21328 * DEG_TO_RAD,
            -21.15422 * DEG_TO_RAD,
            -0.2148 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.875,
        [
            1.21328 * DEG_TO_RAD,
            -21.15422 * DEG_TO_RAD,
            -0.2148 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0,
        [
            1.21328 * DEG_TO_RAD,
            -21.15422 * DEG_TO_RAD,
            -0.2148 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            2.56546 * DEG_TO_RAD,
            0.76525 * DEG_TO_RAD,
            0.57246 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [
            4.53867 * DEG_TO_RAD,
            7.47675 * DEG_TO_RAD,
            0.59181 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [
            4.53867 * DEG_TO_RAD,
            7.47675 * DEG_TO_RAD,
            0.59181 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, -360.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_3_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.1667, [0.0, 0.15451, 0.47553], Interpolation::Linear),
    Keyframe::new(0.25, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.0417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.4583, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.625, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.7083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.5417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.01, -0.03], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.1667,
        [
            -7.38733 * DEG_TO_RAD,
            1.29876 * DEG_TO_RAD,
            9.91615 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -7.38733 * DEG_TO_RAD,
            1.29876 * DEG_TO_RAD,
            9.91615 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 32.5 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [
            -34.55418 * DEG_TO_RAD,
            11.73507 * DEG_TO_RAD,
            36.8361 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4583,
        [
            -82.47403 * DEG_TO_RAD,
            17.82361 * DEG_TO_RAD,
            2.17224 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5,
        [
            -85.08388 * DEG_TO_RAD,
            14.26971 * DEG_TO_RAD,
            1.99595 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            -85.16266 * DEG_TO_RAD,
            13.19102 * DEG_TO_RAD,
            2.43976 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [-92.79 * DEG_TO_RAD, 0.73 * DEG_TO_RAD, 1.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        [-92.79 * DEG_TO_RAD, 0.73 * DEG_TO_RAD, 1.39 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.8333,
        [
            -95.83405 * DEG_TO_RAD,
            33.18639 * DEG_TO_RAD,
            -0.40081 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [-95.83 * DEG_TO_RAD, 33.19 * DEG_TO_RAD, -0.4 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [-98.33 * DEG_TO_RAD, 33.19 * DEG_TO_RAD, -0.4 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5417,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [
            -56.46674 * DEG_TO_RAD,
            3.3853 * DEG_TO_RAD,
            14.45894 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [3.9 * DEG_TO_RAD, -4.38 * DEG_TO_RAD, 3.36 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9167,
        [3.9 * DEG_TO_RAD, -4.38 * DEG_TO_RAD, 3.36 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [
            3.90089 * DEG_TO_RAD,
            -4.3843 * DEG_TO_RAD,
            3.35549 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.375, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.75, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.8333, [0.25358, 0.20153, 2.21248], Interpolation::Linear),
    Keyframe::new(1.25, [0.25, 0.2, 2.21], Interpolation::Linear),
    Keyframe::new(1.2917, [0.25, 0.2, 2.21], Interpolation::Linear),
    Keyframe::new(1.5417, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(1.6667, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(1.8333, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(2.0, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(2.0417, [-0.26323, 1.46323, 0.66566], Interpolation::Linear),
    Keyframe::new(2.1667, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.5, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.5417, [-0.46, 0.88, -0.3], Interpolation::Linear),
    Keyframe::new(2.9167, [-0.46, 0.88, -0.3], Interpolation::Linear),
    Keyframe::new(2.9583, [-0.46, -0.1159, -0.30086], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_6_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [-2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [25.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -37.5 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.25,
        [
            -21.59341 * DEG_TO_RAD,
            -12.60837 * DEG_TO_RAD,
            -45.69252 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2917,
        [
            -120.7755 * DEG_TO_RAD,
            -5.21988 * DEG_TO_RAD,
            -2.02064 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.375,
        [
            -98.27419 * DEG_TO_RAD,
            -1.79323 * DEG_TO_RAD,
            -1.15048 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.4167,
        [-93.27 * DEG_TO_RAD, -1.79 * DEG_TO_RAD, -1.15 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5417,
        [
            -93.27419 * DEG_TO_RAD,
            -1.79323 * DEG_TO_RAD,
            -1.15048 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.5833,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.7083,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.1667,
        [
            -93.55693 * DEG_TO_RAD,
            -22.3224 * DEG_TO_RAD,
            3.64383 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2083,
        [-95.75 * DEG_TO_RAD, -2.42 * DEG_TO_RAD, 5.97 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.25,
        [
            -98.4029 * DEG_TO_RAD,
            -17.39503 * DEG_TO_RAD,
            6.85104 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.2917,
        [
            -101.24523 * DEG_TO_RAD,
            -29.87096 * DEG_TO_RAD,
            7.69993 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.5833,
        [-101.25 * DEG_TO_RAD, -29.87 * DEG_TO_RAD, 7.7 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.6667,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.8333,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0833,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.1667,
        [
            -88.17772 * DEG_TO_RAD,
            -42.09094 * DEG_TO_RAD,
            10.96195 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2083,
        [
            -88.58526 * DEG_TO_RAD,
            -17.10045 * DEG_TO_RAD,
            11.7676 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [-88.59 * DEG_TO_RAD, -17.1 * DEG_TO_RAD, 11.77 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4167,
        [
            -46.59531 * DEG_TO_RAD,
            -16.13694 * DEG_TO_RAD,
            -3.85578 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4583,
        [
            -24.5317 * DEG_TO_RAD,
            -19.0214 * DEG_TO_RAD,
            -13.70805 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5,
        [
            -24.5317 * DEG_TO_RAD,
            -19.0214 * DEG_TO_RAD,
            -13.70805 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.5417,
        [2.41 * DEG_TO_RAD, -0.65 * DEG_TO_RAD, -5.01 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9167,
        [2.41 * DEG_TO_RAD, -0.65 * DEG_TO_RAD, -5.01 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [
            2.41492 * DEG_TO_RAD,
            -0.64686 * DEG_TO_RAD,
            -5.01363 * DEG_TO_RAD,
        ],
        Interpolation::Linear,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_7_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.4167, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.5833, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(1.6667, [-0.00677, 0.76064, 3.19059], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0512, 0.76176, 3.12882], Interpolation::Linear),
    Keyframe::new(2.4583, [0.03, 0.51, 2.09], Interpolation::Linear),
    Keyframe::new(2.5, [0.03, 0.51, 2.09], Interpolation::Linear),
    Keyframe::new(2.5417, [0.03, 1.28, -0.07], Interpolation::Linear),
    Keyframe::new(2.9167, [0.03, 1.28, -0.07], Interpolation::Linear),
    Keyframe::new(2.9583, [0.03, 0.28229, -0.07133], Interpolation::Linear),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_8_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [7.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_9_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_10_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.125,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.2083,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.0417,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.2917,
        [-10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.3333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.9583,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_11_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.125, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(0.2083, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.0417, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.2917, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.3333, [0.0, 0.0, 0.0], Interpolation::Linear),
    Keyframe::new(2.9583, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_1_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_2_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_3_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_6_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_7_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_8_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_9_RIGHT_LEG_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_10_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_11_LEFT_LEG_POSITION,
    },
];

pub static COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.0,
    looping: true,
    channels: COPPER_GOLEM_CHEST_INTERACTION_NOITEM_NOGET_CHANNELS,
};
