use std::collections::HashMap;
use std::sync::Arc;

use super::source::Source;

const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Origin {
    pub(crate) at: u32,
    pub(crate) file: Arc<str>,
    pub(crate) line: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Expanded {
    pub(crate) text: String,
    pub(crate) origins: Vec<Origin>,
}

impl Expanded {
    pub(crate) fn locate(&self, line: u32) -> Option<(&str, u32)> {
        let at = self
            .origins
            .partition_point(|o| o.at <= line)
            .checked_sub(1)?;
        let origin = &self.origins[at];
        Some((&origin.file, origin.line + (line - origin.at)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Error {
    pub(crate) file: String,
    pub(crate) line: u32,
    pub(crate) message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

pub(crate) type Cache = HashMap<Arc<str>, Arc<Expanded>>;

pub(crate) fn expand(
    source: &Source,
    path: &str,
    cache: &mut Cache,
) -> Result<Arc<Expanded>, Error> {
    let mut stack = Vec::new();
    expand_inner(source, path, cache, &mut stack)
}

fn expand_inner(
    source: &Source,
    path: &str,
    cache: &mut Cache,
    stack: &mut Vec<Arc<str>>,
) -> Result<Arc<Expanded>, Error> {
    let path: Arc<str> = Arc::from(path);

    if let Some(done) = cache.get(&path) {
        return Ok(Arc::clone(done));
    }
    if stack.contains(&path) {
        let chain: Vec<&str> = stack
            .iter()
            .map(Arc::as_ref)
            .chain([path.as_ref()])
            .collect();
        return Err(Error {
            file: path.to_string(),
            line: 0,
            message: format!("include cycle: {}", chain.join(" -> ")),
        });
    }
    if stack.len() >= MAX_DEPTH {
        return Err(Error {
            file: path.to_string(),
            line: 0,
            message: format!("includes nest more than {MAX_DEPTH} deep"),
        });
    }

    let source_text = source.read_text(&path).ok_or_else(|| Error {
        file: stack
            .last()
            .map_or_else(|| path.to_string(), ToString::to_string),
        line: 0,
        message: format!("no such file in the pack: {path}"),
    })?;

    stack.push(Arc::clone(&path));
    let expanded = build(source, &path, &source_text, cache, stack);
    stack.pop();

    let expanded = Arc::new(expanded?);
    cache.insert(path, Arc::clone(&expanded));
    Ok(expanded)
}

fn build(
    source: &Source,
    path: &Arc<str>,
    text: &str,
    cache: &mut Cache,
    stack: &mut Vec<Arc<str>>,
) -> Result<Expanded, Error> {
    let mut out = String::with_capacity(text.len());
    let mut origins = Vec::new();
    let mut run: Option<(u32, u32)> = None;
    let mut out_line = 1u32;

    for (index, line) in text.lines().enumerate() {
        let line_no = index as u32 + 1;

        let Some(target) = included(line) else {
            if run.is_none() {
                run = Some((out_line, line_no));
                origins.push(Origin {
                    at: out_line,
                    file: Arc::clone(path),
                    line: line_no,
                });
            }
            out.push_str(line);
            out.push('\n');
            out_line += 1;
            continue;
        };

        let target = absolute(&target, path);
        let nested = expand_inner(source, &target, cache, stack).map_err(|mut e| {
            if e.line == 0 && e.file != *target {
                e.file = path.to_string();
                e.line = line_no;
            }
            e
        })?;

        for origin in &nested.origins {
            origins.push(Origin {
                at: out_line + origin.at - 1,
                file: Arc::clone(&origin.file),
                line: origin.line,
            });
        }
        out.push_str(&nested.text);
        out_line += nested.text.lines().count() as u32;
        run = None;
    }

    Ok(Expanded { text: out, origins })
}

fn included(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start();
    let rest = rest.strip_prefix("include")?;
    if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let rest = rest.trim_start();
    let (open, close) = match rest.as_bytes().first()? {
        b'"' => ('"', '"'),
        b'<' => ('<', '>'),
        _ => return None,
    };
    let inner = rest.strip_prefix(open)?;
    let end = inner.find(close)?;
    Some(inner[..end].to_owned())
}

fn absolute(target: &str, from: &Arc<str>) -> String {
    if target.starts_with('/') {
        return target.to_owned();
    }
    let dir = match from.rfind('/') {
        Some(at) => &from[..at],
        None => "",
    };
    format!("{dir}/{target}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn pack(name: &str, files: &[(&str, &str)]) -> Source {
        let root = std::env::temp_dir().join("torch-client-tests").join(name);
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in files {
            let at: PathBuf = root.join("shaders").join(path.trim_start_matches('/'));
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        Source::open(&root).unwrap()
    }

    fn expand_one(source: &Source, path: &str) -> Result<Arc<Expanded>, Error> {
        expand(source, path, &mut Cache::new())
    }

    #[test]
    fn an_include_is_replaced_by_the_file_it_names() {
        let source = pack(
            "inc-basic",
            &[
                ("/programs/a.glsl", "top\n#include \"/lib/b.glsl\"\ntail\n"),
                ("/lib/b.glsl", "body\n"),
            ],
        );
        let out = expand_one(&source, "/programs/a.glsl").unwrap();
        assert_eq!(out.text, "top\nbody\ntail\n");
    }

    #[test]
    fn a_file_included_twice_appears_twice() {
        let source = pack(
            "inc-diamond",
            &[
                ("/a.glsl", "#include \"/c.glsl\"\n#include \"/c.glsl\"\n"),
                ("/c.glsl", "shared\n"),
            ],
        );
        assert_eq!(
            expand_one(&source, "/a.glsl").unwrap().text,
            "shared\nshared\n"
        );
    }

    #[test]
    fn a_cycle_is_an_error_naming_the_chain() {
        let source = pack(
            "inc-cycle",
            &[
                ("/a.glsl", "#include \"/b.glsl\"\n"),
                ("/b.glsl", "#include \"/a.glsl\"\n"),
            ],
        );
        let error = expand_one(&source, "/a.glsl").unwrap_err();
        assert!(
            error.message.contains("/a.glsl -> /b.glsl -> /a.glsl"),
            "{error}"
        );
    }

    #[test]
    fn a_missing_include_is_reported_against_the_line_that_asked() {
        let source = pack(
            "inc-missing",
            &[("/a.glsl", "x\n#include \"/gone.glsl\"\n")],
        );
        let error = expand_one(&source, "/a.glsl").unwrap_err();
        assert_eq!((error.file.as_str(), error.line), ("/a.glsl", 2));
    }

    #[test]
    fn relative_and_absolute_targets_both_resolve() {
        let source = pack(
            "inc-relative",
            &[
                (
                    "/lib/a.glsl",
                    "#include \"sub/b.glsl\"\n#include \"/lib/c.glsl\"\n",
                ),
                ("/lib/sub/b.glsl", "b\n"),
                ("/lib/c.glsl", "c\n"),
            ],
        );
        assert_eq!(expand_one(&source, "/lib/a.glsl").unwrap().text, "b\nc\n");
    }

    #[test]
    fn expanded_lines_locate_back_to_their_own_file() {
        let source = pack(
            "inc-origins",
            &[
                ("/a.glsl", "one\n#include \"/b.glsl\"\nfour\n"),
                ("/b.glsl", "two\nthree\n"),
            ],
        );
        let out = expand_one(&source, "/a.glsl").unwrap();
        assert_eq!(out.text, "one\ntwo\nthree\nfour\n");
        assert_eq!(out.locate(1), Some(("/a.glsl", 1)));
        assert_eq!(out.locate(2), Some(("/b.glsl", 1)));
        assert_eq!(out.locate(3), Some(("/b.glsl", 2)));
        assert_eq!(out.locate(4), Some(("/a.glsl", 3)));
    }

    #[test]
    fn a_shared_file_is_expanded_once_and_reused() {
        let source = pack(
            "inc-cache",
            &[
                ("/a.glsl", "#include \"/lib.glsl\"\n"),
                ("/b.glsl", "#include \"/lib.glsl\"\n"),
                ("/lib.glsl", "shared\n"),
            ],
        );
        let mut cache = Cache::new();
        let first = expand(&source, "/a.glsl", &mut cache).unwrap();
        let before = Arc::as_ptr(cache.get("/lib.glsl").unwrap());
        let second = expand(&source, "/b.glsl", &mut cache).unwrap();

        assert_eq!(first.text, second.text);
        assert_eq!(before, Arc::as_ptr(cache.get("/lib.glsl").unwrap()));
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn a_real_pack_expands() {
        let packs = crate::shaderpack::discover::list();
        assert!(
            !packs.is_empty(),
            "no pack in {}",
            super::super::discover::dir().display()
        );

        for pack in packs {
            let at = crate::shaderpack::discover::dir().join(&pack.name);
            let source = Source::open(&at).unwrap_or_else(|e| panic!("{}: {e}", pack.name));
            let mut cache = Cache::new();

            let entry = "/final.fsh";
            assert!(source.contains(entry), "{}: no {entry}", pack.name);
            let out = expand(&source, entry, &mut cache).unwrap_or_else(|e| panic!("{e}"));

            let lines = out.text.lines().count();
            println!(
                "{}: {entry} -> {lines} lines from {} files, {} cached expansions",
                pack.name,
                out.origins.len(),
                cache.len()
            );

            assert!(
                lines > 100,
                "{}: {entry} expanded to {lines} lines",
                pack.name
            );
            for origin in &out.origins {
                assert!(
                    source.contains(&origin.file),
                    "{}: {}",
                    pack.name,
                    origin.file
                );
            }
        }
    }

    #[test]
    #[ignore = "needs a shader pack installed"]
    fn a_real_pack_preprocesses() {
        use super::super::preprocess::{Defines, preprocess};

        for pack in crate::shaderpack::discover::list() {
            let at = crate::shaderpack::discover::dir().join(&pack.name);
            let Ok(source) = Source::open(&at) else {
                continue;
            };
            let mut cache = Cache::new();
            let (mut ok, mut failed) = (0usize, Vec::new());

            let mut programs: Vec<String> = source
                .files()
                .into_iter()
                .filter(|f| {
                    [".vsh", ".fsh", ".gsh", ".csh"]
                        .iter()
                        .any(|e| f.ends_with(e))
                })
                .collect();
            programs.sort();

            for name in &programs {
                let expanded = expand(&source, name, &mut cache).unwrap_or_else(|e| panic!("{e}"));
                let mut defines = Defines::new();
                defines.define("MC_VERSION", "12101");
                match preprocess(&expanded.text, &mut defines) {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        let (file, line) = expanded.locate(e.line as u32).unwrap_or((name, 0));
                        failed.push(format!("{name}: {file}:{line}: {}", e.message));
                    }
                }
            }
            println!(
                "{}: {}/{} programs preprocessed, {} distinct files expanded",
                pack.name,
                ok,
                programs.len(),
                cache.len()
            );
            let mut causes: Vec<&String> = failed.iter().collect();
            causes.sort();
            causes.dedup_by_key(|f| f.split_once(": ").map(|(_, rest)| rest.to_owned()));
            for line in causes.iter().take(10) {
                println!("    {line}");
            }
            assert!(
                failed.is_empty(),
                "{} of {} programs failed",
                failed.len(),
                programs.len()
            );
        }
    }

    #[test]
    fn a_line_that_only_looks_like_an_include_is_not_one() {
        assert_eq!(included("#include \"/a.glsl\""), Some("/a.glsl".into()));
        assert_eq!(included("  #  include <a.glsl>"), Some("a.glsl".into()));
        assert_eq!(included("#includes \"/a.glsl\""), None);
        assert_eq!(included("// #include \"/a.glsl\""), None);
        assert_eq!(included("#include"), None);
    }
}
