use azalea::prelude::*;
use azalea::{SprintDirection, WalkDirection};
use azalea_core::{direction::Direction, position::BlockPos as AzBlockPos};
use azalea_inventory::ItemStack;
use azalea_protocol::packets::game::{
    s_chat_command::ServerboundChatCommand,
    s_client_command::{Action as ClientCommand, ServerboundClientCommand},
    s_command_suggestion::ServerboundCommandSuggestion,
    s_edit_book::ServerboundEditBook,
    s_player_action::{Action, ServerboundPlayerAction},
    s_player_command::{Action as PlayerCommand, ServerboundPlayerCommand},
    s_set_command_block::ServerboundSetCommandBlock,
    s_sign_update::ServerboundSignUpdate,
    s_teleport_to_entity::ServerboundTeleportToEntity,
};
use std::collections::HashSet;
use std::sync::Arc;

use crate::client::tracking::{
    GAME_TIME, LOCAL_ANIM_ID, anim_states, equipment, head_yaws, set_current_world,
};
use crate::client::{chat_sign, chat_text, viewwindow, worldsync};
use crate::play::entity_feed::{bed_orientation_at, collect_entities};
use crate::play::interaction;
use crate::play::inventory_bridge::{
    apply_inv_actions, container_kind, potion_contents, slot_stack,
};
use crate::renderer::{AnimInput, HumanoidAnim};
use crate::session::{
    ContainerKind, DropRequest, Gamemode, InvAction, OtherPlayerInfo, SharedMutex, SlotStack,
};
use crate::{blockentities, renderer};

struct TickSnapshot {
    pos: azalea::Vec3,
    other_positions: Vec<OtherPlayerInfo>,
    local_anim: HumanoidAnim,
    #[cfg(feature = "skins")]
    local_skin: crate::client::skins::SkinState,
    entity_anims: std::sync::Arc<Vec<crate::entities::feed::EntityAnim>>,
    menu_slots: Vec<SlotStack>,
    hotbar: Arc<[SlotStack]>,
    armor: crate::renderer::Armor,
    carried: SlotStack,
    container_id: i32,
    container_kind: ContainerKind,
    container_title: std::sync::Arc<[crate::text::Span]>,
    hotbar_sel: u8,
    health: f32,
    dead: bool,
    death_score: i32,
    food: u32,
    saturation: f32,
    air_supply: i32,
    health_display: crate::session::HealthDisplay,
    jumpable_mount: bool,
    active_effects: Vec<crate::play::mob_effects::MobEffectInstance>,
    gamemode: Gamemode,
    crouching: bool,
    fall_flying: bool,
    swimming: bool,
    sleeping: bool,
    bed_orientation: Option<crate::direction::Direction>,
    attack_strength: f32,
    attack_delay: f32,
    fov_modifier: f32,
    xp_progress: f32,
    xp_level: u32,
}

