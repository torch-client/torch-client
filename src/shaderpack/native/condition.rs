use std::collections::BTreeSet;

use super::options::{Kind, Options, Value, Values};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Condition {
    source: String,
    expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Flag(String),
    Choice { id: String, index: u32, equal: bool },
    Int { id: String, value: i32, equal: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Error {
    pub(crate) offset: usize,
    pub(crate) message: String,
    pub(crate) hint: Option<String>,
}

impl Error {
    fn new(offset: usize, message: impl Into<String>) -> Error {
        Error {
            offset,
            message: message.into(),
            hint: None,
        }
    }

    fn hint(mut self, hint: impl Into<String>) -> Error {
        self.hint = Some(hint.into());
        self
    }
}

impl Condition {
    pub(crate) fn parse(source: &str, options: &Options) -> Result<Condition, Vec<Error>> {
        let tokens = lex(source).map_err(|e| vec![e])?;
        let mut parser = Parser {
            tokens: &tokens,
            at: 0,
            options,
            errors: Vec::new(),
            end: source.len(),
        };
        let expr = parser.or().map_err(|e| {
            let mut errors = std::mem::take(&mut parser.errors);
            errors.push(e);
            errors
        })?;
        if let Some(token) = parser.peek() {
            let trailing = Error::new(
                token.at,
                format!(
                    "expected `&&`, `||` or the end, found {}",
                    token.kind.describe()
                ),
            );
            parser.errors.push(trailing);
        }
        if parser.errors.is_empty() {
            Ok(Condition {
                source: source.to_owned(),
                expr,
            })
        } else {
            Err(parser.errors)
        }
    }

    #[allow(
        dead_code,
        reason = "phase 5 of docs/SHADER_FORMAT.md, the problem overlay, names a condition by its text"
    )]
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn eval(&self, values: &Values) -> bool {
        self.expr.eval(values)
    }

    pub(crate) fn mentions(&self) -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        self.expr.mentions(&mut out);
        out
    }
}

impl Expr {
    fn eval(&self, values: &Values) -> bool {
        match self {
            Expr::Or(a, b) => a.eval(values) || b.eval(values),
            Expr::And(a, b) => a.eval(values) && b.eval(values),
            Expr::Not(a) => !a.eval(values),
            Expr::Flag(id) => values.get(id) == Some(Value::Bool(true)),
            Expr::Choice { id, index, equal } => {
                (values.get(id) == Some(Value::Choice(*index))) == *equal
            }
            Expr::Int { id, value, equal } => {
                (values.get(id) == Some(Value::Int(*value))) == *equal
            }
        }
    }

