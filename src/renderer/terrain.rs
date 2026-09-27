use bevy::asset::embedded_asset;
use bevy::image::ImageSampler;
#[cfg(feature = "builtin_shaders")]
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
#[cfg(feature = "builtin_shaders")]
use bevy::render::render_resource::DepthBiasState;
use bevy::shader::ShaderRef;

pub fn embedded(file: &str) -> ShaderRef {
    let crate_name = module_path!().split(':').next().unwrap_or("torch_client");
    ShaderRef::Path(format!("embedded://{crate_name}/renderer/{file}").into())
}

#[cfg(feature = "builtin_shaders")]
pub fn shadow_depth_bias() -> DepthBiasState {
    DepthBiasState {
        constant: SHADOW_DEPTH_BIAS_CONSTANT,
        slope_scale: SHADOW_DEPTH_BIAS_SLOPE,
        clamp: 0.0,
    }
}

#[cfg(feature = "builtin_shaders")]
const SHADOW_DEPTH_BIAS_CONSTANT: i32 = -2;
#[cfg(feature = "builtin_shaders")]
const SHADOW_DEPTH_BIAS_SLOPE: f32 = -2.0;

#[cfg(feature = "builtin_shaders")]
const SHADOW_NORMAL_BIAS: f32 = 1.0;

pub const SHADOW_ALPHA_MIP: f32 = 1.0;

#[cfg(feature = "builtin_shaders")]
#[derive(Component)]
pub struct SunLight;

#[cfg(feature = "builtin_shaders")]
pub const SUN_GAIN: f32 = 1.60;
#[cfg(feature = "builtin_shaders")]
pub const AMBIENT_GAIN: f32 = 0.31;
#[cfg(feature = "builtin_shaders")]
pub const BLOCK_GAIN: f32 = 2.20;

#[cfg(feature = "builtin_shaders")]
#[derive(Resource, Default)]
struct SunClock {
    ticks: f64,
    started: bool,
}

#[cfg(feature = "builtin_shaders")]
const TICKS_PER_SECOND: f64 = 20.0;

#[cfg(feature = "builtin_shaders")]
const SUN_CLOCK_SNAP: f64 = 40.0;

#[cfg(feature = "builtin_shaders")]
const SUN_CLOCK_CATCHUP: f64 = 2.0;

#[cfg(feature = "builtin_shaders")]
#[derive(Resource, Clone, Copy, Debug)]
pub struct Daylight {
    pub to_light: Vec3,
    pub sun: Vec3,
    pub ambient: Vec3,
}

#[cfg(feature = "builtin_shaders")]
impl Default for Daylight {
    fn default() -> Self {
        Daylight {
            to_light: Vec3::Y,
            sun: Vec3::ZERO,
            ambient: Vec3::ZERO,
        }
    }
}

#[cfg(feature = "builtin_shaders")]
const SHADOW_ANCHOR_GRID: f32 = 2.0;

#[cfg(feature = "builtin_shaders")]
fn recentre_cascades(
    cameras: Query<&GlobalTransform, With<super::systems::WorldCamera>>,
    mut lights: Query<&mut bevy::light::Cascades, With<SunLight>>,
) {
    let snap = |v: f32| (v / SHADOW_ANCHOR_GRID).floor() * SHADOW_ANCHOR_GRID;
    for mut cascades in lights.iter_mut() {
        cascades.cascades.retain(|view, _| cameras.contains(*view));
        for (view, list) in cascades.cascades.iter_mut() {
            let Ok(camera) = cameras.get(*view) else {
                continue;
            };
            let t = camera.translation();
            let anchor = Vec3::new(snap(t.x), snap(t.y), snap(t.z));

            for cascade in list.iter_mut() {
                let rotation = Mat3::from_cols(
                    cascade.world_from_cascade.x_axis.truncate(),
                    cascade.world_from_cascade.y_axis.truncate(),
                    cascade.world_from_cascade.z_axis.truncate(),
                );
                let inverse = rotation.transpose();
                let old = inverse * cascade.world_from_cascade.w_axis.truncate();
                let wanted = inverse * anchor;
                let centre = Vec3::new(wanted.x, wanted.y, old.z);

                cascade.world_from_cascade.w_axis = (rotation * centre).extend(1.0);
                let cascade_from_world = Mat4::from_cols(
                    inverse.x_axis.extend(0.0),
                    inverse.y_axis.extend(0.0),
                    inverse.z_axis.extend(0.0),
                    (-centre).extend(1.0),
                );
                cascade.clip_from_world = cascade.clip_from_cascade * cascade_from_world;
            }
        }
    }
}

