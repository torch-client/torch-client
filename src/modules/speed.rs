use super::registry::{Id, speed as setting};
use super::store;

pub fn enabled() -> bool {
    store().enabled(Id::Speed)
}

pub fn per_tick(sneaking: bool, in_fluid: bool, on_ground: bool) -> Option<f64> {
    let s = store();
    if sneaking
        || (in_fluid && !s.flag(setting::IN_LIQUIDS))
        || (!on_ground && s.flag(setting::ONLY_ON_GROUND))
    {
        return None;
    }
    Some(f64::from(s.num(setting::SPEED)) / 20.0)
}
