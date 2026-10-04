#![allow(clippy::approx_constant, dead_code)]

use crate::entities::keyframe::{
    AnimationDefinition, Channel, Interpolation, Keyframe, Target, degree_vec as deg,
    pos_vec as pos, scale_vec as scale,
};

static BABY_TRANSFORM_C0: &[Keyframe] = &[Keyframe::new(
    0.0,
    scale(1.2000000476837158, 1.2000000476837158, 1.2000000476837158),
    Interpolation::Linear,
)];

static BABY_TRANSFORM_C1: &[Keyframe] = &[Keyframe::new(
    0.0,
    pos(0.0, 1.0, 1.0),
    Interpolation::Linear,
)];

static BABY_TRANSFORM_CHANNELS: &[Channel] = &[
    Channel {
        bone: "head",
        target: Target::Scale,
        keyframes: BABY_TRANSFORM_C0,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: BABY_TRANSFORM_C1,
    },
];

pub static BABY_TRANSFORM: AnimationDefinition = AnimationDefinition {
    length_seconds: 0.0,
    looping: false,
    channels: BABY_TRANSFORM_CHANNELS,
};

static SNIFFER_SNIFFSNIFF_C0: &[Keyframe] = &[
    Keyframe::new(0.0, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.5417, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(0.5833, scale(1.0, 0.5, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.6667, scale(1.0, 2.5, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.7917, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.9167, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, scale(1.0, 3.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(2.0, scale(1.0, 1.0, 1.0), Interpolation::Linear),
];

static SNIFFER_SNIFFSNIFF_CHANNELS: &[Channel] = &[Channel {
    bone: "nose",
    target: Target::Scale,
    keyframes: SNIFFER_SNIFFSNIFF_C0,
}];

pub static SNIFFER_SNIFFSNIFF: AnimationDefinition = AnimationDefinition {
    length_seconds: 8.0,
    looping: true,
    channels: SNIFFER_SNIFFSNIFF_CHANNELS,
};

static SNIFFER_LONGSNIFF_C0: &[Keyframe] = &[
    Keyframe::new(0.0, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(
        0.0833,
        scale(1.0, 0.699999988079071, 1.0),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(0.125, scale(1.0, 3.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.25, scale(1.0, 3.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.7083, scale(1.0, 4.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.8333, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.0, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
];

static SNIFFER_LONGSNIFF_C1: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, deg(-5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.875, deg(-20.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_LONGSNIFF_CHANNELS: &[Channel] = &[
    Channel {
        bone: "nose",
        target: Target::Scale,
        keyframes: SNIFFER_LONGSNIFF_C0,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: SNIFFER_LONGSNIFF_C1,
    },
];

pub static SNIFFER_LONGSNIFF: AnimationDefinition = AnimationDefinition {
    length_seconds: 1.0,
    looping: false,
    channels: SNIFFER_LONGSNIFF_CHANNELS,
};

static SNIFFER_WALK_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5833, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_WALK_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 3.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, pos(0.0, 0.0, -1.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 3.0), Interpolation::Linear),
];

static SNIFFER_WALK_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-7.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-7.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_WALK_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 2.67, -0.67), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, 2.0), Interpolation::Linear),
    Keyframe::new(1.1667, pos(0.0, 0.0, 3.0), Interpolation::Linear),
    Keyframe::new(1.9167, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 2.67, -0.67), Interpolation::Linear),
];

static SNIFFER_WALK_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5833, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5833, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_WALK_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(0.5833, pos(0.0, 0.0, 2.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 2.22, 0.78), Interpolation::Linear),
    Keyframe::new(1.3333, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(1.5833, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, -0.5), Interpolation::Linear),
];

static SNIFFER_WALK_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5833, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_WALK_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.0, 0.0, -1.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, 3.0), Interpolation::Linear),
    Keyframe::new(1.75, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_WALK_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_WALK_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 2.0), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.0, 0.0, 3.0), Interpolation::Linear),
    Keyframe::new(0.9167, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(1.1667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 2.0), Interpolation::Linear),
];

static SNIFFER_WALK_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5833, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5833, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(25.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_WALK_C11: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 2.22, 0.78), Interpolation::Linear),
    Keyframe::new(0.3333, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.5833, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(1.5833, pos(0.0, 0.0, 2.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 2.22, 0.78), Interpolation::Linear),
];

