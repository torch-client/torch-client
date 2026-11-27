use std::sync::{Mutex, PoisonError};

use azalea_core::position::BlockPos;

use crate::renderer::world_draw::{self, Node};

const OWNER: &str = "farmer";

const HOME: u32 = 0x3033_FF66;
const HOME_BEAM: u32 = 0xFF33_FF66;
const TARGET: u32 = 0x30FF_AA00;
const LEFT: u32 = 0x18FF_EE55;

const BEAM_HEIGHT: f64 = 32.0;
const BEAM_WIDTH: f64 = 0.15;

pub(super) const SHOWN: usize = 64;

const LANE: u32 = 0x6033_FF66;
const LANE_WIDTH: f64 = 0.08;
const LANE_LIFT: f64 = 0.05;

#[derive(Clone, Copy, PartialEq)]
struct Key {
    home: BlockPos,
    target: Option<BlockPos>,
    to_go: u32,
    plan: u32,
}

static DRAWN: Mutex<Option<Key>> = Mutex::new(None);

fn cube(p: BlockPos, argb: u32) -> Node {
    let min = [f64::from(p.x), f64::from(p.y), f64::from(p.z)];
    Node::boxed(min, [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0], argb, true)
}

pub(super) fn show(
    home: BlockPos,
    target: Option<BlockPos>,
    to_go: u32,
    plan: u32,
    lanes: &[[BlockPos; 2]],
    left: impl FnOnce() -> Vec<BlockPos>,
) {
    let key = Key {
        home,
        target,
        to_go,
        plan,
    };
    let mut drawn = DRAWN.lock().unwrap_or_else(PoisonError::into_inner);
    if *drawn == Some(key) {
        return;
    }
    *drawn = Some(key);
    let left = left();
    let mut nodes = Vec::with_capacity(left.len() + lanes.len() + 3);
    nodes.push(cube(home, HOME));
    let floor = |p: BlockPos| {
        [
            f64::from(p.x) + 0.5,
            f64::from(p.y) + LANE_LIFT,
            f64::from(p.z) + 0.5,
        ]
    };
    nodes.extend(
        lanes
            .iter()
            .map(|[a, b]| Node::line(floor(*a), floor(*b), LANE_WIDTH, LANE, true)),
    );
    let top = [
        f64::from(home.x) + 0.5,
        f64::from(home.y) + 1.0,
        f64::from(home.z) + 0.5,
    ];
    nodes.push(Node::line(
        top,
        [top[0], top[1] + BEAM_HEIGHT, top[2]],
        BEAM_WIDTH,
        HOME_BEAM,
        true,
    ));
    if let Some(t) = target {
        nodes.push(cube(t, TARGET));
    }
    nodes.extend(
        left.into_iter()
            .filter(|p| Some(*p) != target)
            .take(SHOWN)
            .map(|p| cube(p, LEFT)),
    );
    world_draw::set(OWNER, nodes);
}

pub(super) fn clear() {
    let mut drawn = DRAWN.lock().unwrap_or_else(PoisonError::into_inner);
    if drawn.take().is_some() {
        world_draw::clear(OWNER);
    }
}
