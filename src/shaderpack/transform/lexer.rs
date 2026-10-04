#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Word,
    Number,
    Punct,
    Whitespace,
    Comment,
    Directive,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub(crate) kind: Kind,
    pub(crate) text: String,
}

impl Token {
    fn new(kind: Kind, text: &str) -> Token {
        Token {
            kind,
            text: text.to_owned(),
        }
    }

    pub(crate) fn raw(text: &str) -> Token {
        Token::new(Kind::Word, text)
    }

    pub(crate) fn punct(c: char) -> Token {
        Token::new(Kind::Punct, c.encode_utf8(&mut [0; 4]))
    }

    pub(crate) fn space(text: &str) -> Token {
        Token::new(Kind::Whitespace, text)
    }

    pub(crate) fn directive(text: &str) -> Token {
        Token::new(Kind::Directive, text)
    }

    pub(crate) fn number(value: usize) -> Token {
        Token::new(Kind::Number, &value.to_string())
    }

    pub(crate) fn newline() -> Token {
        Token::new(Kind::Whitespace, "\n")
    }

    pub(crate) fn is_word(&self, name: &str) -> bool {
        self.kind == Kind::Word && self.text == name
    }

    pub(crate) fn is_punct(&self, c: char) -> bool {
        self.kind == Kind::Punct && self.text.len() == 1 && self.text.starts_with(c)
    }

    pub(crate) fn is_trivia(&self) -> bool {
        matches!(self.kind, Kind::Whitespace | Kind::Comment)
    }
}

pub(crate) fn lex(source: &str) -> Vec<Token> {
    let mut out = Vec::with_capacity(source.len() / 4);
    let bytes = source.as_bytes();
    let mut at = 0;

    while at < bytes.len() {
        let start = at;
        let c = bytes[at];

        if c == b'/' && bytes.get(at + 1) == Some(&b'/') {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
            out.push(Token::new(Kind::Comment, &source[start..at]));
        } else if c == b'/' && bytes.get(at + 1) == Some(&b'*') {
            at += 2;
            while at < bytes.len() && !(bytes[at] == b'*' && bytes.get(at + 1) == Some(&b'/')) {
                at += 1;
            }
            at = (at + 2).min(bytes.len());
            out.push(Token::new(Kind::Comment, &source[start..at]));
        } else if c == b'#' {
            while at < bytes.len() && bytes[at] != b'\n' {
                if bytes[at] == b'\\' && bytes.get(at + 1) == Some(&b'\n') {
                    at += 1;
                }
                at += 1;
            }
            out.push(Token::new(Kind::Directive, &source[start..at]));
        } else if c.is_ascii_whitespace() {
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            out.push(Token::new(Kind::Whitespace, &source[start..at]));
        } else if c == b'_' || c.is_ascii_alphabetic() {
            while at < bytes.len() && (bytes[at] == b'_' || bytes[at].is_ascii_alphanumeric()) {
                at += 1;
            }
            out.push(Token::new(Kind::Word, &source[start..at]));
        } else if c.is_ascii_digit()
            || (c == b'.' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit))
        {
            let mut seen_exponent = false;
            while at < bytes.len() {
                let d = bytes[at];
                if d.is_ascii_alphanumeric() || d == b'.' {
                    seen_exponent |= d == b'e' || d == b'E';
                    at += 1;
                } else if (d == b'+' || d == b'-') && seen_exponent {
                    seen_exponent = false;
                    at += 1;
                } else {
                    break;
                }
            }
            out.push(Token::new(Kind::Number, &source[start..at]));
        } else {
            at += 1;
            while at < bytes.len() && !source.is_char_boundary(at) {
                at += 1;
            }
            out.push(Token::new(Kind::Punct, &source[start..at]));
        }
    }
    out
}

pub(crate) fn render(tokens: &[Token]) -> String {
    let mut out = String::with_capacity(tokens.iter().map(|t| t.text.len()).sum());
    for token in tokens {
        out.push_str(&token.text);
    }
    out
}