static BUILTIN_SHADERS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn builtin_shaders_enabled() -> bool {
    cfg!(feature = "builtin_shaders") && BUILTIN_SHADERS.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(feature = "builtin_shaders")]
fn sync_builtin_shaders(
    gui: Res<crate::gui::GuiState>,
    mut entity_materials: ResMut<Assets<super::entity_material::EntityMaterial>>,
) {
    let wanted = gui.options.shaders_enabled;
    if BUILTIN_SHADERS.swap(wanted, std::sync::atomic::Ordering::Relaxed) == wanted {
        return;
    }
    for _ in entity_materials.iter_mut() {}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ShaderQuality {
    #[default]
    Fancy,
    Fast,
}

impl ShaderQuality {
    #[cfg(feature = "builtin_shaders")]
    pub fn caption(self) -> &'static str {
        match self {
            ShaderQuality::Fancy => "Fancy",
            ShaderQuality::Fast => "Fast",
        }
    }

    pub fn serialized_name(self) -> &'static str {
        match self {
            ShaderQuality::Fancy => "fancy",
            ShaderQuality::Fast => "fast",
        }
    }

    pub fn from_serialized_name(name: &str) -> Option<ShaderQuality> {
        match name {
            "fancy" => Some(ShaderQuality::Fancy),
            "fast" => Some(ShaderQuality::Fast),
            _ => None,
        }
    }

    #[cfg(feature = "builtin_shaders")]
    pub fn next(self) -> ShaderQuality {
        match self {
            ShaderQuality::Fancy => ShaderQuality::Fast,
            ShaderQuality::Fast => ShaderQuality::Fancy,
        }
    }
}

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
const SHADOW_MAX_DISTANCE: f32 = 112.0;

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
const FIRST_CASCADE_FAR_BOUND: f32 = 6.0;

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
const FIRST_CASCADE_FAR_BOUND_FAST: f32 = 12.0;
#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
const SHADOW_MAX_DISTANCE_FAST: f32 = 64.0;

#[cfg(all(feature = "builtin_shaders", target_arch = "wasm32"))]
const WEB_SHADOW_DISTANCE: f32 = 96.0;
#[cfg(all(feature = "builtin_shaders", target_arch = "wasm32"))]
const WEB_SHADOW_DISTANCE_FAST: f32 = 64.0;

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
fn shadow_map_size(quality: ShaderQuality) -> usize {
    match quality {
        ShaderQuality::Fancy => 2048,
        ShaderQuality::Fast => 1024,
    }
}

#[cfg(all(feature = "builtin_shaders", target_arch = "wasm32"))]
fn shadow_map_size(quality: ShaderQuality) -> usize {
    match quality {
        ShaderQuality::Fancy => 1024,
        ShaderQuality::Fast => 512,
    }
}

#[cfg(feature = "builtin_shaders")]
fn shadow_filtering(quality: ShaderQuality) -> bevy::light::ShadowFilteringMethod {
    match quality {
        ShaderQuality::Fancy => bevy::light::ShadowFilteringMethod::Gaussian,
        ShaderQuality::Fast => bevy::light::ShadowFilteringMethod::Hardware2x2,
    }
}

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
fn cascades(quality: ShaderQuality) -> bevy::light::CascadeShadowConfig {
    let (num_cascades, first, max) = match quality {
        ShaderQuality::Fancy => (4, FIRST_CASCADE_FAR_BOUND, SHADOW_MAX_DISTANCE),
        ShaderQuality::Fast => (2, FIRST_CASCADE_FAR_BOUND_FAST, SHADOW_MAX_DISTANCE_FAST),
    };
    CascadeShadowConfigBuilder {
        num_cascades,
        first_cascade_far_bound: first,
        maximum_distance: max,
        overlap_proportion: 0.2,
        ..default()
    }
    .build()
}

#[cfg(all(feature = "builtin_shaders", target_arch = "wasm32"))]
fn cascades(quality: ShaderQuality) -> bevy::light::CascadeShadowConfig {
    let max = match quality {
        ShaderQuality::Fancy => WEB_SHADOW_DISTANCE,
        ShaderQuality::Fast => WEB_SHADOW_DISTANCE_FAST,
    };
    CascadeShadowConfigBuilder {
        num_cascades: 1,
        first_cascade_far_bound: max,
        maximum_distance: max,
        ..default()
    }
    .build()
}

