use crate::entities::geom::{BakedModel, PartState};
pub use crate::util::mth::DEG_TO_RAD;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Position,
    Rotation,
    Scale,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interpolation {
    Linear,
    CatmullRom,
}

#[derive(Clone, Copy, Debug)]
pub struct Keyframe {
    pub timestamp: f32,
    pub pre: [f32; 3],
    pub post: [f32; 3],
    pub interpolation: Interpolation,
}

impl Keyframe {
    pub const fn new(timestamp: f32, target: [f32; 3], interpolation: Interpolation) -> Keyframe {
        Keyframe {
            timestamp,
            pre: target,
            post: target,
            interpolation,
        }
    }
}

pub const fn pos_vec(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x, -y, z]
}

pub const fn degree_vec(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x * DEG_TO_RAD, y * DEG_TO_RAD, z * DEG_TO_RAD]
}

pub const fn scale_vec(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x - 1.0, y - 1.0, z - 1.0]
}

pub struct Channel {
    pub bone: &'static str,
    pub target: Target,
    pub keyframes: &'static [Keyframe],
}

pub struct AnimationDefinition {
    pub length_seconds: f32,
    pub looping: bool,
    pub channels: &'static [Channel],
}

pub fn apply(
    animation: &AnimationDefinition,
    model: &BakedModel,
    parts: &mut [PartState],
    seconds_since_start: f32,
    target_scale: f32,
) {
    let seconds = if animation.looping {
        seconds_since_start.rem_euclid(animation.length_seconds)
    } else {
        seconds_since_start
    };

    for channel in animation.channels {
        let Some(part) = model.find(channel.bone) else {
            continue;
        };
        let value = sample_channel(channel, seconds, target_scale);
        let state = &mut parts[part];
        match channel.target {
            Target::Position => state.offset_pos(value[0], value[1], value[2]),
            Target::Rotation => state.offset_rotation(value[0], value[1], value[2]),
            Target::Scale => state.offset_scale(value[0], value[1], value[2]),
        }
    }
}

fn sample_channel(channel: &Channel, seconds: f32, target_scale: f32) -> [f32; 3] {
    let frames = channel.keyframes;
    debug_assert!(!frames.is_empty());
    let first_at_or_after = frames
        .iter()
        .position(|frame| seconds <= frame.timestamp)
        .unwrap_or(frames.len());
    let prev = first_at_or_after.saturating_sub(1);
    let next = (prev + 1).min(frames.len() - 1);

    let alpha = if next != prev {
        let span = frames[next].timestamp - frames[prev].timestamp;
        ((seconds - frames[prev].timestamp) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };

    match frames[next].interpolation {
        Interpolation::Linear => {
            let a = frames[prev].post;
            let b = frames[next].pre;
            [
                (a[0] + (b[0] - a[0]) * alpha) * target_scale,
                (a[1] + (b[1] - a[1]) * alpha) * target_scale,
                (a[2] + (b[2] - a[2]) * alpha) * target_scale,
            ]
        }
        Interpolation::CatmullRom => {
            let p0 = frames[prev.saturating_sub(1)].post;
            let p1 = frames[prev].post;
            let p2 = frames[next].post;
            let p3 = frames[(next + 1).min(frames.len() - 1)].post;
            [
                catmullrom(alpha, p0[0], p1[0], p2[0], p3[0]) * target_scale,
                catmullrom(alpha, p0[1], p1[1], p2[1], p3[1]) * target_scale,
                catmullrom(alpha, p0[2], p1[2], p2[2], p3[2]) * target_scale,
            ]
        }
    }
}

fn catmullrom(t: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t * t * t)
}

pub fn apply_static(animation: &AnimationDefinition, model: &BakedModel, parts: &mut [PartState]) {
    apply(animation, model, parts, 0.0, 1.0);
}

pub fn apply_walk(
    animation: &AnimationDefinition,
    model: &BakedModel,
    parts: &mut [PartState],
    animation_pos: f32,
    animation_speed: f32,
    speed_factor: f32,
    scale_factor: f32,
) {
    let seconds = animation_pos * 0.05 * speed_factor;
    let scale = (animation_speed * scale_factor).min(1.0);
    apply(animation, model, parts, seconds, scale);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::geom::{CubeList, LayerDef, MeshDef, PartPose, bake};

    static FRAMES: &[Keyframe] = &[
        Keyframe::new(0.0, [0.0, 0.0, 0.0], Interpolation::Linear),
        Keyframe::new(1.0, [1.0, 2.0, 3.0], Interpolation::Linear),
    ];

    static CHANNELS: &[Channel] = &[Channel {
        bone: "head",
        target: Target::Rotation,
        keyframes: FRAMES,
    }];

    static ANIMATION: AnimationDefinition = AnimationDefinition {
        length_seconds: 1.0,
        looping: true,
        channels: CHANNELS,
    };

    fn one_part_model() -> crate::entities::geom::BakedModel {
        let mut mesh = MeshDef::new();
        mesh.root().child(
            "head",
            CubeList::new().add_box(0.0, 0.0, 0.0, 1.0, 1.0, 1.0),
            PartPose::ZERO,
        );
        bake(&LayerDef::create(mesh, 16, 16))
    }

    #[test]
    fn linear_channel_interpolates_between_frames() {
        let model = one_part_model();
        let mut parts = model.rest_pose();
        apply(&ANIMATION, &model, &mut parts, 0.5, 1.0);
        let head = &parts[model.id("head")];
        assert!((head.x_rot - 0.5).abs() < 1e-6);
        assert!((head.y_rot - 1.0).abs() < 1e-6);
        assert!((head.z_rot - 1.5).abs() < 1e-6);
    }

    #[test]
    fn a_looping_animation_wraps() {
        let model = one_part_model();
        let mut parts = model.rest_pose();
        apply(&ANIMATION, &model, &mut parts, 2.5, 1.0);
        assert!((parts[model.id("head")].x_rot - 0.5).abs() < 1e-6);
    }

    #[test]
    fn target_scale_multiplies_the_result() {
        let model = one_part_model();
        let mut parts = model.rest_pose();
        apply(&ANIMATION, &model, &mut parts, 0.5, 0.5);
        assert!((parts[model.id("head")].x_rot - 0.25).abs() < 1e-6);
    }
}
