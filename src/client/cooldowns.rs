use std::sync::{Mutex, OnceLock};

use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_registry::identifier::Identifier;

struct Cooldown {
    group: Identifier,
    start: u32,
    end: u32,
}

#[derive(Default)]
struct State {
    active: Vec<Cooldown>,
    tick: u32,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(Default::default)
}

pub(crate) fn reset() {
    let mut s = state().lock().unwrap();
    s.active.clear();
    s.tick = 0;
}

pub(crate) fn tick() {
    let mut s = state().lock().unwrap();
    s.tick = s.tick.wrapping_add(1);
    let now = s.tick;
    s.active.retain(|c| c.end > now);
}

pub(crate) fn packet(packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::Cooldown(p) => {
            let mut s = state().lock().unwrap();
            s.active.retain(|c| c.group != p.cooldown_group);
            if p.duration > 0 {
                let start = s.tick;
                s.active.push(Cooldown {
                    group: p.cooldown_group.clone(),
                    start,
                    end: start.saturating_add(p.duration),
                });
            }
        }
        ClientboundGamePacket::StartConfiguration(_) => reset(),
        _ => {}
    }
}

pub(crate) fn any_active() -> bool {
    !state().lock().unwrap().active.is_empty()
}

pub(crate) fn percent(group_override: Option<&Identifier>, item: &str) -> u8 {
    let s = state().lock().unwrap();
    if s.active.is_empty() {
        return 0;
    }
    let matches = |c: &Cooldown| match group_override {
        Some(group) => c.group == *group,
        None => c.group.namespace() == "minecraft" && c.group.path() == item,
    };
    let Some(c) = s.active.iter().find(|c| matches(c)) else {
        return 0;
    };
    let duration = u64::from(c.end.saturating_sub(c.start)).max(1);
    let remaining = u64::from(c.end.saturating_sub(s.tick));
    (remaining * 255).div_ceil(duration).min(255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> Identifier {
        Identifier::new(s)
    }

    fn add(group: &str, duration: u32) {
        packet(&ClientboundGamePacket::Cooldown(
            azalea_protocol::packets::game::c_cooldown::ClientboundCooldown {
                cooldown_group: id(group),
                duration,
            },
        ));
    }

    #[test]
    fn the_cooldown_lifecycle() {
        reset();
        add("minecraft:ender_pearl", 20);
        assert_eq!(percent(None, "ender_pearl"), 255);
        assert_eq!(
            percent(None, "chorus_fruit"),
            0,
            "another item is untouched"
        );
        assert!(any_active());

        for _ in 0..10 {
            tick();
        }
        assert_eq!(
            percent(None, "ender_pearl"),
            128,
            "half the ticks, half the bar"
        );

        for _ in 0..10 {
            tick();
        }
        assert_eq!(percent(None, "ender_pearl"), 0, "expired");
        assert!(!any_active(), "and dropped, not merely reading as zero");

        reset();
        add("minecraft:ender_pearl", 20);
        add("minecraft:ender_pearl", 40);
        assert_eq!(state().lock().unwrap().active.len(), 1);
        add("minecraft:ender_pearl", 0);
        assert!(!any_active());

        reset();
        add("mypack:charges", 10);
        assert_eq!(percent(None, "ender_pearl"), 0, "not filed under the id");
        assert_eq!(percent(Some(&id("mypack:charges")), "ender_pearl"), 255);

        reset();
        add("ender_pearl", 10);
        assert_eq!(percent(None, "ender_pearl"), 255, "namespace is implied");

        reset();
        add("minecraft:shield", 600);
        for _ in 0..599 {
            tick();
        }
        assert_eq!(percent(None, "shield"), 1);

        reset();
        add("minecraft:ender_pearl", 100);
        packet(&ClientboundGamePacket::StartConfiguration(
            azalea_protocol::packets::game::c_start_configuration::ClientboundStartConfiguration,
        ));
        assert!(!any_active());
    }
}