#[cfg(feature = "builtin_shaders")]
fn apply_shader_quality(
    mut commands: Commands,
    gui: Res<crate::gui::GuiState>,
    mut applied: Local<Option<ShaderQuality>>,
    mut shadow_map: ResMut<bevy::light::DirectionalLightShadowMap>,
    mut config: Query<&mut bevy::light::CascadeShadowConfig, With<SunLight>>,
    cameras: Query<
        (Entity, Option<&bevy::light::ShadowFilteringMethod>),
        With<super::systems::WorldCamera>,
    >,
) {
    let quality = gui.options.shader_quality;
    let filtering = shadow_filtering(quality);
    for (entity, current) in &cameras {
        if current != Some(&filtering) {
            commands.entity(entity).insert(filtering);
        }
    }

    let size = shadow_map_size(quality);
    if shadow_map.size != size {
        shadow_map.size = size;
    }

    if *applied != Some(quality) {
        for mut config in config.iter_mut() {
            *config = cascades(quality);
        }
        *applied = Some(quality);
    }
}

#[cfg(feature = "builtin_shaders")]
#[derive(Clone, Copy)]
struct LightStop {
    rgb: [f32; 3],
    intensity: f32,
}

#[cfg(feature = "builtin_shaders")]
const fn stop(r: f32, g: f32, b: f32, intensity: f32) -> LightStop {
    LightStop {
        rgb: [r, g, b],
        intensity,
    }
}

#[cfg(feature = "builtin_shaders")]
impl LightStop {
    fn linear(self) -> Vec3 {
        let scaled = Vec3::from(self.rgb).normalize() * self.intensity;
        scaled * scaled
    }
}

#[cfg(feature = "builtin_shaders")]
struct DayPalette {
    sunrise: LightStop,
    morning: LightStop,
    day: LightStop,
    evening: LightStop,
    sunset: LightStop,
    night: LightStop,
}

#[cfg(feature = "builtin_shaders")]
const SUN_PALETTE: DayPalette = DayPalette {
    sunrise: stop(255.0, 124.0, 82.0, 1.60),
    morning: stop(255.0, 180.0, 88.0, 1.70),
    day: stop(255.0, 236.0, 193.0, 1.85),
    evening: stop(255.0, 180.0, 88.0, 1.70),
    sunset: stop(255.0, 124.0, 82.0, 1.60),
    night: stop(146.0, 182.0, 245.0, 0.35),
};

#[cfg(feature = "builtin_shaders")]
const AMBIENT_PALETTE: DayPalette = DayPalette {
    sunrise: stop(225.0, 174.0, 236.0, 0.45),
    morning: stop(235.0, 214.0, 192.0, 0.60),
    day: stop(192.0, 208.0, 245.0, 0.60),
    evening: stop(235.0, 214.0, 192.0, 0.60),
    sunset: stop(225.0, 174.0, 236.0, 0.45),
    night: stop(146.0, 182.0, 235.0, 0.15),
};

#[cfg(feature = "builtin_shaders")]
const PALETTE_TINT: f32 = 0.85;

#[cfg(feature = "builtin_shaders")]
const AMBIENT_DAY_LEVEL: f32 = 0.2126 * 0.47 + 0.7152 * 0.67 + 0.0722 * 1.00;

#[cfg(feature = "builtin_shaders")]
const MOON_INTENSITY: f32 = 0.22;

#[cfg(feature = "builtin_shaders")]
fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

#[cfg(feature = "builtin_shaders")]
fn blend_palette(
    palette: &DayPalette,
    time_angle: f32,
    time_brightness: f32,
    sun_visibility: f32,
) -> Vec3 {
    let brightness_sqrt = time_brightness.sqrt();
    let mefade = 1.0 - ((time_angle - 0.5).abs() * 8.0 - 1.5).clamp(0.0, 1.0);
    let dfade = 1.0 - (1.0 - time_brightness).powf(1.5);

    let morning_side = palette
        .sunrise
        .linear()
        .lerp(palette.morning.linear(), brightness_sqrt);
    let evening_side = palette
        .evening
        .linear()
        .lerp(palette.sunset.linear(), 1.0 - brightness_sqrt);
    let horizon = morning_side.lerp(evening_side, mefade);
    let sun = horizon.lerp(palette.day.linear(), dfade);
    palette.night.linear().lerp(sun, sun_visibility)
}

