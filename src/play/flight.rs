use std::collections::HashMap;

use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::metadata::Sprinting;
use azalea::entity::{
    ActiveEffects, HasClientLoaded, LastSentPosition, LocalEntity, LookDirection, Physics,
    PlayerAbilities, Position, move_relative,
};
use azalea::packet::game::SendGamePacketEvent;
use azalea::physics::{PhysicsSystems, ai_step, jump_from_ground, travel::travel};
use azalea::world::{WorldName, Worlds};
use azalea_core::game_type::GameMode;
use azalea_core::position::Vec3;
use azalea_core::tick::GameTick;
use azalea_protocol::common::movements::MoveFlags;
use azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos;
use azalea_protocol::packets::game::s_player_abilities::ServerboundPlayerAbilities;

use crate::session::{MOVE_DESCEND, MOVE_JUMP};

use crate::modules::flight as module;

const DEFAULT_FLYING_SPEED: f32 = 0.05;

const SPRINT_FLYING_MULTIPLIER: f32 = 2.0;

const MIN_SCROLL_FLYING_SPEED: f32 = 0.0;
const MAX_SCROLL_FLYING_SPEED: f32 = 0.2;

const VERTICAL_IMPULSE_FACTOR: f64 = 3.0;

const VERTICAL_DRAG: f64 = 0.6;

const AZALEA_AIR_SPEED: f32 = 0.02;
const AZALEA_AIR_SPEED_SPRINTING: f32 = 0.025999999;

#[derive(Resource, Default)]
pub(crate) struct PreTravelVelocityY(HashMap<Entity, f64>);

pub struct FlightPlugin;

impl Plugin for FlightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PreTravelVelocityY>().add_systems(
            GameTick,
            (
                flight_pre_travel
                    .in_set(PhysicsSystems)
                    .after(ai_step)
                    .before(travel),
                flight_post_travel
                    .after(PhysicsSystems)
                    .after(azalea::attack::handle_attack_queued)
                    .before(azalea::movement::send_position),
                flight_anti_kick.after(azalea::movement::send_position),
            ),
        );
    }
}

fn may_fly(abilities: &PlayerAbilities, gamemode: Option<&GameMode>) -> bool {
    abilities.can_fly
        || matches!(
            gamemode,
            Some(GameMode::Creative) | Some(GameMode::Spectator)
        )
}

fn is_spectator(gamemode: Option<&GameMode>) -> bool {
    matches!(gamemode, Some(GameMode::Spectator))
}

fn flying_speed(abilities: &PlayerAbilities, sprinting: bool) -> f32 {
    let base = if abilities.flying_speed > 0.0 {
        abilities.flying_speed
    } else {
        DEFAULT_FLYING_SPEED
    };
    if sprinting {
        base * SPRINT_FLYING_MULTIPLIER
    } else {
        base
    }
}

