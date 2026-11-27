use bevy::color::{LinearRgba, Srgba};
use bevy::math::{Mat4, Vec2};
use bevy::render::view::ExtractedView;

use super::super::inputs::Inputs;
use super::super::uniforms::{FrameValues, celestial_position};
use crate::renderer::RenderOrigin;
use crate::renderer::hand::hand_clip_from_view;
use crate::renderer::terrain::LightmapPixels;
use crate::shaderpack::native::api::{
    Frame, FrameCamera, FramePlayer, FrameShadow, FrameSky, FrameTime, FrameView, FrameWeather,
};

const DAY_TICKS: f64 = 24000.0;

const MAX_LIGHT: f32 = 15.0;

#[derive(Default)]
pub(super) struct FrameHistory {
    previous_projection: Option<Mat4>,
}

pub(super) fn toward(angle: f32) -> [f32; 3] {
    celestial_position(Mat4::IDENTITY, 0.0, angle)
        .normalize_or_zero()
        .to_array()
}

pub(super) fn light_floor(pixels: Option<&LightmapPixels>) -> [f32; 3] {
    let Some(&[r, g, b, _]) = pixels.and_then(|p| p.data.first_chunk::<4>()) else {
        return [0.0; 3];
    };
    let linear = LinearRgba::from(Srgba::rgb_u8(r, g, b));
    [linear.red, linear.green, linear.blue]
}

#[allow(clippy::too_many_arguments)]
pub(super) fn frame(
    values: &FrameValues,
    view: &ExtractedView,
    origin: RenderOrigin,
    inputs: &Inputs,
    size: Vec2,
    light_floor: [f32; 3],
    shadow: FrameShadow,
    history: &mut FrameHistory,
) -> Frame {
    let projection = view.clip_from_view;
    let previous_projection = history
        .previous_projection
        .replace(projection)
        .unwrap_or(projection);
    let world = origin.to_world(values.camera());
    let block = world.floor();
    let position = world.as_vec3();
    let day_ticks = inputs.day_ticks.rem_euclid(DAY_TICKS);
    let size = size.max(Vec2::ONE);
    Frame {
        camera: FrameCamera {
            view: values.view_rotation().to_cols_array_2d(),
            view_inverse: values.view_rotation_inverse().to_cols_array_2d(),
            projection: projection.to_cols_array_2d(),
            projection_inverse: projection.inverse().to_cols_array_2d(),
            previous_view: values.previous_view_rotation().to_cols_array_2d(),
            previous_projection: previous_projection.to_cols_array_2d(),
            hand_projection: hand_clip_from_view(size.x / size.y).to_cols_array_2d(),
            position: position.to_array(),
            near: projection.w_axis.z,
            far: values.far(),
            block: block.as_ivec3().to_array(),
            fraction: (world - block).as_vec3().to_array(),
            ..FrameCamera::default()
        },
        time: FrameTime {
            frame: values.frame_counter(),
            seconds: values.time_counter(),
            delta: values.frame_time(),
            day_ticks: day_ticks as f32,
            day_fraction: (day_ticks / DAY_TICKS) as f32,
            ..FrameTime::default()
        },
        sky: FrameSky {
            sun: toward(inputs.sun_angle),
            moon_phase: inputs.moon_phase.rem_euclid(8) as u32,
            moon: toward(inputs.moon_angle),
            fog_start: inputs.fog_range.x,
            sky_color: inputs.sky_color.to_array(),
            fog_end: inputs.fog_range.y,
            fog_color: inputs.fog_color.to_array(),
            light_floor,
            ..FrameSky::default()
        },
        weather: FrameWeather {
            rain: inputs.rain,
            thunder: inputs.thunder,
            wetness: values.wetness(),
            temperature: inputs.temperature,
            rainfall: inputs.rainfall,
            ..FrameWeather::default()
        },
        player: FramePlayer {
            eye_medium: inputs.eye_in as i32 as u32,
            blindness: inputs.blindness,
            eye_light: (inputs.eye_light.as_vec2() / MAX_LIGHT).to_array(),
            night_vision: inputs.night_vision,
            darkness: inputs.darkness,
            held_light: [
                inputs.held_light[0] as f32 / MAX_LIGHT,
                inputs.held_light[1] as f32 / MAX_LIGHT,
            ],
            ..FramePlayer::default()
        },
        shadow,
        view: FrameView {
            size: size.to_array(),
            texel: (Vec2::ONE / size).to_array(),
            ..FrameView::default()
        },
        ..Frame::default()
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::Vec3;

    use super::*;

    #[test]
    fn the_sun_is_overhead_at_noon() {
        let up = Vec3::from_array(toward(0.0));
        assert!((up - Vec3::Y).length() < 1e-5, "{up}");
        let set = Vec3::from_array(toward(std::f32::consts::FRAC_PI_2));
        assert!(set.y.abs() < 1e-5 && set.z.abs() < 1e-5, "{set}");
    }
}
