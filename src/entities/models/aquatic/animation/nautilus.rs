#![allow(clippy::excessive_precision)]

use crate::entities::keyframe::{AnimationDefinition, Channel, Interpolation, Keyframe, Target};
use crate::entities::models::aquatic::animation::{degree_vec, scale_vec};

static SWIMMING_0_BODY_SCALE: &[Keyframe] = &[
    Keyframe::new(0.0, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        0.5,
        scale_vec(1.0, 1.0, 1.2000000476837158),
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        scale_vec(1.0, 1.0, 0.8999999761581421),
        Interpolation::Linear,
    ),
    Keyframe::new(0.875, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.0, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
];

static SWIMMING_1_UPPER_MOUTH_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.875, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SWIMMING_2_UPPER_MOUTH_SCALE: &[Keyframe] = &[
    Keyframe::new(0.0, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        0.5,
        scale_vec(1.0, 1.0, 1.399999976158142),
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        scale_vec(1.0, 1.0, 0.8999999761581421),
        Interpolation::Linear,
    ),
    Keyframe::new(0.875, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.0, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
];

static SWIMMING_3_INNER_MOUTH_SCALE: &[Keyframe] = &[
    Keyframe::new(0.0, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        0.5,
        scale_vec(0.800000011920929, 0.800000011920929, 1.0),
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        scale_vec(1.0, 1.0, 0.8999999761581421),
        Interpolation::Linear,
    ),
    Keyframe::new(0.875, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.0, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
];

static SWIMMING_4_LOWER_MOUTH_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, degree_vec(-30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.875, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SWIMMING_5_LOWER_MOUTH_SCALE: &[Keyframe] = &[
    Keyframe::new(0.0, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        0.5,
        scale_vec(1.0, 1.0, 1.399999976158142),
        Interpolation::Linear,
    ),
    Keyframe::new(
        0.75,
        scale_vec(1.0, 1.0, 0.8999999761581421),
        Interpolation::Linear,
    ),
    Keyframe::new(0.875, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.0, scale_vec(1.0, 1.0, 1.0), Interpolation::Linear),
];

static SWIMMING_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Scale,
        keyframes: SWIMMING_0_BODY_SCALE,
    },
    Channel {
        bone: "upper_mouth",
        target: Target::Rotation,
        keyframes: SWIMMING_1_UPPER_MOUTH_ROTATION,
    },
    Channel {
        bone: "upper_mouth",
        target: Target::Scale,
        keyframes: SWIMMING_2_UPPER_MOUTH_SCALE,
    },
    Channel {
        bone: "inner_mouth",
        target: Target::Scale,
        keyframes: SWIMMING_3_INNER_MOUTH_SCALE,
    },
    Channel {
        bone: "lower_mouth",
        target: Target::Rotation,
        keyframes: SWIMMING_4_LOWER_MOUTH_ROTATION,
    },
    Channel {
        bone: "lower_mouth",
        target: Target::Scale,
        keyframes: SWIMMING_5_LOWER_MOUTH_SCALE,
    },
];

pub static SWIMMING: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.0,
    looping: true,
    channels: SWIMMING_CHANNELS,
};
