#![allow(clippy::excessive_precision)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, Interpolation, Keyframe, Target, degree_vec, pos_vec, scale_vec,
};

static FROG_CROAK_0_CROAKING_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4167, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, pos_vec(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.9583, pos_vec(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_CROAK_1_CROAKING_BODY_SCALE: &[Keyframe] = &[
    Keyframe::new(0.0, scale_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, scale_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4167, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.4583, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        0.5417,
        scale_vec(1.2999999523162842, 2.0999999046325684, 1.600000023841858),
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.625,
        scale_vec(1.2999999523162842, 2.0999999046325684, 1.600000023841858),
        Interpolation::Linear,
    ),
    Keyframe::new(0.7083, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(2.25, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        2.3333,
        scale_vec(1.2999999523162842, 2.0999999046325684, 1.600000023841858),
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.4167,
        scale_vec(1.2999999523162842, 2.0999999046325684, 1.600000023841858),
        Interpolation::Linear,
    ),
    Keyframe::new(2.5, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(2.5833, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        2.6667,
        scale_vec(1.2999999523162842, 2.0999999046325684, 1.600000023841858),
        Interpolation::Linear,
    ),
    Keyframe::new(
        2.875,
        scale_vec(1.2999999523162842, 2.0999999046325684, 1.600000023841858),
        Interpolation::Linear,
    ),
    Keyframe::new(2.9583, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(3.0, scale_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_CROAK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "croaking_body",
        target: Target::Position,
        keyframes: FROG_CROAK_0_CROAKING_BODY_POSITION,
    },
    Channel {
        bone: "croaking_body",
        target: Target::Scale,
        keyframes: FROG_CROAK_1_CROAKING_BODY_SCALE,
    },
];

pub static FROG_CROAK: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.0,
    looping: false,
    channels: FROG_CROAK_CHANNELS,
};

static FROG_WALK_0_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -5.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2917, degree_vec(7.5, -2.67, -7.5), Interpolation::Linear),
    Keyframe::new(0.625, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7917, degree_vec(22.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.125, degree_vec(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, degree_vec(0.0, -5.0, 0.0), Interpolation::Linear),
];

static FROG_WALK_1_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.1, -2.0), Interpolation::Linear),
    Keyframe::new(0.2917, pos_vec(-0.5, -0.25, -0.13), Interpolation::Linear),
    Keyframe::new(0.625, pos_vec(-0.5, 0.1, 2.0), Interpolation::Linear),
    Keyframe::new(0.9583, pos_vec(0.5, 1.0, -0.11), Interpolation::Linear),
    Keyframe::new(1.25, pos_vec(0.0, 0.1, -2.0), Interpolation::Linear),
];

static FROG_WALK_2_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, degree_vec(22.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, degree_vec(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.625, degree_vec(0.0, 5.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9583, degree_vec(7.5, 2.33, 7.5), Interpolation::Linear),
    Keyframe::new(1.25, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_WALK_3_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.5, 0.1, 2.0), Interpolation::Linear),
    Keyframe::new(0.2917, pos_vec(-0.5, 1.0, 0.12), Interpolation::Linear),
    Keyframe::new(0.625, pos_vec(0.0, 0.1, -2.0), Interpolation::Linear),
    Keyframe::new(0.9583, pos_vec(0.5, -0.25, -0.13), Interpolation::Linear),
    Keyframe::new(1.25, pos_vec(0.5, 0.1, 2.0), Interpolation::Linear),
];

static FROG_WALK_4_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2917, degree_vec(45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.625, degree_vec(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7917, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_WALK_5_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.1, 1.2), Interpolation::Linear),
    Keyframe::new(0.1667, pos_vec(0.0, 0.1, 2.0), Interpolation::Linear),
    Keyframe::new(0.4583, pos_vec(0.0, 2.0, 1.06), Interpolation::Linear),
    Keyframe::new(0.7917, pos_vec(0.0, 0.1, -1.0), Interpolation::Linear),
    Keyframe::new(1.25, pos_vec(0.0, 0.1, 1.2), Interpolation::Linear),
];

static FROG_WALK_6_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-33.75, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.0417, degree_vec(-45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7917, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9583, degree_vec(45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, degree_vec(-33.75, 0.0, 0.0), Interpolation::Linear),
];

static FROG_WALK_7_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 1.14, 0.11), Interpolation::Linear),
    Keyframe::new(0.1667, pos_vec(0.0, 0.1, -1.0), Interpolation::Linear),
    Keyframe::new(0.7917, pos_vec(0.0, 0.1, 2.0), Interpolation::Linear),
    Keyframe::new(1.125, pos_vec(0.0, 2.0, 0.95), Interpolation::Linear),
    Keyframe::new(1.25, pos_vec(0.0, 1.14, 0.11), Interpolation::Linear),
];

