use azalea_core::sound::CustomSound;
use azalea_inventory::ItemStackData;
use azalea_inventory::components::{
    KineticWeapon, KineticWeaponCondition, MinimumAttackCharge, PiercingWeapon, SwingAnimation,
};
use azalea_registry::Holder;
use azalea_registry::builtin::SoundEvent;

use super::component;
use crate::util::mth::ease::{
    in_circ, in_out_elastic, in_out_sine, out_circ, out_cubic, out_quart,
};
use crate::util::mth::{DEG_TO_RAD, progress};

pub const HIT_FEEDBACK_TICKS: u32 = 10;

pub const CROSSBOW_CHARGE_TICKS: f32 = 25.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kinetic {
    pub delay_ticks: i32,
    pub dismount_ticks: i32,
    pub knockback_ticks: i32,
    pub damage_ticks: i32,
    pub forward_movement: f32,
    pub use_sound: Option<SoundEvent>,
    pub hit_sound: Option<SoundEvent>,
}

impl Kinetic {
    fn from_component(k: &KineticWeapon) -> Kinetic {
        let ticks =
            |c: &Option<KineticWeaponCondition>| c.as_ref().map_or(0, |c| c.max_duration_ticks);
        Kinetic {
            delay_ticks: k.delay_ticks,
            dismount_ticks: ticks(&k.dismount_conditions),
            knockback_ticks: ticks(&k.knockback_conditions),
            damage_ticks: ticks(&k.damage_conditions),
            forward_movement: k.forward_movement,
            use_sound: k.sound.as_ref().and_then(builtin),
            hit_sound: k.hit_sound.as_ref().and_then(builtin),
        }
    }

    pub fn use_params(&self, time: f32) -> UseParams {
        let finish_raising = self.delay_ticks as f32;
        let finish_swaying = (self.dismount_ticks + self.delay_ticks) as f32;
        let start_swaying = finish_swaying - 20.0;
        let finish_lowering = (self.knockback_ticks + self.delay_ticks) as f32;
        let start_lowering = finish_lowering - 40.0;
        let finish_raising_back = (self.damage_ticks + self.delay_ticks) as f32;

        let raise = progress(time, 0.0, finish_raising);
        let sway = progress(time, start_swaying, finish_swaying);
        let raise_back = progress(time, finish_raising_back - 5.0, finish_raising_back);
        let sway_intensity = 2.0 * out_circ(sway) - 2.0 * in_circ(raise_back);
        UseParams {
            raise,
            raise_start: progress(raise, 0.0, 0.5),
            raise_middle: progress(raise, 0.5, 0.8),
            raise_end: progress(raise, 0.8, 1.0),
            sway,
            lower: out_cubic(in_out_elastic(progress(
                time - 20.0,
                start_lowering,
                finish_lowering,
            ))),
            raise_back,
            sway_intensity,
            sway_slow: (time * 19.0 * DEG_TO_RAD).sin() * sway_intensity,
            sway_fast: (time * 30.0 * DEG_TO_RAD).sin() * sway_intensity,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UseParams {
    pub raise: f32,
    pub raise_start: f32,
    pub raise_middle: f32,
    pub raise_end: f32,
    pub sway: f32,
    pub lower: f32,
    pub raise_back: f32,
    pub sway_intensity: f32,
    pub sway_slow: f32,
    pub sway_fast: f32,
}

pub fn hit_feedback_amount(ticks_since: f32) -> f32 {
    0.4 * (out_quart(progress(ticks_since, 1.0, 3.0))
        - in_out_sine(progress(ticks_since, 3.0, 10.0)))
}

pub fn kinetic(data: &ItemStackData) -> Option<Kinetic> {
    component::<KineticWeapon>(data).map(|k| Kinetic::from_component(&k))
}

pub fn piercing(data: &ItemStackData) -> Option<Option<SoundEvent>> {
    component::<PiercingWeapon>(data).map(|p| p.sound.as_ref().and_then(builtin))
}

pub fn minimum_attack_charge(data: &ItemStackData) -> f32 {
    component::<MinimumAttackCharge>(data).map_or(0.0, |m| m.value)
}

pub fn swing_ticks(data: &ItemStackData) -> u8 {
    component::<SwingAnimation>(data).map_or(0, |s| s.duration.clamp(0, u8::MAX as i32) as u8)
}

fn builtin(holder: &Holder<SoundEvent, CustomSound>) -> Option<SoundEvent> {
    match holder {
        Holder::Reference(event) => Some(*event),
        Holder::Direct(_) => None,
    }
}

pub fn sound_name(event: SoundEvent) -> &'static str {
    event.to_str().trim_start_matches("minecraft:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use azalea_inventory::ItemStack;
    use azalea_registry::builtin::ItemKind;

    #[test]
    fn iron_spear_defaults_reach_the_animation() {
        let stack = ItemStack::new(ItemKind::IronSpear, 1);
        let data = stack.as_present().unwrap();
        let k = kinetic(data).expect("spears carry kinetic_weapon by default");
        assert_eq!(k.delay_ticks, 12);
        assert_eq!(
            (k.dismount_ticks, k.knockback_ticks, k.damage_ticks),
            (50, 135, 225)
        );
        assert_eq!(k.use_sound.map(sound_name), Some("item.spear.use"));
        assert_eq!(k.hit_sound.map(sound_name), Some("item.spear.hit"));
        assert_eq!(
            piercing(data).flatten().map(sound_name),
            Some("item.spear.attack")
        );
        assert_eq!(minimum_attack_charge(data), 1.0);
        assert_eq!(swing_ticks(data), 19);
    }

    #[test]
    fn a_sword_is_neither() {
        let stack = ItemStack::new(ItemKind::IronSword, 1);
        let data = stack.as_present().unwrap();
        assert!(kinetic(data).is_none());
        assert!(piercing(data).is_none());
        assert_eq!(minimum_attack_charge(data), 0.0);
        assert_eq!(swing_ticks(data), 6);
    }

    #[test]
    fn use_params_phase_in_order() {
        let k = Kinetic {
            delay_ticks: 12,
            dismount_ticks: 50,
            knockback_ticks: 135,
            damage_ticks: 225,
            forward_movement: 0.38,
            ..Kinetic::default()
        };
        let p = k.use_params(6.0);
        assert!((p.raise - 0.5).abs() < 1e-6);
        assert_eq!(p.sway, 0.0);
        let p = k.use_params(62.0);
        assert_eq!(p.raise, 1.0);
        assert_eq!(p.sway, 1.0);
        assert!(p.sway_intensity > 1.9);
        let p = k.use_params(237.0);
        assert_eq!(p.raise_back, 1.0);
        assert!(p.sway_intensity.abs() < 1e-5);
    }

    #[test]
    fn hit_feedback_rises_and_settles() {
        assert_eq!(hit_feedback_amount(0.0), 0.0);
        assert!((hit_feedback_amount(3.0) - 0.4).abs() < 1e-6);
        assert!(hit_feedback_amount(10.0).abs() < 1e-6);
    }
}
