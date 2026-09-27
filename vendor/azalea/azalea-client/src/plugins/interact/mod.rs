pub mod pick;

use std::collections::HashMap;

use azalea_block::BlockState;
use azalea_core::{
    delta::LpVec3,
    direction::Direction,
    game_type::GameMode,
    hit_result::{BlockHitResult, HitResult},
    position::{BlockPos, Vec3},
    tick::GameTick,
};
use azalea_entity::{
    Attributes, LocalEntity, LookDirection, PlayerAbilities, Position,
    attributes::{
        creative_block_interaction_range_modifier, creative_entity_interaction_range_modifier,
    },
    clamp_look_direction,
    indexing::EntityIdIndex,
    inventory::Inventory,
};
use azalea_inventory::{ItemStack, ItemStackData, components};
use azalea_physics::{
    PhysicsSystems, client_movement::ClientMovementState,
    collision::entity_collisions::update_last_bounding_box,
};
use azalea_protocol::packets::game::{
    ServerboundInteract, ServerboundUseItem, s_interact::InteractionHand,
    s_swing::ServerboundSwing, s_use_item_on::ServerboundUseItemOn,
};
use azalea_world::World;
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use tracing::warn;

use super::mining::Mining;
use crate::{
    attack::handle_attack_event,
    interact::pick::{HitResultComponent, update_hit_result_component},
    inventory::InventorySystems,
    local_player::PermissionLevel,
    movement::MoveEventsSystems,
    packet::game::SendGamePacketEvent,
    respawn::perform_respawn,
};

pub struct InteractPlugin;
impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StartUseItemEvent>()
            .add_systems(
                Update,
                (
                    update_attributes_for_gamemode,
                    handle_start_use_item_event,
                    update_hit_result_component
                        .after(clamp_look_direction)
                        .after(update_last_bounding_box),
                )
                    .after(InventorySystems)
                    .after(MoveEventsSystems)
                    .after(perform_respawn)
                    .after(handle_attack_event)
                    .chain(),
            )
            .add_systems(
                GameTick,
                handle_start_use_item_queued.before(PhysicsSystems),
            )
            .add_observer(handle_entity_interact)
            .add_observer(handle_swing_arm_trigger);
    }
}

#[derive(Clone, Component, Debug, Default)]
pub struct BlockStatePredictionHandler {
    seq: u32,
    server_state: HashMap<BlockPos, ServerVerifiedState>,
}
#[derive(Clone, Debug)]
struct ServerVerifiedState {
    seq: u32,
    block_state: BlockState,
    #[allow(unused)]
    player_pos: Vec3,
}

impl BlockStatePredictionHandler {
    pub fn start_predicting(&mut self) -> u32 {
        self.seq += 1;
        self.seq
    }

    pub fn retain_known_server_state(
        &mut self,
        pos: BlockPos,
        old_state: BlockState,
        player_pos: Vec3,
    ) {
        self.server_state
            .entry(pos)
            .and_modify(|s| s.seq = self.seq)
            .or_insert(ServerVerifiedState {
                seq: self.seq,
                block_state: old_state,
                player_pos,
            });
    }

    pub fn update_known_server_state(&mut self, pos: BlockPos, state: BlockState) -> bool {
        if let Some(s) = self.server_state.get_mut(&pos) {
            s.block_state = state;
            true
        } else {
            false
        }
    }

    pub fn end_prediction_up_to(&mut self, seq: u32, world: &World) {
        let mut to_remove = Vec::new();
        for (pos, state) in &self.server_state {
            if state.seq > seq {
                continue;
            }
            to_remove.push(*pos);

            let client_block_state = world.get_block_state(*pos).unwrap_or_default();
            let server_block_state = state.block_state;
            if client_block_state == server_block_state {
                continue;
            }
            world.set_block_state(*pos, server_block_state);
        }

        for pos in to_remove {
            self.server_state.remove(&pos);
        }
    }
}

#[doc(alias("right click"))]
#[derive(Message)]
pub struct StartUseItemEvent {
    pub entity: Entity,
    pub hand: InteractionHand,
    pub force_block: Option<BlockPos>,
}
pub fn handle_start_use_item_event(
    mut commands: Commands,
    mut events: MessageReader<StartUseItemEvent>,
) {
    for event in events.read() {
        commands.entity(event.entity).insert(StartUseItemQueued {
            hand: event.hand,
            force_block: event.force_block,
        });
    }
}

