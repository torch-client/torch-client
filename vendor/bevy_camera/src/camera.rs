use crate::primitives::Frustum;

use super::{
    visibility::{Visibility, VisibleEntities},
    ClearColorConfig, MsaaWriteback,
};
use bevy_asset::Handle;
use bevy_derive::Deref;
use bevy_ecs::{component::Component, entity::Entity, reflect::ReflectComponent};
use bevy_image::Image;
use bevy_math::{ops, Dir3, FloatOrd, Mat4, Ray3d, Rect, URect, UVec2, Vec2, Vec3, Vec3A};
use bevy_reflect::prelude::*;
use bevy_transform::components::{GlobalTransform, Transform};
use bevy_window::{NormalizedWindowRef, WindowRef};
use core::ops::Range;
use derive_more::derive::From;
use thiserror::Error;
use wgpu_types::{BlendState, TextureUsages};

#[derive(Reflect, Debug, Clone)]
#[reflect(Default, Clone)]
pub struct Viewport {
    pub physical_position: UVec2,
    pub physical_size: UVec2,
    pub depth: Range<f32>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            physical_position: Default::default(),
            physical_size: UVec2::new(1, 1),
            depth: 0.0..1.0,
        }
    }
}

impl Viewport {
    pub fn clamp_to_size(&mut self, size: UVec2) {
        if self.physical_size.x + self.physical_position.x > size.x {
            if self.physical_position.x < size.x {
                self.physical_size.x = size.x - self.physical_position.x;
            } else if size.x > 0 {
                self.physical_position.x = size.x - 1;
                self.physical_size.x = 1;
            } else {
                self.physical_position.x = 0;
                self.physical_size.x = 0;
            }
        }
        if self.physical_size.y + self.physical_position.y > size.y {
            if self.physical_position.y < size.y {
                self.physical_size.y = size.y - self.physical_position.y;
            } else if size.y > 0 {
                self.physical_position.y = size.y - 1;
                self.physical_size.y = 1;
            } else {
                self.physical_position.y = 0;
                self.physical_size.y = 0;
            }
        }
    }

    pub fn from_viewport_and_override(
        viewport: Option<&Self>,
        main_pass_resolution_override: Option<&MainPassResolutionOverride>,
    ) -> Option<Self> {
        if let Some(override_size) = main_pass_resolution_override {
            let mut vp = viewport.map_or_else(Self::default, Self::clone);
            vp.physical_size = **override_size;
            Some(vp)
        } else {
            viewport.cloned()
        }
    }
}

#[derive(Component, Reflect, Deref, Debug)]
#[reflect(Component)]
pub struct MainPassResolutionOverride(pub UVec2);

#[derive(Debug, Clone, Copy, Reflect, PartialEq)]
#[reflect(Clone, PartialEq, Default)]
pub struct SubCameraView {
    pub full_size: UVec2,
    pub offset: Vec2,
    pub size: UVec2,
}

