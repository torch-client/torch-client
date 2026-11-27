use std::sync::atomic::{AtomicI32, AtomicI64, Ordering::Relaxed};

use azalea::Client;
use azalea::block::fluid_state::{FluidKind, FluidState};
use azalea::entity::Physics;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::{FallFlying, IsPowered, SwellDir};
use azalea::inventory::ContainerClickEvent;
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea_core::position::Vec3 as AzVec3;
use azalea_inventory::operations::SwapClick;
use azalea_inventory::{ItemStack, Menu};
use azalea_registry::builtin::{EntityKind, ItemKind};

use super::registry::{Id, auto_totem as setting, options};
use super::{MS_PER_TICK, roll, store};
use crate::play::interaction::{select_hotbar_slot, swap_hands};
use crate::session::{Gamemode, SharedMutex};

options! {
    pub enum TotemMode {
        Always = "Always",
        Smart = "Smart",
    }
}

const OFFHAND: u8 = 40;

const SETTLE_TICKS: i64 = 20;

const SAFE_FALL: f64 = 3.0;

const HARD: f32 = 1.5;

const FALL_SEARCH: f64 = 384.0;

const NONE: i32 = -1;

static NEXT: AtomicI64 = AtomicI64::new(0);

static AWAIT: AtomicI64 = AtomicI64::new(0);

static BORROW: AtomicI32 = AtomicI32::new(NONE);

static BACKUP: AtomicI32 = AtomicI32::new(NONE);

pub fn enabled() -> bool {
    store().enabled(Id::AutoTotem)
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Hand,
    Slots,
    Other,
}

pub fn tick(bot: &Client, shared: &SharedMutex) {
    put_back(bot);
    if !enabled() {
        return;
    }
    let s = store();
    let now = crate::client::tracking::client_ticks();
    if now < NEXT.load(Relaxed) {
        return;
    }

    let (health, screen, open) = {
        let st = shared.lock().unwrap();
        if !st.in_world
            || st.session.dead
            || matches!(
                st.session.gamemode,
                Gamemode::Creative | Gamemode::Spectator
            )
        {
            return;
        }
        let screen = if !st.screen_open || st.chat_screen {
            Screen::Hand
        } else if st.slot_screen {
            Screen::Slots
        } else {
            Screen::Other
        };
        (
            st.session.health + st.session.health_display.absorption,
            screen,
            st.screen_open,
        )
    };

    let Some(h) = hands(bot) else {
        return;
    };
    let illegit = s.flag(setting::ILLEGIT);
    let can_click = !h.no_slots
        && if illegit {
            open
        } else {
            screen == Screen::Slots && h.cursor_empty
        };

    if h.offhand {
        AWAIT.store(0, Relaxed);
        if !h.held {
            BACKUP.store(NONE, Relaxed);
            if s.flag(setting::MAINHAND)
                && can_click
                && let Some(slot) = h.spare
                && in_danger(bot, health)
            {
                click(bot, h.window, slot, h.selected);
                BACKUP.store(h.selected as i32, Relaxed);
                pace(now);
            }
        }
        return;
    }
    if now < AWAIT.load(Relaxed) {
        return;
    }

    let backup = h.held && BACKUP.load(Relaxed) == h.selected as i32;
    if !backup && s.choice(setting::MODE) == TotemMode::Smart && !in_danger(bot, health) {
        return;
    }

    let moved = if illegit {
        let source = h.spare.or((h.held && !h.no_slots).then_some(h.held_slot));
        source.is_some_and(|slot| {
            click(bot, h.window, slot, OFFHAND);
            true
        })
    } else {
        match screen {
            Screen::Hand => {
                let borrow = if h.held { None } else { h.hotbar };
                if h.held || borrow.is_some() {
                    if let Some(k) = borrow {
                        BORROW.store(((h.selected as i32) << 8) | k as i32, Relaxed);
                        select_hotbar_slot(bot, k);
                    }
                    swap_hands(bot);
                    AWAIT.store(now + SETTLE_TICKS, Relaxed);
                    true
                } else {
                    false
                }
            }
            Screen::Slots if can_click => {
                let source = h.spare.or(h.held.then_some(h.held_slot));
                source.is_some_and(|slot| {
                    click(bot, h.window, slot, OFFHAND);
                    true
                })
            }
            _ => false,
        }
    };
    if moved {
        pace(now);
    }
}

fn pace(now: i64) {
    let (lo, hi) = store().range(setting::DELAY);
    NEXT.store(now + (roll(lo, hi) / MS_PER_TICK).round() as i64, Relaxed);
}

fn put_back(bot: &Client) {
    let b = BORROW.swap(NONE, Relaxed);
    if b == NONE {
        return;
    }
    let (old, borrowed) = ((b >> 8) as u8, (b & 0xff) as u8);
    if bot.selected_hotbar_slot().is_ok_and(|s| s == borrowed) {
        select_hotbar_slot(bot, old);
    }
}

fn click(bot: &Client, window: i32, slot: u16, target: u8) {
    bot.ecs.write().trigger(ContainerClickEvent {
        entity: bot.entity,
        window_id: window,
        operation: SwapClick {
            source_slot: slot,
            target_slot: target,
        }
        .into(),
    });
}