fn read_tick_snapshot(bot: &Client, shared: &Arc<SharedMutex>) -> Option<TickSnapshot> {
    if let Ok(world) = bot.world() {
        set_current_world(&world);
    }

    let pos = bot.position().ok()?;
    let local_sleeping_pos = bot
        .component::<azalea::entity::metadata::SleepingPos>()
        .ok()
        .and_then(|c| c.0);
    let pos = match local_sleeping_pos {
        Some(bed) => azalea::Vec3 {
            x: bed.x as f64 + 0.5,
            y: bed.y as f64 + 0.6875,
            z: bed.z as f64 + 0.5,
        },
        None => pos,
    };
    if let Ok(partial) = bot.partial_world() {
        viewwindow::keep_view_center_on_the_player(&partial, pos);
        crate::client::mesh_worker::sweep_parked(&partial);
    }

    let (other_positions, seen_players) = collect_other_players(bot);

    let (local_anim, reads) = tick_local_anim(bot, shared, pos);
    #[cfg(feature = "skins")]
    let local_skin = local_skin(bot, reads.skin_parts);
    let entity_anims = collect_entities(bot, &seen_players);

    GAME_TIME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    crate::client::cooldowns::tick();

    let world = bot.world().ok();

    #[cfg(feature = "audio")]
    {
        use std::sync::atomic::Ordering;

        let mut last = LAST_XZ.lock().unwrap();
        let (dx, dz) = match *last {
            Some((lx, lz)) => (pos.x - lx, pos.z - lz),
            None => (0.0, 0.0),
        };
        *last = Some((pos.x, pos.z));
        drop(last);

        let on_ground = bot
            .component::<azalea::entity::Physics>()
            .is_ok_and(|p| p.on_ground());
        if on_ground {
            const STEP_DISTANCE: f32 = 2.0;
            let travelled = f32::from_bits(TRAVELLED.load(Ordering::Relaxed))
                + (dx * dx + dz * dz).sqrt() as f32;
            if travelled >= STEP_DISTANCE {
                TRAVELLED.store((travelled - STEP_DISTANCE).to_bits(), Ordering::Relaxed);
                let feet = AzBlockPos::new(
                    pos.x.floor() as i32,
                    pos.y.floor() as i32 - 1,
                    pos.z.floor() as i32,
                );
                let state = world.as_ref().and_then(|w| w.read().get_block_state(feet));
                if let Some(state) = state
                    && !state.is_air()
                {
                    use azalea::block::BlockTrait;
                    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
                    let group = crate::util::block_model::sound_group(block.id());
                    crate::audio::play_at(
                        group.step_event,
                        crate::audio::SoundCategory::Players,
                        [pos.x as f32, pos.y as f32, pos.z as f32],
                        0.15,
                        1.0,
                    );
                }
            } else {
                TRAVELLED.store(travelled.to_bits(), Ordering::Relaxed);
            }
        }
    }
    let advanced_tooltips = reads.advanced_tooltips;
    let (
        menu_slots,
        hotbar,
        armor,
        armor_value,
        carried,
        container_id,
        container_kind,
        container_title,
    ) = {
        let world_guard = world.as_ref().map(|w| w.read());
        let registries = world_guard.as_ref().map(|g| &g.registries);
        let menu_slots: Vec<SlotStack> = bot
            .menu()
            .map(|menu| {
                menu.slots()
                    .iter()
                    .map(|s| slot_stack(s, registries, advanced_tooltips))
                    .collect()
            })
            .unwrap_or_default();
        let inventory = bot.component::<azalea::entity::inventory::Inventory>();
        let player_slots = inventory
            .as_ref()
            .map(|inv| inv.inventory_menu.slots())
            .unwrap_or_default();
        let hotbar: Arc<[SlotStack]> = player_slots
            .get(36..46)
            .map(|slots| {
                slots
                    .iter()
                    .map(|s| slot_stack(s, registries, advanced_tooltips))
                    .collect()
            })
            .unwrap_or_default();
        let armor: crate::renderer::Armor =
            std::array::from_fn(|i| player_slots.get(5 + i).and_then(item_id));
        let carried = inventory
            .as_ref()
            .map(|inv| slot_stack(&inv.carried, registries, advanced_tooltips))
            .unwrap_or_default();
        let (container_id, kind, container_title) = inventory
            .as_ref()
            .map(|inv| {
                let kind = if inv.container_menu.is_some() {
                    container_kind(inv.menu())
                } else {
                    ContainerKind::None
                };
                let title = inv
                    .container_menu_title
                    .as_ref()
                    .map_or_else(crate::session::empty_spans, |t| {
                        chat_text::to_spans(t).into()
                    });
                (inv.id, kind, title)
            })
            .unwrap_or_else(|_| (0, ContainerKind::None, crate::session::empty_spans()));
        let armor_value = armor_value(&player_slots, bot.selected_hotbar_slot().unwrap_or(0));
        (
            menu_slots,
            hotbar,
            armor,
            armor_value,
            carried,
            container_id,
            kind,
            container_title,
        )
    };

    if let Some(world) = &world {
        let snapshot = {
            let world = world.read();
            blockentities::feed::publish(&world)
        };
        if let Some(snapshot) = snapshot {
            let mut s = shared.lock().unwrap();
            s.session.block_entities = snapshot;
            s.session.block_entities_version = s.session.block_entities_version.wrapping_add(1);
        }
    }

    let hotbar_sel = bot.selected_hotbar_slot().unwrap_or(0);
    let health = bot.health().unwrap_or(20.0);
    let dead = bot.component::<azalea::entity::Dead>().is_ok();
    let death_score = dead
        .then(|| {
            bot.component::<azalea::entity::metadata::Score>()
                .map(|s| s.0)
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let (food, saturation) = bot
        .hunger()
        .map(|h| (h.food, h.saturation))
        .unwrap_or((20, 5.0));
    let air_supply = {
        use azalea::entity::metadata;
        bot.component::<metadata::AirSupply>()
            .map(|c| c.0)
            .unwrap_or(crate::session::MAX_AIR_SUPPLY)
    };
    let active_effects = bot
        .component::<azalea::entity::ActiveEffects>()
        .map(|e| crate::play::mob_effects::tick_active_effects(&e))
        .unwrap_or_default();
    let gamemode = bot
        .component::<azalea_core::game_type::GameMode>()
        .ok()
        .map(|gm| Gamemode::from_azalea(*gm))
        .unwrap_or(Gamemode::Survival);
    let (health_display, jumpable_mount) = health_display(bot, &active_effects, armor_value);
    let crouching = bot
        .component::<azalea::entity::Pose>()
        .map(|p| *p == azalea::entity::Pose::Crouching)
        .unwrap_or(false);
    let (fall_flying, swimming) = {
        use azalea::entity::metadata;
        (
            bot.component::<metadata::FallFlying>()
                .map(|c| c.0)
                .unwrap_or(false),
            bot.component::<metadata::Swimming>()
                .map(|c| c.0)
                .unwrap_or(false),
        )
    };
    let sleeping = local_sleeping_pos.is_some();
    let bed_orientation = local_sleeping_pos.and_then(|pos| bed_orientation_at(bot, pos));

    let attack_delay = {
        use azalea::attack::get_attack_strength_delay;
        bot.component::<azalea::entity::Attributes>()
            .map(|a| get_attack_strength_delay(&a))
            .unwrap_or(DEFAULT_ATTACK_DELAY)
    };
    let attack_ticks = attack_strength_ticker(bot, &hotbar, hotbar_sel);
    let attack_strength = if attack_delay > 0.0 {
        (attack_ticks as f32 / attack_delay).clamp(0.0, 1.0)
    } else {
        1.0
    };

    let experience = bot.experience().unwrap_or_default();

    let fov_modifier = tick_fov(
        bot,
        &reads,
        hotbar
            .get(hotbar_sel as usize)
            .map_or("", |stack| stack.item),
        local_anim.use_ticks(),
    );

    Some(TickSnapshot {
        pos,
        other_positions,
        local_anim,
        #[cfg(feature = "skins")]
        local_skin,
        entity_anims,
        menu_slots,
        hotbar,
        armor,
        carried,
        container_id,
        container_kind,
        container_title,
        hotbar_sel,
        health,
        dead,
        death_score,
        food,
        saturation,
        air_supply,
        health_display,
        jumpable_mount,
        active_effects,
        gamemode,
        crouching,
        fall_flying,
        swimming,
        sleeping,
        bed_orientation,
        attack_strength,
        attack_delay,
        fov_modifier,
        xp_progress: experience.progress,
        xp_level: experience.level,
    })
}

const FLYING_FOV_MODIFIER: f32 = 1.1;

const BOW_DRAW_TICKS: f32 = 20.0;
const BOW_FOV_PULL: f32 = 0.15;

const SCOPING_FOV_MODIFIER: f32 = 0.1;

const FOV_SMOOTHING: f32 = 0.5;
const FOV_MODIFIER_MIN: f32 = 0.1;
const FOV_MODIFIER_MAX: f32 = 1.5;

fn fov_target(
    bot: &Client,
    main_hand: &str,
    use_ticks: u32,
    first_person: bool,
    effect_scale: f32,
) -> f32 {
    use crate::renderer::anim::{UseAnimation, use_animation};

    let mut modifier = 1.0;

    let abilities = bot.component::<azalea::entity::PlayerAbilities>();
    if abilities.as_ref().is_ok_and(|a| a.flying) {
        modifier *= FLYING_FOV_MODIFIER;
    }

    let walking_speed = abilities.map_or(0.0, |a| a.walking_speed);
    if walking_speed != 0.0
        && let Ok(attributes) = bot.component::<azalea::entity::Attributes>()
    {
        let speed = attributes.movement_speed.calculate() as f32;
        modifier *= (speed / walking_speed + 1.0) / 2.0;
    }

    if interaction::is_using_item() {
        match use_animation(main_hand) {
            UseAnimation::Bow => {
                let draw = (use_ticks as f32 / BOW_DRAW_TICKS).min(1.0);
                modifier *= 1.0 - draw * draw * BOW_FOV_PULL;
            }
            UseAnimation::Spyglass if first_person => return SCOPING_FOV_MODIFIER,
            _ => {}
        }
    }

    1.0 + (modifier - 1.0) * effect_scale
}

fn tick_fov(bot: &Client, reads: &TickReads, main_hand: &str, use_ticks: u32) -> f32 {
    let (previous, effect_scale, first_person) =
        (reads.fov_previous, reads.fov_effects, reads.first_person);
    let target = fov_target(bot, main_hand, use_ticks, first_person, effect_scale);
    (previous + (target - previous) * FOV_SMOOTHING).clamp(FOV_MODIFIER_MIN, FOV_MODIFIER_MAX)
}

const DEFAULT_ATTACK_DELAY: f32 = 5.0;

fn attack_strength_ticker(bot: &Client, hotbar: &[SlotStack], hotbar_sel: u8) -> u32 {
    use azalea::attack::TicksSinceLastAttack;

    let held = hotbar
        .get(hotbar_sel as usize)
        .map(|s| s.item)
        .unwrap_or("");
    let since_attack = bot
        .component::<TicksSinceLastAttack>()
        .map(|t| t.0)
        .unwrap_or(u32::MAX);

    let mut ticker = TICKER
        .get_or_init(|| std::sync::Mutex::new(Ticker::default()))
        .lock()
        .unwrap();
    ticker.ticks = ticker.ticks.saturating_add(1).min(since_attack);
    if ticker.main_hand != held {
        ticker.main_hand = held;
        ticker.ticks = 0;
    }
    if ATTACK_TICKER_RESET.swap(false, std::sync::atomic::Ordering::Relaxed) {
        ticker.ticks = 0;
    }
    ticker.ticks
}

static ATTACK_TICKER_RESET: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn request_attack_ticker_reset() {
    ATTACK_TICKER_RESET.store(true, std::sync::atomic::Ordering::Relaxed);
}

static MOVEMENT_WRITE_REPORTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn reset_movement_warning() {
    MOVEMENT_WRITE_REPORTED.store(false, std::sync::atomic::Ordering::Relaxed);
}

#[derive(Default)]
struct Ticker {
    ticks: u32,
    main_hand: &'static str,
}
static TICKER: std::sync::OnceLock<std::sync::Mutex<Ticker>> = std::sync::OnceLock::new();

#[cfg(feature = "audio")]
static TRAVELLED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "audio")]
static LAST_XZ: std::sync::Mutex<Option<(f64, f64)>> = std::sync::Mutex::new(None);

