#![allow(dead_code)]
#![allow(clippy::approx_constant, clippy::eq_op, clippy::excessive_precision)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, DEG_TO_RAD, Interpolation, Keyframe, Target,
};

static WARDEN_EMERGE_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -22.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.2,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.68,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.8,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.28,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.88,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.76,
        [25.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.92,
        [35.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.08,
        [25.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.44,
        [47.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.56,
        [47.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.68,
        [47.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.0,
        [70.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 2.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.8,
        [70.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 2.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        6.64,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_EMERGE_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 63.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.52, [0.0, 56.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.2, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.68, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.8, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.28, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.88, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.16, [0.0, 27.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.76, [0.0, 14.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.92, [0.0, 11.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(4.08, [0.0, 14.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(4.44, [0.0, 6.0, -3.0], Interpolation::CatmullRom),
    Keyframe::new(4.56, [0.0, 4.0, -3.0], Interpolation::CatmullRom),
    Keyframe::new(4.68, [0.0, 6.0, -3.0], Interpolation::CatmullRom),
    Keyframe::new(5.0, [0.0, 3.0, -4.0], Interpolation::CatmullRom),
    Keyframe::new(5.8, [0.0, 3.0, -4.0], Interpolation::CatmullRom),
    Keyframe::new(6.64, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_EMERGE_2_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.92,
        [0.74 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -40.38 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.16,
        [-67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.24,
        [-67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.32,
        [-47.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.4,
        [-67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.68,
        [-67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 15.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.76,
        [-67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.84,
        [-52.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.92,
        [-67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.64,
        [-17.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.76,
        [70.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.04,
        [70.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.12,
        [80.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.24,
        [70.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.0,
        [77.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -2.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        6.64,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_EMERGE_3_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.52, [-8.0, 11.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.92, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.24, [0.0, -0.47, -0.95], Interpolation::CatmullRom),
    Keyframe::new(1.32, [0.0, -0.47, -0.95], Interpolation::CatmullRom),
    Keyframe::new(1.4, [0.0, -0.47, -0.95], Interpolation::CatmullRom),
    Keyframe::new(1.68, [0.0, -1.0, -2.0], Interpolation::CatmullRom),
    Keyframe::new(1.76, [0.0, -1.0, -2.0], Interpolation::CatmullRom),
    Keyframe::new(1.84, [0.0, -1.0, -2.0], Interpolation::CatmullRom),
    Keyframe::new(1.92, [0.0, -1.0, -2.0], Interpolation::CatmullRom),
    Keyframe::new(2.64, [0.0, 2.0, -2.0], Interpolation::CatmullRom),
    Keyframe::new(3.76, [0.0, 4.0, 1.0], Interpolation::CatmullRom),
    Keyframe::new(4.04, [0.0, 1.0, 1.0], Interpolation::CatmullRom),
    Keyframe::new(4.12, [0.0, 1.0, 1.0], Interpolation::CatmullRom),
    Keyframe::new(4.24, [0.0, 1.0, 1.0], Interpolation::CatmullRom),
    Keyframe::new(5.0, [0.0, 1.0, 1.0], Interpolation::CatmullRom),
    Keyframe::new(6.64, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_EMERGE_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.2,
        [-152.5 * DEG_TO_RAD, 2.5 * DEG_TO_RAD, 7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.68,
        [-180.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.8,
        [-90.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.28,
        [-90.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.88,
        [-90.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.08,
        [-95.0 * DEG_TO_RAD, 12.5 * DEG_TO_RAD, -10.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.24,
        [-83.93 * DEG_TO_RAD, 3.93 * DEG_TO_RAD, 5.71 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.36,
        [-80.0 * DEG_TO_RAD, 7.5 * DEG_TO_RAD, 17.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.76,
        [-67.5 * DEG_TO_RAD, 2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.08,
        [-67.5 * DEG_TO_RAD, 2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.44,
        [-55.0 * DEG_TO_RAD, 2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.56,
        [-60.0 * DEG_TO_RAD, 2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.68,
        [-55.0 * DEG_TO_RAD, 2.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.0,
        [-67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.56,
        [-50.45 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 2.69 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        6.08,
        [-62.72 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 4.3 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        6.64,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_EMERGE_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.52, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.2, [0.0, 21.0, 9.0], Interpolation::CatmullRom),
    Keyframe::new(1.68, [2.0, 2.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.8, [2.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.28, [2.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.88, [2.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.08, [2.0, 2.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.24, [2.0, -2.71, 3.86], Interpolation::CatmullRom),
    Keyframe::new(3.36, [2.0, -1.0, 5.0], Interpolation::CatmullRom),
    Keyframe::new(3.76, [2.0, -3.0, 3.0], Interpolation::CatmullRom),
    Keyframe::new(4.08, [2.0, -3.0, 3.0], Interpolation::CatmullRom),
    Keyframe::new(4.44, [2.67, -4.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(4.56, [2.67, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(4.68, [2.67, -4.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(5.0, [0.67, -3.0, 4.0], Interpolation::CatmullRom),
    Keyframe::new(6.64, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_EMERGE_6_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.12,
        [-167.5 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, -7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.6,
        [-167.5 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, -7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.88,
        [-175.0 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, 15.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.16,
        [-190.0 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.28,
        [-90.0 * DEG_TO_RAD, -5.0 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.68,
        [-90.0 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, -12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.8,
        [-90.0 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, -12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.28,
        [-90.0 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, -12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.88,
        [-90.0 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, -12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.04,
        [
            -81.29 * DEG_TO_RAD,
            -10.64 * DEG_TO_RAD,
            -14.21 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.16,
        [-83.5 * DEG_TO_RAD, -5.5 * DEG_TO_RAD, -15.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.76,
        [-62.5 * DEG_TO_RAD, -7.5 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.92,
        [-58.75 * DEG_TO_RAD, -3.75 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.08,
        [-55.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.44,
        [-52.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.56,
        [-50.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.68,
        [-52.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.0,
        [-72.5 * DEG_TO_RAD, -2.5 * DEG_TO_RAD, 5.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.56,
        [-57.5 * DEG_TO_RAD, -4.54 * DEG_TO_RAD, 2.99 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        6.08,
        [-70.99 * DEG_TO_RAD, -5.77 * DEG_TO_RAD, 1.78 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        6.64,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_EMERGE_7_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.12, [0.0, 8.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.6, [0.0, 8.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.88, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.2, [-2.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.68, [-4.0, -3.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.8, [-4.0, -3.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.28, [-4.0, -3.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.88, [-4.0, -3.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.04, [-3.23, -5.7, 4.97], Interpolation::CatmullRom),
    Keyframe::new(3.16, [-1.49, -2.22, 5.25], Interpolation::CatmullRom),
    Keyframe::new(3.76, [-1.14, -1.71, 1.86], Interpolation::CatmullRom),
    Keyframe::new(3.92, [-1.14, -1.21, 3.86], Interpolation::CatmullRom),
    Keyframe::new(4.08, [-1.14, -2.71, 4.86], Interpolation::CatmullRom),
    Keyframe::new(4.44, [-1.0, -1.0, 3.0], Interpolation::CatmullRom),
    Keyframe::new(4.56, [0.0, -1.0, 1.0], Interpolation::CatmullRom),
    Keyframe::new(4.68, [0.0, -1.0, 3.0], Interpolation::CatmullRom),
    Keyframe::new(5.0, [-2.0, 0.0, 4.0], Interpolation::CatmullRom),
    Keyframe::new(6.64, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_EMERGE_8_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.28,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.88,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.36,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.32,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.48,
        [55.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.6,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.8,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        6.64,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_EMERGE_9_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 63.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.52, [0.0, 56.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.2, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.68, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.8, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.28, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.88, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.36, [0.0, 22.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.76, [0.0, 12.28, 2.48], Interpolation::CatmullRom),
    Keyframe::new(3.92, [0.0, 9.28, 2.48], Interpolation::CatmullRom),
    Keyframe::new(4.08, [0.0, 12.28, 2.48], Interpolation::CatmullRom),
    Keyframe::new(4.32, [0.0, 4.14, 4.14], Interpolation::CatmullRom),
    Keyframe::new(4.48, [0.0, 0.57, -8.43], Interpolation::CatmullRom),
    Keyframe::new(4.6, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(5.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(5.8, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(6.64, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_EMERGE_10_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.28,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.88,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.36,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.84,
        [20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -17.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.68,
        [20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.84,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.8,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        6.64,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_EMERGE_11_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 63.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.52, [0.0, 56.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.2, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.68, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.8, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.28, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.88, [0.0, 32.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.36, [0.0, 22.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.84, [-4.0, -2.0, -7.0], Interpolation::CatmullRom),
    Keyframe::new(4.0, [-4.0, 0.0, -5.0], Interpolation::CatmullRom),
    Keyframe::new(4.68, [-4.0, 0.0, -9.0], Interpolation::CatmullRom),
    Keyframe::new(4.84, [-2.0, -2.0, -3.5], Interpolation::CatmullRom),
    Keyframe::new(5.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(5.8, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(6.64, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_EMERGE_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: WARDEN_EMERGE_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: WARDEN_EMERGE_1_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: WARDEN_EMERGE_2_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: WARDEN_EMERGE_3_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: WARDEN_EMERGE_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: WARDEN_EMERGE_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: WARDEN_EMERGE_6_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: WARDEN_EMERGE_7_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: WARDEN_EMERGE_8_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: WARDEN_EMERGE_9_RIGHT_LEG_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: WARDEN_EMERGE_10_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: WARDEN_EMERGE_11_LEFT_LEG_POSITION,
    },
];

pub static WARDEN_EMERGE: AnimationDefinition = AnimationDefinition {
    length_seconds: 6.68,
    looping: false,
    channels: WARDEN_EMERGE_CHANNELS,
};

static WARDEN_DIG_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.25,
        [
            4.13441 * DEG_TO_RAD,
            0.94736 * DEG_TO_RAD,
            1.2694 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.5,
        [50.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.7083,
        [
            54.45407 * DEG_TO_RAD,
            -13.53935 * DEG_TO_RAD,
            -18.14183 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.0417,
        [
            59.46442 * DEG_TO_RAD,
            -10.8885 * DEG_TO_RAD,
            35.7954 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.3333,
        [82.28261 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.625,
        [
            53.23606 * DEG_TO_RAD,
            10.04715 * DEG_TO_RAD,
            -29.72932 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.2083,
        [-17.71739 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5417,
        [112.28261 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.6667,
        [
            116.06889 * DEG_TO_RAD,
            5.11581 * DEG_TO_RAD,
            -24.50117 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.8333,
        [
            121.56244 * DEG_TO_RAD,
            -4.17248 * DEG_TO_RAD,
            19.57737 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.0417,
        [
            138.5689 * DEG_TO_RAD,
            5.11581 * DEG_TO_RAD,
            -24.50117 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.25,
        [
            144.06244 * DEG_TO_RAD,
            -4.17248 * DEG_TO_RAD,
            19.57737 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.375,
        [147.28261 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.625,
        [147.28261 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.875,
        [
            134.36221 * DEG_TO_RAD,
            8.81113 * DEG_TO_RAD,
            -8.90172 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.0417,
        [
            132.05966 * DEG_TO_RAD,
            -8.35927 * DEG_TO_RAD,
            9.70506 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.25,
        [
            134.36221 * DEG_TO_RAD,
            8.81113 * DEG_TO_RAD,
            -8.90172 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.5,
        [147.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static WARDEN_DIG_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.5, [0.0, 16.48454, -6.5784], Interpolation::CatmullRom),
    Keyframe::new(0.7083, [0.0, 16.48454, -6.5784], Interpolation::CatmullRom),
    Keyframe::new(1.0417, [0.0, 16.97, -7.11], Interpolation::CatmullRom),
    Keyframe::new(1.625, [0.0, 13.97, -7.11], Interpolation::CatmullRom),
    Keyframe::new(2.2083, [0.0, 11.48454, -0.5784], Interpolation::CatmullRom),
    Keyframe::new(2.5417, [0.0, 16.48454, -6.5784], Interpolation::CatmullRom),
    Keyframe::new(2.6667, [0.0, 20.27, -5.42], Interpolation::CatmullRom),
    Keyframe::new(3.375, [0.0, 21.48454, -5.5784], Interpolation::CatmullRom),
    Keyframe::new(4.0417, [0.0, 22.48454, -5.5784], Interpolation::CatmullRom),
    Keyframe::new(4.5, [0.0, 40.0, -8.0], Interpolation::Linear),
];

static WARDEN_DIG_2_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.6667,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.2083,
        [12.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.75,
        [45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.375,
        [-22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5417,
        [67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.375,
        [67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_DIG_3_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(4.375, [0.0, 0.0, 0.0], Interpolation::Linear),
];

static WARDEN_DIG_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.5,
        [
            -101.8036 * DEG_TO_RAD,
            -21.29587 * DEG_TO_RAD,
            30.61478 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.7083,
        [
            -101.8036 * DEG_TO_RAD,
            -21.29587 * DEG_TO_RAD,
            30.61478 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.0,
        [
            48.7585 * DEG_TO_RAD,
            -17.61941 * DEG_TO_RAD,
            9.9865 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.1667,
        [
            48.7585 * DEG_TO_RAD,
            -17.61941 * DEG_TO_RAD,
            9.9865 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.4583,
        [
            -101.8036 * DEG_TO_RAD,
            -21.29587 * DEG_TO_RAD,
            30.61478 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.75,
        [
            -89.04994 * DEG_TO_RAD,
            -4.19657 * DEG_TO_RAD,
            -1.47845 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.2083,
        [
            -158.30728 * DEG_TO_RAD,
            3.7152 * DEG_TO_RAD,
            -1.52352 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5417,
        [
            -89.04994 * DEG_TO_RAD,
            -4.19657 * DEG_TO_RAD,
            -1.47845 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.375,
        [-120.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_DIG_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.7083, [2.22, 0.0, 0.86], Interpolation::CatmullRom),
    Keyframe::new(1.0, [3.12, 0.0, 4.29], Interpolation::CatmullRom),
    Keyframe::new(2.2083, [1.0, 0.0, 4.0], Interpolation::CatmullRom),
    Keyframe::new(4.375, [0.0, 0.0, 4.0], Interpolation::CatmullRom),
];

static WARDEN_DIG_6_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.2917,
        [
            -63.89288 * DEG_TO_RAD,
            -0.52011 * DEG_TO_RAD,
            2.09491 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.5,
        [
            -63.89288 * DEG_TO_RAD,
            -0.52011 * DEG_TO_RAD,
            2.09491 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.7083,
        [
            -62.87857 * DEG_TO_RAD,
            15.15061 * DEG_TO_RAD,
            9.97445 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.9167,
        [
            -86.93642 * DEG_TO_RAD,
            17.45026 * DEG_TO_RAD,
            4.05284 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.1667,
        [
            -86.93642 * DEG_TO_RAD,
            17.45026 * DEG_TO_RAD,
            4.05284 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.4583,
        [
            -86.93642 * DEG_TO_RAD,
            17.45026 * DEG_TO_RAD,
            4.05284 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.6667,
        [
            63.0984 * DEG_TO_RAD,
            8.83573 * DEG_TO_RAD,
            -8.71284 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.8333,
        [
            35.5984 * DEG_TO_RAD,
            8.83573 * DEG_TO_RAD,
            -8.71284 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.2083,
        [
            -153.27473 * DEG_TO_RAD,
            -0.02953 * DEG_TO_RAD,
            3.5235 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5417,
        [
            -87.07754 * DEG_TO_RAD,
            -0.02625 * DEG_TO_RAD,
            3.132 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.375,
        [-120.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static WARDEN_DIG_7_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.5, [-0.28, -5.0, 10.0], Interpolation::CatmullRom),
    Keyframe::new(0.7083, [-1.51, -4.35, 4.33], Interpolation::CatmullRom),
    Keyframe::new(0.9167, [-0.6, -3.61, 4.63], Interpolation::CatmullRom),
    Keyframe::new(1.1667, [-0.6, -3.61, 0.63], Interpolation::CatmullRom),
    Keyframe::new(1.6667, [-2.85, 0.1, 3.33], Interpolation::CatmullRom),
    Keyframe::new(2.2083, [-1.0, 0.0, 4.0], Interpolation::CatmullRom),
    Keyframe::new(4.375, [0.0, 0.0, 4.0], Interpolation::Linear),
];

static WARDEN_DIG_8_RIGHT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.5,
        [113.27 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.7083,
        [113.27 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.3333,
        [113.27 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.5833,
        [182.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.8333,
        [120.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.0833,
        [182.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.2917,
        [120.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.5,
        [90.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static WARDEN_DIG_9_RIGHT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.5, [0.0, 13.98, -2.37], Interpolation::CatmullRom),
    Keyframe::new(0.7083, [0.0, 13.98, -2.37], Interpolation::CatmullRom),
    Keyframe::new(3.3333, [0.0, 13.98, -2.37], Interpolation::CatmullRom),
    Keyframe::new(3.5833, [0.0, 7.0, -3.0], Interpolation::CatmullRom),
    Keyframe::new(3.8333, [0.0, 9.0, -3.0], Interpolation::CatmullRom),
    Keyframe::new(4.0833, [0.0, 16.71, -3.69], Interpolation::CatmullRom),
    Keyframe::new(4.2917, [0.0, 28.0, -5.0], Interpolation::Linear),
];

static WARDEN_DIG_10_LEFT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.5,
        [114.98 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.7083,
        [114.98 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.3333,
        [114.98 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.5833,
        [90.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.8333,
        [172.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.0833,
        [90.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.2917,
        [197.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.5,
        [90.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::Linear,
    ),
];

static WARDEN_DIG_11_LEFT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.5, [0.0, 14.01, -2.35], Interpolation::CatmullRom),
    Keyframe::new(0.7083, [0.0, 14.01, -2.35], Interpolation::CatmullRom),
    Keyframe::new(3.3333, [0.0, 14.01, -2.35], Interpolation::CatmullRom),
    Keyframe::new(3.5833, [0.0, 5.0, -4.0], Interpolation::CatmullRom),
    Keyframe::new(3.8333, [0.0, 7.0, -4.0], Interpolation::CatmullRom),
    Keyframe::new(4.0833, [0.0, 15.5, -3.76], Interpolation::CatmullRom),
    Keyframe::new(4.2917, [0.0, 28.0, -5.0], Interpolation::Linear),
];

static WARDEN_DIG_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: WARDEN_DIG_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: WARDEN_DIG_1_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: WARDEN_DIG_2_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: WARDEN_DIG_3_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: WARDEN_DIG_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: WARDEN_DIG_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: WARDEN_DIG_6_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: WARDEN_DIG_7_LEFT_ARM_POSITION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Rotation,
        keyframes: WARDEN_DIG_8_RIGHT_LEG_ROTATION,
    },
    Channel {
        bone: "right_leg",
        target: Target::Position,
        keyframes: WARDEN_DIG_9_RIGHT_LEG_POSITION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Rotation,
        keyframes: WARDEN_DIG_10_LEFT_LEG_ROTATION,
    },
    Channel {
        bone: "left_leg",
        target: Target::Position,
        keyframes: WARDEN_DIG_11_LEFT_LEG_POSITION,
    },
];

pub static WARDEN_DIG: AnimationDefinition = AnimationDefinition {
    length_seconds: 5.0,
    looping: false,
    channels: WARDEN_DIG_CHANNELS,
};

static WARDEN_ROAR_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.24,
        [-25.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.6,
        [32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.84,
        [38.33 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 2.99 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.08,
        [40.97 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -4.3 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.36,
        [44.41 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 6.29 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.0,
        [47.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.2,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_ROAR_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.24, [0.0, 1.0, 3.0], Interpolation::CatmullRom),
    Keyframe::new(1.6, [0.0, 3.0, -6.0], Interpolation::CatmullRom),
    Keyframe::new(3.0, [0.0, 3.0, -6.0], Interpolation::CatmullRom),
    Keyframe::new(4.2, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_ROAR_2_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.24,
        [-32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.6,
        [-32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -27.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.8,
        [-32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 26.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.04,
        [-32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -27.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.44,
        [-32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 26.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.84,
        [-5.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.2,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_ROAR_3_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.24, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.6, [0.0, 2.0, -6.0], Interpolation::CatmullRom),
    Keyframe::new(2.2, [0.0, 2.0, -6.0], Interpolation::CatmullRom),
    Keyframe::new(2.48, [0.0, 2.0, -6.0], Interpolation::CatmullRom),
    Keyframe::new(4.2, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_ROAR_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.72,
        [-120.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.24,
        [-77.5 * DEG_TO_RAD, 3.75 * DEG_TO_RAD, 15.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.48,
        [67.5 * DEG_TO_RAD, -32.5 * DEG_TO_RAD, 20.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.48,
        [37.5 * DEG_TO_RAD, -32.5 * DEG_TO_RAD, 25.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.88,
        [27.6 * DEG_TO_RAD, -17.1 * DEG_TO_RAD, 32.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.2,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_ROAR_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.72, [3.0, 2.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.48, [4.0, 2.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.48, [4.0, 2.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(4.2, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_ROAR_6_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.72,
        [-125.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 20.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.24,
        [-76.25 * DEG_TO_RAD, -17.5 * DEG_TO_RAD, -7.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.48,
        [62.5 * DEG_TO_RAD, 42.5 * DEG_TO_RAD, -12.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.48,
        [37.5 * DEG_TO_RAD, 27.5 * DEG_TO_RAD, -27.5 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.88,
        [25.0 * DEG_TO_RAD, 18.4 * DEG_TO_RAD, -30.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.2,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_ROAR_7_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.72, [-3.0, 2.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.48, [-4.0, 2.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.48, [-4.0, 2.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(4.2, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_ROAR_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: WARDEN_ROAR_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: WARDEN_ROAR_1_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: WARDEN_ROAR_2_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: WARDEN_ROAR_3_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: WARDEN_ROAR_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: WARDEN_ROAR_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: WARDEN_ROAR_6_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: WARDEN_ROAR_7_LEFT_ARM_POSITION,
    },
];

pub static WARDEN_ROAR: AnimationDefinition = AnimationDefinition {
    length_seconds: 4.2,
    looping: false,
    channels: WARDEN_ROAR_CHANNELS,
};

static WARDEN_SNIFF_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.56,
        [17.5 * DEG_TO_RAD, 32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.96,
        [0.0 * DEG_TO_RAD, 32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.2,
        [10.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.8,
        [10.0 * DEG_TO_RAD, -30.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.32,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SNIFF_1_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.68,
        [0.0 * DEG_TO_RAD, 40.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.96,
        [-22.5 * DEG_TO_RAD, 40.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.24,
        [0.0 * DEG_TO_RAD, 20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.52,
        [-35.0 * DEG_TO_RAD, 20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.76,
        [0.0 * DEG_TO_RAD, 20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.28,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.88,
        [0.0 * DEG_TO_RAD, -20.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.32,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SNIFF_2_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.96,
        [17.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.2,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.76,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.32,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SNIFF_3_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.96,
        [-15.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.2,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.76,
        [17.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.32,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SNIFF_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: WARDEN_SNIFF_0_BODY_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: WARDEN_SNIFF_1_HEAD_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: WARDEN_SNIFF_2_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: WARDEN_SNIFF_3_LEFT_ARM_ROTATION,
    },
];

pub static WARDEN_SNIFF: AnimationDefinition = AnimationDefinition {
    length_seconds: 4.16,
    looping: false,
    channels: WARDEN_SNIFF_CHANNELS,
};

static WARDEN_ATTACK_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.0417,
        [-22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.2083,
        [22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.3333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_ATTACK_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.0417, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.2083, [0.0, 1.0, -2.0], Interpolation::CatmullRom),
    Keyframe::new(0.3333, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_ATTACK_2_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.0417,
        [22.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.25,
        [-30.17493 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.3333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_ATTACK_3_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.0417, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.25, [0.0, 2.0, -2.0], Interpolation::CatmullRom),
    Keyframe::new(0.3333, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_ATTACK_4_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.0417,
        [
            -120.36119 * DEG_TO_RAD,
            40.78947 * DEG_TO_RAD,
            -20.94102 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.1667,
        [-90.0 * DEG_TO_RAD, -45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.3333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_ATTACK_5_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.0417, [4.0, 0.0, 5.0], Interpolation::CatmullRom),
    Keyframe::new(0.1667, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.3333, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_ATTACK_6_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.0417,
        [
            -120.36119 * DEG_TO_RAD,
            -40.78947 * DEG_TO_RAD,
            20.94102 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.1667,
        [
            -61.1632 * DEG_TO_RAD,
            42.85882 * DEG_TO_RAD,
            11.52421 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.3333,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_ATTACK_7_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.0417, [-4.0, 0.0, 5.0], Interpolation::CatmullRom),
    Keyframe::new(0.1667, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(0.3333, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_ATTACK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: WARDEN_ATTACK_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: WARDEN_ATTACK_1_BODY_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: WARDEN_ATTACK_2_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: WARDEN_ATTACK_3_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: WARDEN_ATTACK_4_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: WARDEN_ATTACK_5_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: WARDEN_ATTACK_6_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: WARDEN_ATTACK_7_LEFT_ARM_POSITION,
    },
];

pub static WARDEN_ATTACK: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.33333,
    looping: false,
    channels: WARDEN_ATTACK_CHANNELS,
};

static WARDEN_SONIC_BOOM_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.0833,
        [47.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.625,
        [55.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.9167,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.0,
        [-32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.4583,
        [-32.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.7083,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SONIC_BOOM_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.0833, [0.0, 3.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.625, [0.0, 4.0, -1.0], Interpolation::CatmullRom),
    Keyframe::new(1.9167, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.7083, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_SONIC_BOOM_2_RIGHT_RIBCAGE_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.5417,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.7917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.875,
        [0.0 * DEG_TO_RAD, 125.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5,
        [0.0 * DEG_TO_RAD, 125.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.6667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SONIC_BOOM_3_LEFT_RIBCAGE_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.5417,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.7917,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.875,
        [0.0 * DEG_TO_RAD, -125.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5,
        [0.0 * DEG_TO_RAD, -125.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.6667,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SONIC_BOOM_4_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.0,
        [67.5 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.75,
        [80.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.9167,
        [-45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5,
        [-45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.7083,
        [-45.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.875,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SONIC_BOOM_5_HEAD_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.9167, [0.0, 0.0, -3.0], Interpolation::CatmullRom),
    Keyframe::new(2.5, [0.0, 0.0, -3.0], Interpolation::CatmullRom),
    Keyframe::new(2.7083, [0.0, 0.0, -3.0], Interpolation::CatmullRom),
    Keyframe::new(2.875, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_SONIC_BOOM_6_RIGHT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.875,
        [
            -42.28659 * DEG_TO_RAD,
            -32.69813 * DEG_TO_RAD,
            -5.00825 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.1667,
        [
            -29.83757 * DEG_TO_RAD,
            -35.39626 * DEG_TO_RAD,
            -45.28089 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.3333,
        [
            -29.83757 * DEG_TO_RAD,
            -35.39626 * DEG_TO_RAD,
            -45.28089 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.6667,
        [
            -72.28659 * DEG_TO_RAD,
            -32.69813 * DEG_TO_RAD,
            -5.00825 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.8333,
        [
            35.26439 * DEG_TO_RAD,
            -30.0 * DEG_TO_RAD,
            35.26439 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.9167,
        [
            73.75484 * DEG_TO_RAD,
            -13.0931 * DEG_TO_RAD,
            19.20518 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5,
        [
            73.75484 * DEG_TO_RAD,
            -13.0931 * DEG_TO_RAD,
            19.20518 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.75,
        [
            58.20713 * DEG_TO_RAD,
            -21.1064 * DEG_TO_RAD,
            28.7261 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SONIC_BOOM_7_RIGHT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.8333, [3.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.75, [3.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_SONIC_BOOM_8_LEFT_ARM_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.875,
        [
            -33.80694 * DEG_TO_RAD,
            32.31058 * DEG_TO_RAD,
            6.87997 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.1667,
        [
            -17.87827 * DEG_TO_RAD,
            34.62115 * DEG_TO_RAD,
            49.02433 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.3333,
        [
            -17.87827 * DEG_TO_RAD,
            34.62115 * DEG_TO_RAD,
            49.02433 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.6667,
        [
            -51.30694 * DEG_TO_RAD,
            32.31058 * DEG_TO_RAD,
            6.87997 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.8333,
        [
            35.26439 * DEG_TO_RAD,
            30.0 * DEG_TO_RAD,
            -35.26439 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.9167,
        [
            73.75484 * DEG_TO_RAD,
            13.0931 * DEG_TO_RAD,
            -19.20518 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.5,
        [
            73.75484 * DEG_TO_RAD,
            13.0931 * DEG_TO_RAD,
            -19.20518 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.75,
        [
            58.20713 * DEG_TO_RAD,
            21.1064 * DEG_TO_RAD,
            -28.7261 * DEG_TO_RAD,
        ],
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        3.0,
        [0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD, 0.0 * DEG_TO_RAD],
        Interpolation::CatmullRom,
    ),
];

static WARDEN_SONIC_BOOM_9_LEFT_ARM_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(1.8333, [-3.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(2.75, [-3.0, 0.0, 0.0], Interpolation::CatmullRom),
    Keyframe::new(3.0, [0.0, 0.0, 0.0], Interpolation::CatmullRom),
];

static WARDEN_SONIC_BOOM_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: WARDEN_SONIC_BOOM_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: WARDEN_SONIC_BOOM_1_BODY_POSITION,
    },
    Channel {
        bone: "right_ribcage",
        target: Target::Rotation,
        keyframes: WARDEN_SONIC_BOOM_2_RIGHT_RIBCAGE_ROTATION,
    },
    Channel {
        bone: "left_ribcage",
        target: Target::Rotation,
        keyframes: WARDEN_SONIC_BOOM_3_LEFT_RIBCAGE_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: WARDEN_SONIC_BOOM_4_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: WARDEN_SONIC_BOOM_5_HEAD_POSITION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Rotation,
        keyframes: WARDEN_SONIC_BOOM_6_RIGHT_ARM_ROTATION,
    },
    Channel {
        bone: "right_arm",
        target: Target::Position,
        keyframes: WARDEN_SONIC_BOOM_7_RIGHT_ARM_POSITION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Rotation,
        keyframes: WARDEN_SONIC_BOOM_8_LEFT_ARM_ROTATION,
    },
    Channel {
        bone: "left_arm",
        target: Target::Position,
        keyframes: WARDEN_SONIC_BOOM_9_LEFT_ARM_POSITION,
    },
];

pub static WARDEN_SONIC_BOOM: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.0,
    looping: false,
    channels: WARDEN_SONIC_BOOM_CHANNELS,
};
