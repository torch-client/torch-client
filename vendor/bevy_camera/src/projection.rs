use core::fmt::Debug;
use core::ops::{Deref, DerefMut};

use crate::{primitives::Frustum, visibility::VisibilitySystems};
use bevy_app::{App, Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use bevy_math::{ops, vec4, AspectRatio, Mat4, Rect, Vec2, Vec3A, Vec4};
use bevy_reflect::{std_traits::ReflectDefault, Reflect, ReflectDeserialize, ReflectSerialize};
use bevy_transform::{components::GlobalTransform, TransformSystems};
use derive_more::derive::From;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct CameraProjectionPlugin;

impl Plugin for CameraProjectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            crate::visibility::update_frusta
                .in_set(VisibilitySystems::UpdateFrusta)
                .after(TransformSystems::Propagate),
        );
    }
}

pub trait CameraProjection {
    fn get_clip_from_view(&self) -> Mat4;

    fn get_clip_from_view_for_sub(&self, sub_view: &super::SubCameraView) -> Mat4;

    fn update(&mut self, width: f32, height: f32);

    fn far(&self) -> f32;

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8];

    fn compute_frustum(&self, camera_transform: &GlobalTransform) -> Frustum {
        let clip_from_world = self.get_clip_from_view() * camera_transform.affine().inverse();
        Frustum::from_clip_from_world_custom_far(
            &clip_from_world,
            &camera_transform.translation(),
            &camera_transform.back(),
            self.far(),
        )
    }
}

mod sealed {
    use super::CameraProjection;

    pub trait DynCameraProjection:
        CameraProjection + core::fmt::Debug + Send + Sync + downcast_rs::Downcast
    {
        fn clone_box(&self) -> Box<dyn DynCameraProjection>;
    }

    downcast_rs::impl_downcast!(DynCameraProjection);

    impl<T> DynCameraProjection for T
    where
        T: 'static + CameraProjection + core::fmt::Debug + Send + Sync + Clone,
    {
        fn clone_box(&self) -> Box<dyn DynCameraProjection> {
            Box::new(self.clone())
        }
    }
}

#[derive(Debug, Reflect)]
#[reflect(Default, Clone)]
pub struct CustomProjection {
    #[reflect(ignore)]
    dyn_projection: Box<dyn sealed::DynCameraProjection>,
}

impl Default for CustomProjection {
    fn default() -> Self {
        Self {
            dyn_projection: Box::new(PerspectiveProjection::default()),
        }
    }
}

impl Clone for CustomProjection {
    fn clone(&self) -> Self {
        Self {
            dyn_projection: self.dyn_projection.clone_box(),
        }
    }
}

impl CustomProjection {
    pub fn get<P>(&self) -> Option<&P>
    where
        P: CameraProjection + Debug + Send + Sync + Clone + 'static,
    {
        self.dyn_projection.downcast_ref()
    }

    pub fn get_mut<P>(&mut self) -> Option<&mut P>
    where
        P: CameraProjection + Debug + Send + Sync + Clone + 'static,
    {
        self.dyn_projection.downcast_mut()
    }
}

impl Deref for CustomProjection {
    type Target = dyn CameraProjection;

    fn deref(&self) -> &Self::Target {
        self.dyn_projection.as_ref()
    }
}

impl DerefMut for CustomProjection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.dyn_projection.as_mut()
    }
}

#[derive(Component, Debug, Clone, Reflect, From)]
#[reflect(Component, Default, Debug, Clone)]
pub enum Projection {
    Perspective(PerspectiveProjection),
    Orthographic(OrthographicProjection),
    Custom(CustomProjection),
}

impl Projection {
    pub fn custom<P>(projection: P) -> Self
    where
        P: CameraProjection + Debug + Send + Sync + Clone + 'static,
    {
        Projection::Custom(CustomProjection {
            dyn_projection: Box::new(projection),
        })
    }

    pub fn is_perspective(&self) -> bool {
        match self {
            Projection::Perspective(_) => true,
            Projection::Orthographic(_) => false,
            Projection::Custom(projection) => projection.get_clip_from_view().w_axis.w == 0.0,
        }
    }
}

impl Deref for Projection {
    type Target = dyn CameraProjection;

    fn deref(&self) -> &Self::Target {
        match self {
            Projection::Perspective(projection) => projection,
            Projection::Orthographic(projection) => projection,
            Projection::Custom(projection) => projection.deref(),
        }
    }
}

impl DerefMut for Projection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Projection::Perspective(projection) => projection,
            Projection::Orthographic(projection) => projection,
            Projection::Custom(projection) => projection.deref_mut(),
        }
    }
}

