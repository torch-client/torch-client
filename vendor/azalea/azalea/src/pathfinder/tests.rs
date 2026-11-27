use bevy_platform::time::Instant;
use std::{
    collections::HashSet,
    sync::Arc,
    thread,
    time::Duration,
};

use azalea_block::BlockState;
use azalea_core::position::{BlockPos, ChunkPos};
use azalea_registry::builtin::BlockKind;
use azalea_world::{Chunk, ChunkStorage, PartialChunkStorage};

use super::{
    GotoEvent,
    astar::PathfinderTimeout,
    goals::BlockPosGoal,
    moves,
    simulation::{SimulatedPlayerBundle, Simulation},
};
use crate::pathfinder::goto_event::PathfinderOpts;

fn setup_blockposgoal_simulation(
    partial_chunks: &mut PartialChunkStorage,
    start_pos: BlockPos,
    end_pos: BlockPos,
    solid_blocks: &[BlockPos],
) -> Simulation {
    let mut simulation = setup_simulation_world(partial_chunks, start_pos, solid_blocks, &[]);

    simulation.app.world_mut().write_message(GotoEvent {
        entity: simulation.entity,
        goal: Arc::new(BlockPosGoal(end_pos)),
        opts: PathfinderOpts {
            successors_fn: moves::default_move,
            allow_mining: false,
            retry_on_no_path: true,
            min_timeout: PathfinderTimeout::Nodes(1_000_000),
            max_timeout: PathfinderTimeout::Nodes(5_000_000),
            yield_fn: None,
            placing: None,
            safety: super::MoveSafety::new(),
        },
        continuation: false,
    });
    simulation
}

fn setup_simulation_world(
    partial_chunks: &mut PartialChunkStorage,
    start_pos: BlockPos,
    solid_blocks: &[BlockPos],
    extra_blocks: &[(BlockPos, BlockState)],
) -> Simulation {
    let mut chunk_positions = HashSet::new();
    for block_pos in solid_blocks {
        chunk_positions.insert(ChunkPos::from(block_pos));
    }
    for (block_pos, _) in extra_blocks {
        chunk_positions.insert(ChunkPos::from(block_pos));
    }

    let mut chunks = ChunkStorage::default();
    for chunk_pos in chunk_positions {
        partial_chunks.set(&chunk_pos, Some(Chunk::default()), &mut chunks);
    }
    for block_pos in solid_blocks {
        chunks.set_block_state(*block_pos, BlockKind::Stone.into());
    }
    for (block_pos, block_state) in extra_blocks {
        chunks.set_block_state(*block_pos, *block_state);
    }

    let player = SimulatedPlayerBundle::new(start_pos.center_bottom());
    Simulation::new(chunks, player)
}

pub fn assert_simulation_reaches(simulation: &mut Simulation, ticks: usize, end_pos: BlockPos) {
    wait_until_bot_starts_moving(simulation);
    for _ in 0..ticks {
        simulation.tick();
    }
    assert_eq!(BlockPos::from(simulation.position()), end_pos);
}

pub fn wait_until_bot_starts_moving(simulation: &mut Simulation) {
    let start_pos = simulation.position();
    let start_time = Instant::now();
    while simulation.position() == start_pos
        && !simulation.is_mining()
        && start_time.elapsed() < Duration::from_millis(5000)
    {
        simulation.tick();
        thread::yield_now();
    }
}

#[test]
fn test_simple_forward() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(0, 71, 1),
        &[BlockPos::new(0, 70, 0), BlockPos::new(0, 70, 1)],
    );
    assert_simulation_reaches(&mut simulation, 20, BlockPos::new(0, 71, 1));
}

#[test]
fn test_double_diagonal_with_walls() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(2, 71, 2),
        &[
            BlockPos::new(0, 70, 0),
            BlockPos::new(1, 70, 1),
            BlockPos::new(2, 70, 2),
            BlockPos::new(1, 72, 0),
            BlockPos::new(2, 72, 1),
        ],
    );
    assert_simulation_reaches(&mut simulation, 30, BlockPos::new(2, 71, 2));
}

