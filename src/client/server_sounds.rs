use azalea::ecs::prelude::{MessageReader, Query};
use azalea::entity::Position;
use azalea::entity::indexing::EntityIdIndex;
use azalea::packet::game::ReceiveGamePacketEvent;
use azalea_core::sound::CustomSound;
use azalea_protocol::packets::game::ClientboundGamePacket;
use azalea_protocol::packets::game::c_sound::SoundSource;
use azalea_registry::Holder;
use azalea_registry::builtin::SoundEvent;
use azalea_registry::identifier::Identifier;

use crate::audio::{self, SoundCategory};

pub(crate) fn play_server_sounds(
    mut events: MessageReader<ReceiveGamePacketEvent>,
    indexes: Query<&EntityIdIndex>,
    positions: Query<&Position>,
) {
    for event in events.read() {
        match event.packet.as_ref() {
            ClientboundGamePacket::Sound(p) => {
                let Some(name) = event_name(&p.sound) else {
                    continue;
                };
                let at = [p.x, p.y, p.z].map(|v| f64::from(v) / 8.0);
                audio::play_seeded(
                    name,
                    category(p.source),
                    at,
                    p.volume,
                    p.pitch,
                    p.seed as i64,
                );
            }
            ClientboundGamePacket::SoundEntity(p) => {
                let Some(name) = event_name(&p.sound) else {
                    continue;
                };
                let entity = if p.id.0 == crate::client::tracking::local_entity_id() {
                    Some(event.entity)
                } else {
                    indexes
                        .get(event.entity)
                        .ok()
                        .and_then(|i| i.get_by_minecraft_entity(p.id))
                };
                let Some(pos) = entity.and_then(|e| positions.get(e).ok()) else {
                    continue;
                };
                let at = [pos.x, pos.y, pos.z];
                audio::play_seeded(
                    name,
                    category(p.source),
                    at,
                    p.volume,
                    p.pitch,
                    p.seed as i64,
                );
            }
            ClientboundGamePacket::StopSound(p) => {
                let name = p.name.as_ref().map(vanilla_path);
                if name == Some(None) {
                    continue;
                }
                audio::stop(p.source.map(category), name.flatten());
            }
            _ => {}
        }
    }
}

fn event_name(sound: &Holder<SoundEvent, CustomSound>) -> Option<&str> {
    match sound {
        Holder::Reference(event) => Some(crate::items::kinetic::sound_name(*event)),
        Holder::Direct(custom) => vanilla_path(&custom.sound_id),
    }
}

fn vanilla_path(id: &Identifier) -> Option<&str> {
    (id.namespace() == "minecraft").then(|| id.path())
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
    }
}
