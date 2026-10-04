use bevy::math::{IVec2, Vec3};
use bevy::pbr::DistanceFog;
use bevy::prelude::*;

use crate::renderer::environment::Environment;
use crate::renderer::frame_view::FrameView;
use crate::renderer::systems::{BlockAtlasHandle, Shared, WorldCamera};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum EyeIn {
    #[default]
    Air = 0,
    Water = 1,
    Lava = 2,
    PowderSnow = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Vitals {
    pub(crate) health: f32,
    pub(crate) max_health: f32,
    pub(crate) hunger: f32,
    pub(crate) air: f32,
    pub(crate) max_air: f32,
    pub(crate) armor: f32,
}

pub(crate) const MAX_HUNGER: f32 = 20.0;
pub(crate) const MAX_ARMOR: f32 = 50.0;

impl Vitals {
    pub(crate) fn of(
        survival: bool,
        health: f32,
        max_health: f32,
        food: u32,
        air: i32,
        max_air: i32,
        armor: u8,
    ) -> Vitals {
        if !survival {
            return Vitals {
                health: -1.0,
                max_health: -1.0,
                hunger: -1.0,
                air: -1.0,
                max_air: -1.0,
                armor: -1.0,
            };
        }
        Vitals {
            health: health / max_health,
            max_health,
            hunger: food as f32 / MAX_HUNGER,
            air: air as f32 / max_air as f32,
            max_air: max_air as f32,
            armor: f32::from(armor) / MAX_ARMOR,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Vehicle {
    pub(crate) id: i32,
    pub(crate) in_water: bool,
    pub(crate) look: Vec3,
    pub(crate) relative: Vec3,
}

fn view_vector(x_rot: f32, y_rot: f32) -> Vec3 {
    let (x, y) = (x_rot.to_radians(), (-y_rot).to_radians());
    Vec3::new(y.sin() * x.cos(), -x.sin(), y.cos() * x.cos())
}

fn in_shallow_water(feet: Vec3, eye_height: f32) -> bool {
    use crate::util::block_model::{EyeFluid, eye_fluid_at};
    let water = |at: Vec3| {
        eye_fluid_at(f64::from(at.x), f64::from(at.y), f64::from(at.z)) == Some(EyeFluid::Water)
    };
    water(feet) && !water(feet + Vec3::Y * eye_height)
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub(crate) struct Inputs {
    pub(crate) day_ticks: f64,
    pub(crate) sun_angle: f32,
    pub(crate) moon_angle: f32,
    pub(crate) moon_phase: i32,
    pub(crate) sky_color: Vec3,
    pub(crate) fog_color: Vec3,
    pub(crate) fog_range: bevy::math::Vec2,
    pub(crate) screen_brightness: f32,
    pub(crate) body_vector: Vec3,
    pub(crate) rainfall: f32,
    pub(crate) temperature: f32,
    pub(crate) precipitation: i32,
    pub(crate) rain: f32,
    pub(crate) thunder: f32,
    pub(crate) eye_in: EyeIn,
    pub(crate) eye_light: IVec2,
    pub(crate) blindness: f32,
    pub(crate) night_vision: f32,
    pub(crate) darkness: f32,
    pub(crate) biome: Option<u8>,
    pub(crate) atlas_size: IVec2,
    pub(crate) relative_eye: Vec3,
    pub(crate) hide_gui: bool,
    pub(crate) held_items: [i32; 2],
    pub(crate) held_light: [i32; 2],
    pub(crate) lightning: bevy::math::Vec4,
    pub(crate) bedrock_level: i32,
    pub(crate) height_limit: i32,
    pub(crate) logical_height: i32,
    pub(crate) sea_level: i32,
    pub(crate) has_ceiling: bool,
    pub(crate) has_skylight: bool,
    pub(crate) vitals: Vitals,
    pub(crate) spectator: bool,
    pub(crate) first_person: bool,
    pub(crate) riding: bool,
    pub(crate) elytra_flying: bool,
    pub(crate) swimming: bool,
    pub(crate) feet_in_water: bool,
    pub(crate) eye_position: Vec3,
    pub(crate) vehicle: Vehicle,
}

fn fading(duration: i32) -> f32 {
    if duration < 0 {
        1.0
    } else {
        (duration as f32 / 20.0).min(1.0)
    }
}

fn night_vision_scale(duration: i32, partial: f32) -> f32 {
    if duration < 0 || duration > 200 {
        1.0
    } else {
        0.7 + ((duration as f32 - partial) * std::f32::consts::PI * 0.2).sin() * 0.3
    }
}

fn held_block_light(item: &str) -> i32 {
    use std::str::FromStr;
    azalea_registry::builtin::BlockKind::from_str(item).map_or(0, |kind| {
        i32::from(crate::lighting::props::of(azalea::block::BlockState::from(kind)).emission)
    })
}

struct Player {
    riding: bool,
    vehicle_id: i32,
    vehicle_at: Option<(Vec3, Vec3, f32)>,
    vitals: Vitals,
    spectator: bool,
    elytra_flying: bool,
    swimming: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn gather_inputs(
    shared: Res<Shared>,
    view: Res<FrameView>,
    environment: Res<Environment>,
    camera: Query<(&Transform, Option<&DistanceFog>), With<WorldCamera>>,
    atlas: Option<Res<BlockAtlasHandle>>,
    images: Res<Assets<Image>>,
    gui: Res<crate::gui::GuiState>,
    compiled: Option<Res<super::install::CompiledPack>>,
    third_person: Res<crate::renderer::input::ThirdPersonState>,
    freecam: Res<crate::renderer::input::FreecamState>,
    mut inputs: ResMut<Inputs>,
) {
    let item_ids = compiled.as_deref().map(|c| &c.installed.item_ids);
    let entity_ids = compiled.as_deref().map(|c| &c.installed.entity_ids);
    let old_hand_light = compiled
        .as_deref()
        .is_none_or(|c| c.installed.old_hand_light);
    let partial = view.partial;
    let day_ticks = view.day_ticks;
    let pos = Vec3::from(view.player_lerp);
    let eye = pos + Vec3::Y * view.eye_height;
    let body_vector = view_vector(-view.camera_pitch, -view.camera_yaw - 180.0);
    let (effects, held_items, held_light, vitals, elytra_flying, swimming) = {
        let Ok(state) = shared.0.lock();
        let session = &state.session;
        let effect = |name: &str| {
            session
                .active_effects
                .iter()
                .find(|e| e.id == name)
                .map(|e| e.duration)
        };
        let held = |slot: usize| {
            let item = session.hotbar.get(slot).map_or("", |s| s.item);
            item_ids
                .and_then(|ids| ids.get(item))
                .copied()
                .unwrap_or(-1)
        };
        let light = |slot: usize| {
            session
                .hotbar
                .get(slot)
                .map_or(0, |s| held_block_light(s.item))
        };
        let (main, off) = (light(usize::from(session.hotbar_selected)), light(9));
        (
            (
                effect("blindness"),
                effect("night_vision"),
                effect("darkness"),
            ),
            [held(usize::from(session.hotbar_selected)), held(9)],
            [if old_hand_light { main.max(off) } else { main }, off],
            Vitals::of(
                session.gamemode.is_survival(),
                session.health,
                if session.health_display.max_health > 0.0 {
                    session.health_display.max_health
                } else {
                    crate::session::VANILLA_MAX_HEALTH
                },
                session.food,
                session.air_supply,
                crate::session::MAX_AIR_SUPPLY,
                session.health_display.armor,
            ),
            session.fall_flying,
            session.swimming,
        )
    };
    let vehicle_entity = crate::client::tracking::local_vehicle_id();
    let vehicle = vehicle_entity.and_then(|id| view.entities.iter().find(|e| e.id == id));
    let player = Player {
        riding: vehicle_entity.is_some(),
        vehicle_id: vehicle.zip(entity_ids).map_or(0, |(e, ids)| {
            let name = e.kind.to_str();
            let name = name.strip_prefix("minecraft:").unwrap_or(name);
            if ids.is_empty() {
                0
            } else {
                ids.get(name).copied().unwrap_or(-1)
            }
        }),
        vehicle_at: vehicle.map(|e| {
            let (y_rot, x_rot) = e.look(1.0);
            (
                Vec3::from(e.position(partial)),
                view_vector(x_rot, y_rot),
                azalea::entity::dimensions::EntityDimensions::from(e.kind).eye_height,
            )
        }),
        vitals,
        spectator: view.gamemode == crate::session::Gamemode::Spectator,
        elytra_flying,
        swimming,
    };
    let bolt = view
        .entities
        .iter()
        .find(|e| e.kind == azalea_registry::builtin::EntityKind::LightningBolt)
        .map(|e| Vec3::from(e.position(partial)));

    let (camera_at, fog) = match camera.single() {
        Ok((transform, fog)) => (transform.translation, fog),
        Err(_) => (eye, None),
    };
    let fog_color = fog.map_or(Vec3::ZERO, |f| {
        let c = f.color.to_srgba();
        Vec3::new(c.red, c.green, c.blue)
    });
    let fog_range = fog.map_or(bevy::math::Vec2::ZERO, |f| {
        let c = f.directional_light_color.to_linear();
        bevy::math::Vec2::new(c.red, c.green)
    });
    let [r, g, b, _] = environment.sky.sky_color.to_rgba_f32();
    let (block, sky) = crate::renderer::lightmap::levels_at(eye.to_array());
    let eye_in =
        match crate::util::block_model::eye_fluid_at(eye.x as f64, eye.y as f64, eye.z as f64) {
            Some(crate::util::block_model::EyeFluid::Water) => EyeIn::Water,
            Some(crate::util::block_model::EyeFluid::Lava) => EyeIn::Lava,
            None if crate::util::block_model::powder_snow_at(
                camera_at.x as f64,
                camera_at.y as f64,
                camera_at.z as f64,
            ) =>
            {
                EyeIn::PowderSnow
            }
            None => EyeIn::Air,
        };
    let atlas_size = atlas
        .and_then(|handle| images.get(&handle.0))
        .map_or(IVec2::ZERO, |image| image.size().as_ivec2());

    let row = crate::client::envprobe::camera_biome();
    let climate = row.map(|row| {
        (
            crate::util::biome_color::env(row),
            crate::util::biome_color::downfall(row),
        )
    });
    let dimension = crate::renderer::dimension::current();
    let sea_level = crate::renderer::dimension::sea_level();
    let precipitation = climate.map_or(0, |(env, _)| {
        use crate::renderer::environment::{Precipitation, precipitation_at};
        match precipitation_at(env, pos.y.floor() as i32, sea_level) {
            Precipitation::None => 0,
            Precipitation::Rain => 1,
            Precipitation::Snow => 2,
        }
    });

    let (min_y, max_y) = dimension.y_range();
    let vehicle = player
        .vehicle_at
        .map_or_else(Vehicle::default, |(at, look, eye_height)| Vehicle {
            id: player.vehicle_id,
            in_water: in_shallow_water(at, eye_height),
            look,
            relative: camera_at - at,
        });

    *inputs = Inputs {
        day_ticks,
        sun_angle: environment.sky.sun_angle,
        moon_angle: environment.sky.moon_angle,
        moon_phase: environment.sky.moon_phase as i32,
        sky_color: Vec3::new(r, g, b),
        fog_color,
        fog_range,
        screen_brightness: gui.options.gamma as f32 / 100.0,
        body_vector,
        rainfall: climate.map_or(0.0, |(_, downfall)| downfall),
        temperature: climate.map_or(0.0, |(env, _)| env.temperature),
        precipitation,
        rain: environment.rain,
        thunder: environment.thunder,
        eye_in,
        eye_light: IVec2::new(i32::from(block), i32::from(sky)),
        blindness: effects.0.map_or(0.0, fading),
        night_vision: effects.1.map_or(0.0, |d| night_vision_scale(d, partial)),
        darkness: effects.2.map_or(0.0, fading),
        biome: row,
        atlas_size,
        relative_eye: camera_at - eye,
        hide_gui: gui.hide_gui,
        held_items,
        held_light,
        lightning: bolt.map_or(bevy::math::Vec4::ZERO, |at| (at - camera_at).extend(1.0)),
        bedrock_level: min_y,
        height_limit: max_y - min_y,
        logical_height: dimension.logical_height(),
        sea_level,
        has_ceiling: dimension.has_ceiling(),
        has_skylight: dimension.has_skylight(),
        vitals: player.vitals,
        spectator: player.spectator,
        first_person: !third_person.active && !freecam.active,
        riding: player.riding,
        elytra_flying: player.elytra_flying,
        swimming: player.swimming,
        feet_in_water: in_shallow_water(pos, eye.y - pos.y),
        eye_position: eye,
        vehicle,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vitals_read_minus_one_outside_survival() {
        let creative = Vitals::of(false, 20.0, 20.0, 20, 300, 300, 10);
        assert_eq!(
            creative,
            Vitals {
                health: -1.0,
                max_health: -1.0,
                hunger: -1.0,
                air: -1.0,
                max_air: -1.0,
                armor: -1.0
            }
        );
        assert!(crate::session::Gamemode::Adventure.is_survival());
        assert!(!crate::session::Gamemode::Spectator.is_survival());
        let survival = Vitals::of(true, 10.0, 20.0, 10, 150, 300, 25);
        assert_eq!(
            survival,
            Vitals {
                health: 0.5,
                max_health: 20.0,
                hunger: 0.5,
                air: 0.5,
                max_air: 300.0,
                armor: 0.5
            }
        );
    }

    #[test]
    fn view_vectors_follow_vanilla() {
        assert!(view_vector(0.0, 0.0).abs_diff_eq(Vec3::Z, 1e-6));
        assert!(view_vector(0.0, 90.0).abs_diff_eq(-Vec3::X, 1e-6));
        assert!(view_vector(-90.0, 0.0).abs_diff_eq(Vec3::Y, 1e-6));
    }

    #[test]
    fn effects_fade_in_their_last_second() {
        assert_eq!(fading(-1), 1.0);
        assert_eq!(fading(400), 1.0);
        assert_eq!(fading(10), 0.5);
        assert_eq!(night_vision_scale(400, 0.0), 1.0);
        assert!(night_vision_scale(100, 0.0) <= 1.0);
    }
}
