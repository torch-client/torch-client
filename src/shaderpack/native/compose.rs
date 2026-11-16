use std::ops::Range;

use super::api::{self, ModuleKind, PackInterface};
use super::import::{self, File};
use super::manifest;
use super::problem::{Position, Problem};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnginePart {
    Prelude,
    Entries,
}

impl EnginePart {
    fn name(self) -> &'static str {
        match self {
            EnginePart::Prelude => "prelude",
            EnginePart::Entries => "entry points",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Engine(EnginePart),
    File(usize),
}

#[derive(Clone, Debug)]
struct Part {
    range: Range<usize>,
    origin: Origin,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Located<'a> {
    Pack { file: &'a File, offset: usize },
    Engine { part: EnginePart, at: Position },
}

#[derive(Clone, Debug)]
pub(crate) struct Composed {
    text: String,
    parts: Vec<Part>,
    files: Vec<File>,
    entries_start: usize,
    entry_points: Vec<String>,
    bindings: Vec<api::Binding>,
}

pub(crate) fn compose(
    pack: &PackInterface,
    file: &str,
    kind: &ModuleKind,
    read: &import::Read<'_>,
) -> Result<Composed, Vec<Problem>> {
    if let Some(function) = kind.function()
        && !api::is_identifier(function)
    {
        return Err(vec![Problem::file(
            manifest::FILE,
            format!(
                "the pass `{function}` cannot be called from WGSL: its id must be a WGSL identifier that does not start with `torch_`"
            ),
        )]);
    }
    let files = import::resolve(file, read)?;
    let bindings = api::bindings(pack, kind);
    let prelude = api::prelude(pack, &bindings);
    let entries = api::entries(pack, kind);

    let capacity = prelude.len()
        + entries.text.len()
        + files.iter().map(|f| f.text.len() + 1).sum::<usize>()
        + 2;
    let mut text = String::with_capacity(capacity);
    let mut parts = Vec::with_capacity(files.len() + 2);
    push_part(
        &mut text,
        &mut parts,
        Origin::Engine(EnginePart::Prelude),
        |t| t.push_str(&prelude),
    );
    for (index, f) in files.iter().enumerate() {
        push_part(&mut text, &mut parts, Origin::File(index), |t| {
            f.write_blanked(t)
        });
    }
    let entries_start = text.len();
    push_part(
        &mut text,
        &mut parts,
        Origin::Engine(EnginePart::Entries),
        |t| t.push_str(&entries.text),
    );

    Ok(Composed {
        text,
        parts,
        files,
        entries_start,
        entry_points: entries.names,
        bindings,
    })
}

fn push_part(
    text: &mut String,
    parts: &mut Vec<Part>,
    origin: Origin,
    write: impl FnOnce(&mut String),
) {
    let start = text.len();
    write(text);
    parts.push(Part {
        range: start..text.len(),
        origin,
    });
    text.push('\n');
}

impl Composed {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn without_entries(&self) -> &str {
        &self.text[..self.entries_start]
    }

    pub(crate) fn file(&self) -> &str {
        &self.files[self.files.len() - 1].path
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|f| &*f.path)
    }

    pub(crate) fn entry_points(&self) -> &[String] {
        &self.entry_points
    }

    pub(crate) fn bindings(&self) -> &[api::Binding] {
        &self.bindings
    }

    pub(crate) fn locate(&self, offset: usize) -> Located<'_> {
        let index = self
            .parts
            .partition_point(|p| p.range.start <= offset)
            .saturating_sub(1);
        let part = &self.parts[index];
        let within = offset.clamp(part.range.start, part.range.end) - part.range.start;
        match part.origin {
            Origin::File(file) => Located::Pack {
                file: &self.files[file],
                offset: within,
            },
            Origin::Engine(engine) => Located::Engine {
                part: engine,
                at: Position::of(&self.text[part.range.clone()], within),
            },
        }
    }