#[cfg(feature = "builtin_shaders")]
fn palette_hue(color: Vec3) -> Vec3 {
    let l = luminance(color);
    if l <= 0.0 {
        return Vec3::ONE;
    }
    Vec3::ONE.lerp(color / l, PALETTE_TINT)
}

#[cfg(feature = "builtin_shaders")]
fn drive_sun(
    shared: Res<crate::renderer::systems::Shared>,
    mut ambient: ResMut<bevy::light::GlobalAmbientLight>,
    mut daylight: ResMut<Daylight>,
    mut clock: ResMut<SunClock>,
    time: Res<Time>,
    mut light: Query<(&mut Transform, &mut DirectionalLight, &mut Visibility), With<SunLight>>,
) {
    let Ok((mut transform, mut sun, mut visibility)) = light.single_mut() else {
        return;
    };

    let sunlit = crate::renderer::dimension::current().has_sun();
    let wanted = if sunlit && builtin_shaders_enabled() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != wanted {
        *visibility = wanted;
    }
    let (authoritative, rate) = {
        let state = shared.0.lock().unwrap();
        let partial = crate::renderer::systems::partial_ticks(&state);
        (
            state.session.day_clock.at(partial),
            state.session.day_clock.rate as f64,
        )
    };

    let ticks = {
        let drift = authoritative - clock.ticks;
        if !clock.started || drift.abs() >= SUN_CLOCK_SNAP {
            clock.started = true;
            clock.ticks = authoritative;
        } else {
            let base = TICKS_PER_SECOND * rate;
            let correction = (drift / SUN_CLOCK_CATCHUP).clamp(-base * 0.5, base * 0.5);
            clock.ticks += time.delta_secs_f64() * (base + correction);
        }
        clock.ticks
    };
    let sky = crate::renderer::timeline::sample(ticks);

    let direction = |angle: f32| {
        (Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_x(angle))
            * Vec3::Y
    };
    let sun_direction = direction(sky.sun_angle);
    let daytime = sun_direction.y > 0.0;
    let to_light = if daytime {
        sun_direction
    } else {
        direction(sky.moon_angle)
    };

    let time_angle = (ticks / crate::renderer::timeline::DAY_PERIOD as f64).rem_euclid(1.0) as f32;
    let time_brightness = sun_direction.y.max(0.0);
    let sun_visibility = ((sun_direction.y + 0.15) * 3.0).clamp(0.0, 1.0);

    let hue = palette_hue(blend_palette(
        &SUN_PALETTE,
        time_angle,
        time_brightness,
        sun_visibility,
    ));
    sun.color = Color::linear_rgb(hue.x, hue.y, hue.z);
    sun.illuminance = MOON_INTENSITY + (1.0 - MOON_INTENSITY) * sun_visibility;

    let ambient_hue = palette_hue(blend_palette(
        &AMBIENT_PALETTE,
        time_angle,
        time_brightness,
        sun_visibility,
    ));
    ambient.color = Color::linear_rgb(ambient_hue.x, ambient_hue.y, ambient_hue.z);
    ambient.brightness = AMBIENT_DAY_LEVEL;

    *daylight = if sunlit {
        Daylight {
            to_light,
            sun: hue * SUN_GAIN,
            ambient: ambient_hue * AMBIENT_DAY_LEVEL * AMBIENT_GAIN,
        }
    } else {
        Daylight::default()
    };

    *transform = Transform::from_translation(to_light).looking_at(Vec3::ZERO, Vec3::Z);
}

pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "terrain.wgsl");
        embedded_asset!(app, "terrain_prepass.wgsl");

        #[cfg(feature = "builtin_shaders")]
        {
            app.insert_resource(bevy::light::DirectionalLightShadowMap {
                size: shadow_map_size(ShaderQuality::default()),
            });

            let sun = app
                .world_mut()
                .spawn((
                    SunLight,
                    DirectionalLight {
                        illuminance: 1.0,
                        shadows_enabled: true,
                        shadow_depth_bias: 0.03,
                        shadow_normal_bias: SHADOW_NORMAL_BIAS,
                        ..default()
                    },
                    cascades(ShaderQuality::default()),
                    Transform::default(),
                ))
                .id();
            #[cfg(not(target_arch = "wasm32"))]
            app.world_mut()
                .entity_mut(sun)
                .insert(bevy::light::VolumetricLight);
            #[cfg(target_arch = "wasm32")]
            let _ = sun;
            app.init_resource::<Daylight>();
            app.init_resource::<SunClock>();
            app.add_systems(PreUpdate, sync_builtin_shaders);
            app.add_systems(Update, (drive_sun, apply_shader_quality));
            app.add_systems(
                PostUpdate,
                recentre_cascades
                    .after(bevy::light::SimulationLightSystems::UpdateDirectionalLightCascades)
                    .before(bevy::light::SimulationLightSystems::UpdateLightFrusta),
            );
        }
        app.add_plugins(bevy::render::extract_resource::ExtractResourcePlugin::<
            LightmapPixels,
        >::default());
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.add_systems(
                bevy::render::Render,
                upload_lightmap.in_set(bevy::render::RenderSystems::PrepareResources),
            );
        }
    }
}

