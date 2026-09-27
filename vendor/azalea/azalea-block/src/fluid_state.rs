use azalea_registry::builtin::BlockKind;

use crate::block_state::{BlockState, BlockStateIntegerRepr};

#[derive(Clone, Debug)]
pub struct FluidState {
    pub kind: FluidKind,
    pub amount: u8,

    pub falling: bool,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FluidKind {
    #[default]
    Empty,
    Water,
    Lava,
}
impl FluidState {
    pub fn new_source_block(kind: FluidKind, falling: bool) -> Self {
        Self {
            kind,
            amount: 8,
            falling,
        }
    }

    pub fn height(&self) -> f32 {
        self.amount as f32 / 9.
    }
    pub fn is_empty(&self) -> bool {
        self.amount == 0
    }

    pub fn affects_flow(&self, other: &FluidState) -> bool {
        other.amount == 0 || self.is_same_kind(other)
    }

    pub fn is_same_kind(&self, other: &FluidState) -> bool {
        (other.kind == self.kind) || (self.amount == 0 && other.amount == 0)
    }
}

impl Default for FluidState {
    fn default() -> Self {
        Self {
            kind: FluidKind::Empty,
            amount: 0,
            falling: false,
        }
    }
}

impl From<BlockState> for FluidState {
    fn from(state: BlockState) -> Self {

        if state
            .property::<crate::properties::Waterlogged>()
            .unwrap_or_default()
        {
            return Self {
                kind: FluidKind::Water,
                amount: 8,
                falling: false,
            };
        }

        let registry_block = BlockKind::from(state);
        match registry_block {
            BlockKind::Water => {
                let level = state
                    .property::<crate::properties::WaterLevel>()
                    .expect("water block should always have WaterLevel");
                return Self {
                    kind: FluidKind::Water,
                    amount: to_or_from_legacy_fluid_level(level as u8),
                    falling: false,
                };
            }
            BlockKind::Lava => {
                let level = state
                    .property::<crate::properties::LavaLevel>()
                    .expect("lava block should always have LavaLevel");
                return Self {
                    kind: FluidKind::Lava,
                    amount: to_or_from_legacy_fluid_level(level as u8),
                    falling: false,
                };
            }
            BlockKind::BubbleColumn => {
                return Self::new_source_block(FluidKind::Water, false);
            }
            _ => {}
        }

        Self::default()
    }
}

pub fn to_or_from_legacy_fluid_level(level: u8) -> u8 {
    8_u8.saturating_sub(level)
}

impl From<FluidState> for BlockState {
    fn from(state: FluidState) -> Self {
        match state.kind {
            FluidKind::Empty => BlockState::AIR,
            FluidKind::Water => BlockState::from(crate::blocks::Water {
                level: crate::properties::WaterLevel::from(state.amount as BlockStateIntegerRepr),
            }),
            FluidKind::Lava => BlockState::from(crate::blocks::Lava {
                level: crate::properties::LavaLevel::from(state.amount as BlockStateIntegerRepr),
            }),
        }
    }
}
