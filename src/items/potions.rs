use crate::play::mob_effects;

pub struct PotionEffect {
    pub effect: &'static str,
    pub duration: i32,
    pub amplifier: i32,
}

pub struct Potion {
    pub id: &'static str,
    pub name: &'static str,
    pub effects: &'static [PotionEffect],
}

const fn e(effect: &'static str, duration: i32, amplifier: i32) -> PotionEffect {
    PotionEffect {
        effect,
        duration,
        amplifier,
    }
}

const fn p(id: &'static str, name: &'static str, effects: &'static [PotionEffect]) -> Potion {
    Potion { id, name, effects }
}

pub static POTIONS: &[Potion] = &[
    p("water", "water", &[]),
    p("mundane", "mundane", &[]),
    p("thick", "thick", &[]),
    p("awkward", "awkward", &[]),
    p(
        "night_vision",
        "night_vision",
        &[e("night_vision", 3600, 0)],
    ),
    p(
        "long_night_vision",
        "night_vision",
        &[e("night_vision", 9600, 0)],
    ),
    p(
        "invisibility",
        "invisibility",
        &[e("invisibility", 3600, 0)],
    ),
    p(
        "long_invisibility",
        "invisibility",
        &[e("invisibility", 9600, 0)],
    ),
    p("leaping", "leaping", &[e("jump_boost", 3600, 0)]),
    p("long_leaping", "leaping", &[e("jump_boost", 9600, 0)]),
    p("strong_leaping", "leaping", &[e("jump_boost", 1800, 1)]),
    p(
        "fire_resistance",
        "fire_resistance",
        &[e("fire_resistance", 3600, 0)],
    ),
    p(
        "long_fire_resistance",
        "fire_resistance",
        &[e("fire_resistance", 9600, 0)],
    ),
    p("swiftness", "swiftness", &[e("speed", 3600, 0)]),
    p("long_swiftness", "swiftness", &[e("speed", 9600, 0)]),
    p("strong_swiftness", "swiftness", &[e("speed", 1800, 1)]),
    p("slowness", "slowness", &[e("slowness", 1800, 0)]),
    p("long_slowness", "slowness", &[e("slowness", 4800, 0)]),
    p("strong_slowness", "slowness", &[e("slowness", 400, 3)]),
    p(
        "turtle_master",
        "turtle_master",
        &[e("slowness", 400, 3), e("resistance", 400, 2)],
    ),
    p(
        "long_turtle_master",
        "turtle_master",
        &[e("slowness", 800, 3), e("resistance", 800, 2)],
    ),
    p(
        "strong_turtle_master",
        "turtle_master",
        &[e("slowness", 400, 5), e("resistance", 400, 3)],
    ),
    p(
        "water_breathing",
        "water_breathing",
        &[e("water_breathing", 3600, 0)],
    ),
    p(
        "long_water_breathing",
        "water_breathing",
        &[e("water_breathing", 9600, 0)],
    ),
    p("healing", "healing", &[e("instant_health", 1, 0)]),
    p("strong_healing", "healing", &[e("instant_health", 1, 1)]),
    p("harming", "harming", &[e("instant_damage", 1, 0)]),
    p("strong_harming", "harming", &[e("instant_damage", 1, 1)]),
    p("poison", "poison", &[e("poison", 900, 0)]),
    p("long_poison", "poison", &[e("poison", 1800, 0)]),
    p("strong_poison", "poison", &[e("poison", 432, 1)]),
    p("regeneration", "regeneration", &[e("regeneration", 900, 0)]),
    p(
        "long_regeneration",
        "regeneration",
        &[e("regeneration", 1800, 0)],
    ),
    p(
        "strong_regeneration",
        "regeneration",
        &[e("regeneration", 450, 1)],
    ),
    p("strength", "strength", &[e("strength", 3600, 0)]),
    p("long_strength", "strength", &[e("strength", 9600, 0)]),
    p("strong_strength", "strength", &[e("strength", 1800, 1)]),
    p("weakness", "weakness", &[e("weakness", 1800, 0)]),
    p("long_weakness", "weakness", &[e("weakness", 4800, 0)]),
    p("luck", "luck", &[e("luck", 6000, 0)]),
    p(
        "slow_falling",
        "slow_falling",
        &[e("slow_falling", 1800, 0)],
    ),
    p(
        "long_slow_falling",
        "slow_falling",
        &[e("slow_falling", 4800, 0)],
    ),
    p(
        "wind_charged",
        "wind_charged",
        &[e("wind_charged", 3600, 0)],
    ),
    p("weaving", "weaving", &[e("weaving", 3600, 0)]),
    p("oozing", "oozing", &[e("oozing", 3600, 0)]),
    p("infested", "infested", &[e("infested", 3600, 0)]),
];

