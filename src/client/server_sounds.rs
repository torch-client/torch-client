use std::borrow::Cow;

use azalea::ecs::entity::Entity;
use azalea::ecs::prelude::{MessageReader, Query};
use azalea::entity::dimensions::EntityDimensions;
use azalea::entity::indexing::EntityIdIndex;
use azalea::entity::inventory::Inventory;
use azalea::entity::metadata::Silent;
use azalea::entity::{EntityKindComponent, Position};
use azalea::local_player::WorldHolder;
use azalea::packet::game::ReceiveGamePacketEvent;
use azalea_core::data_registry::DataRegistryWithKey;
use azalea_core::entity_id::MinecraftEntityId;
use azalea_core::sound::CustomSound;
use azalea_inventory::components::{BreakSound, EquipmentSlot};
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_game_event::EventType;
use azalea_protocol::packets::game::c_sound::SoundSource;
use azalea_registry::Holder;
use azalea_registry::builtin::{EntityKind, SoundEvent};
use azalea_registry::data::WorldClockKey;
use azalea_registry::identifier::Identifier;

use crate::audio::{self, SoundCategory};
use crate::play::entity_feed::entity_event::{ARMORSTAND_WOBBLE, START_ATTACKING};
use crate::play::entity_sounds::{self, Sound};

const ZOMBIE_CONVERTING: u8 = 16;
const FIREWORKS_EXPLODE: u8 = 17;
const GUARDIAN_ATTACK_SOUND: u8 = 21;
const SNIFFER_DIGGING_SOUND: u8 = 63;
const ARMADILLO_PEEK: u8 = 64;

type Mob<'a> = (
    &'a Position,
    &'a EntityKindComponent,
    Option<&'a Silent>,
    Option<&'a EntityDimensions>,
);

