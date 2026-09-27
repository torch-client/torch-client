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
    pub errors: Vec<(usize, ParseError)>,
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
            ParseError::Argument(message) => format!("Could not parse command: {message}"),
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

    fn can_read(&self, input: &[char]) -> bool {
        self.cursor < input.len()
    }

    pub(super) fn last(&self) -> &Context {
        self.chain.last().expect("a parse always has one context")
    }
}

pub fn parse(tree: &CommandTree, input: &[char], start: usize) -> Parse {
    parse_nodes(
        tree,
        tree.root,
        input,
        start,
        Context::new(tree.root, start),
    )
}

fn parse_nodes(
    tree: &CommandTree,
    node: usize,
    input: &[char],
    cursor: usize,
    context_so_far: Context,
) -> Parse {
    let mut errors: Vec<(usize, ParseError)> = Vec::new();
    let mut potentials: Vec<Parse> = Vec::new();

    for child in tree.relevant_children(node, input, cursor) {
        let mut context = context_so_far.clone();
        let mut reader = cursor;

        match parse_child(tree, child, input, &mut reader, &mut context) {
            Err(error) => {
                errors.push((child, error));
                continue;
            }
            Ok(()) => {}
        }
        if reader < input.len() && input[reader] != ' ' {
            errors.push((child, ParseError::ExpectedSeparator));
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
                    let sub = parse_nodes(tree, redirect, input, reader, child_context);
                    let mut chain = vec![context];
                    chain.extend(sub.chain);
                    return Parse {
                        chain,
                        cursor: sub.cursor,
                        errors: sub.errors,
                    };
                }
                None => potentials.push(parse_nodes(tree, child, input, reader, context)),
            }
        } else {
            potentials.push(Parse::single(context, reader));
        }
    }

    if !potentials.is_empty() {
        if potentials.len() > 1 {
            potentials.sort_by(|a, b| {
                let key = |p: &Parse| (p.can_read(input), !p.errors.is_empty());
                key(a).cmp(&key(b))
            });
        }
        return potentials.into_iter().next().unwrap();
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
            if matched && (end == input.len() || input[end] == ' ') {
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

fn read_unquoted(input: &[char], cursor: &mut usize) -> String {
    let start = *cursor;
    while *cursor < input.len() && is_unquoted_char(input[*cursor]) {
        *cursor += 1;
    }
    input[start..*cursor].iter().collect()
}

fn is_unquoted_char(c: char) -> bool {
    c.is_ascii_digit() || c.is_ascii_alphabetic() || matches!(c, '_' | '-' | '.' | '+')
}

fn read_token(input: &[char], cursor: &mut usize) -> String {
    let start = *cursor;
    while *cursor < input.len() && input[*cursor] != ' ' {
        *cursor += 1;
    }
    input[start..*cursor].iter().collect()
}

fn read_quotable(input: &[char], cursor: &mut usize) -> Result<String, String> {
    if *cursor >= input.len() {
        return Ok(String::new());
    }
    let quote = input[*cursor];
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
    if *cursor >= input.len() || input[*cursor] != open {
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
    if *cursor < input.len() && input[*cursor] == '#' {
        *cursor += 1;
    }
    while *cursor < input.len() && (is_unquoted_char(input[*cursor]) || input[*cursor] == ':') {
        *cursor += 1;
    }
    if *cursor < input.len() && input[*cursor] == '[' {
        read_balanced(input, cursor, '[', ']')?;
    }
    if *cursor < input.len() && input[*cursor] == '{' {
        read_balanced(input, cursor, '{', '}')?;
    }
    Ok(())
}

pub(super) const SELECTORS: [&str; 6] = ["@p", "@a", "@r", "@s", "@e", "@n"];

fn read_selector(input: &[char], cursor: &mut usize) -> Result<(), String> {
    if *cursor < input.len() && input[*cursor] == '@' {
        *cursor += 1;
        if *cursor >= input.len() {
            return Err("Missing selector type".to_string());
        }
        let kind = input[*cursor];
        if !SELECTORS.iter().any(|s| s.ends_with(kind)) {
            return Err(format!("Unknown selector type '@{kind}'"));
        }
        *cursor += 1;
        if *cursor < input.len() && input[*cursor] == '[' {
            read_balanced(input, cursor, '[', ']')?;
        }
        return Ok(());
    }
    let start = *cursor;
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
    if *cursor < input.len() && input[*cursor] == '^' {
        return Err(MIXED_COORDINATES.to_string());
    }
    if *cursor >= input.len() {
        return Err(if integer {
            "Expected a block position".to_string()
        } else {
            "Expected a coordinate".to_string()
        });
    }
    let relative = input[*cursor] == '~';
    if relative {
        *cursor += 1;
    }
    if *cursor >= input.len() || input[*cursor] == ' ' {
        return Ok(());
    }
    read_number(input, cursor, integer && !relative)
}

fn read_local_coordinate(input: &[char], cursor: &mut usize) -> Result<(), String> {
    if *cursor >= input.len() {
        return Err(incomplete(3));
    }
    if input[*cursor] != '^' {
        return Err(MIXED_COORDINATES.to_string());
    }
    *cursor += 1;
    if *cursor >= input.len() || input[*cursor] == ' ' {
        return Ok(());
    }
    read_number(input, cursor, false)
}

fn read_number(input: &[char], cursor: &mut usize, integer: bool) -> Result<(), String> {
    let word = read_number_word(input, cursor);
    if word.is_empty() {
        return Err(format!(
            "Expected {}",
            if integer { "integer" } else { "double" }
        ));
    }
    if integer {
        word.parse::<i32>()
            .map(|_| ())
            .map_err(|_| format!("Invalid integer '{word}'"))
    } else {
        word.parse::<f64>()
            .map(|_| ())
            .map_err(|_| format!("Invalid double '{word}'"))
    }
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
    let is_local = local && *cursor < input.len() && input[*cursor] == '^';
    for i in 0..axes {
        if i > 0 {
            if *cursor >= input.len() || input[*cursor] != ' ' {
                return Err(incomplete(axes));
            }
            *cursor += 1;
        }
        if is_local {
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
    match parser {
        BrigadierParser::Bool => {
            let word = read_unquoted(input, cursor);
            match word.as_str() {
                "true" | "false" => Ok(()),
                "" => Err("Expected boolean".to_string()),
                other => Err(format!(
                    "Invalid boolean: expected 'true' or 'false' but found '{other}'"
                )),
            }
        }
        BrigadierParser::Integer(n) => {
            let word = read_number_word(input, cursor);
            if word.is_empty() {
                return Err("Expected integer".to_string());
            }
            let v: i32 = word
                .parse()
                .map_err(|_| format!("Invalid integer '{word}'"))?;
            check_range("Integer", v, n.min, n.max)
        }
        BrigadierParser::Long(n) => {
            let word = read_number_word(input, cursor);
            if word.is_empty() {
                return Err("Expected long".to_string());
            }
            let v: i64 = word.parse().map_err(|_| format!("Invalid long '{word}'"))?;
            check_range("Long", v, n.min, n.max)
        }
        BrigadierParser::Float(n) => {
            let word = read_number_word(input, cursor);
            if word.is_empty() {
                return Err("Expected float".to_string());
            }
            let v: f32 = word
                .parse()
                .map_err(|_| format!("Invalid float '{word}'"))?;
            check_range("Float", v, n.min, n.max)
        }
        BrigadierParser::Double(n) => {
            let word = read_number_word(input, cursor);
            if word.is_empty() {
                return Err("Expected double".to_string());
            }
            let v: f64 = word
                .parse()
                .map_err(|_| format!("Invalid double '{word}'"))?;
            check_range("Double", v, n.min, n.max)
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
        BrigadierParser::FormattedText | BrigadierParser::Style => {
            if *cursor < input.len() && (input[*cursor] == '{' || input[*cursor] == '[') {
                let (open, close) = if input[*cursor] == '{' {
                    ('{', '}')
                } else {
                    ('[', ']')
                };
                read_balanced(input, cursor, open, close)
            } else {
                read_quotable(input, cursor).map(|_| ())
            }
        }
        BrigadierParser::NbtCompoundTag => read_balanced(input, cursor, '{', '}'),
        BrigadierParser::NbtTag => {
            if *cursor < input.len() && input[*cursor] == '{' {
                read_balanced(input, cursor, '{', '}')
            } else if *cursor < input.len() && input[*cursor] == '[' {
                read_balanced(input, cursor, '[', ']')
            } else {
                read_quotable(input, cursor).map(|_| ())
            }
        }
        BrigadierParser::Entity(_) | BrigadierParser::GameProfile => read_selector(input, cursor),
        BrigadierParser::ScoreHolder { .. } => {
            if *cursor < input.len() && input[*cursor] == '*' {
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
    let start = *cursor;
    while *cursor < input.len()
        && (input[*cursor].is_ascii_digit() || matches!(input[*cursor], '.' | '-'))
    {
        *cursor += 1;
    }
    input[start..*cursor].iter().collect()
}
