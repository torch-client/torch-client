use std::sync::{
    Arc, Mutex,
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

use crate::session::{BreakingBlock, CommandBlockMode, ParticleSpawn, SharedMutex, TargetedBlock};

const DESTROY_DELAY: u32 = 5;
const RIGHT_CLICK_DELAY: u32 = 4;

static BREAK_DELAY: AtomicU32 = AtomicU32::new(0);
static PLACE_DELAY: AtomicU32 = AtomicU32::new(0);
static USE_ITEM: Mutex<UseItem> = Mutex::new(UseItem {
    started: false,
    item: None,
    remaining: 0,
});
static SERVER_USING_ITEM: AtomicBool = AtomicBool::new(false);

struct UseItem {
    started: bool,
    item: Option<ItemKind>,
    remaining: i32,
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

pub fn handle_interaction(bot: &Client, shared: &Arc<SharedMutex>) {
    let hit = block_hit(bot);

    let target = hit.as_ref().and_then(|h| {
        let state = bot.world().ok()?.read().get_block_state(h.block_pos)?;
        let boxes = state
            .outline_shape(h.block_pos)
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
            .collect();
        Some(TargetedBlock {
            pos: [h.block_pos.x, h.block_pos.y, h.block_pos.z],
            boxes,
        })
    });

    let (breaking, mining_particle) = mining_overlay(bot);
    let targeted_entity = living_target(bot);
    let crosshair_entity = picked_entity(bot);

    #[cfg_attr(not(feature = "mobile_ui"), allow(unused_variables))]
    let (attack_held, attack_clicked, use_held, use_clicked, pick, attack_ready) = {
        let mut s = shared.lock().unwrap();
        s.session.targeted_block = target;
        s.session.targeted_entity = targeted_entity;
        s.session.crosshair_entity = crosshair_entity;
        let attack_ready = s.session.attack_strength >= 1.0;
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
            (false, false, false, false, None, attack_ready)
        } else {
            (
                s.session.attack_held,
                std::mem::take(&mut s.session.attack_clicked),
                s.session.use_held,
                std::mem::take(&mut s.session.use_clicked),
                std::mem::take(&mut s.session.pick_clicked).then_some(s.session.pick_include_data),
                attack_ready,
            )
        }
    };

    if let Some(include_data) = pick {
        apply_pick_block(bot, include_data);
    }
    #[cfg(feature = "mobile_ui")]
    let attack_entity = attack_clicked || (attack_held && attack_ready);
    #[cfg(not(feature = "mobile_ui"))]
    let attack_entity = attack_clicked;

    crate::modules::esp::tick(bot, shared);

    crate::modules::aim_assist::tick(bot, shared);

    crate::modules::auto_sell::tick(bot, shared);

    let attack_held = crate::modules::auto_mine::tick(bot, shared, hit.as_ref()) || attack_held;

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

    let attacked_entity = attack_entity
        && match bot.hit_result() {
            Ok(HitResult::Entity(e)) => {
                bot.attack(e.entity);
                crate::client::tick::swing_local_player(false, shared);
                true
            }
            _ => false,
        };

    if attack_clicked && !attacked_entity && hit.is_none() {
        crate::client::tick::request_attack_ticker_reset();
    }

    if !attacked_entity {
        apply_break(bot, shared, hit.as_ref(), attack_held, attack_clicked);
    }
    apply_place(bot, shared, use_held, use_clicked);
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
    let boxes = state
        .outline_shape(pos)
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
        .collect();
    let breaking = BreakingBlock {
        pos: [pos.x, pos.y, pos.z],
        stage: stage.min(9) as u8,
        boxes,
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
    shared: &Arc<SharedMutex>,
    hit: Option<&BlockHitResult>,
    held: bool,
    clicked: bool,
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
    let editing_pos = shared.lock().unwrap().session.sign_edit_open_pos;
    let (held, clicked) = match (hit, editing_pos) {
        (Some(h), Some(editing)) if h.block_pos == editing => (false, false),
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

    let (start, delay) = break_step(
        creative,
        clicked,
        BREAK_DELAY.load(Ordering::Relaxed),
        was_mining,
        mining_at,
        hit.map(|h| h.block_pos),
    );
    BREAK_DELAY.store(delay, Ordering::Relaxed);
    if let Some(pos) = mining_at.or_else(|| hit.map(|h| h.block_pos).filter(|_| start)) {
        note_local_break(pos);
    }
    if start {
        let pos = hit
            .expect("break_step only starts when there is a target")
            .block_pos;
        bot.start_mining(pos);
    }

    if start || mining_at.is_some() {
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
    if delay > 0 {
        return (false, delay - 1);
    }
    if already_using {
        return (false, delay);
    }
    if !held && !clicked {
        return (false, delay);
    }

    let used = use_item();
    let swing = used && game_mode != GameMode::Spectator;
    (swing, RIGHT_CLICK_DELAY)
}

fn apply_place(bot: &Client, shared: &Arc<SharedMutex>, held: bool, clicked: bool) {
    let game_mode = bot
        .component::<GameMode>()
        .map(|gm| *gm)
        .unwrap_or(GameMode::Survival);
    open_book_editor(bot, shared, clicked);
    let secondary_use = {
        let s = shared.lock().unwrap();
        s.move_flags & crate::session::MOVE_SNEAK != 0
    };
    sync_server_using_item(bot);
    updating_using_item(bot);

    let was_using = is_using_item();
    if was_using && !held {
        release_using_item(bot);
    }
    let (swing, delay) = place_step(
        game_mode,
        clicked,
        held,
        PLACE_DELAY.load(Ordering::Relaxed),
        was_using,
        || {
            if predict_place(bot, secondary_use) {
                consume_held_item(bot);
                item_used(shared);
                true
            } else if predict_bucket_use(bot) {
                item_used(shared);
                true
            } else {
                start_use_item(bot, shared)
            }
        },
    );
    PLACE_DELAY.store(delay, Ordering::Relaxed);
    if swing {
        swing_main_hand(bot, shared);
    }
}

fn start_use_item(bot: &Client, shared: &Arc<SharedMutex>) -> bool {
    let hit = match bot.hit_result() {
        Ok(HitResult::Block(hit)) if !hit.miss => hit,
        Ok(HitResult::Entity(_)) => {
            bot.start_use_item();
            return false;
        }
        _ => return use_item(bot, shared),
    };

    let consumed = used_block_interaction(bot, shared, &hit);
    if consumed {
        open_command_block(bot, shared, &hit);
    }
    send_use_item_on(bot, &hit, consumed);
    if consumed {
        return true;
    }

    use_item(bot, shared)
}

fn use_item(bot: &Client, shared: &Arc<SharedMutex>) -> bool {
    let Some(kind) = held_item_kind(bot) else {
        return false;
    };
    send_use_item(bot);

    let duration = held_use_duration(bot);
    if duration == 0 || !can_consume(bot) {
        return false;
    }
    start_using_item(kind, duration);
    item_used(shared);
    false
}

fn can_consume(bot: &Client) -> bool {
    use azalea_inventory::components::Food;

    let Ok(inventory) = bot.component::<azalea::entity::inventory::Inventory>() else {
        return false;
    };
    let Some(food) = inventory.held_item().get_component::<Food>() else {
        return true;
    };
    let can_always_eat = food.can_always_eat;
    drop(inventory);

    if can_always_eat {
        return true;
    }
    if bot
        .component::<azalea::entity::PlayerAbilities>()
        .is_ok_and(|abilities| abilities.invulnerable)
    {
        return true;
    }
    bot.hunger().is_ok_and(|hunger| hunger.food < 20)
}

pub(crate) fn is_using_item() -> bool {
    USE_ITEM.lock().unwrap().started
}

fn start_using_item(kind: ItemKind, duration: i32) {
    let mut use_item = USE_ITEM.lock().unwrap();
    if use_item.started {
        return;
    }
    use_item.started = true;
    use_item.item = Some(kind);
    use_item.remaining = duration;
}

fn stop_using_item() {
    let mut use_item = USE_ITEM.lock().unwrap();
    use_item.started = false;
    use_item.item = None;
    use_item.remaining = 0;
}

fn release_using_item(bot: &Client) {
    bot.write_packet(ServerboundPlayerAction {
        action: Action::ReleaseUseItem,
        pos: BlockPos::new(0, 0, 0),
        direction: Direction::Down,
        seq: 0,
    });
    stop_using_item();
}

fn updating_using_item(bot: &Client) {
    let (started, item) = {
        let use_item = USE_ITEM.lock().unwrap();
        (use_item.started, use_item.item)
    };
    if !started {
        return;
    }
    if held_item_kind(bot) != item {
        stop_using_item();
        return;
    }
    USE_ITEM.lock().unwrap().remaining -= 1;
}

fn sync_server_using_item(bot: &Client) {
    let server_using = bot
        .component::<azalea::entity::metadata::AbstractLivingUsingItem>()
        .map(|using| using.0)
        .unwrap_or(false);
    if SERVER_USING_ITEM.swap(server_using, Ordering::Relaxed) == server_using {
        return;
    }
    match (server_using, is_using_item()) {
        (true, false) => {
            if let Some(kind) = held_item_kind(bot) {
                start_using_item(kind, held_use_duration(bot));
            }
        }
        (false, true) => stop_using_item(),
        _ => {}
    }
}

fn held_item_kind(bot: &Client) -> Option<ItemKind> {
    let inventory = bot
        .component::<azalea::entity::inventory::Inventory>()
        .ok()?;
    match inventory.held_item() {
        ItemStack::Present(data) => Some(data.kind),
        ItemStack::Empty => None,
    }
}

fn held_use_duration(bot: &Client) -> i32 {
    use azalea_inventory::components::{BlocksAttacks, Consumable, KineticWeapon};

    const HELD_INDEFINITELY: i32 = 72000;

    let Ok(inventory) = bot.component::<azalea::entity::inventory::Inventory>() else {
        return 0;
    };
    let held = inventory.held_item();
    if let Some(consumable) = held.get_component::<Consumable>() {
        return (consumable.consume_seconds * 20.0) as i32;
    }
    if held.get_component::<BlocksAttacks>().is_some()
        || held.get_component::<KineticWeapon>().is_some()
    {
        return HELD_INDEFINITELY;
    }
    0
}

fn send_use_item_on(bot: &Client, hit: &BlockHitResult, interacted: bool) {
    use azalea_protocol::packets::game::{
        s_interact::InteractionHand, s_use_item_on::ServerboundUseItemOn,
    };

    let Some(seq) = next_sequence(bot) else {
        return;
    };
    if interacted {
        predict_open_toggle(bot, hit);
    }
    bot.write_packet(ServerboundUseItemOn {
        hand: InteractionHand::MainHand,
        block_hit: hit.into(),
        seq,
    });
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

fn swing_main_hand(bot: &Client, shared: &Arc<SharedMutex>) {
    use azalea_protocol::packets::game::{s_interact::InteractionHand, s_swing::ServerboundSwing};

    crate::client::tick::swing_local_player(false, shared);
    bot.write_packet(ServerboundSwing {
        hand: InteractionHand::MainHand,
    });
}

fn item_used(shared: &Arc<SharedMutex>) {
    let mut s = shared.lock().unwrap();
    s.session.item_used[0] = s.session.item_used[0].wrapping_add(1);
}

fn consume_held_item(bot: &Client) {
    if !matches!(
        bot.component::<GameMode>().map(|gm| *gm),
        Ok(GameMode::Survival)
    ) {
        return;
    }
    let Ok(selected) = bot.selected_hotbar_slot() else {
        return;
    };
    let slot_index = 36 + selected as usize;
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

fn fill_held_bucket(bot: &Client, is_lava: bool) {
    if !matches!(
        bot.component::<GameMode>().map(|gm| *gm),
        Ok(GameMode::Survival) | Ok(GameMode::Creative)
    ) {
        return;
    }
    let Ok(selected) = bot.selected_hotbar_slot() else {
        return;
    };
    let slot_index = 36 + selected as usize;
    let mut ecs = bot.ecs.write();
    if let Some(mut inv) = ecs.get_mut::<azalea::entity::inventory::Inventory>(bot.entity)
        && let Some(slot) = inv.inventory_menu.slot_mut(slot_index)
        && let azalea_inventory::ItemStack::Present(data) = slot
        && data.kind == ItemKind::Bucket
        && data.count == 1
    {
        data.kind = if is_lava {
            ItemKind::LavaBucket
        } else {
            ItemKind::WaterBucket
        };
    }
}

fn used_block_interaction(bot: &Client, shared: &Arc<SharedMutex>, hit: &BlockHitResult) -> bool {
    let secondary_use = {
        let s = shared.lock().unwrap();
        s.move_flags & crate::session::MOVE_SNEAK != 0
    };
    let has_item = bot
        .component::<azalea::entity::inventory::Inventory>()
        .map(|inv| !inv.held_item().is_empty())
        .unwrap_or(false);
    if secondary_use && has_item {
        return false;
    }
    let Ok(world) = bot.world() else { return false };
    let state = world.read().get_block_state(hit.block_pos);
    state.is_some_and(crate::play::placement::has_use_interaction)
}

fn open_command_block(bot: &Client, shared: &Arc<SharedMutex>, hit: &BlockHitResult) {
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
    let Some(mode) = command_block_mode(state) else {
        return;
    };
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

pub(crate) fn command_block_mode(state: azalea::blocks::BlockState) -> Option<CommandBlockMode> {
    match Box::<dyn azalea::block::BlockTrait>::from(state).id() {
        "command_block" => Some(CommandBlockMode::Redstone),
        "repeating_command_block" => Some(CommandBlockMode::Auto),
        "chain_command_block" => Some(CommandBlockMode::Sequence),
        _ => None,
    }
}

fn predict_place(bot: &Client, secondary_use: bool) -> bool {
    use azalea_protocol::packets::game::{
        s_interact::InteractionHand, s_use_item_on::ServerboundUseItemOn,
    };

    let Some(predicted) = predict_use(bot, UseKind::Place, secondary_use) else {
        return false;
    };

    bot.write_packet(ServerboundUseItemOn {
        hand: InteractionHand::MainHand,
        block_hit: (&predicted.hit).into(),
        seq: predicted.seq,
    });
    true
}

fn predict_bucket_use(bot: &Client) -> bool {
    use azalea_protocol::packets::game::{ServerboundUseItem, s_interact::InteractionHand};

    let Some(predicted) = predict_use(bot, UseKind::Bucket, false) else {
        return false;
    };

    bot.write_packet(ServerboundUseItem {
        hand: InteractionHand::MainHand,
        seq: predicted.seq,
        x_rot: predicted.x_rot,
        y_rot: predicted.y_rot,
    });
    true
}

fn open_book_editor(bot: &Client, shared: &Arc<SharedMutex>, clicked: bool) {
    if !clicked {
        return;
    }
    let Ok(inventory) = bot.component::<azalea::entity::inventory::Inventory>() else {
        return;
    };
    let ItemStack::Present(data) = inventory.held_item() else {
        return;
    };
    if data.kind != ItemKind::WritableBook {
        return;
    }
    shared.lock().unwrap().session.book_edit_open = Some(crate::session::BookEditRequest {
        hand: crate::session::InteractionHand::Main,
        author: bot.username(),
    });
}

fn next_sequence(bot: &Client) -> Option<u32> {
    use azalea::interact::BlockStatePredictionHandler;

    let mut ecs = bot.ecs.write();
    let mut handler = ecs.get_mut::<BlockStatePredictionHandler>(bot.entity)?;
    Some(handler.start_predicting())
}

fn send_use_item(bot: &Client) {
    use azalea_protocol::packets::game::{ServerboundUseItem, s_interact::InteractionHand};

    let Ok(look) = bot.component::<azalea::entity::LookDirection>() else {
        return;
    };
    let (x_rot, y_rot) = (look.x_rot(), look.y_rot());
    drop(look);

    let Some(seq) = next_sequence(bot) else {
        return;
    };

    bot.write_packet(ServerboundUseItem {
        hand: InteractionHand::MainHand,
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
    Empty(bool),
    Fill(bool),
}

struct Predicted {
    hit: BlockHitResult,
    x_rot: f32,
    y_rot: f32,
    seq: u32,
}

fn predict_use(bot: &Client, kind: UseKind, secondary_use: bool) -> Option<Predicted> {
    use azalea::interact::BlockStatePredictionHandler;

    match bot.component::<GameMode>().map(|gm| *gm) {
        Ok(GameMode::Survival) | Ok(GameMode::Creative) => {}
        _ => return None,
    }

    if bot.is_mining() {
        return None;
    }

    let hit = block_hit(bot)?;
    let (Ok(world), Ok(inventory), Ok(look), Ok(player_pos)) = (
        bot.world(),
        bot.component::<azalea::entity::inventory::Inventory>(),
        bot.component::<azalea::entity::LookDirection>(),
        bot.position(),
    ) else {
        return None;
    };
    let (x_rot, y_rot) = (look.x_rot(), look.y_rot());

    let predicted = {
        let world = world.read();
        let held = inventory.held_item();
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
            .map(|(pos, state)| (pos, state, None)),
            UseKind::Bucket => match held {
                ItemStack::Present(item) if item.kind == ItemKind::WaterBucket => {
                    crate::play::placement::predict_bucket(held, &hit, min_y, height, |pos| {
                        world.get_block_state(pos)
                    })
                    .map(|(pos, state)| (pos, state, Some(BucketAction::Empty(false))))
                }
                ItemStack::Present(item) if item.kind == ItemKind::LavaBucket => {
                    crate::play::placement::predict_bucket(held, &hit, min_y, height, |pos| {
                        world.get_block_state(pos)
                    })
                    .map(|(pos, state)| (pos, state, Some(BucketAction::Empty(true))))
                }
                _ => crate::play::placement::predict_bucket_fill(held, &hit, |pos| {
                    world.get_block_state(pos)
                })
                .map(|(pos, is_lava)| (pos, BlockState::AIR, Some(BucketAction::Fill(is_lava)))),
            },
        };

        placed.and_then(|(pos, state, bucket_action)| {
            world
                .get_block_state(pos)
                .map(|old| (pos, state, old, bucket_action))
        })
    };
    drop(inventory);
    drop(look);
    let (pos, state, old_state, bucket_action) = predicted?;

    let seq = {
        let mut ecs = bot.ecs.write();
        let mut handler = ecs.get_mut::<BlockStatePredictionHandler>(bot.entity)?;
        let seq = handler.start_predicting();
        handler.retain_known_server_state(pos, old_state, player_pos);
        seq
    };

    world.read().set_block_state(pos, state);

    match bucket_action {
        Some(BucketAction::Empty(_is_lava)) => {
            #[cfg(feature = "audio")]
            {
                let at = [pos.x as f32 + 0.5, pos.y as f32 + 0.5, pos.z as f32 + 0.5];
                let event = if _is_lava {
                    "item.bucket.empty_lava"
                } else {
                    "item.bucket.empty"
                };
                crate::audio::play_at(event, crate::audio::SoundCategory::Blocks, at, 1.0, 1.0);
            }
        }
        Some(BucketAction::Fill(is_lava)) => {
            fill_held_bucket(bot, is_lava);
            #[cfg(feature = "audio")]
            {
                let at = [pos.x as f32 + 0.5, pos.y as f32 + 0.5, pos.z as f32 + 0.5];
                let event = if is_lava {
                    "item.bucket.fill_lava"
                } else {
                    "item.bucket.fill"
                };
                crate::audio::play_at(event, crate::audio::SoundCategory::Blocks, at, 1.0, 1.0);
            }
        }
        None => {}
    }

    Some(Predicted {
        hit,
        x_rot,
        y_rot,
        seq,
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
    fn a_refused_click_neither_uses_nor_swings() {
        let (swing, delay) = place_step(GameMode::Creative, true, false, 0, false, || true);
        assert!(swing);
        assert_eq!(delay, RIGHT_CLICK_DELAY);

        let mut delay = delay;
        for _ in 0..RIGHT_CLICK_DELAY {
            let (swing, next) = place_step(GameMode::Creative, true, true, delay, false, || {
                panic!("used during the delay")
            });
            assert!(!swing, "the arm swung on a click the delay refused");
            delay = next;
        }
        assert_eq!(delay, 0);
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
        assert_eq!(uses, 5);
        assert_eq!(swings, 5);
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

        start_using_item(APPLE, 32);
        assert!(is_using_item());
        assert_eq!(USE_ITEM.lock().unwrap().remaining, 32);

        start_using_item(ItemKind::Bread, 32);
        assert_eq!(USE_ITEM.lock().unwrap().item, Some(APPLE));
        assert_eq!(USE_ITEM.lock().unwrap().remaining, 32);

        stop_using_item();
        assert!(!is_using_item());
        assert_eq!(USE_ITEM.lock().unwrap().item, None);
        assert_eq!(USE_ITEM.lock().unwrap().remaining, 0);
    }

    #[test]
    fn the_countdown_runs_past_zero_and_leaves_the_use_running() {
        stop_using_item();
        start_using_item(ItemKind::Apple, 2);

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
