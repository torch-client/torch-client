use azalea_protocol::packets::game::c_commands::{BrigadierParser, BrigadierString};

use super::suggestion::Range;
use super::tree::{CommandTree, NodeKind};

#[derive(Clone, Debug)]
pub struct Context {
    pub root: usize,
    pub nodes: Vec<(usize, Range)>,
    pub arguments: Vec<(String, Range)>,
    pub range: Range,
}

impl Context {
    fn new(root: usize, start: usize) -> Context {
        Context {
            root,
            nodes: Vec::new(),
            arguments: Vec::new(),
            range: Range::at(start),
        }
    }

    fn with_node(&mut self, node: usize, range: Range) {
        self.nodes.push((node, range));
        self.range = Range::encompassing(self.range, range);
    }
}

#[derive(Clone, Debug)]
pub struct Parse {
    pub chain: Vec<Context>,
    pub cursor: usize,
    pub errors: Vec<(usize, ParseError, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    LiteralIncorrect(String),
    Argument(String),
    ExpectedSeparator,
}

impl ParseError {
    fn message(&self) -> String {
        match self {
            ParseError::LiteralIncorrect(expected) => format!("Expected literal {expected}"),
            ParseError::Argument(message) => message.clone(),
            ParseError::ExpectedSeparator => {
                "Expected whitespace to end one argument, but found trailing data".to_string()
            }
        }
    }
}

pub(super) fn error_context(input: &[char], cursor: usize) -> String {
    let cursor = cursor.min(input.len());
    let from = cursor.saturating_sub(10);
    let mut out = String::new();
    if cursor > 10 {
        out.push_str("...");
    }
    out.extend(&input[from..cursor]);
    out.push_str("<--[HERE]");
    out
}

pub(super) fn exception_message(error: &ParseError, input: &[char], cursor: usize) -> String {
    format!(
        "{} at position {}: {}",
        error.message(),
        cursor,
        error_context(input, cursor)
    )
}

impl Parse {
    fn single(context: Context, cursor: usize) -> Parse {
        Parse {
            chain: vec![context],
            cursor,
            errors: Vec::new(),
        }
    }

    pub(super) fn can_read(&self, input: &[char]) -> bool {
        self.cursor < input.len()
    }

