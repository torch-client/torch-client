#![cfg_attr(not(feature = "click_gui"), allow(dead_code))]

use std::borrow::Cow;
use std::fmt::Write as _;
use std::num::IntErrorKind;

use simdnbt::owned::{NbtCompound, NbtList, NbtTag};
use simdnbt::{Mutf8Str, Mutf8String};

use crate::text::{AQUA, GOLD, GRAY, GREEN, RED, Span};

const MAX_DEPTH: usize = 512;

const TEXT_MAX_DEPTH: usize = 64;
const TEXT_MAX_LENGTH: usize = 128;

const EMPTY_KEY: &str = "Key cannot be empty";
const EXPECTED_BINARY: &str = "Expected a binary number";
const EXPECTED_DECIMAL: &str = "Expected a decimal number";
const EXPECTED_HEX: &str = "Expected a hexadecimal number";
const EXPECTED_NON_NEGATIVE: &str = "Expected a non-negative number";
const EXPECTED_NUMBER_OR_BOOLEAN: &str = "Expected a number or a boolean";
const EXPECTED_STRING_UUID: &str = "Expected a string representing a valid UUID";
const EXPECTED_UNQUOTED: &str = "Expected a valid unquoted string";
const INFINITY_NOT_ALLOWED: &str = "Non-finite numbers are not allowed";
const INVALID_ARRAY_ELEMENT: &str = "Invalid array element type";
const INVALID_CHARACTER_NAME: &str = "Invalid Unicode character name";
const INVALID_STRING_CONTENTS: &str = "Invalid string contents";
const INVALID_UNQUOTED_START: &str = "Unquoted strings can't start with digits 0-9, + or -";
const LEADING_ZERO: &str = "Decimal numbers can't start with 0";
const TRAILING_DATA: &str = "Unexpected trailing data";
const UNDERSCORE: &str = "Underscore characters are not allowed at the start or end of a number";

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub at: usize,
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at position {}", self.message, self.at)
    }
}

impl std::error::Error for Error {}

pub fn parse_prefix(input: &str) -> Result<(NbtTag, usize), Error> {
    let mut parser = Parser::new(input);
    let tag = parser.literal();
    if let Some(error) = parser.fatal.take() {
        return Err(error);
    }
    match tag {
        Some(tag) => Ok((tag, parser.pos)),
        None => Err(parser.into_error()),
    }
}

pub fn parse(input: &str) -> Result<NbtTag, Error> {
    let (tag, end) = parse_prefix(input)?;
    let end = skip_ws(input, end);
    if end < input.len() {
        return Err(Error {
            at: end,
            message: TRAILING_DATA.to_string(),
        });
    }
    Ok(tag)
}

pub fn to_snbt(tag: &NbtTag) -> String {
    let mut out = String::new();
    snbt(&mut out, view(tag));
    out
}

pub fn compound_to_snbt(c: &NbtCompound) -> String {
    let mut out = String::new();
    snbt(&mut out, View::Compound(c));
    out
}

pub fn to_spans(tag: &NbtTag) -> Vec<Span> {
    let mut printer = SpanPrinter::default();
    printer.visit(view(tag));
    printer.out
}

pub fn compound_to_spans(c: &NbtCompound) -> Vec<Span> {
    let mut printer = SpanPrinter::default();
    printer.visit(View::Compound(c));
    printer.out
}

fn is_java_whitespace(c: char) -> bool {
    match c {
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1c}'..='\u{1f}' => true,
        '\u{a0}' | '\u{2007}' | '\u{202f}' => false,
        ' '
        | '\u{1680}'
        | '\u{2000}'..='\u{200a}'
        | '\u{2028}'
        | '\u{2029}'
        | '\u{205f}'
        | '\u{3000}' => true,
        _ => false,
    }
}

fn skip_ws(s: &str, pos: usize) -> usize {
    match s[pos..].find(|c: char| !is_java_whitespace(c)) {
        Some(offset) => pos + offset,
        None => s.len(),
    }
}

fn can_start_number(c: char) -> bool {
    matches!(c, '+' | '-' | '.' | '0'..='9')
}

fn is_unquoted_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+')
}

fn is_binary(c: char) -> bool {
    matches!(c, '0' | '1' | '_')
}