#[derive(Component, Debug)]
pub struct StartUseItemQueued {
    pub hand: InteractionHand,
    pub force_block: Option<BlockPos>,
}
#[allow(clippy::type_complexity)]
pub fn handle_start_use_item_queued(
    mut commands: Commands,
    query: Query<(
        Entity,
        &StartUseItemQueued,
        &mut BlockStatePredictionHandler,
        &HitResultComponent,
        &LookDirection,
        Option<&Mining>,
    )>,
) {
    for (entity, start_use_item, mut prediction_handler, hit_result, look_direction, mining) in
        query
    {
        commands.entity(entity).remove::<StartUseItemQueued>();

        if mining.is_some() {
            warn!("Got a StartUseItemEvent for a client that was mining");
        }

        let mut hit_result = (**hit_result).clone();

        if let Some(force_block) = start_use_item.force_block {
            let hit_result_matches = if let HitResult::Block(block_hit_result) = &hit_result {
                block_hit_result.block_pos == force_block
            } else {
                false
            };

            if !hit_result_matches {
                hit_result = HitResult::Block(BlockHitResult {
                    location: force_block.center(),
                    direction: Direction::Up,
                    block_pos: force_block,
                    inside: false,
                    world_border: false,
                    miss: false,
                });
            }
        }

        match &hit_result {
            HitResult::Block(r) => {
                let seq = prediction_handler.start_predicting();
                if r.miss {
                    commands.trigger(SendGamePacketEvent::new(
                        entity,
                        ServerboundUseItem {
                            hand: start_use_item.hand,
                            seq,
                            x_rot: look_direction.x_rot(),
                            y_rot: look_direction.y_rot(),
                        },
                    ));
                } else {
                    commands.trigger(SendGamePacketEvent::new(
                        entity,
                        ServerboundUseItemOn {
                            hand: start_use_item.hand,
                            block_hit: r.into(),
                            seq,
                        },
                    ));
                }
            }
            HitResult::Entity(r) => {
                commands.trigger(EntityInteractEvent {
                    client: entity,
                    target: r.entity,
                    location: Some(r.location),
                });
            }
        }
    }
}

#[derive(Clone, Debug, EntityEvent)]
pub struct EntityInteractEvent {
    #[event_target]
    pub client: Entity,
    pub target: Entity,
    pub location: Option<Vec3>,
}

pub fn handle_entity_interact(
    trigger: On<EntityInteractEvent>,
    mut commands: Commands,
    client_query: Query<(&ClientMovementState, &EntityIdIndex, &HitResultComponent)>,
    target_query: Query<&Position>,
) {
    let Some((physics_state, entity_id_index, hit_result)) = client_query.get(trigger.client).ok()
    else {
        warn!(
            "tried to interact with an entity but the client didn't have the required components"
        );
        return;
    };

    let Some(entity_id) = entity_id_index.get_by_ecs_entity(trigger.target) else {
        warn!("tried to interact with an entity that isn't known by the client");
        return;
    };

    let location = if let Some(l) = trigger.location {
        l
    } else {
        if let Some(entity_hit_result) = hit_result.as_entity_hit_result()
            && entity_hit_result.entity == trigger.target
        {
            entity_hit_result.location
        } else {
            let Ok(target_position) = target_query.get(trigger.target) else {
                warn!("tried to look at an entity without the entity having a position");
                return;
            };
            **target_position
        }
    };

    let interact = ServerboundInteract {
        entity_id,
        hand: InteractionHand::MainHand,
        location: LpVec3::from(location),
        using_secondary_action: physics_state.trying_to_crouch,
    };
    commands.trigger(SendGamePacketEvent::new(trigger.client, interact.clone()));

    let consumes_action = false;
    if !consumes_action {
        commands.trigger(SendGamePacketEvent::new(trigger.client, interact));
    }
}

pub fn check_is_interaction_restricted(
    world: &World,
    block_pos: BlockPos,
    game_mode: &GameMode,
    inventory: &Inventory,
) -> bool {
    match game_mode {
        GameMode::Adventure => {

            let held_item = inventory.held_item();
            match &held_item {
                ItemStack::Present(item) => {
                    let block = world.chunks.get_block_state(block_pos);
                    let Some(block) = block else {
                        return true;
                    };
                    check_block_can_be_broken_by_item_in_adventure_mode(item, &block)
                }
                _ => true,
            }
        }
        GameMode::Spectator => true,
        _ => false,
    }
}

pub fn check_block_can_be_broken_by_item_in_adventure_mode(
    item: &ItemStackData,
    _block: &BlockState,
) -> bool {

    if item.get_component::<components::CanBreak>().is_none() {
        return false;
    };

    false

}

pub fn can_use_game_master_blocks(
    abilities: &PlayerAbilities,
    permission_level: &PermissionLevel,
) -> bool {
    abilities.instant_break && **permission_level >= 2
}

#[derive(Clone, Debug, EntityEvent)]
pub struct SwingArmEvent {
    pub entity: Entity,
}
pub fn handle_swing_arm_trigger(swing_arm: On<SwingArmEvent>, mut commands: Commands) {
    commands.trigger(SendGamePacketEvent::new(
        swing_arm.entity,
        ServerboundSwing {
            hand: InteractionHand::MainHand,
        },
    ));
}

#[allow(clippy::type_complexity)]
fn update_attributes_for_gamemode(
    query: Query<(&mut Attributes, &GameMode), (With<LocalEntity>, Changed<GameMode>)>,
) {
    for (mut attributes, &game_mode) in query {
        if game_mode == GameMode::Creative {
            attributes
                .block_interaction_range
                .insert(creative_block_interaction_range_modifier());
            attributes
                .entity_interaction_range
                .insert(creative_entity_interaction_range_modifier());
        } else {
            attributes
                .block_interaction_range
                .remove(&creative_block_interaction_range_modifier().id);
            attributes
                .entity_interaction_range
                .remove(&creative_entity_interaction_range_modifier().id);
        }
    }
}