impl Default for SubCameraView {
    fn default() -> Self {
        Self {
            full_size: UVec2::new(1, 1),
            offset: Vec2::new(0., 0.),
            size: UVec2::new(1, 1),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderTargetInfo {
    pub physical_size: UVec2,
    pub scale_factor: f32,
}

impl Default for RenderTargetInfo {
    fn default() -> Self {
        Self {
            physical_size: Default::default(),
            scale_factor: 1.,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct ComputedCameraValues {
    pub clip_from_view: Mat4,
    pub target_info: Option<RenderTargetInfo>,
    pub old_viewport_size: Option<UVec2>,
    pub old_sub_camera_view: Option<SubCameraView>,
}

#[derive(Component, Clone, Copy, Reflect)]
#[reflect(opaque)]
#[reflect(Component, Default, Clone)]
pub struct Exposure {
    pub ev100: f32,
}

impl Exposure {
    pub const SUNLIGHT: Self = Self {
        ev100: Self::EV100_SUNLIGHT,
    };
    pub const OVERCAST: Self = Self {
        ev100: Self::EV100_OVERCAST,
    };
    pub const INDOOR: Self = Self {
        ev100: Self::EV100_INDOOR,
    };
    pub const BLENDER: Self = Self {
        ev100: Self::EV100_BLENDER,
    };

    pub const EV100_SUNLIGHT: f32 = 15.0;
    pub const EV100_OVERCAST: f32 = 12.0;
    pub const EV100_INDOOR: f32 = 7.0;

    pub const EV100_BLENDER: f32 = 9.7;

    pub fn from_physical_camera(physical_camera_parameters: PhysicalCameraParameters) -> Self {
        Self {
            ev100: physical_camera_parameters.ev100(),
        }
    }

    #[inline]
    pub fn exposure(&self) -> f32 {
        ops::exp2(-self.ev100) / 1.2
    }
}

impl Default for Exposure {
    fn default() -> Self {
        Self::BLENDER
    }
}

#[derive(Clone, Copy)]
pub struct PhysicalCameraParameters {
    pub aperture_f_stops: f32,
    pub shutter_speed_s: f32,
    pub sensitivity_iso: f32,
    pub sensor_height: f32,
}

impl PhysicalCameraParameters {
    pub fn ev100(&self) -> f32 {
        ops::log2(
            self.aperture_f_stops * self.aperture_f_stops * 100.0
                / (self.shutter_speed_s * self.sensitivity_iso),
        )
    }
}

impl Default for PhysicalCameraParameters {
    fn default() -> Self {
        Self {
            aperture_f_stops: 1.0,
            shutter_speed_s: 1.0 / 125.0,
            sensitivity_iso: 100.0,
            sensor_height: 0.01866,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Error)]
pub enum ViewportConversionError {
    #[error("pre-computed size of viewport not available")]
    NoViewportSize,
    #[error("computed coordinate beyond `Camera`'s near plane")]
    PastNearPlane,
    #[error("computed coordinate beyond `Camera`'s far plane")]
    PastFarPlane,
    #[error("found NaN while computing NDC")]
    InvalidData,
}

#[derive(Component, Debug, Reflect, Clone)]
#[reflect(Component, Default, Debug, Clone)]
#[require(
    Frustum,
    CameraMainTextureUsages,
    VisibleEntities,
    Transform,
    Visibility,
    RenderTarget
)]
pub struct Camera {
    pub viewport: Option<Viewport>,
    pub order: isize,
    pub is_active: bool,
    #[reflect(ignore, clone)]
    pub computed: ComputedCameraValues,
    pub output_mode: CameraOutputMode,
    pub msaa_writeback: MsaaWriteback,
    pub clear_color: ClearColorConfig,
    pub invert_culling: bool,
    pub sub_camera_view: Option<SubCameraView>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            is_active: true,
            order: 0,
            viewport: None,
            computed: Default::default(),
            output_mode: Default::default(),
            msaa_writeback: MsaaWriteback::default(),
            clear_color: Default::default(),
            invert_culling: false,
            sub_camera_view: None,
        }
    }
}

impl Camera {
    #[inline]
    pub fn to_logical(&self, physical_size: UVec2) -> Option<Vec2> {
        let scale = self.computed.target_info.as_ref()?.scale_factor;
        Some(physical_size.as_vec2() / scale)
    }

    #[inline]
    pub fn physical_viewport_rect(&self) -> Option<URect> {
        let min = self
            .viewport
            .as_ref()
            .map(|v| v.physical_position)
            .unwrap_or(UVec2::ZERO);
        let max = min + self.physical_viewport_size()?;
        Some(URect { min, max })
    }

    #[inline]
    pub fn logical_viewport_rect(&self) -> Option<Rect> {
        let URect { min, max } = self.physical_viewport_rect()?;
        Some(Rect {
            min: self.to_logical(min)?,
            max: self.to_logical(max)?,
        })
    }

    #[inline]
    pub fn logical_viewport_size(&self) -> Option<Vec2> {
        self.viewport
            .as_ref()
            .and_then(|v| self.to_logical(v.physical_size))
            .or_else(|| self.logical_target_size())
    }

    #[inline]
    pub fn physical_viewport_size(&self) -> Option<UVec2> {
        self.viewport
            .as_ref()
            .map(|v| v.physical_size)
            .or_else(|| self.physical_target_size())
    }

    #[inline]
    pub fn logical_target_size(&self) -> Option<Vec2> {
        self.computed
            .target_info
            .as_ref()
            .and_then(|t| self.to_logical(t.physical_size))
    }

    #[inline]
    pub fn physical_target_size(&self) -> Option<UVec2> {
        self.computed.target_info.as_ref().map(|t| t.physical_size)
    }

    #[inline]
    pub fn target_scaling_factor(&self) -> Option<f32> {
        self.computed
            .target_info
            .as_ref()
            .map(|t: &RenderTargetInfo| t.scale_factor)
    }

    #[inline]
    pub fn clip_from_view(&self) -> Mat4 {
        self.computed.clip_from_view
    }

    fn world_to_viewport_core(
        &self,
        camera_transform: &GlobalTransform,
        world_position: Vec3,
    ) -> Result<(Vec2, f32), ViewportConversionError> {
        let target_rect = self
            .logical_viewport_rect()
            .ok_or(ViewportConversionError::NoViewportSize)?;
        let mut ndc_space_coords = self
            .world_to_ndc(camera_transform, world_position)
            .ok_or(ViewportConversionError::InvalidData)?;
        if ndc_space_coords.z < 0.0 {
            return Err(ViewportConversionError::PastFarPlane);
        }
        if ndc_space_coords.z > 1.0 {
            return Err(ViewportConversionError::PastNearPlane);
        }

        let depth = ndc_space_coords.z;

        ndc_space_coords.y = -ndc_space_coords.y;

        let viewport_position =
            (ndc_space_coords.truncate() + Vec2::ONE) / 2.0 * target_rect.size() + target_rect.min;
        Ok((viewport_position, depth))
    }

    #[doc(alias = "world_to_screen")]
    pub fn world_to_viewport(
        &self,
        camera_transform: &GlobalTransform,
        world_position: Vec3,
    ) -> Result<Vec2, ViewportConversionError> {
        Ok(self
            .world_to_viewport_core(camera_transform, world_position)?
            .0)
    }

    #[doc(alias = "world_to_screen_with_depth")]
    pub fn world_to_viewport_with_depth(
        &self,
        camera_transform: &GlobalTransform,
        world_position: Vec3,
    ) -> Result<Vec3, ViewportConversionError> {
        let result = self.world_to_viewport_core(camera_transform, world_position)?;
        let depth = -self.depth_ndc_to_view_z(result.1);
        Ok(result.0.extend(depth))
    }

    pub fn viewport_to_world(
        &self,
        camera_transform: &GlobalTransform,
        viewport_position: Vec2,
    ) -> Result<Ray3d, ViewportConversionError> {
        let ndc_xy = self.viewport_to_ndc(viewport_position)?;

        let ndc_point_near = ndc_xy.extend(1.0).into();
        let ndc_point_far = ndc_xy.extend(f32::EPSILON).into();
        let view_from_clip = self.computed.clip_from_view.inverse();
        let world_from_view = camera_transform.affine();
        let view_point_near = view_from_clip.project_point3a(ndc_point_near);
        let view_point_far = view_from_clip.project_point3a(ndc_point_far);
        let view_dir = view_point_far - view_point_near;
        let origin = world_from_view.transform_point3a(view_point_near).into();
        let direction = world_from_view.transform_vector3a(view_dir).into();

        Dir3::new(direction)
            .map_err(|_| ViewportConversionError::InvalidData)
            .map(|direction| Ray3d { origin, direction })
    }

    pub fn viewport_to_world_2d(
        &self,
        camera_transform: &GlobalTransform,
        viewport_position: Vec2,
    ) -> Result<Vec2, ViewportConversionError> {
        let ndc = self.viewport_to_ndc(viewport_position)?;

        let world_near_plane = self
            .ndc_to_world(camera_transform, ndc.extend(1.))
            .ok_or(ViewportConversionError::InvalidData)?;

        Ok(world_near_plane.truncate())
    }

    pub fn world_to_ndc<V: Into<Vec3A> + From<Vec3A>>(
        &self,
        camera_transform: &GlobalTransform,
        world_point: V,
    ) -> Option<V> {
        let view_from_world = camera_transform.affine().inverse();
        let view_point = view_from_world.transform_point3a(world_point.into());
        let ndc_point = self.computed.clip_from_view.project_point3a(view_point);

        (!ndc_point.is_nan()).then_some(ndc_point.into())
    }

    pub fn ndc_to_world<V: Into<Vec3A> + From<Vec3A>>(
        &self,
        camera_transform: &GlobalTransform,
        ndc_point: V,
    ) -> Option<V> {
        let view_point = self
            .computed
            .clip_from_view
            .inverse()
            .project_point3a(ndc_point.into());
        let world_point = camera_transform.affine().transform_point3a(view_point);

        (!world_point.is_nan()).then_some(world_point.into())
    }

    pub fn depth_ndc_to_view_z(&self, ndc_depth: f32) -> f32 {
        let near = self.clip_from_view().w_axis.z;
        -near / ndc_depth
    }

    pub fn depth_ndc_to_view_z_2d(&self, ndc_depth: f32) -> f32 {
        -(self.clip_from_view().w_axis.z - ndc_depth) / self.clip_from_view().z_axis.z
    }

    pub fn viewport_to_ndc(
        &self,
        viewport_position: Vec2,
    ) -> Result<Vec2, ViewportConversionError> {
        let target_rect = self
            .logical_viewport_rect()
            .ok_or(ViewportConversionError::NoViewportSize)?;
        let rect_relative = (viewport_position - target_rect.min) / target_rect.size();
        let mut ndc = rect_relative * 2. - Vec2::ONE;
        ndc.y = -ndc.y;
        Ok(ndc)
    }
}

#[derive(Debug, Clone, Copy, Reflect)]
pub enum CameraOutputMode {
    Write {
        blend_state: Option<BlendState>,
        clear_color: ClearColorConfig,
    },
    Skip,
}

impl Default for CameraOutputMode {
    fn default() -> Self {
        CameraOutputMode::Write {
            blend_state: None,
            clear_color: ClearColorConfig::Default,
        }
    }
}

#[derive(Component, Debug, Clone, Reflect, From)]
#[reflect(Clone, Component)]
pub enum RenderTarget {
    Window(WindowRef),
    Image(ImageRenderTarget),
    TextureView(ManualTextureViewHandle),
    None {
        size: UVec2,
    },
}

impl RenderTarget {
    pub fn as_image(&self) -> Option<&Handle<Image>> {
        if let Self::Image(image_target) = self {
            Some(&image_target.handle)
        } else {
            None
        }
    }
}

impl RenderTarget {
    pub fn normalize(&self, primary_window: Option<Entity>) -> Option<NormalizedRenderTarget> {
        match self {
            RenderTarget::Window(window_ref) => window_ref
                .normalize(primary_window)
                .map(NormalizedRenderTarget::Window),
            RenderTarget::Image(handle) => Some(NormalizedRenderTarget::Image(handle.clone())),
            RenderTarget::TextureView(id) => Some(NormalizedRenderTarget::TextureView(*id)),
            RenderTarget::None { size } => Some(NormalizedRenderTarget::None {
                width: size.x,
                height: size.y,
            }),
        }
    }
}

#[derive(Debug, Clone, Reflect, PartialEq, Eq, Hash, PartialOrd, Ord, From)]
#[reflect(Clone, PartialEq, Hash)]
pub enum NormalizedRenderTarget {
    Window(NormalizedWindowRef),
    Image(ImageRenderTarget),
    TextureView(ManualTextureViewHandle),
    None {
        width: u32,
        height: u32,
    },
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Component, Reflect)]
#[reflect(Component, Default, Debug, PartialEq, Hash, Clone)]
pub struct ManualTextureViewHandle(pub u32);

#[derive(Debug, Clone, Reflect)]
#[reflect(Clone, PartialEq, Hash)]
pub struct ImageRenderTarget {
    pub handle: Handle<Image>,
    pub scale_factor: f32,
}

impl Eq for ImageRenderTarget {}

impl PartialEq for ImageRenderTarget {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle && FloatOrd(self.scale_factor) == FloatOrd(other.scale_factor)
    }
}

impl core::hash::Hash for ImageRenderTarget {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.handle.hash(state);
        FloatOrd(self.scale_factor).hash(state);
    }
}

