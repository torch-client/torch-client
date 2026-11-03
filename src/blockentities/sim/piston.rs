mod push_reactions;

use azalea::block::{BlockState, BlockTrait};
use azalea_core::direction::Direction;
use azalea_core::position::BlockPos;
use azalea_registry::builtin::BlockKind;

use crate::blockentities::feed::BlockEntityData;
use crate::blocks::facing;
use push_reactions::{has_block_entity, push_reaction};

const MAX_PUSH_DEPTH: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PushReaction {
    Normal,
    Destroy,
    Block,
    PushOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Look {
    #[default]
    None,
    Block(u32),
    Head {
        long: u32,
        short: u32,
    },
    Retracting {
        long: u32,
        short: u32,
        base: u32,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Moving {
    pub direction: Direction,
    pub extending: bool,
    pub look: Look,
}

impl Moving {
    fn new(moved: BlockState, direction: Direction, extending: bool, source: bool) -> Moving {
        let kind = BlockKind::from(moved);
        let look = if is_air(kind) {
            Look::None
        } else if kind == BlockKind::PistonHead {
            Look::Head {
                long: id(with(moved, &[("short", "false")])),
                short: id(with(moved, &[("short", "true")])),
            }
        } else if source && !extending {
            let base = Box::<dyn BlockTrait>::from(moved);
            let head = with(
                BlockState::from(BlockKind::PistonHead),
                &[
                    ("type", piston_type(kind == BlockKind::StickyPiston)),
                    ("facing", base.get_property("facing").unwrap_or("north")),
                ],
            );
            Look::Retracting {
                long: id(with(head, &[("short", "false")])),
                short: id(with(head, &[("short", "true")])),
                base: id(with(moved, &[("extended", "true")])),
            }
        } else {
            Look::Block(id(moved))
        };
        Moving {
            direction,
            extending,
            look,
        }
    }

    pub fn offset(&self, progress: f32) -> [f32; 3] {
        let extended = if self.extending {
            progress - 1.0
        } else {
            1.0 - progress
        };
        [
            self.direction.x() as f32 * extended,
            self.direction.y() as f32 * extended,
            self.direction.z() as f32 * extended,
        ]
    }

    pub fn draws(&self, progress: f32) -> [Option<(u32, [f32; 3])>; 2] {
        let offset = self.offset(progress);
        match self.look {
            Look::None => [None, None],
            Look::Block(state) => [Some((state, offset)), None],
            Look::Head { long, short } => [
                Some((if progress <= 0.5 { short } else { long }, offset)),
                None,
            ],
            Look::Retracting { long, short, base } => [
                Some((if progress >= 0.5 { short } else { long }, offset)),
                Some((base, [0.0; 3])),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub progress: f32,
    pub progress_o: f32,
}

impl Progress {
    pub fn on_data(&mut self, data: &BlockEntityData) {
        if let BlockEntityData::Piston(_) = data {
            *self = Progress::default();
        }
    }

    pub fn tick(&mut self) {
        self.progress_o = self.progress;
        if self.progress_o < 1.0 {
            self.progress = (self.progress + 0.5).min(1.0);
        }
    }

    pub fn at(&self, partial: f32) -> f32 {
        let a = partial.min(1.0);
        self.progress_o + (self.progress - self.progress_o) * a
    }
}

pub fn block_event(
    world: &azalea_world::World,
    pos: [i32; 3],
    action: u8,
    param: u8,
) -> Vec<([i32; 3], Moving)> {
    let world_state = |p: [i32; 3]| {
        world
            .get_block_state(BlockPos::new(p[0], p[1], p[2]))
            .unwrap_or_default()
    };
    let state = world_state(pos);
    if !is_piston(BlockKind::from(state)) {
        return Vec::new();
    }
    let Some(direction) = Box::<dyn BlockTrait>::from(state)
        .get_property("facing")
        .and_then(facing::from_name)
    else {
        return Vec::new();
    };
    let min_y = world.chunks.min_y();
    let retracting = action != 0;
    let arm = relative(pos, direction, 1);
    let head_at_arm = BlockKind::from(world_state(arm)) == BlockKind::PistonHead;
    let moving_piston = BlockState::from(BlockKind::MovingPiston);
    let level = Level {
        get: |p: [i32; 3]| {
            if retracting && p == pos {
                moving_piston
            } else if retracting && head_at_arm && p == arm {
                BlockState::AIR
            } else {
                world_state(p)
            }
        },
        min_y,
        max_y: min_y + world.chunks.height() as i32 - 1,
    };
    trigger_event(&level, state, pos, direction, action, param)
}

fn trigger_event<F: Fn([i32; 3]) -> BlockState>(
    level: &Level<F>,
    state: BlockState,
    pos: [i32; 3],
    direction: Direction,
    action: u8,
    param: u8,
) -> Vec<([i32; 3], Moving)> {
    let kind = BlockKind::from(state);
    let sticky = match kind {
        BlockKind::Piston => false,
        BlockKind::StickyPiston => true,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    match action {
        0 => move_blocks(level, pos, direction, true, sticky, &mut out),
        1 | 2 => {
            let base = facing::from_3d_data_value(i32::from(param & 7));
            let moved = with(BlockState::from(kind), &[("facing", facing::name(base))]);
            out.push((pos, Moving::new(moved, direction, false, true)));
            if sticky {
                let two = relative(pos, direction, 2);
                let moving = level.state(two);
                let moving_kind = BlockKind::from(moving);
                let pulls = action == 1
                    && !is_air(moving_kind)
                    && level.is_pushable(moving, two, direction.opposite(), false, direction)
                    && (reaction(moving) == PushReaction::Normal || is_piston(moving_kind));
                if pulls {
                    move_blocks(level, pos, direction, false, sticky, &mut out);
                }
            }
        }
        _ => {}
    }
    out
}

fn move_blocks<F: Fn([i32; 3]) -> BlockState>(
    level: &Level<F>,
    piston: [i32; 3],
    direction: Direction,
    extending: bool,
    sticky: bool,
    out: &mut Vec<([i32; 3], Moving)>,
) {
    let Some(to_push) = resolve(level, piston, direction, extending) else {
        return;
    };
    let push = if extending {
        direction
    } else {
        direction.opposite()
    };
    out.extend(to_push.iter().rev().map(|&p| {
        (
            relative(p, push, 1),
            Moving::new(level.state(p), direction, extending, false),
        )
    }));
    if extending {
        let head = with(
            BlockState::from(BlockKind::PistonHead),
            &[
                ("facing", facing::name(direction)),
                ("type", piston_type(sticky)),
            ],
        );
        out.push((
            relative(piston, direction, 1),
            Moving::new(head, direction, true, true),
        ));
    }
}

fn resolve<F: Fn([i32; 3]) -> BlockState>(
    level: &Level<F>,
    piston: [i32; 3],
    direction: Direction,
    extending: bool,
) -> Option<Vec<[i32; 3]>> {
    let (push, start) = if extending {
        (direction, relative(piston, direction, 1))
    } else {
        (direction.opposite(), relative(piston, direction, 2))
    };
    let mut resolver = Resolver {
        level,
        piston,
        push,
        to_push: Vec::new(),
    };
    let next = level.state(start);
    if !level.is_pushable(next, start, push, false, direction) {
        return (extending && reaction(next) == PushReaction::Destroy).then(Vec::new);
    }
    if !resolver.add_block_line(start, push) {
        return None;
    }
    let mut i = 0;
    while i < resolver.to_push.len() {
        let pos = resolver.to_push[i];
        if is_sticky(level.state(pos)) && !resolver.add_branching_blocks(pos) {
            return None;
        }
        i += 1;
    }
    Some(resolver.to_push)
}

struct Level<F> {
    get: F,
    min_y: i32,
    max_y: i32,
}

impl<F: Fn([i32; 3]) -> BlockState> Level<F> {
    fn state(&self, pos: [i32; 3]) -> BlockState {
        (self.get)(pos)
    }

    fn is_pushable(
        &self,
        state: BlockState,
        pos: [i32; 3],
        direction: Direction,
        allow_destroyable: bool,
        connection: Direction,
    ) -> bool {
        if pos[1] < self.min_y || pos[1] > self.max_y {
            return false;
        }
        let kind = BlockKind::from(state);
        if is_air(kind) {
            return true;
        }
        if matches!(
            kind,
            BlockKind::Obsidian
                | BlockKind::CryingObsidian
                | BlockKind::RespawnAnchor
                | BlockKind::ReinforcedDeepslate
        ) {
            return false;
        }
        if (direction == Direction::Down && pos[1] == self.min_y)
            || (direction == Direction::Up && pos[1] == self.max_y)
        {
            return false;
        }
        let block = Box::<dyn BlockTrait>::from(state);
        if is_piston(kind) {
            if block.get_property("extended") == Some("true") {
                return false;
            }
        } else {
            if block.behavior().destroy_time == -1.0 {
                return false;
            }
            match push_reaction(block.id()) {
                PushReaction::Block => return false,
                PushReaction::Destroy => return allow_destroyable,
                PushReaction::PushOnly => return direction == connection,
                PushReaction::Normal => {}
            }
        }
        !has_block_entity(block.id())
    }
}

struct Resolver<'a, F> {
    level: &'a Level<F>,
    piston: [i32; 3],
    push: Direction,
    to_push: Vec<[i32; 3]>,
}

impl<F: Fn([i32; 3]) -> BlockState> Resolver<'_, F> {
    fn add_block_line(&mut self, start: [i32; 3], direction: Direction) -> bool {
        let level = self.level;
        let mut next = level.state(start);
        if is_air(BlockKind::from(next))
            || !level.is_pushable(next, start, self.push, false, direction)
            || start == self.piston
            || self.to_push.contains(&start)
        {
            return true;
        }
        let back = self.push.opposite();
        let mut count = 1;
        if count + self.to_push.len() > MAX_PUSH_DEPTH {
            return false;
        }
        while is_sticky(next) {
            let pos = relative(start, back, count as i32);
            let previous = next;
            next = level.state(pos);
            if is_air(BlockKind::from(next))
                || !can_stick_to_each_other(previous, next)
                || !level.is_pushable(next, pos, self.push, false, back)
                || pos == self.piston
            {
                break;
            }
            count += 1;
            if count + self.to_push.len() > MAX_PUSH_DEPTH {
                return false;
            }
        }
        self.to_push
            .extend((0..count).rev().map(|i| relative(start, back, i as i32)));
        let mut added = count;
        let mut i = 1;
        loop {
            let pos = relative(start, self.push, i);
            if let Some(collision) = self.to_push.iter().position(|p| *p == pos) {
                self.to_push[collision..].rotate_right(added);
                for j in 0..=collision + added {
                    let at = self.to_push[j];
                    if is_sticky(level.state(at)) && !self.add_branching_blocks(at) {
                        return false;
                    }
                }
                return true;
            }
            next = level.state(pos);
            if is_air(BlockKind::from(next)) {
                return true;
            }
            if !level.is_pushable(next, pos, self.push, true, self.push) || pos == self.piston {
                return false;
            }
            if reaction(next) == PushReaction::Destroy {
                return true;
            }
            if self.to_push.len() >= MAX_PUSH_DEPTH {
                return false;
            }
            self.to_push.push(pos);
            added += 1;
            i += 1;
        }
    }

    fn add_branching_blocks(&mut self, from: [i32; 3]) -> bool {
        let from_state = self.level.state(from);
        for direction in crate::blocks::direction::Direction::ALL.map(Direction::from) {
            if direction == self.push || direction == self.push.opposite() {
                continue;
            }
            let neighbour = relative(from, direction, 1);
            if can_stick_to_each_other(self.level.state(neighbour), from_state)
                && !self.add_block_line(neighbour, direction)
            {
                return false;
            }
        }
        true
    }
}

fn reaction(state: BlockState) -> PushReaction {
    push_reaction(crate::blocks::block_name(state))
}

fn is_air(kind: BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::Air | BlockKind::CaveAir | BlockKind::VoidAir
    )
}

fn is_piston(kind: BlockKind) -> bool {
    matches!(kind, BlockKind::Piston | BlockKind::StickyPiston)
}

fn is_sticky(state: BlockState) -> bool {
    matches!(
        BlockKind::from(state),
        BlockKind::SlimeBlock | BlockKind::HoneyBlock
    )
}

fn can_stick_to_each_other(a: BlockState, b: BlockState) -> bool {
    match (BlockKind::from(a), BlockKind::from(b)) {
        (BlockKind::HoneyBlock, BlockKind::SlimeBlock)
        | (BlockKind::SlimeBlock, BlockKind::HoneyBlock) => false,
        _ => is_sticky(a) || is_sticky(b),
    }
}

fn piston_type(sticky: bool) -> &'static str {
    if sticky { "sticky" } else { "normal" }
}

fn relative(pos: [i32; 3], direction: Direction, n: i32) -> [i32; 3] {
    [
        pos[0] + direction.x() * n,
        pos[1] + direction.y() * n,
        pos[2] + direction.z() * n,
    ]
}

fn with(state: BlockState, props: &[(&str, &str)]) -> BlockState {
    let mut block = Box::<dyn BlockTrait>::from(state);
    for (name, value) in props {
        let _ = block.set_property(name, value);
    }
    block.as_block_state()
}

fn id(state: BlockState) -> u32 {
    u32::from(state)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    const PISTON: [i32; 3] = [0, 64, 0];

    fn block(kind: BlockKind) -> BlockState {
        BlockState::from(kind)
    }

    fn resolve_in(
        blocks: &[([i32; 3], BlockState)],
        direction: Direction,
        extending: bool,
    ) -> Option<Vec<[i32; 3]>> {
        let map: HashMap<[i32; 3], BlockState> = blocks.iter().copied().collect();
        let level = Level {
            get: |p: [i32; 3]| map.get(&p).copied().unwrap_or_default(),
            min_y: -64,
            max_y: 319,
        };
        resolve(&level, PISTON, direction, extending)
    }

    fn line(n: i32, kind: BlockKind) -> Vec<([i32; 3], BlockState)> {
        (1..=n).map(|i| ([i, 64, 0], block(kind))).collect()
    }

    #[test]
    fn a_line_is_pushed_nearest_first() {
        let pushed = resolve_in(&line(3, BlockKind::Stone), Direction::East, true);
        assert_eq!(pushed, Some(vec![[1, 64, 0], [2, 64, 0], [3, 64, 0]]));
    }

    #[test]
    fn twelve_blocks_push_and_thirteen_do_not() {
        let twelve = resolve_in(&line(12, BlockKind::Stone), Direction::East, true);
        assert_eq!(twelve.map(|v| v.len()), Some(12));
        assert_eq!(
            resolve_in(&line(13, BlockKind::Stone), Direction::East, true),
            None
        );
    }

    #[test]
    fn obsidian_and_block_entities_stop_the_push() {
        let mut blocks = line(2, BlockKind::Stone);
        blocks.push(([3, 64, 0], block(BlockKind::Obsidian)));
        assert_eq!(resolve_in(&blocks, Direction::East, true), None);
        blocks[2].1 = block(BlockKind::Chest);
        assert_eq!(resolve_in(&blocks, Direction::East, true), None);
    }

    #[test]
    fn a_destroyable_block_ends_the_line() {
        let mut blocks = line(2, BlockKind::Stone);
        blocks.push(([3, 64, 0], block(BlockKind::Torch)));
        let pushed = resolve_in(&blocks, Direction::East, true);
        assert_eq!(pushed, Some(vec![[1, 64, 0], [2, 64, 0]]));
        let lone = resolve_in(
            &[([1, 64, 0], block(BlockKind::Torch))],
            Direction::East,
            true,
        );
        assert_eq!(lone, Some(Vec::new()));
    }

    #[test]
    fn slime_drags_its_neighbours() {
        let blocks = [
            ([1, 64, 0], block(BlockKind::SlimeBlock)),
            ([1, 65, 0], block(BlockKind::Stone)),
            ([1, 64, 1], block(BlockKind::Stone)),
            ([1, 63, 0], block(BlockKind::Obsidian)),
        ];
        let pushed = resolve_in(&blocks, Direction::East, true).unwrap();
        assert_eq!(pushed.len(), 3);
        assert_eq!(pushed[0], [1, 64, 0]);
        assert!(pushed.contains(&[1, 65, 0]));
        assert!(pushed.contains(&[1, 64, 1]));
    }

    #[test]
    fn honey_does_not_stick_to_slime() {
        let blocks = [
            ([1, 64, 0], block(BlockKind::SlimeBlock)),
            ([1, 65, 0], block(BlockKind::HoneyBlock)),
        ];
        let pushed = resolve_in(&blocks, Direction::East, true);
        assert_eq!(pushed, Some(vec![[1, 64, 0]]));
    }

    #[test]
    fn a_sticky_retract_pulls_toward_the_piston() {
        let blocks = [([2, 64, 0], block(BlockKind::Stone))];
        let pulled = resolve_in(&blocks, Direction::East, false);
        assert_eq!(pulled, Some(vec![[2, 64, 0]]));
    }

    #[test]
    fn extending_moves_the_line_and_adds_the_head() {
        let map: HashMap<[i32; 3], BlockState> = line(2, BlockKind::Stone).into_iter().collect();
        let level = Level {
            get: |p: [i32; 3]| map.get(&p).copied().unwrap_or_default(),
            min_y: -64,
            max_y: 319,
        };
        let piston = with(block(BlockKind::Piston), &[("facing", "east")]);
        let out = trigger_event(&level, piston, PISTON, Direction::East, 0, 5);
        let positions: Vec<[i32; 3]> = out.iter().map(|(p, _)| *p).collect();
        assert_eq!(positions, vec![[3, 64, 0], [2, 64, 0], [1, 64, 0]]);
        assert!(matches!(out[2].1.look, Look::Head { .. }));
        assert!(
            out.iter()
                .all(|(_, m)| m.extending && m.direction == Direction::East)
        );
    }

    #[test]
    fn progress_takes_two_ticks_and_holds_at_one() {
        let mut progress = Progress::default();
        progress.tick();
        assert_eq!((progress.progress_o, progress.progress), (0.0, 0.5));
        assert_eq!(progress.at(0.5), 0.25);
        progress.tick();
        progress.tick();
        assert_eq!((progress.progress_o, progress.progress), (1.0, 1.0));
        assert_eq!(progress.at(2.0), 1.0);
    }

    #[test]
    fn offsets_start_behind_and_end_at_the_block() {
        let extending = Moving {
            direction: Direction::Up,
            extending: true,
            look: Look::None,
        };
        assert_eq!(extending.offset(0.0), [0.0, -1.0, 0.0]);
        assert_eq!(extending.offset(1.0), [0.0, 0.0, 0.0]);
        let retracting = Moving {
            extending: false,
            ..extending
        };
        assert_eq!(retracting.offset(0.0), [0.0, 1.0, 0.0]);
        assert_eq!(retracting.offset(0.75), [0.0, 0.25, 0.0]);
    }

    #[test]
    fn a_retracting_source_draws_its_head_and_extended_base() {
        let piston = with(block(BlockKind::StickyPiston), &[("facing", "west")]);
        let moving = Moving::new(piston, Direction::West, false, true);
        let Look::Retracting { long, short, base } = moving.look else {
            panic!("{:?}", moving.look);
        };
        let state = |id: u32| Box::<dyn BlockTrait>::from(BlockState::try_from(id).unwrap());
        assert_eq!(state(long).id(), "piston_head");
        assert_eq!(state(long).get_property("type"), Some("sticky"));
        assert_eq!(state(long).get_property("facing"), Some("west"));
        assert_eq!(state(short).get_property("short"), Some("true"));
        assert_eq!(state(base).get_property("extended"), Some("true"));
        let [head, base_draw] = moving.draws(0.75);
        assert_eq!(head.map(|d| d.0), Some(short));
        assert_eq!(base_draw, Some((base, [0.0; 3])));
    }
}
