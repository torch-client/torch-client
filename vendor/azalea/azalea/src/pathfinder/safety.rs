#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveSafety {
    pub avoid_hazards: bool,
    pub max_fall: u32,
}

pub const SAFE_FALL_DISTANCE: u32 = 3;

pub const WATER_FALL_CAP: u32 = 64;

pub const SUBMERGED_MAX_DEPTH: u32 = 5;

impl MoveSafety {
    pub const fn new() -> Self {
        Self {
            avoid_hazards: true,
            max_fall: SAFE_FALL_DISTANCE,
        }
    }
}

impl Default for MoveSafety {
    fn default() -> Self {
        Self::new()
    }
}
