use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use azalea::Client;
use azalea::block::BlockState;
use azalea::mining::{MineBlockPos, MineProgress, StopMiningBlockEvent};
use azalea::physics::collision::BlockWithShape;
use azalea_core::direction::Direction;
use azalea_core::game_type::GameMode;
use azalea_core::hit_result::{BlockHitResult, HitResult};
use azalea_core::position::BlockPos;
use azalea_inventory::ItemStack;
use azalea_protocol::packets::game::s_player_action::{Action, ServerboundPlayerAction};
use azalea_registry::builtin::ItemKind;

use crate::items::consume::Consumption;
use crate::session::{
    BreakingBlock, CommandBlockMode, InteractionHand, ParticleSpawn, SharedMutex, TargetedBlock,
};

const DESTROY_DELAY: u32 = 5;
const RIGHT_CLICK_DELAY: u32 = 4;

static BREAK_DELAY: AtomicU32 = AtomicU32::new(0);
static PLACE_DELAY: AtomicU32 = AtomicU32::new(0);
static SEEN_PUBLISHES: AtomicU32 = AtomicU32::new(0);
static USE_ITEM: Mutex<UseItem> = Mutex::new(UseItem {
    started: false,
    item: None,
    consume: None,
    remaining: 0,
    hand: InteractionHand::Main,
});
static SERVER_USING_ITEM: AtomicBool = AtomicBool::new(false);
static USE_COMPLETED: AtomicBool = AtomicBool::new(false);

struct UseItem {
    started: bool,
    item: Option<ItemKind>,
    consume: Option<Consumption>,
    remaining: i32,
    hand: InteractionHand,
}
static WAS_MINING: AtomicBool = AtomicBool::new(false);

static LOCAL_BREAK: Mutex<Option<(BlockPos, crate::platform::time::Instant)>> = Mutex::new(None);

const LOCAL_BREAK_TTL: std::time::Duration = std::time::Duration::from_secs(1);

fn note_local_break(pos: BlockPos) {
    if let Ok(mut slot) = LOCAL_BREAK.lock() {
        *slot = Some((pos, crate::platform::time::Instant::now()));
    }
}

pub(crate) fn take_local_break(pos: BlockPos) -> bool {
    let Ok(mut slot) = LOCAL_BREAK.lock() else {
        return false;
    };
    let Some((named, at)) = *slot else {
        return false;
    };
    if named != pos || at.elapsed() > LOCAL_BREAK_TTL {
        return false;
    }
    *slot = None;
    true
}

