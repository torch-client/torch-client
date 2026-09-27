use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use azalea::entity::ActiveEffects;
use azalea::registry::builtin::MobEffect as AzMobEffect;

use crate::gui::tooltip::translate;

#[derive(Clone, Debug, PartialEq)]
pub struct MobEffectInstance {
    pub id: String,
    pub amplifier: i32,
    pub duration: i32,
    pub ambient: bool,
    pub show_icon: bool,
}

impl MobEffectInstance {
    pub fn is_infinite(&self) -> bool {
        self.duration == -1
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
    modifier: Option<(&'static str, f64, u8)>,
}

fn effect_meta(id: &str) -> EffectMeta {
    use self::Category::{Beneficial as B, Harmful as H, Neutral as N};
    let (category, color, modifier) = match id {
        "speed" => (B, 3402751, Some(("movement_speed", 0.2, 2))),
        "slowness" => (H, 9154528, Some(("movement_speed", -0.15, 2))),
        "haste" => (B, 14270531, Some(("attack_speed", 0.1, 2))),
        "mining_fatigue" => (H, 4866583, Some(("attack_speed", -0.1, 2))),
        "strength" => (B, 16762624, Some(("attack_damage", 3.0, 0))),
        "instant_health" => (B, 16262179, None),
        "instant_damage" => (H, 11101546, None),
        "jump_boost" => (B, 16646020, Some(("safe_fall_distance", 1.0, 0))),
        "nausea" => (H, 5578058, None),
        "regeneration" => (B, 13458603, None),
        "resistance" => (B, 9520880, None),
        "fire_resistance" => (B, 16750848, None),
        "water_breathing" => (B, 10017472, None),
        "invisibility" => (B, 16185078, Some(("waypoint_transmit_range", -1.0, 2))),
        "blindness" => (H, 2039587, None),
        "night_vision" => (B, 12779366, None),
        "hunger" => (H, 5797459, None),
        "weakness" => (H, 4738376, Some(("attack_damage", -4.0, 0))),
        "poison" => (H, 8889187, None),
        "wither" => (H, 7561558, None),
        "health_boost" => (B, 16284963, Some(("max_health", 4.0, 0))),
        "absorption" => (B, 2445989, Some(("max_absorption", 4.0, 0))),
        "saturation" => (B, 16262179, None),
        "glowing" => (N, 9740385, None),
        "levitation" => (H, 13565951, None),
        "luck" => (B, 5882118, Some(("luck", 1.0, 0))),
        "unluck" => (H, 12624973, Some(("luck", -1.0, 0))),
        "slow_falling" => (B, 15978425, None),
        "conduit_power" => (B, 1950417, None),
        "dolphins_grace" => (B, 8954814, None),
        "bad_omen" => (N, 745784, None),
        "hero_of_the_village" => (B, 4521796, None),
        "darkness" => (H, 2696993, None),
        "trial_omen" => (N, 1484454, None),
        "raid_omen" => (N, 14565464, None),
        "wind_charged" => (H, 12438015, None),
        "weaving" => (H, 7891290, None),
        "oozing" => (H, 10092451, None),
        "infested" => (H, 9214860, None),
        "breath_of_the_nautilus" => (B, 65518, None),
        _ => (H, 0xFFFFFF, None),
    };
    EffectMeta {
        category,
        color,
        modifier,
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
    let (attribute, amount, operation) = effect_meta(id).modifier?;
    Some((
        format!("attribute.name.{attribute}"),
        amount * (amplifier + 1) as f64,
        operation,
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

pub fn format_duration(duration_ticks: i32) -> String {
    if duration_ticks < 0 {
        return translate("effect.duration.infinite", &[]);
    }
    let total_seconds = duration_ticks / 20;
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

type DurationState = HashMap<AzMobEffect, (i32, i32)>;

static STATE: OnceLock<Mutex<DurationState>> = OnceLock::new();

pub fn tick_active_effects(active: &ActiveEffects) -> Vec<MobEffectInstance> {
    let mut state = STATE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap();
    state.retain(|id, _| active.0.contains_key(id));

    let mut out = Vec::with_capacity(active.0.len());
    for (id, data) in &active.0 {
        let entry = state.entry(*id).or_insert((data.duration, data.duration));
        if data.duration == -1 {
            *entry = (-1, -1);
        } else if entry.0 != data.duration {
            *entry = (data.duration, data.duration);
        } else if entry.1 > 0 {
            entry.1 -= 1;
        }

        if entry.1 <= 0 && data.duration != -1 {
            continue;
        }

        out.push(MobEffectInstance {
            id: id.to_str().trim_start_matches("minecraft:").to_string(),
            amplifier: data.amplifier,
            duration: entry.1,
            ambient: data.flags.ambient,
            show_icon: data.flags.show_icon,
        });
    }
    out
}
