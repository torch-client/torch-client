use bevy::camera::RenderTarget;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_resource::TextureFormat;
use bevy::window::PrimaryWindow;
use bevy::window::WindowRef;

use super::hand::HandCamera;
use super::panorama::PanoramaCamera;
use super::systems::WorldCamera;

const SSAA_SCALE: f32 = 2.0;

const MIN_RENDER_SCALE: f32 = 0.5;

fn render_scale(antialiasing: bool, scale_factor: f32) -> f32 {
    let per_logical_pixel = if antialiasing { SSAA_SCALE } else { 1.0 };
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let floor = if cfg!(feature = "mobile_ui") {
        MIN_RENDER_SCALE
    } else {
        1.0
    };
    (per_logical_pixel / scale_factor).clamp(floor.min(per_logical_pixel), per_logical_pixel)
}

#[derive(Resource)]
struct SsaaTarget {
    window_size: UVec2,
    scale: f32,
    camera_count: usize,
}

impl Default for SsaaTarget {
    fn default() -> Self {
        Self {
            window_size: UVec2::ZERO,
            scale: f32::NAN,
            camera_count: 0,
        }
    }
}

#[derive(Resource, Clone, Default, ExtractResource)]
pub struct SceneImage(pub Option<Handle<Image>>);

pub struct SsaaPlugin;

impl Plugin for SsaaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SsaaTarget>()
            .init_resource::<SceneImage>()
            .add_plugins(ExtractResourcePlugin::<SceneImage>::default())
            .add_systems(Update, apply_ssaa);
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_ssaa(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut scene_image: ResMut<SceneImage>,
    mut terrain_params: ResMut<crate::renderer::terrain_pool::TerrainParams>,
    mut target: ResMut<SsaaTarget>,
    windows: Query<&Window, With<PrimaryWindow>>,
    state: Res<crate::gui::GuiState>,
    scene_cameras: Query<Entity, Or<(With<PanoramaCamera>, With<WorldCamera>, With<HandCamera>)>>,
) {
    let Ok(window) = windows.single() else { return };
    let window_size = UVec2::new(
        window.physical_width().max(1),
        window.physical_height().max(1),
    );
    #[cfg(feature = "shader_support")]
    let antialiasing = state.options.antialiasing && !crate::renderer::packvertex::active();
    #[cfg(not(feature = "shader_support"))]
    let antialiasing = state.options.antialiasing;
    let scale = render_scale(antialiasing, window.scale_factor());
    let enabled = scale != 1.0;
    let camera_count = scene_cameras.iter().count();
    if window_size == target.window_size
        && scale == target.scale
        && camera_count == target.camera_count
    {
        return;
    }
    target.window_size = window_size;
    target.scale = scale;
    target.camera_count = camera_count;
    let render_size = UVec2::new(
        ((window_size.x as f32) * scale).round().max(1.0) as u32,
        ((window_size.y as f32) * scale).round().max(1.0) as u32,
    );

    if !enabled {
        if terrain_params.mip_bias != 0.0 {
            terrain_params.mip_bias = 0.0;
        }
        for entity in &scene_cameras {
            commands
                .entity(entity)
                .insert(RenderTarget::Window(WindowRef::Primary));
        }
        if scene_image.0.is_some() {
            scene_image.0 = None;
        }
        return;
    }

    let lod_bias = scale.log2();
    if terrain_params.mip_bias != lod_bias {
        terrain_params.mip_bias = lod_bias;
    }

    let mut image = Image::new_target_texture(
        render_size.x,
        render_size.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.sampler = if scale != 1.0 {
        ImageSampler::linear()
    } else {
        ImageSampler::nearest()
    };
    let image_handle = images.add(image);

    for entity in &scene_cameras {
        commands
            .entity(entity)
            .insert(RenderTarget::Image(image_handle.clone().into()));
    }
    scene_image.0 = Some(image_handle);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_floor() -> f32 {
        if cfg!(feature = "mobile_ui") {
            MIN_RENDER_SCALE
        } else {
            1.0
        }
    }

    #[test]
    fn scene_is_sized_per_logical_pixel() {
        let cases = [
            (1.0, false, 1.0),
            (1.0, true, 2.0),
            (2.0, false, test_floor()),
            (2.0, true, 1.0),
            (2.625, false, test_floor()),
            (2.625, true, (2.0f32 / 2.625).max(test_floor())),
            (3.5, false, test_floor()),
            (3.5, true, (2.0f32 / 3.5).max(test_floor())),
        ];
        for (factor, antialiasing, want) in cases {
            let got = render_scale(antialiasing, factor);
            assert!(
                (got - want).abs() < 1e-6,
                "scale factor {factor}, antialiasing {antialiasing}: got {got}, want {want}",
            );
        }
    }

    #[test]
    fn a_denser_display_never_costs_more() {
        for antialiasing in [false, true] {
            let asked_for = if antialiasing { SSAA_SCALE } else { 1.0 };
            let mut previous = f32::INFINITY;
            for step in 1..=80 {
                let factor = step as f32 * 0.1;
                let scale = render_scale(antialiasing, factor);

                assert!(
                    scale >= test_floor() - 1e-6,
                    "factor {factor}: {scale} is below the floor",
                );
                assert!(
                    scale <= asked_for + 1e-6,
                    "factor {factor}: {scale} is more than the option asked for",
                );
                assert!(
                    scale <= previous + 1e-6,
                    "factor {factor}: {scale} rose above {previous}; a denser display got dearer",
                );

                let per_logical = scale * factor;
                assert!(
                    per_logical <= asked_for + 1e-6 || (scale - test_floor()).abs() < 1e-6,
                    "factor {factor}: {per_logical} per logical pixel exceeds {asked_for} \
                     without the floor being the reason",
                );
                previous = scale;
            }
        }
    }

    #[test]
    fn an_unset_scale_factor_falls_back() {
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(render_scale(false, bad), 1.0, "scale factor {bad}");
            assert_eq!(render_scale(true, bad), SSAA_SCALE, "scale factor {bad}");
        }
    }
}
