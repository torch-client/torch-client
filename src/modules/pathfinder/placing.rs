use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use azalea::entity::inventory::Inventory;
use azalea::pathfinder::PlaceRules;
use azalea::{BlockPos, Client, Vec3};
use azalea_core::direction::Direction;
use azalea_inventory::ItemStack;
use azalea_registry::builtin::ItemKind;

use super::{BUSY, GOING, IDLE, Track, turn_ticks};
use crate::lang::Key;
use crate::modules::auto_mine::shared::{REACH, gate};
use crate::modules::list::{BitList, Built, Entry};
use crate::modules::place::stock::{self, Held};
use crate::modules::place::{self, Click, Fired, Want};
use crate::session::SharedMutex;

pub const PLACE_COST: f32 = 20.0;

const SPARE: u8 = 8;

const JUMP_TICKS: u32 = 12;

const PLACE_TRIES: u32 = 3;

const HOLD_TICKS: u32 = 2;

fn stall_ticks() -> u32 {
    turn_ticks() + PLACE_TRIES * JUMP_TICKS + HOLD_TICKS
}

const THROWAWAY_DEFAULTS: [&str; 4] = ["dirt", "cobblestone", "netherrack", "stone"];

static GROUPS: [Key; 1] = [Key::of("lists.throwaway.group.blocks")];

fn build_throwaway() -> Built {
    let entries = crate::modules::esp::blocks::ids()
        .iter()
        .map(|&id| Entry {
            group: 0,
            label: id.into(),
            shown: Key::NONE,
            on: THROWAWAY_DEFAULTS.contains(&id),
            color: 0,
        })
        .collect();
    (entries, Vec::new())
}

pub static THROWAWAY: BitList = BitList::new(
    "pathfinder_throwaway",
    Key::of("lists.throwaway.title"),
    214.0,
    &GROUPS,
    false,
    crate::modules::esp::blocks::PRESETS,
    build_throwaway,
);

pub fn throwaway(bot: &Client) -> Option<ItemKind> {
    let inv = bot.component::<Inventory>().ok()?;
    let menu = &inv.inventory_menu;
    (0..THROWAWAY.count())
        .filter(|&i| THROWAWAY.enabled(i))
        .find_map(|i| {
            let label = THROWAWAY.label(i);
            menu.player_slots_without_hotbar_range()
                .chain(menu.hotbar_slots_range())
                .filter_map(|s| menu.slot(s))
                .filter(|s| s.is_present())
                .map(|s| s.kind())
                .find(|k| k.to_str().strip_prefix("minecraft:") == Some(label))
        })
}

#[derive(Debug)]
struct Throwaway {
    cost: f32,
}

impl PlaceRules for Throwaway {
    fn place_cost(&self, _: BlockPos) -> f32 {
        self.cost
    }
}

pub(super) fn user_rules(bot: &Client) -> Arc<dyn PlaceRules> {
    let cost = if throwaway(bot).is_some() {
        PLACE_COST
    } else {
        f32::INFINITY
    };
    Arc::new(Throwaway { cost })
}

static PLACING: AtomicBool = AtomicBool::new(false);

static FAILED: AtomicBool = AtomicBool::new(false);

struct Ask {
    want: Want,
    item: ItemKind,
    click: Option<Click>,
    ready: bool,
}

struct PlaceAt {
    ask: Option<Ask>,
    track: Option<Track>,
}

static PLACE_AT: Mutex<PlaceAt> = Mutex::new(PlaceAt {
    ask: None,
    track: None,
});

fn place_at() -> MutexGuard<'static, PlaceAt> {
    PLACE_AT.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn placing() -> bool {
    PLACING.load(Relaxed)
}

pub(super) fn clear(route_over: bool) {
    if !route_over && !PLACING.load(Relaxed) {
        return;
    }
    PLACING.store(false, Relaxed);
    let mut p = place_at();
    p.ask = None;
    if route_over {
        p.track = None;
        FAILED.store(false, Relaxed);
    }
}

pub(super) fn take_failed() -> bool {
    FAILED.swap(false, Relaxed)
}

fn fail() {
    FAILED.store(true, Relaxed);
    BUSY.store(true, Relaxed);
}

fn choose(bot: &Client, cell: BlockPos) -> Option<(Want, ItemKind)> {
    if let Some(state) = crate::modules::builder::route_want(cell)
        && let Some(item) = stock::item_for(state)
        && stock::count(bot, item) > 0
    {
        return Some((
            Want {
                cell,
                state: Some(state),
            },
            item,
        ));
    }
    throwaway(bot).map(|item| (Want { cell, state: None }, item))
}

pub(super) fn point(bot: &Client, cell: BlockPos, eye: Option<[f64; 3]>) -> (Vec3, bool) {
    let centre = Vec3::new(
        cell.x as f64 + 0.5,
        cell.y as f64 + 0.5,
        cell.z as f64 + 0.5,
    );
    PLACING.store(true, Relaxed);

    let known = place_at()
        .ask
        .as_ref()
        .filter(|a| a.want.cell == cell)
        .map(|a| (a.want, a.item, a.click.clone()));
    let (want, item, kept) = match known {
        Some(k) => k,
        None => {
            let Some((want, item)) = choose(bot, cell) else {
                fail();
                return (centre, true);
            };
            let mut p = place_at();
            Track::ask(&mut p.track, cell);
            p.ask = Some(Ask {
                want,
                item,
                click: None,
                ready: false,
            });
            (want, item, None)
        }
    };

    let (want, item, ready) = match stock::hold(bot, item, SPARE) {
        Held::Ready(_) => (want, item, true),
        Held::Moving => (want, item, false),
        Held::Missing => match choose(bot, cell) {
            Some((want, item)) => (want, item, false),
            None => {
                fail();
                return (centre, true);
            }
        },
    };

    let click = match (eye, bot.world()) {
        (Some(eye), Ok(world)) => {
            let world = world.read();
            let found =
                place::candidates(&world, eye, REACH, &ItemStack::new(item, 1), &want, true);
            let same = |c: &Click, k: &Click| {
                c.hit.block_pos == k.hit.block_pos && c.hit.direction == k.hit.direction
            };
            match kept {
                Some(k) if found.iter().any(|c| same(c, &k)) => Some(k),
                _ => found.into_iter().next(),
            }
        }
        _ => kept,
    };

    let limit = stall_ticks();
    let failed = {
        let mut p = place_at();
        if let Some(a) = p.ask.as_mut() {
            a.want = want;
            a.item = item;
            a.click = click.clone();
            a.ready = ready;
        }
        p.track.as_mut().is_some_and(|t| t.step(false, limit))
    };
    if failed {
        fail();
    }

    match click {
        Some(c) => (
            c.hit.location,
            c.hit.direction == Direction::Up && want.state.is_none(),
        ),
        None => (centre, true),
    }
}

pub fn place_tick(bot: &Client, shared: &SharedMutex) {
    if !PLACING.load(Relaxed) || !GOING.load(Relaxed) || IDLE.load(Relaxed) || FAILED.load(Relaxed)
    {
        return;
    }
    let want = match &place_at().ask {
        Some(a) if a.ready => a.want,
        _ => return,
    };
    if gate(shared, false).is_none() || place::fire(bot, shared, &want, REACH) != Fired::Placed {
        return;
    }
    crate::modules::builder::note_route_place(want.cell);
    PLACING.store(false, Relaxed);
    let mut p = place_at();
    p.ask = None;
    if let Some(t) = p.track.as_mut().filter(|t| t.block == want.cell) {
        t.met = true;
        t.idle = 0;
    }
}