    pub(super) fn last(&self) -> &Context {
        self.chain.last().expect("a parse always has one context")
    }
}

pub fn parse(tree: &CommandTree, input: &[char], start: usize) -> Parse {
    parse_as(tree, input, start, true)
}

pub(super) fn parse_as(
    tree: &CommandTree,
    input: &[char],
    start: usize,
    allow_restricted: bool,
) -> Parse {
    parse_nodes(
        tree,
        tree.root,
        input,
        start,
        Context::new(tree.root, start),
        allow_restricted,
    )
}

fn parse_nodes(
    tree: &CommandTree,
    node: usize,
    input: &[char],
    cursor: usize,
    context_so_far: Context,
    allow_restricted: bool,
) -> Parse {
    let mut errors: Vec<(usize, ParseError, usize)> = Vec::new();
    let mut potentials: Vec<Parse> = Vec::new();

    for child in tree.relevant_children(node, input, cursor) {
        if !allow_restricted && tree.node(child).restricted {
            continue;
        }
        let mut context = context_so_far.clone();
        let mut reader = cursor;

        if let Err(error) = parse_child(tree, child, input, &mut reader, &mut context) {
            errors.push((child, error, reader));
            continue;
        }
        if input.get(reader).is_some_and(|&c| c != ' ') {
            errors.push((child, ParseError::ExpectedSeparator, reader));
            continue;
        }

        let need = if tree.node(child).redirect.is_none() {
            2
        } else {
            1
        };
        if reader + need <= input.len() {
            reader += 1;
            match tree.node(child).redirect {
                Some(redirect) => {
                    let child_context = Context::new(redirect, reader);
                    let sub = parse_nodes(
                        tree,
                        redirect,
                        input,
                        reader,
                        child_context,
                        allow_restricted,
                    );
                    let mut chain = vec![context];
                    chain.extend(sub.chain);
                    return Parse {
                        chain,
                        cursor: sub.cursor,
                        errors: sub.errors,
                    };
                }
                None => potentials.push(parse_nodes(
                    tree,
                    child,
                    input,
                    reader,
                    context,
                    allow_restricted,
                )),
            }
        } else {
            potentials.push(Parse::single(context, reader));
        }
    }

    if let Some(best) = potentials
        .into_iter()
        .min_by_key(|p| (p.can_read(input), !p.errors.is_empty()))
    {
        return best;
    }

    Parse {
        chain: vec![context_so_far],
        cursor,
        errors,
    }
}

fn parse_child(
    tree: &CommandTree,
    child: usize,
    input: &[char],
    cursor: &mut usize,
    context: &mut Context,
) -> Result<(), ParseError> {
    let start = *cursor;
    match &tree.node(child).kind {
        NodeKind::Root => Ok(()),
        NodeKind::Literal(name) => {
            let chars: Vec<char> = name.chars().collect();
            let end = start + chars.len();
            let matched = end <= input.len() && input[start..end] == chars[..];
            if matched && matches!(input.get(end), None | Some(' ')) {
                *cursor = end;
                context.with_node(child, Range::between(start, end));
                Ok(())
            } else {
                Err(ParseError::LiteralIncorrect(name.clone()))
            }
        }
        NodeKind::Argument { name, parser, .. } => {
            parse_argument(parser, input, cursor).map_err(ParseError::Argument)?;
            let range = Range::between(start, *cursor);
            context.arguments.push((name.clone(), range));
            context.with_node(child, range);
            Ok(())
        }
    }
}

pub(super) fn read_while<'a>(
    input: &'a [char],
    cursor: &mut usize,
    pred: impl Fn(char) -> bool,
) -> &'a [char] {
    let start = *cursor;
    while input.get(*cursor).is_some_and(|&c| pred(c)) {
        *cursor += 1;
    }
    &input[start..*cursor]
}

fn read_unquoted(input: &[char], cursor: &mut usize) -> String {
    read_while(input, cursor, is_unquoted_char).iter().collect()
}

pub(super) fn is_unquoted_char(c: char) -> bool {
    c.is_ascii_digit() || c.is_ascii_alphabetic() || matches!(c, '_' | '-' | '.' | '+')
}

fn read_token<'a>(input: &'a [char], cursor: &mut usize) -> &'a [char] {
    read_while(input, cursor, |c| c != ' ')
}

fn read_quotable(input: &[char], cursor: &mut usize) -> Result<String, String> {
    let Some(&quote) = input.get(*cursor) else {
        return Ok(String::new());
    };
    if quote != '"' && quote != '\'' {
        return Ok(read_unquoted(input, cursor));
    }
    *cursor += 1;
    let mut out = String::new();
    let mut escaped = false;
    while *cursor < input.len() {
        let c = input[*cursor];
        *cursor += 1;
        if escaped {
            if c == quote || c == '\\' {
                out.push(c);
                escaped = false;
            } else {
                *cursor -= 1;
                return Err(format!("Invalid escape sequence '{c}' in quoted string"));
            }
        } else if c == '\\' {
            escaped = true;
        } else if c == quote {
            return Ok(out);
        } else {
            out.push(c);
        }
    }
    Err("Unclosed quoted string".to_string())
}

fn read_balanced(
    input: &[char],
    cursor: &mut usize,
    open: char,
    close: char,
) -> Result<(), String> {
    if input.get(*cursor) != Some(&open) {
        return Err(format!("Expected '{open}'"));
    }
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    while *cursor < input.len() {
        let c = input[*cursor];
        *cursor += 1;
        if escaped {
            escaped = false;
        } else if c == '\\' && quote.is_some() {
            escaped = true;
        } else if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
        } else if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Ok(());
            }
        }
    }
    Err(format!("Expected '{close}'"))
}

fn read_id_with_state(input: &[char], cursor: &mut usize) -> Result<(), String> {
    if input.get(*cursor) == Some(&'#') {
        *cursor += 1;
    }
    read_while(input, cursor, |c| is_unquoted_char(c) || c == ':');
    if input.get(*cursor) == Some(&'[') {
        read_balanced(input, cursor, '[', ']')?;
    }
    if input.get(*cursor) == Some(&'{') {
        read_balanced(input, cursor, '{', '}')?;
    }
    Ok(())
}

