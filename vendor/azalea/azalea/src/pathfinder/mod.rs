pub mod astar;
pub mod costs;
pub mod custom_state;
pub mod debug;
pub mod execute;
pub mod goals;
mod goto_event;
pub mod mining;
pub mod moves;
pub mod positions;
pub mod simulation;
#[cfg(test)]
mod tests;
pub mod world;

use bevy_platform::time::Instant;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{self, AtomicBool, AtomicUsize},
    },

};

use astar::Edge;
use azalea_client::{StartWalkEvent, inventory::InventorySystems, movement::MoveEventsSystems};
use azalea_core::{
    position::{BlockPos, Vec3},
    tick::GameTick,
};
use azalea_entity::{LocalEntity, Position, inventory::Inventory, metadata::Player};
use azalea_world::{WorldName, Worlds};
use bevy_app::{PreUpdate, Update};
use bevy_ecs::prelude::*;
use bevy_tasks::{AsyncComputeTaskPool, Task};
use custom_state::{CustomPathfinderState, CustomPathfinderStateRef};
use futures_lite::future;
pub use goto_event::{GotoEvent, PathfinderOpts};
use parking_lot::RwLock;
use positions::RelBlockPos;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info, warn};

use self::{
    debug::debug_render_path_with_particles, goals::Goal, mining::MiningCache, moves::SuccessorsFn,
};
use crate::{
    Client, WalkDirection,
    app::{App, Plugin},
    ecs::{
        component::Component,
        entity::Entity,
        query::{With, Without},
        system::{Commands, Query, Res},
    },
    pathfinder::{
        astar::{PathfinderTimeout, a_star},
        execute::{DefaultPathfinderExecutionPlugin, simulation::SimulatingPathState},
        moves::MovesCtx,
        world::CachedWorld,
    },
};

#[derive(Clone, Default)]
pub struct PathfinderPlugin;
impl Plugin for PathfinderPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<GotoEvent>()
            .add_message::<PathFoundEvent>()
            .add_message::<StopPathfindingEvent>()
            .add_systems(
                GameTick,
                debug_render_path_with_particles.in_set(PathfinderSystems),
            )
            .add_systems(PreUpdate, add_default_pathfinder.in_set(PathfinderSystems))
            .add_systems(
                Update,
                (
                    goto_listener,
                    handle_tasks,
                    stop_pathfinding_on_world_change,
                    path_found_listener,
                    handle_stop_pathfinding_event,
                )
                    .chain()
                    .before(MoveEventsSystems)
                    .before(InventorySystems)
                    .in_set(PathfinderSystems),
            )
            .add_plugins(DefaultPathfinderExecutionPlugin);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, SystemSet)]
pub struct PathfinderSystems;

#[derive(Clone, Component, Default)]
#[non_exhaustive]
pub struct Pathfinder {
    pub goal: Option<Arc<dyn Goal>>,
    pub opts: Option<PathfinderOpts>,
    pub is_calculating: bool,
    pub goto_id: Arc<AtomicUsize>,
}

#[derive(Clone, Component)]
pub struct ExecutingPath {
    pub path: VecDeque<astar::Edge<BlockPos, moves::MoveData>>,
    pub queued_path: Option<VecDeque<astar::Edge<BlockPos, moves::MoveData>>>,
    pub last_reached_node: BlockPos,
    pub ticks_since_last_node_reached: usize,
    pub is_path_partial: bool,
}
impl ExecutingPath {
    pub fn is_empty_queued_path(&self) -> bool {
        self.queued_path.is_none() || self.queued_path.as_ref().is_some_and(|p| p.is_empty())
    }
}

#[derive(Clone, Debug, Message)]
#[non_exhaustive]
pub struct PathFoundEvent {
    pub entity: Entity,
    pub start: BlockPos,
    pub path: Option<VecDeque<astar::Edge<BlockPos, moves::MoveData>>>,
    pub is_partial: bool,
    pub successors_fn: SuccessorsFn,
    pub allow_mining: bool,
}

#[allow(clippy::type_complexity)]
pub fn add_default_pathfinder(
    mut commands: Commands,
    mut query: Query<Entity, (Without<Pathfinder>, With<LocalEntity>, With<Player>)>,
) {
    for entity in &mut query {
        commands.entity(entity).insert(Pathfinder::default());
    }
}