fn is_decimal(c: char) -> bool {
    c.is_ascii_digit() || c == '_'
}

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit() || c == '_'
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IntType {
    Byte,
    Short,
    Int,
    Long,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FloatType {
    Float,
    Double,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Base {
    Binary,
    Decimal,
    Hex,
}

struct IntegerLiteral {
    minus: bool,
    base: Base,
    digits: String,
    signed: Option<bool>,
    ty: Option<IntType>,
}

enum ListBody {
    Array(IntType, Vec<IntegerLiteral>),
    List(Vec<NbtTag>),
}

struct FloatParts {
    whole: Option<String>,
    fraction: Option<String>,
    exponent: Option<(bool, String)>,
    suffix: Option<FloatType>,
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
    error_at: usize,
    error: Option<String>,
    fatal: Option<Error>,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self {
        Parser {
            s,
            pos: 0,
            error_at: 0,
            error: None,
            fatal: None,
            depth: 0,
        }
    }

    fn into_error(self) -> Error {
        Error {
            at: self.error_at,
            message: self.error.unwrap_or_else(|| "Failed to parse".to_string()),
        }
    }

    fn store(&mut self, at: usize, message: impl FnOnce() -> String) {
        if self.fatal.is_some() {
            return;
        }
        if self.error.is_none() || at > self.error_at {
            self.error_at = at;
            self.error = Some(message());
        }
    }

    fn ahead(&self) -> Option<char> {
        self.s[skip_ws(self.s, self.pos)..].chars().next()
    }

    fn ch2(&mut self, a: char, b: char) -> bool {
        let start = self.pos;
        self.pos = skip_ws(self.s, self.pos);
        if let Some(c) = self.s[self.pos..].chars().next() {
            if c == a || c == b {
                self.pos += c.len_utf8();
                return true;
            }
        }
        let at = self.pos;
        self.store(at, || {
            if a == b {
                format!("Expected literal {a}")
            } else {
                format!("Expected literal {a}|{b}")
            }
        });
        self.pos = start;
        false
    }

    fn ch(&mut self, c: char) -> bool {
        self.ch2(c, c)
    }

    fn number_run(&mut self, accept: fn(char) -> bool, no_value: &'static str) -> Option<String> {
        let s: &'a str = self.s;
        let restore = self.pos;
        let start = skip_ws(s, self.pos);
        let end = match s[start..].find(|c: char| !accept(c)) {
            Some(offset) => start + offset,
            None => s.len(),
        };
        if end == start {
            self.store(start, || no_value.to_string());
            self.pos = restore;
            return None;
        }
        let run = &s[start..end];
        if run.starts_with('_') || run.ends_with('_') {
            self.store(start, || UNDERSCORE.to_string());
            self.pos = restore;
            return None;
        }
        self.pos = end;
        Some(run.to_string())
    }

    fn decimal(&mut self) -> Option<String> {
        self.number_run(is_decimal, EXPECTED_DECIMAL)
    }

    fn sign(&mut self) -> Option<bool> {
        if self.ch('+') {
            Some(false)
        } else if self.ch('-') {
            Some(true)
        } else {
            None
        }
    }

    fn opt_minus(&mut self) -> bool {
        self.sign().unwrap_or(false)
    }

    fn int_type(&mut self) -> Option<IntType> {
        for (lower, upper, ty) in [
            ('b', 'B', IntType::Byte),
            ('s', 'S', IntType::Short),
            ('i', 'I', IntType::Int),
            ('l', 'L', IntType::Long),
        ] {
            if self.ch2(lower, upper) {
                return Some(ty);
            }
        }
        None
    }

    fn integer_suffix(&mut self) -> (Option<bool>, Option<IntType>) {
        let start = self.pos;
        for (lower, upper, signed) in [('u', 'U', false), ('s', 'S', true)] {
            if self.ch2(lower, upper) {
                if let Some(ty) = self.int_type() {
                    return (Some(signed), Some(ty));
                }
            }
            self.pos = start;
        }
        (None, self.int_type())
    }

    fn integer_literal(&mut self) -> Option<IntegerLiteral> {
        let start = self.pos;
        let minus = self.opt_minus();
        let Some((base, digits)) = self.integer_body() else {
            self.pos = start;
            return None;
        };
        let (signed, ty) = self.integer_suffix();
        Some(IntegerLiteral {
            minus,
            base,
            digits,
            signed,
            ty,
        })
    }

    fn integer_body(&mut self) -> Option<(Base, String)> {
        let start = self.pos;
        if self.ch('0') {
            let after_zero = self.pos;
            if self.ch2('x', 'X') {
                let hex = self.number_run(is_hex, EXPECTED_HEX);
                if hex.is_none() {
                    self.pos = start;
                }
                return hex.map(|digits| (Base::Hex, digits));
            }
            if self.ch2('b', 'B') {
                if let Some(digits) = self.number_run(is_binary, EXPECTED_BINARY) {
                    return Some((Base::Binary, digits));
                }
                self.pos = after_zero;
            }
            if self.decimal().is_some() {
                let at = self.pos;
                self.store(at, || LEADING_ZERO.to_string());
                self.pos = start;
                return None;
            }
            return Some((Base::Decimal, "0".to_string()));
        }
        self.decimal().map(|digits| (Base::Decimal, digits))
    }

    fn float_suffix(&mut self) -> Option<FloatType> {
        if self.ch2('f', 'F') {
            Some(FloatType::Float)
        } else if self.ch2('d', 'D') {
            Some(FloatType::Double)
        } else {
            None
        }
    }

    fn exponent(&mut self) -> Option<(bool, String)> {
        let start = self.pos;
        if !self.ch2('e', 'E') {
            return None;
        }
        let minus = self.opt_minus();
        match self.decimal() {
            Some(digits) => Some((minus, digits)),
            None => {
                self.pos = start;
                None
            }
        }
    }

    fn float_parts(&mut self) -> Option<FloatParts> {
        let start = self.pos;
        if let Some(whole) = self.decimal() {
            if self.ch('.') {
                let fraction = self.decimal();
                let exponent = self.exponent();
                let suffix = self.float_suffix();
                return Some(FloatParts {
                    whole: Some(whole),
                    fraction,
                    exponent,
                    suffix,
                });
            }
        }
        self.pos = start;
        if self.ch('.') {
            let Some(fraction) = self.decimal() else {
                self.pos = start;
                return None;
            };
            let exponent = self.exponent();
            let suffix = self.float_suffix();
            return Some(FloatParts {
                whole: None,
                fraction: Some(fraction),
                exponent,
                suffix,
            });
        }
        if let Some(whole) = self.decimal() {
            if let Some(exponent) = self.exponent() {
                let suffix = self.float_suffix();
                return Some(FloatParts {
                    whole: Some(whole),
                    fraction: None,
                    exponent: Some(exponent),
                    suffix,
                });
            }
        }
        self.pos = start;
        if let Some(whole) = self.decimal() {
            let exponent = self.exponent();
            if let Some(suffix) = self.float_suffix() {
                return Some(FloatParts {
                    whole: Some(whole),
                    fraction: None,
                    exponent,
                    suffix: Some(suffix),
                });
            }
        }
        self.pos = start;
        None
    }

    fn float_literal(&mut self) -> Option<NbtTag> {
        let start = self.pos;
        let minus = self.opt_minus();
        let Some(parts) = self.float_parts() else {
            self.pos = start;
            return None;
        };
        let mut text = String::new();
        if minus {
            text.push('-');
        }
        match &parts.whole {
            Some(whole) => push_without_underscores(&mut text, whole),
            None => text.push('0'),
        }
        if let Some(fraction) = &parts.fraction {
            text.push('.');
            push_without_underscores(&mut text, fraction);
        }
        if let Some((exponent_minus, exponent)) = &parts.exponent {
            text.push('e');
            if *exponent_minus {
                text.push('-');
            }
            push_without_underscores(&mut text, exponent);
        }
        let at = self.pos;
        let tag = match parts.suffix {
            Some(FloatType::Float) => text
                .parse::<f32>()
                .ok()
                .map(|v| (v.is_finite(), NbtTag::Float(v))),
            _ => text
                .parse::<f64>()
                .ok()
                .map(|v| (v.is_finite(), NbtTag::Double(v))),
        };
        match tag {
            Some((true, tag)) => Some(tag),
            Some((false, _)) => {
                self.store(at, || INFINITY_NOT_ALLOWED.to_string());
                self.pos = start;
                None
            }
            None => {
                self.store(at, || {
                    format!("Failed to parse number: For input string: \"{text}\"")
                });
                self.pos = start;
                None
            }
        }
    }

    fn integer_value(&mut self, literal: &IntegerLiteral, ty: IntType) -> Option<i64> {
        let signed = literal.signed.unwrap_or(literal.base == Base::Decimal);
        let at = self.pos;
        if !signed && literal.minus {
            self.store(at, || EXPECTED_NON_NEGATIVE.to_string());
            return None;
        }
        let mut digits = String::with_capacity(literal.digits.len() + 1);
        if literal.minus {
            digits.push('-');
        }
        push_without_underscores(&mut digits, &literal.digits);
        let radix = match literal.base {
            Base::Binary => 2,
            Base::Decimal => 10,
            Base::Hex => 16,
        };
        let value = if signed {
            parse_signed(&digits, radix, ty)
        } else {
            parse_unsigned(&digits, radix, ty)
        };
        match value {
            Ok(value) => Some(value),
            Err(message) => {
                self.store(at, || format!("Failed to parse number: {message}"));
                None
            }
        }
    }

    fn quoted_string(&mut self) -> Option<String> {
        let start = self.pos;
        for quote in ['"', '\''] {
            if self.ch(quote) {
                let contents = self.string_contents(quote);
                if self.ch(quote) {
                    return Some(contents);
                }
                self.pos = start;
                return None;
            }
        }
        None
    }

    fn string_contents(&mut self, quote: char) -> String {
        let mut out = String::new();
        while self.string_chunk(quote, &mut out) {}
        out
    }

    fn string_chunk(&mut self, quote: char, out: &mut String) -> bool {
        let start = self.pos;
        let end = match self.s[start..].find(|c: char| matches!(c, '"' | '\'' | '\\')) {
            Some(offset) => start + offset,
            None => self.s.len(),
        };
        if end > start {
            out.push_str(&self.s[start..end]);
            self.pos = end;
            return true;
        }
        self.store(start, || INVALID_STRING_CONTENTS.to_string());
        if self.ch('\\') {
            if self.escape(out) {
                return true;
            }
            self.pos = start;
        }
        let other = if quote == '"' { '\'' } else { '"' };
        if self.ch(other) {
            out.push(other);
            return true;
        }
        false
    }

    fn escape(&mut self, out: &mut String) -> bool {
        let start = self.pos;
        for (letter, value) in [
            ('b', '\u{8}'),
            ('s', ' '),
            ('t', '\t'),
            ('n', '\n'),
            ('f', '\u{c}'),
            ('r', '\r'),
            ('\\', '\\'),
            ('\'', '\''),
            ('"', '"'),
        ] {
            if self.ch(letter) {
                out.push(value);
                return true;
            }
        }
        for (letter, size) in [('x', 2), ('u', 4), ('U', 8)] {
            if self.ch(letter) {
                if let Some(code_point) = self.hex_escape(size) {
                    if self.push_code_point(code_point, out) {
                        return true;
                    }
                }
                self.pos = start;
            }
        }
        if self.ch('N') && self.ch('{') {
            let s: &'a str = self.s;
            let name_start = self.pos;
            let name_end = match s[name_start..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == ' '))
            {
                Some(offset) => name_start + offset,
                None => s.len(),
            };
            if name_end == name_start {
                self.store(name_start, || INVALID_CHARACTER_NAME.to_string());
            } else {
                self.pos = name_end;
                if self.ch('}') {
                    let at = self.pos;
                    let name = &s[name_start..name_end];
                    self.store(at, || {
                        format!("Unicode character names are not supported: \\N{{{name}}}")
                    });
                }
            }
        }
        self.pos = start;
        false
    }

    fn hex_escape(&mut self, size: usize) -> Option<u32> {
        let start = self.pos;
        let digits = self.s[start..]
            .bytes()
            .take(size)
            .take_while(u8::is_ascii_hexdigit)
            .count();
        if digits < size {
            self.store(start, || {
                format!("Expected a character literal of length {size}")
            });
            return None;
        }
        self.pos = start + size;
        u32::from_str_radix(&self.s[start..start + size], 16).ok()
    }

    fn push_code_point(&mut self, code_point: u32, out: &mut String) -> bool {
        if let Some(c) = char::from_u32(code_point) {
            out.push(c);
            return true;
        }
        if (0xD800..0xDC00).contains(&code_point) {
            let s: &'a str = self.s;
            let rest = &s[self.pos..];
            if let Some(hex) = rest.strip_prefix("\\u").and_then(|r| r.get(..4)) {
                if let Ok(low) = u32::from_str_radix(hex, 16) {
                    if (0xDC00..0xE000).contains(&low) {
                        let pair = 0x10000 + ((code_point - 0xD800) << 10) + (low - 0xDC00);
                        if let Some(c) = char::from_u32(pair) {
                            self.pos += 6;
                            out.push(c);
                            return true;
                        }
                    }
                }
            }
        }
        let at = self.pos;
        self.store(at, || {
            if code_point <= 0x10FFFF {
                format!(
                    "Invalid Unicode character value: U+{code_point:08X} (an unpaired surrogate)"
                )
            } else {
                format!("Invalid Unicode character value: U+{code_point:08X}")
            }
        });
        false
    }

    fn unquoted_string(&mut self) -> Option<&'a str> {
        let s: &'a str = self.s;
        let start = skip_ws(s, self.pos);
        let end = match s[start..].find(|c: char| !is_unquoted_char(c)) {
            Some(offset) => start + offset,
            None => s.len(),
        };
        if end == start {
            self.store(start, || EXPECTED_UNQUOTED.to_string());
            return None;
        }
        self.pos = end;
        Some(&s[start..end])
    }

    fn literal(&mut self) -> Option<NbtTag> {
        if self.fatal.is_some() {
            return None;
        }
        let start = self.pos;
        let tag = self.literal_inner();
        if tag.is_none() {
            self.pos = start;
        }
        tag
    }

    fn literal_inner(&mut self) -> Option<NbtTag> {
        let next = self.ahead();
        if next.is_some_and(can_start_number) {
            if let Some(tag) = self.float_literal() {
                return Some(tag);
            }
            if let Some(literal) = self.integer_literal() {
                let ty = literal.ty.unwrap_or(IntType::Int);
                let value = self.integer_value(&literal, ty)?;
                return Some(match ty {
                    IntType::Byte => NbtTag::Byte(value as i8),
                    IntType::Short => NbtTag::Short(value as i16),
                    IntType::Int => NbtTag::Int(value as i32),
                    IntType::Long => NbtTag::Long(value),
                });
            }
        }
        match next {
            Some('"' | '\'') => {
                return self
                    .quoted_string()
                    .map(|s| NbtTag::String(Mutf8String::from(s)));
            }
            Some('{') => return self.nested(Self::map_literal),
            Some('[') => return self.nested(Self::list_literal),
            _ => {}
        }
        self.unquoted_or_builtin()
    }

    fn nested<T>(&mut self, rule: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        if self.depth >= MAX_DEPTH {
            let at = skip_ws(self.s, self.pos);
            self.fatal = Some(Error {
                at,
                message: format!(
                    "Tried to read NBT tag with too high complexity, depth > {MAX_DEPTH}"
                ),
            });
            return None;
        }
        self.depth += 1;
        let result = rule(self);
        self.depth -= 1;
        result
    }

    fn separated<T>(&mut self, mut element: impl FnMut(&mut Self) -> Option<T>) -> Vec<T> {
        let mut out = Vec::new();
        loop {
            if !out.is_empty() && !self.ch(',') {
                break;
            }
            let after_separator = self.pos;
            match element(self) {
                Some(value) => out.push(value),
                None => {
                    self.pos = after_separator;
                    break;
                }
            }
        }
        out
    }

    fn map_literal(&mut self) -> Option<NbtTag> {
        let start = self.pos;
        if !self.ch('{') {
            return None;
        }
        let entries = self.separated(Self::map_entry);
        if !self.ch('}') {
            self.pos = start;
            return None;
        }
        let mut compound = NbtCompound::new();
        for (key, value) in entries {
            match compound.get_mut(&key) {
                Some(slot) => *slot = value,
                None => compound.insert(key, value),
            }
        }
        Some(NbtTag::Compound(compound))
    }

    fn map_entry(&mut self) -> Option<(String, NbtTag)> {
        let start = self.pos;
        let key = match self.quoted_string() {
            Some(key) => key,
            None => self.unquoted_string()?.to_string(),
        };
        if !self.ch(':') {
            self.pos = start;
            return None;
        }
        let Some(value) = self.literal() else {
            self.pos = start;
            return None;
        };
        if key.is_empty() {
            let at = self.pos;
            self.store(at, || EMPTY_KEY.to_string());
            self.pos = start;
            return None;
        }
        Some((key, value))
    }

    fn array_prefix(&mut self) -> Option<IntType> {
        if self.ch('B') {
            Some(IntType::Byte)
        } else if self.ch('L') {
            Some(IntType::Long)
        } else if self.ch('I') {
            Some(IntType::Int)
        } else {
            None
        }
    }

    fn list_literal(&mut self) -> Option<NbtTag> {
        let start = self.pos;
        if !self.ch('[') {
            return None;
        }
        let after_open = self.pos;
        let body = 'body: {
            if let Some(prefix) = self.array_prefix() {
                if self.ch(';') {
                    break 'body ListBody::Array(prefix, self.separated(Self::integer_literal));
                }
            }
            self.pos = after_open;
            ListBody::List(self.separated(Self::literal))
        };
        if !self.ch(']') {
            self.pos = start;
            return None;
        }
        let tag = match body {
            ListBody::Array(prefix, entries) => self.array(prefix, &entries),
            ListBody::List(tags) => Some(NbtTag::List(list_from_tags(tags))),
        };
        if tag.is_none() {
            self.pos = start;
        }
        tag
    }

    fn array(&mut self, prefix: IntType, entries: &[IntegerLiteral]) -> Option<NbtTag> {
        let mut values = Vec::with_capacity(entries.len());
        for entry in entries {
            let ty = entry.ty.unwrap_or(prefix);
            let allowed = match prefix {
                IntType::Byte => ty == IntType::Byte,
                IntType::Int => ty != IntType::Long,
                IntType::Short | IntType::Long => true,
            };
            if !allowed {
                let at = self.pos;
                self.store(at, || INVALID_ARRAY_ELEMENT.to_string());
                return None;
            }
            values.push(self.integer_value(entry, ty)?);
        }
        Some(match prefix {
            IntType::Byte => NbtTag::ByteArray(values.iter().map(|&v| v as u8).collect()),
            IntType::Int | IntType::Short => {
                NbtTag::IntArray(values.iter().map(|&v| v as i32).collect())
            }
            IntType::Long => NbtTag::LongArray(values),
        })
    }

    fn unquoted_or_builtin(&mut self) -> Option<NbtTag> {
        let start = self.pos;
        let name = self.unquoted_string()?;
        let before_args = self.pos;
        let mut arguments = None;
        if self.ch('(') {
            if let Some(args) = self.nested(|p| Some(p.separated(Self::literal))) {
                if self.ch(')') {
                    arguments = Some(args);
                }
            }
            if arguments.is_none() {
                self.pos = before_args;
            }
        }
        let at = self.pos;
        if name.starts_with(can_start_number) {
            self.store(at, || INVALID_UNQUOTED_START.to_string());
            self.pos = start;
            return None;
        }
        if let Some(arguments) = arguments {
            let tag = self.builtin(name, arguments);
            if tag.is_none() {
                self.pos = start;
            }
            return tag;
        }
        Some(if name.eq_ignore_ascii_case("true") {
            NbtTag::Byte(1)
        } else if name.eq_ignore_ascii_case("false") {
            NbtTag::Byte(0)
        } else {
            NbtTag::String(Mutf8String::from(name))
        })
    }

    fn builtin(&mut self, name: &str, arguments: Vec<NbtTag>) -> Option<NbtTag> {
        let at = self.pos;
        match (name, arguments.as_slice()) {
            ("bool", [argument]) => match number_value(argument) {
                Some(value) => Some(NbtTag::Byte(i8::from(value != 0.0))),
                None => {
                    self.store(at, || EXPECTED_NUMBER_OR_BOOLEAN.to_string());
                    None
                }
            },
            ("uuid", [argument]) => {
                let ints = match argument {
                    NbtTag::String(s) => java_uuid(&s.to_str()),
                    _ => None,
                };
                match ints {
                    Some(ints) => Some(NbtTag::IntArray(ints.to_vec())),
                    None => {
                        self.store(at, || EXPECTED_STRING_UUID.to_string());
                        None
                    }
                }
            }
            _ => {
                let count = arguments.len();
                self.store(at, || format!("No such operation: {name}/{count}"));
                None
            }
        }
    }
}

