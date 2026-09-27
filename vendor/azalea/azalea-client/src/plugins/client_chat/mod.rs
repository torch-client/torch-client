pub mod handler;

use std::sync::Arc;

use azalea_chat::FormattedText;
use azalea_protocol::packets::game::{
    c_disguised_chat::ClientboundDisguisedChat, c_player_chat::ClientboundPlayerChat,
    c_system_chat::ClientboundSystemChat,
};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use handler::{SendChatKindEvent, handle_send_chat_kind_event};
use uuid::Uuid;

pub struct ChatPlugin;
impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SendChatEvent>()
            .add_message::<SendChatKindEvent>()
            .add_message::<ChatReceivedEvent>()
            .add_systems(
                Update,
                (handle_send_chat_event, handle_send_chat_kind_event).chain(),
            );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChatPacket {
    System(Arc<ClientboundSystemChat>),
    Player(Arc<ClientboundPlayerChat>),
    Disguised(Arc<ClientboundDisguisedChat>),
}

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new($re).unwrap());
        &RE
    }};
}

impl ChatPacket {
    pub fn message(&self) -> FormattedText {
        match self {
            ChatPacket::System(p) => p.content.clone(),
            ChatPacket::Player(p) => p.message(),
            ChatPacket::Disguised(p) => p.message(),
        }
    }

    pub fn split_sender_and_content(&self) -> (Option<String>, String) {
        match self {
            ChatPacket::System(p) => {
                let message = p.content.to_string();
                if p.overlay {
                    return (None, message);
                }

                if let Some(m) = regex!(r"^<(?:\[[^\]]+?\] )?(\w{1,16})> (.+)$").captures(&message)
                {
                    return (Some(m[1].to_string()), m[2].to_string());
                }
                if let Some(m) =
                    regex!(r"^\[(?:\[[^\]]+?\] )?(\w{1,16})(?: -> me)?\] (.+)$").captures(&message)
                {
                    return (Some(m[1].to_string()), m[2].to_string());
                }
                if let Some(m) =
                    regex!(r"^(\w{1,16}) whispers(?: to you)?: (.+)$").captures(&message)
                {
                    return (Some(m[1].to_string()), m[2].to_string());
                }
                if let Some(m) =
                    regex!(r"^From (?:\[[^\]]+\] )(\w{1,16}): (.+)$").captures(&message)
                {
                    return (Some(m[1].to_string()), m[2].to_string());
                }

                (None, message)
            }
            ChatPacket::Player(p) => (
                Some(p.chat_type.name.to_string()),
                p.body.content.clone(),
            ),
            ChatPacket::Disguised(p) => (
                Some(p.chat_type.name.to_string()),
                p.message.to_string(),
            ),
        }
    }

    pub fn sender(&self) -> Option<String> {
        self.split_sender_and_content().0
    }

    pub fn sender_uuid(&self) -> Option<Uuid> {
        match self {
            ChatPacket::System(_) => None,
            ChatPacket::Player(m) => Some(m.sender),
            ChatPacket::Disguised(_) => None,
        }
    }

    pub fn content(&self) -> String {
        self.split_sender_and_content().1
    }

    pub fn new(message: &str) -> Self {
        ChatPacket::System(Arc::new(ClientboundSystemChat {
            content: FormattedText::from(message),
            overlay: false,
        }))
    }

    pub fn is_whisper(&self) -> bool {
        match self {
            ChatPacket::System(p) => {
                let message = p.content.to_string();
                if p.overlay {
                    return false;
                }
                if regex!(r"^(-> me|\w{1,16} whispers: )").is_match(&message) {
                    return true;
                }
                if regex!(r"^From (?:\[[^\]]+\] )?\w{1,16}: ").is_match(&message) {
                    return true;
                }

                false
            }
            _ => match self.message() {
                FormattedText::Text(_) => false,
                FormattedText::Translatable(t) => t.key == "commands.message.display.incoming",
            },
        }
    }
}

#[derive(Clone, Debug, Message)]
pub struct ChatReceivedEvent {
    pub entity: Entity,
    pub packet: ChatPacket,
}

#[derive(Message)]
pub struct SendChatEvent {
    pub entity: Entity,
    pub content: String,
}

pub fn handle_send_chat_event(
    mut events: MessageReader<SendChatEvent>,
    mut send_chat_kind_events: MessageWriter<SendChatKindEvent>,
) {
    for event in events.read() {
        if event.content.starts_with('/') {
            send_chat_kind_events.write(SendChatKindEvent {
                entity: event.entity,
                content: event.content[1..].to_string(),
                kind: ChatKind::Command,
            });
        } else {
            send_chat_kind_events.write(SendChatKindEvent {
                entity: event.entity,
                content: event.content.clone(),
                kind: ChatKind::Message,
            });
        }
    }
}

pub enum ChatKind {
    Message,
    Command,
}
