use std::cmp::Ordering;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

use azalea::entity::{ActiveEffects, EffectGeneration, attribute_modifier_for_effect};
use azalea::registry::builtin::MobEffect as AzMobEffect;

use crate::gui::tooltip::translate;
use crate::util::mth::lerp;

#[derive(Clone, Debug, PartialEq)]
pub struct MobEffectInstance {
    pub id: String,
    pub amplifier: i32,
    pub duration: i32,
    pub ambient: bool,
    pub show_icon: bool,
    pub blend: [f32; 2],
}

impl MobEffectInstance {
    pub fn is_infinite(&self) -> bool {
        self.duration == -1
    }

    pub fn blend_factor(&self, partial: f32) -> f32 {
        lerp(partial, self.blend[0], self.blend[1])
    }
}

pub fn ends_within(duration: i32, ticks: i32) -> bool {
    duration != -1 && duration <= ticks
}

pub fn find<'a>(effects: &'a [MobEffectInstance], id: &str) -> Option<&'a MobEffectInstance> {
    effects.iter().find(|e| e.id == id)
}

pub fn has(effects: &[MobEffectInstance], id: &str) -> bool {
    find(effects, id).is_some()
}

pub fn sorted(effects: &[MobEffectInstance]) -> Vec<&MobEffectInstance> {
    let mut sorted: Vec<&MobEffectInstance> = effects.iter().collect();
    sorted.sort_by(|a, b| compare(a, b));
    sorted
}

pub fn sprite(id: &str) -> String {
    format!("mob_effect/{id}")
}

pub fn effect_blend_factor(effects: &[MobEffectInstance], id: &str, partial: f32) -> f32 {
    find(effects, id).map_or(0.0, |e| e.blend_factor(partial))
}

pub fn night_vision_scale(duration: i32, partial: f32) -> f32 {
    if ends_within(duration, 200) {
        0.7 + ((duration as f32 - partial) * std::f32::consts::PI * 0.2).sin() * 0.3
    } else {
        1.0
    }
}

#[derive(Clone, Copy, Default)]
struct BlendState {
    previous: f32,
    factor: f32,
}

impl BlendState {
    fn has_effect(id: &str, duration: i32) -> bool {
        let [_, _, advance] = effect_meta(id).blend;
        !ends_within(duration, advance)
    }

    fn immediate(id: &str, duration: i32) -> BlendState {
        let factor = if Self::has_effect(id, duration) {
            1.0
        } else {
            0.0
        };
        BlendState {
            previous: factor,
            factor,
        }
    }

