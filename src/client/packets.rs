use azalea::local_player::WorldHolder;
use azalea_core::bitset::BitSet;
use azalea_core::position::ChunkPos;
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_block_entity_data::ClientboundBlockEntityData;
use azalea_protocol::packets::game::c_game_event::EventType;
use azalea_protocol::packets::game::c_light_update::ClientboundLightUpdatePacketData;
use azalea_protocol::packets::game::c_waypoint::ClientboundWaypoint;
use std::sync::Arc;

use crate::blockentities;
use crate::client::tick::{held_item_patterned, item_id};
use crate::client::tracking::{
    GAME_TIME, LOCAL_ANIM_ID, LOCAL_VEHICLE_ID, add_entity_data, anim_states, apply_map_patch,
    entity_anims, equipment, head_yaws, mark_meta_dirty, meta_index_18, on_grounds, on_living_anim,
    overworld_clock, passengers, record_velocity,
};
use crate::client::{viewwindow, worldsync};
use crate::entities::feed::EntityAnim;
use crate::lighting::ServerLight;
use crate::lighting::data_layer::{DataLayer, SIZE as LAYER_SIZE};
use crate::play::commands;
use crate::play::entity_feed::{METADATA_INDEX_18, entity_event};
use crate::renderer::HumanoidAnim;
use crate::renderer::environment;
use crate::session::{
    MapDecoration, MapUpdate, SharedMutex, WaypointInfo, WaypointKey, WaypointPos,
};

thread_local! {
    static LAST_BLOCK_UPDATE: std::cell::Cell<
        Option<(azalea::BlockPos, azalea::block::BlockState)>,
    > = const { std::cell::Cell::new(None) };
}

pub(crate) fn handle_packet(
    world: &WorldHolder,
    shared: &SharedMutex,
    packet: &ClientboundGamePacket,
) {
    #[cfg(lua_mods)]
    crate::modules::script::packet(packet);
    #[cfg(lua_mods)]
    if let ClientboundGamePacket::TagQuery(p) = packet {
        crate::modules::script::tag_query(p);
    }
    #[cfg(eagler_skins)]
    if let ClientboundGamePacket::CustomPayload(p) = packet
        && p.identifier.to_string() == crate::eagler::skins::V4_CHANNEL
    {
        crate::eagler::skins::receive_plugin(&p.data);
    }
    if let ClientboundGamePacket::Login(p) = packet {
        crate::client::tracking::LOCAL_ENTITY_ID
            .store(p.player_id.0, std::sync::atomic::Ordering::Relaxed);
        let mut s = shared.lock().unwrap();
        s.session.hardcore = p.hardcore;
        s.session.show_death_screen = p.show_death_screen;
        crate::log_info!(
            "gamerule",
            "reduced debug info {}",
            if p.reduced_debug_info { "on" } else { "off" }
        );
        s.session.health_epoch = s.session.health_epoch.wrapping_add(1);
        s.session.last_set_health = None;
        s.session.chat_ack_pending = 0;
        s.session.chat_ack_last = None;
        s.session.remote_breaking.clear();
        s.session.item_pickups.clear();
        s.session.item_activation = None;
        s.session
            .last_death
            .clone_from(&p.common.last_death_location);
        drop(s);
        crate::play::tracking_emitter::reset();
        unsecure_server_warning(shared, p.enforces_secure_chat);
        crate::client::tick::request_attack_ticker_reset();
    }
    if let ClientboundGamePacket::Respawn(p) = packet {
        let mut s = shared.lock().unwrap();
        s.session
            .last_death
            .clone_from(&p.common.last_death_location);
        s.session.health_epoch = s.session.health_epoch.wrapping_add(1);
        s.session.death_cause = None;
        s.session.remote_breaking.clear();
        s.session.item_pickups.clear();
        s.session.item_activation = None;
        s.session.last_set_health = None;
        drop(s);
        crate::play::tracking_emitter::reset();
        if let Some(anim) = anim_states().lock().unwrap().get_mut(&LOCAL_ANIM_ID) {
            anim.reset_hurt();
        }
        crate::client::tick::request_attack_ticker_reset();
    }
    if let ClientboundGamePacket::SetHealth(p) = packet {
        let last = shared
            .lock()
            .unwrap()
            .session
            .last_set_health
            .replace(p.health);
        if last.is_some_and(|health| p.health < health)
            && let Some(anim) = anim_states().lock().unwrap().get_mut(&LOCAL_ANIM_ID)
        {
            anim.hurt();
        }
    }
    if let ClientboundGamePacket::SetDefaultSpawnPosition(p) = packet {
        shared.lock().unwrap().session.spawn_pos = Some(p.global_pos.clone());
    }
    #[cfg(feature = "audio")]
    if matches!(packet, ClientboundGamePacket::PlayerPosition(_)) {
        crate::play::player_sounds::forget_step_origin();
    }
    if let ClientboundGamePacket::PlayerPosition(p) = packet
        && !p.relative.x
        && !p.relative.z
    {
        viewwindow::note_player_positioned();
        viewwindow::keep_view_center_on_the_player(&world.partial, p.change.pos);
    }

    trace(packet);
    if let ClientboundGamePacket::BlockUpdate(p) = packet {
        LAST_BLOCK_UPDATE.set(Some((p.pos, p.block_state)));
    }
    #[cfg(target_arch = "wasm32")]
    match packet {
        ClientboundGamePacket::BlockUpdate(_) => {
            crate::client::mesh_worker::tally::count("server block updates", 1)
        }
        ClientboundGamePacket::SectionBlocksUpdate(p) => crate::client::mesh_worker::tally::count(
            "server section updates",
            p.states.len() as u64,
        ),
        _ => {}
    }
    weather(packet);
    chunk_accounting(world, packet);
    server_lighting(world, packet);
    block_entities(world, shared, packet);
    animations(packet);
    entity_state(packet);
    maps(shared, packet);
    session_state(world, shared, packet);
    level_lifecycle(world, shared, packet);
    crate::client::tablist::packet(shared, packet);
    crate::client::bossbar::packet(shared, packet);
    crate::client::sky_flash::packet(shared, packet);
    crate::client::hotbar_pop::packet(shared, packet);
    crate::client::cooldowns::packet(packet);
    crate::client::advancements::packet(world, shared, packet);
    crate::client::recipes::packet(world, shared, packet);
    crate::client::stats::packet(shared, packet);
    crate::client::server_screens::packet(shared, packet);
}

static UNSECURE_SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn unsecure_server_warning(shared: &SharedMutex, enforces_secure_chat: bool) {
    use std::sync::atomic::Ordering;

    if enforces_secure_chat || UNSECURE_SEEN.swap(true, Ordering::Relaxed) {
        return;
    }
    crate::session::queue_toasts(
        shared,
        [crate::gui::toast::ToastEvent::System {
            id: crate::gui::toast::SystemId::UnsecureServerWarning,
            title: crate::text::styled(
                &crate::gui::tooltip::translate("multiplayer.unsecureserver.toast.title", &[]),
                crate::text::Style::default(),
            ),
            message: crate::text::styled(
                &crate::gui::tooltip::translate("multiplayer.unsecureserver.toast", &[]),
                crate::text::Style::default(),
            ),
            multiline: true,
        }],
    );
}

