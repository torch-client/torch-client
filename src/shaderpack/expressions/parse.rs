use std::collections::HashMap;

use super::{BinaryOp, Expr, Function, Resolved, Unary};

impl Function {
    fn parse(name: &str) -> Option<(Function, usize)> {
        use Function as F;
        Some(match name {
            "sin" => (F::Sin, 1),
            "cos" => (F::Cos, 1),
            "tan" => (F::Tan, 1),
            "asin" => (F::Asin, 1),
            "acos" => (F::Acos, 1),
            "atan" => (F::Atan, 1),
            "atan2" => (F::Atan2, 2),
            "torad" | "radians" => (F::Radians, 1),
            "todeg" | "degrees" => (F::Degrees, 1),
            "min" => (F::Min, 2),
            "max" => (F::Max, 2),
            "clamp" => (F::Clamp, 3),
            "abs" => (F::Abs, 1),
            "floor" => (F::Floor, 1),
            "ceil" => (F::Ceil, 1),
            "exp" => (F::Exp, 1),
            "exp2" => (F::Exp2, 1),
            "exp10" => (F::Exp10, 1),
            "log" => (F::Log, 1),
            "log2" => (F::Log2, 1),
            "log10" => (F::Log10, 1),
            "frac" => (F::Frac, 1),
            "pow" => (F::Pow, 2),
            "sqrt" => (F::Sqrt, 1),
            "sign" | "signum" => (F::Sign, 1),
            "fmod" => (F::Fmod, 2),
            "random" => (F::Random, 0),
            "between" => (F::Between, 3),
            "equals" => (F::Equals, 3),
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Token {
    Number(f32),
    Name(String),
    Member(usize),
    Op(&'static str),
    Open,
    Close,
    Comma,
}

pub(super) fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() {
            at += 1;
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(at + 1).is_some_and(char::is_ascii_digit))
        {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_digit() || chars[at] == '.') {
                at += 1;
            }
            if matches!(chars.get(at), Some('e' | 'E')) {
                let sign = usize::from(matches!(chars.get(at + 1), Some('+' | '-')));
                if chars.get(at + 1 + sign).is_some_and(char::is_ascii_digit) {
                    at += 1 + sign;
                    while at < chars.len() && chars[at].is_ascii_digit() {
                        at += 1;
                    }
                }
            }
            let text: String = chars[start..at].iter().collect();
            if at < chars.len() && matches!(chars[at], 'f' | 'F') {
                at += 1;
            }
            tokens.push(Token::Number(
                crate::shaderpack::literal::float(&text).unwrap_or(0.0),
            ));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            tokens.push(Token::Name(chars[start..at].iter().collect()));
        } else if c == '.' {
            at += 1;
            let index = chars
                .get(at)
                .and_then(|m| "xyzw".find(*m).or_else(|| "rgba".find(*m)));
            if let Some(index) = index {
                at += 1;
                tokens.push(Token::Member(index));
            } else {
                tokens.push(Token::Op("?"));
            }
        } else {
            let two: String = chars[at..(at + 2).min(chars.len())].iter().collect();
            let op = match two.as_str() {
                "||" => Some("||"),
                "&&" => Some("&&"),
                "==" => Some("=="),
                "!=" => Some("!="),
                "<=" => Some("<="),
                ">=" => Some(">="),
                _ => None,
            };
            if let Some(op) = op {
                tokens.push(Token::Op(op));
                at += 2;
                continue;
            }
            tokens.push(match c {
                '(' => Token::Open,
                ')' => Token::Close,
                ',' => Token::Comma,
                '+' => Token::Op("+"),
                '-' => Token::Op("-"),
                '*' => Token::Op("*"),
                '/' => Token::Op("/"),
                '%' => Token::Op("%"),
                '<' => Token::Op("<"),
                '>' => Token::Op(">"),
                '!' => Token::Op("!"),
                _ => Token::Op("?"),
            });
            at += 1;
        }
    }
    tokens
}

pub(super) struct Parser<'a> {
    pub(super) tokens: Vec<Token>,
    pub(super) at: usize,
    pub(super) resolve: &'a dyn Fn(&str, Option<usize>) -> Option<Resolved>,
    pub(super) customs: &'a HashMap<&'a str, usize>,
    pub(super) smooth_slots: &'a mut usize,
}

fn precedence(op: &str) -> Option<(u8, BinaryOp)> {
    Some(match op {
        "||" => (1, BinaryOp::Or),
        "&&" => (2, BinaryOp::And),
        "==" => (3, BinaryOp::Equal),
        "!=" => (3, BinaryOp::NotEqual),
        "<" => (4, BinaryOp::Less),
        ">" => (4, BinaryOp::Greater),
        "<=" => (4, BinaryOp::LessEqual),
        ">=" => (4, BinaryOp::GreaterEqual),
        "+" => (5, BinaryOp::Add),
        "-" => (5, BinaryOp::Subtract),
        "*" => (6, BinaryOp::Multiply),
        "/" => (6, BinaryOp::Divide),
        "%" => (6, BinaryOp::Remainder),
        _ => return None,
    })
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    pub(super) fn expression(&mut self) -> Result<Expr, String> {
        self.binary(0)
    }

    fn binary(&mut self, min: u8) -> Result<Expr, String> {
        let mut left = self.unary()?;
        while let Some(Token::Op(op)) = self.peek() {
            let Some((power, operator)) = precedence(op) else {
                break;
            };
            if power <= min {
                break;
            }
            self.at += 1;
            let right = self.binary(power)?;
            left = Expr::Binary(operator, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Token::Op("-")) => {
                self.at += 1;
                Ok(Expr::Unary(Unary::Negate, Box::new(self.unary()?)))
            }
            Some(Token::Op("!")) => {
                self.at += 1;
                Ok(Expr::Unary(Unary::Not, Box::new(self.unary()?)))
            }
            _ => self.primary(),
        }
    }

    fn arguments(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if self.peek() == Some(&Token::Close) {
            self.at += 1;
            return Ok(args);
        }
        loop {
            args.push(self.expression()?);
            match self.peek() {
                Some(Token::Comma) => self.at += 1,
                Some(Token::Close) => {
                    self.at += 1;
                    return Ok(args);
                }
                other => return Err(format!("expected , or ) in a call, found {other:?}")),
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let token = self
            .tokens
            .get(self.at)
            .cloned()
            .ok_or("the expression ends early")?;
        self.at += 1;
        match token {
            Token::Number(v) => Ok(Expr::Constant(v)),
            Token::Open => {
                let inner = self.expression()?;
                match self.peek() {
                    Some(Token::Close) => {
                        self.at += 1;
                        Ok(inner)
                    }
                    other => Err(format!("expected ), found {other:?}")),
                }
            }
            Token::Name(name) => {
                if self.peek() == Some(&Token::Open) {
                    self.at += 1;
                    let args = self.arguments()?;
                    return self.call(&name, args);
                }
                let member = match self.peek() {
                    Some(Token::Member(i)) => {
                        let i = *i;
                        self.at += 1;
                        Some(i)
                    }
                    _ => None,
                };
                match name.as_str() {
                    "true" => return Ok(Expr::Constant(1.0)),
                    "false" => return Ok(Expr::Constant(0.0)),
                    "pi" => return Ok(Expr::Constant(std::f32::consts::PI)),
                    _ => {}
                }
                if member.is_none()
                    && let Some(&index) = self.customs.get(name.as_str())
                {
                    return Ok(Expr::Custom(index));
                }
                match (self.resolve)(&name, member) {
                    Some(Resolved::Input(i)) => Ok(Expr::Input(i)),
                    Some(Resolved::Constant(v)) => Ok(Expr::Constant(v)),
                    None => Err(format!("{name} is not a uniform this client supplies")),
                }
            }
            other => Err(format!("unexpected {other:?}")),
        }
    }

    fn call(&mut self, name: &str, mut args: Vec<Expr>) -> Result<Expr, String> {
        match name {
            "if" => {
                if args.len() < 3 || args.len() % 2 == 0 {
                    return Err(format!(
                        "if takes an odd number of arguments, 3 or more, not {}",
                        args.len()
                    ));
                }
                Ok(Expr::If(args))
            }
            "in" => {
                if args.len() < 2 {
                    return Err("in takes a value and at least one candidate".into());
                }
                let x = args.remove(0);
                Ok(Expr::In(Box::new(x), args))
            }
            "smooth" => {
                let literal_first = matches!(args.first(), Some(Expr::Constant(_)));
                let (value, up, down) = match (args.len(), literal_first) {
                    (1, _) => (args.remove(0), Expr::Constant(1.0), Expr::Constant(1.0)),
                    (2, true) => (args.remove(1), Expr::Constant(1.0), Expr::Constant(1.0)),
                    (2, false) => {
                        let fade = args.remove(1);
                        (args.remove(0), fade.clone(), fade)
                    }
                    (3, true) => {
                        let fade = args.remove(2);
                        (args.remove(1), fade.clone(), fade)
                    }
                    (3, false) => {
                        let down = args.remove(2);
                        let up = args.remove(1);
                        (args.remove(0), up, down)
                    }
                    (4, _) => {
                        let down = args.remove(3);
                        let up = args.remove(2);
                        (args.remove(1), up, down)
                    }
                    (n, _) => return Err(format!("smooth takes 1 to 4 arguments, not {n}")),
                };
                let slot = *self.smooth_slots;
                *self.smooth_slots += 1;
                Ok(Expr::Smooth {
                    slot,
                    value: Box::new(value),
                    up: Box::new(up),
                    down: Box::new(down),
                })
            }
            _ => {
                let (function, arity) =
                    Function::parse(name).ok_or_else(|| format!("{name}() is not a function"))?;
                if args.len() != arity {
                    return Err(format!(
                        "{name} takes {arity} arguments, not {}",
                        args.len()
                    ));
                }
                Ok(Expr::Call(function, args))
            }
        }
    }
}
