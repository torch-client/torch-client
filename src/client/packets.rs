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
use crate::client::tick::{held_item, item_id};
use crate::client::tracking::{
    GAME_TIME, LOCAL_VEHICLE_ID, add_entity_data, anim_states, apply_map_patch, entity_anims,
    equipment, head_yaws, mark_meta_dirty, max_healths, meta_index_18, on_grounds, on_living_anim,
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

pub(crate) fn handle_packet(
    world: &WorldHolder,
    shared: &Arc<SharedMutex>,
    packet: &ClientboundGamePacket,
) {
    if let ClientboundGamePacket::Login(p) = packet {
        let mut s = shared.lock().unwrap();
        s.session.enforces_secure_chat = p.enforces_secure_chat;
        s.session.hardcore = p.hardcore;
        s.session.show_death_screen = p.show_death_screen;
        s.session.health_epoch = s.session.health_epoch.wrapping_add(1);
        s.session.chat_ack_pending = 0;
        s.session.chat_ack_last = None;
        drop(s);
        unsecure_server_warning(shared, p.enforces_secure_chat);
    }
    if let ClientboundGamePacket::Respawn(_) = packet {
        let mut s = shared.lock().unwrap();
        s.session.health_epoch = s.session.health_epoch.wrapping_add(1);
        s.session.death_cause = None;
    }
    if let ClientboundGamePacket::PlayerPosition(p) = packet
        && !p.relative.x
        && !p.relative.z
    {
        viewwindow::note_player_positioned();
        viewwindow::keep_view_center_on_the_player(&world.partial, p.change.pos);
    }

    trace(packet);
    weather(packet);
    chunk_accounting(world, packet);
    server_lighting(world, packet);
    block_entities(shared, packet);
    animations(packet);
    entity_state(packet);
    maps(shared, packet);
    session_state(world, shared, packet);
    level_lifecycle(world, shared, packet);
    crate::client::tablist::packet(shared, packet);
    crate::client::bossbar::packet(shared, packet);
    crate::client::cooldowns::packet(packet);
    crate::client::advancements::packet(shared, packet);
    crate::client::recipes::packet(shared, packet);
}

static UNSECURE_SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn unsecure_server_warning(shared: &Arc<SharedMutex>, enforces_secure_chat: bool) {
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
    let Ok(boxed): Result<Box<[u8; LAYER_SIZE]>, _> = bytes.to_vec().into_boxed_slice().try_into()
    else {
        note_bad_light_layer(bytes.len());
        return None;
    };
    Some(DataLayer::from_bytes(boxed))
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
    shared: &Arc<SharedMutex>,
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

fn enchantment_id(world: &WorldHolder, clue: i16) -> Option<Box<str>> {
    use azalea_core::data_registry::DataRegistryWithKey;
    use azalea_registry::DataRegistry;

    if clue < 0 {
        return None;
    }
    let world = world.shared.read();
    let enchantment = azalea_registry::Enchantment::new_raw(clue as u32);
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
            cost_a.count = charged.clamp(1, 255) as u8;
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
    use crate::play::inventory_bridge::slot_stack;

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
            if !world
                .partial
                .read()
                .chunks
                .in_range(&ChunkPos::new(p.x, p.z))
            {
                return;
            }
            if !crate::client::mesh_worker::offer_chunk_packet(p) {
                worldsync::stash_server_light(p.x, p.z, server_light(&p.light_data));
            }
        }
        ClientboundGamePacket::LightUpdate(p) => {
            if !crate::client::mesh_worker::offer_light_update(p.x, p.z, &p.light_data) {
                worldsync::send_server_light(p.x, p.z, server_light(&p.light_data));
            }
        }
        _ => {}
    }
}

fn block_entities(shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::LevelChunkWithLight(p) => {
            blockentities::feed::on_chunk(p.x, p.z, &p.chunk_data.block_entities);
        }
        ClientboundGamePacket::BlockEntityData(p) => {
            blockentities::feed::on_block_entity_data(p);
            command_block_data(shared, p);
        }
        ClientboundGamePacket::BlockEvent(p) => {
            blockentities::feed::on_block_event(p);
        }
        _ => {}
    }
}

