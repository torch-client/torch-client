use azalea_block::{BlockState, BlockTrait, fluid_state::FluidState};
use azalea_core::{direction::Direction, game_type::GameMode, position::BlockPos, tick::GameTick};
use azalea_entity::{
    ActiveEffects, Attributes, FluidOnEyes, Physics, PlayerAbilities, Position,
    inventory::Inventory, mining::get_mine_progress,
};
use azalea_inventory::ItemStack;
use azalea_physics::{PhysicsSystems, collision::BlockWithShape};
use azalea_protocol::packets::game::s_player_action::{self, ServerboundPlayerAction};
use azalea_registry::builtin::{BlockKind, ItemKind};
use azalea_world::{WorldName, Worlds};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use derive_more::{Deref, DerefMut};
use tracing::{debug, trace, warn};

use crate::{
    interact::{
        BlockStatePredictionHandler, SwingArmEvent, can_use_game_master_blocks,
        check_is_interaction_restricted, pick::HitResultComponent,
    },
    inventory::InventorySystems,
    local_player::{PermissionLevel, WorldHolder},
    movement::MoveEventsSystems,
    packet::game::SendGamePacketEvent,
};

fn has_aqua_affinity(inventory: &Inventory, registries: &azalea_core::registry_holder::RegistryHolder) -> bool {
    use azalea_inventory::components::Enchantments;
    use azalea_registry::DataRegistry;
    use azalea_registry::identifier::Identifier;

    const HELMET_SLOT: usize = 5;
    let Some(ItemStack::Present(data)) = inventory.inventory_menu.slot(HELMET_SLOT) else {
        return false;
    };
    let Some(enchantments) = data.get_component::<Enchantments>() else {
        return false;
    };
    let Some(id) = registries
        .enchantment
        .map
        .get_index_of(&Identifier::new("aqua_affinity".to_string()))
    else {
        return false;
    };
    enchantments
        .levels
        .get(&azalea_registry::Enchantment::new_raw(id as u32))
        .is_some_and(|&lvl| lvl > 0)
}

pub struct MiningPlugin;
impl Plugin for MiningPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StartMiningBlockEvent>()
            .add_message::<StopMiningBlockEvent>()
            .add_message::<MineBlockProgressEvent>()
            .add_message::<AttackBlockEvent>()
            .add_systems(
                GameTick,
                (
                    update_mining_component,
                    handle_auto_mine,
                    handle_mining_queued,
                    decrement_mine_delay,
                    continue_mining_block,
                )
                    .chain()
                    .before(PhysicsSystems)
                    .before(super::movement::send_position)
                    .before(super::interact::handle_start_use_item_queued)
                    .after(azalea_entity::update_fluid_on_eyes)
                    .in_set(MiningSystems),
            )
            .add_systems(
                Update,
                (
                    handle_start_mining_block_event,
                    handle_stop_mining_block_event,
                )
                    .chain()
                    .in_set(MiningSystems)
                    .after(InventorySystems)
                    .after(MoveEventsSystems)
                    .after(crate::interact::pick::update_hit_result_component)
                    .after(crate::attack::handle_attack_event),
            )
            .add_observer(handle_finish_mining_block_observer);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, SystemSet)]
pub struct MiningSystems;

#[derive(Component)]
pub struct LeftClickMine;

#[allow(clippy::type_complexity)]
fn handle_auto_mine(
    mut query: Query<
        (
            &HitResultComponent,
            Entity,
            Option<&Mining>,
            &Inventory,
            &MineBlockPos,
            &MineItem,
        ),
        With<LeftClickMine>,
    >,
    mut start_mining_block_event: MessageWriter<StartMiningBlockEvent>,
    mut stop_mining_block_event: MessageWriter<StopMiningBlockEvent>,
) {
    for (
        hit_result_component,
        entity,
        mining,
        inventory,
        current_mining_pos,
        current_mining_item,
    ) in &mut query.iter_mut()
    {
        let block_pos = hit_result_component
            .as_block_hit_result_if_not_miss()
            .map(|b| b.block_pos);

        if let Some(block_pos) = block_pos
            && (mining.is_none()
                || !is_same_mining_target(
                    block_pos,
                    inventory,
                    current_mining_pos,
                    current_mining_item,
                ))
        {
            start_mining_block_event.write(StartMiningBlockEvent {
                entity,
                position: block_pos,
                force: true,
            });
        } else if mining.is_some() && hit_result_component.miss() {
            stop_mining_block_event.write(StopMiningBlockEvent { entity });
        }
    }
}