#[allow(clippy::type_complexity)]
fn flight_pre_travel(
    mut query: Query<
        (
            Entity,
            &mut PlayerAbilities,
            &mut Physics,
            &LookDirection,
            &Position,
            &WorldName,
            &ActiveEffects,
            Option<&Sprinting>,
            Option<&GameMode>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    worlds: Res<Worlds>,
    mut pre_travel_y: ResMut<PreTravelVelocityY>,
    mut commands: Commands,
) {
    let Some(shared) = crate::SHARED.get() else {
        return;
    };

    let (toggle_requested, move_flags, fly_speed_scroll) = {
        let mut s = shared.lock().unwrap();
        (
            std::mem::take(&mut s.session.fly_toggle),
            s.move_flags,
            std::mem::take(&mut s.session.fly_speed_scroll),
        )
    };

    let demand = module::poll();

    let mut flying_now = false;

    for (
        entity,
        mut abilities,
        mut physics,
        direction,
        position,
        world_name,
        active_effects,
        sprinting,
        gamemode,
    ) in &mut query
    {
        let may_fly = may_fly(&abilities, gamemode);
        let spectator = is_spectator(gamemode);
        let was_flying = abilities.flying;

        let forced = match demand {
            module::State::On(speed) if !spectator => Some(speed),
            module::State::Release => {
                abilities.flying = false;
                abilities.can_fly = may_fly;
                abilities.flying_speed = DEFAULT_FLYING_SPEED;
                None
            }
            _ => None,
        };

        if spectator && fly_speed_scroll != 0.0 {
            abilities.flying_speed = (abilities.flying_speed + fly_speed_scroll)
                .clamp(MIN_SCROLL_FLYING_SPEED, MAX_SCROLL_FLYING_SPEED);
        }

        if let Some(speed) = forced {
            abilities.can_fly = true;
            abilities.flying = true;
            abilities.flying_speed = speed;
        } else if !may_fly {
            abilities.flying = false;
        } else if spectator {
            abilities.flying = true;
        } else if toggle_requested {
            abilities.flying = !abilities.flying;
        }

        if abilities.flying != was_flying && matches!(demand, module::State::Idle) {
            commands.trigger(SendGamePacketEvent::new(
                entity,
                ServerboundPlayerAbilities {
                    is_flying: abilities.flying,
                },
            ));
        }

        if !abilities.flying {
            pre_travel_y.0.remove(&entity);
            continue;
        }
        flying_now = true;

        let sprinting = sprinting.is_some_and(|s| **s);

        if abilities.flying != was_flying && physics.on_ground() {
            jump_from_ground(
                &mut physics,
                *position,
                *direction,
                Sprinting(sprinting),
                world_name,
                &worlds,
                active_effects,
            );
        }

        let speed = flying_speed(&abilities, sprinting);

        let vertical =
            (move_flags & MOVE_JUMP != 0) as i32 - (move_flags & MOVE_DESCEND != 0) as i32;
        if vertical != 0 {
            let base = flying_speed(&abilities, false) as f64;
            physics.velocity.y += vertical as f64 * base * VERTICAL_IMPULSE_FACTOR;
        }

        if !physics.on_ground() {
            let azalea_speed = if sprinting {
                AZALEA_AIR_SPEED_SPRINTING
            } else {
                AZALEA_AIR_SPEED
            };
            let acceleration = Vec3::new(
                physics.x_acceleration as f64,
                physics.y_acceleration as f64,
                physics.z_acceleration as f64,
            );
            move_relative(&mut physics, *direction, speed - azalea_speed, acceleration);
        }

        pre_travel_y.0.insert(entity, physics.velocity.y);
    }

    if !flying_now {
        shared.lock().unwrap().session.flying = false;
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn flight_post_travel(
    mut query: Query<
        (
            Entity,
            &mut PlayerAbilities,
            &mut Physics,
            Option<&GameMode>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    mut pre_travel_y: ResMut<PreTravelVelocityY>,
    mut commands: Commands,
) {
    let Some(shared) = crate::SHARED.get() else {
        return;
    };

    for (entity, mut abilities, mut physics, gamemode) in &mut query {
        let Some(pre_y) = pre_travel_y.0.remove(&entity) else {
            continue;
        };
        if !abilities.flying {
            continue;
        }

        physics.velocity.y = pre_y * VERTICAL_DRAG;

        if physics.on_ground() && !is_spectator(gamemode) && !module::forcing() {
            abilities.flying = false;
            commands.trigger(SendGamePacketEvent::new(
                entity,
                ServerboundPlayerAbilities { is_flying: false },
            ));
        }

        shared.lock().unwrap().session.flying = abilities.flying;
    }
}

pub(crate) fn flight_anti_kick(
    mut query: Query<
        (Entity, &Physics, &mut LastSentPosition),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    mut commands: Commands,
) {
    if !module::sinking() {
        return;
    }

    for (entity, physics, mut last_sent) in &mut query {
        if physics.on_ground() {
            continue;
        }
        let pos = Vec3::new(last_sent.x, last_sent.y - module::SINK_STEP, last_sent.z);
        commands.trigger(SendGamePacketEvent::new(
            entity,
            ServerboundMovePlayerPos {
                pos,
                flags: MoveFlags {
                    on_ground: false,
                    horizontal_collision: physics.horizontal_collision,
                },
            },
        ));
        **last_sent = pos;
    }
}
