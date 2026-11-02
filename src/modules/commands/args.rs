#![cfg_attr(not(lua_mods), allow(dead_code))]

#[derive(Clone, Debug, PartialEq)]
pub struct Tok {
    pub text: String,
    pub start: usize,
}

pub fn tokens(raw: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut chars = raw.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let mut text = String::new();
        if c == '"' {
            chars.next();
            let mut closed = false;
            while let Some((_, c)) = chars.next() {
                match c {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\\' => match chars.next() {
                        Some((_, e @ ('"' | '\\'))) => text.push(e),
                        Some((_, e)) => {
                            return Err(format!("Invalid escape sequence '\\{e}' in quoted string"));
                        }
                        None => break,
                    },
                    c => text.push(c),
                }
            }
            if !closed {
                return Err("Unclosed quoted string".to_string());
            }
        } else {
            while let Some(&(_, c)) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                text.push(c);
                chars.next();
            }
        }
        out.push(Tok { text, start });
    }
    Ok(out)
}

pub fn rest(raw: &str, toks: &[Tok], i: usize) -> String {
    toks.get(i)
        .map_or_else(String::new, |t| raw[t.start..].trim_end().to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgKind {
    Word,
    String,
    Int,
    Number,
    Bool,
}

impl ArgKind {
    pub const NAMES: [&'static str; 5] = ["word", "string", "int", "number", "bool"];

    pub fn from_name(name: &str) -> Option<ArgKind> {
        Some(match name {
            "word" => ArgKind::Word,
            "string" => ArgKind::String,
            "int" => ArgKind::Int,
            "number" => ArgKind::Number,
            "bool" => ArgKind::Bool,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub name: &'static str,
    pub kind: ArgKind,
    pub optional: bool,
    pub suggest: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArgValue {
    Str(String),
    Int(i64),
    Num(f64),
    Bool(bool),
}

pub fn usage(args: &[Arg]) -> String {
    let parts: Vec<String> = args
        .iter()
        .map(|a| {
            if a.optional {
                format!("[{}]", a.name)
            } else {
                format!("<{}>", a.name)
            }
        })
        .collect();
    parts.join(" ")
}

pub fn bind(args: &[Arg], raw: &str) -> Result<Vec<(&'static str, ArgValue)>, String> {
    let toks = tokens(raw)?;
    let mut out = Vec::with_capacity(args.len());
    for (i, a) in args.iter().enumerate() {
        let Some(tok) = toks.get(i) else {
            if a.optional {
                return Ok(out);
            }
            return Err(format!("Missing <{}>", a.name));
        };
        let value = match a.kind {
            ArgKind::String => {
                out.push((a.name, ArgValue::Str(rest(raw, &toks, i))));
                return Ok(out);
            }
            ArgKind::Word => ArgValue::Str(tok.text.clone()),
            ArgKind::Int => ArgValue::Int(
                tok.text
                    .parse()
                    .map_err(|_| format!("<{}> is a whole number, not `{}`", a.name, tok.text))?,
            ),
            ArgKind::Number => ArgValue::Num(
                tok.text
                    .parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite())
                    .ok_or_else(|| format!("<{}> is a number, not `{}`", a.name, tok.text))?,
            ),
            ArgKind::Bool => match tok.text.as_str() {
                "true" => ArgValue::Bool(true),
                "false" => ArgValue::Bool(false),
                other => return Err(format!("<{}> is true or false, not `{other}`", a.name)),
            },
        };
        out.push((a.name, value));
    }
    if toks.len() > args.len() {
        return Err(format!(
            "Too many arguments, starting at `{}`",
            toks[args.len()].text
        ));
    }
    Ok(out)
}
