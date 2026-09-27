use azalea_registry::builtin::ItemKind;

use crate::{components::MaxStackSize, default_components::get_default_component};

pub mod consume_effect;

pub trait MaxStackSizeExt {
    fn max_stack_size(&self) -> i32;

    fn stackable(&self) -> bool {
        self.max_stack_size() > 1
    }
}

impl MaxStackSizeExt for ItemKind {
    fn max_stack_size(&self) -> i32 {
        get_default_component::<MaxStackSize>(*self).map_or(64, |s| s.count)
    }
}
