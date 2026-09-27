use std::sync::Arc;

use crate::play::dialog::Click;
use crate::session::SlotStack;
use crate::text::{Span, Style};

#[derive(Clone, Debug)]
pub struct SpanEvent {
    pub click: Option<Click>,
    pub hover: Option<Hover>,
    pub insertion: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Hover {
    Lines(Vec<Vec<Span>>),
    Item(SlotStack),
}

pub type Events = Option<Arc<[SpanEvent]>>;

pub fn get(events: &Events, style: Style) -> Option<&SpanEvent> {
    events.as_ref()?.get(style.event.checked_sub(1)? as usize)
}

pub fn push(table: &mut Vec<SpanEvent>, event: SpanEvent) -> u16 {
    if table.len() >= u16::MAX as usize {
        return 0;
    }
    table.push(event);
    table.len() as u16
}

pub fn split_lines(spans: Vec<Span>) -> Vec<Vec<Span>> {
    if !spans.iter().any(|s| s.text.contains('\n')) {
        return vec![spans];
    }
    let mut out = vec![Vec::new()];
    for span in spans {
        for (i, part) in span.text.split('\n').enumerate() {
            if i > 0 {
                out.push(Vec::new());
            }
            if !part.is_empty() {
                out.last_mut().unwrap().push(Span {
                    text: part.to_string(),
                    style: span.style,
                });
            }
        }
    }
    out
}