pub(super) const SELECTORS: [&str; 6] = ["@p", "@a", "@r", "@s", "@e", "@n"];

fn read_selector(input: &[char], cursor: &mut usize) -> Result<(), String> {
    let start = *cursor;
    if input.get(*cursor) == Some(&'@') {
        *cursor += 1;
        let Some(&kind) = input.get(*cursor) else {
            return Err("Missing selector type".to_string());
        };
        if !SELECTORS.iter().any(|s| s.ends_with(kind)) {
            *cursor = start;
            return Err(format!("Unknown selector type '@{kind}'"));
        }
        *cursor += 1;
        if input.get(*cursor) == Some(&'[') {
            read_balanced(input, cursor, '[', ']')?;
        }
        return Ok(());
    }
    let name = read_quotable(input, cursor)?;
    let length = name.chars().count();
    if length == 0 || (length > 16 && !is_uuid(&name)) {
        *cursor = start;
        return Err("Invalid name or UUID".to_string());
    }
    Ok(())
}

fn is_uuid(name: &str) -> bool {
    let groups: Vec<&str> = name.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, len)| g.len() == len && g.chars().all(|c| c.is_ascii_hexdigit()))
}

fn read_world_coordinate(input: &[char], cursor: &mut usize, integer: bool) -> Result<(), String> {
    let Some(&first) = input.get(*cursor) else {
        return Err(if integer {
            "Expected a block position".to_string()
        } else {
            "Expected a coordinate".to_string()
        });
    };
    if first == '^' {
        return Err(MIXED_COORDINATES.to_string());
    }
    let relative = first == '~';
    if relative {
        *cursor += 1;
    }
    if matches!(input.get(*cursor), None | Some(' ')) {
        return Ok(());
    }
    read_number(input, cursor, integer && !relative)
}

fn read_local_coordinate(input: &[char], cursor: &mut usize) -> Result<(), String> {
    match input.get(*cursor) {
        None => return Err("Expected a coordinate".to_string()),
        Some('^') => {}
        Some(_) => return Err(MIXED_COORDINATES.to_string()),
    }
    *cursor += 1;
    if matches!(input.get(*cursor), None | Some(' ')) {
        return Ok(());
    }
    read_number(input, cursor, false)
}

fn read_number(input: &[char], cursor: &mut usize, integer: bool) -> Result<(), String> {
    if integer {
        read_bounded::<i32>(input, cursor, "integer").map(|_| ())
    } else {
        read_bounded::<f64>(input, cursor, "double").map(|_| ())
    }
}

fn read_bounded<T: std::str::FromStr>(
    input: &[char],
    cursor: &mut usize,
    kind: &str,
) -> Result<T, String> {
    let start = *cursor;
    let word = read_number_word(input, cursor);
    if word.is_empty() {
        return Err(format!("Expected {kind}"));
    }
    word.parse().map_err(|_| {
        *cursor = start;
        format!("Invalid {kind} '{word}'")
    })
}

const MIXED_COORDINATES: &str =
    "Cannot mix world & local coordinates (everything must either use ^ or not)";

fn incomplete(axes: usize) -> String {
    format!("Incomplete (expected {axes} coordinates)")
}

fn read_coordinates(
    input: &[char],
    cursor: &mut usize,
    axes: usize,
    integer: bool,
    local: bool,
) -> Result<(), String> {
    let start = *cursor;
    let is_local = local && input.get(*cursor) == Some(&'^');
    for i in 0..axes {
        if i > 0 {
            if input.get(*cursor) != Some(&' ') {
                *cursor = start;
                return Err(incomplete(axes));
            }
            *cursor += 1;
        }
        if is_local {
            if input.get(*cursor).is_some_and(|&c| c != '^') {
                *cursor = start;
            }
            read_local_coordinate(input, cursor)?;
        } else {
            read_world_coordinate(input, cursor, integer)?;
        }
    }
    Ok(())
}