    pub(crate) fn in_pack(&self, span: naga::Span) -> bool {
        span.to_range()
            .is_some_and(|range| matches!(self.locate(range.start), Located::Pack { .. }))
    }

    pub(crate) fn problem_at(&self, offset: usize, message: impl Into<String>) -> Problem {
        match self.locate(offset) {
            Located::Pack { file, offset } => Problem::at(&*file.path, &file.text, offset, message),
            Located::Engine { part, at } => self.engine_problem(part, at, message.into()),
        }
    }

    pub(crate) fn span_problem(&self, span: naga::Span, message: impl Into<String>) -> Problem {
        match span.to_range() {
            Some(range) => self.problem_at(range.start, message),
            None => Problem::file(self.file(), message),
        }
    }

    fn engine_problem(&self, part: EnginePart, at: Position, message: String) -> Problem {
        Problem::file(
            self.file(),
            format!("{message} (in the engine's {} at {}:{})", part.name(), at.line, at.column),
        )
        .hint("the client's own WGSL failed with this file: a client bug, unless another problem with this file explains it")
    }

    pub(crate) fn parse_problem(&self, error: &naga::front::wgsl::ParseError) -> Problem {
        let labels: Vec<(usize, &str)> = error
            .labels()
            .filter_map(|(span, label)| span.to_range().map(|r| (r.start, label)))
            .collect();
        let chosen = labels
            .iter()
            .find(|(offset, _)| matches!(self.locate(*offset), Located::Pack { .. }))
            .or(labels.first());
        let message = error.message();
        match chosen {
            Some(&(offset, label)) if !label.is_empty() && label != message => {
                self.problem_at(offset, format!("{message}: {label}"))
            }
            Some(&(offset, _)) => self.problem_at(offset, message),
            None => Problem::file(self.file(), message),
        }
    }