pub(crate) fn reset_session_locals(blocking: bool) {
    let ticker = TICKER.get_or_init(Default::default);
    let ticker = if blocking {
        ticker.lock().ok()
    } else {
        ticker.try_lock().ok()
    };
    if let Some(mut ticker) = ticker {
        *ticker = Ticker::default();
    }
    #[cfg(feature = "audio")]
    {
        TRAVELLED.store(0, std::sync::atomic::Ordering::Relaxed);
        let last = if blocking {
            LAST_XZ.lock().ok()
        } else {
            LAST_XZ.try_lock().ok()
        };
        if let Some(mut last) = last {
            *last = None;
        }
    }
}

fn note_movement_write(what: &str, e: impl std::fmt::Display) {
    if MOVEMENT_WRITE_REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    crate::log_warn!(
        "net",
        "{what} could not be applied to the local player: {e}. Movement input is no longer \
         reaching the world; this is reported once per connection."
    );
}

fn trace_menu_change(
    old: &[SlotStack],
    new: &[SlotStack],
    old_carried: &SlotStack,
    new_carried: &SlotStack,
    applied_clicks: usize,
) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var_os("MC_INV_TRACE").is_some()) {
        return;
    }
    let describe = |s: &SlotStack| {
        if s.is_empty() {
            "empty".to_string()
        } else {
            format!("{}x{}", s.item, s.count)
        }
    };
    if old_carried.item != new_carried.item || old_carried.count != new_carried.count {
        azalea::inventory::trace(format_args!(
            "[inv/pub] cursor: {} -> {} (applied {applied_clicks} click(s) this tick)",
            describe(old_carried),
            describe(new_carried)
        ));
    }
    for (i, (a, b)) in old.iter().zip(new).enumerate() {
        if a.item != b.item || a.count != b.count {
            azalea::inventory::trace(format_args!(
                "[inv/pub] slot {i}: {} -> {} (applied {applied_clicks} click(s) this tick)",
                describe(a),
                describe(b)
            ));
        }
    }
}

fn apply_local_trades(session: &mut crate::session::SessionState, actions: &[InvAction]) {
    if actions.is_empty() {
        return;
    }
    if session.container_kind != ContainerKind::Merchant {
        trace_trade("actions queued, but no merchant menu is open");
        return;
    }
    for action in actions {
        let InvAction::Click(op) = action else {
            continue;
        };
        if !crate::gui::slots::takes_slot(op, 2) {
            trace_trade("click did not take the result slot");
            continue;
        }
        let index = {
            let Some([buy_a, buy_b, result]) = session.menu_slots.get(..3) else {
                trace_trade("no payment slots published yet");
                return;
            };
            if result.is_empty() {
                trace_trade("result slot empty at click time");
                return;
            }
            let Some(offers) = session.merchant.as_ref() else {
                trace_trade("no offers");
                return;
            };
            let found = offers.active_offer(buy_a, buy_b, session.merchant_hint);
            match found {
                Some(_) => trace_trade("took a trade"),
                None => trace_trade("payment slots match no offer"),
            }
            found
        };
        if let Some(i) = index
            && let Some(offers) = session.merchant.as_mut()
        {
            Arc::make_mut(offers).notify_trade(i);
        }
    }
}

fn trace_trade(what: &str) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var_os("MC_INV_TRACE").is_some()) {
        return;
    }
    eprintln!("[trade] {what}");
}

fn block_distance_sqr(pos: AzBlockPos, point: [f64; 3]) -> f64 {
    let axis = |min: i32, p: f64| {
        let min = min as f64;
        (min - p).max(p - (min + 1.0)).max(0.0)
    };
    let (dx, dy, dz) = (
        axis(pos.x, point[0]),
        axis(pos.y, point[1]),
        axis(pos.z, point[2]),
    );
    dx * dx + dy * dy + dz * dz
}

