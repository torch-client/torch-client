use std::sync::Mutex;

use azalea::Client;
use azalea::attack::AttackQueued;
use azalea::entity::{EntityKindComponent, Physics};
use azalea_core::aabb::Aabb;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::hit_result::{BlockHitResult, HitResult};
use azalea_core::position::Vec3;
use azalea_registry::builtin::{BlockKind, EntityKind};

use super::registry::{Id, crystal_macro as setting};
use super::{Edge, Phase, delay_ticks, store};
use crate::play::interaction::{is_using_item, module_use_on};
use crate::session::{Gamemode, InteractionHand, SharedMutex};

const RETRY_TICKS: i64 = 4;
const PENDING_TICKS: i64 = 20;

static EDGE: Edge = Edge::new(Id::CrystalMacro);

static RUN: Mutex<Run> = Mutex::new(Run::IDLE);

struct Run {
    next_place: i64,
    next_break: i64,
    last_hit: Option<(i32, i64)>,
    pending: Option<(azalea_core::position::BlockPos, i64)>,
    clock: i64,
}

impl Run {
    const IDLE: Run = Run {
        next_place: 0,
        next_break: 0,
        last_hit: None,
        pending: None,
        clock: 0,
    };
}

pub fn tick(
    bot: &Client,
    shared: &SharedMutex,
    crosshair: Option<&HitResult>,
    held: bool,
    hands_busy: bool,
) -> bool {
    let s = store();
    match EDGE.poll(s) {
        Phase::Off => return false,
        Phase::Stopped => {
            *RUN.lock().unwrap() = Run::IDLE;
            return false;
        }
        Phase::Started => *RUN.lock().unwrap() = Run::IDLE,
        Phase::Running => {}
    }
    let now = {
        let mut run = RUN.lock().unwrap();
        run.clock += 1;
        run.clock
    };
    if !held || hands_busy || bot.is_mining() || is_using_item() {
        return false;
    }
    {
        let st = shared.lock().unwrap();
        if !st.in_world
            || st.screen_open
            || st.session.dead
            || st.session.gamemode == Gamemode::Spectator
        {
            return false;
        }
        let held_crystal = st
            .session
            .hotbar
            .get(st.session.hotbar_selected as usize)
            .is_some_and(|x| x.item == "end_crystal");
        if !held_crystal {
            return false;
        }
    }

    let mut run = RUN.lock().unwrap();
    match crosshair {
        Some(HitResult::Entity(e)) => {
            if !bot
                .entity_component::<EntityKindComponent>(e.entity)
                .is_ok_and(|k| k.0 == EntityKind::EndCrystal)
            {
                return false;
            }
            let Ok(id) = bot
                .entity_component::<MinecraftEntityId>(e.entity)
                .map(|id| id.0)
            else {
                return false;
            };
            let fresh = !run
                .last_hit
                .is_some_and(|(last, at)| last == id && now - at < RETRY_TICKS);
            if fresh && now >= run.next_break && bot.component::<AttackQueued>().is_err() {
                bot.attack(e.entity);
                crate::client::tick::swing_local_player(false, shared);
                run.last_hit = Some((id, now));
                run.pending = None;
                run.next_break = now + delay_ticks(s.range(setting::BREAK_DELAY));
            }
            true
        }
        Some(HitResult::Block(hit)) if !hit.miss => {
            if !can_place(bot, hit) {
                return false;
            }
            let waiting = run
                .pending
                .is_some_and(|(base, at)| base == hit.block_pos && now - at < PENDING_TICKS);
            if now >= run.next_place && !waiting {
                module_use_on(bot, shared, hit, InteractionHand::Main);
                run.pending = Some((hit.block_pos, now));
                run.next_place = now + delay_ticks(s.range(setting::PLACE_DELAY));
            }
            true
        }
        _ => false,
    }
}

fn can_place(bot: &Client, hit: &BlockHitResult) -> bool {
    let above = hit.block_pos.up(1);
    {
        let Ok(world) = bot.world() else {
            return false;
        };
        let w = world.read();
        let base = w.get_block_state(hit.block_pos).map(BlockKind::from);
        if !matches!(base, Some(BlockKind::Obsidian | BlockKind::Bedrock)) {
            return false;
        }
        let air = w.get_block_state(above).map(BlockKind::from);
        if !matches!(
            air,
            Some(BlockKind::Air | BlockKind::CaveAir | BlockKind::VoidAir)
        ) {
            return false;
        }
    }
    let min = Vec3::new(above.x as f64, above.y as f64, above.z as f64);
    let cells = Aabb {
        min,
        max: min + Vec3::new(1.0, 2.0, 1.0),
    };
    if bot
        .component::<Physics>()
        .is_ok_and(|p| p.bounding_box.intersects_aabb(&cells))
    {
        return false;
    }
    let mut blocked = false;
    super::nearby::each(bot, |e| {
        blocked |= e.physics.bounding_box.intersects_aabb(&cells);
    });
    !blocked
}