    pub(crate) fn validation_problem(
        &self,
        error: &naga::WithSpan<naga::valid::ValidationError>,
    ) -> Problem {
        use std::error::Error as _;
        let mut message = error.as_inner().to_string();
        let mut source = error.as_inner().source();
        while let Some(cause) = source {
            message.push_str(": ");
            message.push_str(&cause.to_string());
            source = cause.source();
        }
        let starts: Vec<usize> = error
            .spans()
            .filter_map(|(span, _)| span.to_range().map(|r| r.start))
            .collect();
        let chosen = starts
            .iter()
            .rev()
            .find(|&&offset| matches!(self.locate(offset), Located::Pack { .. }))
            .or(starts.last());
        match chosen {
            Some(&offset) => self.problem_at(offset, message),
            None => Problem::file(self.file(), message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaderpack::native::api::{ModuleKind, OutputSize, ScreenOutput, test_pack};
    use crate::shaderpack::native::import::memory_reader;

    fn screen(function: &str) -> ModuleKind {
        ModuleKind::Screen {
            function: function.to_owned(),
            output: ScreenOutput::Targets(vec![0]),
            size: OutputSize::Scale(1.0),
        }
    }

    fn check(composed: &Composed) -> Result<(), Problem> {
        let module = naga::front::wgsl::parse_str(composed.text())
            .map_err(|e| composed.parse_problem(&e))?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&module)
        .map_err(|e| composed.validation_problem(&e))?;
        Ok(())
    }

    #[test]
    fn a_parse_error_in_an_import_is_reported_in_that_file() {
        let wave = "fn wave(p: vec2<f32>) -> f32 {\n    return p.x + not_declared;\n}\n";
        let files = [
            (
                "lighting.wgsl",
                "#import \"lib/wave.wgsl\"\nfn lighting(in: ScreenInput) -> vec4<f32> {\n    return vec4<f32>(wave(in.uv));\n}\n",
            ),
            ("lib/wave.wgsl", wave),
        ];
        let read = memory_reader(&files);
        let composed = compose(&test_pack(), "lighting.wgsl", &screen("lighting"), &read).unwrap();
        let problem = check(&composed).unwrap_err();
        assert_eq!(problem.file, "lib/wave.wgsl", "{problem}");
        assert_eq!(
            problem.at,
            Some(Position::of(wave, wave.find("not_declared").unwrap())),
            "{problem}"
        );
    }

    #[test]
    fn a_validation_error_in_an_import_is_reported_in_that_file() {
        let math = "fn widen(x: u32) -> f32 {\n    return x + 1u;\n}\n";
        let files = [
            (
                "lighting.wgsl",
                "#import \"lib/math.wgsl\"\nfn lighting(in: ScreenInput) -> vec4<f32> {\n    return vec4<f32>(widen(in.pixel.x));\n}\n",
            ),
            ("lib/math.wgsl", math),
        ];
        let read = memory_reader(&files);
        let composed = compose(&test_pack(), "lighting.wgsl", &screen("lighting"), &read).unwrap();
        let problem = check(&composed).unwrap_err();
        assert_eq!(problem.file, "lib/math.wgsl", "{problem}");
        assert_eq!(problem.at.map(|p| p.line), Some(2), "{problem}");
    }

    #[test]
    fn offsets_map_back_to_each_part() {
        let files = [
            ("a.wgsl", "fn a() {}\n"),
            (
                "main.wgsl",
                "#import \"a.wgsl\"\nfn lighting(in: ScreenInput) -> vec4<f32> { return vec4<f32>(1.0); }",
            ),
        ];
        let read = memory_reader(&files);
        let composed = compose(&test_pack(), "main.wgsl", &screen("lighting"), &read).unwrap();
        let text = composed.text();
        let in_main = text.find("fn lighting").unwrap();
        match composed.locate(in_main) {
            Located::Pack { file, offset } => {
                assert_eq!(&*file.path, "main.wgsl");
                assert_eq!(&file.text[offset..offset + 11], "fn lighting");
            }
            other => panic!("{other:?}"),
        }
        assert!(
            matches!(composed.locate(text.find("fn a()").unwrap()), Located::Pack { file, offset: 0 } if &*file.path == "a.wgsl")
        );
        assert!(matches!(
            composed.locate(0),
            Located::Engine {
                part: EnginePart::Prelude,
                ..
            }
        ));
        let entry = text.find(api::SCREEN_FRAGMENT).unwrap();
        assert!(entry >= composed.without_entries().len());
        assert!(matches!(
            composed.locate(entry),
            Located::Engine {
                part: EnginePart::Entries,
                ..
            }
        ));
        assert_eq!(
            composed.files().collect::<Vec<_>>(),
            ["a.wgsl", "main.wgsl"]
        );
        assert_eq!(composed.file(), "main.wgsl");
        assert!(
            text.contains("fn lighting(in: ScreenInput) -> vec4<f32> { return vec4<f32>(1.0); }\n")
        );
    }

    #[test]
    fn an_error_in_engine_text_names_the_pack_file() {
        let files = [(
            "lighting.wgsl",
            "fn lighting(in: ScreenInput) -> vec3<f32> {\n    return vec3<f32>(in.uv, 0.0);\n}\n",
        )];
        let read = memory_reader(&files);
        let composed = compose(&test_pack(), "lighting.wgsl", &screen("lighting"), &read).unwrap();
        let problem = check(&composed).unwrap_err();
        assert_eq!(problem.file, "lighting.wgsl");
        assert_eq!(problem.at, None);
        assert!(
            problem.message.contains("engine's entry points"),
            "{problem}"
        );
        assert!(naga::front::wgsl::parse_str(composed.without_entries()).is_ok());
    }

    #[test]
    fn a_pass_id_must_be_callable() {
        let files = [("x.wgsl", "")];
        let read = memory_reader(&files);
        for bad in ["torch_x", "a-b", "loop", "9a"] {
            let problems = compose(&test_pack(), "x.wgsl", &screen(bad), &read).unwrap_err();
            assert_eq!(problems[0].file, manifest::FILE, "{bad}");
        }
    }
}