pub(crate) fn handle_tick(bot: &Client, shared: &Arc<SharedMutex>) {
    let _ticking = crate::diag::TickGuard::new();

    let (inv_actions, horse) = {
        let mut s = shared.lock().unwrap();
        let actions = std::mem::take(&mut s.session.inv_actions);
        if let Some(hint) = actions.iter().rev().find_map(|a| match a {
            InvAction::SelectTrade(item) => Some(*item as i32),
            _ => None,
        }) {
            s.session.merchant_hint = hint;
        }
        apply_local_trades(&mut s.session, &actions);
        let horse = if actions.is_empty() {
            None
        } else {
            s.session.horse.clone()
        };
        (actions, horse)
    };
    let applied_clicks = inv_actions.len();
    let closing = inv_actions.iter().any(|a| matches!(a, InvAction::Close));
    apply_inv_actions(bot, inv_actions, horse);
    if closing {
        shared.lock().unwrap().session.horse = None;
    }

    let Some(snapshot) = read_tick_snapshot(bot, shared) else {
        return;
    };
    let TickSnapshot {
        pos,
        other_positions,
        local_anim,
        #[cfg(feature = "skins")]
        local_skin,
        entity_anims,
        menu_slots,
        hotbar,
        armor,
        carried,
        container_id,
        container_kind,
        container_title,
        hotbar_sel,
        health,
        dead,
        death_score,
        food,
        saturation,
        air_supply,
        health_display,
        jumpable_mount,
        active_effects,
        gamemode,
        crouching,
        fall_flying,
        swimming,
        sleeping,
        bed_orientation,
        attack_strength,
        attack_delay,
        fov_modifier,
        xp_progress,
        xp_level,
    } = snapshot;

    let env = crate::client::envprobe::sample(
        bot,
        [
            pos.x,
            pos.y + crate::renderer::systems::eye_height(crouching) as f64,
            pos.z,
        ],
    );

    let (
        flags,
        auto_jump_enabled,
        yaw,
        pitch,
        hotbar_target,
        spectator_teleport_target,
        pending_drop,
        sign_update,
        command_block_update,
        command_block_pos,
        sign_edit_open_pos,
        edit_book,
        outgoing_chat,
        signing_state,
        swap_hands,
    ) = {
        let mut s = shared.lock().unwrap();
        s.session.player_pos_prev = s.session.player_pos;
        s.session.player_pos = [pos.x as f32, pos.y as f32, pos.z as f32];
        s.session.last_tick_time = Some(crate::platform::time::Instant::now());
        s.session.game_time = s.session.game_time.wrapping_add(1);
        s.session.day_clock.advance();
        if let Some(env) = env {
            s.session.env_prev = s.session.env.or(Some(env));
            s.session.env = Some(env);
        }
        s.session.other_players = other_positions;
        s.session.local_anim = local_anim;
        #[cfg(feature = "skins")]
        {
            s.session.local_skin = local_skin;
        }
        s.session.fov.prev = s.session.fov.cur;
        s.session.fov.cur = fov_modifier;
        s.session.entities = entity_anims;
        s.session.status = None;
        let click_in_flight = !s.session.inv_actions.is_empty();
        let horse_open = s
            .session
            .horse
            .as_ref()
            .is_some_and(|h| !h.slots.is_empty());
        if !menu_slots.is_empty() && !click_in_flight && !horse_open {
            trace_menu_change(
                &s.session.menu_slots,
                &menu_slots,
                &s.session.carried,
                &carried,
                applied_clicks,
            );
            s.session.menu_slots = menu_slots;
        }
        if !hotbar.is_empty() && !click_in_flight {
            s.session.hotbar = hotbar;
        }
        if !click_in_flight {
            s.session.armor = armor;
        }
        if let Some(drop) = s.session.pending_drop {
            let remove_from = |slot: &mut SlotStack| {
                if slot.count == 0 {
                    return;
                }
                let remove = match drop {
                    DropRequest::HeldItem => 1,
                    DropRequest::HeldStack => slot.count,
                };
                slot.count -= remove;
                if slot.count == 0 {
                    *slot = SlotStack::default();
                }
            };
            if let Some(slot) = s.session.menu_slots.get_mut(36 + hotbar_sel as usize) {
                remove_from(slot);
            }
            if s.session
                .hotbar
                .get(hotbar_sel as usize)
                .is_some_and(|st| st.count != 0)
                && let Some(slot) =
                    Arc::make_mut(&mut s.session.hotbar).get_mut(hotbar_sel as usize)
            {
                remove_from(slot);
            }
        }
        if !click_in_flight {
            s.session.carried = carried;
        }
        if container_id != s.session.container_id {
            s.session.container_data = Default::default();
            s.session.merchant_hint = -1;
        }
        s.session.container_id = container_id;
        s.session.container_kind = container_kind;
        if let Some(horse) = s.session.horse.take() {
            if !horse.slots.is_empty() {
                s.session.container_kind = ContainerKind::Horse(horse.columns);
                s.session.container_id = horse.container_id;
                let changed = horse.state_id != s.session.horse_published_state_id
                    || s.session.menu_slots.len() != horse.slots.len();
                if changed && !click_in_flight {
                    s.session.horse_published_state_id = horse.state_id;
                    s.session.menu_slots.clear();
                    s.session.menu_slots.extend_from_slice(&horse.slots);
                    let hotbar = Arc::make_mut(&mut s.session.hotbar);
                    for (i, slot) in horse.player_slots().iter().skip(27).take(9).enumerate() {
                        if let Some(dst) = hotbar.get_mut(i) {
                            dst.clone_from(slot);
                        }
                    }
                }
            }
            s.session.horse = Some(horse);
        }
        if container_id != 0 {
            s.session.container_title = container_title;
        }
        let future_xp = match (
            container_kind,
            &s.session.merchant,
            s.session.menu_slots.get(..3),
        ) {
            (ContainerKind::Merchant, Some(offers), Some([a, b, result])) if !result.is_empty() => {
                offers.future_xp(a, b, s.session.merchant_hint)
            }
            _ => 0,
        };
        s.session.merchant_future_xp = future_xp;
        s.session.hotbar_selected = hotbar_sel;
        s.session.health = health;
        if dead && !s.session.dead && !s.session.show_death_screen {
            s.respawn_requested = true;
        }
        s.session.dead = dead;
        s.session.death_score = death_score;
        s.session.food = food;
        s.session.saturation = saturation;
        s.session.air_supply = air_supply;
        s.session.health_display = health_display;
        s.session.active_effects = active_effects;
        s.session.gamemode = gamemode;
        s.session.crouching = crouching;
        s.session.fall_flying = fall_flying;
        s.session.swimming = swimming;
        s.session.sleeping = sleeping;
        s.session.bed_orientation = bed_orientation;
        s.session.sleep_timer = if sleeping {
            (s.session.sleep_timer + 1).min(100)
        } else if s.session.sleep_timer > 0 {
            let next = s.session.sleep_timer + 1;
            if next >= 110 { 0 } else { next }
        } else {
            0
        };
        s.session.attack_strength = attack_strength;
        s.session.attack_delay = attack_delay;
        if s.session.xp_seen {
            if xp_progress != s.session.xp_progress {
                s.session.xp_display_tick = GAME_TIME.load(std::sync::atomic::Ordering::Relaxed);
            }
        } else {
            s.session.xp_seen = true;
        }
        if s.session.container_kind == ContainerKind::Anvil {
            s.session.xp_display_tick = GAME_TIME.load(std::sync::atomic::Ordering::Relaxed);
        }
        s.session.xp_progress = xp_progress;
        s.session.xp_level = xp_level;
        (
            s.move_flags,
            s.session.auto_jump,
            s.camera_yaw,
            s.camera_pitch,
            s.session.hotbar_target.take(),
            s.session.spectator_teleport_target.take(),
            s.session.pending_drop.take(),
            s.session.sign_update.take(),
            s.session.command_block_update.take(),
            s.session.command_block_pos,
            s.session.sign_edit_open_pos,
            s.session.edit_book.take(),
            std::mem::take(&mut s.session.outgoing_chat),
            (
                s.chat_signing_allowed || s.session.chat_signing_session,
                s.session.chat_signing_wait,
            ),
            std::mem::take(&mut s.session.swap_hands),
        )
    };

    let (jump_charge, jump_power) = crate::play::riding::tick_jump_charge(
        jumpable_mount,
        flags & crate::session::MOVE_JUMP != 0,
    );
    shared.lock().unwrap().session.jump_charge = jump_charge;

    if let Some(slot) = hotbar_target {
        bot.set_selected_hotbar_slot(slot);
    }

    if let Some(uuid) = spectator_teleport_target {
        bot.write_packet(ServerboundTeleportToEntity {
            uuid: uuid::Uuid::from_u128(uuid),
        });
    }

    if swap_hands {
        bot.write_packet(ServerboundPlayerAction {
            action: Action::SwapItemWithOffhand,
            pos: AzBlockPos::new(0, 0, 0),
            direction: Direction::Down,
            seq: 0,
        });
    }

    if let Some(drop) = pending_drop {
        bot.write_packet(ServerboundPlayerAction {
            action: match drop {
                DropRequest::HeldItem => Action::DropItem,
                DropRequest::HeldStack => Action::DropAllItems,
            },
            pos: AzBlockPos::new(0, 0, 0),
            direction: Direction::Down,
            seq: 0,
        });
        let idx = 36 + hotbar_sel as usize;
        let mut ecs = bot.ecs.write();
        if let Some(mut inv) = ecs.get_mut::<azalea::entity::inventory::Inventory>(bot.entity)
            && let Some(slot) = inv.inventory_menu.slot_mut(idx)
            && let ItemStack::Present(data) = slot
        {
            let remove = match drop {
                DropRequest::HeldItem => 1,
                DropRequest::HeldStack => data.count,
            };
            data.count -= remove.min(data.count);
            if data.count == 0 {
                *slot = ItemStack::Empty;
            }
        }
    }

    if let Some(update) = sign_update {
        bot.write_packet(ServerboundSignUpdate {
            pos: update.pos,
            is_front_text: update.front,
            lines: update.lines,
        });
    }

    if let Some(pos) = command_block_pos
        && let Ok(world) = bot.world()
        && world
            .read()
            .get_block_state(pos)
            .and_then(crate::play::interaction::command_block_mode)
            .is_none()
    {
        shared.lock().unwrap().session.command_block_pos = None;
    }

    if let Some(sign_pos) = sign_edit_open_pos {
        use azalea::block::BlockTrait;
        let still_sign = bot
            .world()
            .ok()
            .and_then(|world| world.read().get_block_state(sign_pos))
            .is_some_and(|state| {
                crate::session::SignEditKind::from_block(Box::<dyn BlockTrait>::from(state).id())
                    .is_some()
            });
        let range = bot
            .component::<azalea::entity::Attributes>()
            .map(|a| a.block_interaction_range.calculate())
            .unwrap_or(4.5)
            + 4.0;
        let eye = [
            pos.x,
            pos.y + crate::renderer::systems::eye_height(crouching) as f64,
            pos.z,
        ];
        if !still_sign || block_distance_sqr(sign_pos, eye) >= range * range {
            let mut s = shared.lock().unwrap();
            if s.session.sign_edit_open_pos == Some(sign_pos) {
                s.session.sign_edit_open_pos = None;
            }
        }
    }

    if let Some(update) = command_block_update {
        bot.write_packet(ServerboundSetCommandBlock {
            pos: update.pos,
            command: update.command,
            mode: update.mode,
            track_output: update.track_output,
            conditional: update.conditional,
            automatic: update.automatic,
        });
    }

    if let Some(book) = edit_book {
        bot.write_packet(ServerboundEditBook {
            slot: book.slot,
            pages: book.pages,
            title: book.title,
        });
    }

    let (outgoing_chat, signing) = hold_unsigned(bot, shared, outgoing_chat, signing_state);

    for line in outgoing_chat {
        let is_command = line.starts_with('/');
        let content: String = line
            .strip_prefix('/')
            .unwrap_or(&line)
            .chars()
            .filter(|c| *c != '§' && *c >= ' ' && *c != '\x7F')
            .take(256)
            .collect();
        if content.is_empty() {
            continue;
        }
        if is_command {
            bot.write_packet(ServerboundChatCommand { command: content });
        } else {
            bot.write_packet(chat_sign::chat_packet(bot, content, signing));
        }
    }

    let suggestion = {
        let mut s = shared.lock().unwrap();
        s.session.suggestion_request.take()
    };
    if let Some((id, command)) = suggestion {
        bot.write_packet(ServerboundCommandSuggestion { id, command });
    }

    let (sent_yaw, sent_pitch, flags) =
        crate::modules::hooks::sent_look(bot, -yaw - 180.0, -pitch, flags);
    if let Err(e) = bot.set_direction(sent_yaw, sent_pitch) {
        note_movement_write("the look direction", e);
    }

    apply_movement(bot, flags, auto_jump_enabled);

    if let Some(power) = jump_power {
        let id = bot
            .component::<azalea_core::entity_id::MinecraftEntityId>()
            .ok()
            .map(|id| *id);
        if let Some(id) = id {
            bot.write_packet(ServerboundPlayerCommand {
                id,
                action: PlayerCommand::StartRidingJump,
                data: power,
            });
        }
    }
    interaction::handle_interaction(bot, shared);

    viewwindow::apply_client_information_requests(bot, shared);

    let (quit, disconnect, reload_chunks, leave_bed, respawn) = {
        let mut s = shared.lock().unwrap();
        (
            std::mem::take(&mut s.quit_requested),
            std::mem::take(&mut s.disconnect_requested),
            std::mem::take(&mut s.reload_chunks_requested),
            std::mem::take(&mut s.leave_bed_requested),
            std::mem::take(&mut s.respawn_requested),
        )
    };

    if quit {
        bot.disconnect();
    }

    if disconnect {
        bot.disconnect();
    }

    if reload_chunks || crate::client::mesh_worker::take_died() {
        worldsync::reload_all_chunks(bot, shared);
    }

    if leave_bed {
        let id = bot
            .component::<azalea_core::entity_id::MinecraftEntityId>()
            .ok()
            .map(|id| *id);
        if let Some(id) = id {
            bot.write_packet(ServerboundPlayerCommand {
                id,
                action: PlayerCommand::StopSleeping,
                data: 0,
            });
        }
    }

    if respawn {
        bot.write_packet(ServerboundClientCommand {
            action: ClientCommand::PerformRespawn,
        });
    }
}

