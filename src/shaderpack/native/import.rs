use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use super::problem::Problem;

pub(crate) const MAX_DEPTH: usize = 16;

const DIRECTIVE: &str = "#import";

pub(crate) type Read<'a> = dyn Fn(&str) -> Option<Arc<str>> + 'a;

#[derive(Clone, Debug)]
pub(crate) struct File {
    pub(crate) path: Arc<str>,
    pub(crate) text: Arc<str>,
    directives: Vec<Directive>,
}

impl File {
    pub(crate) fn write_blanked(&self, out: &mut String) {
        let mut from = 0;
        for directive in &self.directives {
            out.push_str(&self.text[from..directive.line.start]);
            out.extend(std::iter::repeat_n(' ', directive.line.len()));
            from = directive.line.end;
        }
        out.push_str(&self.text[from..]);
    }
}

#[derive(Clone, Debug)]
struct Directive {
    line: Range<usize>,
    at: usize,
    path: String,
}

pub(crate) fn resolve(root: &str, read: &Read<'_>) -> Result<Vec<File>, Vec<Problem>> {
    let Some(text) = read(root) else {
        return Err(vec![Problem::file(root, "no such file in the pack")]);
    };
    let mut walk = Walk {
        read,
        imports: Vec::new(),
        seen: HashSet::new(),
        stack: Vec::new(),
        problems: Vec::new(),
    };
    let root = walk.load(Arc::from(root), text);
    walk.descend(&root);
    if !walk.problems.is_empty() {
        return Err(walk.problems);
    }
    let mut files = walk.imports;
    files.push(root);
    Ok(files)
}

struct Walk<'r, 'a> {
    read: &'r Read<'a>,
    imports: Vec<File>,
    seen: HashSet<Arc<str>>,
    stack: Vec<Arc<str>>,
    problems: Vec<Problem>,
}

impl Walk<'_, '_> {
    fn load(&mut self, path: Arc<str>, text: Arc<str>) -> File {
        let (directives, problems) = scan(&path, &text);
        self.problems.extend(problems);
        File {
            path,
            text,
            directives,
        }
    }

    fn descend(&mut self, file: &File) {
        self.seen.insert(Arc::clone(&file.path));
        self.stack.push(Arc::clone(&file.path));
        for directive in &file.directives {
            self.visit(file, directive);
        }
        self.stack.pop();
    }

    fn visit(&mut self, from: &File, directive: &Directive) {
        let at = |message: String| Problem::at(&*from.path, &from.text, directive.at, message);
        let path = directive.path.as_str();
        if let Err(why) = check_path(path) {
            self.problems.push(at(why));
            return;
        }
        if let Some(start) = self.stack.iter().position(|p| **p == *path) {
            let chain: Vec<&str> = self.stack[start..]
                .iter()
                .map(|p| &**p)
                .chain([path])
                .collect();
            self.problems
                .push(at(format!("import cycle: {}", chain.join(" -> "))));
            return;
        }
        if self.seen.contains(path) {
            return;
        }
        if self.stack.len() > MAX_DEPTH {
            self.problems
                .push(at(format!("imports nest more than {MAX_DEPTH} deep here")));
            return;
        }
        let Some(text) = (self.read)(path) else {
            self.problems
                .push(at(format!("no such file in the pack: `{path}`")));
            return;
        };
        let file = self.load(Arc::from(path), text);
        self.imports.push(file.clone());
        self.descend(&file);
    }
}

enum Line<'t> {
    Directive { indent: usize, path: &'t str },
    Malformed { indent: usize, why: &'static str },
    Other,
}

fn classify(body: &str) -> Line<'_> {
    let trimmed = body.trim_start();
    let indent = body.len() - trimmed.len();
    let Some(rest) = trimmed.strip_prefix(DIRECTIVE) else {
        return Line::Other;
    };
    let after = rest.trim_start();
    if after.len() == rest.len() {
        return Line::Malformed {
            indent,
            why: "expected `#import \"path\"`",
        };
    }
    let Some((path, tail)) = after
        .strip_prefix('"')
        .and_then(|quoted| quoted.split_once('"'))
    else {
        return Line::Malformed {
            indent,
            why: "expected `#import \"path\"`, with the path in double quotes",
        };
    };
    if !tail.trim().is_empty() {
        return Line::Malformed {
            indent,
            why: "`#import` must stand alone on its line",
        };
    }
    Line::Directive { indent, path }
}

