use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub(crate) struct Defines {
    map: HashMap<String, String>,
}

impl Defines {
    pub(crate) fn new() -> Defines {
        Defines::default()
    }

    pub(crate) fn define(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.map.insert(name.into(), value.into());
    }

    pub(crate) fn undefine(&mut self, name: &str) {
        self.map.remove(name);
    }

    pub(crate) fn is_defined(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.map.get(name).map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Error {
    pub(crate) line: usize,
    pub(crate) message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

fn err(line: usize, message: impl Into<String>) -> Error {
    Error {
        line,
        message: message.into(),
    }
}

struct Frame {
    active: bool,
    taken: bool,
    parent_active: bool,
    line: usize,
}

pub(crate) fn preprocess(source: &str, defines: &mut Defines) -> Result<String, Error> {
    let mut out = String::with_capacity(source.len());
    let mut stack: Vec<Frame> = Vec::new();
    let mut in_comment = false;

    for (text, line, spanned) in logical_lines(source) {
        let commented_out = in_comment;
        in_comment = scan_comments(&text, in_comment);

        let emit = |out: &mut String, content: Option<&str>| {
            if let Some(content) = content {
                out.push_str(content);
            }
            for _ in 0..spanned {
                out.push('\n');
            }
        };

        let active = stack.last().is_none_or(|f| f.active);
        let Some(directive) = (!commented_out).then(|| directive(&text)).flatten() else {
            emit(&mut out, active.then_some(text.as_ref()));
            continue;
        };
        let (name, rest) = split_directive(directive);

        match name {
            "ifdef" | "ifndef" => {
                let wanted = name == "ifdef";
                let taken = active && {
                    let ident = strip_comment(rest).trim();
                    if ident.is_empty() {
                        return Err(err(line, format!("#{name} names nothing")));
                    }
                    defines.is_defined(ident) == wanted
                };
                stack.push(Frame {
                    active: taken,
                    taken,
                    parent_active: active,
                    line,
                });
                emit(&mut out, None);
            }
            "if" => {
                let taken = active && eval(strip_comment(rest), defines, line)? != 0;
                stack.push(Frame {
                    active: taken,
                    taken,
                    parent_active: active,
                    line,
                });
                emit(&mut out, None);
            }
            "elif" | "else" => {
                let Some(frame) = stack.last_mut() else {
                    return Err(err(line, format!("#{name} with no #if")));
                };
                let take = frame.parent_active
                    && !frame.taken
                    && (name == "else" || eval(strip_comment(rest), defines, line)? != 0);
                frame.active = take;
                frame.taken |= take;
                emit(&mut out, None);
            }
            "endif" => {
                if stack.pop().is_none() {
                    return Err(err(line, "#endif with no #if"));
                }
                emit(&mut out, None);
            }
            "define" if active => {
                let (ident, value) = split_directive(strip_comment(rest).trim());
                if ident.is_empty() {
                    return Err(err(line, "#define names nothing"));
                }
                defines.define(ident, value.trim());
                emit(&mut out, Some(text.as_ref()));
            }
            "undef" if active => {
                defines.undefine(strip_comment(rest).trim());
                emit(&mut out, Some(text.as_ref()));
            }
            _ => emit(&mut out, active.then_some(text.as_ref())),
        }
    }

    match stack.first() {
        Some(frame) => Err(err(frame.line, "#if is never closed")),
        None => Ok(out),
    }
}

fn directive(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('#')
}

fn split_directive(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    let end = text
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(text.len());
    (&text[..end], &text[end..])
}

fn strip_comment(text: &str) -> &str {
    let line = text.find("//").unwrap_or(text.len());
    let block = text.find("/*").unwrap_or(text.len());
    &text[..line.min(block)]
}

fn scan_comments(line: &str, mut inside: bool) -> bool {
    let bytes = line.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let pair = (bytes[at], bytes.get(at + 1).copied());
        match (inside, pair) {
            (true, (b'*', Some(b'/'))) => {
                inside = false;
                at += 2;
            }
            (false, (b'/', Some(b'*'))) => {
                inside = true;
                at += 2;
            }
            (false, (b'/', Some(b'/'))) => return false,
            _ => at += 1,
        }
    }
    inside
}

fn logical_lines(source: &str) -> impl Iterator<Item = (Cow<'_, str>, usize, usize)> {
    let mut lines = source.lines().enumerate().peekable();
    std::iter::from_fn(move || {
        let (index, first) = lines.next()?;
        let start = index + 1;

        let Some(head) = continued(first) else {
            return Some((Cow::Borrowed(first), start, 1));
        };

        let mut joined = String::from(head);
        let mut spanned = 1;
        while let Some((_, next)) = lines.next() {
            spanned += 1;
            match continued(next) {
                Some(head) => joined.push_str(head),
                None => {
                    joined.push_str(next);
                    break;
                }
            }
        }
        Some((Cow::Owned(joined), start, spanned))
    })
}

fn continued(line: &str) -> Option<&str> {
    line.strip_suffix('\\')
}

const MAX_EXPANSION: u32 = 32;

fn eval(expr: &str, defines: &Defines, line: usize) -> Result<i64, Error> {
    let tokens = tokenize(expr, line)?;
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        defines,
        line,
        depth: 0,
    };
    let value = parser.expression(0)?;
    if parser.at != parser.tokens.len() {
        return Err(err(line, format!("trailing text in condition: {expr:?}")));
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token<'a> {
    Number(i64),
    Ident(&'a str),
    Op(&'a str),
}

const OPERATORS: &[&str] = &[
    "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "(", ")", "+", "-", "*", "/", "%", "<", ">",
    "!", "~", "&", "|", "^",
];

fn tokenize(expr: &str, line: usize) -> Result<Vec<Token<'_>>, Error> {
    let mut out = Vec::new();
    let bytes = expr.as_bytes();
    let mut at = 0;

    while at < bytes.len() {
        let c = bytes[at];
        if c.is_ascii_whitespace() {
            at += 1;
        } else if c.is_ascii_digit() {
            let start = at;
            let radix = if expr[at..].starts_with("0x") || expr[at..].starts_with("0X") {
                at += 2;
                16
            } else {
                10
            };
            let digits = at;
            while at < bytes.len() && (bytes[at] as char).is_digit(radix) {
                at += 1;
            }
            let value = i64::from_str_radix(&expr[digits..at], radix).map_err(|_| {
                err(
                    line,
                    format!("cannot read the number {:?}", &expr[start..at]),
                )
            })?;
            while at < bytes.len() && bytes[at].is_ascii_alphabetic() {
                at += 1;
            }
            out.push(Token::Number(value));
        } else if c == b'_' || c.is_ascii_alphabetic() {
            let start = at;
            while at < bytes.len() && (bytes[at] == b'_' || bytes[at].is_ascii_alphanumeric()) {
                at += 1;
            }
            out.push(Token::Ident(&expr[start..at]));
        } else {
            let op = OPERATORS
                .iter()
                .find(|op| expr[at..].starts_with(**op))
                .ok_or_else(|| err(line, format!("unexpected {:?} in condition", c as char)))?;
            at += op.len();
            out.push(Token::Op(op));
        }
    }
    Ok(out)
}

struct Parser<'a> {
    tokens: &'a [Token<'a>],
    at: usize,
    defines: &'a Defines,
    line: usize,
    depth: u32,
}

fn precedence(op: &str) -> Option<u8> {
    Some(match op {
        "||" => 1,
        "&&" => 2,
        "|" => 3,
        "^" => 4,
        "&" => 5,
        "==" | "!=" => 6,
        "<" | ">" | "<=" | ">=" => 7,
        "<<" | ">>" => 8,
        "+" | "-" => 9,
        "*" | "/" | "%" => 10,
        _ => return None,
    })
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token<'a>> {
        self.tokens.get(self.at)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.peek() == Some(&Token::Op(op)) {
            self.at += 1;
            return true;
        }
        false
    }