impl PartialOrd for ImageRenderTarget {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ImageRenderTarget {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.handle
            .cmp(&other.handle)
            .then_with(|| FloatOrd(self.scale_factor).cmp(&FloatOrd(other.scale_factor)))
    }
}

impl From<Handle<Image>> for RenderTarget {
    fn from(handle: Handle<Image>) -> Self {
        Self::Image(handle.into())
    }
}

impl From<Handle<Image>> for ImageRenderTarget {
    fn from(handle: Handle<Image>) -> Self {
        Self {
            handle,
            scale_factor: 1.0,
        }
    }
}

impl Default for RenderTarget {
    fn default() -> Self {
        Self::Window(Default::default())
    }
}

#[derive(Component, Clone, Copy, Reflect)]
#[reflect(opaque)]
#[reflect(Component, Default, Clone)]
pub struct CameraMainTextureUsages(pub TextureUsages);

impl Default for CameraMainTextureUsages {
    fn default() -> Self {
        Self(
            TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
        )
    }
}

impl CameraMainTextureUsages {
    pub fn with(mut self, usages: TextureUsages) -> Self {
        self.0 |= usages;
        self
    }
}

#[cfg(test)]
mod test {
    use bevy_math::{Vec2, Vec3};
    use bevy_transform::components::GlobalTransform;