static FROG_WALK_8_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 5.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2917, degree_vec(-7.5, 0.33, 7.5), Interpolation::Linear),
    Keyframe::new(0.625, degree_vec(0.0, -5.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.9583, degree_vec(-7.5, 0.33, -7.5), Interpolation::Linear),
    Keyframe::new(1.25, degree_vec(0.0, 5.0, 0.0), Interpolation::Linear),
];

static FROG_WALK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: FROG_WALK_0_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: FROG_WALK_1_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: FROG_WALK_2_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: FROG_WALK_3_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: FROG_WALK_4_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: FROG_WALK_5_LEFT_LEG_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: FROG_WALK_6_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: FROG_WALK_7_RIGHT_LEG_POSITION,
    },
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: FROG_WALK_8_BODY_ROTATION,
    },
];

pub static FROG_WALK: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.25,
    looping: true,
    channels: FROG_WALK_CHANNELS,
};

static FROG_JUMP_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-22.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(-22.5, 0.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_2_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-56.14, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(-56.14, 0.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_3_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos_vec(0.0, 1.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-56.14, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(-56.14, 0.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos_vec(0.0, 1.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_6_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(45.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_7_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_8_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(45.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(45.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_9_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_JUMP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: FROG_JUMP_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: FROG_JUMP_1_BODY_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: FROG_JUMP_2_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: FROG_JUMP_3_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: FROG_JUMP_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: FROG_JUMP_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: FROG_JUMP_6_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: FROG_JUMP_7_LEFT_LEG_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: FROG_JUMP_8_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: FROG_JUMP_9_RIGHT_LEG_POSITION,
    },
];

pub static FROG_JUMP: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: false,
    channels: FROG_JUMP_CHANNELS,
};

static FROG_TONGUE_0_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.0833, degree_vec(-60.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4167, degree_vec(-60.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_TONGUE_1_HEAD_SCALE: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.0833, degree_vec(0.998, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.4167, degree_vec(0.998, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(1.0, 1.0, 1.0), Interpolation::Linear),
];

static FROG_TONGUE_2_TONGUE_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.0833, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4167, degree_vec(-18.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static FROG_TONGUE_3_TONGUE_SCALE: &[Keyframe] = &[
    Keyframe::new(0.0833, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.1667, scale_vec(0.5, 1.0, 5.0), Interpolation::Linear),
    Keyframe::new(0.4167, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
];

static FROG_TONGUE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: FROG_TONGUE_0_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Scale,
        keyframes: FROG_TONGUE_1_HEAD_SCALE,
    },
    Channel {
        bone: "tongue",
        target: Target::Rotation,
        keyframes: FROG_TONGUE_2_TONGUE_ROTATION,
    },
    Channel {
        bone: "tongue",
        target: Target::Scale,
        keyframes: FROG_TONGUE_3_TONGUE_SCALE,
    },
];

pub static FROG_TONGUE: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: false,
    channels: FROG_TONGUE_CHANNELS,
};

static FROG_SWIM_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.3333,
        degree_vec(10.0, 0.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.6667,
        degree_vec(-10.0, 0.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(1.0417, degree_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static FROG_SWIM_1_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(90.0, 22.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.4583,
        degree_vec(45.0, 22.5, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.6667,
        degree_vec(-22.5, -22.5, -22.5),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.875,
        degree_vec(-45.0, -22.5, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.9583,
        degree_vec(22.5, 0.0, 22.5),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.0417,
        degree_vec(90.0, 22.5, 0.0),
        Interpolation::CatmullRom,
    ),
];

static FROG_SWIM_2_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, -0.64, 2.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, pos_vec(0.0, -0.64, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.6667, pos_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.875, pos_vec(0.0, -0.27, -1.14), Interpolation::CatmullRom),
    Keyframe::new(0.9583, pos_vec(0.0, -1.45, 0.43), Interpolation::CatmullRom),
    Keyframe::new(1.0417, pos_vec(0.0, -0.64, 2.0), Interpolation::CatmullRom),
];

static FROG_SWIM_3_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(90.0, -22.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.4583,
        degree_vec(45.0, -22.5, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.6667,
        degree_vec(-22.5, 22.5, 22.5),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.875,
        degree_vec(-45.0, 22.5, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.9583,
        degree_vec(22.5, 0.0, -22.5),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.0417,
        degree_vec(90.0, -22.5, 0.0),
        Interpolation::CatmullRom,
    ),
];

static FROG_SWIM_4_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, -0.64, 2.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, pos_vec(0.0, -0.64, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.6667, pos_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.875, pos_vec(0.0, -0.27, -1.14), Interpolation::CatmullRom),
    Keyframe::new(0.9583, pos_vec(0.0, -1.45, 0.43), Interpolation::CatmullRom),
    Keyframe::new(1.0417, pos_vec(0.0, -0.64, 2.0), Interpolation::CatmullRom),
];

static FROG_SWIM_5_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, degree_vec(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.4583,
        degree_vec(67.5, -45.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.7917,
        degree_vec(90.0, 45.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.9583,
        degree_vec(90.0, 0.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.0417,
        degree_vec(90.0, 0.0, 0.0),
        Interpolation::CatmullRom,
    ),
];

static FROG_SWIM_6_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(-2.5, 0.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, pos_vec(-2.0, 0.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, pos_vec(1.0, -2.0, -1.0), Interpolation::CatmullRom),
    Keyframe::new(0.7917, pos_vec(0.58, 0.0, -2.83), Interpolation::CatmullRom),
    Keyframe::new(0.9583, pos_vec(-2.5, 0.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.0417, pos_vec(-2.5, 0.0, 1.0), Interpolation::CatmullRom),
];

static FROG_SWIM_7_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, degree_vec(90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.4583,
        degree_vec(67.5, 45.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.7917,
        degree_vec(90.0, -45.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.9583,
        degree_vec(90.0, 0.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.0417,
        degree_vec(90.0, 0.0, 0.0),
        Interpolation::CatmullRom,
    ),
];

static FROG_SWIM_8_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(2.5, 0.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, pos_vec(2.0, 0.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, pos_vec(-1.0, -2.0, -1.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.7917,
        pos_vec(-0.58, 0.0, -2.83),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.9583, pos_vec(2.5, 0.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.0417, pos_vec(2.5, 0.0, 1.0), Interpolation::CatmullRom),
];

static FROG_SWIM_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: FROG_SWIM_0_BODY_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: FROG_SWIM_1_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: FROG_SWIM_2_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: FROG_SWIM_3_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: FROG_SWIM_4_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: FROG_SWIM_5_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: FROG_SWIM_6_LEFT_LEG_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: FROG_SWIM_7_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: FROG_SWIM_8_RIGHT_LEG_POSITION,
    },
];

pub static FROG_SWIM: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.04167,
    looping: true,
    channels: FROG_SWIM_CHANNELS,
};

static FROG_IDLE_WATER_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        1.625,
        degree_vec(-10.0, 0.0, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(3.0, degree_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_1_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, -22.5), Interpolation::CatmullRom),
    Keyframe::new(
        2.2083,
        degree_vec(0.0, 0.0, -45.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(3.0, degree_vec(0.0, 0.0, -22.5), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_2_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(-1.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.2083, pos_vec(-1.0, -0.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(3.0, pos_vec(-1.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_3_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 22.5), Interpolation::CatmullRom),
    Keyframe::new(
        2.2083,
        degree_vec(0.0, 0.0, 45.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(3.0, degree_vec(0.0, 0.0, 22.5), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_4_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(1.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.2083, pos_vec(1.0, -0.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(3.0, pos_vec(1.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_5_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(22.5, -22.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        1.0,
        degree_vec(22.5, -22.5, -45.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(3.0, degree_vec(22.5, -22.5, 0.0), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_6_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, pos_vec(0.0, -1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(3.0, pos_vec(0.0, 0.0, 1.0), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_7_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(22.5, 22.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, degree_vec(22.5, 22.5, 45.0), Interpolation::CatmullRom),
    Keyframe::new(3.0, degree_vec(22.5, 22.5, 0.0), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_8_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, pos_vec(0.0, -1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(3.0, pos_vec(0.0, 0.0, 1.0), Interpolation::CatmullRom),
];

static FROG_IDLE_WATER_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: FROG_IDLE_WATER_0_BODY_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: FROG_IDLE_WATER_1_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: FROG_IDLE_WATER_2_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: FROG_IDLE_WATER_3_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: FROG_IDLE_WATER_4_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: FROG_IDLE_WATER_5_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: FROG_IDLE_WATER_6_LEFT_LEG_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: FROG_IDLE_WATER_7_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: FROG_IDLE_WATER_8_RIGHT_LEG_POSITION,
    },
];

pub static FROG_IDLE_WATER: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.0,
    looping: true,
    channels: FROG_IDLE_WATER_CHANNELS,
};