impl Default for Projection {
    fn default() -> Self {
        Projection::Perspective(Default::default())
    }
}

#[derive(Debug, Clone, Reflect)]
#[reflect(Default, Debug, Clone)]
pub struct PerspectiveProjection {
    pub fov: f32,

    pub aspect_ratio: f32,

    pub near: f32,

    pub far: f32,

    pub near_clip_plane: Vec4,
}

impl CameraProjection for PerspectiveProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        let mut matrix =
            Mat4::perspective_infinite_reverse_rh(self.fov, self.aspect_ratio, self.near);
        self.adjust_perspective_matrix_for_clip_plane(&mut matrix);
        matrix
    }

    fn get_clip_from_view_for_sub(&self, sub_view: &super::SubCameraView) -> Mat4 {
        let full_width = sub_view.full_size.x as f32;
        let full_height = sub_view.full_size.y as f32;
        let sub_width = sub_view.size.x as f32;
        let sub_height = sub_view.size.y as f32;
        let offset_x = sub_view.offset.x;
        let offset_y = full_height - (sub_view.offset.y + sub_height);

        let full_aspect = full_width / full_height;

        let top = self.near * ops::tan(0.5 * self.fov);
        let bottom = -top;
        let right = top * full_aspect;
        let left = -right;

        let width = right - left;
        let height = top - bottom;

        let left_prime = left + (width * offset_x) / full_width;
        let right_prime = left + (width * (offset_x + sub_width)) / full_width;
        let bottom_prime = bottom + (height * offset_y) / full_height;
        let top_prime = bottom + (height * (offset_y + sub_height)) / full_height;

        let x = (2.0 * self.near) / (right_prime - left_prime);
        let y = (2.0 * self.near) / (top_prime - bottom_prime);
        let a = (right_prime + left_prime) / (right_prime - left_prime);
        let b = (top_prime + bottom_prime) / (top_prime - bottom_prime);

        let mut matrix = Mat4::from_cols(
            Vec4::new(x, 0.0, 0.0, 0.0),
            Vec4::new(0.0, y, 0.0, 0.0),
            Vec4::new(a, b, 0.0, -1.0),
            Vec4::new(0.0, 0.0, self.near, 0.0),
        );

        self.adjust_perspective_matrix_for_clip_plane(&mut matrix);
        matrix
    }

    fn update(&mut self, width: f32, height: f32) {
        self.aspect_ratio = AspectRatio::try_new(width, height)
            .expect("Failed to update PerspectiveProjection: width and height must be positive, non-zero values")
            .ratio();
    }

    fn far(&self) -> f32 {
        self.far
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        let tan_half_fov = ops::tan(self.fov / 2.);
        let a = z_near.abs() * tan_half_fov;
        let b = z_far.abs() * tan_half_fov;
        let aspect_ratio = self.aspect_ratio;
        [
            Vec3A::new(a * aspect_ratio, -a, z_near),
            Vec3A::new(a * aspect_ratio, a, z_near),
            Vec3A::new(-a * aspect_ratio, a, z_near),
            Vec3A::new(-a * aspect_ratio, -a, z_near),
            Vec3A::new(b * aspect_ratio, -b, z_far),
            Vec3A::new(b * aspect_ratio, b, z_far),
            Vec3A::new(-b * aspect_ratio, b, z_far),
            Vec3A::new(-b * aspect_ratio, -b, z_far),
        ]
    }
}

impl Default for PerspectiveProjection {
    fn default() -> Self {
        PerspectiveProjection {
            fov: core::f32::consts::PI / 4.0,
            near: 0.1,
            far: 1000.0,
            aspect_ratio: 1.0,
            near_clip_plane: vec4(0.0, 0.0, -1.0, -0.1),
        }
    }
}

impl PerspectiveProjection {
    fn adjust_perspective_matrix_for_clip_plane(&self, matrix: &mut Mat4) {
        if self.near_clip_plane == vec4(0.0, 0.0, -1.0, -self.near) {
            return;
        }

        let c = self.near_clip_plane;

        let q_prime = vec4(c.x.signum(), c.y.signum(), 0.0, 1.0);

        let q = matrix.inverse() * q_prime;

        let a = -q.z / c.dot(q);

        let m3_prime = Vec4::NEG_Z - c * a;

        matrix.x_axis.z = m3_prime.x;
        matrix.y_axis.z = m3_prime.y;
        matrix.z_axis.z = m3_prime.z;
        matrix.w_axis.z = m3_prime.w;
    }
}

