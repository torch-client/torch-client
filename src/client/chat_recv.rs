use crate::platform::time::{SystemTime, UNIX_EPOCH};
use crate::{log_debug, log_info};
use azalea::client_chat::ChatPacket;
use azalea::{Client, FormattedText};
use azalea_core::bitset::BitSet;
use azalea_protocol::packets::game::c_player_chat::{ClientboundPlayerChat, FilterMask};
use azalea_registry::identifier::Identifier;
use std::sync::Arc;

use crate::client::chat_text;
use crate::session::{ChatEntry, ChatTag, SharedMutex};

const CHAT_DISABLED_KEYS: &[(&str, &str, bool)] = &[
    (
        "chat.disabled.missingProfileKey",
        "Chat disabled due to missing profile public key. Please try reconnecting.",
        true,
    ),
    (
        "chat.disabled.chain_broken",
        "Chat disabled due to broken chain. Please try reconnecting.",
        true,
    ),
    (
        "chat.disabled.expiredProfileKey",
        "Chat disabled due to expired profile public key. Please try reconnecting.",
        true,
    ),
    (
        "chat.disabled.invalid_signature",
        "Chat had an invalid signature. Please try reconnecting.",
        true,
    ),
    (
        "chat.disabled.out_of_order_chat",
        "Chat received out-of-order. Did your system time change?",
        false,
    ),
    (
        "chat.disabled.options",
        "Chat disabled in client options.",
        false,
    ),
    (
        "chat.disabled.invalid_command_signature",
        "The command had unexpected or missing command argument signatures.",
        true,
    ),
];

pub(crate) fn handle_chat(bot: &Client, shared: &Arc<SharedMutex>, m: ChatPacket) {
    if let (Some(sender), msg) = m.split_sender_and_content() {
        log_info!("chat", "<{sender}> {msg}");
    } else {
        log_info!("chat", "{}", m.content());
    }

    if let ChatPacket::Player(p) = &m {
        acknowledge(bot, shared, p);
    }

    let world = bot.world().ok();
    let world_guard = world.as_ref().map(|w| w.read());
    let registries = world_guard.as_ref().map(|g| &g.registries);
    if let Some(r) = registries
        && let (Some(bound), kind) = match &m {
            ChatPacket::Player(p) => (Some(&p.chat_type), "player"),
            ChatPacket::Disguised(p) => (Some(&p.chat_type), "disguised"),
            ChatPacket::System(_) => (None, "system"),
        }
    {
        let order = r
            .extra
            .get(&Identifier::new("chat_type"))
            .map(|entries| {
                entries
                    .map
                    .keys()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        log_debug!(
            "chat",
            "protocol {}: {kind} chat_type = {}, registry order = {order:?}",
            crate::protocol::session(),
            bound.debug_source(r)
        );
    }
    let decorated = match (&m, registries) {
        (ChatPacket::Player(p), Some(r)) => p.message_using_registries(r),
        (ChatPacket::Disguised(p), Some(r)) => p.message_using_registries(r),
        _ => m.message(),
    };
    let (decorated_spans, decorated_events) = chat_text::to_spans_events(&decorated, registries);
    drop(world_guard);

    let mut send_error: Option<String> = None;
    let mut signature_wanted = false;
    let mut original: Option<String> = None;
    let mut filtered: Option<Vec<crate::text::Span>> = None;

    let tag = match &m {
        ChatPacket::System(p) => {
            if p.overlay {
                let spans = chat_text::to_spans(&decorated);
                shared.lock().unwrap().session.set_action_bar(spans);
                return;
            }
            if let FormattedText::Translatable(t) = &p.content
                && let Some((_, text, wanted)) =
                    CHAT_DISABLED_KEYS.iter().find(|(key, ..)| *key == t.key)
            {
                send_error = Some((*text).to_string());
                signature_wanted = *wanted;
            }
            ChatTag::System
        }
        ChatPacket::Disguised(_) => ChatTag::System,
        ChatPacket::Player(p) => {
            match &p.filter_mask {
                FilterMask::FullyFiltered => return,
                FilterMask::PartiallyFiltered(bits) => {
                    filtered = Some(filtered_spans(&p.body.content, bits));
                }
                FilterMask::PassThrough => {}
            }
            let tag = trust_level(p, &decorated.to_string());
            if p.signature.is_some() {
                original = Some(p.body.content.clone());
            }
            tag
        }
    };

    let (spans, events) = match filtered {
        Some(spans) => (spans, None),
        None => (decorated_spans, decorated_events),
    };
    let spans = crate::modules::special::filter(spans);

    let mut s = shared.lock().unwrap();

    s.session.chat_incoming.push(ChatEntry {
        spans,
        events,
        tag,
        original,
    });
    if send_error.is_some() {
        s.session.chat_send_error = send_error;
    }
    if signature_wanted
        && !s.chat_signing_allowed
        && !s.session.chat_signing_session
        && !s.session.chat_signing_declined
        && !s.session.chat_signing_prompt
    {
        drop(s);
        let capable = crate::client::chat_sign::capable(bot);
        shared.lock().unwrap().session.chat_signing_prompt = capable;
    }
}

fn acknowledge(bot: &Client, shared: &Arc<SharedMutex>, p: &ClientboundPlayerChat) {
    use azalea_protocol::packets::game::s_chat_ack::ServerboundChatAck;

    const ACK_AFTER: u32 = 64;

    let Some(signature) = &p.signature else {
        return;
    };
    let mut s = shared.lock().unwrap();
    if s.session.chat_ack_last.as_ref() == Some(&signature.bytes) {
        return;
    }
    s.session.chat_ack_last = Some(signature.bytes);
    s.session.chat_ack_pending += 1;
    if s.session.chat_ack_pending <= ACK_AFTER {
        return;
    }
    let messages = std::mem::take(&mut s.session.chat_ack_pending);
    drop(s);
    bot.write_packet(ServerboundChatAck { messages });
}

fn trust_level(p: &ClientboundPlayerChat, decorated: &str) -> ChatTag {
    if p.signature.is_none() {
        return ChatTag::NotSecure;
    }

    const EXPIRY_MS: u64 = 7 * 60 * 1000;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if now_ms > p.body.timestamp.saturating_add(EXPIRY_MS) {
        return ChatTag::NotSecure;
    }

    if !decorated.contains(&p.body.content) {
        return ChatTag::Modified;
    }
    ChatTag::Secure
}

fn filtered_spans(content: &str, bits: &BitSet) -> Vec<crate::text::Span> {
    use crate::text::{Span, Style};

    let span = |text: String, hidden: bool| Span {
        text,
        style: Style {
            color: if hidden { 0x555555 } else { 0xFFFFFF },
            ..Style::default()
        },
    };

    let mut out = Vec::new();
    let mut run = String::new();
    let mut run_hidden = bits.get(0).unwrap_or(false);
    for (i, ch) in content.chars().enumerate() {
        let hidden = bits.get(i).unwrap_or(false);
        if hidden != run_hidden {
            if !run.is_empty() {
                out.push(span(std::mem::take(&mut run), run_hidden));
            }
            run_hidden = hidden;
        }
        run.push(if hidden { '#' } else { ch });
    }
    if !run.is_empty() {
        out.push(span(run, run_hidden));
    }
    out
}
