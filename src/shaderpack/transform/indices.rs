use std::collections::HashMap;

use super::super::literal;
use super::declarations::PackUniform;
use super::lexer::{self, Kind, Token};

pub(super) fn int_constants(tokens: &[Token]) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    for at in 0..tokens.len() {
        if !tokens[at].is_word("const") {
            continue;
        }
        let Some([ty, name, eq, value, end]) = lexer::solid_run::<5>(tokens, at + 1) else {
            continue;
        };
        if (tokens[ty].is_word("int") || tokens[ty].is_word("uint"))
            && tokens[name].kind == Kind::Word
            && tokens[eq].is_punct('=')
            && tokens[end].is_punct(';')
            && let Some(n) = literal::int(&tokens[value].text)
        {
            out.insert(tokens[name].text.clone(), n);
        }
    }
    out
}

fn int_value(token: &Token, constants: &HashMap<String, i64>) -> Option<i64> {
    match token.kind {
        Kind::Number => literal::int(&token.text),
        Kind::Word => constants.get(&token.text).copied(),
        _ => None,
    }
}

fn declares(tokens: &[Token], name: usize) -> bool {
    lexer::after_type(tokens, name)
}

fn array_sizes(
    tokens: &[Token],
    constants: &HashMap<String, i64>,
    uniforms: &[PackUniform],
) -> HashMap<String, Option<i64>> {
    let mut sizes: HashMap<String, Option<i64>> = HashMap::new();
    let mut record = |name: &str, size: Option<i64>| {
        sizes
            .entry(name.to_owned())
            .and_modify(|known| {
                if *known != size {
                    *known = None;
                }
            })
            .or_insert(size);
    };
    for uniform in uniforms {
        if let Some(count) = uniform.array {
            record(&uniform.name, Some(i64::from(count)));
        }
    }
    for at in 0..tokens.len() {
        if tokens[at].kind != Kind::Word {
            continue;
        }
        let Some(next) = lexer::next_solid(tokens, at + 1) else {
            continue;
        };
        if tokens[next].is_punct('[') {
            let Some(close) = lexer::matching(tokens, next) else {
                continue;
            };
            let inner: Vec<usize> = (next + 1..close)
                .filter(|i| !tokens[*i].is_trivia())
                .collect();
            let size = match inner[..] {
                [one] => int_value(&tokens[one], constants),
                _ => None,
            };
            if let Some(name) =
                lexer::next_solid(tokens, close + 1).filter(|n| tokens[*n].kind == Kind::Word)
                && lexer::next_solid(tokens, name + 1).is_some_and(|after| {
                    [';', '=', ',', ')']
                        .iter()
                        .any(|c| tokens[after].is_punct(*c))
                })
            {
                record(&tokens[name].text, size);
            } else if declares(tokens, at) {
                record(&tokens[at].text, size);
            }
        } else if declares(tokens, at) && [';', '=', ','].iter().any(|c| tokens[next].is_punct(*c))
        {
            record(&tokens[at].text, None);
        }
    }
    sizes
}

pub(super) fn clamp_array_indices(tokens: &mut Vec<Token>, uniforms: &[PackUniform]) {
    let constants = int_constants(tokens);
    let sizes = array_sizes(tokens, &constants, uniforms);
    for at in (0..tokens.len()).rev() {
        if tokens[at].kind != Kind::Word {
            continue;
        }
        let Some(Some(size)) = sizes.get(&tokens[at].text) else {
            continue;
        };
        let size = *size;
        if size < 1 || declares(tokens, at) {
            continue;
        }
        let Some(open) = lexer::next_solid(tokens, at + 1).filter(|n| tokens[*n].is_punct('['))
        else {
            continue;
        };
        let Some(close) = lexer::matching(tokens, open) else {
            continue;
        };
        let inner: Vec<usize> = (open + 1..close)
            .filter(|i| !tokens[*i].is_trivia())
            .collect();
        match inner[..] {
            [] => continue,
            [one] if tokens[one].kind == Kind::Number => continue,
            _ => {}
        }
        let expression = lexer::render(&tokens[open + 1..close]);
        let clamped = lexer::lex(&format!("clamp(int({expression}), 0, {})", size - 1));
        tokens.splice(open + 1..close, clamped);
    }
}