#[derive(Default, Debug, Clone, Copy, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize, Default, Clone)]
pub enum ScalingMode {
    #[default]
    WindowSize,
    Fixed { width: f32, height: f32 },
    AutoMin { min_width: f32, min_height: f32 },
    AutoMax { max_width: f32, max_height: f32 },
    FixedVertical { viewport_height: f32 },
    FixedHorizontal { viewport_width: f32 },
}

#[derive(Debug, Clone, Reflect)]
#[reflect(Debug, FromWorld, Clone)]
pub struct OrthographicProjection {
    pub near: f32,
    pub far: f32,
    pub viewport_origin: Vec2,
    pub scaling_mode: ScalingMode,
    pub scale: f32,
    pub area: Rect,
}

impl CameraProjection for OrthographicProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        Mat4::orthographic_rh(
            self.area.min.x,
            self.area.max.x,
            self.area.min.y,
            self.area.max.y,
            self.far,
            self.near,
        )
    }

    fn get_clip_from_view_for_sub(&self, sub_view: &super::SubCameraView) -> Mat4 {
        let full_width = sub_view.full_size.x as f32;
        let full_height = sub_view.full_size.y as f32;
        let offset_x = sub_view.offset.x;
        let offset_y = sub_view.offset.y;
        let sub_width = sub_view.size.x as f32;
        let sub_height = sub_view.size.y as f32;

        let full_aspect = full_width / full_height;

        let top = self.area.max.y;
        let bottom = self.area.min.y;
        let ortho_height = top - bottom;
        let ortho_width = ortho_height * full_aspect;

        let center_x = (self.area.max.x + self.area.min.x) / 2.0;
        let left = center_x - ortho_width / 2.0;
        let right = center_x + ortho_width / 2.0;

        let scale_w = (right - left) / full_width;
        let scale_h = (top - bottom) / full_height;

        let left_prime = left + scale_w * offset_x;
        let right_prime = left_prime + scale_w * sub_width;
        let top_prime = top - scale_h * offset_y;
        let bottom_prime = top_prime - scale_h * sub_height;

        Mat4::orthographic_rh(
            left_prime,
            right_prime,
            bottom_prime,
            top_prime,
            self.far,
            self.near,
        )
    }

    fn update(&mut self, width: f32, height: f32) {
        let (projection_width, projection_height) = match self.scaling_mode {
            ScalingMode::WindowSize => (width, height),
            ScalingMode::AutoMin {
                min_width,
                min_height,
            } => {
                if width * min_height > min_width * height {
                    (width * min_height / height, min_height)
                } else {
                    (min_width, height * min_width / width)
                }
            }
            ScalingMode::AutoMax {
                max_width,
                max_height,
            } => {
                if width * max_height < max_width * height {
                    (width * max_height / height, max_height)
                } else {
                    (max_width, height * max_width / width)
                }
            }
            ScalingMode::FixedVertical { viewport_height } => {
                (width * viewport_height / height, viewport_height)
            }
            ScalingMode::FixedHorizontal { viewport_width } => {
                (viewport_width, height * viewport_width / width)
            }
            ScalingMode::Fixed { width, height } => (width, height),
        };

        let origin_x = projection_width * self.viewport_origin.x;
        let origin_y = projection_height * self.viewport_origin.y;

        self.area = Rect::new(
            self.scale * -origin_x,
            self.scale * -origin_y,
            self.scale * (projection_width - origin_x),
            self.scale * (projection_height - origin_y),
        );
    }

    fn far(&self) -> f32 {
        self.far
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        let area = self.area;
        [
            Vec3A::new(area.max.x, area.min.y, z_near),
            Vec3A::new(area.max.x, area.max.y, z_near),
            Vec3A::new(area.min.x, area.max.y, z_near),
            Vec3A::new(area.min.x, area.min.y, z_near),
            Vec3A::new(area.max.x, area.min.y, z_far),
            Vec3A::new(area.max.x, area.max.y, z_far),
            Vec3A::new(area.min.x, area.max.y, z_far),
            Vec3A::new(area.min.x, area.min.y, z_far),
        ]
    }
}

impl FromWorld for OrthographicProjection {
    fn from_world(_world: &mut World) -> Self {
        OrthographicProjection::default_3d()
    }
}

impl OrthographicProjection {
    pub fn default_2d() -> Self {
        OrthographicProjection {
            near: -1000.0,
            ..OrthographicProjection::default_3d()
        }
    }

    pub fn default_3d() -> Self {
        OrthographicProjection {
            scale: 1.0,
            near: 0.0,
            far: 1000.0,
            viewport_origin: Vec2::new(0.5, 0.5),
            scaling_mode: ScalingMode::WindowSize,
            area: Rect::new(-1.0, -1.0, 1.0, 1.0),
        }
    }
}
