use std::cell::RefCell;
use std::rc::Rc;

use azalea_registry::Registry;
use azalea_registry::builtin::EntityKind;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Reach {
    Always,
    Blocks(f64),
    Scaled(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Extra {
    None,
    AlwaysWithBeam,
    HiddenWhileAttached,
}

fn reach(kind: EntityKind, size: f64) -> (Reach, Extra) {
    use EntityKind as K;
    let base = Reach::Scaled(size * 64.0);
    let rule = match kind {
        K::TextDisplay | K::BlockDisplay | K::ItemDisplay => Reach::Always,
        K::Phantom => Reach::Always,
        K::LightningBolt => Reach::Scaled(64.0),
        K::Snowball
        | K::Egg
        | K::EnderPearl
        | K::ExperienceBottle
        | K::SplashPotion
        | K::LingeringPotion
        | K::EyeOfEnder
        | K::Fireball
        | K::SmallFireball
        | K::DragonFireball
        | K::WitherSkull
        | K::WindCharge
        | K::BreezeWindCharge => Reach::Blocks(size * 4.0 * 64.0),
        K::ArmorStand => Reach::Blocks(if size == 0.0 { 4.0 } else { size * 4.0 } * 64.0),
        K::Arrow | K::SpectralArrow | K::Trident | K::Player => Reach::Scaled(size * 10.0 * 64.0),
        K::ItemFrame | K::GlowItemFrame => Reach::Scaled(16.0 * 64.0),
        K::ShulkerBullet => Reach::Blocks(128.0),
        K::FishingBobber | K::FireworkRocket => Reach::Blocks(64.0),
        K::LeashKnot => Reach::Blocks(32.0),
        _ => base,
    };
    let extra = match kind {
        K::EndCrystal => Extra::AlwaysWithBeam,
        K::FireworkRocket => Extra::HiddenWhileAttached,
        _ => Extra::None,
    };
    (rule, extra)
}

pub struct RenderRanges {
    view_scale: f64,
    max_sq: Vec<f64>,
    extra: Vec<Extra>,
}

impl RenderRanges {
    const fn new() -> Self {
        Self {
            view_scale: f64::NAN,
            max_sq: Vec::new(),
            extra: Vec::new(),
        }
    }

    fn refresh(&mut self, view_scale: f64) {
        self.view_scale = view_scale;
        self.max_sq.clear();
        self.extra.clear();
        let mut id = 0;
        while let Some(kind) = EntityKind::from_u32(id) {
            let dims = azalea::entity::dimensions::EntityDimensions::from(kind);
            let size = f64::from(dims.width + dims.height + dims.width) / 3.0;
            let (rule, extra) = reach(kind, size);
            let blocks = match rule {
                Reach::Always => f64::INFINITY,
                Reach::Blocks(b) => b,
                Reach::Scaled(b) => b * view_scale,
            };
            self.max_sq.push(blocks * blocks);
            self.extra.push(extra);
            id += 1;
        }
    }

    #[inline]
    pub fn drawn(
        &self,
        kind: EntityKind,
        distance_sq: f64,
        beam: impl FnOnce() -> bool,
        attached: impl FnOnce() -> bool,
    ) -> bool {
        let i = kind.to_u32() as usize;
        let Some(&max_sq) = self.max_sq.get(i) else {
            return true;
        };
        let within = distance_sq < max_sq;
        match self.extra[i] {
            Extra::None => within,
            Extra::AlwaysWithBeam => within || beam(),
            Extra::HiddenWhileAttached => within && !attached(),
        }
    }
}

thread_local! {
    static RANGES: RefCell<Rc<RenderRanges>> = RefCell::new(Rc::new(RenderRanges::new()));
}

pub fn ranges(view_scale: f64) -> Rc<RenderRanges> {
    RANGES.with_borrow_mut(|ranges| {
        if ranges.view_scale != view_scale {
            let mut fresh = RenderRanges::new();
            fresh.refresh(view_scale);
            *ranges = Rc::new(fresh);
        }
        Rc::clone(ranges)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(kind: EntityKind, distance: f64, view_scale: f64) -> bool {
        ranges(view_scale).drawn(kind, distance * distance, || false, || false)
    }

    #[test]
    fn a_display_is_never_cut_by_distance() {
        assert!(drawn(EntityKind::TextDisplay, 10_000.0, 1.0));
    }

    #[test]
    fn a_zombie_follows_the_base_rule() {
        assert!(drawn(EntityKind::Zombie, 67.0, 1.0));
        assert!(!drawn(EntityKind::Zombie, 67.5, 1.0));
        assert!(drawn(EntityKind::Zombie, 130.0, 2.0));
    }

    #[test]
    fn fixed_ranges_ignore_the_view_scale() {
        assert!(drawn(EntityKind::LeashKnot, 31.9, 2.5));
        assert!(!drawn(EntityKind::LeashKnot, 32.1, 2.5));
    }

    #[test]
    fn the_two_data_rules_read_their_field() {
        let r = ranges(1.0);
        {
            assert!(r.drawn(EntityKind::EndCrystal, 1e9, || true, || false));
            assert!(!r.drawn(EntityKind::FireworkRocket, 1.0, || false, || true));
            assert!(r.drawn(EntityKind::FireworkRocket, 1.0, || false, || false));
        }
    }
}