pub(super) fn fixed_count(
    tokens: &[Token],
    constants: &HashMap<String, i64>,
    open: usize,
    semicolons: [usize; 2],
    close: usize,
) -> bool {
    let solid = |from: usize, to: usize| -> Vec<usize> {
        (from..to).filter(|i| !tokens[*i].is_trivia()).collect()
    };
    let init = solid(open + 1, semicolons[0]);
    let condition = solid(semicolons[0] + 1, semicolons[1]);
    let step = solid(semicolons[1] + 1, close);
    let is_int = |i: usize| int_value(&tokens[i], constants).is_some();

    let name = match init[..] {
        [ty, name, eq, value]
            if (tokens[ty].is_word("int") || tokens[ty].is_word("uint"))
                && tokens[eq].is_punct('=')
                && is_int(value) =>
        {
            name
        }
        [name, eq, value] if tokens[eq].is_punct('=') && is_int(value) => name,
        _ => return false,
    };
    let var = tokens[name].text.as_str();
    let is_var = |i: usize| tokens[i].is_word(var);

    let rising = match condition[..] {
        [v, op, value] if is_var(v) && is_int(value) && tokens[op].is_punct('<') => Some(true),
        [v, op, value] if is_var(v) && is_int(value) && tokens[op].is_punct('>') => Some(false),
        [v, op, eq, value] if is_var(v) && is_int(value) && tokens[eq].is_punct('=') => {
            if tokens[op].is_punct('<') {
                Some(true)
            } else if tokens[op].is_punct('>') {
                Some(false)
            } else if tokens[op].is_punct('!') {
                None
            } else {
                return false;
            }
        }
        _ => return false,
    };

    let up = match step[..] {
        [v, a, b] if is_var(v) && tokens[a].is_punct('+') && tokens[b].is_punct('+') => true,
        [a, b, v] if is_var(v) && tokens[a].is_punct('+') && tokens[b].is_punct('+') => true,
        [v, a, b] if is_var(v) && tokens[a].is_punct('-') && tokens[b].is_punct('-') => false,
        [a, b, v] if is_var(v) && tokens[a].is_punct('-') && tokens[b].is_punct('-') => false,
        [v, op, eq, value]
            if is_var(v)
                && tokens[eq].is_punct('=')
                && int_value(&tokens[value], constants).is_some_and(|n| n > 0) =>
        {
            if tokens[op].is_punct('+') {
                true
            } else if tokens[op].is_punct('-') {
                false
            } else {
                return false;
            }
        }
        _ => return false,
    };
    if rising.is_some_and(|rising| rising != up) {
        return false;
    }

    let Some(body) = lexer::next_solid(tokens, close + 1).filter(|b| tokens[*b].is_punct('{'))
    else {
        return false;
    };
    let Some(end) = lexer::matching(tokens, body) else {
        return false;
    };
    let body = solid(body + 1, end);
    let assigns = body.iter().enumerate().any(|(k, &i)| {
        if !is_var(i) {
            return false;
        }
        let at = |d: usize| body.get(k + d).map(|&t| &tokens[t]);
        let before = |d: usize| k.checked_sub(d).map(|b| &tokens[body[b]]);
        let written_after = match (at(1), at(2)) {
            (Some(a), Some(b)) if a.is_punct('=') => !b.is_punct('='),
            (Some(a), Some(b)) if b.is_punct('=') => ['+', '-', '*', '/', '%', '&', '|', '^'].iter().any(|c| a.is_punct(*c)),
            (Some(a), Some(b)) => (a.is_punct('+') && b.is_punct('+')) || (a.is_punct('-') && b.is_punct('-')),
            (Some(a), None) => a.is_punct('='),
            _ => false,
        };
        let written_before = matches!((before(2), before(1)), (Some(a), Some(b)) if (a.is_punct('+') && b.is_punct('+')) || (a.is_punct('-') && b.is_punct('-')));
        written_after || written_before
    });
    !assigns
}

fn parity_uses(tokens: &[Token]) -> (Vec<usize>, bool) {
    let mut parity = Vec::new();
    let mut other = false;
    for at in 0..tokens.len() {
        if !tokens[at].is_word("frameCounter") || declares(tokens, at) {
            continue;
        }
        let parity_read = lexer::solid_run::<2>(tokens, at + 1).is_some_and(|[op, n]| {
            let value = literal::int(&tokens[n].text);
            (tokens[op].is_punct('&') && value == Some(1))
                || (tokens[op].is_punct('%') && value == Some(2))
        });
        if parity_read {
            parity.push(at);
        } else {
            other = true;
        }
    }
    let only = !parity.is_empty() && !other;
    (parity, only)
}