    use crate::{
        Camera, OrthographicProjection, PerspectiveProjection, Projection, RenderTargetInfo,
        Viewport,
    };

    fn make_camera(mut projection: Projection, physical_size: Vec2) -> Camera {
        let viewport = Viewport {
            physical_size: physical_size.as_uvec2(),
            ..Default::default()
        };
        let mut camera = Camera {
            viewport: Some(viewport.clone()),
            ..Default::default()
        };
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: viewport.physical_size,
            scale_factor: 1.0,
        });
        projection.update(
            viewport.physical_size.x as f32,
            viewport.physical_size.y as f32,
        );
        camera.computed.clip_from_view = projection.get_clip_from_view();
        camera
    }

    #[test]
    fn viewport_to_world_orthographic_3d_returns_forward() {
        let transform = GlobalTransform::default();
        let size = Vec2::new(1600.0, 900.0);
        let camera = make_camera(
            Projection::Orthographic(OrthographicProjection::default_3d()),
            size,
        );
        let ray = camera.viewport_to_world(&transform, Vec2::ZERO).unwrap();
        assert_eq!(ray.direction, transform.forward());
        assert!(ray
            .origin
            .abs_diff_eq(Vec3::new(-size.x * 0.5, size.y * 0.5, 0.0), 1e-4));
        let ray = camera.viewport_to_world(&transform, size).unwrap();
        assert_eq!(ray.direction, transform.forward());
        assert!(ray
            .origin
            .abs_diff_eq(Vec3::new(size.x * 0.5, -size.y * 0.5, 0.0), 1e-4));
    }

    #[test]
    fn viewport_to_world_orthographic_2d_returns_forward() {
        let transform = GlobalTransform::default();
        let size = Vec2::new(1600.0, 900.0);
        let camera = make_camera(
            Projection::Orthographic(OrthographicProjection::default_2d()),
            size,
        );
        let ray = camera.viewport_to_world(&transform, Vec2::ZERO).unwrap();
        assert_eq!(ray.direction, transform.forward());
        assert!(ray
            .origin
            .abs_diff_eq(Vec3::new(-size.x * 0.5, size.y * 0.5, 1000.0), 1e-4));
        let ray = camera.viewport_to_world(&transform, size).unwrap();
        assert_eq!(ray.direction, transform.forward());
        assert!(ray
            .origin
            .abs_diff_eq(Vec3::new(size.x * 0.5, -size.y * 0.5, 1000.0), 1e-4));
    }

    #[test]
    fn viewport_to_world_perspective_center_returns_forward() {
        let transform = GlobalTransform::default();
        let size = Vec2::new(1600.0, 900.0);
        let camera = make_camera(
            Projection::Perspective(PerspectiveProjection::default()),
            size,
        );
        let ray = camera.viewport_to_world(&transform, size * 0.5).unwrap();
        assert_eq!(ray.direction, transform.forward());
        assert_eq!(ray.origin, transform.forward() * 0.1);
    }
}
