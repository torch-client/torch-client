use azalea::ecs::prelude::*;
use azalea_protocol::common::server_links::{KnownLinkKind, ServerLinkEntry, ServerLinkKind};
use azalea_protocol::common::tags::TagMap;
use azalea_protocol::packets::config::ClientboundConfigPacket;

use crate::SHARED;
use crate::log_warn;
use crate::play::dialog;
use crate::session::ServerLink;

pub(crate) fn links(entries: &[ServerLinkEntry]) -> Vec<ServerLink> {
    entries
        .iter()
        .map(|entry| ServerLink {
            label: match &entry.kind {
                ServerLinkKind::Component(text) => crate::client::chat_text::to_spans(text),
                ServerLinkKind::Known(kind) => {
                    crate::text::styled(known_link_name(*kind), crate::text::Style::default())
                }
            },
            url: entry.link.clone(),
        })
        .collect()
}

fn known_link_name(kind: KnownLinkKind) -> &'static str {
    match kind {
        KnownLinkKind::BugReport => "Report Server Bug",
        KnownLinkKind::CommunityGuidelines => "Community Guidelines",
        KnownLinkKind::Support => "Support",
        KnownLinkKind::Status => "Status",
        KnownLinkKind::Feedback => "Feedback",
        KnownLinkKind::Community => "Community",
        KnownLinkKind::Website => "Website",
        KnownLinkKind::Forums => "Forums",
        KnownLinkKind::News => "News",
        KnownLinkKind::Announcements => "Announcements",
    }
}

pub(crate) fn update_tags(tags: &TagMap) {
    let Some(entries) = tags.get(&azalea_registry::identifier::Identifier::new("dialog")) else {
        return;
    };
    dialog::set_tags(
        entries
            .iter()
            .map(|tag| (tag.name.to_string(), tag.elements.clone())),
    );
}

pub(crate) struct ConfigRelayPlugin;

impl azalea::app::Plugin for ConfigRelayPlugin {
    fn build(&self, app: &mut azalea::app::App) {
        app.add_systems(
            azalea::app::Update,
            (relay_config_packets, send_dialog_submits),
        );
    }
}

fn relay_config_packets(
    mut events: MessageReader<azalea::packet::config::ReceiveConfigPacketEvent>,
    worlds: Query<&azalea::local_player::WorldHolder>,
) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    for event in events.read() {
        match event.packet.as_ref() {
            ClientboundConfigPacket::ShowDialog(p) => {
                let Ok(world) = worlds.get(event.entity) else {
                    continue;
                };
                let parsed = {
                    let world = world.shared.read();
                    dialog::parse_direct(&p.dialog, &world.registries)
                };
                if let Some(parsed) = parsed {
                    shared.lock().unwrap().session.dialog_show = Some(parsed);
                }
            }
            ClientboundConfigPacket::ClearDialog(_) => {
                shared.lock().unwrap().session.dialog_clear = true;
            }
            ClientboundConfigPacket::ServerLinks(p) => {
                shared.lock().unwrap().session.server_links = links(&p.links);
            }
            ClientboundConfigPacket::UpdateTags(p) => update_tags(&p.tags),
            _ => {}
        }
    }
}

fn send_dialog_submits(
    mut commands: Commands,
    query: Query<(Entity, Option<&azalea::InConfigState>), With<azalea::local_player::WorldHolder>>,
) {
    let Some(shared) = SHARED.get() else {
        return;
    };
    let submits = dialog::take_submits(shared);
    if submits.is_empty() {
        return;
    }

    let Ok((entity, in_config)) = query.single() else {
        return;
    };
    let in_config = in_config.is_some();
    for submit in submits {
        match submit {
            dialog::Submit::RunCommand(command) if in_config => {
                log_warn!(
                    "dialog",
                    "commands are not supported in the configuration phase, \
                     so '{command}' was not sent"
                );
            }
            dialog::Submit::RunCommand(command) => {
                use azalea_protocol::packets::game::s_chat_command::ServerboundChatCommand;
                commands.trigger(azalea::packet::game::SendGamePacketEvent::new(
                    entity,
                    ServerboundChatCommand { command },
                ));
            }
            dialog::Submit::Custom { id, payload } => {
                let id = azalea_registry::identifier::Identifier::new(id);
                let payload = nbt(payload);
                if in_config {
                    use azalea_protocol::packets::config::s_custom_click_action::ServerboundCustomClickAction;
                    commands.trigger(azalea::packet::config::SendConfigPacketEvent::new(
                        entity,
                        ServerboundCustomClickAction { id, payload },
                    ));
                } else {
                    use azalea_protocol::packets::game::s_custom_click_action::ServerboundCustomClickAction;
                    commands.trigger(azalea::packet::game::SendGamePacketEvent::new(
                        entity,
                        ServerboundCustomClickAction { id, payload },
                    ));
                }
            }
        }
    }
}

fn nbt(payload: Option<simdnbt::owned::NbtTag>) -> simdnbt::owned::Nbt {
    match payload {
        Some(simdnbt::owned::NbtTag::Compound(compound)) => {
            simdnbt::owned::Nbt::new(simdnbt::Mutf8String::from(""), compound)
        }
        _ => simdnbt::owned::Nbt::None,
    }
}