fn demo_event(shared: &SharedMutex, param: f32) {
    use crate::gui::keybinds::{Action, KeyBinds};
    use crate::gui::tooltip::translate;

    let binds = KeyBinds::default();
    let key = |action| binds.bound(action).display_name().to_string();
    let movement = || [Action::Forward, Action::Left, Action::Back, Action::Right].map(key);
    let lines = if param == 0.0 {
        vec![
            translate("demo.help.title", &[]),
            translate("demo.help.movementShort", &movement()),
            translate("demo.help.movementMouse", &[]),
            translate("demo.help.jump", &[key(Action::Jump)]),
            translate("demo.help.inventory", &[key(Action::Inventory)]),
            translate("demo.help.fullWrapped", &[]),
        ]
    } else if param == 101.0 {
        vec![translate("demo.help.movement", &movement())]
    } else if param == 102.0 {
        vec![translate("demo.help.jump", &[key(Action::Jump)])]
    } else if param == 103.0 {
        vec![translate("demo.help.inventory", &[key(Action::Inventory)])]
    } else if param == 104.0 {
        vec![translate("demo.day.6", &[key(Action::Screenshot)])]
    } else {
        return;
    };
    let mut s = shared.lock().unwrap();
    for line in lines {
        s.session.chat_incoming.push(crate::session::ChatEntry {
            spans: vec![crate::text::Span::colored(line, 0xFF_FFFF)],
            events: None,
            tag: crate::session::ChatTag::System,
            original: None,
        });
    }
}

pub(crate) fn reset_unsecure_warning() {
    UNSECURE_SEEN.store(false, std::sync::atomic::Ordering::Relaxed);
}

fn trace(packet: &ClientboundGamePacket) {
    use azalea_protocol::packets::ProtocolPacket;
    crate::diag::note_packet(packet.name());
    if !crate::diag::tracing_packets() {
        return;
    }
    match packet {
        ClientboundGamePacket::TickingState(p) => {
            crate::diag::note_packet_detail(&format!(
                "tick rate {}, frozen {} (a frozen server never runs its chunk sender)",
                p.tick_rate, p.is_frozen
            ));
        }
        ClientboundGamePacket::SetChunkCacheRadius(p) => {
            crate::diag::note_packet_detail(&format!(
                "server will stream {} chunks around the player",
                p.radius
            ));
        }
        ClientboundGamePacket::SetSimulationDistance(p) => {
            crate::diag::note_packet_detail(&format!(
                "simulation distance {}",
                p.simulation_distance
            ));
        }
        ClientboundGamePacket::GameEvent(p) => {
            crate::diag::note_packet_detail(&format!("{:?}", p.event));
        }
        _ => {}
    }
}

fn weather(packet: &ClientboundGamePacket) {
    let ClientboundGamePacket::GameEvent(p) = packet else {
        return;
    };
    match p.event {
        EventType::StartRaining => environment::set_rain_level(0.0),
        EventType::StopRaining => environment::set_rain_level(1.0),
        EventType::RainLevelChange => environment::set_rain_level(p.param),
        EventType::ThunderLevelChange => environment::set_thunder_level(p.param),
        _ => {}
    }
}

pub(crate) fn server_light(data: &ClientboundLightUpdatePacketData) -> ServerLight {
    ServerLight {
        sky: read_section_list(
            &data.sky_y_mask,
            &data.empty_sky_y_mask,
            &data.sky_updates[..],
        ),
        block: read_section_list(
            &data.block_y_mask,
            &data.empty_block_y_mask,
            &data.block_updates[..],
        ),
    }
}

fn read_section_list(
    y_mask: &BitSet,
    empty_mask: &BitSet,
    updates: &[Box<[u8]>],
) -> Vec<Option<DataLayer>> {
    let count = y_mask.len().max(empty_mask.len());
    let mut next = updates.iter();
    let mut out = Vec::with_capacity(count);

    for i in 0..count {
        let have_data = y_mask.get(i).unwrap_or(false);
        let have_empty = empty_mask.get(i).unwrap_or(false);
        out.push(match (have_data, have_empty) {
            (true, _) => next.next().and_then(|bytes| layer_from_bytes(bytes)),
            (false, true) => Some(DataLayer::empty()),
            (false, false) => None,
        });
    }

    out
}

fn layer_from_bytes(bytes: &[u8]) -> Option<DataLayer> {
    let Ok(array): Result<&[u8; LAYER_SIZE], _> = bytes.try_into() else {
        note_bad_light_layer(bytes.len());
        return None;
    };
    Some(DataLayer::from_wire(array))
}

static BAD_LIGHT_LAYERS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

const BAD_LIGHT_REPORT_EVERY: u32 = 1024;

pub(crate) fn reset_bad_light_layers() {
    BAD_LIGHT_LAYERS.store(0, std::sync::atomic::Ordering::Relaxed);
}

fn note_bad_light_layer(len: usize) {
    let seen = BAD_LIGHT_LAYERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if seen == 0 {
        crate::log_warn!(
            "light",
            "server sent a {len}-byte light layer, expected {LAYER_SIZE}; that section will read \
             as unlit. Further malformed layers are counted rather than logged."
        );
    } else if (seen + 1) % BAD_LIGHT_REPORT_EVERY == 0 {
        crate::log_warn!(
            "light",
            "{} malformed light layers from this server so far; those sections read as unlit",
            seen + 1
        );
    }
}

fn horse_slots(
    world: &WorldHolder,
    shared: &SharedMutex,
    container_id: i32,
    items: &[azalea_inventory::ItemStack],
) -> Option<Vec<crate::session::SlotStack>> {
    use crate::play::inventory_bridge::slot_stack;

    let advanced_tooltips = {
        let s = shared.lock().unwrap();
        let horse = s.session.horse.as_ref()?;
        if horse.container_id != container_id {
            return None;
        }
        s.session.advanced_tooltips
    };

    let world = world.shared.read();
    let registries = Some(&world.registries);
    Some(
        items
            .iter()
            .map(|i| slot_stack(i, registries, advanced_tooltips))
            .collect(),
    )
}

fn block_item_state(stack: &azalea_inventory::ItemStack) -> Option<u32> {
    use azalea::block::{BlockState, BlockTrait};
    use std::str::FromStr;

    let data = stack.as_present()?;
    let kind = azalea_registry::builtin::BlockKind::from_str(data.kind.to_str()).ok()?;
    let mut block = Box::<dyn BlockTrait>::from(BlockState::from(kind));
    if let Some(props) = data.get_component::<azalea_inventory::components::BlockState>() {
        for (name, value) in &props.properties {
            let _ = block.set_property(name, value);
        }
    }
    crate::play::entity_feed::block_state_id(block.as_block_state())
}

