use std::marker::PhantomData;

use bevy_app::{App, Last, Plugin};
use bevy_ecs::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy_tasks::tick_global_task_pools_on_main_thread;
use bevy_tasks::{
    AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool, TaskPoolBuilder,
};

#[derive(Default)]
pub struct TaskPoolPlugin {
    pub task_pool_options: TaskPoolOptions,
}

impl Plugin for TaskPoolPlugin {
    fn build(&self, app: &mut App) {
        self.task_pool_options.create_default_pools();

        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Last, tick_global_task_pools);
    }
}

pub struct NonSendMarker(PhantomData<*mut ()>);
#[cfg(not(target_arch = "wasm32"))]
fn tick_global_task_pools(_main_thread_marker: Option<NonSend<NonSendMarker>>) {
    tick_global_task_pools_on_main_thread();
}

#[derive(Clone, Resource)]
pub struct TaskPoolOptions {
    pub min_total_threads: usize,
    pub max_total_threads: usize,

    pub io: TaskPoolThreadAssignmentPolicy,
    pub async_compute: TaskPoolThreadAssignmentPolicy,
    pub compute: TaskPoolThreadAssignmentPolicy,
}

impl Default for TaskPoolOptions {
    fn default() -> Self {
        TaskPoolOptions {
            min_total_threads: 1,
            max_total_threads: usize::MAX,

            io: TaskPoolThreadAssignmentPolicy {
                min_threads: 1,
                max_threads: 4,
                percent: 0.25,
            },

            async_compute: TaskPoolThreadAssignmentPolicy {
                min_threads: 1,
                max_threads: 4,
                percent: 0.25,
            },

            compute: TaskPoolThreadAssignmentPolicy {
                min_threads: 1,
                max_threads: usize::MAX,
                percent: 1.0,
            },
        }
    }
}

impl TaskPoolOptions {
    pub fn create_default_pools(&self) {
        let total_threads = bevy_tasks::available_parallelism()
            .clamp(self.min_total_threads, self.max_total_threads);

        let mut remaining_threads = total_threads;

        {
            let io_threads = self
                .io
                .get_number_of_threads(remaining_threads, total_threads);

            remaining_threads = remaining_threads.saturating_sub(io_threads);

            IoTaskPool::get_or_init(|| {
                TaskPoolBuilder::default()
                    .num_threads(io_threads)
                    .thread_name("IO Task Pool".to_owned())
                    .build()
            });
        }

        {
            let async_compute_threads = self
                .async_compute
                .get_number_of_threads(remaining_threads, total_threads);

            remaining_threads = remaining_threads.saturating_sub(async_compute_threads);

            AsyncComputeTaskPool::get_or_init(|| {
                TaskPoolBuilder::default()
                    .num_threads(async_compute_threads)
                    .thread_name("Async Compute Task Pool".to_owned())
                    .build()
            });
        }

        {
            let compute_threads = self
                .compute
                .get_number_of_threads(remaining_threads, total_threads);

            ComputeTaskPool::get_or_init(|| {
                TaskPoolBuilder::default()
                    .num_threads(compute_threads)
                    .thread_name("Compute Task Pool".to_owned())
                    .build()
            });
        }
    }
}

#[derive(Clone)]
pub struct TaskPoolThreadAssignmentPolicy {
    pub min_threads: usize,
    pub max_threads: usize,
    pub percent: f32,
}

impl TaskPoolThreadAssignmentPolicy {
    fn get_number_of_threads(&self, remaining_threads: usize, total_threads: usize) -> usize {
        assert!(self.percent >= 0.0);
        let mut desired = (total_threads as f32 * self.percent).round() as usize;

        desired = desired.min(remaining_threads);

        desired.clamp(self.min_threads, self.max_threads)
    }
}
