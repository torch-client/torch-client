use std::collections::HashSet;

use azalea::app::{App, Plugin};
use azalea::ecs::prelude::*;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::FallFlying;
use azalea::entity::{HasClientLoaded, LocalEntity, Physics};
use azalea_core::game_type::GameMode;
use azalea_core::tick::GameTick;
use azalea_inventory::ItemStack;
use azalea_registry::builtin::ItemKind;

use crate::modules::no_fall as module;

#[derive(Resource, Default)]
struct ClaimedGround(HashSet<Entity>);

pub struct NoFallPlugin;

impl Plugin for NoFallPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClaimedGround>().add_systems(
            GameTick,
            (
                no_fall_claim_ground
                    .after(crate::play::flight::flight_post_travel)
                    .before(azalea::movement::send_position),
                no_fall_restore
                    .after(azalea::movement::send_position)
                    .before(crate::play::flight::flight_anti_kick),
            ),
        );
    }
}

fn no_fall_claim_ground(
    mut query: Query<
        (
            Entity,
            &mut Physics,
            &Inventory,
            Option<&FallFlying>,
            Option<&GameMode>,
        ),
        (With<LocalEntity>, With<HasClientLoaded>),
    >,
    mut claimed: ResMut<ClaimedGround>,
) {
    claimed.0.clear();
    if !module::enabled() {
        return;
    }

    for (entity, mut physics, inventory, fall_flying, gamemode) in &mut query {
        if physics.on_ground() {
            continue;
        }
        if matches!(
            gamemode,
            Some(GameMode::Creative) | Some(GameMode::Spectator)
        ) {
            continue;
        }
        let is_mace = |stack: &ItemStack| matches!(stack, ItemStack::Present(data) if data.kind == ItemKind::Mace);
        let mace = is_mace(inventory.held_item())
            || (crate::modules::auto_mace::enabled() && {
                let menu = inventory.menu();
                menu.hotbar_slots_range()
                    .any(|i| menu.slot(i).is_some_and(is_mace))
            });
        if !module::spoofing(physics.velocity.y, fall_flying.is_some_and(|f| **f), mace) {
            continue;
        }

        physics.set_on_ground(true);
        claimed.0.insert(entity);
    }
}

fn no_fall_restore(
    mut query: Query<(Entity, &mut Physics), With<LocalEntity>>,
    mut claimed: ResMut<ClaimedGround>,
) {
    if claimed.0.is_empty() {
        return;
    }

    for (entity, mut physics) in &mut query {
        if claimed.0.contains(&entity) {
            physics.set_on_ground(false);
            physics.set_last_on_ground(false);
        }
    }
    claimed.0.clear();
}