pub(crate) fn parity_only(source: &str) -> bool {
    parity_uses(&lexer::lex(source)).1
}

pub(super) fn fold_frame_parity(tokens: &mut Vec<Token>, parity: u32) {
    let (uses, _) = parity_uses(tokens);
    for at in uses.into_iter().rev() {
        let Some([_, n]) = lexer::solid_run::<2>(tokens, at + 1) else {
            continue;
        };
        tokens.splice(at..=n, lexer::lex(&format!("{parity}")));
    }
    fold_constant_ifs(tokens);
}

fn constant_condition(tokens: &[Token], from: usize, to: usize) -> Option<bool> {
    let solid: Vec<&Token> = tokens[from..to]
        .iter()
        .filter(|t| !t.is_trivia() && !t.is_punct('(') && !t.is_punct(')'))
        .collect();
    let int = |t: &Token| {
        if t.kind == Kind::Number {
            literal::int(&t.text)
        } else {
            None
        }
    };
    match solid[..] {
        [a] => Some(int(a)? != 0),
        [a, op, eq, b] if eq.is_punct('=') => {
            let (a, b) = (int(a)?, int(b)?);
            if op.is_punct('=') {
                Some(a == b)
            } else if op.is_punct('!') {
                Some(a != b)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn fold_constant_ifs(tokens: &mut Vec<Token>) {
    let mut at = tokens.len();
    while at > 0 {
        at -= 1;
        if !tokens[at].is_word("if") {
            continue;
        }
        let Some(open) = lexer::next_solid(tokens, at + 1).filter(|o| tokens[*o].is_punct('('))
        else {
            continue;
        };
        let Some(close) = lexer::matching(tokens, open) else {
            continue;
        };
        let Some(value) = constant_condition(tokens, open + 1, close) else {
            continue;
        };
        let Some(then_open) =
            lexer::next_solid(tokens, close + 1).filter(|b| tokens[*b].is_punct('{'))
        else {
            continue;
        };
        let Some(then_close) = lexer::matching(tokens, then_open) else {
            continue;
        };
        let else_block = lexer::next_solid(tokens, then_close + 1)
            .filter(|e| tokens[*e].is_word("else"))
            .and_then(|e| {
                let open = lexer::next_solid(tokens, e + 1).filter(|b| tokens[*b].is_punct('{'))?;
                Some((open, lexer::matching(tokens, open)?))
            });
        if lexer::next_solid(tokens, then_close + 1).is_some_and(|e| tokens[e].is_word("else"))
            && else_block.is_none()
        {
            continue;
        }
        let end = else_block.map_or(then_close, |(_, close)| close);
        let kept: Vec<Token> = match (value, else_block) {
            (true, _) => tokens[then_open..=then_close].to_vec(),
            (false, Some((open, close))) => tokens[open..=close].to_vec(),
            (false, None) => Vec::new(),
        };
        tokens.splice(at..=end, kept);
    }
}

pub(super) fn drop_unreachable(tokens: &mut Vec<Token>) {
    let mut at = tokens.len();
    while at > 0 {
        at -= 1;
        if !(tokens[at].is_word("return") || tokens[at].is_word("discard")) {
            continue;
        }
        if lexer::prev_solid(tokens, at).is_none() || !lexer::starts_statement(tokens, at) {
            continue;
        }
        let Some(end) = (at..tokens.len()).find(|i| tokens[*i].is_punct(';')) else {
            continue;
        };
        let mut depth = 0u32;
        let mut stop = None;
        for (i, token) in tokens.iter().enumerate().skip(end + 1) {
            if token.is_punct('{') {
                depth += 1;
            } else if token.is_punct('}') {
                if depth == 0 {
                    stop = Some(i);
                    break;
                }
                depth -= 1;
            } else if depth == 0 && (token.is_word("case") || token.is_word("default")) {
                stop = Some(i);
                break;
            }
        }
        let Some(stop) = stop else { continue };
        if tokens[end + 1..stop].iter().any(|t| !t.is_trivia()) {
            tokens.splice(end + 1..stop, [Token::newline()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reachable(source: &str) -> String {
        let mut tokens = lexer::lex(source);
        drop_unreachable(&mut tokens);
        lexer::render(&tokens)
    }

    #[test]
    fn statements_after_a_terminator_are_dropped() {
        let out = reachable(
            "float f(float d) { d *= 2.0; return 0.5 * d; float p = d; if (p > 0.0) { return p; } return d; }\n",
        );
        assert!(out.contains("return 0.5 * d;"), "{out}");
        assert!(
            !out.contains("float p") && !out.contains("return d;"),
            "{out}"
        );
        assert!(
            out.trim_end().ends_with('}'),
            "the function's brace stays: {out}"
        );
        let kept = reachable("void main() { if (a) return; b(); if (c) discard; d(); }");
        assert!(
            kept.contains("b();") && kept.contains("d();"),
            "a branch body is not its block: {kept}"
        );
        let switch = reachable("void f() { switch (k) { case 0: return; x(); case 1: y(); } }");
        assert!(
            !switch.contains("x();") && switch.contains("case 1: y();"),
            "{switch}"
        );
    }

    fn clamped(source: &str) -> String {
        let mut tokens = lexer::lex(source);
        clamp_array_indices(&mut tokens, &[]);
        lexer::render(&tokens)
    }

    #[test]
    fn dynamic_indices_into_known_arrays_are_clamped() {
        let out = clamped(
            "const int N = 8;\nvec2 offsets[N];\nfloat f(int k) { return offsets[k].x + offsets[3].y; }\n",
        );
        assert!(out.contains("offsets[clamp(int(k), 0, 7)].x"), "{out}");
        assert!(
            out.contains("offsets[3].y"),
            "a literal index is left alone: {out}"
        );
        assert!(
            out.contains("vec2 offsets[N];"),
            "the declaration is left alone: {out}"
        );
    }

    #[test]
    fn nested_and_expression_indices_are_clamped() {
        let out = clamped("int a[4];\nfloat b[2];\nvoid main() { b[a[i + 1]] = 1.0; }\n");
        assert!(
            out.contains("b[clamp(int(a[clamp(int(i + 1), 0, 3)]), 0, 1)]"),
            "{out}"
        );
    }

    #[test]
    fn a_name_that_is_also_a_vector_is_left_alone() {
        let out = clamped("float v[2];\nvoid f() { vec4 v; float x = v[i]; }\n");
        assert!(out.contains("v[i]"), "{out}");
    }

    fn loop_is_fixed(source: &str) -> bool {
        let tokens = lexer::lex(source);
        let open = tokens.iter().position(|t| t.is_punct('(')).unwrap();
        let semis: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_punct(';'))
            .map(|(i, _)| i)
            .collect();
        let close = lexer::matching(&tokens, open).unwrap();
        fixed_count(
            &tokens,
            &int_constants(&tokens),
            open,
            [semis[0], semis[1]],
            close,
        )
    }

    #[test]
    fn frame_parity_folds_only_when_it_is_all_that_is_read() {
        let solas = "uniform int frameCounter;\nvoid main() { if ((frameCounter & 1) == 0) { a(); } else { b(); } x = frameCounter % 2; }\n";
        assert!(parity_only(solas));
        let mut tokens = lexer::lex(solas);
        fold_frame_parity(&mut tokens, 1);
        let out = lexer::render(&tokens);
        assert!(
            out.contains("b();") && !out.contains("a();"),
            "the odd branch alone: {out}"
        );
        assert!(out.contains("x = 1;"), "{out}");
        assert!(
            out.contains("uniform int frameCounter;"),
            "the declaration stays: {out}"
        );
        assert!(
            !parity_only("void main() { x = frameCounter & 1; y = frameCounter % 8; }"),
            "another use"
        );
        assert!(!parity_only("void main() { x = 1; }"), "no use at all");
    }

    #[test]
    fn counted_loops_are_recognised() {
        assert!(loop_is_fixed("for (int i = 0; i < 8; i++) { x += s[i]; }"));
        assert!(loop_is_fixed(
            "for (int i = 16; i >= 0; i -= 2) { x += 1.0; }"
        ));
        assert!(
            !loop_is_fixed("for (int i = 0; i < n; i++) { x += 1.0; }"),
            "a data bound"
        );
        assert!(
            !loop_is_fixed("for (int i = 0; i < 8; i++) { i = 0; }"),
            "the body writes it"
        );
        assert!(
            !loop_is_fixed("for (int i = 0; i < 8; i++) { i++; }"),
            "the body steps it"
        );
        assert!(
            !loop_is_fixed("for (int i = 0; i < 8; i--) { x += 1.0; }"),
            "steps away from the bound"
        );
        assert!(
            loop_is_fixed("for (int i = 0; i < 8; i++) { if (i == 3) x = 1.0; }"),
            "a comparison is no write"
        );
    }
}
