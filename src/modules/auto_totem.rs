use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering::Relaxed};

use azalea::Client;
use azalea::block::fluid_state::{FluidKind, FluidState};
use azalea::entity::Physics;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::{FallFlying, IsPowered, SwellDir};
use azalea::inventory::ContainerClickEvent;
use azalea::physics::clip::{BlockShapeType, ClipContext, FluidPickType, clip};
use azalea_core::position::Vec3 as AzVec3;
use azalea_inventory::ItemStack;
use azalea_inventory::operations::SwapClick;
use azalea_registry::builtin::{EntityKind, ItemKind};

use super::registry::{Id, auto_totem as setting, options};
use super::{MS_PER_TICK, roll, store};
use crate::session::{Gamemode, SharedMutex};

options! {
    pub enum TotemMode {
        Always = "Always",
        Smart = "Smart",
    }
}

const OFFHAND: u8 = 40;

const SAFE_FALL: f64 = 3.0;

const HARD: f32 = 1.5;

const FALL_SEARCH: f64 = 384.0;

static NEXT: AtomicI64 = AtomicI64::new(0);

pub fn enabled() -> bool {
    store().enabled(Id::AutoTotem)
}

pub fn tick(bot: &Client, shared: &Arc<SharedMutex>) {
    if !enabled() {
        return;
    }
    let s = store();
    let now = crate::client::tracking::game_time();
    if now < NEXT.load(Relaxed) {
        return;
    }

    let health = {
        let st = shared.lock().unwrap();
        if !st.in_world
            || st.screen_open
            || st.session.dead
            || matches!(
                st.session.gamemode,
                Gamemode::Creative | Gamemode::Spectator
            )
        {
            return;
        }
        st.session.health + st.session.health_display.absorption
    };

    let Some((window_id, slot)) = totem_to_move(bot) else {
        return;
    };

    if s.choice(setting::MODE) == TotemMode::Smart && !in_danger(bot, health) {
        return;
    }

    bot.ecs.write().trigger(ContainerClickEvent {
        entity: bot.entity,
        window_id,
        operation: SwapClick {
            source_slot: slot,
            target_slot: OFFHAND,
        }
        .into(),
    });
    let (lo, hi) = s.range(setting::DELAY);
    NEXT.store(now + (roll(lo, hi) / MS_PER_TICK).round() as i64, Relaxed);
}

fn totem_to_move(bot: &Client) -> Option<(i32, u16)> {
    let inv = bot.component::<Inventory>().ok()?;
    if inv.container_menu.is_some() {
        return None;
    }
    let menu = &inv.inventory_menu;
    if is_totem(&menu.as_player().offhand) {
        return None;
    }
    let slot = menu
        .player_slots_without_hotbar_range()
        .chain(menu.hotbar_slots_range())
        .find(|&i| menu.slot(i).is_some_and(is_totem))?;
    Some((inv.id, slot as u16))
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
        Landing::Fluid => 0.0,
        Landing::None => f32::INFINITY,
    }
}

enum Landing {
    Ground(f64),
    Fluid,
    None,
}

fn landing(bot: &Client, feet: AzVec3) -> Landing {
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
        },
    );
    if hit.miss {
        return Landing::None;
    }
    let fluid = world
        .get_block_state(hit.block_pos)
        .is_some_and(|st| FluidState::from(st).kind != FluidKind::Empty);
    if fluid {
        Landing::Fluid
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
        total += blast(power, distance(feet, at));
    });
    total * HARD
}

fn blast(power: f32, distance: f64) -> f32 {
    let reach = power as f64 * 2.0;
    let d = distance / reach;
    if d > 1.0 {
        return 0.0;
    }
    let p = 1.0 - d;
    ((p * p + p) / 2.0 * 7.0 * reach + 1.0) as f32
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blast_matches_vanilla() {
        assert_eq!(blast(6.0, 0.0), 85.0);
        assert_eq!(blast(6.0, 12.0), 1.0);
        assert_eq!(blast(6.0, 12.01), 0.0);
        assert_eq!(blast(4.0, 4.0), 22.0);
    }

    #[test]
    fn fall_damage_starts_past_three_blocks() {
        assert_eq!(fall_formula(3.0), 0.0);
        assert_eq!(fall_formula(3.9), 0.0);
        assert_eq!(fall_formula(4.0), 1.0);
        assert_eq!(fall_formula(23.5), 20.0);
        assert_eq!(fall_formula(0.0), 0.0);
    }
}
