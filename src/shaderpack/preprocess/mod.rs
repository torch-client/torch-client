use std::borrow::Cow;
use std::collections::HashMap;

mod condition;

use condition::eval;

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

    fn undefine(&mut self, name: &str) {
        self.map.remove(name);
    }

    pub(crate) fn is_defined(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.map.get(name).map(String::as_str)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.map
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
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
                stack.pop();
                emit(&mut out, None);
            }
            "define" if active => {
                let (ident, value) = split_directive(strip_comment(rest).trim());
                if ident.is_empty() {
                    return Err(err(line, "#define names nothing"));
                }
                if defines.is_defined(ident) {
                    out.push_str(&format!("#undef {ident}\n"));
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

pub(crate) fn logical_lines(source: &str) -> impl Iterator<Item = (Cow<'_, str>, usize, usize)> {
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
    fn an_exponential_macro_is_stopped() {
        let mut table = Defines::new();
        let names: Vec<String> = (0..30).map(|i| format!("M{i}")).collect();
        for pair in names.windows(2) {
            table.define(pair[0].clone(), format!("{0}+{0}", pair[1]));
        }
        table.define(names[29].clone(), "1");
        let error = preprocess("#if M0 > 0\nx\n#endif\n", &mut table).unwrap_err();
        assert!(error.message.contains("more than"), "{error:?}");
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
    fn a_redefinition_forgets_the_old_value_first() {
        let out = run("#define A 0\n#define A 1\n#if A == 1\nok\n#endif\n", &[]);
        assert_eq!(out, "#define A 0\n#undef A\n#define A 1\n\nok\n\n");
    }

    #[test]
    fn a_decimal_in_a_condition_reads_its_integer_part() {
        let source = "#if F == 1 && G == 2 && 1.5e2 == 100 && 25e-1 == 2\nok\n#endif\n";
        assert_eq!(run(source, &[("F", "1.0"), ("G", "2.5")]), "\nok\n\n");
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
        assert_eq!(preprocess("#endif\nx\n", &mut table).unwrap(), "\nx\n");

        table.define("SHADOW_DISTANCE", "\"far\"");
        assert!(preprocess("#if SHADOW_DISTANCE > 1\nx\n#endif\n", &mut table).is_err());
    }
}
