use std::fmt::Write;

use super::registry::{Id, extra_hud as setting, options};
use super::store;
use crate::gui::Snapshot;
use crate::gui::painter::Painter;
use crate::session::SlotStack;

options! {
    pub enum Shown {
        Always = "Always",
        IfAny = "If any",
        NotEmpty = "Not empty",
        Damaged = "Damaged",
    }
}

options! {
    pub enum Durability {
        Bar = "Bar",
        Number = "Number",
        Percent = "Percent",
    }
}

const SLOT_BOX: f32 = 22.0;
const SLOT_PITCH: f32 = 20.0;
const WARN: f32 = 8.0;
const LINE: f32 = 9.0;
const HOTBAR_W: f32 = 182.0;
const WARN_RGB: u32 = 0xFF5555;

pub fn enabled() -> bool {
    store().enabled(Id::ExtraHud)
}

fn draw_warning(p: &mut Painter, x: f32, y: f32) {
    p.text_centered("!", x + WARN / 2.0, y, WARN_RGB, true);
}

pub fn draw_armor(p: &mut Painter, snap: &Snapshot) {
    if snap.gamemode == crate::session::Gamemode::Spectator {
        return;
    }
    let Some(armor) = snap.armor.get(..4) else {
        return;
    };
    let s = store();
    let shown: Shown = s.choice(setting::SHOWN);
    let durability: Durability = s.choice(setting::DURABILITY);
    let vertical = s.flag(setting::VERTICAL);
    let warn = s.flag(setting::WARNING);
    let (min_left, min_percent) = (s.num(setting::MIN_LEFT), s.num(setting::MIN_PERCENT));
    let bob = s.num(setting::BOB) as u32;

    let damageable = |st: &SlotStack| st.max_damage > 0 && !st.unbreakable;
    let low = |st: &SlotStack| {
        damageable(st) && {
            let left = st.max_damage.saturating_sub(st.damage) as f32;
            left <= min_left || left * 100.0 / st.max_damage as f32 <= min_percent
        }
    };
    if shown == Shown::IfAny && armor.iter().all(SlotStack::is_empty) {
        return;
    }
    let keep = |st: &SlotStack| match shown {
        Shown::Always | Shown::IfAny => true,
        Shown::NotEmpty => !st.is_empty(),
        Shown::Damaged => low(st),
    };
    let mut order = [0, 1, 2, 3];
    if s.flag(setting::REVERSED) {
        order.reverse();
    }
    let n = order.iter().filter(|&&i| keep(&armor[i])).count();
    if n == 0 {
        return;
    }
    let empty_icons = s.flag(setting::EMPTY_ICONS) && matches!(shown, Shown::Always | Shown::IfAny);
    let text = durability != Durability::Bar;
    let length = SLOT_BOX + (n - 1) as f32 * SLOT_PITCH;
    let free = (4 - n) as f32 * SLOT_PITCH;
    let full = SLOT_BOX + 3.0 * SLOT_PITCH;
    let warn_room = if warn { WARN + 2.0 } else { 0.0 };
    let up = bob.div_ceil(2) as f32;

    let (x0, y0) = if vertical {
        let text_w = if text {
            p.atlas.font.width_str("100%") + 2.0
        } else {
            0.0
        };
        p.fill(0.0, 0.0, SLOT_BOX + warn_room + text_w, full, 0);
        (0.0, 0.0)
    } else {
        let top = if text { LINE } else { 0.0 } + if warn { warn_room + up } else { 0.0 };
        p.fill(0.0, 0.0, full, top + SLOT_BOX, 0);
        (free, top)
    };

    if s.flag(setting::BACKGROUND) {
        if vertical {
            for k in 0..n {
                let h = if k + 1 == n { SLOT_BOX } else { SLOT_PITCH };
                p.sprite_part(
                    "hud/hotbar",
                    0.0,
                    0.0,
                    SLOT_BOX,
                    h,
                    x0,
                    y0 + k as f32 * SLOT_PITCH,
                );
            }
        } else {
            p.sprite_part("hud/hotbar", 0.0, 0.0, length - 3.0, SLOT_BOX, x0, y0);
            p.sprite_part(
                "hud/hotbar",
                HOTBAR_W - 3.0,
                0.0,
                3.0,
                SLOT_BOX,
                x0 + length - 3.0,
                y0,
            );
        }
    }

    let mut rng = crate::gui::health::frame_random(p.frame as u64);
    let mut label = String::new();
    let mut k = 0.0;
    for i in order {
        let st = &armor[i];
        if !keep(st) {
            continue;
        }
        let (x, y) = if vertical {
            (x0, y0 + k * SLOT_PITCH)
        } else {
            (x0 + k * SLOT_PITCH, y0)
        };
        k += 1.0;
        if st.is_empty() {
            if empty_icons
                && let Some(icon) = crate::gui::slots::Layout::PlayerMenu.empty_icon(5 + i)
            {
                p.sprite(icon, x + 3.0, y + 3.0, 16.0, 16.0);
            }
            continue;
        }
        crate::gui::slots::draw_stack(p, st, x + 3.0, y + 3.0);

        label.clear();
        if text && damageable(st) {
            let left = st.max_damage.saturating_sub(st.damage);
            match durability {
                Durability::Number => _ = write!(label, "{left}"),
                Durability::Percent if st.damage > 0 => {
                    _ = write!(
                        label,
                        "{}%",
                        u32::from(left) * 100 / u32::from(st.max_damage)
                    )
                }
                _ => {}
            }
        }
        let color = crate::gui::painter::bar_color(st.damage, st.max_damage);
        let bobbed = if bob == 0 {
            0.0
        } else {
            rng.next_int(bob) as f32 - up
        };
        if vertical {
            if warn && low(st) {
                draw_warning(p, x + SLOT_BOX + 1.0, y + (SLOT_BOX - WARN) / 2.0 + bobbed);
            }
            let text_y = y + ((SLOT_BOX - LINE) / 2.0).floor();
            p.text_plain(&label, x + SLOT_BOX + warn_room + 1.0, text_y, color, true);
        } else {
            let text_y = y - if text { LINE } else { 0.0 };
            p.text_centered(&label, x + SLOT_BOX / 2.0, text_y, color, true);
            if warn && low(st) {
                let wy = text_y - (WARN + 2.0) + 1.0 + bobbed;
                draw_warning(p, x + (SLOT_BOX - WARN) / 2.0, wy);
            }
        }
    }
}