fn push_without_underscores(out: &mut String, digits: &str) {
    out.extend(digits.chars().filter(|&c| c != '_'));
}

fn bad_input(digits: &str, radix: u32) -> String {
    if radix == 10 {
        format!("For input string: \"{digits}\"")
    } else {
        format!("For input string: \"{digits}\" under radix {radix}")
    }
}

fn parse_signed(digits: &str, radix: u32, ty: IntType) -> Result<i64, String> {
    match ty {
        IntType::Byte | IntType::Short => {
            let value = i32::from_str_radix(digits, radix).map_err(|_| bad_input(digits, radix))?;
            let (min, max) = if ty == IntType::Byte {
                (i32::from(i8::MIN), i32::from(i8::MAX))
            } else {
                (i32::from(i16::MIN), i32::from(i16::MAX))
            };
            if value < min || value > max {
                return Err(format!(
                    "Value out of range. Value:\"{digits}\" Radix:{radix}"
                ));
            }
            Ok(i64::from(value))
        }
        IntType::Int => i32::from_str_radix(digits, radix)
            .map(i64::from)
            .map_err(|_| bad_input(digits, radix)),
        IntType::Long => i64::from_str_radix(digits, radix).map_err(|_| bad_input(digits, radix)),
    }
}

fn parse_unsigned(digits: &str, radix: u32, ty: IntType) -> Result<i64, String> {
    match ty {
        IntType::Byte | IntType::Short => {
            let value = i32::from_str_radix(digits, radix).map_err(|_| bad_input(digits, radix))?;
            let bits = if ty == IntType::Byte { 8 } else { 16 };
            if value >> bits != 0 {
                return Err(format!("out of range: {value}"));
            }
            Ok(if ty == IntType::Byte {
                i64::from(value as u8 as i8)
            } else {
                i64::from(value as u16 as i16)
            })
        }
        IntType::Int => match u32::from_str_radix(digits, radix) {
            Ok(value) => Ok(i64::from(value as i32)),
            Err(e) if *e.kind() == IntErrorKind::PosOverflow => Err(format!(
                "String value {digits} exceeds range of unsigned int."
            )),
            Err(_) => Err(bad_input(digits, radix)),
        },
        IntType::Long => match u64::from_str_radix(digits, radix) {
            Ok(value) => Ok(value as i64),
            Err(e) if *e.kind() == IntErrorKind::PosOverflow => Err(format!(
                "String value {digits} exceeds range of unsigned long."
            )),
            Err(_) => Err(bad_input(digits, radix)),
        },
    }
}

