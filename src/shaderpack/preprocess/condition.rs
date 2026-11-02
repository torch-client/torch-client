use super::{Defines, Error, err};

const MAX_EXPANSION: u32 = 32;

const MAX_EXPANSIONS: u32 = 4096;

pub(super) fn eval(expr: &str, defines: &Defines, line: usize) -> Result<i64, Error> {
    let tokens = tokenize(expr);
    let budget = std::cell::Cell::new(MAX_EXPANSIONS);
    let problem = std::cell::RefCell::new(None);
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        defines,
        line,
        depth: 0,
        budget: &budget,
        problem: &problem,
    };
    let value = parser.expression(0)?;
    if parser.at != parser.tokens.len() {
        parser.note("trailing text");
    }
    if let Some(problem) = problem.into_inner() {
        crate::log_debug!(
            "shaders",
            "line {line}: {problem} in condition {expr:?}; read as {value}, as JCPP reads it"
        );
    }
    Ok(value)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token<'a> {
    Number(i64),
    Ident(&'a str),
    Op(&'a str),
    Other(&'a str),
}

const OPERATORS: &[&str] = &[
    "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "(", ")", "+", "-", "*", "/", "%", "<", ">",
    "!", "~", "&", "|", "^",
];

fn tokenize(expr: &str) -> Vec<Token<'_>> {
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
            let Ok(value) = i64::from_str_radix(&expr[digits..at], radix) else {
                out.push(Token::Other(&expr[start..at]));
                continue;
            };
            let mut value = value;
            if radix == 10 {
                if bytes.get(at) == Some(&b'.') {
                    at += 1;
                    while at < bytes.len() && bytes[at].is_ascii_digit() {
                        at += 1;
                    }
                }
                if matches!(bytes.get(at), Some(b'e' | b'E')) {
                    let sign = at + 1;
                    let mut end = sign + usize::from(matches!(bytes.get(sign), Some(b'+' | b'-')));
                    let digits = end;
                    while end < bytes.len() && bytes[end].is_ascii_digit() {
                        end += 1;
                    }
                    if end > digits {
                        let exponent: i32 =
                            expr[sign..end].trim_start_matches('+').parse().unwrap_or(0);
                        let scale = 10i64.saturating_pow(exponent.unsigned_abs());
                        value = if exponent < 0 {
                            value / scale
                        } else {
                            value.saturating_mul(scale)
                        };
                        at = end;
                    }
                }
            }
            while at < bytes.len() && bytes[at].is_ascii_alphanumeric() {
                at += 1;
            }
            out.push(Token::Number(value));
        } else if c == b'_' || c.is_ascii_alphabetic() {
            let start = at;
            while at < bytes.len() && (bytes[at] == b'_' || bytes[at].is_ascii_alphanumeric()) {
                at += 1;
            }
            out.push(Token::Ident(&expr[start..at]));
        } else if let Some(op) = OPERATORS.iter().find(|op| expr[at..].starts_with(**op)) {
            at += op.len();
            out.push(Token::Op(op));
        } else {
            let len = expr[at..].chars().next().map_or(1, char::len_utf8);
            out.push(Token::Other(&expr[at..at + len]));
            at += len;
        }
    }
    out
}

struct Parser<'a> {
    tokens: &'a [Token<'a>],
    at: usize,
    defines: &'a Defines,
    line: usize,
    depth: u32,
    budget: &'a std::cell::Cell<u32>,
    problem: &'a std::cell::RefCell<Option<String>>,
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

    fn note(&self, problem: impl Into<String>) {
        self.problem
            .borrow_mut()
            .get_or_insert_with(|| problem.into());
    }

    fn expression(&mut self, min: u8) -> Result<i64, Error> {
        let Some(mut left) = self.unary()? else {
            return Ok(0);
        };
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

    fn unary(&mut self) -> Result<Option<i64>, Error> {
        Ok(Some(match self.peek() {
            Some(Token::Op("!")) => {
                self.at += 1;
                (self.unary()?.unwrap_or(0) == 0) as i64
            }
            Some(Token::Op("~")) => {
                self.at += 1;
                !self.unary()?.unwrap_or(0)
            }
            Some(Token::Op("-")) => {
                self.at += 1;
                self.unary()?.unwrap_or(0).wrapping_neg()
            }
            Some(Token::Op("+")) => {
                self.at += 1;
                self.unary()?.unwrap_or(0)
            }
            Some(Token::Op("(")) => {
                self.at += 1;
                let value = self.expression(0)?;
                if !self.eat_op(")") {
                    self.note("a ( never closed");
                    return Ok(None);
                }
                value
            }
            Some(Token::Number(value)) => {
                self.at += 1;
                *value
            }
            Some(Token::Ident("defined")) => {
                self.at += 1;
                let parenthesised = self.eat_op("(");
                let Some(Token::Ident(name)) = self.peek() else {
                    self.note("defined naming nothing");
                    return Ok(None);
                };
                self.at += 1;
                if parenthesised && !self.eat_op(")") {
                    self.note("a defined( never closed");
                    return Ok(None);
                }
                self.defines.is_defined(name) as i64
            }
            Some(Token::Ident(name)) => {
                self.at += 1;
                self.substitute(name)?
            }
            Some(Token::Op(text) | Token::Other(text)) => {
                self.note(format!("a stray {text:?}"));
                return Ok(None);
            }
            None => {
                self.note("an early end");
                return Ok(None);
            }
        }))
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
            self.note(format!("{name} expanding into itself"));
            return Ok(0);
        }
        let Some(left) = self.budget.get().checked_sub(1) else {
            return Err(err(
                self.line,
                format!("the condition expands more than {MAX_EXPANSIONS} macros"),
            ));
        };
        self.budget.set(left);
        let tokens = tokenize(value);
        let mut inner = Parser {
            tokens: &tokens,
            at: 0,
            defines: self.defines,
            line: self.line,
            depth: self.depth + 1,
            budget: self.budget,
            problem: self.problem,
        };
        let result = inner.expression(0)?;
        if inner.at != inner.tokens.len() {
            self.note(format!("{name} not being a number ({value:?})"));
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