fn hold_unsigned(
    bot: &Client,
    shared: &Arc<SharedMutex>,
    outgoing: Vec<String>,
    (signing, waited): (bool, u32),
) -> (Vec<String>, bool) {
    const SIGNING_WAIT_TICKS: u32 = 60;
    const UNAVAILABLE: &str =
        "Signed chat is on, but this client has no signing certificate yet. Sent unsigned.";

    if outgoing.is_empty() || !signing {
        return (outgoing, signing);
    }
    if chat_sign::ready(bot) {
        if waited != 0 {
            shared.lock().unwrap().session.chat_signing_wait = 0;
        }
        return (outgoing, true);
    }

    let mut s = shared.lock().unwrap();
    s.session.chat_signing_wait = waited + 1;
    if s.session.chat_signing_wait > SIGNING_WAIT_TICKS {
        s.session.chat_signing_wait = 0;
        s.session.chat_send_error = Some(UNAVAILABLE.to_string());
        return (outgoing, false);
    }
    let mut queue = outgoing;
    queue.append(&mut s.session.outgoing_chat);
    s.session.outgoing_chat = queue;
    (Vec::new(), signing)
}

fn collect_other_players(bot: &Client) -> (Vec<OtherPlayerInfo>, HashSet<i32>) {
    use azalea::entity::Pose;
    use azalea::entity::metadata;

    let heads = head_yaws().lock().unwrap();
    let equip = equipment().lock().unwrap();
    let mut anims = anim_states().lock().unwrap();
    let max_healths = crate::client::tracking::max_healths().lock().unwrap();
    let viewer = crate::client::bot::username();
    let avatar_metadata_trusted = avatar_metadata_trusted();

    let players = bot.nearby_players().unwrap_or_default();
    let mut out = Vec::with_capacity(players.len());
    let mut seen: HashSet<i32> = HashSet::new();
    seen.insert(LOCAL_ANIM_ID);

    crate::client::tablist::with_lookup(|tab| {
        for eref in players.iter() {
            let Some(p) = eref.get_component::<azalea::entity::Position>() else {
                continue;
            };
            let Some(id) = eref.get_component::<azalea_core::entity_id::MinecraftEntityId>() else {
                continue;
            };
            let id = id.0;
            seen.insert(id);

            let (y_rot, pitch) = eref
                .get_component::<azalea::entity::LookDirection>()
                .map(|l| (l.y_rot(), l.x_rot()))
                .unwrap_or((0.0, 0.0));
            let pose = eref.get_component::<Pose>().map(|p| *p).unwrap_or_default();

            let worn = equip.get(&id).cloned().unwrap_or_default();
            let health = eref.get_component::<metadata::Health>().map(|c| c.0);
            let input = AnimInput {
                pos: [p.x, p.y, p.z],
                pitch,
                head_yaw: heads.get(&id).copied().unwrap_or(y_rot),
                y_rot,
                interpolated: true,
                main_hand: worn.main_hand,
                off_hand: worn.off_hand,
                armor: [worn.helmet, worn.chestplate, worn.leggings, worn.boots],
                using_offhand: eref
                    .get_component::<metadata::AbstractLivingUsingOffhand>()
                    .map(|c| c.0)
                    .unwrap_or(false),
                #[cfg(feature = "skins")]
                on_ground: eref
                    .get_component::<azalea::entity::Physics>()
                    .is_some_and(|p| p.on_ground()),
                crouching: pose == Pose::Crouching
                    || eref
                        .get_component::<metadata::AbstractEntityShiftKeyDown>()
                        .map(|c| c.0)
                        .unwrap_or(false),
                sprinting: eref
                    .get_component::<metadata::Sprinting>()
                    .map(|c| c.0)
                    .unwrap_or(false),
                fall_flying: eref
                    .get_component::<metadata::FallFlying>()
                    .map(|c| c.0)
                    .unwrap_or(false),
                visually_swimming: pose == Pose::Swimming,
                spin_attack: pose == Pose::SpinAttack,
                sleeping: pose == Pose::Sleeping,
                bed_orientation: eref
                    .get_component::<metadata::SleepingPos>()
                    .and_then(|c| c.0)
                    .and_then(|pos| bed_orientation_at(bot, pos)),
                using_item: eref
                    .get_component::<metadata::AbstractLivingUsingItem>()
                    .map(|c| c.0)
                    .unwrap_or(false),
                passenger: pose == Pose::Sitting,
                main_arm_left: avatar_metadata_trusted
                    && eref
                        .get_component::<metadata::PlayerMainHand>()
                        .is_some_and(|c| c.0 == azalea::entity::HumanoidArm::Left),
                health: health.unwrap_or(20.0),
            };

            let discrete = input.crouching;
            let anim = anims.entry(id).or_default();
            anim.tick(input);

            let profile = eref.get_component::<azalea::player::GameProfileComponent>();
            let username = profile
                .as_ref()
                .map(|g| g.0.name.clone())
                .unwrap_or_else(|| "Player".to_string());
            let tag = tab.name_tag(
                &username,
                crate::text::styled(&username, crate::text::Style::default()),
                &viewer,
            );
            out.push(OtherPlayerInfo {
                id,
                username,
                name_tag: tag.name,
                below_name: tag.below,
                hidden_by_team: tag.hidden_by_team,
                health,
                max_health: max_healths
                    .get(&id)
                    .copied()
                    .unwrap_or(crate::session::VANILLA_MAX_HEALTH),
                gamemode: profile
                    .as_ref()
                    .and_then(|g| tab.gamemode(g.0.uuid.as_u128())),
                #[cfg(feature = "skins")]
                skin: {
                    let (refs, default_index) = profile
                        .as_ref()
                        .map(|g| tab.skin(g.0.uuid.as_u128()))
                        .unwrap_or_default();
                    crate::client::skins::SkinState {
                        refs,
                        default_index,
                        parts: if avatar_metadata_trusted {
                            eref.get_component::<metadata::PlayerModeCustomisation>()
                                .map(|c| c.0)
                                .unwrap_or(crate::client::skins::ALL_PARTS)
                        } else {
                            crate::client::skins::ALL_PARTS
                        },
                    }
                },
                discrete,
                anim: anim.clone(),
            });
        }
    });

    anims.retain(|id, _| seen.contains(id));

    (out, seen)
}