#[derive(Resource, Clone, bevy::render::extract_resource::ExtractResource)]
pub struct LightmapPixels {
    pub image: Handle<Image>,
    pub data: Vec<u8>,
}

const LIGHTMAP_SIZE: u32 = 16;
const LIGHTMAP_BYTES: usize = (LIGHTMAP_SIZE * LIGHTMAP_SIZE * 4) as usize;

fn upload_lightmap(
    pixels: Option<Res<LightmapPixels>>,
    images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    queue: Res<bevy::render::renderer::RenderQueue>,
) {
    use bevy::render::render_resource::{
        Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
    };
    let Some(pixels) = pixels else { return };
    if pixels.data.len() != LIGHTMAP_BYTES {
        return;
    }
    let Some(gpu) = images.get(&pixels.image) else {
        return;
    };
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: &gpu.texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &pixels.data,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(LIGHTMAP_SIZE * 4),
            rows_per_image: Some(LIGHTMAP_SIZE),
        },
        Extent3d {
            width: LIGHTMAP_SIZE,
            height: LIGHTMAP_SIZE,
            depth_or_array_layers: 1,
        },
    );
}

pub fn new_lightmap_image() -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let mut image = Image::new_fill(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD | bevy::asset::RenderAssetUsages::MAIN_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

#[cfg(all(test, feature = "builtin_shaders"))]
mod tests {
    use super::*;

    fn inputs(ticks: f64) -> (f32, f32, f32) {
        let sky = crate::renderer::timeline::sample(ticks);
        let y = (Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
            * Quat::from_rotation_x(sky.sun_angle)
            * Vec3::Y)
            .y;
        (
            (ticks / crate::renderer::timeline::DAY_PERIOD as f64).rem_euclid(1.0) as f32,
            y.max(0.0),
            ((y + 0.15) * 3.0).clamp(0.0, 1.0),
        )
    }

    fn sun_at(ticks: f64) -> Vec3 {
        let (a, b, v) = inputs(ticks);
        palette_hue(blend_palette(&SUN_PALETTE, a, b, v))
    }

    #[test]
    fn every_hour_is_written_at_one_brightness() {
        for tick in (0..24_000).step_by(250) {
            let l = luminance(sun_at(tick as f64));
            assert!((l - 1.0).abs() < 1e-4, "tick {tick}: {l}");
        }
    }

    #[test]
    fn the_palette_turns_over_the_day() {
        let noon = sun_at(6000.0);
        assert!(noon.x > noon.z && noon.x < 1.3, "{noon:?}");

        let sunset = sun_at(12_800.0);
        assert!(sunset.x > 2.0 && sunset.x > sunset.z * 3.0, "{sunset:?}");

        let midnight = sun_at(18_000.0);
        assert!(midnight.z > midnight.x * 2.0, "{midnight:?}");
    }

    #[test]
    fn the_celestial_track_stays_out_of_z() {
        for tick in (0..24_000).step_by(97) {
            let sky = crate::renderer::timeline::sample(tick as f64);
            for angle in [sky.sun_angle, sky.moon_angle] {
                let d = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
                    * Quat::from_rotation_x(angle)
                    * Vec3::Y;
                assert!(d.z.abs() < 1e-6, "tick {tick}: {d:?}");
            }
        }
    }

    #[test]
    fn noon_ambient_keeps_the_old_level() {
        let (a, b, v) = inputs(6000.0);
        let ambient = palette_hue(blend_palette(&AMBIENT_PALETTE, a, b, v)) * AMBIENT_DAY_LEVEL;
        let old = Vec3::new(0.47, 0.67, 1.00);
        assert!((luminance(ambient) - luminance(old)).abs() < 1e-4);
        assert!(
            ambient.z > ambient.y && ambient.y > ambient.x,
            "{ambient:?}"
        );
    }
}