fn command_block_data(shared: &Arc<SharedMutex>, packet: &ClientboundBlockEntityData) {
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
            match p.action {
                AnimationAction::CriticalHit => {
                    crate::client::tracking::crit_hits()
                        .lock()
                        .unwrap()
                        .push((p.id.0, false));
                }
                AnimationAction::MagicCriticalHit => {
                    crate::client::tracking::crit_hits()
                        .lock()
                        .unwrap()
                        .push((p.id.0, true));
                }
                _ => {}
            }
            let left = match p.action {
                AnimationAction::SwingMainHand => Some(false),
                AnimationAction::SwingOffHand => Some(true),
                _ => None,
            };
            if let Some(left) = left {
                anim_states()
                    .lock()
                    .unwrap()
                    .entry(p.id.0)
                    .or_default()
                    .swing(left);
                if let Some(anim) = entity_anims().lock().unwrap().get_mut(&p.id.0) {
                    anim.swing(left);
                }
            }
        }
        ClientboundGamePacket::HurtAnimation(p) => {
            on_living_anim(p.id.0, HumanoidAnim::hurt, EntityAnim::hurt);
        }
        ClientboundGamePacket::DamageEvent(p) => {
            on_living_anim(p.entity_id.0, HumanoidAnim::damage, EntityAnim::damage);
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
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn maps(shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
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

fn attribute_total(
    value: &azalea_protocol::packets::game::c_update_attributes::AttributeSnapshot,
) -> f64 {
    use azalea_core::attribute_modifier_operation::AttributeModifierOperation as Op;

    let mut total = value.base;
    for m in &value.modifiers {
        if m.operation == Op::AddValue {
            total += m.amount;
        }
    }
    for m in &value.modifiers {
        if m.operation == Op::AddMultipliedBase {
            total += m.amount * value.base;
        }
    }
    for m in &value.modifiers {
        if m.operation == Op::AddMultipliedTotal {
            total *= 1.0 + m.amount;
        }
    }
    total
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
        ClientboundGamePacket::UpdateAttributes(p) => {
            use azalea_registry::builtin::Attribute as Attr;
            for value in &p.values {
                match value.attribute {
                    Attr::MaxHealth => {
                        max_healths()
                            .lock()
                            .unwrap()
                            .insert(p.entity_id.0, attribute_total(value) as f32);
                    }
                    Attr::MovementSpeed | Attr::JumpStrength | Attr::StepHeight => {
                        let total = attribute_total(value);
                        let mut map = crate::client::tracking::mount_attributes().lock().unwrap();
                        let entry = map.entry(p.entity_id.0).or_default();
                        match value.attribute {
                            Attr::MovementSpeed => entry.movement_speed = Some(total),
                            Attr::JumpStrength => entry.jump_strength = Some(total),
                            _ => entry.step_height = Some(total),
                        }
                    }
                    _ => {}
                }
            }
        }
        ClientboundGamePacket::MoveVehicle(p) => {
            crate::play::riding::note_vehicle_correction(p.pos);
        }
        ClientboundGamePacket::AddEntity(p) => {
            mark_meta_dirty(p.id.0);
            if p.entity_type == azalea_registry::builtin::EntityKind::FallingBlock {
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

fn session_state(world: &WorldHolder, shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::EntityEvent(p)
            if (entity_event::PERMISSION_LEVEL_ALL..=entity_event::PERMISSION_LEVEL_OWNERS)
                .contains(&p.event_id)
                && p.entity_id.0 == crate::client::tracking::local_entity_id() =>
        {
            shared.lock().unwrap().session.op_level =
                p.event_id - entity_event::PERMISSION_LEVEL_ALL;
        }
        ClientboundGamePacket::OpenSignEditor(p) => {
            use crate::blockentities::feed::BlockEntityData;
            let mut s = shared.lock().unwrap();
            let key = [p.pos.x, p.pos.y, p.pos.z];
            let lines = s
                .session
                .block_entities
                .get(&key)
                .and_then(|info| match &*info.data {
                    BlockEntityData::Sign(sign) => Some(if p.is_front_text {
                        &sign.front
                    } else {
                        &sign.back
                    }),
                    #[cfg(feature = "skins")]
                    BlockEntityData::Skull(_) => None,
                    BlockEntityData::Banner(_) | BlockEntityData::None => None,
                })
                .map(|face| {
                    std::array::from_fn(|i| face.lines[i].iter().map(|s| s.text.as_str()).collect())
                })
                .unwrap_or_default();
            s.session.sign_editor_open = Some(crate::session::SignEditRequest {
                pos: p.pos,
                front: p.is_front_text,
                lines,
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
            use azalea_inventory::components::EquipmentSlot;
            mark_meta_dirty(p.entity_id.0);
            let mut map = equipment().lock().unwrap();
            let entry = map.entry(p.entity_id.0).or_default();
            for (slot, stack) in &p.slots.slots {
                match slot {
                    EquipmentSlot::Mainhand => entry.main_hand = held_item(stack),
                    EquipmentSlot::Offhand => entry.off_hand = held_item(stack),
                    EquipmentSlot::Head => entry.helmet = item_id(stack),
                    EquipmentSlot::Chest => entry.chestplate = item_id(stack),
                    EquipmentSlot::Legs => entry.leggings = item_id(stack),
                    EquipmentSlot::Feet => entry.boots = item_id(stack),
                    EquipmentSlot::Body => entry.body_armor = item_id(stack),
                    EquipmentSlot::Saddle => entry.saddle = item_id(stack),
                }
            }
        }
        ClientboundGamePacket::SetTime(p) => {
            GAME_TIME.store(p.game_time as i64, std::sync::atomic::Ordering::Relaxed);
            if let Some(clock) = overworld_clock(world, p) {
                shared.lock().unwrap().session.day_clock.resync(clock);
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

fn level_particles(
    shared: &Arc<SharedMutex>,
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

fn waypoint(shared: &Arc<SharedMutex>, p: &ClientboundWaypoint) {
    use azalea_protocol::packets::game::c_waypoint::{
        WaypointData, WaypointIdentifier, WaypointOperation,
    };

    let key = match &p.waypoint.identifier {
        WaypointIdentifier::Uuid(u) => WaypointKey::Uuid(u.as_u128()),
        WaypointIdentifier::String(s) => WaypointKey::Name(s.clone()),
    };

    let mut s = shared.lock().unwrap();
    if matches!(p.operation, WaypointOperation::Untrack) {
        Arc::make_mut(&mut s.session.waypoints).retain(|w| w.id != key);
        return;
    }

    let Some(pos) = (match &p.waypoint.data {
        WaypointData::Empty => None,
        WaypointData::Vec3i(v) => Some(WaypointPos::Block([v.x, v.y, v.z])),
        WaypointData::Chunk { x, z } => Some(WaypointPos::Chunk { x: *x, z: *z }),
        WaypointData::Azimuth { angle } => Some(WaypointPos::Azimuth(*angle)),
    }) else {
        Arc::make_mut(&mut s.session.waypoints).retain(|w| w.id != key);
        return;
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
    let waypoints = Arc::make_mut(&mut s.session.waypoints);
    if let Some(existing) = waypoints.iter_mut().find(|w| w.id == key) {
        *existing = info;
    } else {
        waypoints.push(info);
    }
}

fn level_lifecycle(world: &WorldHolder, shared: &Arc<SharedMutex>, packet: &ClientboundGamePacket) {
    match packet {
        ClientboundGamePacket::ForgetLevelChunk(p) => {
            worldsync::forget_chunk(world, shared, p.pos.x, p.pos.z);
        }
        ClientboundGamePacket::Respawn(p) => {
            worldsync::set_dimension(&p.common);
            worldsync::reset_level(shared);
        }
        ClientboundGamePacket::Login(p) => {
            worldsync::set_dimension(&p.common);
            worldsync::reset_level(shared);
        }
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