fn enchantment_id(world: &WorldHolder, clue: i16) -> Option<Box<str>> {
    use azalea_core::data_registry::DataRegistryWithKey;
    use azalea_registry::DataRegistry;

    if clue < 0 {
        return None;
    }
    let world = world.shared.read();
    let enchantment = azalea_registry::data::Enchantment::new_raw(clue as u32);
    let key = enchantment.key_owned(&world.registries)?;
    Some(Box::from(
        azalea_registry::DataRegistryKey::into_ident(key).path(),
    ))
}

fn modified_cost_count(
    base: i32,
    demand: i32,
    price_multiplier: f32,
    special_price_diff: i32,
) -> i32 {
    let surcharge = (base as f32 * demand.max(0) as f32 * price_multiplier).floor() as i32;
    base.saturating_add(surcharge.max(0))
        .saturating_add(special_price_diff)
        .max(1)
}

fn merchant_offers(
    world: &WorldHolder,
    p: &azalea_protocol::packets::game::c_merchant_offers::ClientboundMerchantOffers,
) -> crate::session::MerchantOffers {
    use crate::play::inventory_bridge::slot_stack;

    let world = world.shared.read();
    let registries = Some(&world.registries);
    let stack = |s: &azalea_inventory::ItemStack| slot_stack(s, registries, false);

    let offers = p
        .offers
        .iter()
        .map(|o| {
            let base = o.base_cost_a.count;
            let charged =
                modified_cost_count(base, o.demand, o.price_multiplier, o.special_price_diff);

            let mut cost_a = stack(&azalea_inventory::ItemStack::Present(
                o.base_cost_a.clone().into_item_stack(),
            ));
            let max_stack = crate::generated_items::item(cost_a.item).map_or(64, |d| d.max_stack);
            cost_a.count = charged.clamp(1, max_stack.max(1) as i32) as u8;
            let mut base_cost_a = cost_a.clone();
            base_cost_a.count = base.clamp(1, 255) as u8;

            crate::session::MerchantOffer {
                cost_a,
                base_cost_a,
                cost_b: o
                    .cost_b
                    .clone()
                    .map(|c| stack(&azalea_inventory::ItemStack::Present(c.into_item_stack())))
                    .unwrap_or_default(),
                result: stack(&o.result),
                out_of_stock: o.out_of_stock,
                uses: o.uses.max(0) as u32,
                max_uses: o.max_uses.max(0) as u32,
                xp: o.xp.max(0) as u32,
            }
        })
        .collect();

    crate::session::MerchantOffers {
        container_id: p.container_id,
        offers,
        level: p.villager_level,
        xp: p.villager_xp,
        show_progress: p.show_progress,
        can_restock: p.can_restock,
    }
}

fn stonecutter_recipes(
    world: &WorldHolder,
    p: &azalea_protocol::packets::game::c_update_recipes::ClientboundUpdateRecipes,
) -> Vec<crate::session::StonecutterRecipe> {
    let world = world.shared.read();
    let registries = Some(&world.registries);

    p.stonecutter_recipes
        .iter()
        .filter_map(|entry| {
            let azalea_registry::HolderSet::Direct { contents } = &entry.input.allowed else {
                return None;
            };
            let mut input: Vec<Box<str>> =
                contents.iter().map(|k| item_kind_id(k).into()).collect();
            input.sort_unstable();
            let result = slot_display_stack(&entry.recipe.option_display, registries)?;
            Some(crate::session::StonecutterRecipe {
                input: input.into_boxed_slice(),
                result,
            })
        })
        .collect()
}

fn item_kind_id(kind: &azalea_registry::builtin::ItemKind) -> &'static str {
    kind.to_str().trim_start_matches("minecraft:")
}

fn slot_display_stack(
    display: &azalea_protocol::common::recipe::SlotDisplayData,
    registries: Option<&azalea_core::registry_holder::RegistryHolder>,
) -> Option<crate::session::SlotStack> {
    use crate::play::inventory_bridge::slot_stack;
    use azalea_protocol::common::recipe::SlotDisplayData as D;

    match display {
        D::Item(i) => {
            let item = item_kind_id(&i.item);
            (!item.is_empty()).then(|| crate::session::SlotStack {
                item,
                count: 1,
                ..Default::default()
            })
        }
        D::ItemStack(i) => Some(slot_stack(&i.stack, registries, false)),
        D::Composite(c) => c
            .contents
            .iter()
            .find_map(|d| slot_display_stack(d, registries)),
        _ => None,
    }
}

fn chunk_accounting(world: &WorldHolder, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::LevelChunkWithLight(p) => {
            viewwindow::note_chunk_packet(&world.partial, p.x, p.z);
        }
        ClientboundGamePacket::SetChunkCacheCenter(p) => {
            crate::diag::note_cache_center((p.x, p.z));
            viewwindow::note_server_centre();
        }
        _ => {}
    }
}

fn server_lighting(world: &WorldHolder, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::LevelChunkWithLight(p) => {
            if crate::client::mesh_worker::offer_chunk_packet(p) {
                return;
            }
            if !world
                .partial
                .read()
                .chunks
                .in_range(&ChunkPos::new(p.x, p.z))
            {
                return;
            }
            worldsync::stash_server_light(p.x, p.z, server_light(&p.light_data));
        }
        ClientboundGamePacket::LightUpdate(p) => {
            if !crate::client::mesh_worker::offer_light_update(p.x, p.z, &p.light_data) {
                worldsync::send_server_light(p.x, p.z, server_light(&p.light_data));
            }
        }
        _ => {}
    }
}

fn block_entities(world: &WorldHolder, shared: &SharedMutex, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::LevelChunkWithLight(p) => {
            blockentities::feed::on_chunk(p.x, p.z, &p.chunk_data.block_entities);
        }
        ClientboundGamePacket::BlockEntityData(p) => {
            blockentities::feed::on_block_entity_data(p);
            command_block_data(shared, p);
        }
        ClientboundGamePacket::BlockEvent(p) => {
            blockentities::feed::on_block_event(p, &world.shared.read());
        }
        ClientboundGamePacket::TestInstanceBlockStatus(p) => test_instance_status(shared, p),
        _ => {}
    }
}

fn test_instance_status(
    shared: &SharedMutex,
    packet: &azalea_protocol::packets::game::c_test_instance_block_status::ClientboundTestInstanceBlockStatus,
) {
    let Some(pos) = shared.lock().unwrap().session.test_instance_pos else {
        return;
    };
    let error = blockentities::feed::game_master_tag([pos.x, pos.y, pos.z])
        .and_then(|(_, tag)| tag)
        .and_then(|tag| blockentities::game_master::test_instance_error(&tag));
    let status = crate::session::TestInstanceStatus {
        status: crate::client::chat_text::to_spans(&packet.status),
        size: packet.size.map(|s| [s.x, s.y, s.z]),
        error,
    };
    let mut s = shared.lock().unwrap();
    if s.session.test_instance_pos == Some(pos) {
        s.session.test_instance_status.push(status);
    }
}

