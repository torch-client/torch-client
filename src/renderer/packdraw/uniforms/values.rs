use bevy::math::{IVec2, IVec3, Mat4, Vec2, Vec3, Vec4};
use bevy::render::view::ExtractedView;

use super::builtins::{BUILTINS, Builtin, Value};
use super::{BlockHead, MatrixSet};
use crate::renderer::packdraw::inputs::{Inputs, MAX_ARMOR, MAX_HUNGER};
use crate::shaderpack::directives::Constants;
use crate::shaderpack::expressions::Smooth;

const FRAME_COUNTER_WRAP: u32 = 720_720;

pub(crate) const NEAR: f32 = 0.05;

const FAR_PLANE_PER_BLOCK: f32 = 4.0;

const DAY_TICKS: f64 = 24000.0;

const HAND_FOV_DEGREES: f32 = 70.0;

const CAMERA_WALK_RANGE: f64 = 30000.0;
const CAMERA_TP_RANGE: f64 = 1000.0;

fn camera_shift(value: f64, previous: f64) -> f64 {
    if value.abs() > CAMERA_WALK_RANGE || (value - previous).abs() > CAMERA_TP_RANGE {
        -(value - value % CAMERA_WALK_RANGE)
    } else {
        0.0
    }
}

fn shifted_camera(history: &mut History, camera: bevy::math::DVec3) -> (Vec3, Vec3) {
    let shift = bevy::math::DVec3::new(history.camera_shift.x, 0.0, history.camera_shift.y);
    let mut current = camera + shift;
    let mut previous = history.shifted_previous.unwrap_or(current);
    let dx = camera_shift(current.x, previous.x);
    let dz = camera_shift(current.z, previous.z);
    if dx != 0.0 || dz != 0.0 {
        history.camera_shift += bevy::math::DVec2::new(dx, dz);
        current += bevy::math::DVec3::new(dx, 0.0, dz);
        previous += bevy::math::DVec3::new(dx, 0.0, dz);
    }
    history.shifted_previous = Some(current);
    (current.as_vec3(), previous.as_vec3())
}

fn split_position(position: bevy::math::DVec3) -> (IVec3, Vec3) {
    let floor = position.floor();
    (floor.as_ivec3(), (position - floor).as_vec3())
}

