use azalea::movement::KnockbackData;

use super::registry::{Id, velocity as setting};
use super::store;

pub fn enabled() -> bool {
    store().enabled(Id::Velocity)
}

pub fn keeps(data: &KnockbackData) -> bool {
    let s = store();
    if !s.enabled(Id::Velocity) {
        return true;
    }
    !s.flag(match data {
        KnockbackData::Set(_) => setting::KNOCKBACK,
        KnockbackData::Add(_) => setting::EXPLOSIONS,
    })
}

pub fn no_web() -> bool {
    let s = store();
    s.enabled(Id::Velocity) && s.flag(setting::NO_WEB)
}

pub fn no_water_drag() -> bool {
    let s = store();
    s.enabled(Id::Velocity) && s.flag(setting::NO_WATER_DRAG)
}