pub(crate) fn play_server_sounds(
    mut events: MessageReader<ReceiveGamePacketEvent>,
    indexes: Query<&EntityIdIndex>,
    mobs: Query<Mob<'static>>,
    inventories: Query<&Inventory>,
    holders: Query<&WorldHolder>,
) {
    for event in events.read() {
        match event.packet.as_ref() {
            ClientboundGamePacket::Sound(p) => {
                let name = event_name(&p.sound);
                let at = [p.x, p.y, p.z].map(|v| f64::from(v) / 8.0);
                audio::play_seeded(
                    &name,
                    category(p.source),
                    at,
                    p.volume,
                    p.pitch,
                    p.seed as i64,
                );
            }
            ClientboundGamePacket::SoundEntity(p) => {
                let name = event_name(&p.sound);
                let Some(entity) = find_entity(event, p.id, &indexes) else {
                    continue;
                };
                let Ok((pos, _, silent, _)) = mobs.get(entity) else {
                    continue;
                };
                if silent.is_some_and(|s| s.0) {
                    continue;
                }
                let sound = Sound {
                    event: &name,
                    category: category(p.source),
                    volume: p.volume,
                    pitch: p.pitch,
                    seed: p.seed as i64,
                };
                entity_sounds::follow(entity, [pos.x, pos.y, pos.z], sound, false);
            }
            ClientboundGamePacket::StopSound(p) => {
                let name = p.name.as_ref().map(identifier_name);
                audio::stop(p.source.map(category), name.as_deref());
            }
            ClientboundGamePacket::Explode(p) => {
                let name = event_name(&p.explosion_sound);
                let at = [p.center.x, p.center.y, p.center.z];
                let pitch = (1.0 + (audio::random_f32() - audio::random_f32()) * 0.2) * 0.7;
                audio::play_at(&name, SoundCategory::Blocks, at, 4.0, pitch);
            }
            ClientboundGamePacket::AddEntity(p) if p.entity_type == EntityKind::LightningBolt => {
                let at = [p.position.x, p.position.y, p.position.z];
                let thunder = 0.8 + audio::random_f32() * 0.2;
                audio::play_at(
                    "entity.lightning_bolt.thunder",
                    SoundCategory::Weather,
                    at,
                    10000.0,
                    thunder,
                );
                let impact = 0.5 + audio::random_f32() * 0.2;
                audio::play_at(
                    "entity.lightning_bolt.impact",
                    SoundCategory::Weather,
                    at,
                    2.0,
                    impact,
                );
            }
            ClientboundGamePacket::GameEvent(p) => {
                let Ok((pos, _, _, dims)) = mobs.get(event.entity) else {
                    continue;
                };
                match p.event {
                    EventType::ArrowHitPlayer => {
                        let at = [pos.x, pos.y + eye_height(dims), pos.z];
                        audio::play_at(
                            "entity.arrow.hit_player",
                            SoundCategory::Players,
                            at,
                            0.18,
                            0.45,
                        );
                    }
                    EventType::PufferFishSting => {
                        let at = [pos.x, pos.y, pos.z];
                        audio::play_at(
                            "entity.puffer_fish.sting",
                            SoundCategory::Neutral,
                            at,
                            1.0,
                            1.0,
                        );
                    }
                    _ => {}
                }
            }
            ClientboundGamePacket::EntityEvent(p) if p.event_id == FIREWORKS_EXPLODE => {
                entity_sounds::firework_blast(p.entity_id.0);
            }
            ClientboundGamePacket::EntityEvent(p) => {
                let Some(entity) = find_entity(event, p.entity_id, &indexes) else {
                    continue;
                };
                let Ok(mob) = mobs.get(entity) else { continue };
                let local = (entity == event.entity)
                    .then(|| inventories.get(entity).ok())
                    .flatten();
                entity_event(p.event_id, p.entity_id.0, entity, mob, local);
            }
            ClientboundGamePacket::DamageEvent(p) => {
                let entity = find_entity(event, p.entity_id, &indexes);
                let mob = entity.and_then(|e| mobs.get(e).ok());
                if mob.is_some_and(|(_, kind, ..)| kind.0 == EntityKind::Illusioner) {
                    entity_sounds::hurt(p.entity_id.0);
                }
            }
            ClientboundGamePacket::SetTime(p) => {
                let Ok(holder) = holders.get(event.entity) else {
                    continue;
                };
                let world = holder.shared.read();
                let end = p
                    .clock_updates
                    .iter()
                    .find(|(id, _)| id.key_owned(&world.registries) == Some(WorldClockKey::TheEnd));
                if let Some((_, state)) = end {
                    let ticks = state.total_ticks as i64;
                    entity_sounds::set_end_clock(
                        p.game_time as i64,
                        ticks,
                        state.partial_tick,
                        state.rate,
                    );
                }
            }
            _ => {}
        }
    }
}

fn find_entity(
    event: &ReceiveGamePacketEvent,
    id: MinecraftEntityId,
    indexes: &Query<&EntityIdIndex>,
) -> Option<Entity> {
    if id.0 == crate::client::tracking::local_entity_id() {
        return Some(event.entity);
    }
    indexes
        .get(event.entity)
        .ok()
        .and_then(|i| i.get_by_minecraft_entity(id))
}

fn eye_height(dims: Option<&EntityDimensions>) -> f64 {
    dims.map_or(0.0, |d| f64::from(d.eye_height))
}