#[test]
fn test_jump_with_sideways_momentum() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 3),
        BlockPos::new(5, 76, 0),
        &[
            BlockPos::new(0, 70, 3),
            BlockPos::new(0, 70, 2),
            BlockPos::new(0, 70, 1),
            BlockPos::new(0, 70, 0),
            BlockPos::new(1, 71, 0),
            BlockPos::new(2, 72, 0),
            BlockPos::new(3, 73, 0),
            BlockPos::new(4, 74, 0),
            BlockPos::new(5, 75, 0),
        ],
    );
    assert_simulation_reaches(&mut simulation, 120, BlockPos::new(5, 76, 0));
}

#[test]
fn test_parkour_2_block_gap() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(0, 71, 3),
        &[BlockPos::new(0, 70, 0), BlockPos::new(0, 70, 3)],
    );
    assert_simulation_reaches(&mut simulation, 40, BlockPos::new(0, 71, 3));
}

#[test]
fn test_descend_and_parkour_2_block_gap() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(3, 67, 4),
        &[
            BlockPos::new(0, 70, 0),
            BlockPos::new(0, 69, 1),
            BlockPos::new(0, 68, 2),
            BlockPos::new(0, 67, 3),
            BlockPos::new(0, 66, 4),
            BlockPos::new(3, 66, 4),
        ],
    );
    assert_simulation_reaches(&mut simulation, 100, BlockPos::new(3, 67, 4));
}

#[test]
fn test_small_descend_and_parkour_2_block_gap() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(0, 70, 5),
        &[
            BlockPos::new(0, 70, 0),
            BlockPos::new(0, 70, 1),
            BlockPos::new(0, 69, 2),
            BlockPos::new(0, 69, 5),
        ],
    );
    assert_simulation_reaches(&mut simulation, 40, BlockPos::new(0, 70, 5));
}

#[test]
fn test_quickly_descend() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(0, 68, 3),
        &[
            BlockPos::new(0, 70, 0),
            BlockPos::new(0, 69, 1),
            BlockPos::new(0, 68, 2),
            BlockPos::new(0, 67, 3),
        ],
    );
    assert_simulation_reaches(&mut simulation, 60, BlockPos::new(0, 68, 3));
}

#[test]
fn test_2_gap_ascend_thrice() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(3, 74, 0),
        &[
            BlockPos::new(0, 70, 0),
            BlockPos::new(0, 71, 3),
            BlockPos::new(3, 72, 3),
            BlockPos::new(3, 73, 0),
        ],
    );
    assert_simulation_reaches(&mut simulation, 60, BlockPos::new(3, 74, 0));
}

#[test]
fn test_consecutive_3_gap_parkour() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(4, 71, 12),
        &[
            BlockPos::new(0, 70, 0),
            BlockPos::new(0, 70, 4),
            BlockPos::new(0, 70, 8),
            BlockPos::new(0, 70, 12),
            BlockPos::new(4, 70, 12),
        ],
    );
    assert_simulation_reaches(&mut simulation, 80, BlockPos::new(4, 71, 12));
}

#[test]
fn test_jumps_with_more_sideways_momentum() {
    let mut partial_chunks = PartialChunkStorage::default();
    let mut simulation = setup_blockposgoal_simulation(
        &mut partial_chunks,
        BlockPos::new(0, 71, 0),
        BlockPos::new(4, 74, 9),
        &[
            BlockPos::new(0, 70, 0),
            BlockPos::new(0, 70, 1),
            BlockPos::new(0, 70, 2),
            BlockPos::new(0, 71, 3),
            BlockPos::new(0, 72, 6),
            BlockPos::new(0, 73, 9),
            BlockPos::new(2, 73, 9),
            BlockPos::new(4, 73, 9),
        ],
    );
    assert_simulation_reaches(&mut simulation, 80, BlockPos::new(4, 74, 9));
}