    fn mentions<'a>(&'a self, out: &mut BTreeSet<&'a str>) {
        match self {
            Expr::Or(a, b) | Expr::And(a, b) => {
                a.mentions(out);
                b.mentions(out);
            }
            Expr::Not(a) => a.mentions(out),
            Expr::Flag(id) | Expr::Choice { id, .. } | Expr::Int { id, .. } => {
                out.insert(id);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum TokenKind {
    Id(String),
    Str(String),
    Int(i64),
    OrOr,
    AndAnd,
    Bang,
    Open,
    Close,
    EqEq,
    NotEq,
}

impl TokenKind {
    fn describe(&self) -> String {
        match self {
            TokenKind::Id(id) => format!("`{id}`"),
            TokenKind::Str(s) => format!("\"{s}\""),
            TokenKind::Int(i) => format!("`{i}`"),
            TokenKind::OrOr => "`||`".to_owned(),
            TokenKind::AndAnd => "`&&`".to_owned(),
            TokenKind::Bang => "`!`".to_owned(),
            TokenKind::Open => "`(`".to_owned(),
            TokenKind::Close => "`)`".to_owned(),
            TokenKind::EqEq => "`==`".to_owned(),
            TokenKind::NotEq => "`!=`".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    at: usize,
}

fn lex(source: &str) -> Result<Vec<Token>, Error> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let at = i;
        let two = bytes.get(i..i + 2);
        let kind = match bytes[i] {
            b' ' | b'\t' | b'\r' | b'\n' => {
                i += 1;
                continue;
            }
            _ if two == Some(b"||".as_slice()) => TokenKind::OrOr,
            _ if two == Some(b"&&".as_slice()) => TokenKind::AndAnd,
            _ if two == Some(b"==".as_slice()) => TokenKind::EqEq,
            _ if two == Some(b"!=".as_slice()) => TokenKind::NotEq,
            b'!' => TokenKind::Bang,
            b'(' => TokenKind::Open,
            b')' => TokenKind::Close,
            quote @ (b'"' | b'\'') => {
                let rest = &source[i + 1..];
                let Some(len) = rest.find(quote as char) else {
                    return Err(Error::new(at, "this quoted value has no closing quote"));
                };
                i += len + 2;
                out.push(Token {
                    kind: TokenKind::Str(rest[..len].to_owned()),
                    at,
                });
                continue;
            }
            b'-' | b'0'..=b'9' => {
                let start = i;
                i += 1;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_')
                {
                    i += 1;
                }
                let text = &source[start..i];
                let Ok(value) = text.parse::<i64>() else {
                    return Err(Error::new(at, format!("`{text}` is not an integer"))
                        .hint("only int options are compared with a number"));
                };
                out.push(Token {
                    kind: TokenKind::Int(value),
                    at,
                });
                continue;
            }
            b if b.is_ascii_alphabetic() || b == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                out.push(Token {
                    kind: TokenKind::Id(source[start..i].to_owned()),
                    at,
                });
                continue;
            }
            _ => {
                let c = source[i..].chars().next().unwrap_or('?');
                return Err(Error::new(
                    at,
                    format!("`{c}` has no meaning in a condition"),
                ));
            }
        };
        i += if matches!(kind, TokenKind::Bang | TokenKind::Open | TokenKind::Close) {
            1
        } else {
            2
        };
        out.push(Token { kind, at });
    }
    Ok(out)
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    options: &'a Options,
    errors: Vec<Error>,
    end: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek().is_some_and(|t| &t.kind == kind) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn or(&mut self) -> Result<Expr, Error> {
        let mut left = self.and()?;
        while self.eat(&TokenKind::OrOr) {
            left = Expr::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, Error> {
        let mut left = self.unary()?;
        while self.eat(&TokenKind::AndAnd) {
            left = Expr::And(Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if self.eat(&TokenKind::Bang) {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.eat(&TokenKind::Open) {
            let inner = self.or()?;
            if !self.eat(&TokenKind::Close) {
                return Err(self.expected("`)`"));
            }
            return Ok(inner);
        }
        self.atom()
    }

    fn expected(&self, what: &str) -> Error {
        match self.peek() {
            Some(token) => Error::new(
                token.at,
                format!("expected {what}, found {}", token.kind.describe()),
            ),
            None => Error::new(self.end, format!("expected {what}, found the end")),
        }
    }

    fn atom(&mut self) -> Result<Expr, Error> {
        let Some(Token {
            kind: TokenKind::Id(id),
            at,
        }) = self.peek().cloned()
        else {
            return Err(self.expected("an option id, `!` or `(`"));
        };
        self.at += 1;
        let equal = if self.eat(&TokenKind::EqEq) {
            true
        } else if self.eat(&TokenKind::NotEq) {
            false
        } else {
            return Ok(self.flag(id, at));
        };
        let Some(value) = self.peek().cloned() else {
            return Err(self.expected("a value"));
        };
        if !matches!(value.kind, TokenKind::Str(_) | TokenKind::Int(_)) {
            return Err(self.expected("a quoted value or an integer"));
        }
        self.at += 1;
        Ok(self.compare(id, at, equal, value))
    }

    fn flag(&mut self, id: String, at: usize) -> Expr {
        match self.options.get(&id).map(|decl| &decl.kind) {
            None => self.errors.push(unknown(&id, at)),
            Some(Kind::Bool) => {}
            Some(kind) => {
                let hint = match kind {
                    Kind::Choice { values } => format!(
                        "compare it: {id} == '{}'",
                        values.first().map_or("", String::as_str)
                    ),
                    Kind::Int { min, .. } => format!("compare it: {id} == {min}"),
                    _ => "a float option cannot be used in a condition".to_owned(),
                };
                self.errors.push(
                    Error::new(
                        at,
                        format!(
                            "`{id}` is a {} option; a bare id must be a bool option",
                            kind.type_name()
                        ),
                    )
                    .hint(hint),
                );
            }
        }
        Expr::Flag(id)
    }

    fn compare(&mut self, id: String, at: usize, equal: bool, value: Token) -> Expr {
        let stand_in = Expr::Flag(id.clone());
        let Some(decl) = self.options.get(&id) else {
            self.errors.push(unknown(&id, at));
            return stand_in;
        };
        match (&decl.kind, value.kind) {
            (Kind::Choice { values }, TokenKind::Str(s)) => {
                match values.iter().position(|v| *v == s) {
                    Some(index) => Expr::Choice {
                        id,
                        index: index as u32,
                        equal,
                    },
                    None => {
                        self.errors.push(
                            Error::new(value.at, format!("`{s}` is not a value of `{id}`"))
                                .hint(format!("one of: {}", values.join(", "))),
                        );
                        stand_in
                    }
                }
            }
            (Kind::Int { .. }, TokenKind::Int(n)) => match i32::try_from(n) {
                Ok(value) => Expr::Int { id, value, equal },
                Err(_) => {
                    self.errors.push(Error::new(
                        value.at,
                        format!("{n} does not fit an int option"),
                    ));
                    stand_in
                }
            },
            (Kind::Choice { .. }, _) => {
                self.errors.push(Error::new(
                    value.at,
                    format!("`{id}` is a choice; compare it with a quoted value"),
                ));
                stand_in
            }
            (Kind::Int { .. }, _) => {
                self.errors.push(Error::new(
                    value.at,
                    format!("`{id}` is an int; compare it with a number"),
                ));
                stand_in
            }
            (Kind::Bool, _) => {
                self.errors.push(
                    Error::new(
                        at,
                        format!("`{id}` is a bool option and is tested by its id alone"),
                    )
                    .hint(if equal { id.clone() } else { format!("!{id}") }),
                );
                stand_in
            }
            (Kind::Float { .. }, _) => {
                self.errors.push(Error::new(
                    at,
                    format!("`{id}` is a float option; only choice and int options compare"),
                ));
                stand_in
            }
        }
    }
}

fn unknown(id: &str, at: usize) -> Error {
    Error::new(at, format!("no option `{id}`"))
}

#[cfg(test)]
mod tests {
    use super::super::options::tests::sample;
    use super::*;

    fn parse(source: &str) -> Result<Condition, Vec<Error>> {
        Condition::parse(source, &sample())
    }

    #[test]
    fn precedence_and_evaluation() {
        let options = sample();
        let mut values = Values::defaults(&options);
        let c = parse("!shadows || quality == 'high' && samples != 4").unwrap();
        assert!(!c.eval(&values));
        values.set(&options, "quality", Value::Choice(2)).unwrap();
        values.set(&options, "samples", Value::Int(8)).unwrap();
        assert!(c.eval(&values));
        let c = parse("(shadows || shadows) && quality != \"low\"").unwrap();
        assert!(c.eval(&values));
        let c = parse("samples == -3").unwrap();
        assert!(!c.eval(&values));
    }

    #[test]
    fn mentions_lists_each_option_once() {
        let c = parse("shadows && (samples == 2 || !shadows) && quality == 'low'").unwrap();
        assert_eq!(
            c.mentions().into_iter().collect::<Vec<_>>(),
            ["quality", "samples", "shadows"]
        );
    }

    #[test]
    fn type_errors_are_all_reported_with_offsets() {
        let errors = parse("quality && exposure == 1 && shadows == 'x' && nope").unwrap_err();
        let offsets: Vec<usize> = errors.iter().map(|e| e.offset).collect();
        assert_eq!(offsets, [0, 11, 28, 46], "{errors:?}");
    }

    #[test]
    fn comparisons_need_the_right_value() {
        assert_eq!(parse("quality == 2").unwrap_err()[0].offset, 11);
        assert_eq!(parse("quality == 'ultra'").unwrap_err()[0].offset, 11);
        assert_eq!(parse("samples == 'four'").unwrap_err()[0].offset, 11);
        assert_eq!(parse("samples == 99999999999").unwrap_err()[0].offset, 11);
    }

    #[test]
    fn syntax_errors_point_at_the_token() {
        assert_eq!(parse("shadows &&").unwrap_err()[0].offset, 10);
        assert_eq!(parse("(shadows").unwrap_err()[0].offset, 8);
        assert_eq!(parse("shadows shadows").unwrap_err()[0].offset, 8);
        assert_eq!(parse("shadows & x").unwrap_err()[0].offset, 8);
        assert_eq!(parse("quality == 'low").unwrap_err()[0].offset, 11);
        assert_eq!(parse("samples == 1.5").unwrap_err()[0].offset, 11);
        assert_eq!(parse("").unwrap_err()[0].offset, 0);
    }
}