fn number_value(tag: &NbtTag) -> Option<f64> {
    Some(match tag {
        NbtTag::Byte(v) => f64::from(*v),
        NbtTag::Short(v) => f64::from(*v),
        NbtTag::Int(v) => f64::from(*v),
        NbtTag::Long(v) => *v as f64,
        NbtTag::Float(v) => f64::from(*v),
        NbtTag::Double(v) => *v,
        _ => return None,
    })
}

fn java_uuid(s: &str) -> Option<[i32; 4]> {
    if s.encode_utf16().count() > 36 {
        return None;
    }
    let dash = |from: usize| s[from..].find('-').map(|offset| from + offset);
    let d1 = dash(0)?;
    let d2 = dash(d1 + 1)?;
    let d3 = dash(d2 + 1)?;
    let d4 = dash(d3 + 1)?;
    if dash(d4 + 1).is_some() {
        return None;
    }
    let field = |from: usize, to: usize| i64::from_str_radix(&s[from..to], 16).ok();
    let mut most = field(0, d1)? & 0xFFFF_FFFF;
    most = (most << 16) | (field(d1 + 1, d2)? & 0xFFFF);
    most = (most << 16) | (field(d2 + 1, d3)? & 0xFFFF);
    let mut least = field(d3 + 1, d4)? & 0xFFFF;
    least = (least << 48) | (field(d4 + 1, s.len())? & 0xFFFF_FFFF_FFFF);
    Some([
        (most >> 32) as i32,
        most as i32,
        (least >> 32) as i32,
        least as i32,
    ])
}

