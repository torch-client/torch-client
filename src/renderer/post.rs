#[cfg(feature = "builtin_shaders")]
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
#[cfg(feature = "builtin_shaders")]
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection, Hdr};

#[cfg(feature = "builtin_shaders")]
use super::hand::HandCamera;
#[cfg(feature = "builtin_shaders")]
use super::systems::WorldCamera;

#[cfg(feature = "builtin_shaders")]
const EXPOSURE_EV: f32 = -0.25;

#[cfg(feature = "builtin_shaders")]
const POST_SATURATION: f32 = 1.08;

#[cfg(feature = "builtin_shaders")]
const SHADOW_GAMMA: f32 = 0.92;
#[cfg(feature = "builtin_shaders")]
const HIGHLIGHT_GAIN: f32 = 1.05;

#[cfg(feature = "builtin_shaders")]
fn grading() -> ColorGrading {
    ColorGrading {
        global: ColorGradingGlobal {
            exposure: EXPOSURE_EV,
            post_saturation: POST_SATURATION,
            ..default()
        },
        shadows: ColorGradingSection {
            gamma: SHADOW_GAMMA,
            ..default()
        },
        highlights: ColorGradingSection {
            gain: HIGHLIGHT_GAIN,
            ..default()
        },
        ..default()
    }
}

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
fn bloom() -> bevy::post_process::bloom::Bloom {
    use bevy::post_process::bloom::{Bloom, BloomPrefilter};
    Bloom {
        intensity: 0.16,
        low_frequency_boost: 0.5,
        low_frequency_boost_curvature: 0.9,
        prefilter: BloomPrefilter {
            threshold: 1.0,
            threshold_softness: 0.6,
        },
        ..Bloom::NATURAL
    }
}

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
const FOG_VOLUME_SIZE: f32 = 320.0;

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
#[derive(Component)]
struct SunShaftVolume;

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
fn shafts_enabled() -> bool {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| matches!(std::env::var("MC_SHAFTS").as_deref(), Ok("1") | Ok("true")))
}

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
fn follow_camera(
    camera: Query<&GlobalTransform, With<WorldCamera>>,
    mut volume: Query<&mut Transform, With<SunShaftVolume>>,
) {
    let Ok(camera) = camera.single() else { return };
    let Ok(mut transform) = volume.single_mut() else {
        return;
    };
    transform.translation = camera.translation();
}

#[cfg(feature = "builtin_shaders")]
#[derive(Component)]
struct Graded;

#[cfg(feature = "builtin_shaders")]
#[allow(clippy::type_complexity)]
fn apply_pipeline(
    mut commands: Commands,
    cameras: Query<(Entity, Has<Hdr>), With<Camera>>,
    hand: Query<(Entity, Has<Graded>), With<HandCamera>>,
    mut world_camera: Query<&mut Tonemapping, With<WorldCamera>>,
) {
    let fancy = super::terrain::builtin_shaders_enabled();
    for (entity, hdr) in &cameras {
        if fancy && !hdr {
            commands.entity(entity).insert(Hdr);
        } else if !fancy && hdr {
            commands.entity(entity).remove::<Hdr>();
        }
    }
    for (entity, graded) in &hand {
        if fancy && !graded {
            commands.entity(entity).insert((grading(), Graded));
        } else if !fancy && graded {
            commands
                .entity(entity)
                .insert(ColorGrading::default())
                .remove::<Graded>();
        }
    }
    let wanted = if fancy {
        Tonemapping::None
    } else {
        Tonemapping::default()
    };
    for mut tonemapping in &mut world_camera {
        if *tonemapping != wanted {
            *tonemapping = wanted;
        }
    }
}

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
fn apply_bloom(
    mut commands: Commands,
    gui: Res<crate::gui::GuiState>,
    cameras: Query<(Entity, Has<bevy::post_process::bloom::Bloom>), With<HandCamera>>,
) {
    let wanted = super::terrain::builtin_shaders_enabled()
        && gui.options.shader_quality == super::terrain::ShaderQuality::Fancy;
    for (entity, present) in &cameras {
        if wanted && !present {
            commands.entity(entity).insert(bloom());
        } else if !wanted && present {
            commands
                .entity(entity)
                .remove::<bevy::post_process::bloom::Bloom>();
        }
    }
}

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
fn setup_shafts(
    mut commands: Commands,
    camera: Query<Entity, (With<WorldCamera>, Without<bevy::light::VolumetricFog>)>,
    existing: Query<(), With<SunShaftVolume>>,
) {
    use bevy::light::{FogVolume, VolumetricFog};

    for entity in &camera {
        commands.entity(entity).insert(VolumetricFog {
            ambient_intensity: 0.0,
            step_count: 32,
            ..default()
        });
    }

    if existing.iter().next().is_none() {
        commands.spawn((
            FogVolume {
                density_factor: 0.012,
                scattering_asymmetry: 0.7,
                scattering: 0.5,
                absorption: 0.05,
                ..default()
            },
            SunShaftVolume,
            Transform::from_scale(Vec3::splat(FOG_VOLUME_SIZE)),
        ));
    }
}

#[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
fn remove_shafts(
    mut commands: Commands,
    camera: Query<Entity, (With<WorldCamera>, With<bevy::light::VolumetricFog>)>,
    volume: Query<Entity, With<SunShaftVolume>>,
) {
    for entity in &camera {
        commands
            .entity(entity)
            .remove::<bevy::light::VolumetricFog>();
    }
    for entity in &volume {
        commands.entity(entity).despawn();
    }
}

pub struct PostPlugin;

impl Plugin for PostPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "builtin_shaders")]
        app.add_systems(Update, apply_pipeline);
        #[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
        app.add_systems(Update, apply_bloom);
        #[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
        app.add_systems(
            Update,
            (setup_shafts, follow_camera)
                .run_if(shafts_enabled)
                .run_if(super::terrain::builtin_shaders_enabled),
        );
        #[cfg(all(feature = "builtin_shaders", not(target_arch = "wasm32")))]
        app.add_systems(
            Update,
            remove_shafts
                .run_if(shafts_enabled)
                .run_if(not(super::terrain::builtin_shaders_enabled)),
        );
        #[cfg(not(feature = "builtin_shaders"))]
        let _ = app;
    }
}