pub trait PathfinderClientExt {
    fn goto(&self, goal: impl Goal + 'static) -> impl Future<Output = ()>;
    fn goto_with_opts(
        &self,
        goal: impl Goal + 'static,
        opts: PathfinderOpts,
    ) -> impl Future<Output = ()>;
    fn start_goto(&self, goal: impl Goal + 'static);
    fn start_goto_with_opts(&self, goal: impl Goal + 'static, opts: PathfinderOpts);
    fn stop_pathfinding(&self);
    fn force_stop_pathfinding(&self);
    fn wait_until_goto_target_reached(&self) -> impl Future<Output = ()>;
    fn is_goto_target_reached(&self) -> bool;
    fn is_executing_path(&self) -> bool;
    fn is_calculating_path(&self) -> bool;
}

impl PathfinderClientExt for Client {
    async fn goto(&self, goal: impl Goal + 'static) {
        self.goto_with_opts(goal, PathfinderOpts::new()).await;
    }
    async fn goto_with_opts(&self, goal: impl Goal + 'static, opts: PathfinderOpts) {
        self.start_goto_with_opts(goal, opts);
        self.wait_until_goto_target_reached().await;
    }
    fn start_goto(&self, goal: impl Goal + 'static) {
        self.start_goto_with_opts(goal, PathfinderOpts::new());
    }
    fn start_goto_with_opts(&self, goal: impl Goal + 'static, opts: PathfinderOpts) {
        self.ecs
            .write()
            .write_message(GotoEvent::new(self.entity, goal, opts));
    }
    fn stop_pathfinding(&self) {
        self.ecs.write().write_message(StopPathfindingEvent {
            entity: self.entity,
            force: false,
        });
    }
    fn force_stop_pathfinding(&self) {
        self.ecs.write().write_message(StopPathfindingEvent {
            entity: self.entity,
            force: true,
        });
    }

    async fn wait_until_goto_target_reached(&self) {
        self.wait_updates(1).await;

        let mut tick_broadcaster = self.get_tick_broadcaster();
        while !self.is_goto_target_reached() {
            match tick_broadcaster.recv().await {
                Ok(_) => (),
                Err(RecvError::Closed) => return,
                Err(err) => warn!("{err}"),
            };
        }
    }
    fn is_goto_target_reached(&self) -> bool {
        self.component::<Pathfinder>()
            .ok()
            .is_none_or(|p| p.goal.is_none() && !p.is_calculating)
    }
    fn is_executing_path(&self) -> bool {
        self.component::<ExecutingPath>().is_ok()
    }
    fn is_calculating_path(&self) -> bool {
        self.component::<Pathfinder>()
            .is_ok_and(|p| p.is_calculating)
    }
}

#[derive(Component)]
pub struct ComputePath(Task<Option<PathFoundEvent>>);

#[allow(clippy::type_complexity)]
pub fn goto_listener(
    mut commands: Commands,
    mut events: MessageReader<GotoEvent>,
    mut path_found_events: MessageWriter<PathFoundEvent>,
    mut query: Query<(
        &mut Pathfinder,
        Option<&mut ExecutingPath>,
        Option<&SimulatingPathState>,
        &Position,
        &WorldName,
        &Inventory,
        Option<&CustomPathfinderState>,
    )>,
    worlds: Res<Worlds>,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    for event in events.read() {
        let Ok((
            mut pathfinder,
            executing_path,
            simulating_path_state,
            position,
            world_name,
            inventory,
            custom_state,
        )) = query.get_mut(event.entity)
        else {
            warn!("got goto event for an entity that can't pathfind");
            continue;
        };

        if env!("OPT_LEVEL") == "0" {
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, atomic::Ordering::Relaxed) {
                warn!(
                    "Azalea was compiled with no optimizations, which may result in significantly reduced pathfinding performance. Consider following the steps at https://azalea.matdoes.dev/azalea/#optimization for faster performance in debug mode."
                )
            }
        }

        let cur_pos = player_pos_to_block_pos(**position);

        if event.goal.success(cur_pos) {
            pathfinder.goal = None;
            pathfinder.opts = None;
            pathfinder.is_calculating = false;
            debug!("already at goal, not pathfinding");
            continue;
        }

        pathfinder.goal = Some(event.goal.clone());
        pathfinder.opts = Some(event.opts.clone());
        pathfinder.is_calculating = true;

        let world_lock = worlds
            .get(world_name)
            .expect("Entity tried to pathfind but the entity isn't in a valid world");

        let goal = event.goal.clone();
        let entity = event.entity;

        let goto_id_atomic = pathfinder.goto_id.clone();

        let allow_mining = event.opts.allow_mining;
        let inventory_menu = if allow_mining {
            Some(inventory.inventory_menu.clone())
        } else {
            None
        };

        let custom_state = custom_state.cloned().unwrap_or_default();
        let opts = event.opts.clone();

        let mut start = cur_pos;

        if let Some(mut executing_path) = executing_path {

            let instant_path_start = simulating_path_state
                .and_then(|s| s.as_simulated().map(|s| s.target))
                .unwrap_or_else(|| {
                    executing_path
                        .path
                        .iter()
                        .next()
                        .map(|e| e.movement.target)
                        .unwrap_or(cur_pos)
                });

            let path_found_event = calculate_path(CalculatePathCtx {
                entity,
                start: instant_path_start,
                goal: goal.clone(),
                world_lock: world_lock.clone(),
                goto_id_atomic: goto_id_atomic.clone(),
                mining_cache: MiningCache::new(inventory_menu.clone()),
                custom_state: custom_state.clone(),
                opts: PathfinderOpts {
                    min_timeout: PathfinderTimeout::Nodes(2_000),
                    max_timeout: PathfinderTimeout::Nodes(2_000),
                    ..opts
                },
            });

            if let Some(path_found_event) = path_found_event
                && !path_found_event.is_partial
            {
                debug!("Found path instantly!");

                let instant_path_start_index = executing_path
                    .path
                    .iter()
                    .position(|e| e.movement.target == instant_path_start);
                if let Some(instant_path_start_index) = instant_path_start_index {
                    let truncate_to_len = instant_path_start_index + 1;
                    debug!("truncating to {truncate_to_len} for instant path");
                    executing_path.path.truncate(truncate_to_len);

                    path_found_events.write(path_found_event);

                    continue;
                } else {
                    warn!(
                        "we just calculated an instant path, but the start of it isn't in the current path? instant_path_start: {instant_path_start:?}, simulating_path_state: {simulating_path_state:?}, executing_path.path: {:?}",
                        executing_path.path
                    )
                }
            } else {
                debug!("Couldn't find path instantly...");
            }

            if !executing_path.path.is_empty() {

                let executing_path_limit = 50;

                executing_path.path.truncate(executing_path_limit);

                start = executing_path
                    .path
                    .back()
                    .expect("path was just checked to not be empty")
                    .movement
                    .target;
            }
        }

        if start == cur_pos {
            info!("got goto {:?}, starting from {start:?}", event.goal);
        } else {
            info!(
                "got goto {:?}, starting from {start:?} (currently at {cur_pos:?})",
                event.goal,
            );
        }

        let mining_cache = MiningCache::new(inventory_menu);
        let task = thread_pool.spawn(async move {
            calculate_path(CalculatePathCtx {
                entity,
                start,
                goal,
                world_lock,
                goto_id_atomic,
                mining_cache,
                custom_state,
                opts,
            })
        });

        commands.entity(event.entity).insert(ComputePath(task));
    }
}

#[inline]
pub fn player_pos_to_block_pos(position: Vec3) -> BlockPos {
    BlockPos::from(position.up(0.5))
}

pub struct CalculatePathCtx {
    pub entity: Entity,
    pub start: BlockPos,
    pub goal: Arc<dyn Goal>,
    pub world_lock: Arc<RwLock<azalea_world::World>>,
    pub goto_id_atomic: Arc<AtomicUsize>,
    pub mining_cache: MiningCache,
    pub custom_state: CustomPathfinderState,

    pub opts: PathfinderOpts,
}

pub fn calculate_path(ctx: CalculatePathCtx) -> Option<PathFoundEvent> {
    debug!("start: {}", ctx.start);

    let goto_id = ctx.goto_id_atomic.fetch_add(1, atomic::Ordering::SeqCst) + 1;

    let origin = ctx.start;
    let cached_world = CachedWorld::new(ctx.world_lock, origin);
    let successors = |pos: RelBlockPos| {
        call_successors_fn(
            &cached_world,
            &ctx.mining_cache,
            &ctx.custom_state.0.read(),
            ctx.opts.successors_fn,
            pos,
        )
    };

    let start_time = Instant::now();

    let astar::Path {
        movements,
        is_partial,
        cost,
    } = a_star(
        RelBlockPos::get_origin(origin),
        |n| ctx.goal.heuristic(n.apply(origin)),
        successors,
        |n| ctx.goal.success(n.apply(origin)),
        ctx.opts.min_timeout,
        ctx.opts.max_timeout,
    );
    let end_time = Instant::now();
    debug!("partial: {is_partial:?}, cost: {cost}");
    let duration = end_time - start_time;
    if is_partial {
        if movements.is_empty() {
            info!("Pathfinder took {duration:?} (empty path)");
        } else {
            info!("Pathfinder took {duration:?} (incomplete path)");
        }
    } else {
        info!("Pathfinder took {duration:?}");
    }

    debug!("Path:");
    for movement in &movements {
        debug!("  {}", movement.target.apply(origin));
    }

    let path = movements.into_iter().collect::<VecDeque<_>>();

    let goto_id_now = ctx.goto_id_atomic.load(atomic::Ordering::SeqCst);
    if goto_id != goto_id_now {
        warn!("finished calculating a path, but it's outdated");
        return None;
    }

    if path.is_empty() && is_partial {
        debug!("this path is empty, we might be stuck :(");
    }

    let mut mapped_path = VecDeque::with_capacity(path.len());
    let mut current_position = RelBlockPos::get_origin(origin);
    for movement in path {
        let mut found_edge = None;
        for edge in successors(current_position) {
            if edge.movement.target == movement.target {
                found_edge = Some(edge);
                break;
            }
        }

        let found_edge = found_edge.expect(
            "path should always still be possible because we're using the same world cache",
        );
        current_position = found_edge.movement.target;

        mapped_path.push_back(Edge {
            movement: astar::Movement {
                target: movement.target.apply(origin),
                data: movement.data,
            },
            cost: found_edge.cost,
        });
    }

    Some(PathFoundEvent {
        entity: ctx.entity,
        start: ctx.start,
        path: Some(mapped_path),
        is_partial,
        successors_fn: ctx.opts.successors_fn,
        allow_mining: ctx.opts.allow_mining,
    })
}

pub fn handle_tasks(
    mut commands: Commands,
    mut transform_tasks: Query<(Entity, &mut ComputePath)>,
    mut path_found_events: MessageWriter<PathFoundEvent>,
) {
    for (entity, mut task) in &mut transform_tasks {
        if let Some(optional_path_found_event) = future::block_on(future::poll_once(&mut task.0)) {
            if let Some(path_found_event) = optional_path_found_event {
                path_found_events.write(path_found_event);
            }

            commands.entity(entity).remove::<ComputePath>();
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn path_found_listener(
    mut events: MessageReader<PathFoundEvent>,
    mut query: Query<(
        &mut Pathfinder,
        Option<&mut ExecutingPath>,
        &WorldName,
        &Inventory,
        Option<&CustomPathfinderState>,
    )>,
    worlds: Res<Worlds>,
    mut commands: Commands,
) {
    for event in events.read() {
        let Ok((mut pathfinder, executing_path, world_name, inventory, custom_state)) =
            query.get_mut(event.entity)
        else {
            debug!("got path found event for an entity that can't pathfind");
            continue;
        };
        if let Some(found_path) = &event.path {
            if let Some(mut executing_path) = executing_path {
                let mut new_path = VecDeque::new();

                if let Some(last_node_of_current_path) = executing_path.path.back() {
                    let world_lock = worlds
                        .get(world_name)
                        .expect("Entity tried to pathfind but the entity isn't in a valid world");
                    let origin = event.start;
                    let successors_fn: moves::SuccessorsFn = event.successors_fn;
                    let cached_world = CachedWorld::new(world_lock, origin);
                    let mining_cache = MiningCache::new(if event.allow_mining {
                        Some(inventory.inventory_menu.clone())
                    } else {
                        None
                    });
                    let custom_state = custom_state.cloned().unwrap_or_default();
                    let custom_state_ref = custom_state.0.read();
                    let successors = |pos: RelBlockPos| {
                        call_successors_fn(
                            &cached_world,
                            &mining_cache,
                            &custom_state_ref,
                            successors_fn,
                            pos,
                        )
                    };

                    if let Some(first_node_of_new_path) = found_path.front() {
                        let last_target_of_current_path = RelBlockPos::from_origin(
                            origin,
                            last_node_of_current_path.movement.target,
                        );
                        let first_target_of_new_path = RelBlockPos::from_origin(
                            origin,
                            first_node_of_new_path.movement.target,
                        );

                        let successors = successors(last_target_of_current_path);
                        if successors
                            .iter()
                            .any(|edge| edge.movement.target == first_target_of_new_path)
                        {
                            debug!("combining old and new paths");
                            debug!(
                                "old path: {:?}",
                                executing_path.path.iter().collect::<Vec<_>>()
                            );
                            debug!(
                                "new path: {:?}",
                                found_path.iter().take(10).collect::<Vec<_>>()
                            );
                            new_path.extend(executing_path.path.iter().cloned());
                        } else {
                            debug!(
                                "failed to combine old and new paths. first_target_of_new_path: {first_target_of_new_path:?}, successors: {successors:?}"
                            )
                        }
                    } else {
                        new_path.extend(executing_path.path.iter().cloned());
                    }
                }

                new_path.extend(found_path.to_owned());

                debug!(
                    "set queued path to {:?}",
                    new_path.iter().take(10).collect::<Vec<_>>()
                );
                executing_path.queued_path = Some(new_path);
                executing_path.is_path_partial = event.is_partial;
            } else if found_path.is_empty() {
                debug!("calculated path is empty, so didn't add ExecutingPath");
                if !pathfinder.opts.as_ref().is_some_and(|o| o.retry_on_no_path) {
                    debug!("retry_on_no_path is set to false, removing goal");
                    pathfinder.goal = None;
                }
            } else {
                commands.entity(event.entity).insert(ExecutingPath {
                    path: found_path.to_owned(),
                    queued_path: None,
                    last_reached_node: event.start,
                    ticks_since_last_node_reached: 0,
                    is_path_partial: event.is_partial,
                });
                debug!(
                    "set path to {:?}",
                    found_path.iter().take(10).collect::<Vec<_>>()
                );
                debug!("partial: {}", event.is_partial);
            }
        } else {
            error!("No path found");
            if let Some(mut executing_path) = executing_path {
                executing_path.queued_path = Some(VecDeque::new());
            } else {
            }
        }
        pathfinder.is_calculating = false;
    }
}

#[derive(Message)]
pub struct StopPathfindingEvent {
    pub entity: Entity,
    pub force: bool,
}

pub fn handle_stop_pathfinding_event(
    mut events: MessageReader<StopPathfindingEvent>,
    mut query: Query<(&mut Pathfinder, &mut ExecutingPath)>,
    mut walk_events: MessageWriter<StartWalkEvent>,
    mut commands: Commands,
) {
    for event in events.read() {
        commands.entity(event.entity).remove::<ComputePath>();

        let Ok((mut pathfinder, mut executing_path)) = query.get_mut(event.entity) else {
            continue;
        };
        pathfinder.goal = None;
        if event.force {
            executing_path.path.clear();
            executing_path.queued_path = None;
        } else {
            executing_path.queued_path = Some(VecDeque::new());
            executing_path.is_path_partial = false;
        }

        if executing_path.path.is_empty() {
            walk_events.write(StartWalkEvent {
                entity: event.entity,
                direction: WalkDirection::None,
            });
            commands.entity(event.entity).remove::<ExecutingPath>();
        }
    }
}

pub fn stop_pathfinding_on_world_change(
    mut query: Query<(Entity, &mut ExecutingPath), Changed<WorldName>>,
    mut stop_pathfinding_events: MessageWriter<StopPathfindingEvent>,
) {
    for (entity, mut executing_path) in &mut query {
        if !executing_path.path.is_empty() {
            debug!("world changed, clearing path");
            executing_path.path.clear();
            stop_pathfinding_events.write(StopPathfindingEvent {
                entity,
                force: true,
            });
        }
    }
}

pub fn call_successors_fn(
    cached_world: &CachedWorld,
    mining_cache: &MiningCache,
    custom_state: &CustomPathfinderStateRef,
    successors_fn: SuccessorsFn,
    pos: RelBlockPos,
) -> Vec<astar::Edge<RelBlockPos, moves::MoveData>> {
    let mut edges = Vec::with_capacity(16);
    let mut ctx = MovesCtx {
        edges: &mut edges,
        world: cached_world,
        mining_cache,
        custom_state,
    };
    successors_fn(&mut ctx, pos);
    edges
}