fn list_from_tags(tags: Vec<NbtTag>) -> NbtList {
    macro_rules! collect {
        ($tags:expr, $variant:ident) => {
            NbtList::$variant(
                $tags
                    .into_iter()
                    .filter_map(|tag| match tag {
                        NbtTag::$variant(v) => Some(v),
                        _ => None,
                    })
                    .collect(),
            )
        };
    }
    let Some(first) = tags.first() else {
        return NbtList::Empty;
    };
    let id = first.id();
    if tags.iter().any(|tag| tag.id() != id) {
        return NbtList::Compound(tags.into_iter().map(wrap_if_needed).collect());
    }
    match id {
        1 => collect!(tags, Byte),
        2 => collect!(tags, Short),
        3 => collect!(tags, Int),
        4 => collect!(tags, Long),
        5 => collect!(tags, Float),
        6 => collect!(tags, Double),
        7 => collect!(tags, ByteArray),
        8 => collect!(tags, String),
        9 => collect!(tags, List),
        11 => collect!(tags, IntArray),
        12 => collect!(tags, LongArray),
        _ => NbtList::Compound(tags.into_iter().map(wrap_if_needed).collect()),
    }
}

fn is_wrapper(c: &NbtCompound) -> bool {
    c.len() == 1 && c.contains("")
}

fn wrap_if_needed(tag: NbtTag) -> NbtCompound {
    match tag {
        NbtTag::Compound(c) if !is_wrapper(&c) => c,
        other => NbtCompound::from_values(vec![(Mutf8String::from(""), other)]),
    }
}

fn unwrap_element(c: &NbtCompound) -> Option<&NbtTag> {
    if c.len() == 1 { c.get("") } else { None }
}

#[derive(Clone, Copy)]
enum View<'a> {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(&'a [u8]),
    String(&'a Mutf8Str),
    List(&'a NbtList),
    Compound(&'a NbtCompound),
    IntArray(&'a [i32]),
    LongArray(&'a [i64]),
}

fn view(tag: &NbtTag) -> View<'_> {
    match tag {
        NbtTag::Byte(v) => View::Byte(*v),
        NbtTag::Short(v) => View::Short(*v),
        NbtTag::Int(v) => View::Int(*v),
        NbtTag::Long(v) => View::Long(*v),
        NbtTag::Float(v) => View::Float(*v),
        NbtTag::Double(v) => View::Double(*v),
        NbtTag::ByteArray(v) => View::ByteArray(v.as_slice()),
        NbtTag::String(v) => View::String(v.as_str()),
        NbtTag::List(v) => View::List(v),
        NbtTag::Compound(v) => View::Compound(v),
        NbtTag::IntArray(v) => View::IntArray(v.as_slice()),
        NbtTag::LongArray(v) => View::LongArray(v.as_slice()),
    }
}

fn list_views(list: &NbtList) -> Vec<View<'_>> {
    match list {
        NbtList::Empty => Vec::new(),
        NbtList::Byte(v) => v.iter().map(|&x| View::Byte(x)).collect(),
        NbtList::Short(v) => v.iter().map(|&x| View::Short(x)).collect(),
        NbtList::Int(v) => v.iter().map(|&x| View::Int(x)).collect(),
        NbtList::Long(v) => v.iter().map(|&x| View::Long(x)).collect(),
        NbtList::Float(v) => v.iter().map(|&x| View::Float(x)).collect(),
        NbtList::Double(v) => v.iter().map(|&x| View::Double(x)).collect(),
        NbtList::ByteArray(v) => v.iter().map(|x| View::ByteArray(x.as_slice())).collect(),
        NbtList::String(v) => v.iter().map(|x| View::String(x.as_str())).collect(),
        NbtList::List(v) => v.iter().map(View::List).collect(),
        NbtList::Compound(v) => v
            .iter()
            .map(|c| match unwrap_element(c) {
                Some(inner) => view(inner),
                None => View::Compound(c),
            })
            .collect(),
        NbtList::IntArray(v) => v.iter().map(|x| View::IntArray(x.as_slice())).collect(),
        NbtList::LongArray(v) => v.iter().map(|x| View::LongArray(x.as_slice())).collect(),
    }
}

fn java_decimal(scientific: &str) -> String {
    let (negative, rest) = match scientific.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, scientific),
    };
    let (mantissa, exponent) = rest.split_once('e').unwrap_or((rest, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if (-3..7).contains(&exponent) {
        if exponent >= 0 {
            let whole = exponent as usize + 1;
            if digits.len() <= whole {
                out.push_str(&digits);
                out.extend(std::iter::repeat_n('0', whole - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..whole]);
                out.push('.');
                out.push_str(&digits[whole..]);
            }
        } else {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-exponent - 1) as usize));
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        if digits.len() > 1 {
            out.push_str(&digits[1..]);
        } else {
            out.push('0');
        }
        let _ = write!(out, "E{exponent}");
    }
    out
}

fn java_double(v: f64) -> String {
    if v.is_nan() {
        "NaN".to_string()
    } else if v.is_infinite() {
        (if v > 0.0 { "Infinity" } else { "-Infinity" }).to_string()
    } else {
        java_decimal(&format!("{v:e}"))
    }
}

fn java_float(v: f32) -> String {
    if v.is_nan() {
        "NaN".to_string()
    } else if v.is_infinite() {
        (if v > 0.0 { "Infinity" } else { "-Infinity" }).to_string()
    } else {
        java_decimal(&format!("{v:e}"))
    }
}

