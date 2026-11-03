use azalea_core::sound::CustomSound;
use azalea_inventory::ItemStackData;
use azalea_inventory::components::{Consumable, ItemUseAnimation};
use azalea_registry::Holder;
use azalea_registry::builtin::SoundEvent;

const EFFECTS_INTERVAL: i32 = 4;
const EFFECTS_START_FRACTION: f32 = 0.21875;

#[derive(Clone, Debug, PartialEq)]
pub struct Consumption {
    pub ticks: i32,
    pub drink: bool,
    pub particles: bool,
    #[cfg_attr(not(feature = "audio"), allow(dead_code))]
    pub sound: Holder<SoundEvent, CustomSound>,
}

impl Consumption {
    pub fn of(data: &ItemStackData) -> Option<Consumption> {
        let consumable = data.get_component::<Consumable>()?;
        Some(Consumption {
            ticks: (consumable.consume_seconds * 20.0) as i32,
            drink: matches!(consumable.animation, ItemUseAnimation::Drink),
            particles: consumable.has_consume_particles,
            sound: consumable.sound.clone(),
        })
    }

    pub fn should_emit(&self, remaining: i32) -> bool {
        let used = self.ticks - remaining;
        let wait = (self.ticks as f32 * EFFECTS_START_FRACTION) as i32;
        used > wait && remaining % EFFECTS_INTERVAL == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lasting(ticks: i32) -> Consumption {
        Consumption {
            ticks,
            drink: false,
            particles: true,
            sound: Holder::Reference(SoundEvent::EntityGenericEat),
        }
    }

    fn emitting(ticks: i32) -> Vec<i32> {
        (-8..=ticks)
            .rev()
            .filter(|&r| lasting(ticks).should_emit(r))
            .collect()
    }

    #[test]
    fn effects_start_after_the_wait_and_run_on_past_zero() {
        assert_eq!(emitting(32), [24, 20, 16, 12, 8, 4, 0, -4, -8]);
        assert_eq!(emitting(16), [12, 8, 4, 0, -4, -8]);
    }
}
