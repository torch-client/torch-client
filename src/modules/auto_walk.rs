use super::registry::{Id, auto_walk as setting};
use super::store;
use crate::session::{MOVE_FORWARD, MOVE_SPRINT, SharedState};

pub fn enabled() -> bool {
    store().enabled(Id::AutoWalk)
}

pub fn flags(s: &SharedState) -> u8 {
    let st = store();
    if s.screen_open && st.flag(setting::PAUSE_ON_SCREEN) {
        return 0;
    }
    MOVE_FORWARD
        | if st.flag(setting::SPRINT) {
            MOVE_SPRINT
        } else {
            0
        }
}

pub fn auto_jump() -> bool {
    enabled() && store().flag(setting::AUTO_JUMP)
}
