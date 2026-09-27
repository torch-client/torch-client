use std::sync::OnceLock;

use azalea_block::BlockState;
use azalea_core::position::BlockPos;

pub type BlockChangeHook = Box<dyn Fn(BlockPos, BlockState, BlockState) + Send + Sync>;

static HOOK: OnceLock<BlockChangeHook> = OnceLock::new();

pub fn set_block_change_hook(hook: BlockChangeHook) -> Result<(), BlockChangeHook> {
    HOOK.set(hook)
}

pub(crate) fn notify(pos: BlockPos, old: BlockState, new: BlockState) {
    if old == new {
        return;
    }
    if let Some(hook) = HOOK.get() {
        hook(pos, old, new);
    }
}