fn command_block_data(shared: &SharedMutex, packet: &ClientboundBlockEntityData) {
    let mut s = shared.lock().unwrap();
    if s.session.command_block_pos != Some(packet.pos) {
        return;
    }
    let track_output = packet.tag.byte("TrackOutput").unwrap_or(1) != 0;
    let last_output = if track_output {
        packet
            .tag
            .get("LastOutput")
            .and_then(crate::client::chat_text::from_nbt_tag)
            .map(|text| {
                crate::client::chat_text::to_spans(&text)
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    s.session.command_block_data = Some(crate::session::CommandBlockData {
        command: packet
            .tag
            .string("Command")
            .map(|c| c.to_string())
            .unwrap_or_default(),
        track_output,
        last_output,
        automatic: packet.tag.byte("auto").unwrap_or(0) != 0,
    });
}

fn animations(packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::RotateHead(p) => {
            head_yaws()
                .lock()
                .unwrap()
                .insert(p.entity_id.0, p.y_head_rot as f32 * 360.0 / 256.0);
        }
        ClientboundGamePacket::Animate(p) => {
            use azalea_protocol::packets::game::c_animate::AnimationAction;
            let crit = match p.action {
                AnimationAction::CriticalHit => Some(azalea::entity::particle::Particle::Crit),
                AnimationAction::MagicCriticalHit => {
                    Some(azalea::entity::particle::Particle::EnchantedHit)
                }
                _ => None,
            };
            if let Some(particle) = crit {
                crate::play::tracking_emitter::start(
                    p.id.0,
                    particle,
                    crate::play::tracking_emitter::DEFAULT_LIFE,
                    None,
                );
            }
            let left = match p.action {
                AnimationAction::SwingMainHand => Some(false),
                AnimationAction::SwingOffHand => Some(true),
                _ => None,
            };
            if let Some(off_hand) = left {
                anim_states()
                    .lock()
                    .unwrap()
                    .entry(p.id.0)
                    .or_default()
                    .swing_hand(off_hand);
                if let Some(anim) = entity_anims().lock().unwrap().get_mut(&p.id.0) {
                    anim.swing_hand(off_hand);
                }
            }
        }
        ClientboundGamePacket::ProjectilePower(p) => {
            if let Some(anim) = entity_anims().lock().unwrap().get_mut(&p.id.0) {
                anim.set_acceleration_power(p.acceleration_power);
            }
        }
        ClientboundGamePacket::HurtAnimation(p) => {
            on_living_anim(p.id.0, |anim| anim.hurt_from(p.yaw), EntityAnim::hurt);
        }
        ClientboundGamePacket::DamageEvent(p) => {
            on_living_anim(p.entity_id.0, HumanoidAnim::damage, EntityAnim::damage);
        }
        ClientboundGamePacket::EntityEvent(p) if p.event_id == entity_event::KINETIC_HIT => {
            crate::play::spear::kinetic_hit(p.entity_id.0);
        }
        ClientboundGamePacket::EntityEvent(p)
            if p.event_id == entity_event::USE_ITEM_COMPLETE
                && p.entity_id.0 == crate::client::tracking::local_entity_id() =>
        {
            crate::play::interaction::note_use_completed();
        }
        ClientboundGamePacket::EntityEvent(p) if p.event_id == entity_event::ARROW_POOF => {
            crate::play::arrow_particles::poof(p.entity_id.0);
        }
        ClientboundGamePacket::EntityEvent(p) => {
            if let Some(anim) = entity_anims().lock().unwrap().get_mut(&p.entity_id.0) {
                match p.event_id {
                    entity_event::JUMP => anim.jump(),
                    entity_event::START_ATTACKING => anim.attack_animation(),
                    entity_event::SQUID_ANIM_SYNCH => anim.squid_anim_sync(),
                    entity_event::START_RAM => anim.lower_head(true),
                    entity_event::END_RAM => anim.lower_head(false),
                    entity_event::SHAKE_WETNESS => anim.shake_wetness(),
                    entity_event::CANCEL_SHAKE_WETNESS => anim.cancel_shake(),
                    entity_event::EAT_GRASS => anim.eat_grass(),
                    entity_event::OFFER_FLOWER => anim.offer_flower(true),
                    entity_event::STOP_OFFER_FLOWER => anim.offer_flower(false),
                    entity_event::ARMORSTAND_WOBBLE => anim.wiggle(),
                    entity_event::RAVAGER_STUNNED => anim.stunned(),
                    entity_event::TENDRILS_SHIVER => anim.tendril_shiver(),
                    entity_event::SONIC_CHARGE => anim.sonic_charge(),
                    entity_event::ARMADILLO_PEEK => anim.armadillo_peek(),
                    entity_event::SHAKE => anim.creaking_shake(),
                    entity_event::TNT_PRIME => anim.prime_tnt_minecart(),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn maps(shared: &SharedMutex, packet: &ClientboundGamePacket) {
    let ClientboundGamePacket::MapItemData(p) = packet else {
        return;
    };
    let id = p.map_id as i32;
    let colors = match &p.color_patch.0 {
        Some(patch) => apply_map_patch(
            id,
            patch.start_x,
            patch.start_y,
            patch.width,
            patch.height,
            &patch.map_colors,
        ),
        None => apply_map_patch(id, 0, 0, 0, 0, &[]),
    };
    let decorations = p.decorations.as_ref().map(|list| {
        Arc::new(
            list.iter()
                .map(|d| MapDecoration {
                    kind: d.decoration_type as u8,
                    x: d.x,
                    y: d.y,
                    rot: d.rot as u8 & 15,
                    name: d
                        .name
                        .as_ref()
                        .map(|name| crate::client::chat_text::to_spans(name).into()),
                })
                .collect(),
        )
    });
    let mut s = shared.lock().unwrap();
    match s.session.map_updates.iter_mut().find(|u| u.id == id) {
        Some(pending) => {
            pending.colors = colors;
            if decorations.is_some() {
                pending.decorations = decorations;
            }
        }
        None => s.session.map_updates.push(MapUpdate {
            id,
            colors,
            decorations,
        }),
    }
}

fn entity_state(packet: &ClientboundGamePacket) {
    use std::sync::atomic::Ordering;

    match packet {
        ClientboundGamePacket::SetPassengers(p) => {
            let new: Vec<i32> = p.passengers.iter().map(|id| id.0).collect();
            for id in &new {
                mark_meta_dirty(*id);
            }
            mark_meta_dirty(p.vehicle.0);
            let local = crate::client::tracking::local_entity_id();
            if new.contains(&local) {
                LOCAL_VEHICLE_ID.store(p.vehicle.0, Ordering::Relaxed);
            } else if LOCAL_VEHICLE_ID.load(Ordering::Relaxed) == p.vehicle.0 {
                LOCAL_VEHICLE_ID.store(-1, Ordering::Relaxed);
            }
            let old = passengers().lock().unwrap().insert(p.vehicle.0, new);
            for id in old.into_iter().flatten() {
                mark_meta_dirty(id);
            }
        }
        ClientboundGamePacket::SetEntityLink(p) => {
            let mut map = crate::client::tracking::leashes().lock().unwrap();
            match p.dest_id.0 {
                0 => map.remove(&p.source_id.0),
                holder => map.insert(p.source_id.0, holder),
            };
        }
        ClientboundGamePacket::UpdateAttributes(p) => {
            crate::client::tracking::update_attributes(p);
        }
        ClientboundGamePacket::MoveVehicle(p) => {
            crate::play::riding::note_vehicle_correction(
                p.pos,
                p.look_direction.y_rot(),
                p.look_direction.x_rot(),
            );
        }
        ClientboundGamePacket::AddEntity(p) => {
            mark_meta_dirty(p.id.0);
            use azalea_registry::builtin::EntityKind;
            if matches!(
                p.entity_type,
                EntityKind::FallingBlock
                    | EntityKind::ItemFrame
                    | EntityKind::GlowItemFrame
                    | EntityKind::Painting
                    | EntityKind::FishingBobber
            ) {
                add_entity_data().lock().unwrap().insert(p.id.0, p.data);
            }
            record_velocity(p.id.0, p.movement.to_vec3());
        }
        ClientboundGamePacket::SetEntityMotion(p) => {
            record_velocity(p.id.0, p.delta.to_vec3());
        }
        ClientboundGamePacket::SetEntityData(p) => {
            use azalea::entity::EntityDataValue;
            mark_meta_dirty(p.id.0);
            if p.packed_items
                .0
                .iter()
                .any(|item| matches!(item.value, EntityDataValue::Direction(_)))
            {
                crate::log_debug!(
                    "itemframe",
                    "set_entity_data id={} items={:?}",
                    p.id.0,
                    p.packed_items.0
                );
            }
            for item in &p.packed_items.0 {
                if item.index == METADATA_INDEX_18
                    && let EntityDataValue::Byte(bits) = item.value
                {
                    meta_index_18().lock().unwrap().insert(p.id.0, bits);
                }
            }
        }
        ClientboundGamePacket::MoveEntityPos(p) => {
            on_grounds()
                .lock()
                .unwrap()
                .insert(p.entity_id.0, p.on_ground);
        }
        ClientboundGamePacket::MoveEntityPosRot(p) => {
            on_grounds()
                .lock()
                .unwrap()
                .insert(p.entity_id.0, p.on_ground);
        }
        ClientboundGamePacket::MoveEntityRot(p) => {
            on_grounds()
                .lock()
                .unwrap()
                .insert(p.entity_id.0, p.on_ground);
        }
        ClientboundGamePacket::TeleportEntity(p) => {
            on_grounds().lock().unwrap().insert(p.id.0, p.on_ground);
        }
        ClientboundGamePacket::EntityPositionSync(p) => {
            on_grounds().lock().unwrap().insert(p.id.0, p.on_ground);
        }
        _ => {}
    }
}

fn session_state(world: &WorldHolder, shared: &SharedMutex, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::TakeItemEntity(p) => crate::play::item_pickup::take(shared, p),
        ClientboundGamePacket::BlockDestruction(p) => {
            let remote = &mut shared.lock().unwrap().session.remote_breaking;
            if p.progress <= 9 {
                remote.insert(
                    p.id.0,
                    crate::session::RemoteBreak {
                        pos: [p.pos.x, p.pos.y, p.pos.z],
                        stage: p.progress,
                        age: 0,
                    },
                );
            } else {
                remote.remove(&p.id.0);
            }
        }
        ClientboundGamePacket::EntityEvent(p)
            if (entity_event::PERMISSION_LEVEL_ALL..=entity_event::PERMISSION_LEVEL_OWNERS)
                .contains(&p.event_id)
                && p.entity_id.0 == crate::client::tracking::local_entity_id() =>
        {
            shared.lock().unwrap().session.op_level =
                p.event_id - entity_event::PERMISSION_LEVEL_ALL;
        }
        ClientboundGamePacket::EntityEvent(p)
            if p.event_id == entity_event::PROTECTED_FROM_DEATH =>
        {
            crate::play::totem::protected_from_death(shared, p.entity_id.0);
        }
        ClientboundGamePacket::GameEvent(p) if p.event == EventType::ImmediateRespawn => {
            shared.lock().unwrap().session.show_death_screen = p.param == 0.0;
        }
        ClientboundGamePacket::TickingState(p) => {
            shared.lock().unwrap().session.tick_rate = crate::session::TickRate(p.tick_rate);
        }
        ClientboundGamePacket::GameEvent(p) if p.event == EventType::GuardianElderEffect => {
            let mut s = shared.lock().unwrap();
            s.session.elder_guardian_effects += 1;
            #[cfg(feature = "audio")]
            if (p.param + 0.5).floor() as i32 == 1 {
                let at = s.session.player_pos;
                drop(s);
                crate::audio::play_at(
                    "entity.elder_guardian.curse",
                    crate::audio::SoundCategory::Hostile,
                    at,
                    1.0,
                    1.0,
                );
            }
        }
        ClientboundGamePacket::GameEvent(p) if p.event == EventType::DemoEvent => {
            demo_event(shared, p.param);
        }
        ClientboundGamePacket::EntityEvent(p)
            if (p.event_id == entity_event::REDUCED_DEBUG_INFO
                || p.event_id == entity_event::FULL_DEBUG_INFO)
                && p.entity_id.0 == crate::client::tracking::local_entity_id() =>
        {
            crate::log_info!(
                "gamerule",
                "reduced debug info {}",
                if p.event_id == entity_event::REDUCED_DEBUG_INFO {
                    "on"
                } else {
                    "off"
                }
            );
        }
        ClientboundGamePacket::OpenSignEditor(p) => {
            use crate::blockentities::feed::{BlockEntityData, SignFace};
            use crate::session::SignEditKind;
            let world_block = world
                .shared
                .read()
                .get_block_state(p.pos)
                .map(|state| crate::blocks::block_name(state).to_string());
            let updated_block = LAST_BLOCK_UPDATE
                .get()
                .filter(|(pos, _)| *pos == p.pos)
                .map(|(_, state)| crate::blocks::block_name(state));
            let mut s = shared.lock().unwrap();
            let key = [p.pos.x, p.pos.y, p.pos.z];
            let entry = s.session.block_entities.get(&key);
            let Some((kind, wood)) = entry
                .and_then(|info| SignEditKind::from_block(&info.state.block))
                .or_else(|| world_block.as_deref().and_then(SignEditKind::from_block))
                .or_else(|| updated_block.and_then(SignEditKind::from_block))
            else {
                crate::log_warn!(
                    "sign",
                    "ignoring OpenSignEditor at {:?}: no sign there",
                    p.pos
                );
                return;
            };
            let face = entry
                .and_then(|info| match &*info.data {
                    BlockEntityData::Sign(sign) => Some(if p.is_front_text {
                        sign.front.clone()
                    } else {
                        sign.back.clone()
                    }),
                    _ => None,
                })
                .unwrap_or_else(SignFace::default);
            s.session.sign_editor_open = Some(crate::session::SignEditRequest {
                pos: p.pos,
                front: p.is_front_text,
                kind,
                wood,
                face,
            });
        }
        ClientboundGamePacket::OpenBook(p) => {
            shared.lock().unwrap().session.book_open = Some(hand(p.hand));
        }
        ClientboundGamePacket::ContainerSetData(p) => {
            let clue = (p.id as usize)
                .checked_sub(4)
                .filter(|row| *row < 3)
                .map(|row| (row, enchantment_id(world, p.value as i16)));

            let mut s = shared.lock().unwrap();
            if let Some(slot) = s.session.container_data.get_mut(p.id as usize) {
                *slot = p.value as i16;
            }
            if let Some((row, name)) = clue {
                s.session.enchant_clues[row] = name;
            }
        }
        ClientboundGamePacket::MountScreenOpen(p) => {
            let mut s = shared.lock().unwrap();
            s.session.horse = Some(crate::session::HorseMenu {
                container_id: p.container_id,
                columns: p.inventory_columns.min(5) as u8,
                state_id: 0,
                slots: Vec::new(),
            });
            s.session.horse_published_state_id = u32::MAX;
        }
        ClientboundGamePacket::ContainerSetContent(p) => {
            let Some(slots) = horse_slots(world, shared, p.container_id, &p.items) else {
                return;
            };
            let mut s = shared.lock().unwrap();
            if let Some(horse) = s.session.horse.as_mut() {
                horse.slots = slots;
                horse.state_id = p.state_id;
            }
        }
        ClientboundGamePacket::ContainerSetSlot(p) => {
            let Some(mut slots) = horse_slots(
                world,
                shared,
                p.container_id,
                std::slice::from_ref(&p.item_stack),
            ) else {
                return;
            };
            let mut s = shared.lock().unwrap();
            if let Some(horse) = s.session.horse.as_mut()
                && let Some(dst) = horse.slots.get_mut(p.slot as usize)
            {
                *dst = slots.remove(0);
                horse.state_id = p.state_id;
            }
        }
        ClientboundGamePacket::ContainerClose(_) => {
            shared.lock().unwrap().session.horse = None;
        }
        ClientboundGamePacket::MerchantOffers(p) => {
            let offers = merchant_offers(world, p);
            shared.lock().unwrap().session.merchant = Some(Arc::new(offers));
        }
        ClientboundGamePacket::UpdateRecipes(p) => {
            let recipes = stonecutter_recipes(world, p);
            shared.lock().unwrap().session.stonecutter_recipes = recipes.into();
        }
        ClientboundGamePacket::Commands(p) => {
            let tree = commands::CommandTree::from_stubs(&p.entries, p.root_index);
            shared.lock().unwrap().session.command_tree = Some(Arc::new(tree));
        }
        ClientboundGamePacket::CommandSuggestions(p) => {
            let range = commands::Range::between(
                p.suggestions.range().start(),
                p.suggestions.range().end(),
            );
            let list = p
                .suggestions
                .list()
                .iter()
                .map(|s| commands::Suggestion {
                    range,
                    text: s.text(),
                    tooltip: s.tooltip.clone(),
                })
                .collect();
            shared.lock().unwrap().session.suggestion_reply =
                Some((p.id, commands::Suggestions { range, list }));
        }
        ClientboundGamePacket::CustomChatCompletions(p) => {
            use azalea_protocol::packets::game::c_custom_chat_completions::Action;
            let mut s = shared.lock().unwrap();
            match p.action {
                Action::Add => s
                    .session
                    .custom_completions
                    .extend(p.entries.iter().cloned()),
                Action::Remove => s
                    .session
                    .custom_completions
                    .retain(|e| !p.entries.contains(e)),
                Action::Set => s.session.custom_completions = p.entries.clone(),
            }
        }
        ClientboundGamePacket::SetEquipment(p) => {
            use azalea_inventory::components::{BannerPatterns, EquipmentSlot};
            mark_meta_dirty(p.entity_id.0);
            let patterned = p.slots.slots.iter().any(|(_, s)| {
                s.as_present().is_some_and(|d| {
                    d.get_component::<BannerPatterns>().is_some()
                        || d.get_component::<azalea_inventory::components::Trim>()
                            .is_some()
                        || d.get_component::<azalea_inventory::components::Enchantments>()
                            .is_some_and(|e| !e.levels.is_empty())
                })
            });
            let (mut main_hand, mut off_hand, mut head) = (None, None, None);
            let mut trims: [Option<&'static str>; 4] = [None; 4];
            let mut worn = [crate::util::combat::Worn::default(); 4];
            let mut stacks: [Option<crate::session::SlotStack>; 4] = Default::default();
            {
                let guard = patterned.then(|| world.shared.read());
                let registries = guard.as_deref().map(|w| &w.registries);
                let item =
                    |stack: &azalea_inventory::ItemStack| held_item_patterned(stack, registries);
                let trim = |stack: &azalea_inventory::ItemStack| {
                    item_id(stack).and_then(|id| {
                        crate::entities::render::humanoid::trim::resolve(stack, id, registries)
                    })
                };
                use azalea_inventory::components::EquipmentSlotGroup as Group;
                let wear = |stack: &azalea_inventory::ItemStack, group: Group| {
                    crate::util::combat::Worn::of(stack, group, registries)
                };
                let gui = |stack: &azalea_inventory::ItemStack| {
                    Some(crate::play::inventory_bridge::slot_stack(
                        stack, registries, false,
                    ))
                };
                for (slot, stack) in &p.slots.slots {
                    match slot {
                        EquipmentSlot::Mainhand => main_hand = Some(item(stack)),
                        EquipmentSlot::Offhand => off_hand = Some(item(stack)),
                        EquipmentSlot::Head => {
                            head = Some(item(stack));
                            trims[0] = trim(stack);
                            worn[0] = wear(stack, Group::Head);
                            stacks[0] = gui(stack);
                        }
                        EquipmentSlot::Chest => {
                            trims[1] = trim(stack);
                            worn[1] = wear(stack, Group::Chest);
                            stacks[1] = gui(stack);
                        }
                        EquipmentSlot::Legs => {
                            trims[2] = trim(stack);
                            worn[2] = wear(stack, Group::Legs);
                            stacks[2] = gui(stack);
                        }
                        EquipmentSlot::Feet => {
                            trims[3] = trim(stack);
                            worn[3] = wear(stack, Group::Feet);
                            stacks[3] = gui(stack);
                        }
                        _ => {}
                    }
                }
            }
            let mut map = equipment().lock().unwrap();
            let entry = map.entry(p.entity_id.0).or_default();
            let foil = |stack: &azalea_inventory::ItemStack| {
                stack
                    .as_present()
                    .is_some_and(crate::items::glint::has_foil)
            };
            let consume = |stack: &azalea_inventory::ItemStack| {
                stack
                    .as_present()
                    .and_then(crate::items::consume::Consumption::of)
            };
            for (slot, stack) in &p.slots.slots {
                match slot {
                    EquipmentSlot::Mainhand => {
                        entry.main_hand = main_hand.take().unwrap_or_default();
                        entry.consume[0] = consume(stack);
                    }
                    EquipmentSlot::Offhand => {
                        entry.off_hand = off_hand.take().unwrap_or_default();
                        entry.consume[1] = consume(stack);
                    }
                    EquipmentSlot::Head => {
                        entry.head = head.take().unwrap_or_default();
                        entry.armor_foil[0] = foil(stack);
                        entry.armor_trim[0] = trims[0];
                        entry.worn[0] = worn[0];
                    }
                    EquipmentSlot::Chest => {
                        entry.chestplate = item_id(stack);
                        entry.armor_foil[1] = foil(stack);
                        entry.armor_trim[1] = trims[1];
                        entry.worn[1] = worn[1];
                    }
                    EquipmentSlot::Legs => {
                        entry.leggings = item_id(stack);
                        entry.armor_foil[2] = foil(stack);
                        entry.armor_trim[2] = trims[2];
                        entry.worn[2] = worn[2];
                    }
                    EquipmentSlot::Feet => {
                        entry.boots = item_id(stack);
                        entry.armor_foil[3] = foil(stack);
                        entry.armor_trim[3] = trims[3];
                        entry.worn[3] = worn[3];
                    }
                    EquipmentSlot::Body => {
                        entry.body_armor = item_id(stack);
                        entry.body_block = block_item_state(stack);
                    }
                    EquipmentSlot::Saddle => {
                        entry.saddle = item_id(stack);
                        entry.saddle_block = block_item_state(stack);
                    }
                }
            }
            if stacks.iter().any(Option::is_some) {
                let armor = Arc::make_mut(&mut entry.armor);
                for (dst, src) in armor.iter_mut().zip(stacks) {
                    if let Some(src) = src {
                        *dst = src;
                    }
                }
            }
        }
        ClientboundGamePacket::ChangeDifficulty(p) => {
            crate::client::tracking::DIFFICULTY
                .store(p.difficulty as u8, std::sync::atomic::Ordering::Relaxed);
        }
        ClientboundGamePacket::SetTime(p) => {
            GAME_TIME.store(p.game_time as i64, std::sync::atomic::Ordering::Relaxed);
            let clock = overworld_clock(world, p);
            let end = crate::client::tracking::end_clock(world, p);
            let mut s = shared.lock().unwrap();
            match end {
                Some((ticks, partial, rate)) => {
                    s.session
                        .end_clock
                        .set(p.game_time as i64, ticks, partial, rate)
                }
                None => {
                    s.session.end_clock.tick(p.game_time as i64);
                }
            }
            s.session.game_time = p.game_time as u64;
            if let Some(clock) = clock {
                s.session.day_clock.resync(clock);
            }
        }
        ClientboundGamePacket::SetActionBarText(p) => {
            let spans = crate::client::chat_text::to_spans(&p.text);
            shared.lock().unwrap().session.set_action_bar(spans);
        }
        ClientboundGamePacket::SetTitleText(p) => {
            let spans = crate::client::chat_text::to_spans(&p.text);
            shared.lock().unwrap().session.set_title(spans);
        }
        ClientboundGamePacket::SetSubtitleText(p) => {
            let spans = crate::client::chat_text::to_spans(&p.text);
            shared.lock().unwrap().session.set_subtitle(spans);
        }
        ClientboundGamePacket::SetTitlesAnimation(p) => {
            shared.lock().unwrap().session.set_title_times(
                p.fade_in as i32,
                p.stay as i32,
                p.fade_out as i32,
            );
        }
        ClientboundGamePacket::ClearTitles(p) => {
            shared.lock().unwrap().session.clear_titles(p.reset_times);
        }
        ClientboundGamePacket::ShowDialog(p) => {
            let dialog = {
                let world = world.shared.read();
                crate::play::dialog::parse_holder(&p.dialog, &world.registries)
            };
            if let Some(dialog) = dialog {
                shared.lock().unwrap().session.dialog_show = Some(dialog);
            }
        }
        ClientboundGamePacket::ClearDialog(_) => {
            shared.lock().unwrap().session.dialog_clear = true;
        }
        ClientboundGamePacket::ServerLinks(p) => {
            shared.lock().unwrap().session.server_links = crate::client::dialogs::links(&p.links);
        }
        ClientboundGamePacket::UpdateTags(p) => {
            crate::client::dialogs::update_tags(&p.tags);
        }
        ClientboundGamePacket::Waypoint(p) => waypoint(shared, p),
        ClientboundGamePacket::LevelParticles(p) => level_particles(shared, p),
        ClientboundGamePacket::Explode(p) => {
            let mut s = shared.lock().unwrap();
            crate::session::push_particle_emit(
                &mut s.session.particle_emits,
                crate::session::ParticleEmit::Explosion(crate::session::ExplosionEmit {
                    center: [p.center.x, p.center.y, p.center.z],
                    radius: p.radius,
                    block_count: p.block_count,
                    particle: p.explosion_particle.clone(),
                    block_particles: p.block_particles[..p
                        .block_particles
                        .len()
                        .min(crate::session::MAX_EXPLOSION_BLOCK_PARTICLES)]
                        .to_vec(),
                }),
            );
        }
        ClientboundGamePacket::LevelEvent(p) if matches!(p.event_type, 1010 | 1011) => {
            jukebox_event(
                world,
                shared,
                p.event_type,
                [p.pos.x, p.pos.y, p.pos.z],
                p.data,
            );
        }
        #[cfg(feature = "audio")]
        ClientboundGamePacket::DamageEvent(p)
            if p.entity_id.0 == crate::client::tracking::local_entity_id() =>
        {
            local_hurt(world, shared, p.source_type_id);
        }
        #[cfg(feature = "audio")]
        ClientboundGamePacket::EntityEvent(p)
            if p.event_id == 3 && p.entity_id.0 == crate::client::tracking::local_entity_id() =>
        {
            let at = shared.lock().unwrap().session.player_pos;
            crate::play::player_sounds::death(at);
        }
        ClientboundGamePacket::LevelEvent(p) => {
            let mut s = shared.lock().unwrap();
            crate::session::push_particle_emit(
                &mut s.session.particle_emits,
                crate::session::ParticleEmit::Event(crate::session::LevelEventEmit {
                    id: p.event_type,
                    pos: [p.pos.x, p.pos.y, p.pos.z],
                    data: p.data,
                    global: p.global_event,
                }),
            );
        }
        _ => {}
    }
}

#[cfg(feature = "audio")]
fn local_hurt(world: &WorldHolder, shared: &SharedMutex, id: u32) {
    use azalea_core::data_registry::DataRegistryWithKey;
    use azalea_registry::DataRegistry;

    let path = {
        let world = world.shared.read();
        azalea_registry::data::DamageKind::new_raw(id)
            .key_owned(&world.registries)
            .map(|key| Box::<str>::from(azalea_registry::DataRegistryKey::into_ident(key).path()))
    };
    let at = shared.lock().unwrap().session.player_pos;
    crate::play::player_sounds::hurt(path.as_deref(), at);
}

#[cfg_attr(not(feature = "audio"), allow(unused_variables))]
fn jukebox_event(world: &WorldHolder, shared: &SharedMutex, id: u32, pos: [i32; 3], data: u32) {
    use azalea_core::data_registry::DataRegistryWithKey;
    use azalea_registry::DataRegistry;

    if id == 1011 {
        #[cfg(feature = "audio")]
        crate::audio::jukebox_stop(pos);
        return;
    }
    let path = {
        let world = world.shared.read();
        let song = azalea_registry::data::JukeboxSong::new_raw(data);
        song.key_owned(&world.registries)
            .map(|key| Box::<str>::from(azalea_registry::DataRegistryKey::into_ident(key).path()))
    };
    let Some(song) = path.and_then(|path| crate::util::datapack::entry("jukebox_song", &path))
    else {
        return;
    };
    #[cfg(feature = "audio")]
    if let Some(event) = song
        .get("sound_event")
        .and_then(crate::util::datapack::bare_id)
    {
        crate::audio::jukebox_start(pos, event);
    }
    let description = song
        .get("description")
        .and_then(|d| d.get("translate"))
        .and_then(|key| key.as_str());
    if let Some(key) = description {
        use crate::gui::tooltip::translate;
        let line = translate("record.nowPlaying", &[translate(key, &[])]);
        shared
            .lock()
            .unwrap()
            .session
            .set_now_playing(vec![crate::text::Span::new(line)]);
    }
}

fn level_particles(
    shared: &SharedMutex,
    p: &azalea_protocol::packets::game::c_level_particles::ClientboundLevelParticles,
) {
    let mut s = shared.lock().unwrap();
    crate::session::push_particle_emit(
        &mut s.session.particle_emits,
        crate::session::ParticleEmit::Level(crate::session::LevelParticle {
            particle: p.particle.clone(),
            pos: [p.pos.x, p.pos.y, p.pos.z],
            dist: [p.x_dist, p.y_dist, p.z_dist],
            max_speed: p.max_speed,
            count: p.count,
            override_limiter: p.override_limiter,
            always_show: p.always_show,
        }),
    );
}

pub(crate) fn waypoint_parts(p: &ClientboundWaypoint) -> (WaypointKey, Option<WaypointInfo>) {
    use azalea_protocol::packets::game::c_waypoint::{
        WaypointData, WaypointIdentifier, WaypointOperation,
    };

    let key = match &p.waypoint.identifier {
        WaypointIdentifier::Uuid(u) => WaypointKey::Uuid(u.as_u128()),
        WaypointIdentifier::String(s) => WaypointKey::Name(s.clone()),
    };
    if matches!(p.operation, WaypointOperation::Untrack) {
        return (key, None);
    }
    let pos = match &p.waypoint.data {
        WaypointData::Empty => return (key, None),
        WaypointData::Vec3i(v) => WaypointPos::Block([v.x, v.y, v.z]),
        WaypointData::Chunk { x, z } => WaypointPos::Chunk { x: *x, z: *z },
        WaypointData::Azimuth { angle } => WaypointPos::Azimuth(*angle),
    };
    let info = WaypointInfo {
        id: key.clone(),
        style: p.waypoint.icon.style.path().to_string(),
        color: p
            .waypoint
            .icon
            .color
            .map(|c| [c.red(), c.green(), c.blue()].map(|v| v as f32 / 255.0)),
        pos,
    };
    (key, Some(info))
}

fn waypoint(shared: &SharedMutex, p: &ClientboundWaypoint) {
    let (key, info) = waypoint_parts(p);
    let mut s = shared.lock().unwrap();
    let Some(info) = info else {
        Arc::make_mut(&mut s.session.waypoints).retain(|w| w.id != key);
        return;
    };
    let waypoints = Arc::make_mut(&mut s.session.waypoints);
    if let Some(existing) = waypoints.iter_mut().find(|w| w.id == key) {
        *existing = info;
    } else {
        waypoints.push(info);
    }
}

fn level_lifecycle(world: &WorldHolder, shared: &SharedMutex, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::ForgetLevelChunk(p) => {
            #[cfg(feature = "render_lod")]
            if crate::client::lod::cache::is_cached(p.pos.x, p.pos.z) {
                return;
            }
            worldsync::forget_chunk(world, shared, p.pos.x, p.pos.z);
        }
        ClientboundGamePacket::Respawn(p) => {
            #[cfg(feature = "audio")]
            {
                crate::audio::level_respawn(!crate::renderer::dimension::is_current_level(
                    &p.common.dimension,
                ));
                crate::play::player_sounds::reset(false);
            }
            worldsync::set_dimension(world, &p.common);
            worldsync::reset_level(world, shared);
        }
        ClientboundGamePacket::Login(p) => {
            #[cfg(feature = "audio")]
            crate::audio::level_login();
            crate::client::worker::reset_biome_table();
            #[cfg(feature = "audio")]
            crate::audio::publish_synced(&world.shared.read().registries);
            #[cfg(feature = "seed_cracker")]
            crate::modules::seed_cracker::logged_in();
            worldsync::set_dimension(world, &p.common);
            worldsync::reset_level(world, shared);
        }
        ClientboundGamePacket::StartConfiguration(_) => crate::client::worker::reset_biome_table(),
        _ => {}
    }
}

fn hand(
    h: azalea_protocol::packets::game::s_interact::InteractionHand,
) -> crate::session::InteractionHand {
    use azalea_protocol::packets::game::s_interact::InteractionHand as Az;
    match h {
        Az::MainHand => crate::session::InteractionHand::Main,
        Az::OffHand => crate::session::InteractionHand::Off,
    }
}

#[cfg(test)]
mod merchant_price_tests {
    use super::modified_cost_count;

    #[test]
    fn plain_offer_charges_the_base_price() {
        assert_eq!(modified_cost_count(12, 0, 0.05, 0), 12);
    }

    #[test]
    fn demand_adds_a_surcharge() {
        assert_eq!(modified_cost_count(20, 3, 0.05, 0), 23);
        assert_eq!(modified_cost_count(20, -8, 0.05, 0), 20);
    }

    #[test]
    fn special_price_discounts_down_to_one() {
        assert_eq!(modified_cost_count(10, 0, 0.0, -3), 7);
        assert_eq!(modified_cost_count(10, 0, 0.0, -400), 1);
    }

    #[test]
    fn hostile_values_saturate_instead_of_overflowing() {
        assert_eq!(modified_cost_count(i32::MAX, 0, 0.0, 1), i32::MAX);
        assert_eq!(modified_cost_count(i32::MAX, 1, 1.0, i32::MAX), i32::MAX);
        assert_eq!(modified_cost_count(i32::MAX, i32::MAX, 1.0, 0), i32::MAX);
        assert_eq!(modified_cost_count(i32::MIN, 0, 0.0, i32::MIN), 1);
    }

    #[test]
    fn non_finite_multipliers_are_bounded() {
        assert_eq!(modified_cost_count(2, 1, f32::INFINITY, 0), i32::MAX);
        assert_eq!(modified_cost_count(2, 1, f32::NAN, 0), 2);
        assert_eq!(modified_cost_count(2, 1, f32::NEG_INFINITY, 0), 2);
    }
}
