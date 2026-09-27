use super::registry::{Id, special as setting};
use super::store;
use crate::text::Span;

pub fn filter(spans: Vec<Span>) -> Vec<Span> {
    let s = store();
    if !s.enabled(Id::Special) {
        return spans;
    }
    let phrase = s.text(Id::Special, setting::PHRASE);
    if phrase.is_empty() {
        return spans;
    }
    remove_substring(spans, &phrase)
}

fn remove_substring(spans: Vec<Span>, needle: &str) -> Vec<Span> {
    let whole: String = spans.iter().map(|s| s.text.as_str()).collect();
    if !whole.contains(needle) {
        return spans;
    }
    let mut cut = vec![false; whole.len()];
    let mut start = 0;
    while let Some(i) = whole[start..].find(needle) {
        let from = start + i;
        let to = from + needle.len();
        cut[from..to].fill(true);
        start = to;
    }

    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    let mut offset = 0;
    for span in spans {
        let mut kept = String::with_capacity(span.text.len());
        for (i, c) in span.text.char_indices() {
            if !cut[offset + i] {
                kept.push(c);
            }
        }
        offset += span.text.len();
        if !kept.is_empty() {
            out.push(Span {
                text: kept,
                style: span.style,
            });
        }
    }
    out
}
