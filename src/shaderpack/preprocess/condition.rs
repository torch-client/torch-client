use super::{Defines, Error, err};

const MAX_EXPANSION: u32 = 32;

const MAX_EXPANSIONS: u32 = 4096;

pub(super) fn eval(expr: &str, defines: &Defines, line: usize) -> Result<i64, Error> {
    let tokens = tokenize(expr, line)?;
    let budget = std::cell::Cell::new(MAX_EXPANSIONS);
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        defines,
        line,
        depth: 0,
        budget: &budget,
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
    budget: &'a std::cell::Cell<u32>,
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
        let Some(left) = self.budget.get().checked_sub(1) else {
            return Err(err(
                self.line,
                format!("the condition expands more than {MAX_EXPANSIONS} macros"),
            ));
        };
        self.budget.set(left);
        let tokens = tokenize(value, self.line)?;
        let mut inner = Parser {
            tokens: &tokens,
            at: 0,
            defines: self.defines,
            line: self.line,
            depth: self.depth + 1,
            budget: self.budget,
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