pub const POTION_ITEMS: [&str; 4] = [
    "potion",
    "splash_potion",
    "lingering_potion",
    "tipped_arrow",
];

pub const BASE_COLOR: u32 = 0x385DC6;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PotionContents {
    pub potion: Option<String>,
    pub custom_color: Option<u32>,
    pub custom_effects: Vec<PotionEffectInstance>,
    pub custom_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PotionEffectInstance {
    pub effect: String,
    pub duration: i32,
    pub amplifier: i32,
}

pub fn potion(id: &str) -> Option<&'static Potion> {
    POTIONS.iter().find(|p| p.id == id)
}

pub fn is_potion_item(item: &str) -> bool {
    POTION_ITEMS.contains(&item)
}

impl PotionContents {
    pub fn all_effects(&self) -> Vec<PotionEffectInstance> {
        let mut out: Vec<PotionEffectInstance> = self
            .potion
            .as_deref()
            .and_then(potion)
            .map(|p| {
                p.effects
                    .iter()
                    .map(|e| PotionEffectInstance {
                        effect: e.effect.to_string(),
                        duration: e.duration,
                        amplifier: e.amplifier,
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.extend(self.custom_effects.iter().cloned());
        out
    }

    pub fn color(&self) -> u32 {
        if let Some(color) = self.custom_color {
            return color & 0xFF_FFFF;
        }
        let (mut r, mut g, mut b, mut weight) = (0u32, 0u32, 0u32, 0u32);
        for effect in self.all_effects() {
            let color = mob_effects::color(&effect.effect);
            let amplifier = (effect.amplifier + 1).max(0) as u32;
            r += amplifier * ((color >> 16) & 0xFF);
            g += amplifier * ((color >> 8) & 0xFF);
            b += amplifier * (color & 0xFF);
            weight += amplifier;
        }
        if weight == 0 {
            return BASE_COLOR;
        }
        ((r / weight) << 16) | ((g / weight) << 8) | (b / weight)
    }

    pub fn name_key(&self, item: &str) -> String {
        let suffix = self
            .custom_name
            .as_deref()
            .or_else(|| self.potion.as_deref().and_then(potion).map(|p| p.name))
            .unwrap_or("empty");
        format!("item.minecraft.{item}.effect.{suffix}")
    }
}

pub fn potion_color(p: &Potion) -> u32 {
    PotionContents {
        potion: Some(p.id.to_string()),
        ..Default::default()
    }
    .color()
}

pub fn tint_key(item: &str, color: u32) -> String {
    format!("{item}#{:06x}", color & 0xFF_FFFF)
}

pub fn parse_tint_key(key: &str) -> Option<(&str, u32)> {
    let (item, hex) = key.split_once('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((item, u32::from_str_radix(hex, 16).ok()?))
}

pub fn layer_key(item: &str, layer: usize) -> String {
    format!("{item}#layer{layer}")
}

pub fn baked_colors() -> Vec<u32> {
    let mut out: Vec<u32> = POTIONS.iter().map(potion_color).collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_matches_vanilla() {
        assert_eq!(POTIONS.len(), 46);
        assert!(POTIONS.iter().all(|p| !p.id.is_empty()));
        let ids: std::collections::HashSet<&str> = POTIONS.iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), POTIONS.len(), "duplicate potion id");
        assert!(potion("long_night_vision").is_some());
        assert!(potion("night_vision_long").is_none());
    }

    fn contents(id: &str) -> PotionContents {
        PotionContents {
            potion: Some(id.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn long_and_strong_share_a_name() {
        for (id, name) in [
            ("night_vision", "night_vision"),
            ("long_night_vision", "night_vision"),
            ("strong_leaping", "leaping"),
            ("long_turtle_master", "turtle_master"),
        ] {
            assert_eq!(potion(id).unwrap().name, name);
        }
        assert_eq!(
            contents("long_leaping").name_key("splash_potion"),
            "item.minecraft.splash_potion.effect.leaping"
        );
        assert_eq!(
            PotionContents::default().name_key("potion"),
            "item.minecraft.potion.effect.empty"
        );
        let named = PotionContents {
            potion: Some("swiftness".into()),
            custom_name: Some("luck".into()),
            ..Default::default()
        };
        assert_eq!(
            named.name_key("potion"),
            "item.minecraft.potion.effect.luck"
        );
    }

    #[test]
    fn colors_average_by_amplifier() {
        assert_eq!(contents("water").color(), BASE_COLOR);
        assert_eq!(contents("awkward").color(), BASE_COLOR);
        assert_eq!(PotionContents::default().color(), BASE_COLOR);
        let speed = mob_effects::color("speed");
        assert_eq!(contents("swiftness").color(), speed);
        assert_eq!(contents("strong_swiftness").color(), speed);
        let (slow, resist) = (
            mob_effects::color("slowness"),
            mob_effects::color("resistance"),
        );
        let mix = |shift: u32| {
            ((4 * ((slow >> shift) & 0xFF) + 3 * ((resist >> shift) & 0xFF)) / 7) & 0xFF
        };
        let expected = (mix(16) << 16) | (mix(8) << 8) | mix(0);
        assert_eq!(contents("turtle_master").color(), expected);
    }

    #[test]
    fn custom_color_and_effects_override() {
        let custom = PotionContents {
            potion: Some("swiftness".into()),
            custom_color: Some(0xFF_12_34_56),
            ..Default::default()
        };
        assert_eq!(custom.color(), 0x123456);
        let mixed = PotionContents {
            potion: Some("swiftness".into()),
            custom_effects: vec![PotionEffectInstance {
                effect: "poison".into(),
                duration: 200,
                amplifier: 0,
            }],
            ..Default::default()
        };
        let effects = mixed.all_effects();
        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0].effect, "speed");
        assert_eq!(effects[1].effect, "poison");
        let (speed, poison) = (mob_effects::color("speed"), mob_effects::color("poison"));
        let mix = |shift: u32| (((speed >> shift) & 0xFF) + ((poison >> shift) & 0xFF)) / 2;
        assert_eq!(mixed.color(), (mix(16) << 16) | (mix(8) << 8) | mix(0));
        let only_custom = PotionContents {
            custom_effects: mixed.custom_effects.clone(),
            ..Default::default()
        };
        assert_eq!(only_custom.color(), poison);
    }

    #[test]
    fn tint_keys_round_trip() {
        assert_eq!(tint_key("potion", 0x385DC6), "potion#385dc6");
        assert_eq!(tint_key("potion", 0xFF38_5DC6), "potion#385dc6");
        assert_eq!(
            parse_tint_key("splash_potion#385dc6"),
            Some(("splash_potion", 0x385DC6))
        );
        assert_eq!(parse_tint_key("diamond_sword"), None);
        assert_eq!(parse_tint_key(&layer_key("potion", 0)), None);
        assert_eq!(parse_tint_key("potion#water"), None);
    }

    #[test]
    fn baked_colors_cover_every_potion_and_dedupe() {
        let colors = baked_colors();
        for p in POTIONS {
            assert!(colors.contains(&potion_color(p)), "{}", p.id);
        }
        assert!(colors.len() < POTIONS.len(), "{}", colors.len());
    }

    #[test]
    fn every_effect_is_known() {
        for p in POTIONS {
            for effect in p.effects {
                assert_ne!(
                    mob_effects::color(effect.effect),
                    0xFFFFFF,
                    "unknown effect {}",
                    effect.effect
                );
            }
        }
    }
}
