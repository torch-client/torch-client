mod search;

use std::sync::Mutex;

use azalea::Client;
use azalea::attack::AttackQueued;
use azalea::entity::Physics;
use azalea::entity::inventory::Inventory;
use azalea_core::position::BlockPos;
use azalea_inventory::ItemStack;
use azalea_inventory::components::EquipmentSlotGroup as Group;
use azalea_registry::builtin::ItemKind;

use self::search::{Body, Crystal, Rules, Scene, click_on, top_click};
use super::registry::{Id, crystal_aura as setting, options};
use super::sent::{Look, look_at, move_fix, ray_hits, sent_ray, sent_view};
use super::{Edge, MS_PER_TICK, Phase, Store, roll, store, turn};
use crate::play::interaction::{module_use_on, select_hotbar_slot};
use crate::renderer::world_draw::{self, Node};
use crate::session::{Gamemode, InteractionHand, SharedMutex};
use crate::util::combat::{Defense, Worn};
use crate::util::mth::wrap_degrees;

options! {
    pub enum CrystalSwitch {
        Normal = "Normal",
        Silent = "Silent",
        Off = "Off",
    }
}

const TICK: f32 = MS_PER_TICK / 1000.0;
const BLOCK_REACH: f32 = 4.5;
const ENTITY_REACH: f32 = 3.0;
const RETRY_TICKS: i64 = 4;
const OWN_TICKS: i64 = 20;
const OWNER: &str = "crystal_aura";
const BOX_ARGB: u32 = 0x2DFF_FFFF;

static EDGE: Edge = Edge::new(Id::CrystalAura);

static RUN: Mutex<Run> = Mutex::new(Run::IDLE);

struct Run {
    aim: Option<[f64; 3]>,
    spot: Option<Spot>,
    silent: Option<Look>,
    turn: f32,
    next_place: i64,
    next_break: i64,
    restore: Option<Restore>,
    hits: Vec<Hit>,
    placed: Vec<(BlockPos, i64)>,
    base: Option<BlockPos>,
    shown: Option<BlockPos>,
}

impl Run {
    const IDLE: Run = Run {
        aim: None,
        spot: None,
        silent: None,
        turn: 0.0,
        next_place: 0,
        next_break: 0,
        restore: None,
        hits: Vec::new(),
        placed: Vec::new(),
        base: None,
        shown: None,
    };
}

#[derive(Clone, Copy, PartialEq)]
enum Spot {
    Base(BlockPos),
    Crystal(i32),
}

#[derive(Clone, Copy)]
struct Restore {
    home: u8,
    taken: u8,
    at: i64,
}

struct Hit {
    id: i32,
    count: u8,
    at: i64,
}

#[derive(Clone, Copy)]
enum Hand {
    Held,
    Off,
    Slot(u8),
}

pub(super) struct Me {
    pub eye: [f64; 3],
    pub body: Body,
    pub health: f32,
    pub defense: Defense,
    selected: u8,
    crystals: Option<Hand>,
    weapon: Option<u8>,
    weak: bool,
}

enum Plan {
    Idle,
    Break(Crystal, Option<[f64; 3]>),
    Place {
        base: BlockPos,
        aim: [f64; 3],
        click: Option<azalea_core::hit_result::BlockHitResult>,
    },
}

pub fn enabled() -> bool {
    store().enabled(Id::CrystalAura)
}

pub fn steer(bot: &Client, yaw: f32, pitch: f32, flags: u8) -> Option<(f32, f32, u8)> {
    let camera = Look { yaw, pitch };
    let mut run = RUN.lock().unwrap();
    let want = match run.aim {
        Some(aim) => {
            let Ok(eye) = bot.eye_position() else {
                run.silent = None;
                return None;
            };
            look_at([eye.x, eye.y, eye.z], aim)
        }
        None if run.silent.is_some() => camera,
        None => return None,
    };
    let from = run.silent.unwrap_or(camera);
    let next = Look {
        yaw: from.yaw + turn::step(wrap_degrees(want.yaw - from.yaw), run.turn, TICK),
        pitch: (from.pitch + turn::step(want.pitch - from.pitch, run.turn, TICK))
            .clamp(-90.0, 90.0),
    };
    if run.aim.is_none()
        && wrap_degrees(next.yaw - yaw).abs() < turn::DEADZONE
        && (next.pitch - pitch).abs() < turn::DEADZONE
    {
        run.silent = None;
        return None;
    }
    run.silent = Some(next);
    let flags = if store().flag(setting::MOVE_FIX) {
        move_fix(flags, yaw, next.yaw)
    } else {
        flags
    };
    Some((next.yaw, next.pitch, flags))
}

