use std::sync::{Mutex, PoisonError};

use azalea_core::position::BlockPos;

use super::area::Area;
use crate::renderer::world_draw::{self, Node};

const OWNER: &str = "auto_mine";

const AREA: u32 = 0x0C33_CCFF;
const TARGET: u32 = 0x30FF_AA00;
const PREVIEW: u32 = 0x0CFF_FFFF;

#[derive(Clone, Copy, PartialEq)]
enum Key {
    Preview(Option<BlockPos>, Option<BlockPos>, Option<BlockPos>),
    Area(BlockPos, BlockPos, Option<BlockPos>),
}

static DRAWN: Mutex<Option<Key>> = Mutex::new(None);

fn cube(min: BlockPos, max: BlockPos) -> ([f64; 3], [f64; 3]) {
    (
        [f64::from(min.x), f64::from(min.y), f64::from(min.z)],
        [
            f64::from(max.x) + 1.0,
            f64::from(max.y) + 1.0,
            f64::from(max.z) + 1.0,
        ],
    )
}

fn boxed(a: BlockPos, b: BlockPos, argb: u32) -> Node {
    let lo = BlockPos::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z));
    let hi = BlockPos::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z));
    let (min, max) = cube(lo, hi);
    Node::boxed(min, max, argb, true)
}

fn publish(key: Key, nodes: impl FnOnce() -> Vec<Node>) {
    let mut drawn = DRAWN.lock().unwrap_or_else(PoisonError::into_inner);
    if *drawn == Some(key) {
        return;
    }
    *drawn = Some(key);
    world_draw::set(OWNER, nodes());
}

pub fn preview(a: Option<BlockPos>, b: Option<BlockPos>, cursor: Option<BlockPos>) {
    publish(Key::Preview(a, b, cursor), || {
        let mut nodes = Vec::with_capacity(3);
        for c in [a, b].into_iter().flatten() {
            nodes.push(boxed(c, c, TARGET));
        }
        match (a.or(b), a.and(b).or(cursor)) {
            (Some(from), Some(to)) => nodes.push(boxed(from, to, PREVIEW)),
            (None, Some(at)) => nodes.push(boxed(at, at, PREVIEW)),
            _ => {}
        }
        nodes
    });
}

pub fn area(area: &Area, target: Option<BlockPos>) {
    publish(Key::Area(area.min, area.max, target), || {
        let mut nodes = vec![boxed(area.min, area.max, AREA)];
        if let Some(t) = target {
            nodes.push(boxed(t, t, TARGET));
        }
        nodes
    });
}

pub fn clear() {
    let mut drawn = DRAWN.lock().unwrap_or_else(PoisonError::into_inner);
    if drawn.take().is_some() {
        world_draw::clear(OWNER);
    }
}