pub(crate) fn next_solid(tokens: &[Token], from: usize) -> Option<usize> {
    (from..tokens.len()).find(|at| !tokens[*at].is_trivia())
}

pub(crate) fn solid_run<const N: usize>(tokens: &[Token], from: usize) -> Option<[usize; N]> {
    let mut out = [0usize; N];
    let mut at = from;
    for slot in &mut out {
        *slot = next_solid(tokens, at)?;
        at = *slot + 1;
    }
    Some(out)
}

pub(crate) fn prev_solid(tokens: &[Token], at: usize) -> Option<usize> {
    (0..at).rev().find(|i| !tokens[*i].is_trivia())
}

pub(crate) fn matching(tokens: &[Token], open: usize) -> Option<usize> {
    let (opener, closer) = match tokens[open].text.as_str() {
        "(" => ('(', ')'),
        "{" => ('{', '}'),
        "[" => ('[', ']'),
        _ => return None,
    };
    let mut depth = 0i32;
    for (at, token) in tokens.iter().enumerate().skip(open) {
        if token.is_punct(opener) {
            depth += 1;
        } else if token.is_punct(closer) {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
        }
    }
    None
}

pub(crate) fn arguments(tokens: &[Token], open: usize) -> Option<(Vec<(usize, usize)>, usize)> {
    let mut depth = 0i32;
    let mut items = Vec::new();
    let mut start = open + 1;
    for (at, token) in tokens.iter().enumerate().skip(open) {
        if token.is_punct('(') || token.is_punct('[') {
            depth += 1;
        } else if token.is_punct(')') || token.is_punct(']') {
            depth -= 1;
            if depth == 0 {
                if (start..at).any(|i| !tokens[i].is_trivia()) {
                    items.push((start, at));
                }
                return Some((items, at));
            }
        } else if depth == 1 && token.is_punct(',') {
            items.push((start, at));
            start = at + 1;
        }
    }
    None
}

pub(crate) fn at_depth_zero(
    tokens: &[Token],
    from: usize,
    to: usize,
    is: impl Fn(&Token) -> bool,
) -> Vec<usize> {
    let mut depth = 0i32;
    let mut out = Vec::new();
    for (at, token) in tokens.iter().enumerate().take(to).skip(from) {
        if token.is_punct('(') || token.is_punct('[') {
            depth += 1;
        } else if token.is_punct(')') || token.is_punct(']') {
            depth -= 1;
        } else if depth == 0 && is(token) {
            out.push(at);
        }
    }
    out
}

pub(crate) fn brace_depths(tokens: &[Token]) -> Vec<i32> {
    let mut depth = 0i32;
    tokens
        .iter()
        .map(|token| {
            if token.is_punct('{') {
                depth += 1;
            } else if token.is_punct('}') {
                depth -= 1;
            }
            depth
        })
        .collect()
}

pub(crate) fn after_type(tokens: &[Token], at: usize) -> bool {
    prev_solid(tokens, at).is_some_and(|p| {
        tokens[p].kind == Kind::Word
            && !["return", "else", "case", "do"].contains(&tokens[p].text.as_str())
    })
}

pub(crate) fn starts_statement(tokens: &[Token], at: usize) -> bool {
    prev_solid(tokens, at)
        .is_none_or(|p| [';', '{', '}', ':'].iter().any(|c| tokens[p].is_punct(*c)))
}

pub(crate) fn uniform_declarations(
    tokens: &[Token],
) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
    (0..tokens.len())
        .filter(|at| tokens[*at].is_word("uniform"))
        .filter_map(|at| {
            let [ty, name] = solid_run::<2>(tokens, at + 1)?;
            (tokens[name].kind == Kind::Word).then_some((at, ty, name))
        })
}