struct Hands {
    window: i32,
    no_slots: bool,
    cursor_empty: bool,
    offhand: bool,
    selected: u8,
    held: bool,
    held_slot: u16,
    hotbar: Option<u8>,
    spare: Option<u16>,
}

fn hands(bot: &Client) -> Option<Hands> {
    let inv = bot.component::<Inventory>().ok()?;
    let player = &inv.inventory_menu;
    let menu = inv.menu();
    let selected = inv.selected_hotbar_slot;
    let at = |m: &Menu, k: u8| *m.hotbar_slots_range().start() + k as usize;

    let held = player.slot(at(player, selected)).is_some_and(is_totem);
    let hotbar = (0..9u8)
        .filter(|&k| k != selected)
        .find(|&k| player.slot(at(player, k)).is_some_and(is_totem));

    let no_slots = matches!(menu, Menu::Lectern { .. });
    let (held_slot, spare) = if no_slots {
        (0, None)
    } else {
        let held_slot = at(menu, selected);
        let spare = menu
            .player_slots_without_hotbar_range()
            .chain(menu.hotbar_slots_range())
            .find(|&i| i != held_slot && menu.slot(i).is_some_and(is_totem));
        (held_slot as u16, spare.map(|i| i as u16))
    };

    Some(Hands {
        window: inv.id,
        no_slots,
        cursor_empty: inv.carried.is_empty(),
        offhand: is_totem(&player.as_player().offhand),
        selected,
        held,
        held_slot,
        hotbar,
        spare,
    })
}

fn is_totem(stack: &ItemStack) -> bool {
    matches!(stack, ItemStack::Present(d) if d.kind == ItemKind::TotemOfUndying)
}

fn in_danger(bot: &Client, health: f32) -> bool {
    let s = store();
    if health <= s.num(setting::HEALTH) {
        return true;
    }
    if s.flag(setting::ELYTRA) && bot.component::<FallFlying>().is_ok_and(|f| **f) {
        return true;
    }
    if s.flag(setting::FALL) && fall_damage(bot) >= health {
        return true;
    }
    s.flag(setting::EXPLOSION) && explosion_damage(bot) >= health
}

fn fall_damage(bot: &Client) -> f32 {
    let (fallen, falling) = {
        let Ok(p) = bot.component::<Physics>() else {
            return 0.0;
        };
        (p.fall_distance, !p.on_ground() && p.velocity.y < 0.0)
    };
    if !falling {
        return 0.0;
    }
    let Ok(feet) = bot.position() else {
        return 0.0;
    };
    match landing(bot, feet) {
        Landing::Ground(drop) => fall_formula(fallen + drop),
        Landing::Fluid(_) => 0.0,
        Landing::None => f32::INFINITY,
    }
}

pub(super) enum Landing {
    Ground(f64),
    Fluid(f64),
    None,
}

pub(super) fn landing(bot: &Client, feet: AzVec3) -> Landing {
    let Ok(world) = bot.world() else {
        return Landing::None;
    };
    let world = world.read();
    let hit = clip(
        &world.chunks,
        ClipContext {
            from: feet,
            to: AzVec3 {
                x: feet.x,
                y: feet.y - FALL_SEARCH,
                z: feet.z,
            },
            block_shape_type: BlockShapeType::Collider,
            fluid_pick_type: FluidPickType::Any,
            held_item: azalea_registry::builtin::ItemKind::Air,
            entity: None,
        },
    );
    if hit.miss {
        return Landing::None;
    }
    let fluid = world
        .get_block_state(hit.block_pos)
        .is_some_and(|st| FluidState::from(st).kind != FluidKind::Empty);
    if fluid {
        Landing::Fluid(feet.y - hit.location.y)
    } else {
        Landing::Ground(feet.y - hit.location.y)
    }
}

fn fall_formula(distance: f64) -> f32 {
    (distance + 1.0e-6 - SAFE_FALL).floor().max(0.0) as f32
}

fn explosion_damage(bot: &Client) -> f32 {
    let Ok(feet) = bot.position() else {
        return 0.0;
    };
    let feet = [feet.x, feet.y, feet.z];
    let mut total = 0.0;
    super::nearby::each(bot, |e| {
        let power = match e.kind {
            EntityKind::EndCrystal => 6.0,
            EntityKind::Tnt | EntityKind::TntMinecart => 4.0,
            EntityKind::Creeper => {
                let swelling = e.world.get::<SwellDir>(e.entity).is_some_and(|d| d.0 > 0);
                if !swelling {
                    return;
                }
                let charged = e.world.get::<IsPowered>(e.entity).is_some_and(|p| p.0);
                if charged { 6.0 } else { 3.0 }
            }
            _ => return,
        };
        let b = &e.physics.bounding_box;
        let at = [
            (b.min.x + b.max.x) * 0.5,
            b.min.y,
            (b.min.z + b.max.z) * 0.5,
        ];
        total += crate::util::combat::blast(power, distance(feet, at), 1.0);
    });
    total * HARD
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fall_damage_starts_past_three_blocks() {
        assert_eq!(fall_formula(3.0), 0.0);
        assert_eq!(fall_formula(3.9), 0.0);
        assert_eq!(fall_formula(4.0), 1.0);
        assert_eq!(fall_formula(23.5), 20.0);
        assert_eq!(fall_formula(0.0), 0.0);
    }
}