fn tick_local_anim(
    bot: &Client,
    shared: &Arc<SharedMutex>,
    pos: azalea::Vec3,
) -> (HumanoidAnim, TickReads) {
    use azalea::entity::Pose;
    use azalea::entity::metadata;

    let (yaw, camera_pitch, sprinting, attacking, main_hand, off_hand, armor, prefs, reads) = {
        let s = shared.lock().unwrap();
        let hand = |i: usize| {
            s.session
                .hotbar
                .get(i)
                .map(|st| renderer::HeldItem {
                    id: st.item,
                    charged: st.charged,
                    count: st.count,
                    tint: st.potion.as_ref().map(|c| c.color()),
                    map_id: st.map_id,
                    layers: st.banner_layers.clone(),
                })
                .unwrap_or_default()
        };
        (
            s.camera_yaw,
            s.camera_pitch,
            s.move_flags & crate::session::MOVE_SPRINT != 0,
            s.session.attack_held || s.session.attack_clicked,
            hand(s.session.hotbar_selected.min(8) as usize),
            hand(9),
            s.session.armor,
            s.skin_prefs,
            TickReads {
                #[cfg(feature = "skins")]
                skin_parts: s.skin_prefs.skin_parts,
                advanced_tooltips: s.session.advanced_tooltips,
                fov_previous: s.session.fov.cur,
                fov_effects: s.session.fov.effects,
                first_person: s.session.fov.first_person,
            },
        )
    };
    let pose = bot.component::<Pose>().ok().map(|p| *p).unwrap_or_default();
    let local_sleeping_pos = bot
        .component::<metadata::SleepingPos>()
        .ok()
        .and_then(|c| c.0);
    let head_yaw = -yaw - 180.0;
    let input = AnimInput {
        pos: [pos.x, pos.y, pos.z],
        pitch: -camera_pitch,
        head_yaw,
        y_rot: head_yaw,
        interpolated: false,
        main_hand,
        off_hand,
        armor,
        using_offhand: false,
        main_arm_left: prefs.main_hand_left,
        crouching: pose == Pose::Crouching,
        sprinting,
        fall_flying: pose == Pose::FallFlying,
        visually_swimming: pose == Pose::Swimming,
        spin_attack: pose == Pose::SpinAttack,
        sleeping: local_sleeping_pos.is_some(),
        bed_orientation: local_sleeping_pos.and_then(|pos| bed_orientation_at(bot, pos)),
        using_item: interaction::is_using_item(),
        passenger: false,
        #[cfg(feature = "skins")]
        on_ground: bot
            .component::<azalea::entity::Physics>()
            .is_ok_and(|p| p.on_ground()),
        health: bot.health().unwrap_or(20.0),
    };
    let mut anims = anim_states().lock().unwrap();
    let anim = anims.entry(LOCAL_ANIM_ID).or_default();
    if attacking {
        anim.swing(prefs.main_hand_left);
    }
    anim.tick(input);
    (anim.clone(), reads)
}