fn check_range<T: PartialOrd + std::fmt::Display>(
    kind: &str,
    value: T,
    min: Option<T>,
    max: Option<T>,
) -> Result<(), String> {
    if let Some(min) = min
        && value < min
    {
        return Err(format!("{kind} must not be less than {min}: found {value}"));
    }
    if let Some(max) = max
        && value > max
    {
        return Err(format!("{kind} must not be more than {max}: found {value}"));
    }
    Ok(())
}

pub(super) fn parse_argument(
    parser: &BrigadierParser,
    input: &[char],
    cursor: &mut usize,
) -> Result<(), String> {
    let start = *cursor;
    match parser {
        BrigadierParser::Bool => {
            let word = read_unquoted(input, cursor);
            match word.as_str() {
                "true" | "false" => Ok(()),
                "" => Err("Expected boolean".to_string()),
                other => {
                    *cursor = start;
                    Err(format!(
                        "Invalid boolean: expected 'true' or 'false' but found '{other}'"
                    ))
                }
            }
        }
        BrigadierParser::Integer(n) => {
            let v: i32 = read_bounded(input, cursor, "integer")?;
            check_range("Integer", v, n.min, n.max).inspect_err(|_| *cursor = start)
        }
        BrigadierParser::Long(n) => {
            let v: i64 = read_bounded(input, cursor, "long")?;
            check_range("Long", v, n.min, n.max).inspect_err(|_| *cursor = start)
        }
        BrigadierParser::Float(n) => {
            let v: f32 = read_bounded(input, cursor, "float")?;
            check_range("Float", v, n.min, n.max).inspect_err(|_| *cursor = start)
        }
        BrigadierParser::Double(n) => {
            let v: f64 = read_bounded(input, cursor, "double")?;
            check_range("Double", v, n.min, n.max).inspect_err(|_| *cursor = start)
        }
        BrigadierParser::String(kind) => match kind {
            BrigadierString::SingleWord => {
                read_unquoted(input, cursor);
                Ok(())
            }
            BrigadierString::QuotablePhrase => read_quotable(input, cursor).map(|_| ()),
            BrigadierString::GreedyPhrase => {
                *cursor = input.len();
                Ok(())
            }
        },
        BrigadierParser::Message => {
            *cursor = input.len();
            Ok(())
        }
        BrigadierParser::FormattedText | BrigadierParser::Style | BrigadierParser::NbtTag => {
            match input.get(*cursor) {
                Some('{') => read_balanced(input, cursor, '{', '}'),
                Some('[') => read_balanced(input, cursor, '[', ']'),
                _ => read_quotable(input, cursor).map(|_| ()),
            }
        }
        BrigadierParser::NbtCompoundTag => read_balanced(input, cursor, '{', '}'),
        BrigadierParser::Entity(_) | BrigadierParser::GameProfile => read_selector(input, cursor),
        BrigadierParser::ScoreHolder { .. } => {
            if input.get(*cursor) == Some(&'*') {
                *cursor += 1;
                Ok(())
            } else {
                read_selector(input, cursor)
            }
        }
        BrigadierParser::BlockPos => read_coordinates(input, cursor, 3, true, true),
        BrigadierParser::Vec3 => read_coordinates(input, cursor, 3, false, true),
        BrigadierParser::ColumnPos => read_coordinates(input, cursor, 2, true, false),
        BrigadierParser::Vec2 => read_coordinates(input, cursor, 2, false, false),
        BrigadierParser::Rotation => read_coordinates(input, cursor, 2, false, false),
        BrigadierParser::BlockState
        | BrigadierParser::BlockPredicate
        | BrigadierParser::ItemStack
        | BrigadierParser::ItemPredicate
        | BrigadierParser::Particle => read_id_with_state(input, cursor),
        _ => {
            if read_token(input, cursor).is_empty() {
                Err("Expected a value".to_string())
            } else {
                Ok(())
            }
        }
    }
}

fn read_number_word(input: &[char], cursor: &mut usize) -> String {
    read_while(input, cursor, |c| {
        c.is_ascii_digit() || matches!(c, '.' | '-')
    })
    .iter()
    .collect()
}
