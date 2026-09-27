use crate::text::{Span, Style};

use super::parse::Parse;

pub(super) const LITERAL_COLOR: u32 = 0xAAAAAA;
pub(super) const UNPARSED_COLOR: u32 = 0xFF5555;
pub(super) const ARGUMENT_COLORS: [u32; 5] = [0x55FFFF, 0xFFFF55, 0x55FF55, 0xFF55FF, 0xFFAA00];

pub fn format_text(parse: &Parse, input: &[char]) -> Vec<Span> {
    let mut parts: Vec<Span> = Vec::new();
    let mut unformatted_start = 0usize;
    let mut next_color = 0usize;
    let push = |parts: &mut Vec<Span>, from: usize, to: usize, color: u32| {
        if to > from {
            let text: String = input[from..to].iter().collect();
            parts.push(Span {
                text,
                style: Style::colored(color),
            });
        }
    };

    for (_, range) in &parse.last().arguments {
        let color = ARGUMENT_COLORS[next_color % ARGUMENT_COLORS.len()];
        next_color += 1;
        let start = range.start;
        if start >= input.len() {
            break;
        }
        let end = range.end.min(input.len());
        if end > 0 {
            push(&mut parts, unformatted_start, start, LITERAL_COLOR);
            push(&mut parts, start, end, color);
            unformatted_start = end;
        }
    }

    if parse.cursor < input.len() {
        let start = parse.cursor;
        push(&mut parts, unformatted_start, start, LITERAL_COLOR);
        push(&mut parts, start, input.len(), UNPARSED_COLOR);
        unformatted_start = input.len();
    }

    push(&mut parts, unformatted_start, input.len(), LITERAL_COLOR);
    parts
}