fn scan(file: &str, text: &str) -> (Vec<Directive>, Vec<Problem>) {
    let mut directives = Vec::new();
    let mut problems = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for raw in text.split_inclusive('\n') {
        let body = raw.strip_suffix('\n').unwrap_or(raw);
        let line = if depth == 0 {
            classify(body)
        } else {
            Line::Other
        };
        match line {
            Line::Directive { indent, path } => directives.push(Directive {
                line: start..start + body.len(),
                at: start + indent,
                path: path.to_owned(),
            }),
            Line::Malformed { indent, why } => {
                problems.push(Problem::at(file, text, start + indent, why))
            }
            Line::Other => {
                if let Some(column) = code_directive(body, &mut depth) {
                    problems.push(
                        Problem::at(
                            file,
                            text,
                            start + column,
                            "`#import` must stand alone on its line",
                        )
                        .hint("put it on a line of its own, before or after this one"),
                    );
                }
            }
        }
        start += raw.len();
    }
    (directives, problems)
}

fn code_directive(line: &str, depth: &mut usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut found = None;
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        if rest.starts_with(b"/*") {
            *depth += 1;
            i += 2;
        } else if *depth > 0 {
            if rest.starts_with(b"*/") {
                *depth -= 1;
                i += 2;
            } else {
                i += 1;
            }
        } else if rest.starts_with(b"//") {
            break;
        } else {
            if found.is_none() && rest.starts_with(DIRECTIVE.as_bytes()) {
                found = Some(i);
            }
            i += 1;
        }
    }
    found
}

