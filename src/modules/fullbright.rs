use super::registry::{Id, fullbright as setting, options};
use super::store;

options! {
    pub enum FullbrightMode {
        Lighting = "Lighting",
        NightVision = "Night Vision",
    }
}

pub fn active() -> bool {
    let s = store();
    s.enabled(Id::Fullbright) && s.choice(setting::MODE) == FullbrightMode::Lighting
}

pub fn night_vision() -> bool {
    let s = store();
    s.enabled(Id::Fullbright) && s.choice(setting::MODE) == FullbrightMode::NightVision
}
