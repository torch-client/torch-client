use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

use azalea_registry::builtin::EntityKind;

use super::registry::{Id, trigger_bot as setting};
use super::store;

static COOLDOWN: AtomicU32 = AtomicU32::new(0);

fn roll_delay(lo: f32, hi: f32) -> u32 {
    (super::roll(lo, hi) / super::MS_PER_TICK).round() as u32
}

pub fn should_attack(target: Option<EntityKind>, strength: f32, weapon: bool) -> bool {
    let s = store();
    if !s.enabled(Id::TriggerBot) {
        return false;
    }

    let left = COOLDOWN.load(Relaxed);
    if left > 0 {
        COOLDOWN.store(left - 1, Relaxed);
        return false;
    }

    if s.flag(Id::TriggerBot, setting::WAIT_FOR_COOLDOWN) && strength < 1.0 {
        return false;
    }
    if s.flag(Id::TriggerBot, setting::REQUIRE_WEAPON) && !weapon {
        return false;
    }
    let Some(kind) = target else {
        return false;
    };
    if !super::entities::kind_enabled(kind) {
        return false;
    }

    let (lo, hi) = s.range(Id::TriggerBot, setting::DELAY);
    COOLDOWN.store(roll_delay(lo, hi), Relaxed);
    true
}

pub fn enabled() -> bool {
    store().enabled(Id::TriggerBot)
}

pub fn is_weapon(item: &str) -> bool {
    item.ends_with("_sword") || item.ends_with("_axe") || item == "mace" || item == "trident"
}
