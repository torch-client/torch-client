use super::registry::Id;
use super::store;
use crate::session::{MOVE_DESCEND, MOVE_SNEAK};

pub fn enabled() -> bool {
    store().enabled(Id::Sneak)
}

pub fn flags(flying: bool) -> u8 {
    MOVE_SNEAK | if flying { MOVE_DESCEND } else { 0 }
}