pub(crate) fn reset_session_state() {
    BREAK_DELAY.store(0, Ordering::Relaxed);
    PLACE_DELAY.store(0, Ordering::Relaxed);
    SERVER_USING_ITEM.store(false, Ordering::Relaxed);
    USE_COMPLETED.store(false, Ordering::Relaxed);
    WAS_MINING.store(false, Ordering::Relaxed);
    let use_item = match USE_ITEM.try_lock() {
        Ok(guard) => Some(guard),
        Err(std::sync::TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => None,
    };
    if let Some(mut use_item) = use_item {
        use_item.started = false;
        use_item.item = None;
        use_item.consume = None;
        use_item.remaining = 0;
        use_item.hand = InteractionHand::Main;
    }
    let local_break = match LOCAL_BREAK.try_lock() {
        Ok(guard) => Some(guard),
        Err(std::sync::TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(std::sync::TryLockError::WouldBlock) => None,
    };
    if let Some(mut local_break) = local_break {
        *local_break = None;
    }
}

pub fn handle_interaction(bot: &Client, shared: &SharedMutex) {
    let hit = block_hit(bot);

    let target = hit
        .as_ref()
        .filter(|h| within_border(bot, h.block_pos))
        .and_then(|h| {
            let state = bot.world().ok()?.read().get_block_state(h.block_pos)?;
            Some(TargetedBlock {
                pos: [h.block_pos.x, h.block_pos.y, h.block_pos.z],
                boxes: outline_boxes(state, h.block_pos, main_hand_kind(bot)),
            })
        });

    let remote: Vec<([i32; 3], u8)> = {
        let mut s = shared.lock().unwrap();
        let remote = &mut s.session.remote_breaking;
        remote.retain(|_, r| {
            r.age += 1;
            r.age <= REMOTE_BREAK_TIMEOUT
        });
        let mut out: Vec<_> = remote.values().map(|r| (r.pos, r.stage)).collect();
        out.sort_unstable();
        out
    };

    let (local, mining_particle) = mining_overlay(bot);
    let breaking = breaking_overlays(bot, local, &remote);
    let targeted_entity = living_target(bot);
    let crosshair_entity = picked_entity(bot);

    let (attack_held, attack_clicked, use_held, use_clicked, pick, attack_strength) = {
        let mut s = shared.lock().unwrap();
        s.session.targeted_block = target;
        s.session.targeted_entity = targeted_entity;
        s.session.crosshair_entity = crosshair_entity;
        let attack_strength = s.session.attack_strength;
        s.session.breaking = breaking;
        if let Some(spawn) = mining_particle {
            crate::session::push_particle_emit(
                &mut s.session.particle_emits,
                crate::session::ParticleEmit::Block(spawn),
            );
        }
        if s.screen_open {
            s.session.attack_clicked = false;
            s.session.use_clicked = false;
            s.session.pick_clicked = false;
            (false, false, false, false, None, attack_strength)
        } else {
            (
                s.session.attack_held,
                std::mem::take(&mut s.session.attack_clicked),
                s.session.use_held,
                std::mem::take(&mut s.session.use_clicked),
                std::mem::take(&mut s.session.pick_clicked).then_some(s.session.pick_include_data),
                attack_strength,
            )
        }
    };

    let (attack_held, attack_clicked, use_held, use_clicked) =
        crate::modules::auto_mine::pick_gate(attack_held, attack_clicked, use_held, use_clicked);
    let freecam = if !crate::modules::freecam::active() {
        FreecamClick::Attached
    } else if attack_held || attack_clicked || use_held || use_clicked {
        freecam_click(bot)
    } else {
        FreecamClick::Swallow
    };
    let (attack_held, attack_clicked, use_held, use_clicked) = match freecam {
        FreecamClick::Swallow => (false, false, false, false),
        _ => (attack_held, attack_clicked, use_held, use_clicked),
    };
    let forced_face = matches!(freecam, FreecamClick::At(_));
    let hand_hit = match &freecam {
        FreecamClick::At(h) => Some(h.clone()),
        _ => hit.clone(),
    };

    if let Some(include_data) = pick {
        apply_pick_block(bot, include_data);
    }
    let hands_busy = crate::play::riding::hands_busy();
    let attack_clicked = attack_clicked && !hands_busy;
    #[cfg(feature = "mobile_ui")]
    let attack_entity = attack_clicked || (attack_held && attack_strength >= 1.0 && !hands_busy);
    #[cfg(not(feature = "mobile_ui"))]
    let attack_entity = attack_clicked;
    let attack_entity = attack_entity && !forced_face;

    crate::modules::esp::tick(bot, shared);

    crate::modules::seed_cracker::tick(bot, shared);

    crate::modules::aim_assist::tick(bot, shared);

    crate::modules::auto_sell::tick(bot, shared);

    crate::modules::spam::tick(shared);

    crate::modules::auto_totem::tick(bot, shared);

    let attack_held = crate::modules::auto_mine::tick(bot, shared, hit.as_ref()) || attack_held;

    crate::modules::auto_mace::tick(bot, shared);

    crate::modules::crystal_aura::tick(bot, shared);

    crate::modules::scaffold::tick(bot, shared);

    crate::modules::pathfinder::tick(bot, shared);

    let attack_entity = attack_entity
        || (crate::modules::triggerbot::enabled() && {
            let kind = crosshair_kind(bot);
            let (strength, weapon) = {
                let s = shared.lock().unwrap();
                let weapon = s
                    .session
                    .hotbar
                    .get(s.session.hotbar_selected as usize)
                    .is_some_and(|st| {
                        !st.is_empty() && crate::modules::triggerbot::is_weapon(st.item)
                    });
                (s.session.attack_strength, weapon)
            };
            crate::modules::triggerbot::should_attack(kind, strength, weapon)
        });

    if bot
        .component::<GameMode>()
        .is_ok_and(|gm| *gm == GameMode::Spectator)
    {
        if attack_clicked {
            let target = match bot.hit_result() {
                Ok(HitResult::Entity(e)) => bot
                    .entity_component::<azalea_core::entity_id::MinecraftEntityId>(e.entity)
                    .map(|id| *id)
                    .ok(),
                _ => None,
            };
            bot.write_packet(
                azalea_protocol::packets::game::s_spectator_action::ServerboundSpectatorAction {
                    spectate_entity_id: azalea_core::entity_id::OptionalEntityId(target),
                },
            );
        }
        apply_break(
            bot,
            shared,
            hand_hit.as_ref(),
            attack_held,
            attack_clicked,
            forced_face,
        );
        apply_place(bot, shared, use_held, use_clicked, hands_busy);
        publish_item_in_use(bot);
        return;
    }

    let weapon = if attack_entity || attack_held {
        crate::play::spear::held_weapon(bot)
    } else {
        crate::play::spear::HeldWeapon::default()
    };
    let attack_entity = attack_entity && !weapon.cannot_attack(attack_strength);
    let attack_clicked = attack_clicked && !weapon.cannot_attack(attack_strength);
    let piercing = weapon.piercing.is_some();
    if let Some(sound) = weapon.piercing
        && attack_entity
        && !hands_busy
        && !is_using_item()
        && bot
            .component::<GameMode>()
            .map_or(true, |gm| *gm != GameMode::Spectator)
    {
        crate::play::spear::stab(bot, shared, sound);
    }

    let attacked_entity = !piercing
        && attack_entity
        && match bot.hit_result() {
            #[cfg(lua_mods)]
            Ok(HitResult::Entity(e)) if crate::modules::script::attack(bot, shared, e.entity) => {
                true
            }
            Ok(HitResult::Entity(e)) => {
                bot.attack(e.entity);
                crate::client::tick::swing_local_player(false, shared);
                true
            }
            _ => false,
        };

    if !piercing && attack_clicked && !attacked_entity && hit.is_none() {
        crate::client::tick::request_attack_ticker_reset();
    }

    if !piercing && !attacked_entity {
        apply_break(
            bot,
            shared,
            hand_hit.as_ref(),
            attack_held,
            attack_clicked,
            forced_face,
        );
    }
    let restore = match &freecam {
        FreecamClick::At(h) if use_held || use_clicked => {
            swap_hit_result(bot, HitResult::Block(h.clone()))
        }
        _ => None,
    };
    apply_place(bot, shared, use_held, use_clicked, hands_busy);
    if let Some(old) = restore {
        swap_hit_result(bot, old);
    }
    publish_item_in_use(bot);
}

fn publish_item_in_use(bot: &Client) {
    use azalea::movement::ItemInUse;
    use azalea_inventory::components::UseEffects;

    let want = using_hand().map(|hand| {
        let effects = bot
            .component::<azalea::entity::inventory::Inventory>()
            .ok()
            .and_then(|inv| {
                inv.get_equipment(hand.equipment())?
                    .get_component::<UseEffects>()
                    .map(|e| e.into_owned())
            })
            .unwrap_or_default();
        ItemInUse {
            speed_multiplier: effects.speed_multiplier,
            can_sprint: effects.can_sprint,
        }
    });
    let have = bot.component::<ItemInUse>().ok().map(|u| *u);
    if have == want {
        return;
    }
    let mut ecs = bot.ecs.write();
    let mut player = ecs.entity_mut(bot.entity);
    match want {
        Some(u) => {
            player.insert(u);
        }
        None => {
            player.remove::<ItemInUse>();
        }
    }
}

const REMOTE_BREAK_TIMEOUT: u32 = 400;

fn outline_boxes(state: BlockState, pos: BlockPos, held: ItemKind) -> Vec<[f32; 6]> {
    let shape = match azalea::physics::clip::holding_outline(state, held) {
        Some(full) => std::borrow::Cow::Borrowed(full),
        None => state.outline_shape(pos),
    };
    shape
        .to_aabbs()
        .iter()
        .map(|a| {
            [
                a.min.x as f32,
                a.min.y as f32,
                a.min.z as f32,
                a.max.x as f32,
                a.max.y as f32,
                a.max.z as f32,
            ]
        })
        .collect()
}

fn breaking_overlays(
    bot: &Client,
    local: Option<BreakingBlock>,
    remote: &[([i32; 3], u8)],
) -> Vec<BreakingBlock> {
    let mut out: Vec<BreakingBlock> = local.into_iter().collect();
    if remote.is_empty() {
        return out;
    }
    let Ok(world) = bot.world() else { return out };
    let world = world.read();
    for &(pos, stage) in remote {
        if let Some(b) = out.iter_mut().find(|b| b.pos == pos) {
            b.stage = b.stage.max(stage);
            continue;
        }
        let block = BlockPos::new(pos[0], pos[1], pos[2]);
        let Some(state) = world.get_block_state(block) else {
            continue;
        };
        out.push(BreakingBlock {
            pos,
            stage,
            boxes: outline_boxes(state, block, ItemKind::Air),
        });
    }
    out
}

fn mining_overlay(bot: &Client) -> (Option<BreakingBlock>, Option<ParticleSpawn>) {
    let Some(pos) = bot.component::<MineBlockPos>().ok().and_then(|p| p.0) else {
        return (None, None);
    };
    let Some(stage) = bot
        .component::<MineProgress>()
        .ok()
        .and_then(|p| p.destroy_stage())
    else {
        return (None, None);
    };
    let Some(state) = bot.world().ok().and_then(|w| w.read().get_block_state(pos)) else {
        return (None, None);
    };
    let breaking = BreakingBlock {
        pos: [pos.x, pos.y, pos.z],
        stage: stage.min(9) as u8,
        boxes: outline_boxes(state, pos, ItemKind::Air),
    };
    let face = match block_hit(bot) {
        Some(h) if h.block_pos == pos => {
            let n = h.direction.normal_vec3();
            [n.x as f32, n.y as f32, n.z as f32]
        }
        _ => [0.0, 1.0, 0.0],
    };
    let spawn = ParticleSpawn {
        pos: breaking.pos,
        state,
        mining: true,
        face,
    };
    (Some(breaking), Some(spawn))
}

enum FreecamClick {
    Attached,
    Swallow,
    At(BlockHitResult),
}

const FREECAM_PICK: f64 = 128.0;

fn freecam_click(bot: &Client) -> FreecamClick {
    if crate::modules::freecam::pause_clicks() || crate::modules::auto_mine::enabled() {
        return FreecamClick::Swallow;
    }
    let Some((eye, dir)) = crate::modules::freecam::ray() else {
        return FreecamClick::Attached;
    };
    let (Ok(body), Ok(attrs)) = (
        bot.eye_position(),
        bot.component::<azalea::entity::Attributes>(),
    ) else {
        return FreecamClick::Swallow;
    };
    let range = attrs.block_interaction_range.calculate() + 1.0;
    drop(attrs);
    let held_item = main_hand_kind(bot);
    let Some(hit) = crate::modules::freecam::far_hit(bot, eye, dir, FREECAM_PICK, held_item) else {
        return FreecamClick::Swallow;
    };
    let p = hit.block_pos;
    let gap = |e: f64, lo: i32| (f64::from(lo) - e).max(0.0).max(e - f64::from(lo) - 1.0);
    let d2 = gap(body.x, p.x).powi(2) + gap(body.y, p.y).powi(2) + gap(body.z, p.z).powi(2);
    if d2 >= range * range {
        return FreecamClick::Swallow;
    }
    FreecamClick::At(hit)
}

fn swap_hit_result(bot: &Client, with: HitResult) -> Option<HitResult> {
    let mut ecs = bot.ecs.write();
    let mut current = ecs.get_mut::<azalea::interact::pick::HitResultComponent>(bot.entity)?;
    Some(std::mem::replace(&mut **current, with))
}

fn block_hit(bot: &Client) -> Option<BlockHitResult> {
    match bot.hit_result().ok()? {
        HitResult::Block(b) if !b.miss => Some(b),
        _ => None,
    }
}

fn break_step(
    creative: bool,
    clicked: bool,
    delay: u32,
    was_mining: bool,
    mining_at: Option<BlockPos>,
    target: Option<BlockPos>,
) -> (bool, u32) {
    let delay = if was_mining && mining_at.is_none() {
        DESTROY_DELAY
    } else {
        delay
    };

    if !clicked && delay > 0 {
        return (false, delay - 1);
    }
    let Some(target) = target else {
        return (false, delay);
    };

    if creative {
        return (true, DESTROY_DELAY);
    }

    if mining_at == Some(target) {
        return (false, delay);
    }
    (true, delay)
}

fn apply_break(
    bot: &Client,
    shared: &SharedMutex,
    hit: Option<&BlockHitResult>,
    held: bool,
    clicked: bool,
    forced_face: bool,
) {
    let game_mode = bot
        .component::<GameMode>()
        .map(|gm| *gm)
        .unwrap_or(GameMode::Survival);
    let creative = game_mode == GameMode::Creative;
    let (held, clicked) = match game_mode {
        GameMode::Spectator => (false, false),
        _ => (held, clicked),
    };

    let mining_at = if bot.is_mining() {
        bot.component::<MineBlockPos>().ok().and_then(|p| p.0)
    } else {
        None
    };
    let was_mining = WAS_MINING.swap(mining_at.is_some(), Ordering::Relaxed);

    if !held && !clicked {
        BREAK_DELAY.store(0, Ordering::Relaxed);
        if mining_at.is_some() {
            bot.ecs
                .write()
                .write_message(StopMiningBlockEvent { entity: bot.entity });
            crate::client::tick::request_attack_ticker_reset();
        }
        return;
    }

    let (mining_at, was_mining) = if hit.is_none() && mining_at.is_some() {
        bot.ecs
            .write()
            .write_message(StopMiningBlockEvent { entity: bot.entity });
        crate::client::tick::request_attack_ticker_reset();
        WAS_MINING.store(false, Ordering::Relaxed);
        (None, false)
    } else {
        (mining_at, was_mining)
    };

    let target = hit.map(|h| h.block_pos);
    let outside = target.is_some_and(|pos| !within_border(bot, pos));
    let (start, delay) = break_step(
        creative,
        clicked,
        BREAK_DELAY.load(Ordering::Relaxed),
        was_mining,
        mining_at,
        target.filter(|_| !outside),
    );
    BREAK_DELAY.store(delay, Ordering::Relaxed);
    if let Some(pos) = mining_at.or_else(|| hit.map(|h| h.block_pos).filter(|_| start)) {
        note_local_break(pos);
    }
    if start {
        let pos = hit
            .expect("break_step only starts when there is a target")
            .block_pos;
        if forced_face {
            let face = hit.expect("checked above").direction;
            bot.start_mining_with_face(pos, face, !clicked);
        } else {
            bot.start_mining_with(pos, !clicked);
        }
    }

    if start || mining_at.is_some() || (clicked && outside) {
        crate::client::tick::swing_local_player(false, shared);
    }
}

fn place_step(
    game_mode: GameMode,
    clicked: bool,
    held: bool,
    delay: u32,
    already_using: bool,
    use_item: impl FnOnce() -> bool,
) -> (bool, u32) {
    let delay = delay.saturating_sub(1);
    if already_using {
        return (false, delay);
    }
    if !clicked && !(held && delay == 0) {
        return (false, delay);
    }

    let used = use_item();
    let swing = used && game_mode != GameMode::Spectator;
    (swing, RIGHT_CLICK_DELAY)
}

fn apply_place(bot: &Client, shared: &SharedMutex, held: bool, clicked: bool, hands_busy: bool) {
    let game_mode = bot
        .component::<GameMode>()
        .map(|gm| *gm)
        .unwrap_or(GameMode::Survival);
    #[cfg(lua_mods)]
    let offered = clicked && !hands_busy && !is_using_item();
    #[cfg(lua_mods)]
    if offered && crate::modules::script::use_item(bot, shared) {
        sync_server_using_item(bot, shared);
        updating_using_item(bot, shared);
        return;
    }
    sync_server_using_item(bot, shared);

    let was_using = is_using_item();
    if was_using && !held {
        release_using_item(bot);
    }
    let publishes = crate::renderer::intent::publishes();
    let confirmed = SEEN_PUBLISHES.swap(publishes, Ordering::Relaxed) != publishes;
    let mut swung = InteractionHand::Main;
    let (swing, delay) = place_step(
        game_mode,
        clicked,
        held && confirmed,
        PLACE_DELAY.load(Ordering::Relaxed),
        was_using,
        || {
            if hands_busy || bot.is_mining() {
                return false;
            }
            #[cfg(lua_mods)]
            if !offered && crate::modules::script::use_item(bot, shared) {
                return false;
            }
            let secondary_use = shared.lock().unwrap().move_flags & crate::session::MOVE_SNEAK != 0;
            let hit = bot.hit_result().ok();
            let click = Click {
                hit: hit.as_ref(),
                secondary_use,
                fresh: clicked,
                spectator: game_mode == GameMode::Spectator,
            };
            for hand in InteractionHand::BOTH {
                if let Used::Done { swing } = use_hand(bot, shared, hand, &click) {
                    swung = hand;
                    return swing;
                }
            }
            false
        },
    );
    PLACE_DELAY.store(delay, Ordering::Relaxed);
    if swing {
        swing_hand(bot, shared, swung);
    }
    updating_using_item(bot, shared);
}

struct Click<'a> {
    hit: Option<&'a HitResult>,
    secondary_use: bool,
    fresh: bool,
    spectator: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Used {
    Pass,
    Done { swing: bool },
}

fn use_hand(bot: &Client, shared: &SharedMutex, hand: InteractionHand, click: &Click) -> Used {
    if predict_place(bot, click.secondary_use, hand) {
        consume_held_item(bot, hand);
        item_used(shared, hand);
        return Used::Done { swing: true };
    }
    let block_takes_it = matches!(click.hit, Some(HitResult::Block(hit))
        if !hit.miss && !click.spectator && used_block_interaction(bot, hit, hand, click.secondary_use));
    let entity_first = matches!(click.hit, Some(HitResult::Entity(_)));
    if !block_takes_it && !entity_first && predict_bucket_use(bot, shared, hand) {
        item_used(shared, hand);
        return Used::Done { swing: true };
    }
    let stack = hand_stack(bot, hand);
    match click.hit {
        Some(HitResult::Entity(hit)) => {
            let block = bot
                .entity_component::<azalea::entity::Position>(hit.entity)
                .map(|pos| pos.to_block_pos_floor());
            if block.is_ok_and(|block| !within_border(bot, block)) {
                return Used::Done { swing: false };
            }
            match interact_entity(bot, hit, click, hand, stack.map(|s| s.kind)) {
                EntityUse::Success => return Used::Done { swing: true },
                EntityUse::Unknown => return Used::Done { swing: false },
                EntityUse::Pass => {
                    if predict_bucket_use(bot, shared, hand) {
                        item_used(shared, hand);
                        return Used::Done { swing: true };
                    }
                }
            }
        }
        Some(HitResult::Block(hit)) if !hit.miss => {
            if !within_border(bot, hit.block_pos) {
                return Used::Done { swing: false };
            }
            let consumed = block_takes_it;
            if consumed {
                open_game_master_block(bot, shared, hit);
            }
            send_use_item_on(bot, hit, consumed, hand);
            #[cfg(feature = "audio")]
            if consumed && let Some(stack) = stack.as_ref().filter(|s| !s.on_cooldown) {
                use crate::play::player_sounds as sounds;
                if sounds::item_runs_past_block(bot, stack.kind, hit) {
                    sounds::item_used_on(
                        bot,
                        shared,
                        stack.kind,
                        hit,
                        axe_declines(bot, hand, click),
                    );
                }
                sounds::bottle_on_hive(bot, stack.kind, hit);
            }
            if consumed {
                let light = bot
                    .world()
                    .ok()
                    .and_then(|w| w.read().get_block_state(hit.block_pos))
                    .is_some_and(|s| {
                        s.as_block_kind() == azalea_registry::builtin::BlockKind::Light
                    });
                return Used::Done { swing: !light };
            }
            if click.spectator {
                return Used::Done { swing: false };
            }
            if let Some(stack) = stack
                && !stack.on_cooldown
            {
                #[cfg(feature = "audio")]
                crate::play::player_sounds::item_used_on(
                    bot,
                    shared,
                    stack.kind,
                    hit,
                    axe_declines(bot, hand, click),
                );
                if stack.kind == ItemKind::FireworkRocket && !fall_flying(bot) {
                    return Used::Done { swing: true };
                }
                if stack.use_on_ends {
                    if stack.consumable && stack.use_duration > 0 {
                        start_use(bot, shared, hand, &stack);
                    }
                    return Used::Done { swing: false };
                }
            }
        }
        _ => {}
    }
    match stack {
        Some(stack) => use_item(bot, shared, hand, stack, click),
        None => Used::Pass,
    }
}

#[cfg(feature = "audio")]
fn axe_declines(bot: &Client, hand: InteractionHand, click: &Click) -> bool {
    use azalea_inventory::components::BlocksAttacks;

    hand == InteractionHand::Main
        && !click.secondary_use
        && bot
            .component::<azalea::entity::inventory::Inventory>()
            .is_ok_and(|inv| {
                inv.get_equipment(InteractionHand::Off.equipment())
                    .and_then(ItemStack::as_present)
                    .is_some_and(|off| off.get_component::<BlocksAttacks>().is_some())
            })
}

fn within_border(bot: &Client, pos: BlockPos) -> bool {
    bot.component::<azalea::world::WorldBorder>()
        .map_or(true, |b| b.is_within_block(pos))
}

fn interact_entity(
    bot: &Client,
    hit: &azalea_core::hit_result::EntityHitResult,
    click: &Click,
    hand: InteractionHand,
    held: Option<ItemKind>,
) -> EntityUse {
    use azalea::interact::EntityInteractEvent;

    bot.ecs.write().trigger(EntityInteractEvent {
        client: bot.entity,
        target: hit.entity,
        location: Some(hit.location),
        hand: hand.packet(),
    });

    if click.spectator {
        return EntityUse::Pass;
    }
    let Some(target) = entity_target(bot, hit.entity) else {
        return EntityUse::Unknown;
    };
    #[cfg(feature = "audio")]
    if let Some(item) = held {
        crate::play::player_sounds::dye_sheep(bot, hit.entity, item);
    }
    entity_use(target, click.secondary_use, held)
}

#[derive(Clone, Copy, Debug)]
struct EntityTarget {
    kind: azalea_registry::builtin::EntityKind,
    has_riders: bool,
    hull_under_water: bool,
}

fn entity_target(bot: &Client, entity: azalea::ecs::entity::Entity) -> Option<EntityTarget> {
    use azalea::entity::dimensions::EntityDimensions;
    use azalea::entity::{EntityKindComponent, Position};
    use azalea_core::entity_id::MinecraftEntityId;

    let (kind, pos, id) = {
        let ecs = bot.ecs.read();
        (
            **ecs.get::<EntityKindComponent>(entity)?,
            **ecs.get::<Position>(entity)?,
            ecs.get::<MinecraftEntityId>(entity)?.0,
        )
    };
    let has_riders = crate::client::tracking::passengers()
        .lock()
        .unwrap()
        .get(&id)
        .is_some_and(|riders| !riders.is_empty());
    let hull_under_water = crate::play::riding::Hull::of(kind).is_some() && {
        let hull = EntityDimensions::from(kind).make_bounding_box(pos);
        bot.world()
            .is_ok_and(|world| crate::play::riding::hull_under_water(&world.read(), &hull))
    };
    Some(EntityTarget {
        kind,
        has_riders,
        hull_under_water,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntityUse {
    Success,
    Pass,
    Unknown,
}

fn entity_use(target: EntityTarget, secondary_use: bool, held: Option<ItemKind>) -> EntityUse {
    use azalea_registry::builtin::EntityKind;

    let boat = crate::play::riding::Hull::of(target.kind).is_some();
    if !boat && target.kind != EntityKind::Minecart {
        return other_entity_use(target.kind, held);
    }
    if matches!(held, Some(ItemKind::Lead | ItemKind::Shears)) {
        return EntityUse::Unknown;
    }
    if secondary_use {
        return EntityUse::Pass;
    }
    if boat {
        if target.hull_under_water {
            return EntityUse::Unknown;
        }
    } else if target.has_riders {
        return EntityUse::Pass;
    }
    EntityUse::Success
}

fn other_entity_use(
    kind: azalea_registry::builtin::EntityKind,
    held: Option<ItemKind>,
) -> EntityUse {
    use crate::client::recipes::item_in_tag;
    use azalea_registry::builtin::EntityKind as E;

    if kind == E::Player {
        return if held == Some(ItemKind::Shears) {
            EntityUse::Unknown
        } else {
            EntityUse::Pass
        };
    }
    if matches!(
        kind,
        E::Horse
            | E::Donkey
            | E::Mule
            | E::Llama
            | E::TraderLlama
            | E::ZombieHorse
            | E::SkeletonHorse
            | E::Camel
            | E::CamelHusk
            | E::Pig
            | E::Strider
            | E::Nautilus
            | E::ZombieNautilus
            | E::HappyGhast
            | E::Wolf
            | E::Cat
            | E::Parrot
            | E::Panda
            | E::Allay
            | E::CopperGolem
            | E::Villager
            | E::WanderingTrader
            | E::ArmorStand
            | E::ItemFrame
            | E::GlowItemFrame
            | E::LeashKnot
            | E::Interaction
            | E::ChestMinecart
            | E::FurnaceMinecart
            | E::HopperMinecart
            | E::TntMinecart
            | E::SpawnerMinecart
            | E::CommandBlockMinecart
    ) {
        return EntityUse::Unknown;
    }
    let Some(item) = held else {
        return EntityUse::Pass;
    };
    let id = item.to_str();
    if matches!(
        item,
        ItemKind::Lead | ItemKind::Shears | ItemKind::NameTag | ItemKind::Saddle
    ) || id.ends_with("_spawn_egg")
        || id.ends_with("_harness")
        || id.ends_with("nautilus_armor")
    {
        return EntityUse::Unknown;
    }
    let food =
        |tag: &str| item == ItemKind::GoldenDandelion || item_in_tag(tag, item) != Some(false);
    let claimed = match kind {
        E::Cow => item == ItemKind::Bucket || food("cow_food"),
        E::Mooshroom => {
            item == ItemKind::Bowl
                || item_in_tag("small_flowers", item) != Some(false)
                || food("cow_food")
        }
        E::Sheep => id.ends_with("_dye") || food("sheep_food"),
        E::Goat => item == ItemKind::Bucket || food("goat_food"),
        E::Chicken => food("chicken_food"),
        E::Rabbit => food("rabbit_food"),
        E::Fox => food("fox_food"),
        E::Frog => food("frog_food"),
        E::Turtle => food("turtle_food"),
        E::Sniffer => food("sniffer_food"),
        E::Hoglin => food("hoglin_food"),
        E::Bee => food("bee_food"),
        E::Ocelot => food("ocelot_food"),
        E::Armadillo => item == ItemKind::Brush || food("armadillo_food"),
        E::Axolotl => item == ItemKind::WaterBucket || food("axolotl_food"),
        E::Cod | E::Salmon | E::Pufferfish | E::TropicalFish => item == ItemKind::WaterBucket,
        E::Tadpole => matches!(
            item,
            ItemKind::WaterBucket | ItemKind::SlimeBall | ItemKind::GoldenDandelion
        ),
        E::Dolphin => item_in_tag("fishes", item) != Some(false),
        E::IronGolem => item == ItemKind::IronIngot,
        E::Creeper => matches!(item, ItemKind::FlintAndSteel | ItemKind::FireCharge),
        E::ZombieVillager => item == ItemKind::GoldenApple,
        E::Piglin => item == ItemKind::GoldIngot,
        _ => false,
    };
    if claimed {
        EntityUse::Unknown
    } else {
        EntityUse::Pass
    }
}

fn use_item(
    bot: &Client,
    shared: &SharedMutex,
    hand: InteractionHand,
    stack: HandStack,
    click: &Click,
) -> Used {
    if click.spectator {
        return Used::Pass;
    }
    send_use_item(bot, hand);
    if stack.on_cooldown {
        return Used::Pass;
    }
    if stack.kind == ItemKind::Trident && !crate::play::riptide::can_start_use(bot, hand) {
        return Used::Pass;
    }
    let used = if stack.use_duration > 0 && stack.kind != ItemKind::Brush {
        if !start_use(bot, shared, hand, &stack) {
            return Used::Pass;
        }
        #[cfg(feature = "audio")]
        if stack.kind == ItemKind::Spyglass {
            crate::play::player_sounds::spyglass(bot, true);
        }
        Used::Done {
            swing: is_bundle(stack.kind),
        }
    } else {
        let used = match instant_use(bot, hand, &stack) {
            Used::Pass => return Used::Pass,
            used => used,
        };
        if stack.consumable
            && let Some(consume) = hand_consumption(bot, hand)
        {
            crate::play::item_use::emit_local(
                bot,
                shared,
                stack.kind,
                &consume,
                crate::play::item_use::FINISH_PARTICLES,
            );
        }
        used
    };
    if stack.kind == ItemKind::WritableBook && click.fresh {
        open_book_editor(bot, shared, hand);
    }
    item_used(shared, hand);
    used
}

fn start_use(bot: &Client, shared: &SharedMutex, hand: InteractionHand, stack: &HandStack) -> bool {
    if stack.needs_appetite && !can_eat(bot) {
        return false;
    }
    start_using_item(
        stack.kind,
        stack.use_duration,
        hand,
        hand_consumption(bot, hand),
    );
    crate::play::spear::charge_started(bot, shared, hand);
    true
}

fn hand_consumption(bot: &Client, hand: InteractionHand) -> Option<Consumption> {
    let inventory = bot
        .component::<azalea::entity::inventory::Inventory>()
        .ok()?;
    inventory
        .get_equipment(hand.equipment())?
        .as_present()
        .and_then(Consumption::of)
}

fn fall_flying(bot: &Client) -> bool {
    bot.component::<azalea::entity::metadata::FallFlying>()
        .is_ok_and(|f| f.0)
}

fn can_eat(bot: &Client) -> bool {
    if bot
        .component::<azalea::entity::PlayerAbilities>()
        .is_ok_and(|abilities| abilities.invulnerable)
    {
        return true;
    }
    bot.hunger().is_ok_and(|hunger| hunger.food < 20)
}

#[derive(Clone, Copy)]
struct HandStack {
    kind: ItemKind,
    use_duration: i32,
    consumable: bool,
    needs_appetite: bool,
    swappable: bool,
    on_cooldown: bool,
    use_on_ends: bool,
}

fn hand_stack(bot: &Client, hand: InteractionHand) -> Option<HandStack> {
    use azalea_inventory::components::{Consumable, Equippable, Food};

    let inventory = bot
        .component::<azalea::entity::inventory::Inventory>()
        .ok()?;
    let ItemStack::Present(data) = inventory.get_equipment(hand.equipment())? else {
        return None;
    };
    Some(HandStack {
        kind: data.kind,
        use_duration: use_duration(data),
        consumable: data.get_component::<Consumable>().is_some(),
        needs_appetite: data
            .get_component::<Food>()
            .is_some_and(|f| !f.can_always_eat),
        swappable: data
            .get_component::<Equippable>()
            .is_some_and(|e| e.swappable),
        on_cooldown: crate::client::cooldowns::stack_percent(data) > 0,
        use_on_ends: use_on_ends(data.kind),
    })
}

fn use_on_ends(kind: ItemKind) -> bool {
    use ItemKind as I;
    if matches!(kind, I::LilyPad | I::Frogspawn) {
        return false;
    }
    let id = kind.to_str().trim_start_matches("minecraft:");
    matches!(
        kind,
        I::Painting
            | I::ItemFrame
            | I::GlowItemFrame
            | I::EndCrystal
            | I::Brush
            | I::FlintAndSteel
            | I::ArmorStand
            | I::FireCharge
            | I::DebugStick
    ) || id.ends_with("_spawn_egg")
        || id.ends_with("minecart")
        || crate::items::placed::placed_block(id).is_some()
}

fn instant_use(bot: &Client, hand: InteractionHand, stack: &HandStack) -> Used {
    use ItemKind as I;
    use azalea::block::fluid_state::{FluidKind, FluidState};
    use azalea::physics::clip::FluidPickType as Fluid;

    const SUCCESS: Used = Used::Done { swing: true };
    const CONSUME: Used = Used::Done { swing: false };
    let pass_unless = |done: bool, used: Used| if done { used } else { Used::Pass };
    let fluid_at = |state: Option<BlockState>| {
        state.map_or(FluidState::from(BlockState::AIR), FluidState::from)
    };

    let id = stack.kind.to_str().trim_start_matches("minecraft:");
    match stack.kind {
        I::CarrotOnAStick | I::WarpedFungusOnAStick => Used::Pass,
        I::FireworkRocket => pass_unless(fall_flying(bot), SUCCESS),
        I::EnderEye => pass_unless(
            !pov_hit(bot, Fluid::None)
                .is_some_and(|state| crate::blocks::block_name(state) == "end_portal_frame"),
            CONSUME,
        ),
        I::Bow => pass_unless(has_ammo(bot, false), CONSUME),
        I::Crossbow => pass_unless(crossbow_charged(bot, hand) || has_ammo(bot, true), CONSUME),
        I::Bucket => pass_unless(
            fluid_at(pov_hit(bot, Fluid::SourceOnly)).is_source(),
            SUCCESS,
        ),
        I::WaterBucket
        | I::LavaBucket
        | I::PufferfishBucket
        | I::SalmonBucket
        | I::CodBucket
        | I::TropicalFishBucket
        | I::AxolotlBucket
        | I::TadpoleBucket => pass_unless(pov_hit(bot, Fluid::None).is_some(), SUCCESS),
        I::GlassBottle => {
            let fills = fluid_at(pov_hit(bot, Fluid::SourceOnly)).kind == FluidKind::Water;
            #[cfg(feature = "audio")]
            if fills {
                crate::play::player_sounds::bottle_filled(bot);
            }
            pass_unless(fills, SUCCESS)
        }
        I::WritableBook
        | I::WrittenBook
        | I::WindCharge
        | I::SplashPotion
        | I::LingeringPotion
        | I::KnowledgeBook
        | I::Snowball
        | I::Map
        | I::Egg
        | I::BlueEgg
        | I::BrownEgg
        | I::FishingRod
        | I::ExperienceBottle
        | I::EnderPearl => SUCCESS,
        I::GoatHorn => {
            #[cfg(feature = "audio")]
            crate::play::player_sounds::goat_horn(bot, hand);
            start_using_item(stack.kind, 140, hand, None);
            CONSUME
        }
        _ if id.ends_with("_boat") || id.ends_with("_raft") => {
            pass_unless(pov_hit(bot, Fluid::Any).is_some(), SUCCESS)
        }
        _ if id.ends_with("_spawn_egg") => {
            pass_unless(pov_hit(bot, Fluid::SourceOnly).is_some(), SUCCESS)
        }
        _ if stack.consumable => pass_unless(!stack.needs_appetite || can_eat(bot), CONSUME),
        _ if stack.swappable => SUCCESS,
        _ => Used::Pass,
    }
}

fn pov_hit(bot: &Client, fluid: azalea::physics::clip::FluidPickType) -> Option<BlockState> {
    let hit = pov_block_hit(bot, fluid)?;
    bot.world().ok()?.read().get_block_state(hit.block_pos)
}

fn main_hand_kind(bot: &Client) -> ItemKind {
    bot.component::<azalea::entity::inventory::Inventory>()
        .map(|inv| inv.held_item().kind())
        .unwrap_or(ItemKind::Air)
}

fn pov_block_hit(
    bot: &Client,
    fluid: azalea::physics::clip::FluidPickType,
) -> Option<BlockHitResult> {
    use azalea::physics::clip::{BlockShapeType, ClipContext, clip};

    let eye = bot.eye_position().ok()?;
    let look = *bot.component::<azalea::entity::LookDirection>().ok()?;
    let range = bot
        .component::<azalea::entity::Attributes>()
        .ok()?
        .block_interaction_range
        .calculate();
    let held_item = main_hand_kind(bot);
    let world = bot.world().ok()?;
    let world = world.read();
    let hit = clip(
        &world.chunks,
        ClipContext {
            from: eye,
            to: eye + azalea::entity::view_vector(look) * range,
            block_shape_type: BlockShapeType::Outline,
            fluid_pick_type: fluid,
            held_item,
            entity: None,
        },
    );
    (!hit.miss).then_some(hit)
}

fn has_ammo(bot: &Client, fireworks: bool) -> bool {
    if bot
        .component::<azalea::entity::PlayerAbilities>()
        .is_ok_and(|a| a.instant_break)
    {
        return true;
    }
    let Ok(inventory) = bot.component::<azalea::entity::inventory::Inventory>() else {
        return false;
    };
    (9..=45).any(|i| match inventory.inventory_menu.slot(i) {
        Some(ItemStack::Present(data)) => {
            matches!(
                data.kind,
                ItemKind::Arrow | ItemKind::TippedArrow | ItemKind::SpectralArrow
            ) || (fireworks && data.kind == ItemKind::FireworkRocket)
        }
        _ => false,
    })
}

fn crossbow_charged(bot: &Client, hand: InteractionHand) -> bool {
    use azalea_inventory::components::ChargedProjectiles;

    bot.component::<azalea::entity::inventory::Inventory>()
        .is_ok_and(|inventory| {
            inventory
                .get_equipment(hand.equipment())
                .is_some_and(|stack| {
                    stack
                        .get_component::<ChargedProjectiles>()
                        .is_some_and(|c| !c.items.is_empty())
                })
        })
}

pub(crate) fn is_using_item() -> bool {
    USE_ITEM.lock().unwrap().started
}

pub(crate) fn using_hand() -> Option<InteractionHand> {
    let use_item = USE_ITEM.lock().unwrap();
    use_item.started.then_some(use_item.hand)
}

fn start_using_item(
    kind: ItemKind,
    duration: i32,
    hand: InteractionHand,
    consume: Option<Consumption>,
) {
    let mut use_item = USE_ITEM.lock().unwrap();
    if use_item.started {
        return;
    }
    use_item.started = true;
    use_item.item = Some(kind);
    use_item.consume = consume;
    use_item.remaining = duration;
    use_item.hand = hand;
}

fn stop_using_item() {
    let mut use_item = USE_ITEM.lock().unwrap();
    use_item.started = false;
    use_item.item = None;
    use_item.consume = None;
    use_item.remaining = 0;
}

pub(crate) fn note_use_completed() {
    USE_COMPLETED.store(true, Ordering::Relaxed);
}

fn complete_using_item(bot: &Client, shared: &SharedMutex) {
    let (item, consume, hand) = {
        let mut use_item = USE_ITEM.lock().unwrap();
        if !use_item.started {
            return;
        }
        (use_item.item, use_item.consume.take(), use_item.hand)
    };
    let (Some(kind), Some(consume)) = (item, consume) else {
        #[cfg(feature = "audio")]
        if item == Some(ItemKind::Spyglass) {
            crate::play::player_sounds::spyglass(bot, false);
        }
        stop_using_item();
        return;
    };
    crate::play::item_use::emit_local(
        bot,
        shared,
        kind,
        &consume,
        crate::play::item_use::FINISH_PARTICLES,
    );
    let more_left = bot
        .component::<azalea::entity::inventory::Inventory>()
        .is_ok_and(|inventory| {
            inventory
                .get_equipment(hand.equipment())
                .and_then(ItemStack::as_present)
                .is_some_and(|held| held.kind == kind && held.count > 1)
        });
    if more_left {
        stop_using_item();
    }
}

fn release_using_item(bot: &Client) {
    bot.write_packet(ServerboundPlayerAction {
        action: Action::ReleaseUseItem,
        pos: BlockPos::new(0, 0, 0),
        direction: Direction::Down,
        seq: 0,
    });
    let (item, remaining, hand) = {
        let use_item = USE_ITEM.lock().unwrap();
        (use_item.item, use_item.remaining, use_item.hand)
    };
    if item == Some(ItemKind::Trident) && held_item_kind(bot, hand) == item {
        crate::play::riptide::release(bot, hand, HELD_INDEFINITELY - remaining);
    }
    #[cfg(feature = "audio")]
    if item == Some(ItemKind::Spyglass) && held_item_kind(bot, hand) == item {
        crate::play::player_sounds::spyglass(bot, false);
    }
    stop_using_item();
}

fn updating_using_item(bot: &Client, shared: &SharedMutex) {
    let (started, item, hand) = {
        let use_item = USE_ITEM.lock().unwrap();
        (use_item.started, use_item.item, use_item.hand)
    };
    if !started {
        return;
    }
    if held_item_kind(bot, hand) != item {
        stop_using_item();
        return;
    }
    let (due, _remaining) = {
        let mut use_item = USE_ITEM.lock().unwrap();
        let remaining = use_item.remaining;
        use_item.remaining -= 1;
        let due = use_item
            .consume
            .as_ref()
            .filter(|consume| consume.should_emit(remaining))
            .cloned();
        (due, remaining)
    };
    #[cfg(feature = "audio")]
    if item == Some(ItemKind::Brush)
        && _remaining >= 0
        && (BRUSH_USE_DURATION - _remaining + 1) % 10 == 5
        && let Some(hit) = block_hit(bot)
        && let Ok(world) = bot.world()
        && let Some(state) = world.read().get_block_state(hit.block_pos)
    {
        crate::play::player_sounds::brush_stroke(state, hit.block_pos);
    }
    #[cfg(feature = "audio")]
    if item.is_some_and(is_bundle)
        && _remaining > 0
        && (_remaining == BUNDLE_USE_DURATION
            || (_remaining < BUNDLE_USE_DURATION - 10 && _remaining % 2 == 0))
    {
        crate::play::player_sounds::bundle_drop(bot, hand, _remaining == BUNDLE_USE_DURATION);
    }
    if let (Some(consume), Some(kind)) = (due, item) {
        crate::play::item_use::emit_local(
            bot,
            shared,
            kind,
            &consume,
            crate::play::item_use::USE_TICK_PARTICLES,
        );
    }
}

fn sync_server_using_item(bot: &Client, shared: &SharedMutex) {
    let completed = USE_COMPLETED.swap(false, Ordering::Relaxed);
    if completed {
        complete_using_item(bot, shared);
    }
    let server_using = bot
        .component::<azalea::entity::metadata::AbstractLivingUsingItem>()
        .map(|using| using.0)
        .unwrap_or(false);
    if SERVER_USING_ITEM.swap(server_using, Ordering::Relaxed) == server_using {
        return;
    }
    match (server_using, is_using_item()) {
        (true, false) if !completed => {
            let hand = if bot
                .component::<azalea::entity::metadata::AbstractLivingUsingOffhand>()
                .is_ok_and(|off| off.0)
            {
                InteractionHand::Off
            } else {
                InteractionHand::Main
            };
            if let Some(stack) = hand_stack(bot, hand) {
                start_using_item(
                    stack.kind,
                    stack.use_duration,
                    hand,
                    hand_consumption(bot, hand),
                );
            }
        }
        (false, true) => stop_using_item(),
        _ => {}
    }
}

fn held_item_kind(bot: &Client, hand: InteractionHand) -> Option<ItemKind> {
    let inventory = bot
        .component::<azalea::entity::inventory::Inventory>()
        .ok()?;
    match inventory.get_equipment(hand.equipment())? {
        ItemStack::Present(data) => Some(data.kind),
        ItemStack::Empty => None,
    }
}

pub(crate) fn use_duration(held: &azalea_inventory::ItemStackData) -> i32 {
    use azalea_inventory::components::{BlocksAttacks, KineticWeapon};

    if let Some(consume) = Consumption::of(held) {
        return consume.ticks;
    }
    if held.kind == ItemKind::Trident
        || held.get_component::<BlocksAttacks>().is_some()
        || held.get_component::<KineticWeapon>().is_some()
    {
        return HELD_INDEFINITELY;
    }
    match held.kind {
        ItemKind::Spyglass => SPYGLASS_USE_DURATION,
        ItemKind::Brush => BRUSH_USE_DURATION,
        kind if is_bundle(kind) => BUNDLE_USE_DURATION,
        _ => 0,
    }
}

const SPYGLASS_USE_DURATION: i32 = 1200;
const BRUSH_USE_DURATION: i32 = 200;
const BUNDLE_USE_DURATION: i32 = 200;

fn is_bundle(kind: ItemKind) -> bool {
    azalea_inventory::bundle::is_bundle(kind)
}

const HELD_INDEFINITELY: i32 = 72000;

fn send_use_item_on(bot: &Client, hit: &BlockHitResult, interacted: bool, hand: InteractionHand) {
    use azalea_protocol::packets::game::s_use_item_on::ServerboundUseItemOn;

    let Some(seq) = next_sequence(bot) else {
        return;
    };
    if interacted {
        predict_open_toggle(bot, hit);
    }
    bot.write_packet(ServerboundUseItemOn {
        hand: hand.packet(),
        block_hit: hit.into(),
        seq,
    });
}

#[cfg(lua_mods)]
pub(crate) fn place_at(
    bot: &Client,
    hit: &BlockHitResult,
    offhand: bool,
    slot: Option<u8>,
    sneak: bool,
) {
    use azalea::movement::LastSentInput;
    use azalea_protocol::packets::game::{
        s_interact::InteractionHand, s_player_input::ServerboundPlayerInput,
        s_use_item_on::ServerboundUseItemOn,
    };

    if let Some(slot) = slot {
        select_hotbar_slot(bot, slot);
    }
    let input = bot
        .component::<LastSentInput>()
        .map(|i| i.0.clone())
        .unwrap_or_default();
    let press = sneak && !input.shift;
    if press {
        bot.write_packet(ServerboundPlayerInput {
            shift: true,
            ..input.clone()
        });
    }
    if let Some(seq) = next_sequence(bot) {
        bot.write_packet(ServerboundUseItemOn {
            hand: if offhand {
                InteractionHand::OffHand
            } else {
                InteractionHand::MainHand
            },
            block_hit: hit.into(),
            seq,
        });
    }
    if press {
        bot.write_packet(input);
    }
}

fn predict_open_toggle(bot: &Client, hit: &BlockHitResult) {
    use azalea::interact::BlockStatePredictionHandler;

    if bot.is_mining() {
        return;
    }
    if matches!(
        bot.component::<GameMode>().map(|gm| *gm),
        Ok(GameMode::Spectator)
    ) {
        return;
    }
    let (Ok(world), Ok(player_pos)) = (bot.world(), bot.position()) else {
        return;
    };

    #[cfg(feature = "audio")]
    {
        let clicked = world.read().get_block_state(hit.block_pos);
        let may_build = !matches!(
            bot.component::<GameMode>().map(|gm| *gm),
            Ok(GameMode::Adventure)
        );
        if let Some(clicked) = clicked {
            crate::play::player_sounds::block_used(clicked, hit.block_pos, may_build);
        }
    }

    let writes = {
        let world = world.read();
        let Some(old) = world.get_block_state(hit.block_pos) else {
            return;
        };
        let Some(new) = crate::play::placement::toggled_open(old) else {
            return;
        };
        let mut writes = vec![(hit.block_pos, old, new)];
        if let Some(other) = crate::play::placement::door_other_half(old, new, hit.block_pos, |p| {
            world.get_block_state(p)
        }) {
            writes.push(other);
        }
        writes
    };

    {
        let mut ecs = bot.ecs.write();
        let Some(mut handler) = ecs.get_mut::<BlockStatePredictionHandler>(bot.entity) else {
            return;
        };
        for (pos, old, _) in &writes {
            handler.retain_known_server_state(*pos, *old, player_pos);
        }
    }

    let world = world.read();
    for (pos, _, new) in &writes {
        world.set_block_state(*pos, *new);
    }
}

pub(crate) fn swing_hand(bot: &Client, shared: &SharedMutex, hand: InteractionHand) {
    use azalea_protocol::packets::game::s_swing::ServerboundSwing;

    crate::client::tick::swing_local_player(hand == InteractionHand::Off, shared);
    bot.write_packet(ServerboundSwing {
        hand: hand.packet(),
    });
}

fn item_used(shared: &SharedMutex, hand: InteractionHand) {
    let mut s = shared.lock().unwrap();
    let used = &mut s.session.item_used[hand as usize];
    *used = used.wrapping_add(1);
}

fn consume_held_item(bot: &Client, hand: InteractionHand) {
    if !matches!(
        bot.component::<GameMode>().map(|gm| *gm),
        Ok(GameMode::Survival)
    ) {
        return;
    }
    let Ok(selected) = bot.selected_hotbar_slot() else {
        return;
    };
    let slot_index = hand.menu_slot(selected);
    let mut ecs = bot.ecs.write();
    if let Some(mut inv) = ecs.get_mut::<azalea::entity::inventory::Inventory>(bot.entity)
        && let Some(slot) = inv.inventory_menu.slot_mut(slot_index)
        && let azalea_inventory::ItemStack::Present(data) = slot
    {
        data.count -= 1.min(data.count);
        if data.count == 0 {
            *slot = azalea_inventory::ItemStack::Empty;
        }
    }
}

fn fill_held_bucket(bot: &Client, filled: ItemKind, hand: InteractionHand) {
    if !matches!(
        bot.component::<GameMode>().map(|gm| *gm),
        Ok(GameMode::Survival)
    ) {
        return;
    }
    let Ok(selected) = bot.selected_hotbar_slot() else {
        return;
    };
    let slot_index = hand.menu_slot(selected);
    let mut ecs = bot.ecs.write();
    if let Some(mut inv) = ecs.get_mut::<azalea::entity::inventory::Inventory>(bot.entity)
        && let Some(slot) = inv.inventory_menu.slot_mut(slot_index)
        && let azalea_inventory::ItemStack::Present(data) = slot
        && data.kind == ItemKind::Bucket
        && data.count == 1
    {
        data.kind = filled;
    }
}

fn used_block_interaction(
    bot: &Client,
    hit: &BlockHitResult,
    hand: InteractionHand,
    secondary_use: bool,
) -> bool {
    if hand == InteractionHand::Off {
        return false;
    }
    let has_item = bot
        .component::<azalea::entity::inventory::Inventory>()
        .is_ok_and(|inv| {
            InteractionHand::BOTH.iter().any(|hand| {
                inv.get_equipment(hand.equipment())
                    .is_some_and(|s| !s.is_empty())
            })
        });
    if secondary_use && has_item {
        return false;
    }
    let Ok(world) = bot.world() else { return false };
    let state = world.read().get_block_state(hit.block_pos);
    let held = held_item_kind(bot, hand);
    state.is_some_and(|state| {
        crate::play::placement::has_use_interaction(state) || cauldron_takes(state, held)
    })
}

fn cauldron_takes(state: BlockState, held: Option<ItemKind>) -> bool {
    use azalea_registry::builtin::BlockKind;
    let fills = matches!(
        held,
        Some(ItemKind::WaterBucket | ItemKind::LavaBucket | ItemKind::PowderSnowBucket)
    );
    match state.as_block_kind() {
        BlockKind::Cauldron => fills,
        BlockKind::LavaCauldron => fills || held == Some(ItemKind::Bucket),
        _ => false,
    }
}

fn open_game_master_block(bot: &Client, shared: &SharedMutex, hit: &BlockHitResult) {
    let creative = bot
        .component::<GameMode>()
        .is_ok_and(|gm| *gm == GameMode::Creative);
    if !creative || shared.lock().unwrap().session.op_level < 2 {
        return;
    }
    let Ok(world) = bot.world() else { return };
    let Some(state) = world.read().get_block_state(hit.block_pos) else {
        return;
    };
    match command_block_mode(state) {
        Some(mode) => open_command_block(shared, hit, state, mode),
        None => open_operator_block(shared, hit, state),
    }
}

fn open_operator_block(shared: &SharedMutex, hit: &BlockHitResult, state: BlockState) {
    use crate::blockentities::game_master::{
        JigsawEdit, StructureEdit, TestBlockEdit, TestInstanceEdit,
    };
    use crate::session::GameMasterOpen;
    use azalea_registry::builtin::BlockEntityKind;

    let pos = hit.block_pos;
    let Some((kind, tag)) = crate::blockentities::feed::game_master_tag([pos.x, pos.y, pos.z])
    else {
        return;
    };
    let block = Box::<dyn azalea::block::BlockTrait>::from(state);
    let tag = tag.as_deref();
    let open = match (crate::blocks::block_name(state), kind) {
        ("structure_block", BlockEntityKind::StructureBlock) => {
            GameMasterOpen::Structure(pos, StructureEdit::decode(tag, block.get_property("mode")))
        }
        ("jigsaw", BlockEntityKind::Jigsaw) => GameMasterOpen::Jigsaw(
            pos,
            JigsawEdit::decode(tag, block.get_property("orientation").unwrap_or("")),
        ),
        ("test_block", BlockEntityKind::TestBlock) => {
            GameMasterOpen::Test(pos, TestBlockEdit::decode(tag, block.get_property("mode")))
        }
        ("test_instance_block", BlockEntityKind::TestInstanceBlock) => {
            GameMasterOpen::TestInstance(pos, TestInstanceEdit::decode(tag))
        }
        _ => return,
    };
    let mut s = shared.lock().unwrap();
    if let GameMasterOpen::TestInstance(..) = open {
        s.session.test_instance_pos = Some(pos);
        s.session.test_instance_status.clear();
    }
    s.session.game_master_open = Some(open);
}

fn open_command_block(
    shared: &SharedMutex,
    hit: &BlockHitResult,
    state: BlockState,
    mode: CommandBlockMode,
) {
    let conditional = Box::<dyn azalea::block::BlockTrait>::from(state).get_property("conditional")
        == Some("true");
    let mut s = shared.lock().unwrap();
    s.session.command_block_open = Some(crate::session::CommandBlockOpen {
        pos: hit.block_pos,
        mode,
        conditional,
    });
    s.session.command_block_pos = Some(hit.block_pos);
}

pub(crate) fn command_block_mode(state: azalea::block::BlockState) -> Option<CommandBlockMode> {
    match crate::blocks::block_name(state) {
        "command_block" => Some(CommandBlockMode::Redstone),
        "repeating_command_block" => Some(CommandBlockMode::Auto),
        "chain_command_block" => Some(CommandBlockMode::Sequence),
        _ => None,
    }
}

fn predict_place(bot: &Client, secondary_use: bool, hand: InteractionHand) -> bool {
    use azalea_protocol::packets::game::s_use_item_on::ServerboundUseItemOn;

    let Some(predicted) = predict_use(bot, UseKind::Place, secondary_use, None, hand) else {
        return false;
    };

    bot.write_packet(ServerboundUseItemOn {
        hand: hand.packet(),
        block_hit: (&predicted.hit).into(),
        seq: predicted.seq,
    });
    crate::modules::scaffold::note_hand_place(predicted.pos);
    true
}

pub(crate) fn module_place(
    bot: &Client,
    shared: &SharedMutex,
    hit: &BlockHitResult,
    look: (f32, f32),
    secondary_use: bool,
) -> bool {
    use azalea_protocol::packets::game::s_use_item_on::ServerboundUseItemOn;

    let main = InteractionHand::Main;
    let Some(predicted) = predict_use(
        bot,
        UseKind::Place,
        secondary_use,
        Some((hit.clone(), look)),
        main,
    ) else {
        return false;
    };
    bot.write_packet(ServerboundUseItemOn {
        hand: main.packet(),
        block_hit: (&predicted.hit).into(),
        seq: predicted.seq,
    });
    consume_held_item(bot, main);
    item_used(shared, main);
    swing_hand(bot, shared, main);
    true
}

pub(crate) fn module_use_on(
    bot: &Client,
    shared: &SharedMutex,
    hit: &BlockHitResult,
    hand: InteractionHand,
) {
    send_use_item_on(bot, hit, false, hand);
    consume_held_item(bot, hand);
    item_used(shared, hand);
    swing_hand(bot, shared, hand);
}

pub(crate) fn select_hotbar_slot(bot: &Client, slot: u8) {
    use azalea::inventory::LastSentSelectedHotbarSlot;
    use azalea_protocol::packets::game::s_set_carried_item::ServerboundSetCarriedItem;

    if !bot.selected_hotbar_slot().is_ok_and(|s| s != slot) {
        return;
    }
    bot.set_selected_hotbar_slot(slot);
    bot.write_packet(ServerboundSetCarriedItem { slot: slot as u16 });
    bot.ecs
        .write()
        .entity_mut(bot.entity)
        .insert(LastSentSelectedHotbarSlot { slot });
}

fn predict_bucket_use(bot: &Client, shared: &SharedMutex, hand: InteractionHand) -> bool {
    use azalea_protocol::packets::game::ServerboundUseItem;
    use azalea_protocol::packets::game::s_use_item_on::ServerboundUseItemOn;

    let Some(predicted) = predict_use(bot, UseKind::Bucket, false, None, hand) else {
        return false;
    };

    if let Some((hit, seq)) = &predicted.use_on {
        bot.write_packet(ServerboundUseItemOn {
            hand: hand.packet(),
            block_hit: hit.into(),
            seq: *seq,
        });
    }
    bot.write_packet(ServerboundUseItem {
        hand: hand.packet(),
        seq: predicted.seq,
        x_rot: predicted.x_rot,
        y_rot: predicted.y_rot,
    });
    if predicted.evaporated {
        evaporate(shared, predicted.pos);
    }
    true
}

fn evaporate(shared: &SharedMutex, pos: BlockPos) {
    use crate::session::{LevelParticle, ParticleEmit, push_particle_emits};
    use crate::util::javarandom::JavaRandom;
    use crate::util::level_events::{Bus, play};

    let mut rng = JavaRandom::time_seeded();
    let pitch = 2.6 + (rng.next_f32() - rng.next_f32()) * 0.8;
    let at = crate::util::block_centre([pos.x, pos.y, pos.z]).to_array();
    play("block.fire.extinguish", Bus::Blocks, at, 0.5, pitch);
    let smoke = (0..8).map(|_| {
        let [x, y, z] = [rng.next_f32(), rng.next_f32(), rng.next_f32()];
        ParticleEmit::Level(LevelParticle::add(
            azalea::entity::particle::Particle::LargeSmoke,
            [
                f64::from(pos.x as f32 + x),
                f64::from(pos.y as f32 + y),
                f64::from(pos.z as f32 + z),
            ],
            [0.0; 3],
        ))
    });
    push_particle_emits(&mut shared.lock().unwrap().session.particle_emits, smoke);
}

fn open_book_editor(bot: &Client, shared: &SharedMutex, hand: InteractionHand) {
    let author = bot.username();
    shared.lock().unwrap().session.book_edit_open =
        Some(crate::session::BookEditRequest { hand, author });
}

fn next_sequence(bot: &Client) -> Option<u32> {
    use azalea::interact::BlockStatePredictionHandler;

    let mut ecs = bot.ecs.write();
    let mut handler = ecs.get_mut::<BlockStatePredictionHandler>(bot.entity)?;
    Some(handler.start_predicting())
}

fn send_use_item(bot: &Client, hand: InteractionHand) {
    use azalea_protocol::packets::game::ServerboundUseItem;

    let Ok(look) = bot.component::<azalea::entity::LookDirection>() else {
        return;
    };
    let (x_rot, y_rot) = (look.x_rot(), look.y_rot());
    drop(look);

    let Some(seq) = next_sequence(bot) else {
        return;
    };

    bot.write_packet(ServerboundUseItem {
        hand: hand.packet(),
        seq,
        x_rot,
        y_rot,
    });
}

enum UseKind {
    Place,
    Bucket,
}

enum BucketAction {
    Empty(ItemKind),
    Evaporate,
    Fill(ItemKind),
}

struct PlannedWrite {
    pos: BlockPos,
    new: BlockState,
    old: BlockState,
    bucket: Option<BucketAction>,
}

struct Predicted {
    hit: BlockHitResult,
    pos: BlockPos,
    x_rot: f32,
    y_rot: f32,
    seq: u32,
    use_on: Option<(BlockHitResult, u32)>,
    evaporated: bool,
}

fn predict_use(
    bot: &Client,
    kind: UseKind,
    secondary_use: bool,
    aimed: Option<(BlockHitResult, (f32, f32))>,
    hand: InteractionHand,
) -> Option<Predicted> {
    use azalea::interact::BlockStatePredictionHandler;

    let creative = match bot.component::<GameMode>().map(|gm| *gm) {
        Ok(GameMode::Survival) => false,
        Ok(GameMode::Creative) => true,
        _ => return None,
    };

    if bot.is_mining() {
        return None;
    }

    let crosshair = block_hit(bot);
    let (hit, (x_rot, y_rot)) = match aimed {
        Some(aimed) => aimed,
        None => {
            let fills = matches!(kind, UseKind::Bucket)
                && held_item_kind(bot, hand) == Some(ItemKind::Bucket);
            let hit = if fills {
                pov_block_hit(bot, azalea::physics::clip::FluidPickType::SourceOnly)?
            } else {
                crosshair.clone()?
            };
            let look = bot.component::<azalea::entity::LookDirection>().ok()?;
            (hit, (look.x_rot(), look.y_rot()))
        }
    };
    if !within_border(bot, hit.block_pos)
        || crosshair
            .as_ref()
            .is_some_and(|c| !within_border(bot, c.block_pos))
    {
        return None;
    }
    let (Ok(world), Ok(inventory), Ok(player_pos), Ok(physics)) = (
        bot.world(),
        bot.component::<azalea::entity::inventory::Inventory>(),
        bot.position(),
        bot.component::<azalea::entity::Physics>(),
    ) else {
        return None;
    };
    let player_box = physics.bounding_box;
    drop(physics);

    let predicted = {
        let world = world.read();
        let held = inventory.get_equipment(hand.equipment())?;
        let min_y = world.chunks.min_y();
        let height = world.chunks.height();

        let placed = match kind {
            UseKind::Place => crate::play::placement::predict(
                held,
                &hit,
                y_rot,
                x_rot,
                secondary_use,
                min_y,
                height,
                |pos| world.get_block_state(pos),
            )
            .filter(|&(pos, state)| {
                crate::play::placement::is_unobstructed(state, pos, &[player_box])
            })
            .map(|(pos, state)| (pos, state, None)),
            UseKind::Bucket => match held {
                ItemStack::Present(item)
                    if crate::play::placement::bucket_fluid(item.kind).is_some() =>
                {
                    let evaporates = crate::play::placement::bucket_fluid(item.kind)
                        == Some(azalea_registry::builtin::BlockKind::Water)
                        && crate::renderer::dimension::current()
                            == crate::renderer::dimension::Dimension::Nether;
                    crate::play::placement::predict_bucket(held, &hit, min_y, height, |pos| {
                        world.get_block_state(pos)
                    })
                    .map(|(pos, state)| {
                        let action = if evaporates {
                            BucketAction::Evaporate
                        } else {
                            BucketAction::Empty(item.kind)
                        };
                        (pos, state, Some(action))
                    })
                }
                _ => crate::play::placement::predict_bucket_fill(held, &hit, creative, |pos| {
                    world.get_block_state(pos)
                })
                .map(|(pos, new, filled)| (pos, new, Some(BucketAction::Fill(filled)))),
            },
        };

        placed.and_then(|(pos, new, bucket)| {
            world.get_block_state(pos).map(|old| PlannedWrite {
                pos,
                new: if matches!(bucket, Some(BucketAction::Evaporate)) {
                    old
                } else {
                    new
                },
                old,
                bucket,
            })
        })
    };
    drop(inventory);
    let PlannedWrite {
        pos,
        new: state,
        old: old_state,
        bucket: bucket_action,
    } = predicted?;

    let (seq, use_on) = {
        let mut ecs = bot.ecs.write();
        let mut handler = ecs.get_mut::<BlockStatePredictionHandler>(bot.entity)?;
        let use_on = crosshair
            .filter(|_| matches!(kind, UseKind::Bucket))
            .map(|hit| (hit, handler.start_predicting()));
        let seq = handler.start_predicting();
        handler.retain_known_server_state(pos, old_state, player_pos);
        (seq, use_on)
    };

    if state != old_state {
        world.read().set_block_state(pos, state);
    }
    let evaporated = matches!(bucket_action, Some(BucketAction::Evaporate));

    match bucket_action {
        Some(BucketAction::Empty(bucket)) => {
            use crate::util::level_events::{Bus, play};
            let (event, bus) = match bucket {
                ItemKind::LavaBucket => ("item.bucket.empty_lava", Bus::Blocks),
                ItemKind::WaterBucket => ("item.bucket.empty", Bus::Blocks),
                ItemKind::AxolotlBucket => ("item.bucket.empty_axolotl", Bus::Neutral),
                ItemKind::TadpoleBucket => ("item.bucket.empty_tadpole", Bus::Neutral),
                _ => ("item.bucket.empty_fish", Bus::Neutral),
            };
            play(
                event,
                bus,
                crate::util::block_centre([pos.x, pos.y, pos.z]).to_array(),
                1.0,
                1.0,
            );
        }
        Some(BucketAction::Evaporate) => {}
        Some(BucketAction::Fill(filled)) => {
            use crate::util::level_events::{Bus, play};
            fill_held_bucket(bot, filled, hand);
            let event = match filled {
                ItemKind::LavaBucket => "item.bucket.fill_lava",
                ItemKind::PowderSnowBucket => "item.bucket.fill_powder_snow",
                _ => "item.bucket.fill",
            };
            play(
                event,
                Bus::Players,
                [player_pos.x, player_pos.y, player_pos.z],
                1.0,
                1.0,
            );
        }
        None => {
            #[cfg(feature = "audio")]
            {
                let group = crate::blocks::sound_types::of(state);
                let at = crate::util::block_centre([pos.x, pos.y, pos.z]).to_array();
                let event =
                    if state.as_block_kind() == azalea_registry::builtin::BlockKind::PowderSnow {
                        "item.bucket.empty_powder_snow"
                    } else {
                        group.place_event
                    };
                crate::audio::play_at(
                    event,
                    crate::audio::SoundCategory::Blocks,
                    at,
                    (group.volume + 1.0) / 2.0,
                    group.pitch * 0.8,
                );
            }
        }
    }

    Some(Predicted {
        hit,
        pos,
        x_rot,
        y_rot,
        seq,
        use_on,
        evaporated,
    })
}

fn living_target(bot: &Client) -> bool {
    let Ok(HitResult::Entity(hit)) = bot.hit_result() else {
        return false;
    };
    bot.entity_component::<azalea::entity::metadata::Health>(hit.entity)
        .map(|h| h.0 > 0.0)
        .unwrap_or(false)
}

fn crosshair_kind(bot: &Client) -> Option<azalea_registry::builtin::EntityKind> {
    let Ok(HitResult::Entity(hit)) = bot.hit_result() else {
        return None;
    };
    if !bot
        .entity_component::<azalea::entity::metadata::Health>(hit.entity)
        .is_ok_and(|h| h.0 > 0.0)
    {
        return None;
    }
    bot.entity_component::<azalea::entity::EntityKindComponent>(hit.entity)
        .ok()
        .map(|k| k.0)
}

fn picked_entity(bot: &Client) -> Option<i32> {
    let Ok(HitResult::Entity(hit)) = bot.hit_result() else {
        return None;
    };
    bot.entity_component::<azalea_core::entity_id::MinecraftEntityId>(hit.entity)
        .map(|id| id.0)
        .ok()
}

fn apply_pick_block(bot: &Client, ctrl: bool) {
    use azalea_protocol::packets::game::{
        s_pick_item_from_block::ServerboundPickItemFromBlock,
        s_pick_item_from_entity::ServerboundPickItemFromEntity,
    };

    match bot.hit_result() {
        Ok(HitResult::Block(b)) if !b.miss => {
            bot.write_packet(ServerboundPickItemFromBlock {
                pos: b.block_pos,
                include_data: ctrl,
            });
        }
        Ok(HitResult::Entity(e)) => {
            let Ok(id) =
                bot.entity_component::<azalea_core::entity_id::MinecraftEntityId>(e.entity)
            else {
                return;
            };
            bot.write_packet(ServerboundPickItemFromEntity {
                id: *id,
                include_data: ctrl,
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: BlockPos = BlockPos { x: 1, y: 2, z: 3 };
    const Q: BlockPos = BlockPos { x: 1, y: 2, z: 4 };

    fn target(kind: azalea_registry::builtin::EntityKind) -> EntityTarget {
        EntityTarget {
            kind,
            has_riders: false,
            hull_under_water: false,
        }
    }

    #[test]
    fn a_boat_click_swings_and_a_sneak_passes() {
        use azalea_registry::builtin::EntityKind;
        for kind in [EntityKind::OakBoat, EntityKind::BambooChestRaft] {
            assert_eq!(entity_use(target(kind), false, None), EntityUse::Success);
            assert_eq!(
                entity_use(target(kind), false, Some(ItemKind::Bread)),
                EntityUse::Success
            );
            assert_eq!(
                entity_use(target(kind), true, Some(ItemKind::Bread)),
                EntityUse::Pass
            );
        }
    }

    #[test]
    fn entity_use_declines_what_it_cannot_know() {
        use azalea_registry::builtin::EntityKind;
        let ridden = |kind| EntityTarget {
            has_riders: true,
            ..target(kind)
        };
        assert_eq!(
            entity_use(ridden(EntityKind::OakBoat), false, None),
            EntityUse::Success
        );
        assert_eq!(
            entity_use(ridden(EntityKind::Minecart), false, None),
            EntityUse::Pass
        );
        assert_eq!(
            entity_use(target(EntityKind::Minecart), false, None),
            EntityUse::Success
        );

        let sunk = EntityTarget {
            hull_under_water: true,
            ..target(EntityKind::OakBoat)
        };
        assert_eq!(entity_use(sunk, false, None), EntityUse::Unknown);
        assert_eq!(
            entity_use(target(EntityKind::OakBoat), false, Some(ItemKind::Lead)),
            EntityUse::Unknown
        );
        assert_eq!(
            entity_use(target(EntityKind::Villager), false, None),
            EntityUse::Unknown
        );
        assert_eq!(
            entity_use(target(EntityKind::ChestMinecart), false, None),
            EntityUse::Unknown
        );
    }

    #[test]
    fn entity_use_passes_items_mobs_ignore() {
        use azalea_registry::builtin::EntityKind;
        for item in [
            ItemKind::Bow,
            ItemKind::Shield,
            ItemKind::Bread,
            ItemKind::Potion,
        ] {
            assert_eq!(
                entity_use(target(EntityKind::Player), false, Some(item)),
                EntityUse::Pass
            );
            assert_eq!(
                entity_use(target(EntityKind::Zombie), false, Some(item)),
                EntityUse::Pass
            );
            assert_eq!(
                entity_use(target(EntityKind::Villager), false, Some(item)),
                EntityUse::Unknown
            );
        }
        assert_eq!(
            entity_use(target(EntityKind::Cow), false, Some(ItemKind::Bucket)),
            EntityUse::Unknown
        );
        assert_eq!(
            entity_use(target(EntityKind::Zombie), false, Some(ItemKind::NameTag)),
            EntityUse::Unknown
        );
        assert_eq!(
            entity_use(
                target(EntityKind::Creeper),
                false,
                Some(ItemKind::FlintAndSteel)
            ),
            EntityUse::Unknown
        );
        assert_eq!(
            entity_use(target(EntityKind::Player), false, Some(ItemKind::Shears)),
            EntityUse::Unknown
        );
    }

    #[test]
    fn use_on_ends_for_block_items_and_the_never_pass_overrides() {
        for item in [
            ItemKind::Dirt,
            ItemKind::Torch,
            ItemKind::SweetBerries,
            ItemKind::WheatSeeds,
            ItemKind::PigSpawnEgg,
            ItemKind::Minecart,
            ItemKind::ChestMinecart,
            ItemKind::ItemFrame,
            ItemKind::FlintAndSteel,
        ] {
            assert!(use_on_ends(item), "{item:?}");
        }
        for item in [
            ItemKind::DiamondSword,
            ItemKind::Shield,
            ItemKind::Bread,
            ItemKind::Bucket,
            ItemKind::WaterBucket,
            ItemKind::Wheat,
            ItemKind::TotemOfUndying,
            ItemKind::LilyPad,
        ] {
            assert!(!use_on_ends(item), "{item:?}");
        }
    }

    #[test]
    fn creative_holds_the_destroy_delay() {
        let (start, delay) = break_step(true, true, 0, false, None, Some(P));
        assert!(start);
        assert_eq!(delay, DESTROY_DELAY);

        let mut delay = delay;
        for _ in 0..DESTROY_DELAY {
            let (start, next) = break_step(true, false, delay, false, None, Some(P));
            assert!(!start, "creative broke a block during the delay");
            delay = next;
        }
        assert_eq!(delay, 0);
        assert_eq!(break_step(true, false, delay, false, None, Some(P)).0, true);
    }

    #[test]
    fn survival_starts_once_then_leaves_azalea_to_continue() {
        let (start, delay) = break_step(false, true, 0, false, None, Some(P));
        assert!(start);
        assert_eq!(delay, 0, "survival must not arm the delay while mining");

        for _ in 0..10 {
            assert_eq!(
                break_step(false, false, 0, true, Some(P), Some(P)),
                (false, 0)
            );
        }
    }

    #[test]
    fn survival_restarts_on_target_change() {
        assert_eq!(
            break_step(false, false, 0, true, Some(P), Some(Q)),
            (true, 0)
        );
    }

    #[test]
    fn survival_arms_the_delay_when_the_break_finishes() {
        let (start, delay) = break_step(false, false, 0, true, None, Some(Q));
        assert!(!start);
        assert_eq!(
            delay,
            DESTROY_DELAY - 1,
            "the finishing tick also decrements"
        );
    }

    #[test]
    fn a_held_button_waits_out_the_delay() {
        let (swing, delay) = place_step(GameMode::Creative, true, false, 0, false, || true);
        assert!(swing);
        assert_eq!(delay, RIGHT_CLICK_DELAY);

        let mut delay = delay;
        for _ in 1..RIGHT_CLICK_DELAY {
            let (swing, next) = place_step(GameMode::Creative, false, true, delay, false, || {
                panic!("used during the delay")
            });
            assert!(!swing, "the arm swung on a repeat the delay refused");
            delay = next;
        }
        assert_eq!(delay, 1);
    }

    #[test]
    fn a_fresh_click_ignores_the_delay() {
        let mut uses = 0;
        let (_, delay) = place_step(
            GameMode::Survival,
            true,
            true,
            RIGHT_CLICK_DELAY,
            false,
            || {
                uses += 1;
                true
            },
        );
        assert_eq!(uses, 1);
        assert_eq!(delay, RIGHT_CLICK_DELAY);
    }

    #[test]
    fn a_held_right_click_swings_on_every_repeat() {
        let mut delay = 0;
        let mut swings = 0;
        let mut uses = 0;
        for _ in 0..21 {
            let (swing, next) = place_step(GameMode::Survival, false, true, delay, false, || {
                uses += 1;
                true
            });
            swings += swing as u32;
            delay = next;
        }
        assert_eq!(uses, 6);
        assert_eq!(swings, 6);
    }

    #[test]
    fn a_use_that_consumed_nothing_does_not_swing() {
        assert_eq!(
            place_step(GameMode::Survival, true, false, 0, false, || false),
            (false, RIGHT_CLICK_DELAY)
        );
    }

    #[test]
    fn a_spectator_never_swings() {
        assert_eq!(
            place_step(GameMode::Spectator, true, false, 0, false, || true),
            (false, RIGHT_CLICK_DELAY)
        );
        assert_eq!(
            place_step(GameMode::Spectator, false, true, 0, false, || true),
            (false, RIGHT_CLICK_DELAY)
        );
    }

    #[test]
    fn an_untouched_button_does_nothing() {
        assert_eq!(
            place_step(GameMode::Creative, false, false, 0, false, || panic!(
                "used without a button"
            )),
            (false, 0)
        );
    }

    #[test]
    fn a_use_already_in_progress_is_not_restarted() {
        assert_eq!(
            place_step(GameMode::Survival, false, true, 0, true, || panic!(
                "restarted a use that was already running"
            )),
            (false, 0)
        );
    }

    #[test]
    fn a_click_during_a_use_is_swallowed() {
        assert_eq!(
            place_step(GameMode::Survival, true, true, 0, true, || panic!(
                "a click restarted a use that was already running"
            )),
            (false, 0)
        );
    }

    #[test]
    fn no_target_does_nothing() {
        assert_eq!(break_step(false, true, 0, false, None, None), (false, 0));
        assert_eq!(break_step(true, true, 0, false, None, None), (false, 0));
    }

    #[test]
    fn the_use_item_state_machine_starts_ticks_down_and_stops() {
        const APPLE: ItemKind = ItemKind::Apple;
        stop_using_item();

        start_using_item(APPLE, 32, InteractionHand::Off, None);
        assert_eq!(using_hand(), Some(InteractionHand::Off));
        assert_eq!(USE_ITEM.lock().unwrap().remaining, 32);

        start_using_item(ItemKind::Bread, 32, InteractionHand::Main, None);
        assert_eq!(USE_ITEM.lock().unwrap().item, Some(APPLE));
        assert_eq!(using_hand(), Some(InteractionHand::Off));
        assert_eq!(USE_ITEM.lock().unwrap().remaining, 32);

        stop_using_item();
        assert!(!is_using_item());
        assert_eq!(USE_ITEM.lock().unwrap().item, None);
        assert_eq!(USE_ITEM.lock().unwrap().remaining, 0);
    }

    #[test]
    fn the_countdown_runs_past_zero_and_leaves_the_use_running() {
        stop_using_item();
        start_using_item(ItemKind::Apple, 2, InteractionHand::Main, None);

        for expected in [1, 0, -1, -2] {
            USE_ITEM.lock().unwrap().remaining -= 1;
            assert_eq!(USE_ITEM.lock().unwrap().remaining, expected);
            assert!(
                is_using_item(),
                "the use ended on its own; only the server, a release or an \
                 item change may end one"
            );
        }

        stop_using_item();
        assert!(!is_using_item());
    }
}