#[test]
fn test_mine_through_non_colliding_block() {
    let mut partial_chunks = PartialChunkStorage::default();

    let mut simulation = setup_simulation_world(
        &mut partial_chunks,
        BlockPos::new(0, 72, 1),
        &[BlockPos::new(0, 71, 1)],
        &[
            (BlockPos::new(0, 71, 0), BlockKind::SculkVein.into()),
            (BlockPos::new(0, 70, 0), BlockKind::GrassBlock.into()),
            (BlockPos::new(0, 69, 0), BlockKind::Tnt.into()),
        ],
    );

    simulation.app.world_mut().write_message(GotoEvent {
        entity: simulation.entity,
        goal: Arc::new(BlockPosGoal(BlockPos::new(0, 70, 0))),
        opts: PathfinderOpts::new()
            .min_timeout(PathfinderTimeout::Nodes(1_000_000))
            .max_timeout(PathfinderTimeout::Nodes(5_000_000)),
        continuation: false,
    });

    assert_simulation_reaches(&mut simulation, 200, BlockPos::new(0, 70, 0));
}

mod search {
    use std::sync::atomic::AtomicUsize;

    use bevy_ecs::entity::Entity;
    use parking_lot::RwLock;

    use super::*;
    use crate::pathfinder::{
        CalculatePathCtx, MoveSafety, calculate_path, custom_state::CustomPathfinderState,
        mining::MiningCache,
    };

    type Blocks = Vec<(BlockPos, BlockState)>;

    fn world(
        blocks: &[(BlockPos, BlockState)],
    ) -> (PartialChunkStorage, Arc<RwLock<azalea_world::World>>) {
        let mut partial = PartialChunkStorage::default();
        let mut chunks = ChunkStorage::default();
        for x in -1..=1 {
            for z in -1..=1 {
                partial.set(&ChunkPos::new(x, z), Some(Chunk::default()), &mut chunks);
            }
        }
        for (pos, state) in blocks {
            chunks.set_block_state(*pos, *state);
        }
        (partial, Arc::new(RwLock::new(chunks.into())))
    }

    fn fill(out: &mut Vec<(BlockPos, BlockState)>, a: BlockPos, b: BlockPos, kind: BlockKind) {
        for x in a.x.min(b.x)..=a.x.max(b.x) {
            for y in a.y.min(b.y)..=a.y.max(b.y) {
                for z in a.z.min(b.z)..=a.z.max(b.z) {
                    out.push((BlockPos::new(x, y, z), kind.into()));
                }
            }
        }
    }

    fn route(
        blocks: &[(BlockPos, BlockState)],
        start: BlockPos,
        goal: BlockPos,
        safety: MoveSafety,
    ) -> (bool, Vec<BlockPos>) {
        let (_chunks, world_lock) = world(blocks);
        let event = calculate_path(CalculatePathCtx {
            entity: Entity::PLACEHOLDER,
            start,
            goal: Arc::new(BlockPosGoal(goal)),
            world_lock,
            goto_id_atomic: Arc::new(AtomicUsize::new(0)),
            mining_cache: MiningCache::new(None),
            custom_state: CustomPathfinderState::default(),
            opts: PathfinderOpts::new()
                .allow_mining(false)
                .safety(safety)
                .min_timeout(PathfinderTimeout::Nodes(200_000))
                .max_timeout(PathfinderTimeout::Nodes(200_000)),
        })
        .expect("nothing else bumped the goto id");
        let nodes: Vec<BlockPos> = event
            .path
            .unwrap_or_default()
            .iter()
            .map(|e| e.movement.target)
            .collect();
        let reached = !event.is_partial && nodes.last() == Some(&goal);
        (reached, nodes)
    }

    fn safe() -> MoveSafety {
        MoveSafety::new()
    }

    fn beside(node: BlockPos, cells: &[BlockPos]) -> bool {
        cells.iter().any(|c| {
            (c.y == node.y || c.y == node.y + 1)
                && (c.x - node.x).abs() + (c.z - node.z).abs() == 1
        })
    }

    #[test]
    fn a_pond_is_swum_across() {
        let mut b: Blocks = Vec::new();
        fill(&mut b, BlockPos::new(-1, 70, -3), BlockPos::new(12, 70, 3), BlockKind::Stone);
        fill(&mut b, BlockPos::new(2, 69, -3), BlockPos::new(8, 70, 3), BlockKind::Water);
        fill(&mut b, BlockPos::new(2, 68, -3), BlockPos::new(8, 68, 3), BlockKind::Stone);
        let (reached, nodes) = route(&b, BlockPos::new(0, 71, 0), BlockPos::new(10, 71, 0), safe());
        assert!(reached, "{nodes:?}");
        assert!(nodes.iter().any(|n| n.y == 70 && (2..=8).contains(&n.x)), "{nodes:?}");
    }

