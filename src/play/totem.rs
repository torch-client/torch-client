use std::sync::atomic::{AtomicU32, Ordering};

use crate::renderer::HeldItem;
use crate::session::{ItemActivation, SharedMutex};

const EMITTER_TICKS: u32 = 30;

const TOTEM_SOUND: &str = "item.totem.use";

static LAST_SERIAL: AtomicU32 = AtomicU32::new(0);

fn pick_totem(held: [&HeldItem; 2]) -> HeldItem {
    held.into_iter()
        .find(|item| item.death_protection)
        .cloned()
        .unwrap_or_else(|| HeldItem {
            id: "totem_of_undying",
            count: 1,
            death_protection: true,
            ..HeldItem::default()
        })
}

pub(crate) fn protected_from_death(shared: &SharedMutex, entity: i32) {
    crate::play::tracking_emitter::start(
        entity,
        azalea::entity::particle::Particle::TotemOfUndying,
        EMITTER_TICKS,
        Some(TOTEM_SOUND),
    );
    if entity != crate::client::tracking::local_entity_id() {
        return;
    }
    let mut s = shared.lock().unwrap();
    let item = pick_totem(s.session.local_anim.held());
    let serial = LAST_SERIAL.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    s.session.item_activation = Some(ItemActivation { serial, item });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding(id: &'static str, death_protection: bool) -> HeldItem {
        HeldItem {
            id,
            count: 1,
            death_protection,
            ..HeldItem::default()
        }
    }

    #[test]
    fn picks_the_main_hand_first() {
        let mut main = holding("totem_of_undying", true);
        main.count = 3;
        let off = holding("totem_of_undying", true);
        assert_eq!(pick_totem([&main, &off]).count, 3);
    }

    #[test]
    fn picks_by_the_component_not_the_id() {
        let stripped = holding("totem_of_undying", false);
        let custom = holding("stick", true);
        assert_eq!(pick_totem([&stripped, &custom]), custom);
    }

    #[test]
    fn falls_back_to_a_fresh_totem() {
        let sword = holding("diamond_sword", false);
        let empty = HeldItem::default();
        assert_eq!(
            pick_totem([&sword, &empty]),
            holding("totem_of_undying", true)
        );
    }
}