pub(crate) fn is_version(token: &Token) -> bool {
    token.kind == Kind::Directive && token.text.trim_start().starts_with("#version")
}

pub(crate) fn in_define(token: &mut Token, mut rename: impl FnMut(&mut Token)) {
    in_define_body(token, |body| {
        for word in body.iter_mut().filter(|t| t.kind == Kind::Word) {
            rename(word);
        }
    });
}

pub(crate) fn in_define_body(token: &mut Token, rewrite: impl FnOnce(&mut Vec<Token>)) {
    let Some(split) = define_body_start(&token.text) else {
        return;
    };
    let mut body = lex(&token.text[split..]);
    rewrite(&mut body);
    token.text = format!("{}{}", &token.text[..split], render(&body));
}

pub(crate) fn define_body(token: &Token) -> Option<Vec<Token>> {
    if token.kind != Kind::Directive {
        return None;
    }
    define_body_start(&token.text).map(|split| lex(&token.text[split..]))
}

fn define_body_start(text: &str) -> Option<usize> {
    let rest = text.trim_start().strip_prefix('#')?;
    let rest = rest.trim_start().strip_prefix("define")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let head_len = text.len() - rest.len();
    let name_start = rest.len() - rest.trim_start().len();
    let after_name = rest[name_start..]
        .find(|c: char| !(c == '_' || c.is_ascii_alphanumeric()))
        .map_or(rest.len(), |n| name_start + n);
    let body_start = if rest[after_name..].starts_with('(') {
        rest[after_name..]
            .find(')')
            .map_or(rest.len(), |close| after_name + close + 1)
    } else {
        after_name
    };
    Some(head_len + body_start)
}

pub(crate) fn in_defines(tokens: &mut [Token], mut rewrite: impl FnMut(&mut Vec<Token>)) {
    for token in tokens.iter_mut().filter(|t| t.kind == Kind::Directive) {
        in_define_body(token, &mut rewrite);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(source: &str) -> Vec<&str> {
        lex(source)
            .iter()
            .filter(|t| t.kind == Kind::Word)
            .map(|t| t.text.clone())
            .map(|s| Box::leak(s.into_boxed_str()) as &str)
            .collect()
    }

    #[test]
    fn every_byte_survives_the_round_trip() {
        let source = "#version 130\n/* c */ void main() { /* x */ } // done\n";
        assert_eq!(render(&lex(source)), source);
    }

    #[test]
    fn a_longer_name_is_not_a_shorter_one() {
        let found = words("texture2DLod(x); // texture2D\n");
        assert_eq!(found, ["texture2DLod", "x"]);
    }

    #[test]
    fn comments_and_directives_are_whole_tokens() {
        let tokens = lex("#version 450 core\n/* a\nb */x");
        assert_eq!(tokens[0].kind, Kind::Directive);
        assert_eq!(tokens[0].text, "#version 450 core");
        assert!(
            tokens
                .iter()
                .any(|t| t.kind == Kind::Comment && t.text.contains('\n'))
        );
    }

    #[test]
    fn numbers_hold_together_but_do_not_swallow_operators() {
        let numbers: Vec<String> = lex("1.0e-3 + .5f - 0x1F-2")
            .iter()
            .filter(|t| t.kind == Kind::Number)
            .map(|t| t.text.clone())
            .collect();
        assert_eq!(numbers, ["1.0e-3", ".5f", "0x1F", "2"]);
    }

    #[test]
    fn a_pattern_matches_however_it_is_spaced() {
        for source in ["gl_FragData[0]", "gl_FragData [ 0 ]", "gl_FragData/*x*/[0]"] {
            let tokens = lex(source);
            let [a, b, c] = solid_run::<3>(&tokens, 0).expect(source);
            assert!(tokens[a].is_word("gl_FragData"), "{source}");
            assert!(tokens[b].is_punct('['), "{source}");
            assert_eq!(tokens[c].kind, Kind::Number, "{source}");
        }
    }
}