pub fn tick(bot: &Client, shared: &SharedMutex) {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return,
        Phase::Stopped => {
            let run = std::mem::replace(&mut *RUN.lock().unwrap(), Run::IDLE);
            if let Some(r) = run.restore {
                put_back(bot, r);
            }
            world_draw::clear(OWNER);
            return;
        }
        Phase::Started => *RUN.lock().unwrap() = Run::IDLE,
        Phase::Running => {}
    }

    let now = crate::client::tracking::game_time();
    let mut run = RUN.lock().unwrap();
    let busy = act(bot, shared, s, &mut run, now);

    if !busy
        && let Some(r) = run.restore
        && r.at < now
    {
        put_back(bot, r);
        run.restore = None;
    }

    let shown = if s.flag(setting::RENDER) {
        run.base
    } else {
        None
    };
    if shown != run.shown {
        run.shown = shown;
        match shown {
            Some(b) => {
                let min = [b.x as f64, b.y as f64, b.z as f64];
                let max = [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0];
                world_draw::set(OWNER, vec![Node::boxed(min, max, BOX_ARGB, false)]);
            }
            None => world_draw::clear(OWNER),
        }
    }
}

fn act(bot: &Client, shared: &SharedMutex, s: &Store, run: &mut Run, now: i64) -> bool {
    let strict = s.flag(setting::STRICT);
    run.base = None;
    let Some(me) = me(bot, shared, s) else {
        idle(run);
        return false;
    };
    let cap = |v: f32, limit: f32| (if strict { v.min(limit) } else { v }) as f64;
    let place_reach = cap(s.num(setting::PLACE_RANGE), BLOCK_REACH);
    let break_reach = cap(s.num(setting::BREAK_RANGE), ENTITY_REACH);
    let rules = Rules {
        target_range: s.num(setting::TARGET_RANGE) as f64,
        min_damage: s.num(setting::MIN_DAMAGE),
        max_self: s.num(setting::MAX_SELF),
        anti_suicide: s.flag(setting::ANTI_SUICIDE),
        face: s
            .flag(setting::FACE_PLACE)
            .then(|| (s.num(setting::FACE_HEALTH), s.num(setting::FACE_DURABILITY))),
        difficulty: crate::client::tracking::difficulty(),
    };

    let scene = Scene::gather(bot, &me, &rules, place_reach.max(break_reach));
    run.hits
        .retain(|h| scene.crystals.iter().any(|c| c.id == h.id));
    run.placed.retain(|&(_, at)| now - at < OWN_TICKS);
    if scene.targets.is_empty() {
        idle(run);
        return false;
    }

    let plan = {
        let attempts = s.num(setting::ATTEMPTS) as u8;
        let hits = &run.hits;
        let gone = |id: i32| hits.iter().any(|h| h.id == id && now - h.at < RETRY_TICKS);
        let spent = |id: i32| gone(id) || hits.iter().any(|h| h.id == id && h.count >= attempts);
        let sent = sent_view(bot);
        let Ok(world) = bot.world() else {
            return false;
        };
        let w = world.read();
        let crystal = if s.flag(setting::BREAK) {
            scene.best_crystal(&w, &me, &rules, break_reach, !strict, &spent)
        } else {
            None
        };
        match crystal {
            Some((c, aim)) => Plan::Break(c, aim),
            None if s.flag(setting::PLACE) && me.crystals.is_some() => {
                match scene.best_base(&w, &me, &rules, place_reach, !strict, &gone) {
                    Some((base, aim, _)) => {
                        let click = if strict {
                            sent.and_then(|(eye, look)| click_on(&w, eye, look, base, place_reach))
                        } else {
                            Some(top_click(base))
                        };
                        Plan::Place { base, aim, click }
                    }
                    None => Plan::Idle,
                }
            }
            None => Plan::Idle,
        }
    };

    match plan {
        Plan::Idle => {
            idle(run);
            false
        }
        Plan::Break(c, aim) => {
            face(run, s, Spot::Crystal(c.id), aim);
            let own =
                s.flag(setting::FAST_BREAK) && run.placed.iter().any(|&(cell, _)| cell == c.cell());
            if now < run.next_break && !own {
                return true;
            }
            if bot.component::<AttackQueued>().is_ok() {
                return true;
            }
            if strict
                && !sent_ray(bot)
                    .is_some_and(|(o, d)| ray_hits(o, d, break_reach, c.body.min, c.body.max))
            {
                return true;
            }
            if me.weak && s.flag(setting::ANTI_WEAKNESS) {
                match me.weapon {
                    Some(slot) if slot != me.selected => take(bot, run, me.selected, slot, now),
                    Some(_) => {}
                    None => return true,
                }
            }
            bot.ecs
                .write()
                .entity_mut(bot.entity)
                .insert(AttackQueued { target: c.entity });
            crate::client::tick::swing_local_player(false, shared);
            match run.hits.iter_mut().find(|h| h.id == c.id) {
                Some(h) => {
                    h.count = h.count.saturating_add(1);
                    h.at = now;
                }
                None => run.hits.push(Hit {
                    id: c.id,
                    count: 1,
                    at: now,
                }),
            }
            run.placed.retain(|&(cell, _)| cell != c.cell());
            run.next_break = now + delay_ticks(s.range(setting::BREAK_DELAY));
            true
        }
        Plan::Place { base, aim, click } => {
            run.base = Some(base);
            face(run, s, Spot::Base(base), Some(aim));
            if now < run.next_place {
                return true;
            }
            let Some(hit) = click else {
                return true;
            };
            let silent = s.choice(setting::SWITCH) == CrystalSwitch::Silent;
            let hand = match me.crystals {
                Some(Hand::Held) => InteractionHand::Main,
                Some(Hand::Off) => InteractionHand::Off,
                Some(Hand::Slot(slot)) => {
                    if silent {
                        take(bot, run, me.selected, slot, now);
                    } else {
                        select_hotbar_slot(bot, slot);
                    }
                    InteractionHand::Main
                }
                None => return false,
            };
            module_use_on(bot, shared, &hit, hand);
            if !strict
                && silent
                && let Some(r) = run.restore.take()
            {
                put_back(bot, r);
            }
            run.placed.push((base.up(1), now));
            run.next_place = now + delay_ticks(s.range(setting::PLACE_DELAY));
            true
        }
    }
}