fn entity_event(
    event: u8,
    id: i32,
    entity: Entity,
    (pos, kind, silent, dims): Mob<'_>,
    local: Option<&Inventory>,
) {
    let at = [pos.x, pos.y, pos.z];
    let silent = silent.is_some_and(|s| s.0);
    let random = audio::random_f32;
    match (event, kind.0) {
        (START_ATTACKING, EntityKind::EvokerFangs) if !silent => {
            let pitch = random() * 0.2 + 0.85;
            audio::play_at(
                "entity.evoker_fangs.attack",
                SoundCategory::Neutral,
                at,
                1.0,
                pitch,
            );
        }
        (ZOMBIE_CONVERTING, EntityKind::ZombieVillager) if !silent => {
            let eye = [pos.x, pos.y + eye_height(dims), pos.z];
            let (volume, pitch) = (1.0 + random(), random() * 0.7 + 0.3);
            audio::play_at(
                "entity.zombie_villager.cure",
                SoundCategory::Hostile,
                eye,
                volume,
                pitch,
            );
        }
        (ARMORSTAND_WOBBLE, EntityKind::ArmorStand) => {
            audio::play_at(
                "entity.armor_stand.hit",
                SoundCategory::Neutral,
                at,
                0.3,
                1.0,
            );
        }
        (ARMADILLO_PEEK, EntityKind::Armadillo) => {
            audio::play_at(
                "entity.armadillo.peek",
                SoundCategory::Neutral,
                at,
                1.0,
                1.0,
            );
        }
        (GUARDIAN_ATTACK_SOUND, EntityKind::Guardian | EntityKind::ElderGuardian) if !silent => {
            entity_sounds::guardian_attack(id);
        }
        (SNIFFER_DIGGING_SOUND, EntityKind::Sniffer) if !silent => {
            let sound = Sound {
                event: "entity.sniffer.digging",
                category: SoundCategory::Neutral,
                volume: 1.0,
                pitch: 1.0,
                seed: audio::random_seed(),
            };
            entity_sounds::follow(entity, at, sound, true);
        }
        (47..=52 | 65 | 68, kind) if !silent => {
            let slot = match event {
                47 => EquipmentSlot::Mainhand,
                48 => EquipmentSlot::Offhand,
                49 => EquipmentSlot::Head,
                50 => EquipmentSlot::Chest,
                51 => EquipmentSlot::Legs,
                52 => EquipmentSlot::Feet,
                65 => EquipmentSlot::Body,
                _ => EquipmentSlot::Saddle,
            };
            let stack = local.and_then(|inventory| inventory.get_equipment(slot));
            let sound = stack.and_then(|stack| stack.get_component::<BreakSound>());
            let name = sound.as_ref().map(|c| event_name(&c.sound));
            let name = name.unwrap_or_else(|| remote_break_sound(id, slot).into());
            let source = crate::play::tracking_emitter::sound_source(kind);
            audio::play_at(&name, source, at, 0.8, 0.8 + random() * 0.4);
        }
        _ => {}
    }
}

fn remote_break_sound(id: i32, slot: EquipmentSlot) -> &'static str {
    let equipment = crate::client::tracking::equipment().lock().unwrap();
    let item = equipment.get(&id).and_then(|worn| match slot {
        EquipmentSlot::Mainhand => Some(worn.main_hand.id),
        EquipmentSlot::Offhand => Some(worn.off_hand.id),
        EquipmentSlot::Head => Some(worn.head.id),
        EquipmentSlot::Chest => worn.chestplate,
        EquipmentSlot::Legs => worn.leggings,
        EquipmentSlot::Feet => worn.boots,
        EquipmentSlot::Body => worn.body_armor,
        EquipmentSlot::Saddle => worn.saddle,
    });
    match item {
        Some("shield") => "item.shield.break",
        Some("wolf_armor") => "item.wolf_armor.break",
        _ => "entity.item.break",
    }
}

pub(crate) fn event_name(sound: &Holder<SoundEvent, CustomSound>) -> Cow<'_, str> {
    match sound {
        Holder::Reference(event) => crate::items::kinetic::sound_name(*event).into(),
        Holder::Direct(custom) => identifier_name(&custom.sound_id),
    }
}

fn identifier_name(id: &Identifier) -> Cow<'_, str> {
    if id.namespace() == "minecraft" {
        id.path().into()
    } else {
        format!("{}:{}", id.namespace(), id.path()).into()
    }
}

fn category(source: SoundSource) -> SoundCategory {
    SoundCategory::ALL[source as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_map_to_their_namesakes() {
        assert_eq!(category(SoundSource::Master), SoundCategory::Master);
        assert_eq!(category(SoundSource::Records), SoundCategory::Records);
        assert_eq!(category(SoundSource::Players), SoundCategory::Players);
        assert_eq!(category(SoundSource::Voice), SoundCategory::Voice);
        assert_eq!(category(SoundSource::Ui), SoundCategory::Ui);
    }
}
