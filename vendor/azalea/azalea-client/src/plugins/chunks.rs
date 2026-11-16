use bevy_platform::time::Instant;
use std::time::Duration;

use azalea_core::position::ChunkPos;
use azalea_protocol::packets::game::c_level_chunk_with_light::ClientboundLevelChunkWithLight;
use azalea_world::Chunk;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use tracing::trace;

use crate::local_player::WorldHolder;

pub struct ChunksPlugin;
impl Plugin for ChunksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ReceiveChunkEvent>()
        .add_message::<ChunkBatchStartEvent>()
        .add_message::<ChunkBatchFinishedEvent>();
    }
}

#[derive(Message)]
pub struct ReceiveChunkEvent {
    pub entity: Entity,
    pub pos: ChunkPos,
}

#[derive(Clone, Component, Debug)]
pub struct ChunkBatchInfo {
    pub start_time: Instant,
    pub aggregated_duration_per_chunk: Duration,
    pub old_samples_weight: u32,
}

#[derive(Message)]
pub struct ChunkBatchStartEvent {
    pub entity: Entity,
}
#[derive(Message)]
pub struct ChunkBatchFinishedEvent {
    pub entity: Entity,
    pub batch_size: u32,
}

pub fn store_chunk(holder: &WorldHolder, packet: &ClientboundLevelChunkWithLight) {
    let pos = ChunkPos::new(packet.x, packet.z);
    let (height, min_y, biomes) = {
        let world = holder.shared.read();
        (
            world.chunks.height(),
            world.chunks.min_y(),
            azalea_world::chunk::biome_registry_len(&world.registries),
        )
    };
    let (chunk, failure) = Chunk::read_lenient(
        &packet.chunk_data.data,
        height,
        min_y,
        &packet.chunk_data.heightmaps,
        biomes,
    );
    {
        let mut world = holder.shared.write();
        if world.chunks.height() != height || world.chunks.min_y() != min_y {
            trace!("Dropping chunk {pos:?}: the level changed while it was decoded");
            return;
        }
        let mut partial = holder.partial.write();
        partial.chunks.set(&pos, Some(chunk), &mut world.chunks);
    }
    trace!("Stored chunk {pos:?}");
    if let Some(failure) = failure {
        azalea_world::chunk::report_decode_failure(pos, &failure);
    }
}

impl ChunkBatchInfo {
    pub fn batch_finished(&mut self, batch_size: u32) {
        if batch_size == 0 {
            return;
        }
        let batch_duration = self.start_time.elapsed();
        let duration_per_chunk = batch_duration / batch_size;
        let clamped_duration = Duration::clamp(
            duration_per_chunk,
            self.aggregated_duration_per_chunk / 3,
            self.aggregated_duration_per_chunk * 3,
        );
        self.aggregated_duration_per_chunk =
            ((self.aggregated_duration_per_chunk * self.old_samples_weight) + clamped_duration)
                / (self.old_samples_weight + 1);
        self.old_samples_weight = u32::min(49, self.old_samples_weight + 1);
    }

    pub fn desired_chunks_per_tick(&self) -> f32 {
        let measured = (7000000. / self.aggregated_duration_per_chunk.as_nanos() as f64) as f32;
        measured.max(min_chunks_per_tick())
    }
}

fn min_chunks_per_tick() -> f32 {
    static MIN: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *MIN.get_or_init(|| {
        std::env::var("MC_CHUNK_RATE_MIN")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v >= 0.0)
            .unwrap_or(1.0)
    })
}

pub fn start_chunk_batch(info: &mut ChunkBatchInfo) {
    info.start_time = Instant::now();
}

pub fn finish_chunk_batch(info: &mut ChunkBatchInfo, batch_size: u32) -> f32 {
    info.batch_finished(batch_size);
    let desired_chunks_per_tick = info.desired_chunks_per_tick();
    tracing::debug!(
        "chunk batch: {batch_size} chunks, {:?} per chunk, asking for \
         {desired_chunks_per_tick:.3} chunks/tick",
        info.aggregated_duration_per_chunk,
    );
    desired_chunks_per_tick
}

#[derive(Clone, Debug)]
pub struct ChunkReceiveSpeedAccumulator {
    batch_sizes: Vec<u32>,
    batch_durations: Vec<u32>,
    index: usize,
    filled_size: usize,
}
impl ChunkReceiveSpeedAccumulator {
    pub fn new(capacity: usize) -> Self {
        Self {
            batch_sizes: vec![0; capacity],
            batch_durations: vec![0; capacity],
            index: 0,
            filled_size: 0,
        }
    }

    pub fn accumulate(&mut self, batch_size: u32, batch_duration: Duration) {
        self.batch_sizes[self.index] = batch_size;
        self.batch_durations[self.index] =
            f32::clamp(batch_duration.as_millis() as f32, 0., 15000.) as u32;
        self.index = (self.index + 1) % self.batch_sizes.len();
        if self.filled_size < self.batch_sizes.len() {
            self.filled_size += 1;
        }
    }

    pub fn get_millis_per_chunk(&self) -> f64 {
        let mut total_batch_size = 0;
        let mut total_batch_duration = 0;
        for i in 0..self.filled_size {
            total_batch_size += self.batch_sizes[i];
            total_batch_duration += self.batch_durations[i];
        }
        if total_batch_size == 0 {
            return 0.;
        }
        total_batch_duration as f64 / total_batch_size as f64
    }
}

impl Default for ChunkBatchInfo {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            aggregated_duration_per_chunk: Duration::from_millis(2),
            old_samples_weight: 1,
        }
    }
}
