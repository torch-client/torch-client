use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Stat {
    ChunksReceived,
    ChunksDropped,
    ColumnsLit,
    LightJobsDropped,
    MeshJobs,
    SectionsMeshed,
    MeshPanics,
    MeshJobsSuperseded,
    SectionsUploaded,
    ViewRecentred,
    ChunkPacketsSeen,
    ChunkPacketsOutOfWindow,
    CacheCenterMoves,
}

const STAT_COUNT: usize = Stat::CacheCenterMoves as usize + 1;

static STATS: [AtomicU64; STAT_COUNT] = [const { AtomicU64::new(0) }; STAT_COUNT];

pub fn bump(stat: Stat) {
    add(stat, 1);
}

pub fn add(stat: Stat, n: u64) {
    STATS[stat as usize].fetch_add(n, Ordering::Relaxed);
}

pub fn get(stat: Stat) -> u64 {
    STATS[stat as usize].load(Ordering::Relaxed)
}

pub(super) fn reset() {
    for stat in STATS.iter() {
        stat.store(0, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub chunks_received: u64,
    pub chunks_dropped: u64,
    pub columns_lit: u64,
    pub light_jobs_dropped: u64,
    pub mesh_jobs: u64,
    pub sections_meshed: u64,
    pub mesh_panics: u64,
    pub mesh_jobs_superseded: u64,
    pub sections_uploaded: u64,
    pub view_recentred: u64,
    pub chunk_packets_seen: u64,
    pub chunk_packets_out_of_window: u64,
    pub cache_center_moves: u64,
}

pub fn counts() -> Counts {
    Counts {
        chunks_received: get(Stat::ChunksReceived),
        chunks_dropped: get(Stat::ChunksDropped),
        columns_lit: get(Stat::ColumnsLit),
        light_jobs_dropped: get(Stat::LightJobsDropped),
        mesh_jobs: get(Stat::MeshJobs),
        sections_meshed: get(Stat::SectionsMeshed),
        mesh_panics: get(Stat::MeshPanics),
        mesh_jobs_superseded: get(Stat::MeshJobsSuperseded),
        sections_uploaded: get(Stat::SectionsUploaded),
        view_recentred: get(Stat::ViewRecentred),
        chunk_packets_seen: get(Stat::ChunkPacketsSeen),
        chunk_packets_out_of_window: get(Stat::ChunkPacketsOutOfWindow),
        cache_center_moves: get(Stat::CacheCenterMoves),
    }
}