    fn expression(&mut self, min: u8) -> Result<i64, Error> {
        let mut left = self.unary()?;
        while let Some(Token::Op(op)) = self.peek() {
            let Some(power) = precedence(op) else { break };
            if power < min {
                break;
            }
            self.at += 1;
            let right = self.expression(power + 1)?;
            left = apply(op, left, right);
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<i64, Error> {
        match self.peek() {
            Some(Token::Op("!")) => {
                self.at += 1;
                Ok((self.unary()? == 0) as i64)
            }
            Some(Token::Op("~")) => {
                self.at += 1;
                Ok(!self.unary()?)
            }
            Some(Token::Op("-")) => {
                self.at += 1;
                Ok(self.unary()?.wrapping_neg())
            }
            Some(Token::Op("+")) => {
                self.at += 1;
                self.unary()
            }
            Some(Token::Op("(")) => {
                self.at += 1;
                let value = self.expression(0)?;
                if !self.eat_op(")") {
                    return Err(err(self.line, "a ( in the condition is never closed"));
                }
                Ok(value)
            }
            Some(Token::Number(value)) => {
                self.at += 1;
                Ok(*value)
            }
            Some(Token::Ident("defined")) => {
                self.at += 1;
                let parenthesised = self.eat_op("(");
                let Some(Token::Ident(name)) = self.peek() else {
                    return Err(err(self.line, "defined names nothing"));
                };
                self.at += 1;
                if parenthesised && !self.eat_op(")") {
                    return Err(err(self.line, "defined( is never closed"));
                }
                Ok(self.defines.is_defined(name) as i64)
            }
            Some(Token::Ident(name)) => {
                self.at += 1;
                self.substitute(name)
            }
            _ => Err(err(self.line, "the condition ends early")),
        }
    }

    fn substitute(&mut self, name: &str) -> Result<i64, Error> {
        let Some(value) = self.defines.get(name) else {
            return Ok(0);
        };
        let value = value.trim();
        if value.is_empty() {
            return Ok(1);
        }
        if self.depth >= MAX_EXPANSION {
            return Err(err(self.line, format!("{name} expands into itself")));
        }
        let tokens = tokenize(value, self.line)?;
        let mut inner = Parser {
            tokens: &tokens,
            at: 0,
            defines: self.defines,
            line: self.line,
            depth: self.depth + 1,
        };
        let result = inner.expression(0)?;
        if inner.at != inner.tokens.len() {
            return Err(err(
                self.line,
                format!("{name} is not a number in a condition: {value:?}"),
            ));
        }
        Ok(result)
    }
}

fn apply(op: &str, left: i64, right: i64) -> i64 {
    match op {
        "||" => (left != 0 || right != 0) as i64,
        "&&" => (left != 0 && right != 0) as i64,
        "|" => left | right,
        "^" => left ^ right,
        "&" => left & right,
        "==" => (left == right) as i64,
        "!=" => (left != right) as i64,
        "<" => (left < right) as i64,
        ">" => (left > right) as i64,
        "<=" => (left <= right) as i64,
        ">=" => (left >= right) as i64,
        "<<" => left.wrapping_shl(right as u32),
        ">>" => left.wrapping_shr(right as u32),
        "+" => left.wrapping_add(right),
        "-" => left.wrapping_sub(right),
        "*" => left.wrapping_mul(right),
        "/" => left.checked_div(right).unwrap_or(0),
        "%" => left.checked_rem(right).unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str, defines: &[(&str, &str)]) -> String {
        let mut table = Defines::new();
        for (name, value) in defines {
            table.define(*name, *value);
        }
        preprocess(source, &mut table).expect("preprocessing failed")
    }

    #[test]
    fn ifdef_and_ifndef_select_branches_and_keep_line_numbers() {
        let source = "#ifdef ON\nyes\n#endif\n#ifndef ON\nno\n#endif\n";
        assert_eq!(run(source, &[("ON", "")]), "\nyes\n\n\n\n\n");
    }

    #[test]
    fn if_compares_numbers_through_the_table() {
        let source = "#if MC_VERSION >= 12104\nnew\n#endif\n";
        assert_eq!(run(source, &[("MC_VERSION", "12104")]), "\nnew\n\n");
        assert_eq!(run(source, &[("MC_VERSION", "12006")]), "\n\n\n");
    }

    #[test]
    fn an_elif_chain_takes_exactly_one_branch() {
        let source = "#if N == 128\nsmall\n#elif N == 192\nmid\n#else\nbig\n#endif\n";
        assert_eq!(run(source, &[("N", "192")]), "\n\n\nmid\n\n\n\n");
        assert_eq!(run(source, &[("N", "512")]), "\n\n\n\n\nbig\n\n");
    }

    #[test]
    fn a_nested_block_stays_dead_inside_a_dead_one() {
        let source = "#ifdef OFF\n#if 1\nx\n#elif 1\ny\n#endif\n#endif\n";
        assert_eq!(run(source, &[]), "\n\n\n\n\n\n\n");
    }

    #[test]
    fn a_trailing_backslash_joins_the_next_line() {
        let out = run("a=1 + \\\n      2\n", &[]);
        assert_eq!(out, "a=1 +       2\n\n");
    }

    #[test]
    fn conditions_follow_c() {
        let bound = "#if 1 + 2 * 3 == 7 && (0 || 1)\nok\n#endif\n";
        assert_eq!(run(bound, &[]), "\nok\n\n");

        let names = "#if defined(A) && defined B && !defined(C) && BARE && !UNKNOWN\nok\n#endif\n";
        assert_eq!(
            run(names, &[("A", ""), ("B", ""), ("BARE", "")]),
            "\nok\n\n"
        );
    }

    #[test]
    fn comments_yield_no_values_and_no_directives() {
        let source = "#define DRM 1 //[1]\n/*\n#endif\n*/\n#if DRM != 1\nbad\n#endif\n";
        assert_eq!(
            run(source, &[]),
            "#define DRM 1 //[1]\n/*\n#endif\n*/\n\n\n\n"
        );
    }

    #[test]
    fn unknown_directives_pass_through() {
        let source = "#version 130\n#extension GL_ARB_x : enable\n#include \"/lib/a.glsl\"\n";
        assert_eq!(run(source, &[]), source);
    }

    #[test]
    fn malformed_input_is_an_error() {
        let mut table = Defines::new();
        assert_eq!(
            preprocess("a\n#ifdef X\nb\n", &mut table).unwrap_err().line,
            2
        );
        assert!(preprocess("#endif\n", &mut table).is_err());

        table.define("SHADOW_DISTANCE", "192.0");
        assert!(preprocess("#if SHADOW_DISTANCE > 1\nx\n#endif\n", &mut table).is_err());
    }
}
