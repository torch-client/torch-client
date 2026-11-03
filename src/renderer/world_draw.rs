use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Box {
        min: [f64; 3],
        max: [f64; 3],
    },
    Line {
        from: [f64; 3],
        to: [f64; 3],
        width: f64,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub shape: Shape,
    pub argb: u32,
    pub outline: bool,
    pub through: bool,
}

impl Node {
    pub fn boxed(min: [f64; 3], max: [f64; 3], argb: u32, through: bool) -> Node {
        Node {
            shape: Shape::Box { min, max },
            argb,
            outline: true,
            through,
        }
    }

    pub fn line(from: [f64; 3], to: [f64; 3], width: f64, argb: u32, through: bool) -> Node {
        Node {
            shape: Shape::Line { from, to, width },
            argb,
            outline: false,
            through,
        }
    }
}

struct Scene {
    owners: BTreeMap<&'static str, Arc<Vec<Node>>>,
    merged: Option<Arc<Vec<Node>>>,
}

static SCENE: Mutex<Scene> = Mutex::new(Scene {
    owners: BTreeMap::new(),
    merged: None,
});

fn lock() -> MutexGuard<'static, Scene> {
    SCENE.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn set(owner: &'static str, nodes: Vec<Node>) {
    let mut s = lock();
    if nodes.is_empty() {
        if s.owners.remove(owner).is_none() {
            return;
        }
    } else {
        s.owners.insert(owner, Arc::new(nodes));
    }
    s.merged = match s.owners.len() {
        0 => None,
        1 => s.owners.values().next().cloned(),
        _ => Some(Arc::new(
            s.owners.values().flat_map(|n| n.iter().copied()).collect(),
        )),
    };
}

pub fn clear(owner: &'static str) {
    set(owner, Vec::new());
}

pub fn scene() -> Option<Arc<Vec<Node>>> {
    lock().merged.clone()
}
