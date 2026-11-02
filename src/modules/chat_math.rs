use crate::session::{ChatEntry, ChatTag, SessionState};
use crate::text::{Span, Style};

pub(super) fn show_local(session: &mut SessionState, text: &str) {
    show_local_colored(session, text, GREEN);
}

pub(super) const GREEN: u32 = 0x55FF55;
#[cfg_attr(not(feature = "click_gui"), allow(dead_code))]
pub(super) const RED: u32 = 0xFF5555;

pub(super) fn show_local_colored(session: &mut SessionState, text: &str, color: u32) {
    show_local_spans(
        session,
        vec![Span {
            text: text.to_string(),
            style: Style::colored(color),
        }],
    );
}

pub(super) fn show_local_spans(session: &mut SessionState, spans: Vec<Span>) {
    session.chat_incoming.push(ChatEntry {
        spans,
        events: None,
        tag: ChatTag::System,
        original: None,
    });
}
