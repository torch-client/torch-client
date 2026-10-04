use std::sync::atomic::{AtomicU64, Ordering};

counted! {
    #[derive(PartialEq, Eq, Debug)]
    pub enum Stat / STAT_COUNT {
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
}

static STATS: [AtomicU64; STAT_COUNT] = [const { AtomicU64::new(0) }; STAT_COUNT];

pub fn bump(stat: Stat) {
    add(stat, 1);
}

pub(super) fn bump_counted(stat: Stat) -> u64 {
    STATS[stat as usize].fetch_add(1, Ordering::Relaxed)
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
pub struct Counts([u64; STAT_COUNT]);

impl std::ops::Index<Stat> for Counts {
    type Output = u64;

    fn index(&self, stat: Stat) -> &u64 {
        &self.0[stat as usize]
    }
}

pub fn counts() -> Counts {
    Counts(std::array::from_fn(|i| STATS[i].load(Ordering::Relaxed)))
}
