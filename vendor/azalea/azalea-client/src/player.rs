use azalea_auth::game_profile::GameProfile;
use azalea_chat::FormattedText;
use azalea_core::game_type::GameMode;
use azalea_entity::indexing::EntityUuidIndex;
use bevy_ecs::{
    component::Component,
    message::MessageReader,
    system::{Commands, Res},
};
use derive_more::{Deref, DerefMut};
use uuid::Uuid;

use crate::packet::game::AddPlayerEvent;

#[derive(Clone, Debug)]
pub struct PlayerInfo {
    pub profile: GameProfile,
    pub uuid: Uuid,
    pub gamemode: GameMode,
    pub latency: i32,
    pub display_name: Option<Box<FormattedText>>,
}

#[derive(Clone, Component, Debug, Deref, DerefMut)]
pub struct GameProfileComponent(pub GameProfile);

pub fn retroactively_add_game_profile_component(
    mut commands: Commands,
    mut events: MessageReader<AddPlayerEvent>,
    entity_uuid_index: Res<EntityUuidIndex>,
) {
    for event in events.read() {
        if let Some(entity) = entity_uuid_index.get(&event.info.uuid) {
            commands
                .entity(entity)
                .insert(GameProfileComponent(event.info.profile.clone()));
        }
    }
}