static SNIFFER_WALK_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(1.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(0.5, deg(-1.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(1.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(1.5, deg(-1.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(1.0, 0.0, -2.5), Interpolation::Linear),
];

static SNIFFER_WALK_C13: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.2083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.375, pos(0.0, -1.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, pos(0.0, -1.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_WALK_C14: &[Keyframe] = &[
    Keyframe::new(0.0, deg(7.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.1667, deg(9.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.875, deg(-1.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.25, deg(7.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.75, deg(5.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.0, deg(7.5, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_WALK_C15: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, -2.5), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(0.0, 0.0, -7.5), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(0.0, 0.0, -2.5), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, -7.5), Interpolation::CatmullRom),
    Keyframe::new(2.0, deg(0.0, 0.0, -2.5), Interpolation::CatmullRom),
];

static SNIFFER_WALK_C16: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 2.5), Interpolation::CatmullRom),
    Keyframe::new(0.5, deg(0.0, 0.0, 7.5), Interpolation::CatmullRom),
    Keyframe::new(1.0, deg(0.0, 0.0, 2.5), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, 7.5), Interpolation::CatmullRom),
    Keyframe::new(2.0, deg(0.0, 0.0, 2.5), Interpolation::CatmullRom),
];

static SNIFFER_WALK_CHANNELS: &[Channel] = &[
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C0,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_WALK_C1,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C2,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_WALK_C3,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C4,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_WALK_C5,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C6,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_WALK_C7,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C8,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_WALK_C9,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C10,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_WALK_C11,
    },
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C12,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: SNIFFER_WALK_C13,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C14,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C15,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_WALK_C16,
    },
];

pub static SNIFFER_WALK: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.0,
    looping: true,
    channels: SNIFFER_WALK_CHANNELS,
};

static SNIFFER_SNIFF_SEARCH_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5833, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 3.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, pos(0.0, 0.0, -1.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 3.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-7.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-7.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 2.67, -0.67), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, 2.0), Interpolation::Linear),
    Keyframe::new(1.1667, pos(0.0, 0.0, 3.0), Interpolation::Linear),
    Keyframe::new(1.9167, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 2.67, -0.67), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5833, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5833, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C5: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(0.5833, pos(0.0, 0.0, 2.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 2.22, 0.78), Interpolation::Linear),
    Keyframe::new(1.3333, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(1.5833, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, -0.5), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5833, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.0, 0.0, -1.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, 3.0), Interpolation::Linear),
    Keyframe::new(1.75, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C8: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.1667, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C9: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 2.0), Interpolation::Linear),
    Keyframe::new(0.1667, pos(0.0, 0.0, 3.0), Interpolation::Linear),
    Keyframe::new(0.9167, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(1.1667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 0.0, 2.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C10: &[Keyframe] = &[
    Keyframe::new(0.0, deg(25.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.1667, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5833, deg(-35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5833, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(25.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C11: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 2.22, 0.78), Interpolation::Linear),
    Keyframe::new(0.3333, pos(0.0, 4.0, -1.0), Interpolation::Linear),
    Keyframe::new(0.5833, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, 0.0, -2.0), Interpolation::Linear),
    Keyframe::new(1.0, pos(0.0, 0.0, -0.5), Interpolation::Linear),
    Keyframe::new(1.5833, pos(0.0, 0.0, 2.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, 2.22, 0.78), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(1.25, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(2.5, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C13: &[Keyframe] = &[
    Keyframe::new(0.0, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.125, deg(33.61503, 11.46526, 9.803), Interpolation::Linear),
    Keyframe::new(
        0.875,
        deg(34.71128, 17.67415, 14.15251),
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.125,
        deg(37.21128, -17.67415, -14.15251),
        Interpolation::Linear,
    ),
    Keyframe::new(
        1.875,
        deg(38.30529, -21.62827, -17.40292),
        Interpolation::Linear,
    ),
    Keyframe::new(2.0, deg(35.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C14: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, -2.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, pos(0.0, -2.0, 0.0), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C15: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, -15.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, -15.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, -15.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(1.75, deg(0.0, 0.0, -15.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, -2.5), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C16: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(0.25, deg(0.0, 0.0, 15.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(0.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(0.75, deg(0.0, 0.0, 15.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(0.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, 15.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(1.75, deg(0.0, 0.0, 15.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 2.5), Interpolation::Linear),
];

static SNIFFER_SNIFF_SEARCH_C17: &[Keyframe] = &[
    Keyframe::new(0.0, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.0833, scale(1.0, 1.5, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.2083, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.375, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, scale(1.0, 2.5, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.625, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.8333, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(0.9167, scale(1.0, 2.5, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.0833, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.2917, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.3333, scale(1.0, 2.5, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.625, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.6667, scale(1.0, 3.5, 1.0), Interpolation::CatmullRom),
    Keyframe::new(1.8333, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
    Keyframe::new(2.0, scale(1.0, 1.0, 1.0), Interpolation::CatmullRom),
];

static SNIFFER_SNIFF_SEARCH_CHANNELS: &[Channel] = &[
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C0,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_SNIFF_SEARCH_C1,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C2,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_SNIFF_SEARCH_C3,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C4,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_SNIFF_SEARCH_C5,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C6,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_SNIFF_SEARCH_C7,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C8,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_SNIFF_SEARCH_C9,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C10,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_SNIFF_SEARCH_C11,
    },
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C12,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C13,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: SNIFFER_SNIFF_SEARCH_C14,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C15,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_SNIFF_SEARCH_C16,
    },
    Channel {
        bone: "nose",
        target: Target::Scale,
        keyframes: SNIFFER_SNIFF_SEARCH_C17,
    },
];

pub static SNIFFER_SNIFF_SEARCH: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.0,
    looping: true,
    channels: SNIFFER_SNIFF_SEARCH_CHANNELS,
};

static SNIFFER_DIG_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.5, deg(1.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, deg(-5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.5, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(4.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(4.5, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(5.6667, deg(5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(5.8333, deg(-2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(6.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C1: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, pos(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, -7.0, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C2: &[Keyframe] = &[
    Keyframe::new(0.0, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.5, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        1.5417,
        scale(1.0399999618530273, 0.9800000190734863, 1.0199999809265137),
        Interpolation::Linear,
    ),
    Keyframe::new(1.5833, scale(1.0, 1.0, 1.0), Interpolation::Linear),
];

static SNIFFER_DIG_C3: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.1667, deg(10.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.4167, deg(-10.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5833, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.875, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.0833, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.5, deg(47.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.6667, deg(38.44, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        2.875,
        deg(10.95951, 13.57454, -14.93501),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(3.2083, deg(47.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(3.5833, deg(55.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        3.7917,
        deg(4.2932, -16.187, 10.90042),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(4.125, deg(47.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        4.4167,
        deg(54.71135, 7.98009, -5.56662),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.5,
        deg(55.72895, -6.77684, 4.46197),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.5833,
        deg(54.71135, 7.98009, -5.56662),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.6667,
        deg(55.72895, -6.77684, 4.46197),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.75,
        deg(54.71135, 7.98009, -5.56662),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(
        4.8333,
        deg(55.72895, -6.77684, 4.46197),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(5.0, deg(65.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.75, deg(65.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(5.9167, deg(-32.5, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(6.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C4: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.625, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, pos(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5833, pos(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.875, pos(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0833, pos(0.0, 3.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.2917, pos(0.0, 6.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.6667, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.2083, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.5833, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(4.125, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(5.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(5.75, pos(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(6.0, pos(0.0, 1.5, 0.0), Interpolation::Linear),
    Keyframe::new(6.25, pos(0.0, 1.0, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C5: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, -2.5), Interpolation::Linear),
    Keyframe::new(1.4167, deg(0.0, 0.0, -50.0), Interpolation::Linear),
    Keyframe::new(1.5833, deg(0.0, 0.0, -30.0), Interpolation::Linear),
    Keyframe::new(5.9167, deg(0.0, 0.0, -30.0), Interpolation::Linear),
    Keyframe::new(6.0833, deg(0.0, 0.0, -65.0), Interpolation::Linear),
    Keyframe::new(6.3333, deg(0.0, 0.0, -30.0), Interpolation::Linear),
];

static SNIFFER_DIG_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, 2.5), Interpolation::Linear),
    Keyframe::new(1.4167, deg(0.0, 0.0, 50.0), Interpolation::Linear),
    Keyframe::new(1.5833, deg(0.0, 0.0, 30.0), Interpolation::Linear),
    Keyframe::new(5.9167, deg(0.0, 0.0, 30.0), Interpolation::Linear),
    Keyframe::new(6.0833, deg(0.0, 0.0, 65.0), Interpolation::Linear),
    Keyframe::new(6.3333, deg(0.0, 0.0, 30.0), Interpolation::Linear),
];

static SNIFFER_DIG_C7: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2083, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, deg(0.0, 0.0, 90.0), Interpolation::Linear),
];

static SNIFFER_DIG_C8: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2917, pos(-2.0, -0.75, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, pos(-4.0, -5.5, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C9: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4167, deg(0.0, 0.0, 90.0), Interpolation::Linear),
];

static SNIFFER_DIG_C10: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, pos(-2.0, -0.75, 0.0), Interpolation::Linear),
    Keyframe::new(1.4167, pos(-4.0, -5.5, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C11: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 90.0), Interpolation::Linear),
];

static SNIFFER_DIG_C12: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4167, pos(-2.0, -0.75, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(-4.0, -5.5, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C13: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2083, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, deg(0.0, 0.0, -90.0), Interpolation::Linear),
];

static SNIFFER_DIG_C14: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2917, pos(2.0, -0.75, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, pos(4.0, -5.5, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C15: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4167, deg(0.0, 0.0, -90.0), Interpolation::Linear),
];

static SNIFFER_DIG_C16: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, pos(2.0, -0.75, 0.0), Interpolation::Linear),
    Keyframe::new(1.4167, pos(4.0, -5.5, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_C17: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, -90.0), Interpolation::Linear),
];

static SNIFFER_DIG_C18: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.3333, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.4167, pos(2.0, -0.75, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(4.0, -5.5, 0.0), Interpolation::Linear),
];

static SNIFFER_DIG_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: SNIFFER_DIG_C1,
    },
    Channel {
        bone: "body",
        target: Target::Scale,
        keyframes: SNIFFER_DIG_C2,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C3,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: SNIFFER_DIG_C4,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C5,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C6,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C7,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_DIG_C8,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C9,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_DIG_C10,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C11,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_DIG_C12,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C13,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_DIG_C14,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C15,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_DIG_C16,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_DIG_C17,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_DIG_C18,
    },
];

pub static SNIFFER_DIG: AnimationDefinition = AnimationDefinition {
    length_seconds: 8.0,
    looping: false,
    channels: SNIFFER_DIG_CHANNELS,
};

static SNIFFER_STAND_UP_C0: &[Keyframe] = &[
    Keyframe::new(0.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, deg(2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-2.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7083, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_STAND_UP_C1: &[Keyframe] = &[
    Keyframe::new(0.25, pos(0.0, -7.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.75, pos(0.0, -7.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.7083, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_STAND_UP_C2: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.3333, deg(-5.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(0.7083, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.0, deg(10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_STAND_UP_C3: &[Keyframe] = &[
    Keyframe::new(0.0, pos(0.0, 1.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, pos(0.0, 1.0, 0.0), Interpolation::Linear),
];

static SNIFFER_STAND_UP_C4: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, -30.0), Interpolation::Linear),
    Keyframe::new(0.9167, deg(0.0, 0.0, -30.0), Interpolation::Linear),
    Keyframe::new(1.2083, deg(0.0, 0.0, -5.0), Interpolation::Linear),
];

static SNIFFER_STAND_UP_C5: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 30.0), Interpolation::Linear),
    Keyframe::new(0.9167, deg(0.0, 0.0, 30.0), Interpolation::Linear),
    Keyframe::new(1.2083, deg(0.0, 0.0, 5.0), Interpolation::Linear),
];

static SNIFFER_STAND_UP_C6: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 90.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C7: &[Keyframe] = &[
    Keyframe::new(0.0, pos(-4.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2083, pos(6.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C8: &[Keyframe] = &[
    Keyframe::new(0.0833, deg(0.0, 0.0, 90.0), Interpolation::CatmullRom),
    Keyframe::new(0.5833, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C9: &[Keyframe] = &[
    Keyframe::new(0.0833, pos(-4.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.3333, pos(6.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5833, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C10: &[Keyframe] = &[
    Keyframe::new(0.1667, deg(0.0, 0.0, 90.0), Interpolation::CatmullRom),
    Keyframe::new(0.6667, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C11: &[Keyframe] = &[
    Keyframe::new(0.1667, pos(-4.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4167, pos(6.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.6667, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C12: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, -90.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C13: &[Keyframe] = &[
    Keyframe::new(0.0, pos(4.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.2083, pos(-6.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4583, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C14: &[Keyframe] = &[
    Keyframe::new(0.0833, deg(0.0, 0.0, -90.0), Interpolation::CatmullRom),
    Keyframe::new(0.5833, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C15: &[Keyframe] = &[
    Keyframe::new(0.0833, pos(4.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.3333, pos(-6.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.5833, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C16: &[Keyframe] = &[
    Keyframe::new(0.1667, deg(0.0, 0.0, -90.0), Interpolation::CatmullRom),
    Keyframe::new(0.6667, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_C17: &[Keyframe] = &[
    Keyframe::new(0.1667, pos(4.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.4167, pos(-6.0, -5.5, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.6667, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_STAND_UP_CHANNELS: &[Channel] = &[
    Channel {
        bone: "body",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C0,
    },
    Channel {
        bone: "body",
        target: Target::Position,
        keyframes: SNIFFER_STAND_UP_C1,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C2,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: SNIFFER_STAND_UP_C3,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C4,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C5,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C6,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_STAND_UP_C7,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C8,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_STAND_UP_C9,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C10,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_STAND_UP_C11,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C12,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_STAND_UP_C13,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C14,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_STAND_UP_C15,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_STAND_UP_C16,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_STAND_UP_C17,
    },
];

pub static SNIFFER_STAND_UP: AnimationDefinition = AnimationDefinition {
    length_seconds: 3.0,
    looping: false,
    channels: SNIFFER_STAND_UP_CHANNELS,
};

static SNIFFER_BABY_FALL_C0: &[Keyframe] = &[
    Keyframe::new(1.0, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, deg(-98.91, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.9583, deg(-90.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.7083, deg(-68.28, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(2.9583, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_BABY_FALL_C1: &[Keyframe] = &[
    Keyframe::new(1.0, pos(0.0, 20.0, 17.0), Interpolation::CatmullRom),
    Keyframe::new(1.5, pos(0.0, 25.19, 20.37), Interpolation::CatmullRom),
    Keyframe::new(1.9583, pos(0.0, 20.0, 17.0), Interpolation::CatmullRom),
    Keyframe::new(2.7083, pos(0.0, 17.06, 11.25), Interpolation::CatmullRom),
    Keyframe::new(2.8333, pos(0.0, 9.85, 2.2), Interpolation::CatmullRom),
    Keyframe::new(2.9583, pos(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_BABY_FALL_C2: &[Keyframe] = &[
    Keyframe::new(1.0, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(1.9583, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(2.9167, scale(1.0, 1.0, 1.0), Interpolation::Linear),
    Keyframe::new(
        3.0,
        scale(1.0499999523162842, 0.949999988079071, 1.0499999523162842),
        Interpolation::Linear,
    ),
    Keyframe::new(3.0833, scale(1.0, 1.0, 1.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C3: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.2917, deg(17.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9583, deg(-10.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.75, deg(35.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.9167, deg(-30.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0417, deg(7.5, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.125, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C4: &[Keyframe] = &[
    Keyframe::new(1.0, pos(0.0, 7.0, 19.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 7.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.9583, pos(0.0, 7.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.75, pos(0.0, 7.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.9583, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C5: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, -5.0), Interpolation::Linear),
    Keyframe::new(1.9583, deg(0.0, 0.0, -5.0), Interpolation::Linear),
    Keyframe::new(2.7083, deg(0.0, 0.0, -5.0), Interpolation::Linear),
    Keyframe::new(2.9167, deg(0.0, 0.0, -90.0), Interpolation::CatmullRom),
    Keyframe::new(3.125, deg(0.0, 0.0, -5.0), Interpolation::CatmullRom),
];

static SNIFFER_BABY_FALL_C6: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, 5.0), Interpolation::Linear),
    Keyframe::new(1.9583, deg(0.0, 0.0, 5.0), Interpolation::Linear),
    Keyframe::new(2.7083, deg(0.0, 0.0, 5.0), Interpolation::Linear),
    Keyframe::new(2.9167, deg(0.0, 0.0, 90.0), Interpolation::CatmullRom),
    Keyframe::new(3.125, deg(0.0, 0.0, 5.0), Interpolation::CatmullRom),
];

static SNIFFER_BABY_FALL_C7: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.25, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.75, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C8: &[Keyframe] = &[
    Keyframe::new(1.0, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C9: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.625, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.875, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.125, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.375, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.625, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C10: &[Keyframe] = &[
    Keyframe::new(1.0, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C11: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.25, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C12: &[Keyframe] = &[
    Keyframe::new(1.0, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C13: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.25, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.75, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C14: &[Keyframe] = &[
    Keyframe::new(1.0, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C15: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.375, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.625, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.875, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.125, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.375, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.625, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C16: &[Keyframe] = &[
    Keyframe::new(1.0, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C17: &[Keyframe] = &[
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.25, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.75, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.0, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.25, deg(-15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(2.5, deg(15.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(3.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_C18: &[Keyframe] = &[
    Keyframe::new(1.0, pos(0.0, 4.0, 0.0), Interpolation::Linear),
    Keyframe::new(1.5, pos(0.0, 0.0, 0.0), Interpolation::Linear),
];

static SNIFFER_BABY_FALL_CHANNELS: &[Channel] = &[
    Channel {
        bone: "bone",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C0,
    },
    Channel {
        bone: "bone",
        target: Target::Position,
        keyframes: SNIFFER_BABY_FALL_C1,
    },
    Channel {
        bone: "body",
        target: Target::Scale,
        keyframes: SNIFFER_BABY_FALL_C2,
    },
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C3,
    },
    Channel {
        bone: "head",
        target: Target::Position,
        keyframes: SNIFFER_BABY_FALL_C4,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C5,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C6,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C7,
    },
    Channel {
        bone: "right_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_BABY_FALL_C8,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C9,
    },
    Channel {
        bone: "right_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_BABY_FALL_C10,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C11,
    },
    Channel {
        bone: "right_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_BABY_FALL_C12,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C13,
    },
    Channel {
        bone: "left_front_leg",
        target: Target::Position,
        keyframes: SNIFFER_BABY_FALL_C14,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C15,
    },
    Channel {
        bone: "left_mid_leg",
        target: Target::Position,
        keyframes: SNIFFER_BABY_FALL_C16,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Rotation,
        keyframes: SNIFFER_BABY_FALL_C17,
    },
    Channel {
        bone: "left_hind_leg",
        target: Target::Position,
        keyframes: SNIFFER_BABY_FALL_C18,
    },
];

pub static SNIFFER_BABY_FALL: AnimationDefinition = AnimationDefinition {
    length_seconds: 4.0,
    looping: false,
    channels: SNIFFER_BABY_FALL_CHANNELS,
};

static SNIFFER_HAPPY_C0: &[Keyframe] = &[
    Keyframe::new(0.0, deg(0.0, 0.0, 0.0), Interpolation::Linear),
    Keyframe::new(
        0.5,
        deg(-32.00206, 19.3546, -11.70092),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(1.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(
        1.5,
        deg(-32.00206, -19.3546, 11.70092),
        Interpolation::CatmullRom,
    ),
    Keyframe::new(2.0, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_HAPPY_C1: &[Keyframe] = &[
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, -67.5), Interpolation::CatmullRom),
    Keyframe::new(0.9583, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, deg(0.0, 0.0, -67.5), Interpolation::CatmullRom),
    Keyframe::new(1.2917, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_HAPPY_C2: &[Keyframe] = &[
    Keyframe::new(0.5, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(0.75, deg(0.0, 0.0, 67.5), Interpolation::CatmullRom),
    Keyframe::new(0.9583, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
    Keyframe::new(1.125, deg(0.0, 0.0, 67.5), Interpolation::CatmullRom),
    Keyframe::new(1.2917, deg(0.0, 0.0, 0.0), Interpolation::CatmullRom),
];

static SNIFFER_HAPPY_CHANNELS: &[Channel] = &[
    Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: SNIFFER_HAPPY_C0,
    },
    Channel {
        bone: "left_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_HAPPY_C1,
    },
    Channel {
        bone: "right_ear",
        target: Target::Rotation,
        keyframes: SNIFFER_HAPPY_C2,
    },
];

pub static SNIFFER_HAPPY: AnimationDefinition = AnimationDefinition {
    length_seconds: 2.0,
    looping: true,
    channels: SNIFFER_HAPPY_CHANNELS,
};
