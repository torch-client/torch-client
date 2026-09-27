use azalea_buf::AzBuf;
use azalea_chat::FormattedText;
use azalea_core::registry_holder::RegistryHolder;
use azalea_protocol_macros::ClientboundGamePacket;

use super::c_player_chat::ChatTypeBound;
use crate::packets::game::c_player_chat::GUESSED_DEFAULT_REGISTRIES_FOR_CHAT;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundDisguisedChat {
    pub message: FormattedText,
    pub chat_type: ChatTypeBound,
}

impl ClientboundDisguisedChat {
    #[must_use]
    pub fn message(&self) -> FormattedText {
        self.message_using_registries(&GUESSED_DEFAULT_REGISTRIES_FOR_CHAT)
    }

    #[must_use]
    pub fn message_using_registries(&self, registries: &RegistryHolder) -> FormattedText {
        self.chat_type.decorate(self.message.clone(), registries)
    }
}