fn check_path(path: &str) -> Result<(), String> {
    if !super::package::is_plain_path(path) {
        return Err(format!("`{path}` is not a plain path from the pack root"));
    }
    if !path.ends_with(".wgsl") {
        return Err(format!("`{path}` is not a `.wgsl` file"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn memory_reader<'a>(
    files: &'a [(&'a str, &'a str)],
) -> impl Fn(&str) -> Option<Arc<str>> + 'a {
    move |path: &str| {
        files
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, text)| Arc::from(*text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::native::problem::Position;

    fn paths(files: &[File]) -> Vec<&str> {
        files.iter().map(|f| &*f.path).collect()
    }

    #[test]
    fn imports_come_in_first_use_order_once_each() {
        let files = [
            (
                "main.wgsl",
                "#import \"a.wgsl\"\n#import \"b.wgsl\"\nfn main_fn() {}\n",
            ),
            ("a.wgsl", "#import \"c.wgsl\"\nfn a() {}\n"),
            (
                "b.wgsl",
                "  #import \"c.wgsl\"\n#import \"a.wgsl\"\nfn b() {}\n",
            ),
            ("c.wgsl", "fn c() {}"),
        ];
        let read = memory_reader(&files);
        let resolved = resolve("main.wgsl", &read).unwrap();
        assert_eq!(
            paths(&resolved),
            ["a.wgsl", "c.wgsl", "b.wgsl", "main.wgsl"]
        );
    }

    #[test]
    fn blanking_keeps_every_offset() {
        let files = [
            (
                "main.wgsl",
                "#import \"a.wgsl\"\r\nfn f() {}\n    #import \"a.wgsl\"\n",
            ),
            ("a.wgsl", ""),
        ];
        let read = memory_reader(&files);
        let resolved = resolve("main.wgsl", &read).unwrap();
        let root = resolved.last().unwrap();
        let mut out = String::new();
        root.write_blanked(&mut out);
        assert_eq!(out.len(), root.text.len());
        assert!(!out.contains('#'));
        let breaks = |s: &str| s.match_indices('\n').map(|(i, _)| i).collect::<Vec<_>>();
        assert_eq!(breaks(&out), breaks(&root.text));
        assert_eq!(out.find("fn f"), root.text.find("fn f"));
    }

    #[test]
    fn a_cycle_names_the_chain() {
        let files = [
            ("main.wgsl", "#import \"a.wgsl\"\n"),
            ("a.wgsl", "#import \"b.wgsl\"\n"),
            ("b.wgsl", "\n#import \"a.wgsl\"\n"),
        ];
        let read = memory_reader(&files);
        let problems = resolve("main.wgsl", &read).unwrap_err();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].file, "b.wgsl");
        assert_eq!(problems[0].at, Some(Position { line: 2, column: 1 }));
        assert!(
            problems[0].message.contains("a.wgsl -> b.wgsl -> a.wgsl"),
            "{}",
            problems[0].message
        );

        let own = [("main.wgsl", "#import \"main.wgsl\"\n")];
        let read = memory_reader(&own);
        let problems = resolve("main.wgsl", &read).unwrap_err();
        assert!(
            problems[0].message.contains("main.wgsl -> main.wgsl"),
            "{}",
            problems[0].message
        );
    }

    #[test]
    fn imports_nest_at_most_sixteen_deep() {
        let chain = |last: usize| -> Vec<(String, String)> {
            (0..=last)
                .map(|n| {
                    let text = if n < last {
                        format!("#import \"f{}.wgsl\"\n", n + 1)
                    } else {
                        String::new()
                    };
                    (format!("f{n}.wgsl"), text)
                })
                .collect()
        };
        let run = |last: usize| {
            let owned = chain(last);
            let files: Vec<(&str, &str)> = owned
                .iter()
                .map(|(p, t)| (p.as_str(), t.as_str()))
                .collect();
            let read = memory_reader(&files);
            resolve("f0.wgsl", &read).map(|f| f.len())
        };
        assert_eq!(run(MAX_DEPTH), Ok(MAX_DEPTH + 1));
        let problems = run(MAX_DEPTH + 1).unwrap_err();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].file, format!("f{MAX_DEPTH}.wgsl"));
        assert!(problems[0].message.contains("deep"));
    }

    #[test]
    fn a_directive_stands_alone_and_comments_hide_it() {
        let files = [
            (
                "main.wgsl",
                "fn f() {} #import \"a.wgsl\"\n// #import \"a.wgsl\"\n\
                 /* one /* two */\n#import \"a.wgsl\"\n*/\n\
                 #import \"a.wgsl\" // why\n\
                 #importx\n",
            ),
            ("a.wgsl", ""),
        ];
        let read = memory_reader(&files);
        let problems = resolve("main.wgsl", &read).unwrap_err();
        let at: Vec<Option<Position>> = problems.iter().map(|p| p.at).collect();
        assert_eq!(
            at,
            [
                Some(Position {
                    line: 1,
                    column: 11
                }),
                Some(Position { line: 6, column: 1 }),
                Some(Position { line: 7, column: 1 }),
            ],
            "{problems:?}"
        );
        assert!(problems[1].message.contains("stand alone"));
    }

    #[test]
    fn paths_are_plain_and_name_wgsl_files() {
        for bad in [
            "../a.wgsl",
            "/a.wgsl",
            "a\\b.wgsl",
            "lib//a.wgsl",
            ".hidden.wgsl",
            "a.glsl",
            "",
        ] {
            let text = format!("#import \"{bad}\"\n");
            let files = [("main.wgsl", text.as_str())];
            let read = memory_reader(&files);
            let problems = resolve("main.wgsl", &read).unwrap_err();
            assert_eq!(problems.len(), 1, "{bad}: {problems:?}");
        }
        let files = [
            ("main.wgsl", "#import \"lib/noise-2.v1.wgsl\"\n"),
            ("lib/noise-2.v1.wgsl", ""),
        ];
        let read = memory_reader(&files);
        assert!(resolve("main.wgsl", &read).is_ok());
    }

    #[test]
    fn a_missing_file_is_reported_where_it_is_imported() {
        let files = [("main.wgsl", "fn f() {}\n  #import \"gone.wgsl\"\n")];
        let read = memory_reader(&files);
        let problems = resolve("main.wgsl", &read).unwrap_err();
        assert_eq!(problems[0].file, "main.wgsl");
        assert_eq!(problems[0].at, Some(Position { line: 2, column: 3 }));
        assert_eq!(resolve("nothing.wgsl", &read).unwrap_err()[0].at, None);
    }
}
