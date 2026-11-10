use super::registry::{Id, handle, no_render as setting};
use super::store;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    HurtCam,
    Portal,
    Fire,
    LiquidOverlay,
    InWall,
    Pumpkin,
    Vignette,
    Totem,
    BossBars,
    Scoreboard,
    PotionIcons,
    Titles,
    Fog,
    LiquidFog,
    Blindness,
    BossSky,
    Weather,
    Lightning,
}

const PARTS: [(Part, handle::Toggle); 18] = [
    (Part::HurtCam, setting::HURT_CAM),
    (Part::Portal, setting::PORTAL),
    (Part::Fire, setting::FIRE),
    (Part::LiquidOverlay, setting::LIQUID_OVERLAY),
    (Part::InWall, setting::IN_WALL),
    (Part::Pumpkin, setting::PUMPKIN),
    (Part::Vignette, setting::VIGNETTE),
    (Part::Totem, setting::TOTEM),
    (Part::BossBars, setting::BOSS_BARS),
    (Part::Scoreboard, setting::SCOREBOARD),
    (Part::PotionIcons, setting::POTION_ICONS),
    (Part::Titles, setting::TITLES),
    (Part::Fog, setting::FOG),
    (Part::LiquidFog, setting::LIQUID_FOG),
    (Part::Blindness, setting::BLINDNESS),
    (Part::BossSky, setting::BOSS_SKY),
    (Part::Weather, setting::WEATHER),
    (Part::Lightning, setting::LIGHTNING),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hide(u32);

impl Hide {
    pub const NONE: Hide = Hide(0);

    #[inline]
    pub fn has(self, part: Part) -> bool {
        self.0 & (1 << part as u32) != 0
    }
}

pub fn config() -> Hide {
    let s = store();
    if !s.enabled(Id::NoRender) {
        return Hide::NONE;
    }
    let mut bits = 0;
    for (part, toggle) in PARTS {
        if s.flag(toggle) {
            bits |= 1 << part as u32;
        }
    }
    Hide(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_part_has_its_own_bit() {
        let mut seen = 0u32;
        for (part, _) in PARTS {
            let bit = 1u32 << part as u32;
            assert_eq!(seen & bit, 0, "{part:?} listed twice");
            seen |= bit;
        }
        assert_eq!(seen.count_ones() as usize, PARTS.len());
        assert!(
            Part::Lightning as u32 == PARTS.len() as u32 - 1,
            "a Part is missing from PARTS"
        );
    }
}