    #[test]
    fn a_one_deep_pool_is_left_onto_a_one_high_bank() {
        let mut b: Blocks = Vec::new();
        fill(&mut b, BlockPos::new(0, 69, -1), BlockPos::new(2, 69, 1), BlockKind::Stone);
        fill(&mut b, BlockPos::new(0, 70, -1), BlockPos::new(2, 70, 1), BlockKind::Water);
        fill(&mut b, BlockPos::new(3, 70, -1), BlockPos::new(6, 70, 1), BlockKind::Stone);
        let (reached, nodes) = route(&b, BlockPos::new(1, 70, 0), BlockPos::new(5, 71, 0), safe());
        assert!(reached, "{nodes:?}");
    }

    #[test]
    fn a_lava_pool_is_walked_round_and_never_beside() {
        let mut b: Blocks = Vec::new();
        fill(&mut b, BlockPos::new(-1, 70, -5), BlockPos::new(10, 70, 5), BlockKind::Stone);
        let mut lava = Vec::new();
        fill(&mut lava, BlockPos::new(4, 71, -1), BlockPos::new(5, 71, 1), BlockKind::Lava);
        b.extend(lava.iter().copied());
        let lava: Vec<BlockPos> = lava.iter().map(|(p, _)| *p).collect();
        let (reached, nodes) = route(&b, BlockPos::new(0, 71, 0), BlockPos::new(9, 71, 0), safe());
        assert!(reached, "{nodes:?}");
        for n in &nodes {
            assert!(!lava.contains(n), "{n:?} in lava: {nodes:?}");
            assert!(!beside(*n, &lava), "{n:?} beside lava: {nodes:?}");
        }
    }

    #[test]
    fn a_four_block_drop_onto_stone_is_refused_unless_allowed() {
        let mut b: Blocks = Vec::new();
        b.push((BlockPos::new(0, 80, 0), BlockKind::Stone.into()));
        b.push((BlockPos::new(1, 76, 0), BlockKind::Stone.into()));
        let (start, goal) = (BlockPos::new(0, 81, 0), BlockPos::new(1, 77, 0));
        let (reached, nodes) = route(&b, start, goal, safe());
        assert!(!reached, "{nodes:?}");
        let (reached, nodes) = route(
            &b,
            start,
            goal,
            MoveSafety {
                max_fall: 4,
                ..safe()
            },
        );
        assert!(reached, "{nodes:?}");
    }

    #[test]
    fn a_ten_block_drop_is_taken_into_deep_enough_water() {
        let start = BlockPos::new(0, 81, 0);
        let goal = BlockPos::new(1, 70, 0);
        let mut b: Blocks = vec![(BlockPos::new(0, 80, 0), BlockKind::Stone.into())];
        fill(&mut b, BlockPos::new(1, 69, 0), BlockPos::new(1, 70, 0), BlockKind::Water);
        b.push((BlockPos::new(1, 68, 0), BlockKind::Stone.into()));
        let (reached, nodes) = route(&b, start, goal, safe());
        assert!(reached, "{nodes:?}");
        let b: Blocks = vec![
            (BlockPos::new(0, 80, 0), BlockKind::Stone.into()),
            (BlockPos::new(1, 70, 0), BlockKind::Water.into()),
            (BlockPos::new(1, 69, 0), BlockKind::Stone.into()),
        ];
        let (reached, nodes) = route(&b, start, goal, safe());
        assert!(!reached, "{nodes:?}");
    }