fn hand_projection(aspect: f32, far_plane: f32) -> Mat4 {
    Mat4::from_scale(Vec3::new(1.0, 1.0, 0.125))
        * Mat4::perspective_rh_gl(HAND_FOV_DEGREES.to_radians(), aspect, NEAR, far_plane)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct History {
    previous: Option<(Mat4, Mat4)>,
    camera_shift: bevy::math::DVec2,
    shifted_previous: Option<bevy::math::DVec3>,
    unshifted_previous: Option<bevy::math::DVec3>,
    eye_brightness: [Smooth; 2],
    wetness: Smooth,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FrameValues {
    view_rotation: Mat4,
    view_rotation_inverse: Mat4,
    projection_inverse: Mat4,
    shadow_model_view_inverse: Mat4,
    shadow_projection_inverse: Mat4,
    projection: Mat4,
    previous_view_rotation: Mat4,
    previous_projection: Mat4,
    camera: Vec3,
    camera_int: IVec3,
    camera_fract: Vec3,
    previous_camera_int: IVec3,
    previous_camera_fract: Vec3,
    camera_uniform: Vec3,
    previous_camera_uniform: Vec3,
    size: Vec2,
    time_counter: f32,
    frame_time: f32,
    frame_counter: u32,
    far: f32,
    sun_angle: f32,
    shadow_angle: f32,
    sun_position: Vec3,
    moon_position: Vec3,
    up_position: Vec3,
    is_day: bool,
    shadow_model_view: Mat4,
    shadow_projection: Mat4,
    hand_projection: Mat4,
    eye_brightness_smooth: IVec2,
    wetness: f32,
    inputs: Inputs,
}

fn celestial_fraction(angle: f32) -> f32 {
    (angle.to_degrees() + 90.0).rem_euclid(360.0) / 360.0
}

impl FrameValues {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn of(
        view: &ExtractedView,
        time_counter: f32,
        frame_time: f32,
        frames: u32,
        far: f32,
        inputs: Inputs,
        constants: &Constants,
        history: &mut History,
    ) -> FrameValues {
        let mut view_rotation = view.world_from_view.to_matrix().inverse();
        view_rotation.w_axis = Vec4::W;
        let size = Vec2::new(view.viewport.z as f32, view.viewport.w as f32).max(Vec2::ONE);
        let focal = view.clip_from_view.y_axis.y.max(f32::EPSILON);
        let fov = 2.0 * (1.0 / focal).atan();
        let far_plane = (far * FAR_PLANE_PER_BLOCK).max(NEAR * 2.0);
        let projection = Mat4::perspective_rh_gl(fov, size.x / size.y, NEAR, far_plane);
        let camera = view.world_from_view.translation();

        let (previous_view_rotation, previous_projection) =
            history.previous.unwrap_or((view_rotation, projection));
        history.previous = Some((view_rotation, projection));
        let (camera_uniform, previous_camera_uniform) = shifted_camera(history, camera.as_dvec3());
        let previous_unshifted = history.unshifted_previous.unwrap_or(camera.as_dvec3());
        history.unshifted_previous = Some(camera.as_dvec3());
        let (camera_int, camera_fract) = split_position(camera.as_dvec3());
        let (previous_camera_int, previous_camera_fract) = split_position(previous_unshifted);

        let celestial = |angle: f32| -> Vec3 {
            let m = view_rotation
                * Mat4::from_rotation_y((-90.0f32).to_radians())
                * Mat4::from_rotation_z(constants.sun_path_rotation.to_radians())
                * Mat4::from_rotation_x(angle);
            m.transform_point3(Vec3::new(0.0, 100.0, 0.0))
        };
        let sun_angle = celestial_fraction(inputs.sun_angle);
        let is_day = sun_angle < 0.5;
        let shadow_angle = if is_day {
            sun_angle
        } else {
            celestial_fraction(inputs.moon_angle)
        };

        let rain = inputs.rain;
        let wetness = history.wetness.update(
            rain,
            constants.wetness_half_life,
            constants.dryness_half_life,
            frame_time,
        );
        let brightness = inputs.eye_light * 16;
        let smooth_brightness = [0, 1].map(|axis| {
            history.eye_brightness[axis].update(
                brightness[axis] as f32,
                constants.eye_brightness_half_life,
                constants.eye_brightness_half_life,
                frame_time,
            ) as i32
        });

        let shadow_model_view = shadow_model_view(shadow_angle, constants, camera);
        let shadow_projection = Mat4::orthographic_rh_gl(
            -constants.shadow_distance,
            constants.shadow_distance,
            -constants.shadow_distance,
            constants.shadow_distance,
            constants.shadow_near_plane,
            constants.shadow_far_plane,
        );
        FrameValues {
            view_rotation,
            view_rotation_inverse: view_rotation.inverse(),
            projection_inverse: projection.inverse(),
            shadow_model_view_inverse: shadow_model_view.inverse(),
            shadow_projection_inverse: shadow_projection.inverse(),
            projection,
            previous_view_rotation,
            previous_projection,
            camera,
            camera_int,
            camera_fract,
            previous_camera_int,
            previous_camera_fract,
            camera_uniform,
            previous_camera_uniform,
            size,
            time_counter,
            frame_time,
            frame_counter: frames % FRAME_COUNTER_WRAP,
            far,
            sun_angle,
            shadow_angle,
            sun_position: celestial(inputs.sun_angle),
            moon_position: celestial(inputs.moon_angle),
            up_position: view_rotation.transform_vector3(Vec3::new(0.0, 100.0, 0.0)),
            is_day,
            shadow_model_view,
            shadow_projection,
            hand_projection: hand_projection(size.x / size.y, far_plane),
            eye_brightness_smooth: IVec2::from(smooth_brightness),
            wetness,
            inputs,
        }
    }

    pub(crate) fn camera_clip(&self) -> Mat4 {
        self.projection * self.view_rotation
    }

    pub(crate) fn toward_light(&self) -> Vec3 {
        self.shadow_model_view_inverse
            .transform_vector3(Vec3::Z)
            .normalize_or_zero()
    }

    pub(crate) fn camera(&self) -> Vec3 {
        self.camera
    }

    pub(crate) fn get(&self, builtin: Builtin) -> Value {
        use Builtin as B;
        let inputs = &self.inputs;
        match builtin {
            B::ModelView => Value::Mat4(self.view_rotation),
            B::ModelViewInverse => Value::Mat4(self.view_rotation_inverse),
            B::Projection => Value::Mat4(self.projection),
            B::ProjectionInverse => Value::Mat4(self.projection_inverse),
            B::PreviousModelView => Value::Mat4(self.previous_view_rotation),
            B::PreviousProjection => Value::Mat4(self.previous_projection),
            B::CameraPosition => Value::Vec3(self.camera_uniform),
            B::PreviousCameraPosition => Value::Vec3(self.previous_camera_uniform),
            B::CameraPositionInt => Value::IVec3(self.camera_int),
            B::CameraPositionFract => Value::Vec3(self.camera_fract),
            B::PreviousCameraPositionInt => Value::IVec3(self.previous_camera_int),
            B::PreviousCameraPositionFract => Value::Vec3(self.previous_camera_fract),
            B::EyeAltitude => Value::Float(self.camera.y),
            B::RelativeEyePosition => Value::Vec3(inputs.relative_eye),
            B::ViewWidth => Value::Float(self.size.x),
            B::ViewHeight => Value::Float(self.size.y),
            B::AspectRatio => Value::Float(self.size.x / self.size.y),
            B::Near => Value::Float(NEAR),
            B::Far => Value::Float(self.far),
            B::FrameTimeCounter => Value::Float(self.time_counter),
            B::FrameTime => Value::Float(self.frame_time),
            B::FrameCounter => Value::Int(self.frame_counter as i32),
            B::WorldTime => Value::Int(inputs.day_ticks.rem_euclid(DAY_TICKS) as i32),
            B::WorldDay => Value::Int((inputs.day_ticks / DAY_TICKS).floor() as i32),
            B::MoonPhase => Value::Int(inputs.moon_phase),
            B::SunAngle => Value::Float(self.sun_angle),
            B::ShadowAngle => Value::Float(self.shadow_angle),
            B::SunPosition => Value::Vec3(self.sun_position),
            B::MoonPosition => Value::Vec3(self.moon_position),
            B::ShadowLightPosition => Value::Vec3(if self.is_day {
                self.sun_position
            } else {
                self.moon_position
            }),
            B::UpPosition => Value::Vec3(self.up_position),
            B::SkyColor => Value::Vec3(inputs.sky_color),
            B::FogColor => Value::Vec3(inputs.fog_color),
            B::RainStrength => Value::Float(inputs.rain),
            B::Wetness => Value::Float(self.wetness),
            B::ThunderStrength => Value::Float(inputs.thunder),
            B::EyeBrightness => Value::IVec2(inputs.eye_light * 16),
            B::EyeBrightnessSmooth => Value::IVec2(self.eye_brightness_smooth),
            B::IsEyeInWater => Value::Int(inputs.eye_in as i32),
            B::Blindness => Value::Float(inputs.blindness),
            B::NightVision => Value::Float(inputs.night_vision),
            B::DarknessFactor => Value::Float(inputs.darkness),
            B::HideGui => Value::Int(i32::from(inputs.hide_gui)),
            B::HeldItemId => Value::Int(inputs.held_items[0]),
            B::HeldItemId2 => Value::Int(inputs.held_items[1]),
            B::HeldBlockLightValue => Value::Int(inputs.held_light[0]),
            B::HeldBlockLightValue2 => Value::Int(inputs.held_light[1]),
            B::HeldBlockLightColor => Value::Vec3(Vec3::ONE),
            B::FogStart => Value::Float(inputs.fog_range.x),
            B::FogEnd => Value::Float(inputs.fog_range.y),
            B::ScreenBrightness => Value::Float(inputs.screen_brightness),
            B::PlayerBodyVector => Value::Vec3(inputs.body_vector),
            B::Rainfall => Value::Float(inputs.rainfall),
            B::Temperature => Value::Float(inputs.temperature),
            B::BiomePrecipitation => Value::Int(inputs.precipitation),
            B::RenderStage => Value::Int(0),
            B::LightningBoltPosition => Value::Vec4(inputs.lightning),
            B::NoId => Value::Int(-1),
            B::NoEntityColor => Value::Vec4(Vec4::ZERO),
            B::AtlasSize => Value::IVec2(inputs.atlas_size),
            B::Biome => Value::Int(inputs.biome.map_or(-1, i32::from)),
            B::ShadowModelView => Value::Mat4(self.shadow_model_view),
            B::ShadowModelViewInverse => Value::Mat4(self.shadow_model_view_inverse),
            B::ShadowProjection => Value::Mat4(self.shadow_projection),
            B::ShadowProjectionInverse => Value::Mat4(self.shadow_projection_inverse),
            B::BedrockLevel => Value::Int(inputs.bedrock_level),
            B::HeightLimit => Value::Int(inputs.height_limit),
            B::LogicalHeightLimit => Value::Int(inputs.logical_height),
            B::SeaLevel => Value::Int(inputs.sea_level),
            B::HasCeiling => Value::Int(i32::from(inputs.has_ceiling)),
            B::HasSkylight => Value::Int(i32::from(inputs.has_skylight)),
            B::CurrentPlayerHealth => Value::Float(inputs.vitals.health),
            B::MaxPlayerHealth => Value::Float(inputs.vitals.max_health),
            B::CurrentPlayerHunger => Value::Float(inputs.vitals.hunger),
            B::MaxPlayerHunger => Value::Float(MAX_HUNGER),
            B::CurrentPlayerAir => Value::Float(inputs.vitals.air),
            B::MaxPlayerAir => Value::Float(inputs.vitals.max_air),
            B::CurrentPlayerArmor => Value::Float(inputs.vitals.armor),
            B::MaxPlayerArmor => Value::Float(MAX_ARMOR),
            B::IsSpectator => Value::Int(i32::from(inputs.spectator)),
            B::FirstPersonCamera => Value::Int(i32::from(inputs.first_person)),
            B::IsRiding => Value::Int(i32::from(inputs.riding)),
            B::IsElytraFlying => Value::Int(i32::from(inputs.elytra_flying)),
            B::InSwimmingAnimation => Value::Int(i32::from(inputs.swimming)),
            B::FeetInWater => Value::Int(i32::from(inputs.feet_in_water)),
            B::EyePosition => Value::Vec3(inputs.eye_position),
            B::PlayerLookVector => Value::Vec3(inputs.body_vector),
            B::VehicleId => Value::Int(inputs.vehicle.id),
            B::VehicleInWater => Value::Int(i32::from(inputs.vehicle.in_water)),
            B::VehicleLookVector => Value::Vec3(inputs.vehicle.look),
            B::RelativeVehiclePosition => Value::Vec3(inputs.vehicle.relative),
        }
    }

    pub(crate) fn input(&self, id: u32) -> f32 {
        let (index, slot) = (id as usize / 5, id as usize % 5);
        let member = slot.checked_sub(1);
        BUILTINS
            .get(index)
            .map_or(0.0, |(_, _, b)| self.get(*b).component(member))
    }

    pub(super) fn head(&self, set: MatrixSet) -> BlockHead {
        let (model_view, projection) = match set {
            MatrixSet::Camera => (self.view_rotation, self.projection),
            MatrixSet::Shadow => (self.shadow_model_view, self.shadow_projection),
            MatrixSet::Hand => (Mat4::IDENTITY, self.hand_projection),
            MatrixSet::Screen => (
                Mat4::IDENTITY,
                Mat4::orthographic_rh_gl(0.0, 1.0, 0.0, 1.0, -1.0, 1.0),
            ),
        };
        BlockHead {
            model_view,
            projection,
            model_view_projection: projection * model_view,
            camera: self.camera.extend(0.0),
            screen: Vec4::new(
                self.size.x,
                self.size.y,
                1.0 / self.size.x,
                1.0 / self.size.y,
            ),
            time: Vec4::new(
                self.time_counter,
                self.frame_time,
                self.frame_counter as f32,
                0.0,
            ),
        }
    }
}

fn shadow_model_view(shadow_angle: f32, constants: &Constants, camera: Vec3) -> Mat4 {
    let sky_angle = if shadow_angle < 0.25 {
        shadow_angle + 0.75
    } else {
        shadow_angle - 0.25
    };
    let mut m = Mat4::from_rotation_x(90.0f32.to_radians())
        * Mat4::from_rotation_z((sky_angle * -360.0).to_radians())
        * Mat4::from_rotation_x(constants.sun_path_rotation.to_radians());
    let interval = constants.shadow_interval;
    if interval != 0.0 {
        let offset = Vec3::new(
            camera.x % interval,
            camera.y % interval,
            camera.z % interval,
        ) - interval / 2.0;
        m *= Mat4::from_translation(offset);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_splits_into_its_floor_and_remainder() {
        let (int, fract) = split_position(bevy::math::DVec3::new(-1.25, 64.5, 3.0));
        assert_eq!(int, IVec3::new(-2, 64, 3));
        assert_eq!(fract, Vec3::new(0.75, 0.5, 0.0));
        assert_eq!(Value::IVec3(int).component(Some(0)), -2.0);
    }

    #[test]
    fn the_projection_is_opengl_s() {
        let projection = Mat4::perspective_rh_gl(1.2, 1.5, NEAR, 400.0);
        let near = projection * Vec4::new(0.0, 0.0, -NEAR, 1.0);
        let far = projection * Vec4::new(0.0, 0.0, -400.0, 1.0);
        assert!((near.z / near.w + 1.0).abs() < 1e-4);
        assert!((far.z / far.w - 1.0).abs() < 1e-4);
    }

    #[test]
    fn the_hand_sits_in_front_of_the_world() {
        let projection = hand_projection(16.0 / 9.0, 512.0);
        let window = |distance: f32| {
            let clip = projection * Vec4::new(0.0, 0.0, -distance, 1.0);
            (clip.z / clip.w + 1.0) * 0.5
        };
        assert!(
            window(0.6) > 0.4375 && window(0.6) < 0.56,
            "{}",
            window(0.6)
        );
        assert!(window(500.0) <= 0.5625 + 1e-5);
        assert!(window(NEAR) >= 0.4375 - 1e-5);
    }

    #[test]
    fn camera_position_shifts_as_iris_does() {
        let mut history = History::default();
        let (current, _) =
            shifted_camera(&mut history, bevy::math::DVec3::new(100.0, 64.0, -200.0));
        assert_eq!(current, Vec3::new(100.0, 64.0, -200.0));
        let (current, previous) =
            shifted_camera(&mut history, bevy::math::DVec3::new(100.5, 64.0, -200.0));
        assert_eq!(current - previous, Vec3::new(0.5, 0.0, 0.0));
        let (current, _) =
            shifted_camera(&mut history, bevy::math::DVec3::new(65000.0, 64.0, 10.0));
        assert!((current.x - 5000.0).abs() < 1e-3, "{current}");
        let (current, previous) =
            shifted_camera(&mut history, bevy::math::DVec3::new(65001.0, 64.0, 10.0));
        assert!((current.x - 5001.0).abs() < 1e-3 && (current.x - previous.x - 1.0).abs() < 1e-3);
    }

    #[test]
    fn the_shadow_view_looks_down_the_light_at_noon() {
        let constants = Constants {
            shadow_interval: 0.0,
            ..Constants::default()
        };
        let m = shadow_model_view(0.25, &constants, Vec3::new(10.5, 70.0, -3.25));
        let toward_light = m.inverse().transform_vector3(Vec3::Z);
        assert!(toward_light.abs_diff_eq(Vec3::Y, 1e-5), "{toward_light:?}");
        assert!(m.w_axis.truncate().abs_diff_eq(Vec3::ZERO, 1e-5));
    }

    #[test]
    fn celestial_angles_follow_iris() {
        assert!((celestial_fraction(0.0) - 0.25).abs() < 1e-6);
        assert!((celestial_fraction(std::f32::consts::FRAC_PI_2) - 0.5).abs() < 1e-6);
    }
}
