use azalea_client::client_chat::{ChatKind, SendChatEvent, handler::SendChatKindEvent};

use crate::Client;

impl Client {
    pub fn write_chat_packet(&self, message: &str) {
        self.ecs.write().write_message(SendChatKindEvent {
            entity: self.entity,
            content: message.to_owned(),
            kind: ChatKind::Message,
        });
    }

    pub fn write_command_packet(&self, command: &str) {
        self.ecs.write().write_message(SendChatKindEvent {
            entity: self.entity,
            content: command.to_owned(),
            kind: ChatKind::Command,
        });
    }

    pub fn chat(&self, content: impl Into<String>) {
        self.ecs.write().write_message(SendChatEvent {
            entity: self.entity,
            content: content.into(),
        });
    }
}
