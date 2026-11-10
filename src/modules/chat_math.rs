use crate::session::{ChatEntry, ChatTag, SessionState, SharedMutex};
use crate::text::{Span, Style};

pub(super) fn show_local(session: &mut SessionState, text: &str) {
    show_local_colored(session, text, GREEN);
}

pub(super) const GREEN: u32 = crate::text::GREEN;
#[cfg_attr(not(feature = "click_gui"), allow(dead_code))]
pub(super) const RED: u32 = crate::text::RED;

pub(super) fn show_local_colored(session: &mut SessionState, text: &str, color: u32) {
    show_local_spans(
        session,
        vec![Span {
            text: text.to_string(),
            style: Style::colored(color),
        }],
    );
}

pub(super) fn say(shared: &SharedMutex, rgb: u32, text: impl Into<String>) {
    let mut s = shared.lock().unwrap();
    show_local_spans(&mut s.session, vec![Span::colored(text, rgb)]);
}

pub(super) fn show_local_spans(session: &mut SessionState, spans: Vec<Span>) {
    show_local_events(session, spans, None);
}

pub(super) fn show_local_events(
    session: &mut SessionState,
    spans: Vec<Span>,
    events: crate::text::events::Events,
) {
    session.chat_incoming.push(ChatEntry {
        spans,
        events,
        tag: ChatTag::System,
        original: None,
    });
}
