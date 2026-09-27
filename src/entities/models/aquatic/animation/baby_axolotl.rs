#![allow(clippy::excessive_precision)]

use crate::entities::keyframe::{AnimationDefinition, Channel, Interpolation, Keyframe, Target};
use crate::entities::models::aquatic::animation::{degree_vec, pos_vec, scale_vec};

static BABY_AXOLOTL_IDLE_FLOOR_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.8, degree_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_IDLE_FLOOR_1_RIGHT_FRONT_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(160.0, -10.0, -37.5),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_IDLE_FLOOR_2_RIGHT_FRONT_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.2, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_IDLE_FLOOR_3_RIGHT_HIND_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    1.72,
    degree_vec(360.0, 0.0, -45.0),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_IDLE_FLOOR_4_LEFT_FRONT_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(160.0, 10.0, 37.5),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_IDLE_FLOOR_5_LEFT_FRONT_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(-0.2, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_IDLE_FLOOR_6_LEFT_HIND_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, 0.0, 37.5),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_IDLE_FLOOR_7_TAIL_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 10.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.72, degree_vec(0.0, -14.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.96, degree_vec(0.0, -21.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4, degree_vec(0.0, -25.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        2.04,
        degree_vec(0.0, -16.75, 0.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(2.84, degree_vec(0.0, 10.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_IDLE_FLOOR_8_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.04, degree_vec(0.0, 0.0, -6.0), Interpolation::CatmullRom),
    Keyframe::new(1.32, degree_vec(-6.45, -1.45, -6.5), Interpolation::Linear),
    Keyframe::new(1.48, degree_vec(-6.45, -1.45, -6.5), Interpolation::Linear),
    Keyframe::new(2.84, degree_vec(0.0, 0.0, -6.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_IDLE_FLOOR_9_LEFT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 38.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.28, degree_vec(0.0, 45.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.64, degree_vec(0.0, 45.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.8, degree_vec(0.0, 38.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_IDLE_FLOOR_10_RIGHT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.04, degree_vec(0.0, -50.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.32, degree_vec(0.0, -59.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.6, degree_vec(0.0, -59.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.84, degree_vec(0.0, -50.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_IDLE_FLOOR_11_TOP_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(33.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.28, degree_vec(47.4, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.64, degree_vec(47.4, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.8, degree_vec(33.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_IDLE_FLOOR_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_0_BODY_ROTATION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_1_RIGHT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_2_RIGHT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_3_RIGHT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_4_LEFT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_5_LEFT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_6_LEFT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_7_TAIL_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_8_HEAD_ROTATION,
    },
    Channel {
        bone: "left_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_9_LEFT_GILLS_ROTATION,
    },
    Channel {
        bone: "right_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_10_RIGHT_GILLS_ROTATION,
    },
    Channel {
        bone: "top_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_IDLE_FLOOR_11_TOP_GILLS_ROTATION,
    },
];

pub static BABY_AXOLOTL_IDLE_FLOOR: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.88,
    looping: true,
    channels: BABY_AXOLOTL_IDLE_FLOOR_CHANNELS,
};

static IDLE_FLOOR_UNDERWATER_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.12, degree_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_FLOOR_UNDERWATER_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 1.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.12, pos_vec(0.0, 1.5, 0.0), Interpolation::CatmullRom),
];

static IDLE_FLOOR_UNDERWATER_2_RIGHT_FRONT_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    5.04,
    degree_vec(190.0, -15.0, -30.0),
    Interpolation::CatmullRom,
)];

static IDLE_FLOOR_UNDERWATER_3_RIGHT_FRONT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.3, 0.0, 0.5), Interpolation::CatmullRom),
    Keyframe::new(5.12, pos_vec(0.3, 0.0, 0.5), Interpolation::CatmullRom),
];

static IDLE_FLOOR_UNDERWATER_4_RIGHT_HIND_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(360.0, 0.0, -30.0),
    Interpolation::CatmullRom,
)];

static IDLE_FLOOR_UNDERWATER_5_RIGHT_HIND_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.18, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(5.12, pos_vec(0.18, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_FLOOR_UNDERWATER_6_LEFT_FRONT_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    5.04,
    degree_vec(190.0, 18.0, 30.0),
    Interpolation::CatmullRom,
)];

static IDLE_FLOOR_UNDERWATER_7_LEFT_FRONT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(-0.1, 0.0, 0.5), Interpolation::CatmullRom),
    Keyframe::new(5.12, pos_vec(-0.1, 0.0, 0.5), Interpolation::CatmullRom),
];

static IDLE_FLOOR_UNDERWATER_8_LEFT_HIND_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    5.04,
    degree_vec(180.0, 0.0, 30.0),
    Interpolation::CatmullRom,
)];

static IDLE_FLOOR_UNDERWATER_9_LEFT_HIND_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(-0.2, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(5.12, pos_vec(-0.2, 0.0, 0.0), Interpolation::Linear),
];

static IDLE_FLOOR_UNDERWATER_10_TAIL_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.04, degree_vec(0.0, -12.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.56, degree_vec(0.0, 12.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.12, degree_vec(0.0, -12.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_FLOOR_UNDERWATER_11_LEFT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 10.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(3.12, degree_vec(0.0, -16.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.12, degree_vec(0.0, 10.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_FLOOR_UNDERWATER_12_RIGHT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -16.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.08, degree_vec(0.0, 12.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.12, degree_vec(0.0, -16.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_FLOOR_UNDERWATER_13_TOP_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(10.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.56, degree_vec(-16.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.12, degree_vec(10.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_FLOOR_UNDERWATER_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: IDLE_FLOOR_UNDERWATER_1_BODY_POSITION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_2_RIGHT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: IDLE_FLOOR_UNDERWATER_3_RIGHT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_4_RIGHT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: IDLE_FLOOR_UNDERWATER_5_RIGHT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_6_LEFT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: IDLE_FLOOR_UNDERWATER_7_LEFT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_8_LEFT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: IDLE_FLOOR_UNDERWATER_9_LEFT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_10_TAIL_ROTATION,
    },
    Channel {
        bone: "left_gills",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_11_LEFT_GILLS_ROTATION,
    },
    Channel {
        bone: "right_gills",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_12_RIGHT_GILLS_ROTATION,
    },
    Channel {
        bone: "top_gills",
        target: Target::Rotation,
        keyframes: IDLE_FLOOR_UNDERWATER_13_TOP_GILLS_ROTATION,
    },
];

pub static IDLE_FLOOR_UNDERWATER: AnimationDefinition = AnimationDefinition {
    length_seconds: 5.12,
    looping: true,
    channels: IDLE_FLOOR_UNDERWATER_CHANNELS,
};

static IDLE_UNDERWATER_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-8.9, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.6, degree_vec(-2.6, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.2, degree_vec(-8.9, 0.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_UNDERWATER_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.6, pos_vec(0.0, -0.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.2, pos_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_UNDERWATER_2_RIGHT_FRONT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(-5.0, -20.0, -55.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.6,
        degree_vec(1.0, -20.0, -65.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.2,
        degree_vec(-5.0, -20.0, -55.0),
        Interpolation::CatmullRom,
    ),
];

static IDLE_UNDERWATER_3_RIGHT_FRONT_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.15, -0.3391, 0.0876),
    Interpolation::CatmullRom,
)];

static IDLE_UNDERWATER_4_RIGHT_HIND_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(60.0, -60.0, 15.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(2.6, degree_vec(75.0, -60.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        5.2,
        degree_vec(60.0, -60.0, 15.0),
        Interpolation::CatmullRom,
    ),
];

static IDLE_UNDERWATER_5_RIGHT_HIND_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    5.2,
    pos_vec(0.0, -0.4, 0.0),
    Interpolation::CatmullRom,
)];

static IDLE_UNDERWATER_6_LEFT_FRONT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 20.0, 50.0), Interpolation::CatmullRom),
    Keyframe::new(2.6, degree_vec(0.0, 20.0, 60.0), Interpolation::CatmullRom),
    Keyframe::new(5.2, degree_vec(0.0, 20.0, 50.0), Interpolation::CatmullRom),
];

static IDLE_UNDERWATER_7_LEFT_FRONT_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    2.68,
    pos_vec(-0.15, -0.4, 0.3),
    Interpolation::CatmullRom,
)];

static IDLE_UNDERWATER_8_LEFT_HIND_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(-135.0, -50.0, -330.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.6,
        degree_vec(-160.0, -60.0, -295.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        5.2,
        degree_vec(-135.0, -50.0, -330.0),
        Interpolation::CatmullRom,
    ),
];

static IDLE_UNDERWATER_9_LEFT_HIND_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    5.2,
    pos_vec(0.0, -0.5, 0.0),
    Interpolation::CatmullRom,
)];

static IDLE_UNDERWATER_10_TAIL_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 15.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.6, degree_vec(0.0, -32.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.2, degree_vec(0.0, 15.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_UNDERWATER_11_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(11.7, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.6, degree_vec(2.4, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.2, degree_vec(11.7, 0.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_UNDERWATER_12_LEFT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -25.6, 0.0), Interpolation::CatmullRom),
    Keyframe::new(3.12, degree_vec(0.0, 16.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.2, degree_vec(0.0, -25.6, 0.0), Interpolation::CatmullRom),
];

static IDLE_UNDERWATER_13_RIGHT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 23.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.08, degree_vec(0.0, -26.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.2, degree_vec(0.0, 23.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_UNDERWATER_14_TOP_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-19.2, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.6, degree_vec(12.3, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.2, degree_vec(-19.2, 0.0, 0.0), Interpolation::CatmullRom),
];

static IDLE_UNDERWATER_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: IDLE_UNDERWATER_1_BODY_POSITION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_2_RIGHT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: IDLE_UNDERWATER_3_RIGHT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_4_RIGHT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: IDLE_UNDERWATER_5_RIGHT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_6_LEFT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: IDLE_UNDERWATER_7_LEFT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_8_LEFT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: IDLE_UNDERWATER_9_LEFT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_10_TAIL_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_11_HEAD_ROTATION,
    },
    Channel {
        bone: "left_gills",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_12_LEFT_GILLS_ROTATION,
    },
    Channel {
        bone: "right_gills",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_13_RIGHT_GILLS_ROTATION,
    },
    Channel {
        bone: "top_gills",
        target: Target::Rotation,
        keyframes: IDLE_UNDERWATER_14_TOP_GILLS_ROTATION,
    },
];

pub static IDLE_UNDERWATER: AnimationDefinition = AnimationDefinition {
    length_seconds: 5.2,
    looping: true,
    channels: IDLE_UNDERWATER_CHANNELS,
};

static BABY_AXOLOTL_SWIM_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(8.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.24, degree_vec(12.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.68, degree_vec(-7.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, degree_vec(8.0, 0.0, 0.0), Interpolation::Linear),
];

static BABY_AXOLOTL_SWIM_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.52, pos_vec(0.0, -1.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, pos_vec(0.0, -0.05, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_SWIM_2_RIGHT_FRONT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(310.0, 70.0, 400.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.24,
        degree_vec(330.0, 70.0, 420.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        degree_vec(290.0, 65.0, 384.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.76,
        degree_vec(290.0, 70.0, 380.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.96,
        degree_vec(310.0, 70.0, 400.0),
        Interpolation::CatmullRom,
    ),
];

static BABY_AXOLOTL_SWIM_3_RIGHT_HIND_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(105.0, -95.0, 180.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.28,
        degree_vec(105.0, -85.0, 180.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        degree_vec(105.0, -80.0, 180.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.76,
        degree_vec(105.0, -95.0, 180.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.96,
        degree_vec(105.0, -95.0, 180.0),
        Interpolation::CatmullRom,
    ),
];

static BABY_AXOLOTL_SWIM_4_LEFT_FRONT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(-50.0, -70.0, -40.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        degree_vec(-60.0, -68.0, -30.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.76,
        degree_vec(-55.0, -70.0, -30.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.96,
        degree_vec(-50.0, -70.0, -40.0),
        Interpolation::CatmullRom,
    ),
];

static BABY_AXOLOTL_SWIM_5_LEFT_HIND_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(70.0, -80.0, 15.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.28,
        degree_vec(130.0, -80.0, -45.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.52,
        degree_vec(120.0, -70.0, -35.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.76,
        degree_vec(80.0, -70.0, 5.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.96,
        degree_vec(70.0, -80.0, 15.0),
        Interpolation::CatmullRom,
    ),
];

static BABY_AXOLOTL_SWIM_6_TAIL_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 15.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.52, degree_vec(0.0, -15.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, degree_vec(0.0, 13.36, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_SWIM_7_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-7.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.28, degree_vec(-13.9, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.72, degree_vec(14.23, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, degree_vec(-7.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_SWIM_8_LEFT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -72.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2, degree_vec(0.0, -79.9, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.64, degree_vec(0.0, -38.1, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, degree_vec(0.0, -72.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_SWIM_9_RIGHT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 72.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2, degree_vec(0.0, 86.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.64, degree_vec(0.0, 26.7, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, degree_vec(0.0, 72.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_SWIM_10_TOP_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-57.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2, degree_vec(-68.7, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.64, degree_vec(-24.2, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, degree_vec(-57.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static BABY_AXOLOTL_SWIM_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_SWIM_1_BODY_POSITION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_2_RIGHT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_3_RIGHT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_4_LEFT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_5_LEFT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_6_TAIL_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_7_HEAD_ROTATION,
    },
    Channel {
        bone: "left_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_8_LEFT_GILLS_ROTATION,
    },
    Channel {
        bone: "right_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_9_RIGHT_GILLS_ROTATION,
    },
    Channel {
        bone: "top_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_SWIM_10_TOP_GILLS_ROTATION,
    },
];

pub static BABY_AXOLOTL_SWIM: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.0,
    looping: true,
    channels: BABY_AXOLOTL_SWIM_CHANNELS,
};

static AXOLOTL_WALK_FLOOR_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.72, degree_vec(0.0, 0.0, 0.0), Interpolation::Linear),
];

static AXOLOTL_WALK_FLOOR_1_RIGHT_FRONT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(2.5, 10.0, -30.0), Interpolation::CatmullRom),
    Keyframe::new(
        1.32,
        degree_vec(2.5, -22.5, -30.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.72,
        degree_vec(2.5, 10.0, -30.0),
        Interpolation::CatmullRom,
    ),
];

static AXOLOTL_WALK_FLOOR_2_RIGHT_FRONT_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    1.92,
    pos_vec(0.5, 0.0, 0.8),
    Interpolation::CatmullRom,
)];

static AXOLOTL_WALK_FLOOR_3_RIGHT_HIND_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(260.0, -10.0, 230.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.32,
        degree_vec(90.0, -20.0, 50.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.72,
        degree_vec(260.0, -10.0, 230.0),
        Interpolation::CatmullRom,
    ),
];

static AXOLOTL_WALK_FLOOR_4_RIGHT_HIND_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.72, pos_vec(0.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static AXOLOTL_WALK_FLOOR_5_LEFT_FRONT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-2.5, 32.5, 30.0), Interpolation::CatmullRom),
    Keyframe::new(
        1.32,
        degree_vec(-2.5, -12.0, 30.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.72,
        degree_vec(-2.5, 32.5, 30.0),
        Interpolation::CatmullRom,
    ),
];

static AXOLOTL_WALK_FLOOR_6_LEFT_FRONT_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(-0.5, 0.0, 0.7),
    Interpolation::CatmullRom,
)];

static AXOLOTL_WALK_FLOOR_7_LEFT_HIND_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(-2.5, -30.0, 30.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(1.32, degree_vec(2.5, 12.0, 30.0), Interpolation::CatmullRom),
    Keyframe::new(
        2.72,
        degree_vec(-2.5, -30.0, 30.0),
        Interpolation::CatmullRom,
    ),
];

static AXOLOTL_WALK_FLOOR_8_LEFT_HIND_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(-0.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.72, pos_vec(-0.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static AXOLOTL_WALK_FLOOR_9_TAIL_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -10.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.32, degree_vec(0.0, 10.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.72, degree_vec(0.0, -10.0, 0.0), Interpolation::CatmullRom),
];

static AXOLOTL_WALK_FLOOR_10_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -7.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.32, degree_vec(0.0, 6.7, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.72, degree_vec(0.0, -7.5, 0.0), Interpolation::CatmullRom),
];

static AXOLOTL_WALK_FLOOR_11_LEFT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 38.1, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.08, degree_vec(0.0, 45.6, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.44, degree_vec(0.0, 45.6, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.72, degree_vec(0.0, 38.1, 0.0), Interpolation::CatmullRom),
];

static AXOLOTL_WALK_FLOOR_12_RIGHT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -50.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.08, degree_vec(0.0, -59.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.36, degree_vec(0.0, -59.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.72, degree_vec(0.0, -50.0, 0.0), Interpolation::CatmullRom),
];

static AXOLOTL_WALK_FLOOR_13_TOP_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(33.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.08, degree_vec(47.4, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.44, degree_vec(47.4, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.72, degree_vec(33.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static AXOLOTL_WALK_FLOOR_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_0_BODY_ROTATION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_1_RIGHT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: AXOLOTL_WALK_FLOOR_2_RIGHT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_3_RIGHT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: AXOLOTL_WALK_FLOOR_4_RIGHT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_5_LEFT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: AXOLOTL_WALK_FLOOR_6_LEFT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_7_LEFT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: AXOLOTL_WALK_FLOOR_8_LEFT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_9_TAIL_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_10_HEAD_ROTATION,
    },
    Channel {
        bone: "left_gills",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_11_LEFT_GILLS_ROTATION,
    },
    Channel {
        bone: "right_gills",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_12_RIGHT_GILLS_ROTATION,
    },
    Channel {
        bone: "top_gills",
        target: Target::Rotation,
        keyframes: AXOLOTL_WALK_FLOOR_13_TOP_GILLS_ROTATION,
    },
];

pub static AXOLOTL_WALK_FLOOR: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.72,
    looping: true,
    channels: AXOLOTL_WALK_FLOOR_CHANNELS,
};

static WALK_FLOOR_UNDERWATER_0_BODY_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -5.0, 6.0), Interpolation::CatmullRom),
    Keyframe::new(1.04, degree_vec(0.0, 5.0, -4.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, degree_vec(0.0, -5.0, 6.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_1_BODY_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 1.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, pos_vec(0.0, 1.5, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_2_RIGHT_FRONT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(0.0, -15.0, -30.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.72, degree_vec(0.0, 9.0, -30.0), Interpolation::CatmullRom),
    Keyframe::new(
        1.12,
        degree_vec(0.0, 13.0, -24.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(1.44, degree_vec(0.0, 9.0, -30.0), Interpolation::CatmullRom),
    Keyframe::new(
        2.04,
        degree_vec(0.0, -15.0, -30.0),
        Interpolation::CatmullRom,
    ),
];

static WALK_FLOOR_UNDERWATER_3_RIGHT_FRONT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, pos_vec(0.4, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, pos_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_4_RIGHT_HIND_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(95.0, -20.0, 70.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        0.68,
        degree_vec(255.0, -20.0, 230.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.12,
        degree_vec(280.0, -25.0, 255.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.52,
        degree_vec(255.0, -20.0, 230.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        2.04,
        degree_vec(95.0, -20.0, 70.0),
        Interpolation::CatmullRom,
    ),
];

static WALK_FLOOR_UNDERWATER_5_RIGHT_HIND_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(0.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, pos_vec(0.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, pos_vec(0.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_6_LEFT_FRONT_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(
        0.0,
        degree_vec(-5.0, -20.0, 30.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.72, degree_vec(0.0, 10.0, 25.0), Interpolation::CatmullRom),
    Keyframe::new(1.44, degree_vec(0.0, 10.0, 25.0), Interpolation::CatmullRom),
    Keyframe::new(
        2.04,
        degree_vec(-5.0, -20.0, 30.0),
        Interpolation::CatmullRom,
    ),
];

static WALK_FLOOR_UNDERWATER_7_LEFT_FRONT_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(-0.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, pos_vec(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, pos_vec(-0.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_8_LEFT_HIND_LEG_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(5.0, 15.0, 25.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.72,
        degree_vec(-10.0, -20.0, 35.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        1.44,
        degree_vec(-10.0, -20.0, 35.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(2.04, degree_vec(5.0, 15.0, 25.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_9_LEFT_HIND_LEG_POSITION: &[Keyframe] = &[
    Keyframe::new(0.0, pos_vec(-0.7, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.96, pos_vec(-0.4, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, pos_vec(-0.7, 0.0, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_10_TAIL_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 16.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.56, degree_vec(0.0, 1.12, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.44, degree_vec(0.0, 17.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, degree_vec(0.0, 16.5, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_11_HEAD_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 4.0, -5.8), Interpolation::CatmullRom),
    Keyframe::new(1.04, degree_vec(0.0, -4.0, 5.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, degree_vec(0.0, 4.0, -5.8), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_12_LEFT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, -60.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.24, degree_vec(0.0, -45.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, degree_vec(0.0, -60.0, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_13_RIGHT_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(0.0, 38.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.64, degree_vec(0.0, 53.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, degree_vec(0.0, 38.0, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_14_TOP_GILLS_ROTATION: &[Keyframe] = &[
    Keyframe::new(0.0, degree_vec(-34.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.04, degree_vec(-41.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.04, degree_vec(-34.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static WALK_FLOOR_UNDERWATER_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: WALK_FLOOR_UNDERWATER_1_BODY_POSITION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_2_RIGHT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: WALK_FLOOR_UNDERWATER_3_RIGHT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_4_RIGHT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: WALK_FLOOR_UNDERWATER_5_RIGHT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_6_LEFT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: WALK_FLOOR_UNDERWATER_7_LEFT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_8_LEFT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: WALK_FLOOR_UNDERWATER_9_LEFT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_10_TAIL_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_11_HEAD_ROTATION,
    },
    Channel {
        bone: "left_gills",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_12_LEFT_GILLS_ROTATION,
    },
    Channel {
        bone: "right_gills",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_13_RIGHT_GILLS_ROTATION,
    },
    Channel {
        bone: "top_gills",
        target: Target::Rotation,
        keyframes: WALK_FLOOR_UNDERWATER_14_TOP_GILLS_ROTATION,
    },
];

pub static WALK_FLOOR_UNDERWATER: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.04,
    looping: true,
    channels: WALK_FLOOR_UNDERWATER_CHANNELS,
};

static BABY_AXOLOTL_PLAY_DEAD_0_BODY_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, 0.0, 30.0),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_PLAY_DEAD_1_BODY_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_2_BODY_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_3_RIGHT_FRONT_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(202.35, -30.33, -40.82),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_PLAY_DEAD_4_RIGHT_FRONT_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.2, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_5_RIGHT_FRONT_LEG_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_6_RIGHT_HIND_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(418.0, -50.0, 35.8),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_PLAY_DEAD_7_RIGHT_HIND_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_8_RIGHT_HIND_LEG_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_9_LEFT_FRONT_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(177.0, 22.76, 15.9),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_PLAY_DEAD_10_LEFT_FRONT_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(-0.2, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_11_LEFT_FRONT_LEG_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_12_LEFT_HIND_LEG_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(16.4287, -37.6467, 16.9822),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_PLAY_DEAD_13_LEFT_HIND_LEG_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_14_LEFT_HIND_LEG_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_15_TAIL_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, -18.67, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_16_TAIL_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_17_TAIL_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_18_HEAD_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(-4.21, -0.95, -6.33),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_19_HEAD_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_20_HEAD_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_21_LEFT_GILLS_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, 43.21, 0.0),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_PLAY_DEAD_22_LEFT_GILLS_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_23_LEFT_GILLS_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_24_RIGHT_GILLS_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, -55.87, 0.0),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_PLAY_DEAD_25_RIGHT_GILLS_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_26_RIGHT_GILLS_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_27_TOP_GILLS_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(43.0, 0.0, 0.0),
    Interpolation::CatmullRom,
)];

static BABY_AXOLOTL_PLAY_DEAD_28_TOP_GILLS_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_29_TOP_GILLS_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_30_ROOT_ROTATION: &[Keyframe] = &[Keyframe::new(
    0.0,
    degree_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_31_ROOT_POSITION: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos_vec(0.0, 0.0, 0.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_32_ROOT_SCALE: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale_vec(1.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_AXOLOTL_PLAY_DEAD_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_0_BODY_ROTATION,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_1_BODY_POSITION,
    },
    Channel {
        bone: "body",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_2_BODY_SCALE,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_3_RIGHT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_4_RIGHT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_5_RIGHT_FRONT_LEG_SCALE,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_6_RIGHT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_7_RIGHT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_8_RIGHT_HIND_LEG_SCALE,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_9_LEFT_FRONT_LEG_ROTATION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_10_LEFT_FRONT_LEG_POSITION,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_11_LEFT_FRONT_LEG_SCALE,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_12_LEFT_HIND_LEG_ROTATION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_13_LEFT_HIND_LEG_POSITION,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_14_LEFT_HIND_LEG_SCALE,
    },
    Channel {
        bone: "tail",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_15_TAIL_ROTATION,
    },
    Channel {
        bone: "tail",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_16_TAIL_POSITION,
    },
    Channel {
        bone: "tail",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_17_TAIL_SCALE,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_18_HEAD_ROTATION,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_19_HEAD_POSITION,
    },
    Channel {
        bone: "head",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_20_HEAD_SCALE,
    },
    Channel {
        bone: "left_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_21_LEFT_GILLS_ROTATION,
    },
    Channel {
        bone: "left_gills",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_22_LEFT_GILLS_POSITION,
    },
    Channel {
        bone: "left_gills",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_23_LEFT_GILLS_SCALE,
    },
    Channel {
        bone: "right_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_24_RIGHT_GILLS_ROTATION,
    },
    Channel {
        bone: "right_gills",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_25_RIGHT_GILLS_POSITION,
    },
    Channel {
        bone: "right_gills",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_26_RIGHT_GILLS_SCALE,
    },
    Channel {
        bone: "top_gills",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_27_TOP_GILLS_ROTATION,
    },
    Channel {
        bone: "top_gills",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_28_TOP_GILLS_POSITION,
    },
    Channel {
        bone: "top_gills",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_29_TOP_GILLS_SCALE,
    },
    Channel {
        bone: "root",
        target: Target::Rotation,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_30_ROOT_ROTATION,
    },
    Channel {
        bone: "root",
        target: Target::Position,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_31_ROOT_POSITION,
    },
    Channel {
        bone: "root",
        target: Target::Scale,
        keyframes: BABY_AXOLOTL_PLAY_DEAD_32_ROOT_SCALE,
    },
];

pub static BABY_AXOLOTL_PLAY_DEAD: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.04,
    looping: false,
    channels: BABY_AXOLOTL_PLAY_DEAD_CHANNELS,
};