fn push_control_escape(out: &mut String, c: char) -> bool {
    match c {
        '\u{8}' => out.push('b'),
        '\t' => out.push('t'),
        '\n' => out.push('n'),
        '\u{c}' => out.push('f'),
        '\r' => out.push('r'),
        c if c < ' ' => {
            let _ = write!(out, "x{:02X}", c as u32);
        }
        _ => return false,
    }
    true
}

fn quote_and_escape(out: &mut String, s: &str) {
    let mark = out.len();
    out.push(' ');
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' | '\'' => {
                let q = *quote.get_or_insert(if c == '"' { '\'' } else { '"' });
                if q == c {
                    out.push('\\');
                }
                out.push(c);
            }
            _ => {
                let before = out.len();
                out.push('\\');
                if !push_control_escape(out, c) {
                    out.truncate(before);
                    out.push(c);
                }
            }
        }
    }
    let quote = if quote == Some('\'') { "'" } else { "\"" };
    out.replace_range(mark..mark + 1, quote);
    out.push_str(quote);
}

fn is_plain_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '.' || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
        }
        _ => false,
    }
}

fn is_simple_value(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

fn snbt(out: &mut String, tag: View<'_>) {
    match tag {
        View::Byte(v) => {
            let _ = write!(out, "{v}b");
        }
        View::Short(v) => {
            let _ = write!(out, "{v}s");
        }
        View::Int(v) => {
            let _ = write!(out, "{v}");
        }
        View::Long(v) => {
            let _ = write!(out, "{v}L");
        }
        View::Float(v) => {
            out.push_str(&java_float(v));
            out.push('f');
        }
        View::Double(v) => {
            out.push_str(&java_double(v));
            out.push('d');
        }
        View::ByteArray(v) => snbt_array(out, "[B;", v.iter().map(|&b| i64::from(b as i8)), "B"),
        View::IntArray(v) => snbt_array(out, "[I;", v.iter().map(|&i| i64::from(i)), ""),
        View::LongArray(v) => snbt_array(out, "[L;", v.iter().copied(), "L"),
        View::String(s) => quote_and_escape(out, &s.to_str()),
        View::List(list) => {
            out.push('[');
            for (i, item) in list_views(list).into_iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                snbt(out, item);
            }
            out.push(']');
        }
        View::Compound(c) => {
            out.push('{');
            let mut entries: Vec<(Cow<'_, str>, &NbtTag)> =
                c.iter().map(|(k, v)| (k.to_str(), v)).collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            for (i, (key, value)) in entries.iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                if !key.eq_ignore_ascii_case("true")
                    && !key.eq_ignore_ascii_case("false")
                    && is_plain_key(key)
                {
                    out.push_str(key);
                } else {
                    quote_and_escape(out, key);
                }
                out.push(':');
                snbt(out, view(value));
            }
            out.push('}');
        }
    }
}

fn snbt_array(out: &mut String, open: &str, values: impl Iterator<Item = i64>, suffix: &str) {
    out.push_str(open);
    for (i, value) in values.enumerate() {
        if i != 0 {
            out.push(',');
        }
        let _ = write!(out, "{value}{suffix}");
    }
    out.push(']');
}

#[derive(Default)]
struct SpanPrinter {
    out: Vec<Span>,
    depth: usize,
}

impl SpanPrinter {
    fn push(&mut self, text: &str, color: Option<u32>) {
        if text.is_empty() {
            return;
        }
        let span = match color {
            Some(rgb) => Span::colored(text, rgb),
            None => Span::new(text),
        };
        if let Some(last) = self.out.last_mut() {
            if last.style == span.style {
                last.text.push_str(text);
                return;
            }
        }
        self.out.push(span);
    }

    fn number(&mut self, digits: &str, suffix: &str) {
        self.push(digits, Some(GOLD));
        self.push(suffix, Some(RED));
    }

    fn quoted(&mut self, s: &str, color: u32) {
        let mut quoted = String::new();
        quote_and_escape(&mut quoted, s);
        let quote = &quoted[..1];
        self.push(quote, None);
        self.push(&quoted[1..quoted.len() - 1], Some(color));
        self.push(quote, None);
    }

    fn sub(&mut self, tag: View<'_>) {
        self.depth += 1;
        self.visit(tag);
        self.depth -= 1;
    }

    fn visit(&mut self, tag: View<'_>) {
        match tag {
            View::String(s) => self.quoted(&s.to_str(), GREEN),
            View::Byte(v) => self.number(&v.to_string(), "b"),
            View::Short(v) => self.number(&v.to_string(), "s"),
            View::Int(v) => self.number(&v.to_string(), ""),
            View::Long(v) => self.number(&v.to_string(), "L"),
            View::Float(v) => self.number(&java_float(v), "f"),
            View::Double(v) => self.number(&java_double(v), "d"),
            View::ByteArray(v) => {
                self.array("B", v.len(), v.iter().map(|&b| (b as i8).to_string()), "b")
            }
            View::IntArray(v) => self.array("I", v.len(), v.iter().map(i32::to_string), ""),
            View::LongArray(v) => self.array("L", v.len(), v.iter().map(i64::to_string), "L"),
            View::List(list) => self.list(list),
            View::Compound(c) => self.compound(c),
        }
    }

    fn array(
        &mut self,
        prefix: &str,
        len: usize,
        items: impl Iterator<Item = String>,
        suffix: &str,
    ) {
        self.push("[", None);
        self.push(prefix, Some(RED));
        self.push(";", None);
        for (i, item) in items.take(TEXT_MAX_LENGTH).enumerate() {
            self.push(" ", None);
            self.number(&item, suffix);
            if i != len - 1 {
                self.push(",", None);
            }
        }
        if len > TEXT_MAX_LENGTH {
            self.push("<...>", Some(GRAY));
        }
        self.push("]", None);
    }

    fn list(&mut self, list: &NbtList) {
        let items = list_views(list);
        if items.is_empty() {
            self.push("[]", None);
        } else if self.depth >= TEXT_MAX_DEPTH {
            self.push("[", None);
            self.push("<...>", Some(GRAY));
            self.push("]", None);
        } else {
            self.push("[", None);
            for (i, item) in items.into_iter().enumerate() {
                if i != 0 {
                    self.push(", ", None);
                }
                self.sub(item);
            }
            self.push("]", None);
        }
    }

