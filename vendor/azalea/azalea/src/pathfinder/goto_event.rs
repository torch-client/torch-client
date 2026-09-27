use std::{sync::Arc, time::Duration};

use bevy_ecs::prelude::*;

use crate::pathfinder::{
    astar::PathfinderTimeout,
    goals::Goal,
    moves::{self, SuccessorsFn},
};

#[derive(Message)]
#[non_exhaustive]
pub struct GotoEvent {
    pub entity: Entity,
    pub goal: Arc<dyn Goal>,
    pub opts: PathfinderOpts,
}

impl GotoEvent {
    pub fn new(entity: Entity, goal: impl Goal + 'static, opts: PathfinderOpts) -> Self {
        Self {
            entity,
            goal: Arc::new(goal),
            opts,
        }
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct PathfinderOpts {
    pub(crate) successors_fn: SuccessorsFn,
    pub(crate) allow_mining: bool,
    pub(crate) retry_on_no_path: bool,
    pub(crate) min_timeout: PathfinderTimeout,
    pub(crate) max_timeout: PathfinderTimeout,
}

impl PathfinderOpts {
    pub const fn new() -> Self {
        Self {
            successors_fn: moves::default_move,
            allow_mining: true,
            retry_on_no_path: true,
            min_timeout: PathfinderTimeout::Time(Duration::from_secs(1)),
            max_timeout: PathfinderTimeout::Time(Duration::from_secs(5)),
        }
    }
    pub fn successors_fn(mut self, successors_fn: SuccessorsFn) -> Self {
        self.successors_fn = successors_fn;
        self
    }
    pub fn allow_mining(mut self, allow_mining: bool) -> Self {
        self.allow_mining = allow_mining;
        self
    }
    pub fn retry_on_no_path(mut self, retry_on_no_path: bool) -> Self {
        self.retry_on_no_path = retry_on_no_path;
        self
    }
    pub fn min_timeout(mut self, min_timeout: impl Into<PathfinderTimeout>) -> Self {
        self.min_timeout = min_timeout.into();
        self
    }
    pub fn max_timeout(mut self, max_timeout: impl Into<PathfinderTimeout>) -> Self {
        self.max_timeout = max_timeout.into();
        self
    }
}
impl Default for PathfinderOpts {
    fn default() -> Self {
        Self::new()
    }
}