#[derive(Clone, Component, Debug)]
#[component(storage = "SparseSet")]
pub struct Mining {
    pub pos: BlockPos,
    pub dir: Direction,
    pub force: bool,
}

#[derive(Debug, Message)]
pub struct StartMiningBlockEvent {
    pub entity: Entity,
    pub position: BlockPos,
    pub force: bool,
}
fn handle_start_mining_block_event(
    mut commands: Commands,
    mut events: MessageReader<StartMiningBlockEvent>,
    mut query: Query<&HitResultComponent>,
) {
    for event in events.read() {
        trace!("{event:?}");
        let hit_result = query.get_mut(event.entity).unwrap();
        if event.force {
            let direction = if let Some(block_hit_result) =
                hit_result.as_block_hit_result_if_not_miss()
                && block_hit_result.block_pos == event.position
            {
                block_hit_result.direction
            } else {
                debug!(
                    "Got StartMiningBlockEvent but we're not looking at the block ({hit_result:?}.block_pos != {:?}). Picking an arbitrary direction instead.",
                    event.position
                );
                Direction::Down
            };
            commands.entity(event.entity).insert(MiningQueued {
                position: event.position,
                direction,
                force: true,
            });
        } else {
            if let Some(block_hit_result) = hit_result.as_block_hit_result_if_not_miss()
                && block_hit_result.block_pos == event.position
            {
                commands.entity(event.entity).insert(MiningQueued {
                    position: event.position,
                    direction: block_hit_result.direction,
                    force: false,
                });
            } else {
                warn!(
                    "Got StartMiningBlockEvent with force=false but we're not looking at the block ({hit_result:?}.block_pos != {:?}). You should've looked at the block before trying to mine with force=false.",
                    event.position
                );
            };
        }
    }
}