struct TickReads {
    #[cfg(feature = "skins")]
    skin_parts: u8,
    advanced_tooltips: bool,
    fov_previous: f32,
    fov_effects: f32,
    first_person: bool,
}

fn avatar_metadata_trusted() -> bool {
    #[cfg(feature = "multiversion")]
    {
        crate::protocol::entity_metadata_is_native()
    }
    #[cfg(not(feature = "multiversion"))]
    {
        true
    }
}

#[cfg(feature = "skins")]
fn local_skin(bot: &Client, parts: u8) -> crate::client::skins::SkinState {
    let uuid = bot
        .component::<azalea::player::GameProfileComponent>()
        .map(|g| g.0.uuid.as_u128())
        .unwrap_or_default();

    let (refs, default_index) = crate::client::tablist::with_lookup(|tab| tab.skin(uuid));
    crate::client::skins::SkinState {
        refs,
        default_index,
        parts,
    }
}

pub(crate) fn swing_local_player(left_arm: bool, shared: &Arc<SharedMutex>) {
    let snapshot = {
        let mut anims = anim_states().lock().unwrap();
        let anim = anims.entry(LOCAL_ANIM_ID).or_default();
        anim.swing(left_arm);
        anim.clone()
    };
    shared.lock().unwrap().session.local_anim = snapshot;
}

pub(crate) fn held_item(stack: &ItemStack) -> renderer::HeldItem {
    use azalea_inventory::components as comp;
    let ItemStack::Present(data) = stack else {
        return renderer::HeldItem::default();
    };
    let id = data.kind.to_str().trim_start_matches("minecraft:");
    renderer::HeldItem {
        tint: potion_contents(data).map(|c| c.color()),
        id,
        charged: data
            .get_component::<comp::ChargedProjectiles>()
            .is_some_and(|c| !c.items.is_empty()),
        count: data.count.clamp(0, u8::MAX as i32) as u8,
        map_id: data.get_component::<comp::MapId>().map(|m| m.id),
        layers: None,
    }
}

pub(crate) fn item_id(stack: &ItemStack) -> Option<&'static str> {
    let ItemStack::Present(data) = stack else {
        return None;
    };
    Some(data.kind.to_str().trim_start_matches("minecraft:"))
}

