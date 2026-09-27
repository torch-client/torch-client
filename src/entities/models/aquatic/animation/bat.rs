#![allow(clippy::excessive_precision)]

use crate::entities::keyframe::{AnimationDefinition, Channel, Interpolation, Keyframe, Target};
use crate::entities::models::aquatic::animation::{degree_vec, pos_vec};

static BAT_RESTING_0_HEAD_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(180.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_1_HEAD_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.5, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_2_BODY_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(180.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_3_BODY_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.5, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_4_FEET_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_5_RIGHT_WING_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, -10.0, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_6_RIGHT_WING_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 1.0),
    Interpolation::Linear,
)];

static BAT_RESTING_7_RIGHT_WING_TIP_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, -120.0, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_8_LEFT_WING_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, 10.0, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_9_LEFT_WING_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 1.0),
    Interpolation::Linear,
)];

static BAT_RESTING_10_LEFT_WING_TIP_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, 120.0, 0.0),
    Interpolation::Linear,
)];

static BAT_RESTING_CHANNELS: &[Channel] = &[
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: BAT_RESTING_0_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: BAT_RESTING_1_HEAD_POSITION,
    },
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: BAT_RESTING_2_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: BAT_RESTING_3_BODY_POSITION,
    },
    Channel {
        bone: "feet",
        target: Target::Rotation,
        keyframes: BAT_RESTING_4_FEET_ROTATION,
    },
    Channel {
        bone: "right_wing",
        target: Target::Rotation,
        keyframes: BAT_RESTING_5_RIGHT_WING_ROTATION,
    },
    Channel {
        bone: "right_wing",
        target: Target::Position,
        keyframes: BAT_RESTING_6_RIGHT_WING_POSITION,
    },
    Channel {
        bone: "right_wing_tip",
        target: Target::Rotation,
        keyframes: BAT_RESTING_7_RIGHT_WING_TIP_ROTATION,
    },
    Channel {
        bone: "left_wing",
        target: Target::Rotation,
        keyframes: BAT_RESTING_8_LEFT_WING_ROTATION,
    },
    Channel {
        bone: "left_wing",
        target: Target::Position,
        keyframes: BAT_RESTING_9_LEFT_WING_POSITION,
    },
    Channel {
        bone: "left_wing_tip",
        target: Target::Rotation,
        keyframes: BAT_RESTING_10_LEFT_WING_TIP_ROTATION,
    },
];

pub static BAT_RESTING: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: true,
    channels: BAT_RESTING_CHANNELS,
};

static BAT_FLYING_0_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, degree_vec(20.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static BAT_FLYING_1_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, pos_vec(0.0, 2.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos_vec(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, pos_vec(0.0, -1.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static BAT_FLYING_2_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(40.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, degree_vec(52.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(40.0, 0.0, 0.0), Interpolation::Linear),
];

static BAT_FLYING_3_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, pos_vec(0.0, 2.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, pos_vec(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.4583, pos_vec(0.0, -1.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, pos_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static BAT_FLYING_4_FEET_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, degree_vec(-21.25, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, degree_vec(-12.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(10.0, 0.0, 0.0), Interpolation::Linear),
];

static BAT_FLYING_5_RIGHT_WING_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 85.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, degree_vec(0.0, -55.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, degree_vec(0.0, 50.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, degree_vec(0.0, 70.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(0.0, 85.0, 0.0), Interpolation::Linear),
];

static BAT_FLYING_6_RIGHT_WING_TIP_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 10.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.0417, degree_vec(0.0, 65.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, degree_vec(0.0, -135.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(0.0, 10.5, 0.0), Interpolation::Linear),
];

static BAT_FLYING_7_LEFT_WING_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -85.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, degree_vec(0.0, 55.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.25, degree_vec(0.0, -50.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, degree_vec(0.0, -70.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(0.0, -85.0, 0.0), Interpolation::Linear),
];

static BAT_FLYING_8_LEFT_WING_TIP_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -10.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.0417, degree_vec(0.0, -65.5, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, degree_vec(0.0, 135.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(0.0, -10.5, 0.0), Interpolation::Linear),
];

static BAT_FLYING_CHANNELS: &[Channel] = &[
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: BAT_FLYING_0_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: BAT_FLYING_1_HEAD_POSITION,
    },
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: BAT_FLYING_2_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: BAT_FLYING_3_BODY_POSITION,
    },
    Channel {
        bone: "feet",
        target: Target::Rotation,
        keyframes: BAT_FLYING_4_FEET_ROTATION,
    },
    Channel {
        bone: "right_wing",
        target: Target::Rotation,
        keyframes: BAT_FLYING_5_RIGHT_WING_ROTATION,
    },
    Channel {
        bone: "right_wing_tip",
        target: Target::Rotation,
        keyframes: BAT_FLYING_6_RIGHT_WING_TIP_ROTATION,
    },
    Channel {
        bone: "left_wing",
        target: Target::Rotation,
        keyframes: BAT_FLYING_7_LEFT_WING_ROTATION,
    },
    Channel {
        bone: "left_wing_tip",
        target: Target::Rotation,
        keyframes: BAT_FLYING_8_LEFT_WING_TIP_ROTATION,
    },
];

pub static BAT_FLYING: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.5,
    looping: true,
    channels: BAT_FLYING_CHANNELS,
};