    #[test]
    fn a_magma_strip_is_neither_stepped_on_nor_jumped() {
        let mut b: Blocks = Vec::new();
        fill(&mut b, BlockPos::new(0, 70, -2), BlockPos::new(6, 70, 2), BlockKind::Stone);
        fill(&mut b, BlockPos::new(3, 70, -2), BlockPos::new(3, 70, 1), BlockKind::MagmaBlock);
        let (reached, nodes) = route(&b, BlockPos::new(0, 71, 0), BlockPos::new(6, 71, 0), safe());
        assert!(reached, "{nodes:?}");
        assert!(nodes.contains(&BlockPos::new(3, 71, 2)), "{nodes:?}");
    }

    #[test]
    fn a_fire_line_is_passed_away_from_the_flames() {
        let mut b: Blocks = Vec::new();
        fill(&mut b, BlockPos::new(0, 70, -3), BlockPos::new(6, 70, 3), BlockKind::Stone);
        let mut fire = Vec::new();
        fill(&mut fire, BlockPos::new(3, 71, -3), BlockPos::new(3, 71, 1), BlockKind::Fire);
        b.extend(fire.iter().copied());
        let fire: Vec<BlockPos> = fire.iter().map(|(p, _)| *p).collect();
        let (reached, nodes) = route(&b, BlockPos::new(0, 71, 0), BlockPos::new(6, 71, 0), safe());
        assert!(reached, "{nodes:?}");
        assert!(nodes.contains(&BlockPos::new(3, 71, 3)), "{nodes:?}");
        for n in &nodes {
            assert!(!beside(*n, &fire), "{n:?} beside fire: {nodes:?}");
        }
    }

    #[test]
    fn a_swim_stays_within_reach_of_air() {
        let mut b: Blocks = Vec::new();
        fill(&mut b, BlockPos::new(-1, 59, -2), BlockPos::new(9, 70, 2), BlockKind::Stone);
        fill(&mut b, BlockPos::new(0, 60, -1), BlockPos::new(8, 70, 1), BlockKind::Water);
        let start = BlockPos::new(-1, 71, 0);
        let (reached, nodes) = route(&b, start, BlockPos::new(4, 66, 0), safe());
        assert!(reached, "{nodes:?}");
        let (reached, nodes) = route(&b, start, BlockPos::new(4, 60, 0), safe());
        assert!(!reached, "{nodes:?}");
    }

    #[test]
    fn a_wall_into_the_water_is_dived_under() {
        let mut b: Blocks = Vec::new();
        fill(&mut b, BlockPos::new(-1, 63, -2), BlockPos::new(9, 70, 2), BlockKind::Stone);
        fill(&mut b, BlockPos::new(0, 64, -1), BlockPos::new(8, 70, 1), BlockKind::Water);
        fill(&mut b, BlockPos::new(4, 67, -2), BlockPos::new(4, 72, 2), BlockKind::Stone);
        let (reached, nodes) =
            route(&b, BlockPos::new(-1, 71, 0), BlockPos::new(7, 70, 0), safe());
        assert!(reached, "{nodes:?}");
        assert!(nodes.iter().any(|n| n.x == 4 && n.y <= 65), "{nodes:?}");
    }

    #[test]
    fn danger_and_water_classes() {
        use crate::pathfinder::world::{
            is_block_state_danger, is_block_state_hot, is_block_state_water,
        };
        for kind in [
            BlockKind::Water,
            BlockKind::BubbleColumn,
            BlockKind::Seagrass,
            BlockKind::KelpPlant,
        ] {
            assert!(is_block_state_water(kind.into()), "{kind:?}");
        }
        assert!(!is_block_state_water(BlockKind::Lava.into()));
        assert!(!is_block_state_water(BlockState::AIR));
        for kind in [
            BlockKind::Lava,
            BlockKind::Fire,
            BlockKind::SoulFire,
            BlockKind::MagmaBlock,
            BlockKind::Cactus,
            BlockKind::SweetBerryBush,
            BlockKind::WitherRose,
            BlockKind::PowderSnow,
            BlockKind::BubbleColumn,
        ] {
            assert!(is_block_state_danger(kind.into()), "{kind:?}");
        }
        assert!(!is_block_state_danger(BlockKind::Stone.into()));
        assert!(!is_block_state_danger(BlockKind::Water.into()));
        assert!(is_block_state_hot(BlockKind::Lava.into()));
        assert!(!is_block_state_hot(BlockKind::Cactus.into()));
    }
}
