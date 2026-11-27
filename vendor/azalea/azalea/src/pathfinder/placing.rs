use azalea_core::position::BlockPos;

pub trait PlaceRules: Send + Sync + std::fmt::Debug {
    fn place_cost(&self, pos: BlockPos) -> f32;
    fn break_multiplier(&self, _pos: BlockPos) -> f32 {
        1.0
    }
}