fn idle(run: &mut Run) {
    run.aim = None;
    run.spot = None;
}

fn face(run: &mut Run, s: &Store, spot: Spot, aim: Option<[f64; 3]>) {
    if run.spot != Some(spot) {
        let (lo, hi) = s.range(setting::TURN);
        run.turn = roll(lo, hi);
        run.spot = Some(spot);
    }
    if aim.is_some() {
        run.aim = aim;
    }
}

fn take(bot: &Client, run: &mut Run, selected: u8, slot: u8, now: i64) {
    let home = run.restore.map_or(selected, |r| r.home);
    select_hotbar_slot(bot, slot);
    run.restore = (home != slot).then_some(Restore {
        home,
        taken: slot,
        at: now,
    });
}

fn put_back(bot: &Client, r: Restore) {
    if bot.selected_hotbar_slot().is_ok_and(|s| s == r.taken) {
        select_hotbar_slot(bot, r.home);
    }
}

fn delay_ticks((lo, hi): (f32, f32)) -> i64 {
    (roll(lo, hi) / MS_PER_TICK).round() as i64
}

fn me(bot: &Client, shared: &SharedMutex, s: &Store) -> Option<Me> {
    if s.flag(setting::PAUSE_EAT) && crate::play::interaction::is_using_item() {
        return None;
    }
    let (health, resistance, weak, selected, hotbar_crystals, weapon) = {
        let st = shared.lock().unwrap();
        if !st.in_world
            || st.screen_open
            || st.session.dead
            || st.session.gamemode == Gamemode::Spectator
        {
            return None;
        }
        let effects = &st.session.active_effects;
        let level =
            |id: &str| crate::play::mob_effects::find(effects, id).map_or(0, |e| e.amplifier + 1);
        let weak = 1 + 3 * level("strength") - 4 * level("weakness") <= 0;
        let resistance = crate::play::mob_effects::find(effects, "resistance").map(|e| e.amplifier);
        let selected = st.session.hotbar_selected;
        let item = |i: u8| {
            st.session
                .hotbar
                .get(i as usize)
                .map_or("", |stack| stack.item)
        };
        let is_crystal = |i: u8| item(i) == "end_crystal";
        let crystals = if is_crystal(selected) {
            Some(selected)
        } else {
            (0..9u8).find(|&i| is_crystal(i))
        };
        let is_weapon = |i: u8| super::triggerbot::is_weapon(item(i));
        let weapon = if is_weapon(selected) {
            Some(selected)
        } else {
            (0..9u8).find(|&i| is_weapon(i))
        };
        let health = st.session.health + st.session.health_display.absorption;
        (health, resistance, weak, selected, crystals, weapon)
    };

    let (offhand, armor) = {
        let inv = bot.component::<Inventory>().ok()?;
        let menu = &inv.inventory_menu;
        let offhand = menu
            .slot(45)
            .is_some_and(|st| st.kind() == ItemKind::EndCrystal);
        let armor: [ItemStack; 4] =
            std::array::from_fn(|i| menu.slot(5 + i).cloned().unwrap_or(ItemStack::Empty));
        (offhand, armor)
    };
    let worn: [Worn; 4] = {
        let world = bot.world().ok()?;
        let w = world.read();
        const SLOTS: [Group; 4] = [Group::Head, Group::Chest, Group::Legs, Group::Feet];
        std::array::from_fn(|i| Worn::of(&armor[i], SLOTS[i], Some(&w.registries)))
    };

    let crystals = match hotbar_crystals {
        Some(i) if i == selected => Some(Hand::Held),
        _ if offhand => Some(Hand::Off),
        Some(i) if s.choice(setting::SWITCH) != CrystalSwitch::Off => Some(Hand::Slot(i)),
        _ => None,
    };

    let eye = bot.eye_position().ok()?;
    let bb = bot.component::<Physics>().ok()?.bounding_box;
    Some(Me {
        eye: [eye.x, eye.y, eye.z],
        body: Body::of(&bb),
        health,
        defense: Defense::of(&worn, resistance),
        selected,
        crystals,
        weapon,
        weak,
    })
}