    fn compound(&mut self, c: &NbtCompound) {
        if c.is_empty() {
            self.push("{}", None);
        } else if self.depth >= TEXT_MAX_DEPTH {
            self.push("{", None);
            self.push("<...>", Some(GRAY));
            self.push("}", None);
        } else {
            self.push("{", None);
            let count = c.len();
            for (i, (key, value)) in c.iter().enumerate() {
                let key = key.to_str();
                if is_simple_value(&key) {
                    self.push(&key, Some(AQUA));
                } else {
                    self.quoted(&key, AQUA);
                }
                self.push(": ", None);
                self.sub(view(value));
                if i + 1 != count {
                    self.push(", ", None);
                }
            }
            self.push("}", None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> NbtTag {
        NbtTag::String(text.into())
    }

    fn ok(input: &str) -> NbtTag {
        match parse(input) {
            Ok(tag) => tag,
            Err(e) => panic!("{input:?}: {e}"),
        }
    }

    fn err(input: &str) -> Error {
        match parse(input) {
            Ok(tag) => panic!("{input:?} parsed as {tag:?}"),
            Err(e) => e,
        }
    }

    fn wrapped(tag: NbtTag) -> NbtCompound {
        NbtCompound::from_values(vec![("".into(), tag)])
    }

    #[test]
    fn integer_suffixes() {
        assert_eq!(ok("1b"), NbtTag::Byte(1));
        assert_eq!(ok("1B"), NbtTag::Byte(1));
        assert_eq!(ok("1s"), NbtTag::Short(1));
        assert_eq!(ok("1"), NbtTag::Int(1));
        assert_eq!(ok("1i"), NbtTag::Int(1));
        assert_eq!(ok("1L"), NbtTag::Long(1));
        assert_eq!(ok("-5sb"), NbtTag::Byte(-5));
        assert_eq!(ok("255ub"), NbtTag::Byte(-1));
        assert_eq!(ok("65535us"), NbtTag::Short(-1));
        assert_eq!(ok("4294967295ui"), NbtTag::Int(-1));
        assert_eq!(ok("18446744073709551615ul"), NbtTag::Long(-1));
        assert_eq!(ok("-9223372036854775808L"), NbtTag::Long(i64::MIN));
        assert_eq!(ok("1_000_000"), NbtTag::Int(1_000_000));
        assert_eq!(ok("+7"), NbtTag::Int(7));
        assert_eq!(ok("1 b"), NbtTag::Byte(1));
    }

    #[test]
    fn hex_and_binary() {
        assert_eq!(ok("0xFF"), NbtTag::Int(255));
        assert_eq!(ok("0x1b"), NbtTag::Int(27));
        assert_eq!(ok("0xFFFFFFFF"), NbtTag::Int(-1));
        assert_eq!(ok("0x80sb"), NbtTag::Byte(-128));
        assert_eq!(ok("0xFFub"), NbtTag::Byte(-1));
        assert_eq!(ok("0b101"), NbtTag::Int(5));
        assert_eq!(ok("0b1111_1111ub"), NbtTag::Byte(-1));
        assert_eq!(ok("0b"), NbtTag::Byte(0));
        assert_eq!(ok("0"), NbtTag::Int(0));
        assert_eq!(
            err("[I;-0x1]"),
            Error {
                at: 8,
                message: EXPECTED_NON_NEGATIVE.into()
            }
        );
        assert_eq!(ok("[I;-0x1si]"), NbtTag::IntArray(vec![-1]));
    }

    #[test]
    fn floats() {
        assert_eq!(ok("1.5"), NbtTag::Double(1.5));
        assert_eq!(ok("1.5f"), NbtTag::Float(1.5));
        assert_eq!(ok("1.5D"), NbtTag::Double(1.5));
        assert_eq!(ok("1d"), NbtTag::Double(1.0));
        assert_eq!(ok("2f"), NbtTag::Float(2.0));
        assert_eq!(ok("1."), NbtTag::Double(1.0));
        assert_eq!(ok(".5"), NbtTag::Double(0.5));
        assert_eq!(ok("-.5f"), NbtTag::Float(-0.5));
        assert_eq!(ok("1e3"), NbtTag::Double(1000.0));
        assert_eq!(ok("1E-2f"), NbtTag::Float(0.01));
        assert_eq!(ok("1_0.2_5"), NbtTag::Double(10.25));
        assert_eq!(parse_prefix("1e999").unwrap(), (NbtTag::Int(1), 1));
    }

    #[test]
    fn number_errors() {
        let e = err("128b");
        assert_eq!(e.at, 4);
        assert!(e.message.starts_with("Failed to parse number"), "{e}");
        assert_eq!(
            err("-1ub"),
            Error {
                at: 4,
                message: EXPECTED_NON_NEGATIVE.into()
            }
        );
        assert_eq!(err("00").at, 2);
        assert_eq!(err("1_").at, 2);
        assert_eq!(
            err("1a"),
            Error {
                at: 1,
                message: TRAILING_DATA.into()
            }
        );
        assert_eq!(
            err("1 2"),
            Error {
                at: 2,
                message: TRAILING_DATA.into()
            }
        );
    }

    #[test]
    fn arrays() {
        assert_eq!(ok("[B;1b,2b]"), NbtTag::ByteArray(vec![1, 2]));
        assert_eq!(ok("[B;1,-1]"), NbtTag::ByteArray(vec![1, 255]));
        assert_eq!(ok("[B;0xFF]"), NbtTag::ByteArray(vec![255]));
        assert_eq!(ok("[I;1,2s,3b]"), NbtTag::IntArray(vec![1, 2, 3]));
        assert_eq!(ok("[I; 255ub]"), NbtTag::IntArray(vec![-1]));
        assert_eq!(ok("[L;1,2L,]"), NbtTag::LongArray(vec![1, 2]));
        assert_eq!(ok("[I;]"), NbtTag::IntArray(vec![]));
        assert_eq!(
            err("[I;1L]"),
            Error {
                at: 6,
                message: INVALID_ARRAY_ELEMENT.into()
            }
        );
        assert_eq!(err("[B;1s]").message, INVALID_ARRAY_ELEMENT);
        assert_eq!(ok("[I]"), NbtTag::List(NbtList::String(vec!["I".into()])));
    }

    #[test]
    fn lists() {
        assert_eq!(ok("[]"), NbtTag::List(NbtList::Empty));
        assert_eq!(ok("[1,2,3,]"), NbtTag::List(NbtList::Int(vec![1, 2, 3])));
        assert_eq!(
            ok("[[1],[2b]]"),
            NbtTag::List(NbtList::List(vec![
                NbtList::Int(vec![1]),
                NbtList::Byte(vec![2])
            ]))
        );
        assert_eq!(
            ok("[1, \"a\"]"),
            NbtTag::List(NbtList::Compound(vec![
                wrapped(NbtTag::Int(1)),
                wrapped(s("a"))
            ]))
        );
        let plain = NbtCompound::from_values(vec![("a".into(), NbtTag::Int(1))]);
        assert_eq!(
            ok("[{a:1}, 2]"),
            NbtTag::List(NbtList::Compound(vec![
                plain.clone(),
                wrapped(NbtTag::Int(2))
            ]))
        );
        assert_eq!(ok("[{a:1}]"), NbtTag::List(NbtList::Compound(vec![plain])));
        assert!(parse("[1,,2]").is_err());
    }

    #[test]
    fn compounds() {
        let tag = ok("{ a : 1 , 'b c' : \"x\" , }");
        let NbtTag::Compound(c) = tag else { panic!() };
        assert_eq!(c.get("a"), Some(&NbtTag::Int(1)));
        assert_eq!(c.get("b c"), Some(&s("x")));
        let NbtTag::Compound(c) = ok("{a:1,a:2}") else {
            panic!()
        };
        assert_eq!(c.len(), 1);
        assert_eq!(c.get("a"), Some(&NbtTag::Int(2)));
        assert_eq!(
            ok("{1:2}"),
            NbtTag::Compound(NbtCompound::from_values(vec![("1".into(), NbtTag::Int(2))]))
        );
        assert_eq!(err("{\"\":1}").at, 5);
        assert!(parse("{,}").is_err());
        assert_eq!(parse_prefix("{a:1} rest").unwrap().1, 5);
    }

    #[test]
    fn strings() {
        assert_eq!(ok("abc"), s("abc"));
        assert_eq!(ok("a.b+c-d_e"), s("a.b+c-d_e"));
        assert_eq!(ok("'a\"b'"), s("a\"b"));
        assert_eq!(ok("\"a'b\""), s("a'b"));
        assert_eq!(ok(r#""a\nb\tc\\d\"e\'f\sg""#), s("a\nb\tc\\d\"e'f g"));
        assert_eq!(ok(r#""\x41\u0042\U00000043""#), s("ABC"));
        assert_eq!(ok(r#""\uD83D\uDE00""#), s("\u{1F600}"));
        assert_eq!(ok("\"\""), s(""));
        assert_eq!(err(r#""\q""#).at, 2);
        assert!(err(r#""\x4""#).message.contains("length 2"));
        assert!(err(r#""\U00110000""#).message.contains("U+00110000"));
        assert!(err(r#""\N{SNOWMAN}""#).message.contains("SNOWMAN"));
        assert_eq!(err("\"abc").at, 4);
        assert_eq!(err("+abc").message, INVALID_UNQUOTED_START);
    }

    #[test]
    fn booleans_and_builtins() {
        assert_eq!(ok("true"), NbtTag::Byte(1));
        assert_eq!(ok("FALSE"), NbtTag::Byte(0));
        assert_eq!(ok("bool(5)"), NbtTag::Byte(1));
        assert_eq!(ok("bool(0.0)"), NbtTag::Byte(0));
        assert_eq!(ok("bool(true)"), NbtTag::Byte(1));
        assert_eq!(err("bool(\"x\")").message, EXPECTED_NUMBER_OR_BOOLEAN);
        assert_eq!(
            ok("uuid(\"f81d4fae-7dec-11d0-a765-00a0c91e6bf6\")"),
            NbtTag::IntArray(vec![
                0xf81d4fae_u32 as i32,
                0x7dec11d0,
                0xa76500a0_u32 as i32,
                0xc91e6bf6_u32 as i32
            ])
        );
        assert_eq!(err("uuid(\"nope\")").message, EXPECTED_STRING_UUID);
        assert_eq!(
            err("foo(1)"),
            Error {
                at: 6,
                message: "No such operation: foo/1".into()
            }
        );
        assert_eq!(ok("uuid"), s("uuid"));
    }

    #[test]
    fn depth_limit() {
        let run = std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let e = err(&"[".repeat(600));
            assert_eq!(e.at, MAX_DEPTH);
            assert!(e.message.contains("depth"), "{e}");
            let deep = format!("{}{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
            assert!(parse(&deep).is_ok());
        });
        run.unwrap().join().unwrap();
    }

    #[test]
    fn printing() {
        assert_eq!(to_snbt(&ok("{b:1,a:'x'}")), r#"{a:"x",b:1}"#);
        assert_eq!(
            to_snbt(&ok("{\"true\":1b,\"a b\":2s,c:3L}")),
            r#"{"a b":2s,c:3L,"true":1b}"#
        );
        assert_eq!(to_snbt(&NbtTag::Float(1.0)), "1.0f");
        assert_eq!(to_snbt(&NbtTag::Float(0.1)), "0.1f");
        assert_eq!(to_snbt(&NbtTag::Double(1e10)), "1.0E10d");
        assert_eq!(to_snbt(&NbtTag::Double(1e7)), "1.0E7d");
        assert_eq!(to_snbt(&NbtTag::Double(123456.789)), "123456.789d");
        assert_eq!(to_snbt(&NbtTag::Double(0.001)), "0.001d");
        assert_eq!(to_snbt(&NbtTag::Double(1e-4)), "1.0E-4d");
        assert_eq!(to_snbt(&NbtTag::Double(-0.0)), "-0.0d");
        assert_eq!(to_snbt(&NbtTag::Double(-2.5e-7)), "-2.5E-7d");
        assert_eq!(to_snbt(&s("a'b")), r#""a'b""#);
        assert_eq!(to_snbt(&s("a\"b")), r#"'a"b'"#);
        assert_eq!(to_snbt(&s("it's \"x\"")), r#""it's \"x\"""#);
        assert_eq!(to_snbt(&s("a\\b\n\u{1}")), r#""a\\b\n\x01""#);
        assert_eq!(to_snbt(&NbtTag::ByteArray(vec![1, 255])), "[B;1B,-1B]");
        assert_eq!(to_snbt(&NbtTag::IntArray(vec![1, -2])), "[I;1,-2]");
        assert_eq!(to_snbt(&NbtTag::LongArray(vec![1])), "[L;1L]");
        assert_eq!(to_snbt(&ok("[1, \"a\", {b:2}]")), r#"[1,"a",{b:2}]"#);
    }

    #[test]
    fn round_trip() {
        let input = r#"{name:"Steve",pos:[1.5d,2.0d,-3.25d],flags:[B;1B,0B],ids:[I;1,2,3],
            big:[L;9223372036854775807L],mixed:[1,"two",{three:3},[4]],nested:{deep:[[1b],[2s]]},
            f:0.5f,tiny:1.0E-10d,"quoted key":'it\'s',esc:"\t\x7F\u00e9",empty:{},none:[]}"#;
        let first = ok(input);
        let printed = to_snbt(&first);
        let second = ok(&printed);
        assert_eq!(printed, to_snbt(&second));
    }

    #[test]
    fn spans() {
        let spans = to_spans(&ok("{a:1b,s:'x',l:[1,2]}"));
        let text: String = spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(text, r#"{a: 1b, s: "x", l: [1, 2]}"#);
        assert!(spans.contains(&Span::colored("a", AQUA)));
        assert!(spans.contains(&Span::colored("1", GOLD)));
        assert!(spans.contains(&Span::colored("b", RED)));
        assert!(spans.contains(&Span::colored("x", GREEN)));
        let text: String = to_spans(&NbtTag::ByteArray(vec![1, 2]))
            .iter()
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(text, "[B; 1b, 2b]");
        let NbtTag::Compound(c) = ok("{}") else {
            panic!()
        };
        let text: String = compound_to_spans(&c)
            .iter()
            .map(|span| span.text.as_str())
            .collect();
        assert_eq!(text, "{}");
        assert_eq!(compound_to_snbt(&c), "{}");
    }
}