fn health_display(
    bot: &Client,
    active_effects: &[crate::play::mob_effects::MobEffectInstance],
    armor: u8,
) -> (crate::session::HealthDisplay, bool) {
    use crate::session::{HealthDisplay, HeartKind};
    use azalea::entity::metadata;
    use azalea_core::entity_id::MinecraftEntityId;

    let has = |id: &str| active_effects.iter().any(|e| e.id == id);
    let ticks_frozen = bot
        .component::<metadata::TicksFrozen>()
        .map(|t| t.0)
        .unwrap_or(0);
    let kind = if has("poison") {
        HeartKind::Poisoned
    } else if has("wither") {
        HeartKind::Withered
    } else if ticks_frozen >= crate::play::entity_feed::TICKS_REQUIRED_TO_FREEZE {
        HeartKind::Frozen
    } else {
        HeartKind::Normal
    };

    let local_id = bot.component::<MinecraftEntityId>().map(|id| id.0).ok();
    if let Some(id) = local_id {
        crate::client::tracking::LOCAL_ENTITY_ID.store(id, std::sync::atomic::Ordering::Relaxed);
    }

    let vehicle_id = crate::client::tracking::local_vehicle_id();

    let (max_health, vehicle_max) = {
        let map = crate::client::tracking::max_healths().lock().unwrap();
        (
            local_id
                .and_then(|id| map.get(&id).copied())
                .unwrap_or(crate::session::VANILLA_MAX_HEALTH),
            vehicle_id.and_then(|id| map.get(&id).copied()),
        )
    };
    let saddled = vehicle_id.is_some_and(|id| {
        equipment()
            .lock()
            .unwrap()
            .get(&id)
            .is_some_and(|worn| worn.saddle.is_some())
    });

    let (vehicle, jumpable) = match vehicle_id {
        Some(vehicle_id) => {
            let ecs = bot.ecs.read();
            let entity = ecs
                .get::<azalea::entity::indexing::EntityIdIndex>(bot.entity)
                .and_then(|index| index.get_by_minecraft_entity(MinecraftEntityId(vehicle_id)));
            let health = entity.and_then(|e| ecs.get::<metadata::Health>(e).map(|h| h.0));
            let jumpable = saddled
                && entity.is_some_and(|e| {
                    ecs.get::<azalea::entity::EntityKindComponent>(e)
                        .is_some_and(|k| is_rideable_jumping(**k))
                });
            let max = vehicle_max
                .or(health)
                .unwrap_or(crate::session::VANILLA_MAX_HEALTH);
            (health.map(|h| (h, max)), jumpable)
        }
        None => (None, false),
    };

    (
        HealthDisplay {
            max_health,
            absorption: bot
                .component::<metadata::PlayerAbsorption>()
                .map(|a| a.0)
                .unwrap_or(0.0),
            armor,
            kind,
            regenerating: has("regeneration"),
            hunger_effect: has("hunger"),
            ticks_frozen,
            vehicle,
        },
        jumpable,
    )
}

fn is_rideable_jumping(kind: azalea_registry::builtin::EntityKind) -> bool {
    use azalea_registry::builtin::EntityKind as K;
    matches!(
        kind,
        K::Horse | K::Donkey | K::Mule | K::SkeletonHorse | K::ZombieHorse | K::Camel
    )
}

fn armor_value(player_slots: &[ItemStack], selected: u8) -> u8 {
    use azalea_core::attribute_modifier_operation::AttributeModifierOperation as Op;
    use azalea_inventory::components as comp;
    use azalea_inventory::default_components::get_default_component;
    use azalea_registry::builtin::{Attribute, ItemKind};
    use comp::EquipmentSlotGroup as Group;

    const WORN: [(usize, Group); 4] = [
        (5, Group::Head),
        (6, Group::Chest),
        (7, Group::Legs),
        (8, Group::Feet),
    ];

    let held = [
        (36 + selected.min(8) as usize, Group::Mainhand),
        (45, Group::Offhand),
    ];
    let slots = WORN.iter().chain(held.iter());

    let mut key = [None; 6];
    let mut explicit = false;
    for (dst, (slot, _)) in key.iter_mut().zip(slots.clone()) {
        if let Some(ItemStack::Present(data)) = player_slots.get(*slot) {
            *dst = Some(data.kind);
            explicit |= data.get_component::<comp::AttributeModifiers>().is_some();
        }
    }

    static ARMOR_CACHE: std::sync::Mutex<([Option<ItemKind>; 6], u8)> =
        std::sync::Mutex::new(([None; 6], 0));
    let mut cache = ARMOR_CACHE.lock().unwrap();
    if !explicit && cache.0 == key {
        return cache.1;
    }

    let admits = |entry: Group, worn: Group| match entry {
        Group::Any => true,
        Group::Armor => matches!(worn, Group::Head | Group::Chest | Group::Legs | Group::Feet),
        Group::Hand => matches!(worn, Group::Mainhand | Group::Offhand),
        other => other == worn,
    };

    let mut total = 0.0f64;
    for (slot, group) in slots {
        let Some(stack @ ItemStack::Present(data)) = player_slots.get(*slot) else {
            continue;
        };
        let modifiers = match stack.get_component::<comp::AttributeModifiers>() {
            Some(own) => own.into_owned(),
            None => match get_default_component::<comp::AttributeModifiers>(data.kind) {
                Some(default) => default,
                None => continue,
            },
        };
        for entry in &modifiers.modifiers {
            if entry.kind == Attribute::Armor
                && entry.modifier.operation == Op::AddValue
                && admits(entry.slot, *group)
            {
                total += entry.modifier.amount;
            }
        }
    }

    let value = total.floor().clamp(0.0, u8::MAX as f64) as u8;
    *cache = (key, value);
    value
}

fn apply_movement(bot: &Client, flags: u8, auto_jump_enabled: bool) {
    let fwd = flags & 1 != 0;
    let back = flags & 2 != 0;
    let left = flags & 4 != 0;
    let right = flags & 8 != 0;
    let jump = flags & 16 != 0;
    let sprint = flags & 32 != 0;
    let sneak = flags & crate::session::MOVE_SNEAK != 0;

    let flying = bot
        .component::<azalea::entity::PlayerAbilities>()
        .is_ok_and(|a| a.flying);

    if let Err(e) = bot.set_crouching(sneak && !flying) {
        note_movement_write("the sneak flag", e);
    }

    let walk_dir = match (fwd, back, left, right) {
        (true, false, false, false) => WalkDirection::Forward,
        (false, true, false, false) => WalkDirection::Backward,
        (false, false, true, false) => WalkDirection::Left,
        (false, false, false, true) => WalkDirection::Right,
        (true, false, true, false) => WalkDirection::ForwardLeft,
        (true, false, false, true) => WalkDirection::ForwardRight,
        (false, true, true, false) => WalkDirection::BackwardLeft,
        (false, true, false, true) => WalkDirection::BackwardRight,
        _ => WalkDirection::None,
    };

    if sprint
        && matches!(
            walk_dir,
            WalkDirection::Forward | WalkDirection::ForwardLeft | WalkDirection::ForwardRight
        )
    {
        bot.sprint(match walk_dir {
            WalkDirection::ForwardLeft => SprintDirection::ForwardLeft,
            WalkDirection::ForwardRight => SprintDirection::ForwardRight,
            _ => SprintDirection::Forward,
        });
    } else {
        bot.walk(walk_dir);
    }

    let (on_ground, horizontal_collision) = {
        let physics = bot.component::<azalea::entity::Physics>();
        let (on_ground, horizontal_collision) = physics
            .as_ref()
            .map(|phys| (phys.on_ground(), phys.horizontal_collision))
            .unwrap_or((false, false));
        (on_ground, horizontal_collision)
    };

    let auto_jump = auto_jump_enabled
        && !jump
        && !flying
        && !sneak
        && walk_dir != WalkDirection::None
        && on_ground
        && horizontal_collision;

    if let Err(e) = bot.set_jumping(jump || auto_jump) {
        note_movement_write("the jump flag", e);
    }
}