#[derive(Clone, Component, Debug)]
pub struct MiningQueued {
    pub position: BlockPos,
    pub direction: Direction,
    pub force: bool,
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn handle_mining_queued(
    mut commands: Commands,
    mut attack_block_events: MessageWriter<AttackBlockEvent>,
    mut mine_block_progress_events: MessageWriter<MineBlockProgressEvent>,
    query: Query<(
        Entity,
        &MiningQueued,
        &WorldHolder,
        &GameMode,
        &Inventory,
        &ActiveEffects,
        &FluidOnEyes,
        &Physics,
        &Attributes,
        Option<&mut Mining>,
        &mut BlockStatePredictionHandler,
        (
            &mut MineDelay,
            &mut MineProgress,
            &mut MineTicks,
            &mut MineItem,
            &mut MineBlockPos,
        ),
    )>,
) {
    for (
        entity,
        mining_queued,
        world_holder,
        &game_mode,
        inventory,
        active_effects,
        fluid_on_eyes,
        physics,
        attributes,
        mut mining,
        mut sequence_number,
        (
            mut mine_delay,
            mut mine_progress,
            mut mine_ticks,
            mut current_mining_item,
            mut current_mining_pos,
        ),
    ) in query
    {
        trace!("handle_mining_queued {mining_queued:?}");
        commands.entity(entity).remove::<MiningQueued>();

        let world = world_holder.shared.read();
        if check_is_interaction_restricted(&world, mining_queued.position, &game_mode, inventory) {
            continue;
        }

        if let Some(mining) = &mut mining {
            if mining_queued.force {
                mining.force = true;
            }
        }

        if game_mode == GameMode::Creative {
            commands.trigger(SendGamePacketEvent::new(
                entity,
                ServerboundPlayerAction {
                    action: s_player_action::Action::StartDestroyBlock,
                    pos: mining_queued.position,
                    direction: mining_queued.direction,
                    seq: sequence_number.start_predicting(),
                },
            ));
            commands.trigger(FinishMiningBlockEvent {
                entity,
                position: mining_queued.position,
            });
            **mine_delay = 5;
            commands.trigger(SwingArmEvent { entity });
        } else if mining.is_none()
            || !is_same_mining_target(
                mining_queued.position,
                inventory,
                &current_mining_pos,
                &current_mining_item,
            )
        {
            if mining.is_some() {
                commands.trigger(SendGamePacketEvent::new(
                    entity,
                    ServerboundPlayerAction {
                        action: s_player_action::Action::AbortDestroyBlock,
                        pos: current_mining_pos
                            .expect("IsMining is true so MineBlockPos must be present"),
                        direction: mining_queued.direction,
                        seq: 0,
                    },
                ));
            }

            let target_block_state = world
                .get_block_state(mining_queued.position)
                .unwrap_or_default();

            let block_is_solid = !target_block_state
                .outline_shape(mining_queued.position)
                .is_empty();

            if block_is_solid && **mine_progress == 0. {
                attack_block_events.write(AttackBlockEvent {
                    entity,
                    position: mining_queued.position,
                });
            }

            let block = Box::<dyn BlockTrait>::from(target_block_state);

            let held_item = inventory.held_item();

            if block_is_solid
                && get_mine_progress(
                    block.as_ref(),
                    held_item,
                    fluid_on_eyes,
                    physics,
                    attributes,
                    active_effects,
                    has_aqua_affinity(inventory, &world.registries),
                ) >= 1.
            {
                commands.trigger(FinishMiningBlockEvent {
                    entity,
                    position: mining_queued.position,
                });
            } else {
                let mining = Mining {
                    pos: mining_queued.position,
                    dir: mining_queued.direction,
                    force: mining_queued.force,
                };
                trace!("inserting mining component {mining:?} for entity {entity:?}");
                commands.entity(entity).insert(mining);
                **current_mining_pos = Some(mining_queued.position);
                **current_mining_item = held_item.clone();
                **mine_progress = 0.;
                **mine_ticks = 0.;
                mine_block_progress_events.write(MineBlockProgressEvent {
                    entity,
                    position: mining_queued.position,
                    destroy_stage: mine_progress.destroy_stage(),
                });
            }

            commands.trigger(SendGamePacketEvent::new(
                entity,
                ServerboundPlayerAction {
                    action: s_player_action::Action::StartDestroyBlock,
                    pos: mining_queued.position,
                    direction: mining_queued.direction,
                    seq: sequence_number.start_predicting(),
                },
            ));
            commands.trigger(SwingArmEvent { entity });
        }
    }
}

#[derive(Message)]
pub struct MineBlockProgressEvent {
    pub entity: Entity,
    pub position: BlockPos,
    pub destroy_stage: Option<u32>,
}

#[derive(Message)]
pub struct AttackBlockEvent {
    pub entity: Entity,
    pub position: BlockPos,
}

fn is_same_mining_target(
    target_block: BlockPos,
    inventory: &Inventory,
    current_mining_pos: &MineBlockPos,
    current_mining_item: &MineItem,
) -> bool {
    let held_item = inventory.held_item();
    Some(target_block) == current_mining_pos.0 && held_item == &current_mining_item.0
}

#[derive(Bundle, Clone, Default)]
pub struct MineBundle {
    pub delay: MineDelay,
    pub progress: MineProgress,
    pub ticks: MineTicks,
    pub mining_pos: MineBlockPos,
    pub mine_item: MineItem,
}

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct MineDelay(pub u32);

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct MineProgress(pub f32);

impl MineProgress {
    pub fn destroy_stage(&self) -> Option<u32> {
        if self.0 > 0. {
            Some((self.0 * 10.) as u32)
        } else {
            None
        }
    }
}

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct MineTicks(pub f32);

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct MineBlockPos(pub Option<BlockPos>);

#[derive(Clone, Component, Debug, Default, Deref, DerefMut)]
pub struct MineItem(pub ItemStack);

#[derive(EntityEvent)]
pub struct FinishMiningBlockEvent {
    pub entity: Entity,
    pub position: BlockPos,
}

pub fn handle_finish_mining_block_observer(
    finish_mining_block: On<FinishMiningBlockEvent>,
    mut query: Query<(
        &WorldName,
        &GameMode,
        &Inventory,
        &PlayerAbilities,
        &PermissionLevel,
        &Position,
        &mut BlockStatePredictionHandler,
    )>,
    worlds: Res<Worlds>,
) {
    let event = finish_mining_block.event();

    let (
        world_name,
        &game_mode,
        inventory,
        abilities,
        permission_level,
        player_pos,
        mut prediction_handler,
    ) = query.get_mut(finish_mining_block.entity).unwrap();
    let world_lock = worlds.get(world_name).unwrap();
    let world = world_lock.read();
    if check_is_interaction_restricted(&world, event.position, &game_mode, inventory) {
        return;
    }

    if game_mode == GameMode::Creative {
        let held_item = inventory.held_item().kind();
        if matches!(held_item, ItemKind::Trident | ItemKind::DebugStick)
            || azalea_registry::tags::items::SWORDS.contains(&held_item)
        {
            return;
        }
    }

    let Some(block_state) = world.get_block_state(event.position) else {
        return;
    };

    let registry_block = block_state.as_block_kind();
    if !can_use_game_master_blocks(abilities, permission_level)
        && matches!(
            registry_block,
            BlockKind::CommandBlock | BlockKind::StructureBlock
        )
    {
        return;
    }
    if block_state == BlockState::AIR {
        return;
    }

    let fluid_state = FluidState::from(block_state);
    let block_state_for_fluid = BlockState::from(fluid_state);
    let old_state = world
        .set_block_state(event.position, block_state_for_fluid)
        .unwrap_or_default();
    prediction_handler.retain_known_server_state(event.position, old_state, **player_pos);
}

#[derive(Message)]
pub struct StopMiningBlockEvent {
    pub entity: Entity,
}
pub fn handle_stop_mining_block_event(
    mut events: MessageReader<StopMiningBlockEvent>,
    mut commands: Commands,
    mut mine_block_progress_events: MessageWriter<MineBlockProgressEvent>,
    mut query: Query<(&MineBlockPos, &mut MineProgress)>,
) {
    for event in events.read() {
        let (mine_block_pos, mut mine_progress) = query.get_mut(event.entity).unwrap();

        let mine_block_pos =
            mine_block_pos.expect("IsMining is true so MineBlockPos must be present");
        commands.trigger(SendGamePacketEvent::new(
            event.entity,
            ServerboundPlayerAction {
                action: s_player_action::Action::AbortDestroyBlock,
                pos: mine_block_pos,
                direction: Direction::Down,
                seq: 0,
            },
        ));
        commands.entity(event.entity).remove::<Mining>();
        **mine_progress = 0.;
        mine_block_progress_events.write(MineBlockProgressEvent {
            entity: event.entity,
            position: mine_block_pos,
            destroy_stage: None,
        });
    }
}

pub fn decrement_mine_delay(mut query: Query<&mut MineDelay>) {
    for mut mine_delay in &mut query {
        if **mine_delay > 0 {
            **mine_delay -= 1;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn continue_mining_block(
    mut query: Query<(
        Entity,
        &WorldName,
        &GameMode,
        &Inventory,
        &MineBlockPos,
        &MineItem,
        &ActiveEffects,
        &FluidOnEyes,
        &Physics,
        &Attributes,
        &Mining,
        &mut MineDelay,
        &mut MineProgress,
        &mut MineTicks,
        &mut BlockStatePredictionHandler,
    )>,
    mut commands: Commands,
    mut mine_block_progress_events: MessageWriter<MineBlockProgressEvent>,
    worlds: Res<Worlds>,
) {
    for (
        entity,
        world_name,
        &game_mode,
        inventory,
        current_mining_pos,
        current_mining_item,
        active_effects,
        fluid_on_eyes,
        physics,
        attributes,
        mining,
        mut mine_delay,
        mut mine_progress,
        mut mine_ticks,
        mut prediction_handler,
    ) in query.iter_mut()
    {
        if game_mode == GameMode::Creative {
            **mine_delay = 5;
            commands.trigger(SendGamePacketEvent::new(
                entity,
                ServerboundPlayerAction {
                    action: s_player_action::Action::StartDestroyBlock,
                    pos: mining.pos,
                    direction: mining.dir,
                    seq: prediction_handler.start_predicting(),
                },
            ));
            commands.trigger(FinishMiningBlockEvent {
                entity,
                position: mining.pos,
            });
            commands.trigger(SwingArmEvent { entity });
        } else if mining.force
            || is_same_mining_target(
                mining.pos,
                inventory,
                current_mining_pos,
                current_mining_item,
            )
        {
            trace!("continue mining block at {:?}", mining.pos);
            let world_lock = worlds.get(world_name).unwrap();
            let world = world_lock.read();
            let target_block_state = world.get_block_state(mining.pos).unwrap_or_default();

            trace!("target_block_state: {target_block_state:?}");

            if target_block_state.is_air() {
                commands.entity(entity).remove::<Mining>();
                continue;
            }
            let block = Box::<dyn BlockTrait>::from(target_block_state);
            **mine_progress += get_mine_progress(
                block.as_ref(),
                current_mining_item,
                fluid_on_eyes,
                physics,
                attributes,
                active_effects,
                has_aqua_affinity(inventory, &world.registries),
            );

            if **mine_ticks % 4. == 0. {
            }
            **mine_ticks += 1.;

            if **mine_progress >= 1. {
                commands.entity(entity).remove::<(Mining, MiningQueued)>();
                trace!("finished mining block at {:?}", mining.pos);
                commands.trigger(FinishMiningBlockEvent {
                    entity,
                    position: mining.pos,
                });
                commands.trigger(SendGamePacketEvent::new(
                    entity,
                    ServerboundPlayerAction {
                        action: s_player_action::Action::StopDestroyBlock,
                        pos: mining.pos,
                        direction: mining.dir,
                        seq: prediction_handler.start_predicting(),
                    },
                ));
                **mine_progress = 0.;
                **mine_ticks = 0.;
                **mine_delay = 5;
            }

            mine_block_progress_events.write(MineBlockProgressEvent {
                entity,
                position: mining.pos,
                destroy_stage: mine_progress.destroy_stage(),
            });
            commands.trigger(SwingArmEvent { entity });
        } else {
            trace!("switching mining target to {:?}", mining.pos);
            commands.entity(entity).insert(MiningQueued {
                position: mining.pos,
                direction: mining.dir,
                force: false,
            });
        }
    }
}

pub fn update_mining_component(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Mining, &HitResultComponent)>,
) {
    for (entity, mut mining, hit_result_component) in &mut query.iter_mut() {
        if let Some(block_hit_result) = hit_result_component.as_block_hit_result_if_not_miss() {
            if mining.force && block_hit_result.block_pos != mining.pos {
                continue;
            }

            if mining.pos != block_hit_result.block_pos {
                debug!(
                    "Updating Mining::pos from {:?} to {:?}",
                    mining.pos, block_hit_result.block_pos
                );
                mining.pos = block_hit_result.block_pos;
            }
            mining.dir = block_hit_result.direction;
        } else {
            if mining.force {
                continue;
            }

            debug!("Removing mining component because we're no longer looking at the block");
            commands.entity(entity).remove::<Mining>();
        }
    }
}
