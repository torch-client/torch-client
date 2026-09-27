use crate::session::{ChatEntry, ChatTag, SessionState};
use crate::text::{Span, Style};

pub(super) fn show_local(session: &mut SessionState, text: &str) {
    session.chat_incoming.push(ChatEntry {
        spans: vec![Span {
            text: text.to_string(),
            style: Style::colored(0x55FF55),
        }],
        events: None,
        tag: ChatTag::System,
        original: None,
    });
}
