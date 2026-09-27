use crate::{primitives::Frustum, Camera, CameraProjection, OrthographicProjection, Projection};
use bevy_ecs::prelude::*;
use bevy_reflect::{std_traits::ReflectDefault, Reflect, ReflectDeserialize, ReflectSerialize};
use bevy_transform::prelude::{GlobalTransform, Transform};
use serde::{Deserialize, Serialize};
use wgpu_types::{LoadOp, TextureUsages};

#[derive(Component, Default, Reflect, Clone)]
#[reflect(Component, Default, Clone)]
#[require(
    Camera,
    Projection::Orthographic(OrthographicProjection::default_2d()),
    Frustum = OrthographicProjection::default_2d().compute_frustum(&GlobalTransform::from(Transform::default())),
)]
pub struct Camera2d;

#[derive(Component, Reflect, Clone)]
#[reflect(Component, Default, Clone)]
#[require(Camera, Projection)]
pub struct Camera3d {
    pub depth_load_op: Camera3dDepthLoadOp,
    pub depth_texture_usages: Camera3dDepthTextureUsage,
    pub screen_space_specular_transmission_steps: usize,
    pub screen_space_specular_transmission_quality: ScreenSpaceTransmissionQuality,
}

impl Default for Camera3d {
    fn default() -> Self {
        Self {
            depth_load_op: Default::default(),
            depth_texture_usages: TextureUsages::RENDER_ATTACHMENT.into(),
            screen_space_specular_transmission_steps: 1,
            screen_space_specular_transmission_quality: Default::default(),
        }
    }
}

#[derive(Clone, Copy, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize, Clone)]
pub struct Camera3dDepthTextureUsage(pub u32);

impl From<TextureUsages> for Camera3dDepthTextureUsage {
    fn from(value: TextureUsages) -> Self {
        Self(value.bits())
    }
}

impl From<Camera3dDepthTextureUsage> for TextureUsages {
    fn from(value: Camera3dDepthTextureUsage) -> Self {
        Self::from_bits_truncate(value.0)
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Debug)]
#[reflect(Serialize, Deserialize, Clone, Default)]
pub enum Camera3dDepthLoadOp {
    Clear(f32),
    Load,
}

impl Default for Camera3dDepthLoadOp {
    fn default() -> Self {
        Camera3dDepthLoadOp::Clear(0.0)
    }
}

impl From<Camera3dDepthLoadOp> for LoadOp<f32> {
    fn from(config: Camera3dDepthLoadOp) -> Self {
        match config {
            Camera3dDepthLoadOp::Clear(x) => LoadOp::Clear(x),
            Camera3dDepthLoadOp::Load => LoadOp::Load,
        }
    }
}

#[derive(Resource, Default, Clone, Copy, Reflect, PartialEq, PartialOrd, Debug)]
#[reflect(Resource, Default, Clone, Debug, PartialEq)]
pub enum ScreenSpaceTransmissionQuality {
    Low,

    #[default]
    Medium,

    High,

    Ultra,
}
