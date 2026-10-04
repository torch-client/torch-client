use super::indices;
use super::lexer::{self, Kind, Token};

fn loop_body(tokens: &[Token], keyword: usize) -> Option<(usize, usize)> {
    let start = if tokens[keyword].is_word("do") {
        lexer::next_solid(tokens, keyword + 1)?
    } else {
        let open = lexer::next_solid(tokens, keyword + 1).filter(|o| tokens[*o].is_punct('('))?;
        lexer::next_solid(tokens, lexer::matching(tokens, open)? + 1)?
    };
    if tokens[start].is_punct('{') {
        return Some((start, lexer::matching(tokens, start)?));
    }
    if tokens[start].kind == Kind::Word
        && matches!(
            tokens[start].text.as_str(),
            "if" | "for" | "while" | "do" | "switch"
        )
    {
        return None;
    }
    let mut depth = 0i32;
    for at in start..tokens.len() {
        if tokens[at].is_punct('(') || tokens[at].is_punct('[') {
            depth += 1;
        } else if tokens[at].is_punct(')') || tokens[at].is_punct(']') {
            depth -= 1;
        } else if depth == 0 && tokens[at].is_punct(';') {
            return Some((start, at));
        }
    }
    None
}

pub(super) fn sequence_increments(tokens: &mut Vec<Token>) {
    let mut from = 0;
    'scan: loop {
        for at in from..tokens.len() {
            if !tokens[at].is_word("for") {
                continue;
            }
            let Some(open) = lexer::next_solid(tokens, at + 1).filter(|o| tokens[*o].is_punct('('))
            else {
                continue;
            };
            let Some(close) = lexer::matching(tokens, open) else {
                continue;
            };
            let semicolons = lexer::at_depth_zero(tokens, open + 1, close, |t| t.is_punct(';'));
            let [_, second] = semicolons[..] else {
                continue;
            };
            let commas: Vec<usize> =
                lexer::at_depth_zero(tokens, second + 1, close, |t| t.is_punct(','));
            if commas.is_empty() {
                continue;
            }
            let Some((body_start, body_end)) = loop_body(tokens, at) else {
                continue;
            };
            let mut moved: Vec<Token> = Vec::new();
            let mut part_start = semicolons[1] + 1;
            for &comma in &commas {
                let first = (part_start..comma)
                    .find(|i| !tokens[*i].is_trivia())
                    .unwrap_or(comma);
                moved.extend(tokens[first..comma].iter().cloned());
                moved.extend([Token::punct(';'), Token::space(" ")]);
                part_start = comma + 1;
            }
            let last_comma = *commas.last().expect("checked non-empty");

            let mut nested: Vec<(usize, usize)> = Vec::new();
            for i in body_start + 1..=body_end {
                if (tokens[i].is_word("for")
                    || tokens[i].is_word("while")
                    || tokens[i].is_word("do"))
                    && let Some(range) = loop_body(tokens, i)
                {
                    nested.push(range);
                }
            }
            let continues: Vec<usize> = (body_start..=body_end)
                .filter(|i| tokens[*i].is_word("continue"))
                .filter(|i| !nested.iter().any(|(a, b)| a <= i && i <= b))
                .collect();

            let braced = tokens[body_start].is_punct('{');
            let mut end_insert: Vec<Token> = Vec::new();
            end_insert.extend(moved.iter().cloned());
            if braced {
                tokens.splice(body_end..body_end, end_insert);
            } else {
                end_insert.extend([Token::space(" "), Token::punct('}')]);
                tokens.splice(body_end + 1..body_end + 1, end_insert);
            }
            for &c in continues.iter().rev() {
                let mut before = vec![Token::punct('{'), Token::space(" ")];
                before.extend(moved.iter().cloned());
                if let Some(semi) =
                    lexer::next_solid(tokens, c + 1).filter(|s| tokens[*s].is_punct(';'))
                {
                    tokens.splice(semi + 1..semi + 1, [Token::space(" "), Token::punct('}')]);
                    tokens.splice(c..c, before);
                }
            }
            if !braced {
                tokens.splice(
                    body_start..body_start,
                    [Token::punct('{'), Token::space(" ")],
                );
            }
            tokens.drain(semicolons[1] + 1..=last_comma);
            from = at + 1;
            continue 'scan;
        }
        break;
    }
}

pub(super) const LOOP_BUDGET: u32 = 4096;

pub(super) fn guard_loops(tokens: &mut Vec<Token>) -> bool {
    const TEST: &str = "iris_LoopBudget-- > 0";
    let mut inserts: Vec<(usize, String)> = Vec::new();
    let constants = indices::int_constants(tokens);
    for at in 0..tokens.len() {
        let is_for = tokens[at].is_word("for");
        if !(is_for || tokens[at].is_word("while")) {
            continue;
        }
        let Some(open) = lexer::next_solid(tokens, at + 1).filter(|o| tokens[*o].is_punct('('))
        else {
            continue;
        };
        let Some(close) = lexer::matching(tokens, open) else {
            continue;
        };
        let semicolons = lexer::at_depth_zero(tokens, open + 1, close, |t| t.is_punct(';'));
        let (from, to) = if is_for {
            match semicolons[..] {
                [first, second]
                    if indices::fixed_count(tokens, &constants, open, [first, second], close) =>
                {
                    continue;
                }
                [first, second] => (first, second),
                _ => continue,
            }
        } else {
            (open, close)
        };
        match (from + 1..to).find(|i| !tokens[*i].is_trivia()) {
            Some(first) => {
                inserts.push((to, format!(") && ({TEST})")));
                inserts.push((first, "(".to_owned()));
            }
            None => inserts.push((to, format!(" {TEST}"))),
        }
    }
    let found = !inserts.is_empty();
    inserts.sort_by(|a, b| b.0.cmp(&a.0));
    for (index, text) in inserts {
        tokens.insert(index, Token::raw(&text));
    }
    found
}