    fn tick(&mut self, id: &str, duration: i32) {
        self.previous = self.factor;
        let has_effect = Self::has_effect(id, duration);
        let target = if has_effect { 1.0 } else { 0.0 };
        if self.factor != target {
            let [blend_in, blend_out, _] = effect_meta(id).blend;
            let ticks = if has_effect { blend_in } else { blend_out };
            if ticks == 0 {
                self.factor = target;
            } else {
                let max_delta = 1.0 / ticks as f32;
                self.factor += (target - self.factor).clamp(-max_delta, max_delta);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Beneficial,
    Harmful,
    Neutral,
}

struct EffectMeta {
    category: Category,
    color: u32,
    blend: [i32; 3],
}

fn effect_meta(id: &str) -> EffectMeta {
    use self::Category::{Beneficial as B, Harmful as H, Neutral as N};
    let (category, color) = match id {
        "speed" => (B, 3402751),
        "slowness" => (H, 9154528),
        "haste" => (B, 14270531),
        "mining_fatigue" => (H, 4866583),
        "strength" => (B, 16762624),
        "instant_health" => (B, 16262179),
        "instant_damage" => (H, 11101546),
        "jump_boost" => (B, 16646020),
        "nausea" => (H, 5578058),
        "regeneration" => (B, 13458603),
        "resistance" => (B, 9520880),
        "fire_resistance" => (B, 16750848),
        "water_breathing" => (B, 10017472),
        "invisibility" => (B, 16185078),
        "blindness" => (H, 2039587),
        "night_vision" => (B, 12779366),
        "hunger" => (H, 5797459),
        "weakness" => (H, 4738376),
        "poison" => (H, 8889187),
        "wither" => (H, 7561558),
        "health_boost" => (B, 16284963),
        "absorption" => (B, 2445989),
        "saturation" => (B, 16262179),
        "glowing" => (N, 9740385),
        "levitation" => (H, 13565951),
        "luck" => (B, 5882118),
        "unluck" => (H, 12624973),
        "slow_falling" => (B, 15978425),
        "conduit_power" => (B, 1950417),
        "dolphins_grace" => (B, 8954814),
        "bad_omen" => (N, 745784),
        "hero_of_the_village" => (B, 4521796),
        "darkness" => (H, 2696993),
        "trial_omen" => (N, 1484454),
        "raid_omen" => (N, 14565464),
        "wind_charged" => (H, 12438015),
        "weaving" => (H, 7891290),
        "oozing" => (H, 10092451),
        "infested" => (H, 9214860),
        "breath_of_the_nautilus" => (B, 65518),
        _ => (H, 0xFFFFFF),
    };
    let blend = match id {
        "nausea" => [150, 20, 60],
        "darkness" => [22; 3],
        _ => [0; 3],
    };
    EffectMeta {
        category,
        color,
        blend,
    }
}

pub fn is_beneficial(id: &str) -> bool {
    effect_meta(id).category == Category::Beneficial
}

pub fn category(id: &str) -> Category {
    effect_meta(id).category
}

pub fn color(id: &str) -> u32 {
    effect_meta(id).color
}

pub fn attribute_modifier(id: &str, amplifier: i32) -> Option<(String, f64, u8)> {
    let (attribute, template) = attribute_modifier_for_effect(AzMobEffect::from_str(id).ok()?)?;
    let modifier = template.create(amplifier);
    let name = attribute.to_str().trim_start_matches("minecraft:");
    Some((
        format!("attribute.name.{name}"),
        modifier.amount,
        modifier.operation as u8,
    ))
}

pub fn display_name(id: &str, amplifier: i32) -> String {
    let name = translate(&format!("effect.minecraft.{id}"), &[]);
    if (1..=9).contains(&amplifier) {
        format!(
            "{name} {}",
            translate(&format!("enchantment.level.{}", amplifier + 1), &[])
        )
    } else {
        name
    }
}

pub fn format_duration(duration_ticks: i32, scale: f32, tick_rate: f32) -> String {
    if duration_ticks < 0 {
        return translate("effect.duration.infinite", &[]);
    }
    let ticks = (duration_ticks as f32 * scale).floor() as i32;
    let total_seconds = (ticks as f32 / tick_rate).floor() as i32;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let hours = minutes / 60;
    let minutes = minutes % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub fn swing_adjust(active: &ActiveEffects) -> i32 {
    match active.get_dig_speed_amplifier() {
        Some(amplifier) => -(1 + amplifier),
        None => active
            .get_level(AzMobEffect::MiningFatigue)
            .map_or(0, |amplifier| (1 + amplifier) * 2),
    }
}

pub fn compare(a: &MobEffectInstance, b: &MobEffectInstance) -> Ordering {
    let short = a.duration <= 32147 || b.duration <= 32147;
    let not_both_ambient = !a.ambient || !b.ambient;
    let color = |i: &MobEffectInstance| effect_meta(&i.id).color;
    if short && not_both_ambient {
        a.ambient
            .cmp(&b.ambient)
            .then(a.is_infinite().cmp(&b.is_infinite()))
            .then(a.duration.cmp(&b.duration))
            .then(color(a).cmp(&color(b)))
    } else {
        a.ambient.cmp(&b.ambient).then(color(a).cmp(&color(b)))
    }
}

type DurationState = HashMap<AzMobEffect, (EffectGeneration, i32, BlendState)>;

static STATE: OnceLock<Mutex<DurationState>> = OnceLock::new();

pub fn tick_active_effects(active: &ActiveEffects) -> Vec<MobEffectInstance> {
    let mut state = STATE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap();
    state.retain(|id, _| active.0.contains_key(id));

    let mut out = Vec::with_capacity(active.0.len());
    for (id, data) in &active.0 {
        let name = id.to_str().trim_start_matches("minecraft:");
        let new_blend = || {
            if data.flags.blend {
                BlendState::default()
            } else {
                BlendState::immediate(name, data.duration)
            }
        };
        let generation = active.generation(*id);
        let entry = state
            .entry(*id)
            .or_insert_with(|| (generation, data.duration, new_blend()));
        if entry.0 != generation {
            if entry.0.instance != generation.instance {
                entry.2 = new_blend();
            }
            (entry.0, entry.1) = (generation, data.duration);
        }
        if entry.1 > 0 {
            entry.1 -= 1;
        }
        entry.2.tick(name, entry.1);

        out.push(MobEffectInstance {
            id: name.to_string(),
            amplifier: data.amplifier,
            duration: entry.1,
            ambient: data.flags.ambient,
            show_icon: data.flags.show_icon,
            blend: [entry.2.previous, entry.2.factor],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use azalea::entity::MobEffectData;

    #[test]
    fn an_infinite_effect_never_ends_within_anything() {
        assert!(!ends_within(-1, 200));
        assert!(ends_within(200, 200));
        assert!(!ends_within(201, 200));
    }

    #[test]
    fn modifiers_scale_by_amplifier_and_name_the_attribute() {
        let (name, amount, operation) = attribute_modifier("strength", 1).unwrap();
        assert_eq!(name, "attribute.name.attack_damage");
        assert_eq!(amount, 6.0);
        assert_eq!(operation, 0);
        assert_eq!(attribute_modifier("speed", 0).unwrap().2, 2);
        assert!(attribute_modifier("poison", 0).is_none());
    }

    #[test]
    fn night_vision_flickers_only_at_its_end() {
        assert_eq!(night_vision_scale(400, 0.0), 1.0);
        assert_eq!(night_vision_scale(-1, 0.0), 1.0);
        assert!(night_vision_scale(100, 0.0) <= 1.0);
    }

    #[test]
    fn an_equal_refresh_restarts_the_countdown() {
        let effect = AzMobEffect::ConduitPower;
        let data = MobEffectData {
            duration: 260,
            ..Default::default()
        };
        let mut active = ActiveEffects::default();
        active.insert(effect, data);
        let duration = |active: &ActiveEffects| tick_active_effects(active)[0].duration;

        for _ in 0..40 {
            tick_active_effects(&active);
        }
        assert_eq!(duration(&active), 219);

        active.insert(
            effect,
            MobEffectData {
                duration: 260,
                ..Default::default()
            },
        );
        assert_eq!(duration(&active), 259);

        for _ in 0..300 {
            tick_active_effects(&active);
        }
        assert_eq!(duration(&active), 0);

        active.remove(effect);
        assert!(tick_active_effects(&active).is_empty());

        let mut darkness = MobEffectData {
            duration: 100,
            ..Default::default()
        };
        darkness.flags.blend = true;
        active.insert(AzMobEffect::Darkness, darkness.clone());
        for _ in 0..5 {
            tick_active_effects(&active);
        }
        let mut respawned = ActiveEffects::default();
        respawned.insert(AzMobEffect::Darkness, darkness);
        assert_eq!(tick_active_effects(&respawned)[0].blend[0], 0.0);
    }
}
