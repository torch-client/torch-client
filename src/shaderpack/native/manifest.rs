use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::{Range, RangeInclusive};

use toml_edit::{Document, Item, TableLike};

use super::api::{self, SkyPart, Stage};
use super::caps::{self, Format, Needs, TierName};
use super::condition::Condition;
use super::interface;
use super::options::{self, Decl, Field, Kind, Literal, Options, Preset, Value, Values};
use super::package;
use super::problem::{Position, Problem};

pub(crate) const FILE: &str = "pack.toml";

pub(crate) const FORMAT: i64 = 1;

pub(crate) const API: Api = Api { major: 1, minor: 0 };

#[allow(
    dead_code,
    reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, looks labels up"
)]
pub(crate) const FALLBACK_LOCALE: &str = "en_us";

pub(crate) const MAX_TARGETS: usize = crate::shaderpack::directives::MAX_COLOR_TARGETS;

pub(crate) const MAX_OUTPUTS: usize = 8;

pub(crate) const MAX_TEXTURES: usize = 16;

const MAX_TARGET_SIDE: i64 = 8192;

const VANILLA_DIMENSIONS: &[&str] = &["overworld", "the_nether", "the_end"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Api {
    pub(crate) major: u32,
    pub(crate) minor: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Named {
    pub(crate) id: String,
    pub(crate) at: Position,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileRef {
    pub(crate) path: String,
    pub(crate) at: Position,
}

#[derive(Debug)]
pub(crate) struct Manifest {
    pub(crate) api: Api,
    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md names the saved option file by it"
    )]
    pub(crate) id: String,
    #[allow(
        dead_code,
        reason = "phase 2 of docs/SHADER_FORMAT.md: the pack list shows it"
    )]
    pub(crate) name: String,
    #[allow(
        dead_code,
        reason = "phase 2 of docs/SHADER_FORMAT.md: the pack list shows it"
    )]
    pub(crate) version: String,
    pub(crate) requires: Requires,
    targets: Vec<Target>,
    geometry: Geometry,
    shadow: Option<Shadow>,
    passes: Vec<Pass>,
    pub(crate) dimensions: Vec<Dimension>,
    pub(crate) textures: Vec<Texture>,
    pub(crate) options: Options,
    pub(crate) presets: Vec<Preset>,
    pub(crate) screens: Vec<Screen>,
    pub(crate) materials: Vec<Material>,
}

#[derive(Clone, Debug)]
pub(crate) struct Requires {
    pub(crate) compute: Option<bool>,
    pub(crate) outputs: Option<u32>,
    pub(crate) formats: Vec<Format>,
    pub(crate) storage_textures: Option<bool>,
    pub(crate) at: Position,
}

impl Requires {
    fn none(at: Position) -> Requires {
        Requires {
            compute: None,
            outputs: None,
            formats: Vec::new(),
            storage_textures: None,
            at,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub(crate) id: String,
    pub(crate) formats: Vec<Format>,
    pub(crate) size: Size,
    #[allow(
        dead_code,
        reason = "phase 6 of docs/SHADER_FORMAT.md builds mip chains"
    )]
    pub(crate) mips: bool,
    pub(crate) history: bool,
    pub(crate) clear: Option<[f64; 4]>,
    pub(crate) when: Option<Condition>,
    pub(crate) at: Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Size {
    Scale(Scale),
    Fixed { width: u32, height: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scale {
    Full,
    Half,
    Quarter,
    Eighth,
}

impl Scale {
    fn from_factor(factor: f64) -> Option<Scale> {
        [
            (1.0, Scale::Full),
            (0.5, Scale::Half),
            (0.25, Scale::Quarter),
            (0.125, Scale::Eighth),
        ]
        .into_iter()
        .find(|(f, _)| *f == factor)
        .map(|(_, scale)| scale)
    }

    pub(crate) fn divisor(self) -> u32 {
        match self {
            Scale::Full => 1,
            Scale::Half => 2,
            Scale::Quarter => 4,
            Scale::Eighth => 8,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Geometry {
    pub(crate) opaque: Option<Group>,
    pub(crate) translucent: Option<Group>,
}

impl Geometry {
    pub(crate) fn groups(&self) -> impl Iterator<Item = (api::Group, &Group)> {
        [
            (api::Group::Opaque, &self.opaque),
            (api::Group::Translucent, &self.translucent),
        ]
        .into_iter()
        .filter_map(|(kind, group)| Some((kind, group.as_ref()?)))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub(crate) file: FileRef,
    pub(crate) writes: Vec<Named>,
    pub(crate) stages: Vec<Stage>,
    pub(crate) vertex: bool,
    pub(crate) sky_parts: Vec<SkyPart>,
    pub(crate) at: Position,
}

fn parse_stage(name: &str) -> Option<Stage> {
    Stage::ALL
        .into_iter()
        .find(|stage| stage.info().function == name)
}

#[derive(Clone, Debug)]
pub(crate) struct Shadow {
    pub(crate) resolution: Setting,
    pub(crate) distance: Setting,
    pub(crate) color: bool,
    pub(crate) file: Option<FileRef>,
    pub(crate) when: Option<Condition>,
    pub(crate) at: Position,
}

const SHADOW_RESOLUTION: f64 = 2048.0;
const SHADOW_DISTANCE: f64 = 128.0;

impl Shadow {
    pub(crate) fn resolution(&self, values: &Values) -> u32 {
        self.resolution.at(values, SHADOW_RESOLUTION) as u32
    }

    pub(crate) fn distance(&self, values: &Values) -> f64 {
        self.distance.at(values, SHADOW_DISTANCE)
    }

    fn options(&self) -> impl Iterator<Item = &str> {
        [&self.resolution, &self.distance]
            .into_iter()
            .filter_map(Setting::option)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Setting {
    Fixed(f64),
    Option(String),
}

impl Setting {
    fn at(&self, values: &Values, fallback: f64) -> f64 {
        match self {
            Setting::Fixed(value) => *value,
            Setting::Option(id) => match values.get(id) {
                Some(Value::Int(i)) => f64::from(i),
                Some(Value::Float(f)) => f,
                _ => fallback,
            },
        }
    }

    fn option(&self) -> Option<&str> {
        match self {
            Setting::Fixed(_) => None,
            Setting::Option(id) => Some(id),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Anchor {
    Begin,
    Deferred,
    Composite,
    Final,
}

impl Anchor {
    fn parse(name: &str) -> Option<Anchor> {
        match name {
            "begin" => Some(Anchor::Begin),
            "deferred" => Some(Anchor::Deferred),
            "composite" => Some(Anchor::Composite),
            "final" => Some(Anchor::Final),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Anchor::Begin => "begin",
            Anchor::Deferred => "deferred",
            Anchor::Composite => "composite",
            Anchor::Final => "final",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Pass {
    pub(crate) id: String,
    pub(crate) anchor: Anchor,
    pub(crate) kind: PassKind,
    pub(crate) file: FileRef,
    pub(crate) writes: Vec<Named>,
    pub(crate) when: Option<Condition>,
    pub(crate) at: Position,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PassKind {
    Screen,
    Compute {
        workgroup: [u32; 3],
        dispatch: Dispatch,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Dispatch {
    Groups([u32; 3]),
    Cover(Named),
}

#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "phase 6 of docs/SHADER_FORMAT.md uploads and samples custom textures"
)]
pub(crate) struct Texture {
    pub(crate) id: String,
    pub(crate) file: FileRef,
    pub(crate) filter: Filter,
    pub(crate) wrap: Wrap,
    pub(crate) mips: bool,
    pub(crate) at: Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Filter {
    Linear,
    Nearest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wrap {
    Repeat,
    Clamp,
}

#[derive(Clone, Debug)]
pub(crate) struct Screen {
    pub(crate) id: String,
    pub(crate) items: Vec<ScreenItem>,
    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, lays out by it"
    )]
    pub(crate) columns: u8,
    pub(crate) at: Position,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ScreenItem {
    Option(Named),
    Link(Named),
    Gap,
}

#[derive(Clone, Debug)]
pub(crate) struct Dimension {
    pub(crate) id: String,
    pub(crate) targets: Option<Vec<Target>>,
    pub(crate) geometry: Option<Geometry>,
    pub(crate) shadow: Option<Option<Shadow>>,
    pub(crate) passes: Option<Vec<Pass>>,
}

#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "entity and item materials are numbered in a later step of phase 3 of docs/SHADER_FORMAT.md"
)]
pub(crate) struct Material {
    pub(crate) id: String,
    pub(crate) blocks: Vec<BlockEntry>,
    pub(crate) entities: Vec<String>,
    pub(crate) items: Vec<String>,
    pub(crate) at: Position,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BlockEntry {
    Block(String),
    State {
        block: String,
        property: String,
        value: String,
    },
    Tag(String),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout<'a> {
    pub(crate) dimension: Option<&'a str>,
    pub(crate) targets: &'a [Target],
    pub(crate) geometry: &'a Geometry,
    pub(crate) shadow: Option<&'a Shadow>,
    pub(crate) passes: &'a [Pass],
}

#[derive(Debug)]
pub(crate) struct Parsed {
    pub(crate) manifest: Option<Manifest>,
    pub(crate) problems: Vec<Problem>,
}

impl Manifest {
    pub(crate) fn parse(text: &str) -> Parsed {
        let mut cx = Ctx {
            file: FILE,
            text,
            problems: Vec::new(),
        };
        let doc = match Document::parse(text) {
            Ok(doc) => doc,
            Err(e) => {
                cx.report(
                    e.span().map_or(0, |s| s.start),
                    e.message().trim().to_owned(),
                );
                return Parsed {
                    manifest: None,
                    problems: cx.problems,
                };
            }
        };
        let manifest = read(&mut cx, doc.as_table());
        validate(&mut cx, &manifest);
        Parsed {
            manifest: Some(manifest),
            problems: cx.problems,
        }
    }

    pub(crate) fn layout(&self, dimension: &str) -> Layout<'_> {
        let id = if dimension.contains(':') {
            dimension.to_owned()
        } else {
            format!("minecraft:{dimension}")
        };
        self.layout_of(self.dimensions.iter().find(|d| d.id == id))
    }

    pub(crate) fn layouts(&self) -> impl Iterator<Item = Layout<'_>> {
        std::iter::once(self.layout_of(None))
            .chain(self.dimensions.iter().map(|d| self.layout_of(Some(d))))
    }

    fn layout_of<'a>(&'a self, dimension: Option<&'a Dimension>) -> Layout<'a> {
        Layout {
            dimension: dimension.map(|d| d.id.as_str()),
            targets: dimension
                .and_then(|d| d.targets.as_deref())
                .unwrap_or(&self.targets),
            geometry: dimension
                .and_then(|d| d.geometry.as_ref())
                .unwrap_or(&self.geometry),
            shadow: match dimension.and_then(|d| d.shadow.as_ref()) {
                Some(own) => own.as_ref(),
                None => self.shadow.as_ref(),
            },
            passes: dimension
                .and_then(|d| d.passes.as_deref())
                .unwrap_or(&self.passes),
        }
    }

    pub(crate) fn manifest_options(&self) -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        for layout in self.layouts() {
            let conditions = layout
                .targets
                .iter()
                .filter_map(|t| t.when.as_ref())
                .chain(layout.passes.iter().filter_map(|p| p.when.as_ref()))
                .chain(layout.shadow.and_then(|s| s.when.as_ref()));
            for condition in conditions {
                out.extend(condition.mentions());
            }
            out.extend(layout.shadow.into_iter().flat_map(Shadow::options));
        }
        out
    }

    pub(crate) fn default_values(&self, tier: TierName) -> Values {
        let mut values = Values::defaults(&self.options);
        if let Some(preset) = self
            .presets
            .iter()
            .find(|p| p.default_on.iter().any(|(t, _)| *t == tier))
        {
            values.apply(preset);
        }
        values
    }

    pub(crate) fn needs(&self) -> Needs {
        let mut needs = Needs::default();
        let mut formats = BTreeSet::new();
        for layout in self.layouts() {
            formats.extend(layout.targets.iter().map(|t| t.formats.clone()));
            for pass in layout.passes {
                match pass.kind {
                    PassKind::Screen => {
                        needs.outputs = needs.outputs.max(pass.writes.len().max(1) as u32)
                    }
                    PassKind::Compute { .. } => {
                        needs.compute = true;
                        needs.storage_textures |= !pass.writes.is_empty();
                    }
                }
            }
            for (_, group) in layout.geometry.groups() {
                needs.outputs = needs.outputs.max(group.writes.len() as u32);
            }
            if layout.shadow.is_some_and(|s| s.color) {
                needs.outputs = needs.outputs.max(1);
            }
        }
        needs.format_lists = formats.into_iter().collect();
        needs
    }

    pub(crate) fn requires_problems(&self) -> Vec<Problem> {
        let needs = self.needs();
        let declared = &self.requires;
        let mut out = Vec::new();
        let mut report = |message: String| out.push(problem_at(FILE, declared.at, message));
        for (key, needed, said) in [
            ("compute", needs.compute, declared.compute),
            (
                "storage_textures",
                needs.storage_textures,
                declared.storage_textures,
            ),
        ] {
            match (needed, said.unwrap_or(false)) {
                (true, false) => report(format!(
                    "the pack needs {key} but [requires] does not say `{key} = true`"
                )),
                (false, true) => report(format!(
                    "[requires] says `{key} = true` but nothing in the pack needs it"
                )),
                _ => {}
            }
        }
        let said = declared.outputs.unwrap_or(1);
        if said < needs.outputs {
            report(format!(
                "a pass writes {} outputs but [requires] says `outputs = {said}`",
                needs.outputs
            ));
        } else if said > needs.outputs.max(1) {
            report(format!(
                "[requires] says `outputs = {said}` but no pass writes more than {}",
                needs.outputs.max(1)
            ));
        }
        let needed = needs.declared_formats();
        for format in &needed {
            if !declared.formats.contains(format) {
                report(format!(
                    "a target needs {} but [requires] `formats` does not list it",
                    format.name()
                ));
            }
        }
        for format in &declared.formats {
            if format.info().baseline {
                report(format!(
                    "{} is in the baseline and does not go in [requires] `formats`",
                    format.name()
                ));
            } else if !needed.contains(format) {
                report(format!(
                    "[requires] lists {} but no target needs it; a format list needs only its last entry",
                    format.name()
                ));
            }
        }
        out
    }
}

fn problem_at(file: &str, at: Position, message: impl Into<String>) -> Problem {
    Problem {
        file: file.to_owned(),
        at: Some(at),
        message: message.into(),
        hint: None,
    }
}

struct Ctx<'t> {
    file: &'t str,
    text: &'t str,
    problems: Vec<Problem>,
}

impl Ctx<'_> {
    fn pos(&self, offset: usize) -> Position {
        Position::of(self.text, offset)
    }

    fn report(&mut self, offset: usize, message: impl Into<String>) {
        self.problems
            .push(Problem::at(self.file, self.text, offset, message));
    }

    fn report_hint(&mut self, offset: usize, message: impl Into<String>, hint: impl Into<String>) {
        self.problems
            .push(Problem::at(self.file, self.text, offset, message).hint(hint));
    }

    fn report_at(&mut self, at: Position, message: impl Into<String>) {
        self.problems.push(problem_at(self.file, at, message));
    }
}

#[derive(Clone, Debug)]
struct Spanned<T> {
    value: T,
    span: Range<usize>,
}

#[derive(Clone, Copy)]
struct Entry<'a> {
    key: &'a str,
    at: usize,
    item: &'a Item,
}

impl<'a> Entry<'a> {
    fn span(&self) -> Range<usize> {
        self.item.span().unwrap_or(self.at..self.at)
    }

    fn wrong(&self, cx: &mut Ctx<'_>, what: &str) {
        cx.report(self.span().start, format!("`{}` must be {what}", self.key));
    }

    fn string(self, cx: &mut Ctx<'_>) -> Option<Spanned<String>> {
        match self.item.as_str() {
            Some(s) => Some(Spanned {
                value: s.to_owned(),
                span: self.span(),
            }),
            None => {
                self.wrong(cx, "a string");
                None
            }
        }
    }

    fn boolean(self, cx: &mut Ctx<'_>) -> Option<bool> {
        let value = self.item.as_bool();
        if value.is_none() {
            self.wrong(cx, "true or false");
        }
        value
    }

    fn integer(self, cx: &mut Ctx<'_>, range: RangeInclusive<i64>) -> Option<i64> {
        let Some(value) = self.item.as_integer() else {
            self.wrong(cx, "an integer");
            return None;
        };
        if !range.contains(&value) {
            cx.report(
                self.span().start,
                format!(
                    "`{}` is {value}, outside {}..{}",
                    self.key,
                    range.start(),
                    range.end()
                ),
            );
            return None;
        }
        Some(value)
    }

    fn number(self, cx: &mut Ctx<'_>) -> Option<Spanned<f64>> {
        let value = self
            .item
            .as_float()
            .or_else(|| self.item.as_integer().map(|i| i as f64));
        match value {
            Some(v) if v.is_finite() => Some(Spanned {
                value: v,
                span: self.span(),
            }),
            _ => {
                self.wrong(cx, "a finite number");
                None
            }
        }
    }

    fn number_in(self, cx: &mut Ctx<'_>, range: RangeInclusive<f64>) -> Option<f64> {
        let n = self.number(cx)?;
        if !range.contains(&n.value) {
            cx.report(
                n.span.start,
                format!(
                    "`{}` is {}, outside {}..{}",
                    self.key,
                    n.value,
                    range.start(),
                    range.end()
                ),
            );
            return None;
        }
        Some(n.value)
    }

    fn strings(self, cx: &mut Ctx<'_>) -> Option<Vec<Spanned<String>>> {
        let Some(array) = self.item.as_array() else {
            self.wrong(cx, "a list of strings");
            return None;
        };
        let mut out = Vec::with_capacity(array.len());
        for value in array.iter() {
            let span = value.span().unwrap_or_else(|| self.span());
            match value.as_str() {
                Some(s) => out.push(Spanned {
                    value: s.to_owned(),
                    span,
                }),
                None => cx.report(
                    span.start,
                    format!("each entry of `{}` must be a string", self.key),
                ),
            }
        }
        Some(out)
    }

    fn integers<const N: usize>(
        self,
        cx: &mut Ctx<'_>,
        range: RangeInclusive<i64>,
    ) -> Option<[u32; N]> {
        let array = self.item.as_array().filter(|a| a.len() == N);
        let Some(array) = array else {
            self.wrong(cx, &format!("a list of {N} integers"));
            return None;
        };
        let mut out = [0u32; N];
        let mut ok = true;
        for (slot, value) in out.iter_mut().zip(array.iter()) {
            let at = value.span().map_or(self.at, |s| s.start);
            match value.as_integer() {
                Some(i) if range.contains(&i) => *slot = i as u32,
                Some(i) => {
                    cx.report(
                        at,
                        format!("{i} is outside {}..{}", range.start(), range.end()),
                    );
                    ok = false;
                }
                None => {
                    cx.report(
                        at,
                        format!("each entry of `{}` must be an integer", self.key),
                    );
                    ok = false;
                }
            }
        }
        ok.then_some(out)
    }

    fn numbers<const N: usize>(self, cx: &mut Ctx<'_>) -> Option<[f64; N]> {
        let array = self.item.as_array().filter(|a| a.len() == N);
        let Some(array) = array else {
            self.wrong(cx, &format!("a list of {N} numbers"));
            return None;
        };
        let mut out = [0.0; N];
        let mut ok = true;
        for (slot, value) in out.iter_mut().zip(array.iter()) {
            match value
                .as_float()
                .or_else(|| value.as_integer().map(|i| i as f64))
            {
                Some(v) if v.is_finite() => *slot = v,
                _ => {
                    cx.report(
                        value.span().map_or(self.at, |s| s.start),
                        format!("each entry of `{}` must be a finite number", self.key),
                    );
                    ok = false;
                }
            }
        }
        ok.then_some(out)
    }

    fn literal(self, cx: &mut Ctx<'_>) -> Option<Literal<'a>> {
        let item: &'a Item = self.item;
        let literal = if let Some(b) = item.as_bool() {
            Literal::Bool(b)
        } else if let Some(i) = item.as_integer() {
            Literal::Int(i)
        } else if let Some(f) = item.as_float() {
            Literal::Float(f)
        } else if let Some(s) = item.as_str() {
            Literal::Str(s)
        } else {
            self.wrong(cx, "true, false, a number or a string");
            return None;
        };
        Some(literal)
    }

    fn table(self, cx: &mut Ctx<'_>) -> Option<(&'a dyn TableLike, usize)> {
        match self.item.as_table_like() {
            Some(table) => Some((table, self.item.span().map_or(self.at, |s| s.start))),
            None => {
                self.wrong(cx, "a table");
                None
            }
        }
    }

    fn tables(self, cx: &mut Ctx<'_>) -> Vec<(&'a dyn TableLike, usize)> {
        if let Some(array) = self.item.as_array_of_tables() {
            return array
                .iter()
                .map(|t| (t as &dyn TableLike, t.span().map_or(self.at, |s| s.start)))
                .collect();
        }
        let Some(array) = self.item.as_array() else {
            self.wrong(cx, &format!("a list of tables, written [[{}]]", self.key));
            return Vec::new();
        };
        let mut out = Vec::with_capacity(array.len());
        for value in array.iter() {
            let at = value.span().map_or(self.at, |s| s.start);
            match value.as_inline_table() {
                Some(t) => out.push((t as &dyn TableLike, at)),
                None => cx.report(at, format!("each entry of `{}` must be a table", self.key)),
            }
        }
        out
    }
}

struct Fields<'a> {
    table: &'a dyn TableLike,
    at: usize,
    what: String,
    taken: Vec<&'a str>,
}

impl<'a> Fields<'a> {
    fn new(table: &'a dyn TableLike, at: usize, what: impl Into<String>) -> Fields<'a> {
        Fields {
            table,
            at,
            what: what.into(),
            taken: Vec::new(),
        }
    }

    fn get(&mut self, key: &str) -> Option<Entry<'a>> {
        let table: &'a dyn TableLike = self.table;
        let (k, item) = table.get_key_value(key)?;
        let key = k.get();
        self.taken.push(key);
        let at = k
            .span()
            .or_else(|| item.span())
            .map_or(self.at, |s| s.start);
        Some(Entry { key, at, item })
    }

    fn need(&mut self, cx: &mut Ctx<'_>, key: &str) -> Option<Entry<'a>> {
        let entry = self.get(key);
        if entry.is_none() {
            cx.report(self.at, format!("{} needs `{key}`", self.what));
        }
        entry
    }

    fn finish(self, cx: &mut Ctx<'_>) {
        for (key, _) in self.table.iter() {
            if !self.taken.contains(&key) {
                let at = self
                    .table
                    .key(key)
                    .and_then(|k| k.span())
                    .map_or(self.at, |s| s.start);
                cx.report(at, format!("`{key}` is not a key of {}", self.what));
            }
        }
    }
}

fn is_id(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn is_pack_id(s: &str) -> bool {
    (1..=64).contains(&s.len()) && options::is_value_name(s)
}

fn is_resource_part(s: &str, path: bool) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || b"_.-".contains(&b)
                || (path && b == b'/')
        })
}

fn is_resource(s: &str) -> bool {
    match s.split_once(':') {
        Some((namespace, path)) => {
            is_resource_part(namespace, false) && is_resource_part(path, true)
        }
        None => is_resource_part(s, true),
    }
}

fn check_id(cx: &mut Ctx<'_>, id: &Spanned<String>, what: &str) {
    if !is_id(&id.value) {
        cx.report_hint(
            id.span.start,
            format!("`{}` is not a valid {what} id", id.value),
            "use [a-z][a-z0-9_]*",
        );
    }
}

fn string_offset(text: &str, string: &Spanned<String>, offset: usize) -> usize {
    let raw = text.get(string.span.clone()).unwrap_or("");
    let delimiter = if raw.starts_with("\"\"\"") || raw.starts_with("'''") {
        3
    } else {
        1
    };
    let inner = raw.get(delimiter..raw.len().saturating_sub(delimiter));
    if inner == Some(string.value.as_str()) {
        string.span.start + delimiter + offset
    } else {
        string.span.start
    }
}

fn read_when(cx: &mut Ctx<'_>, entry: Entry<'_>, options: &Options) -> Option<Condition> {
    let source = entry.string(cx)?;
    match Condition::parse(&source.value, options) {
        Ok(condition) => Some(condition),
        Err(errors) => {
            for error in errors {
                let at = string_offset(cx.text, &source, error.offset);
                let mut problem = Problem::at(cx.file, cx.text, at, error.message);
                problem.hint = error.hint;
                cx.problems.push(problem);
            }
            None
        }
    }
}

fn read_file_ref(cx: &mut Ctx<'_>, entry: Entry<'_>, extension: &str) -> Option<FileRef> {
    let path = entry.string(cx)?;
    if !package::is_plain_path(&path.value) {
        cx.report_hint(
            path.span.start,
            format!("`{}` is not a plain pack path", path.value),
            "relative to the pack root, parts of [A-Za-z0-9._-], none starting with a dot",
        );
        return None;
    }
    if !path.value.ends_with(extension) {
        cx.report(
            path.span.start,
            format!("`{}` should be a {extension} file", path.value),
        );
        return None;
    }
    Some(FileRef {
        path: path.value,
        at: cx.pos(path.span.start),
    })
}

fn read_named_list(cx: &mut Ctx<'_>, entry: Entry<'_>) -> Vec<Named> {
    entry
        .strings(cx)
        .unwrap_or_default()
        .into_iter()
        .map(|s| Named {
            at: cx.pos(s.span.start),
            id: s.value,
        })
        .collect()
}

fn read(cx: &mut Ctx<'_>, root: &dyn TableLike) -> Manifest {
    let mut top = Fields::new(root, 0, "the top level of pack.toml");

    match top
        .need(cx, "format")
        .and_then(|e| e.integer(cx, 1..=i64::MAX).map(|f| (e, f)))
    {
        Some((entry, format)) if format > FORMAT => cx.report_hint(
            entry.span().start,
            format!("this pack is format {format}; this client reads formats up to {FORMAT}"),
            "a newer client reads it",
        ),
        _ => {}
    }

    let api = top
        .need(cx, "api")
        .and_then(|e| e.string(cx))
        .and_then(|s| {
            let parsed = s.value.split_once('.').and_then(|(major, minor)| {
                Some(Api {
                    major: major.parse().ok()?,
                    minor: minor.parse().ok()?,
                })
            });
            match parsed {
                None => cx.report_hint(
                    s.span.start,
                    format!("`{}` is not an API version", s.value),
                    "write \"major.minor\", as \"1.0\"",
                ),
                Some(api) if api.major != API.major || api.minor > API.minor => cx.report_hint(
                    s.span.start,
                    format!(
                        "this pack is written for API {}.{}; this client provides {}.{}",
                        api.major, api.minor, API.major, API.minor
                    ),
                    "a pack runs on the same major version with an equal or newer minor",
                ),
                Some(_) => {}
            }
            parsed
        });

    let id = top.need(cx, "id").and_then(|e| e.string(cx));
    if let Some(id) = &id
        && !is_pack_id(&id.value)
    {
        cx.report_hint(
            id.span.start,
            format!("`{}` is not a valid pack id", id.value),
            "use [a-z0-9_], at most 64 characters",
        );
    }
    let name = top.need(cx, "name").and_then(|e| e.string(cx));
    let version = top.need(cx, "version").and_then(|e| e.string(cx));

    let decls: Vec<Decl> = top
        .get("option")
        .map(|e| e.tables(cx))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(table, at)| read_option(cx, table, at))
        .collect();
    let options = Options::new(decls.clone());
    check_unique(cx, decls.iter().map(|d| (d.id.as_str(), d.at)), "option");

    let requires = match top.get("requires") {
        Some(entry) => read_requires(cx, entry),
        None => Requires::none(Position { line: 1, column: 1 }),
    };

    let parts = read_layout(cx, &mut top, &options, false);

    let dimensions = top
        .get("dimension")
        .map(|e| read_dimensions(cx, e, &options))
        .unwrap_or_default();
    let textures: Vec<Texture> = top
        .get("texture")
        .map(|e| e.tables(cx))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(table, at)| read_texture(cx, table, at))
        .collect();
    let presets: Vec<Preset> = top
        .get("preset")
        .map(|e| e.tables(cx))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(table, at)| read_preset(cx, table, at, &options))
        .collect();
    let screens: Vec<Screen> = top
        .get("screen")
        .map(|e| e.tables(cx))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(table, at)| read_screen(cx, table, at))
        .collect();
    let materials: Vec<Material> = top
        .get("material")
        .map(|e| e.tables(cx))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(table, at)| read_material(cx, table, at))
        .collect();
    top.finish(cx);

    Manifest {
        api: api.unwrap_or(API),
        id: id.map(|s| s.value).unwrap_or_default(),
        name: name.map(|s| s.value).unwrap_or_default(),
        version: version.map(|s| s.value).unwrap_or_default(),
        requires,
        targets: parts.targets.unwrap_or_default(),
        geometry: parts.geometry.unwrap_or_default(),
        shadow: parts.shadow.flatten(),
        passes: parts.passes.unwrap_or_default(),
        dimensions,
        textures,
        options,
        presets,
        screens,
        materials,
    }
}

struct Parts {
    targets: Option<Vec<Target>>,
    geometry: Option<Geometry>,
    shadow: Option<Option<Shadow>>,
    passes: Option<Vec<Pass>>,
}

fn read_layout(
    cx: &mut Ctx<'_>,
    fields: &mut Fields<'_>,
    options: &Options,
    dimension: bool,
) -> Parts {
    let targets = fields.get("target").map(|e| {
        e.tables(cx)
            .into_iter()
            .filter_map(|(table, at)| read_target(cx, table, at, options))
            .collect::<Vec<Target>>()
    });
    let geometry = fields.get("geometry").and_then(|e| read_geometry(cx, e));
    let shadow = fields.get("shadow").and_then(|e| match e.item.as_bool() {
        Some(false) if dimension => Some(None),
        Some(_) => {
            cx.report_hint(
                e.span().start,
                "`shadow` is a [shadow] table here",
                "`shadow = false` goes in a [dimension.<name>] table, where it removes the top level's [shadow]",
            );
            None
        }
        None => read_shadow(cx, e, options).map(Some),
    });
    let passes = fields.get("pass").map(|e| {
        e.tables(cx)
            .into_iter()
            .filter_map(|(table, at)| read_pass(cx, table, at, options))
            .collect::<Vec<Pass>>()
    });
    Parts {
        targets,
        geometry,
        shadow,
        passes,
    }
}

fn read_requires(cx: &mut Ctx<'_>, entry: Entry<'_>) -> Requires {
    let Some((table, at)) = entry.table(cx) else {
        return Requires::none(cx.pos(entry.at));
    };
    let mut f = Fields::new(table, at, "[requires]");
    let compute = f.get("compute").and_then(|e| e.boolean(cx));
    let outputs = f
        .get("outputs")
        .and_then(|e| e.integer(cx, 1..=MAX_OUTPUTS as i64))
        .map(|n| n as u32);
    let mut formats = Vec::new();
    for name in f
        .get("formats")
        .and_then(|e| e.strings(cx))
        .unwrap_or_default()
    {
        match Format::parse(&name.value) {
            Some(format) if !formats.contains(&format) => formats.push(format),
            Some(_) => cx.report(name.span.start, format!("{} is listed twice", name.value)),
            None => cx.report(
                name.span.start,
                format!("`{}` is not a target format", name.value),
            ),
        }
    }
    let storage_textures = f.get("storage_textures").and_then(|e| e.boolean(cx));
    f.finish(cx);
    Requires {
        compute,
        outputs,
        formats,
        storage_textures,
        at: cx.pos(at),
    }
}

fn read_target(
    cx: &mut Ctx<'_>,
    table: &dyn TableLike,
    at: usize,
    options: &Options,
) -> Option<Target> {
    let mut f = Fields::new(table, at, "[[target]]");
    let id = f.need(cx, "id").and_then(|e| e.string(cx));
    let formats = read_formats(cx, &mut f, at);
    let size = match (f.get("scale"), f.get("size")) {
        (Some(scale), Some(_)) => {
            cx.report(scale.at, "a target has `scale` or `size`, not both");
            None
        }
        (Some(scale), None) => scale.number(cx).and_then(|n| {
            let scale = Scale::from_factor(n.value);
            if scale.is_none() {
                cx.report(
                    n.span.start,
                    format!("scale {} is not one of 1, 0.5, 0.25, 0.125", n.value),
                );
            }
            scale.map(Size::Scale)
        }),
        (None, Some(size)) => size
            .integers::<2>(cx, 1..=MAX_TARGET_SIDE)
            .map(|[width, height]| Size::Fixed { width, height }),
        (None, None) => Some(Size::Scale(Scale::Full)),
    };
    let mips = f.get("mips").and_then(|e| e.boolean(cx)).unwrap_or(false);
    let history = f
        .get("history")
        .and_then(|e| e.boolean(cx))
        .unwrap_or(false);
    let clear = f.get("clear").and_then(|e| e.numbers::<4>(cx));
    let when = f.get("when").and_then(|e| read_when(cx, e, options));
    f.finish(cx);

    let id = id?;
    check_id(cx, &id, "target");
    Some(Target {
        id: id.value,
        formats,
        size: size?,
        mips,
        history,
        clear,
        when,
        at: cx.pos(at),
    })
}

fn read_formats(cx: &mut Ctx<'_>, f: &mut Fields<'_>, at: usize) -> Vec<Format> {
    let stand_in = vec![Format::Rgba8Unorm];
    let Some(entry) = f.get("format") else {
        cx.report_hint(
            at,
            "[[target]] needs `format`",
            "\"rgba8unorm\" renders on every tier; [\"rgba16float\", \"rgba8unorm\"] is HDR where the device has it",
        );
        return stand_in;
    };
    let names = if entry.item.as_str().is_some() {
        entry.string(cx).into_iter().collect::<Vec<_>>()
    } else if let Some(array) = entry.item.as_array() {
        let names = entry.strings(cx).unwrap_or_default();
        if names.len() != array.len() {
            return stand_in;
        }
        names
    } else {
        entry.wrong(cx, "a format name or a list of them");
        return stand_in;
    };

    let mut ok = true;
    if names.is_empty() || names.len() > caps::MAX_FORMAT_LIST {
        cx.report(
            entry.at,
            format!(
                "`format` lists {} formats; a list has 1 to {}",
                names.len(),
                caps::MAX_FORMAT_LIST
            ),
        );
        ok = false;
    }
    let mut formats: Vec<Format> = Vec::with_capacity(names.len());
    for name in &names {
        match Format::parse(&name.value) {
            Some(Format::Depth) => {
                cx.report_hint(
                    name.span.start,
                    "a [[target]] is a colour target",
                    "the scene's depth is the engine's scene_depth",
                );
                ok = false;
            }
            Some(format) if formats.contains(&format) => {
                cx.report(name.span.start, format!("{} is listed twice", name.value));
                ok = false;
            }
            Some(format) => formats.push(format),
            None => {
                cx.report_hint(
                    name.span.start,
                    format!("`{}` is not a target format", name.value),
                    "see native::caps::FORMATS for the names",
                );
                ok = false;
            }
        }
    }
    if let Some(first) = formats.first()
        && formats
            .iter()
            .any(|format| format.info().class != first.info().class)
    {
        cx.report_hint(
            entry.at,
            "the formats in `format` mix float and unsigned integer",
            "every entry needs the same class, so the target's WGSL type does not depend on which one the device gets",
        );
        ok = false;
    }
    if ok { formats } else { stand_in }
}

fn read_geometry(cx: &mut Ctx<'_>, entry: Entry<'_>) -> Option<Geometry> {
    let (table, at) = entry.table(cx)?;
    let mut f = Fields::new(table, at, "[geometry]");
    let opaque = f
        .get("opaque")
        .and_then(|e| read_group(cx, e, api::Group::Opaque));
    let translucent = f
        .get("translucent")
        .and_then(|e| read_group(cx, e, api::Group::Translucent));
    f.finish(cx);
    Some(Geometry {
        opaque,
        translucent,
    })
}

fn read_group(cx: &mut Ctx<'_>, entry: Entry<'_>, kind: api::Group) -> Option<Group> {
    let (table, at) = entry.table(cx)?;
    let what = format!("[geometry.{}]", kind.name());
    let mut f = Fields::new(table, at, what.clone());
    let file = f
        .need(cx, "file")
        .and_then(|e| read_file_ref(cx, e, ".wgsl"));
    let writes = f
        .need(cx, "writes")
        .map(|e| read_named_list(cx, e))
        .unwrap_or_default();
    let mut stages = Vec::new();
    for name in f
        .get("stages")
        .and_then(|e| e.strings(cx))
        .unwrap_or_default()
    {
        match parse_stage(&name.value) {
            None => cx.report(
                name.span.start,
                format!("`{}` is not a geometry stage", name.value),
            ),
            Some(stage) if stage.info().group != kind => cx.report(
                name.span.start,
                format!(
                    "`{}` draws in the {} group, not {}",
                    name.value,
                    stage.info().group.name(),
                    kind.name()
                ),
            ),
            Some(stage) if stages.contains(&stage) => {
                cx.report(name.span.start, format!("`{}` is listed twice", name.value))
            }
            Some(stage) => stages.push(stage),
        }
    }
    let vertex = f.get("vertex").and_then(|e| e.boolean(cx)).unwrap_or(false);
    let sky_parts = match kind {
        api::Group::Opaque => match f.get("sky_parts") {
            Some(entry) => read_sky_parts(cx, entry),
            None => SkyPart::ALL.to_vec(),
        },
        api::Group::Translucent => Vec::new(),
    };
    f.finish(cx);

    if writes.is_empty() {
        cx.report(at, format!("{what} writes no target"));
    }
    check_writes(cx, &writes, &what);
    Some(Group {
        file: file?,
        writes,
        stages,
        vertex,
        sky_parts,
        at: cx.pos(at),
    })
}

fn read_sky_parts(cx: &mut Ctx<'_>, entry: Entry<'_>) -> Vec<SkyPart> {
    let mut parts = Vec::new();
    for name in entry.strings(cx).unwrap_or_default() {
        match SkyPart::parse(&name.value) {
            Some(part) if parts.contains(&part) => {
                cx.report(name.span.start, format!("`{}` is listed twice", name.value))
            }
            Some(part) => parts.push(part),
            None => {
                let names: Vec<&str> = SkyPart::ALL.iter().map(|p| p.name()).collect();
                cx.report_hint(
                    name.span.start,
                    format!("`{}` is not a sky part", name.value),
                    format!("one of: {}", names.join(", ")),
                );
            }
        }
    }
    parts.sort_by_key(|part| part.id());
    parts
}

fn check_writes(cx: &mut Ctx<'_>, writes: &[Named], what: &str) {
    if writes.len() > MAX_OUTPUTS {
        cx.report_at(
            writes[MAX_OUTPUTS].at,
            format!(
                "{what} writes {} targets; the most is {MAX_OUTPUTS}",
                writes.len()
            ),
        );
    }
    for (i, write) in writes.iter().enumerate() {
        if writes[..i].iter().any(|w| w.id == write.id) {
            cx.report_at(
                write.at,
                format!("`{}` is written twice by one {what}", write.id),
            );
        }
    }
}

fn read_shadow(cx: &mut Ctx<'_>, entry: Entry<'_>, options: &Options) -> Option<Shadow> {
    let (table, at) = entry.table(cx)?;
    let mut f = Fields::new(table, at, "[shadow]");
    let resolution = f
        .get("resolution")
        .and_then(|e| read_setting(cx, e, options, 256.0..=8192.0, true));
    let distance = f
        .get("distance")
        .and_then(|e| read_setting(cx, e, options, 16.0..=512.0, false));
    let color = f.get("color").and_then(|e| e.boolean(cx)).unwrap_or(false);
    let file_entry = f.get("file");
    let file = file_entry.and_then(|e| read_file_ref(cx, e, ".wgsl"));
    let when = f.get("when").and_then(|e| read_when(cx, e, options));
    f.finish(cx);

    match (color, file_entry) {
        (true, None) => cx.report(at, "[shadow] needs `file` when `color = true`"),
        (false, Some(e)) => cx.report_hint(
            e.at,
            "`file` is read only when `color = true`",
            "set `color = true` or remove `file`",
        ),
        _ => {}
    }
    Some(Shadow {
        resolution: resolution.unwrap_or(Setting::Fixed(SHADOW_RESOLUTION)),
        distance: distance.unwrap_or(Setting::Fixed(SHADOW_DISTANCE)),
        color,
        file,
        when,
        at: cx.pos(at),
    })
}

fn read_setting(
    cx: &mut Ctx<'_>,
    entry: Entry<'_>,
    options: &Options,
    range: RangeInclusive<f64>,
    integer: bool,
) -> Option<Setting> {
    let (lo, hi) = (*range.start(), *range.end());
    let Some(id) = entry.item.as_str() else {
        return if integer {
            entry
                .integer(cx, lo as i64..=hi as i64)
                .map(|n| Setting::Fixed(n as f64))
        } else {
            entry.number_in(cx, range).map(Setting::Fixed)
        };
    };
    let at = entry.span().start;
    let key = entry.key;
    let Some(decl) = options.get(id) else {
        cx.report_hint(
            at,
            format!("`{key}` names no option `{id}`"),
            "a number, or the id of an option that sets it",
        );
        return None;
    };
    let (min, max) = match (&decl.kind, integer) {
        (Kind::Int { min, max }, _) => (f64::from(*min), f64::from(*max)),
        (Kind::Float { min, max, .. }, false) => (*min, *max),
        (kind, _) => {
            let wanted = if integer {
                "an int option"
            } else {
                "an int or float option"
            };
            cx.report(
                at,
                format!(
                    "`{key}` is set by `{id}`, a {} option; it takes {wanted}",
                    kind.type_name()
                ),
            );
            return None;
        }
    };
    if min < lo || max > hi {
        cx.report_hint(
            at,
            format!("option `{id}` runs {min}..{max}, and `{key}` must stay inside {lo}..{hi}"),
            format!("narrow the option's `min` and `max` to {lo}..{hi} or less"),
        );
        return None;
    }
    Some(Setting::Option(id.to_owned()))
}

fn read_pass(
    cx: &mut Ctx<'_>,
    table: &dyn TableLike,
    at: usize,
    options: &Options,
) -> Option<Pass> {
    let mut f = Fields::new(table, at, "[[pass]]");
    let id = f.need(cx, "id").and_then(|e| e.string(cx));
    let anchor = f.need(cx, "at").and_then(|e| e.string(cx)).and_then(|s| {
        let anchor = Anchor::parse(&s.value);
        if anchor.is_none() {
            cx.report_hint(
                s.span.start,
                format!("`{}` is not an anchor", s.value),
                "begin, deferred, composite or final",
            );
        }
        anchor
    });
    let file = f
        .need(cx, "file")
        .and_then(|e| read_file_ref(cx, e, ".wgsl"));
    let writes = f
        .get("writes")
        .map(|e| read_named_list(cx, e))
        .unwrap_or_default();
    let when = f.get("when").and_then(|e| read_when(cx, e, options));
    let kind = match f.get("kind").and_then(|e| e.string(cx)) {
        None => Some(PassKind::Screen),
        Some(s) if s.value == "screen" => Some(PassKind::Screen),
        Some(s) if s.value == "compute" => {
            f.what = "a compute [[pass]]".to_owned();
            let workgroup = f.need(cx, "workgroup").and_then(|e| {
                let size = e.integers::<3>(cx, 1..=256)?;
                if size[2] > 64 || size.iter().product::<u32>() > 256 {
                    cx.report(
                        e.span().start,
                        "a workgroup has at most 256 invocations and a z of at most 64",
                    );
                    return None;
                }
                Some(size)
            });
            let dispatch = f.need(cx, "dispatch").and_then(|e| {
                if let Some(text) = e.item.as_str() {
                    match text.strip_prefix("target:") {
                        Some(target) => Some(Dispatch::Cover(Named {
                            id: target.to_owned(),
                            at: cx.pos(e.span().start),
                        })),
                        None => {
                            e.wrong(cx, "[x, y, z] or \"target:<id>\"");
                            None
                        }
                    }
                } else {
                    e.integers::<3>(cx, 1..=65535).map(Dispatch::Groups)
                }
            });
            workgroup
                .zip(dispatch)
                .map(|(workgroup, dispatch)| PassKind::Compute {
                    workgroup,
                    dispatch,
                })
        }
        Some(s) => {
            cx.report_hint(
                s.span.start,
                format!("`{}` is not a pass kind", s.value),
                "screen or compute",
            );
            None
        }
    };
    if kind == Some(PassKind::Screen) {
        f.what = "a screen [[pass]]".to_owned();
    }
    f.finish(cx);

    let id = id?;
    check_id(cx, &id, "pass");
    let what = format!("[[pass]] `{}`", id.value);
    check_writes(cx, &writes, &what);
    Some(Pass {
        id: id.value,
        anchor: anchor?,
        kind: kind?,
        file: file?,
        writes,
        when,
        at: cx.pos(at),
    })
}

fn read_texture(cx: &mut Ctx<'_>, table: &dyn TableLike, at: usize) -> Option<Texture> {
    let mut f = Fields::new(table, at, "[[texture]]");
    let id = f.need(cx, "id").and_then(|e| e.string(cx));
    let file = f
        .need(cx, "file")
        .and_then(|e| read_file_ref(cx, e, ".png"));
    let filter = match f.get("filter").and_then(|e| e.string(cx)) {
        None => Some(Filter::Linear),
        Some(s) if s.value == "linear" => Some(Filter::Linear),
        Some(s) if s.value == "nearest" => Some(Filter::Nearest),
        Some(s) => {
            cx.report_hint(
                s.span.start,
                format!("`{}` is not a filter", s.value),
                "linear or nearest",
            );
            None
        }
    };
    let wrap = match f.get("wrap").and_then(|e| e.string(cx)) {
        None => Some(Wrap::Repeat),
        Some(s) if s.value == "repeat" => Some(Wrap::Repeat),
        Some(s) if s.value == "clamp" => Some(Wrap::Clamp),
        Some(s) => {
            cx.report_hint(
                s.span.start,
                format!("`{}` is not a wrap mode", s.value),
                "repeat or clamp",
            );
            None
        }
    };
    let mips = f.get("mips").and_then(|e| e.boolean(cx)).unwrap_or(false);
    f.finish(cx);

    let id = id?;
    check_id(cx, &id, "texture");
    Some(Texture {
        id: id.value,
        file: file?,
        filter: filter?,
        wrap: wrap?,
        mips,
        at: cx.pos(at),
    })
}

fn read_option(cx: &mut Ctx<'_>, table: &dyn TableLike, at: usize) -> Option<Decl> {
    let mut f = Fields::new(table, at, "[[option]]");
    let id = f.need(cx, "id").and_then(|e| e.string(cx));
    let ty = f.need(cx, "type").and_then(|e| e.string(cx));
    let name = f.get("name").and_then(|e| e.string(cx)).map(|s| s.value);
    let default = f.need(cx, "default");

    let mut spans: HashMap<Field, usize> = HashMap::new();
    let kind = match ty.as_ref().map(|t| t.value.as_str()) {
        None => None,
        Some("bool") => Some(Kind::Bool),
        Some("int") => {
            let mut bound = |key: &str, field: Field| -> Option<i32> {
                let entry = f.need(cx, key)?;
                spans.insert(field, entry.span().start);
                entry
                    .integer(cx, i32::MIN.into()..=i32::MAX.into())
                    .map(|n| n as i32)
            };
            let min = bound("min", Field::Min);
            let max = bound("max", Field::Max);
            min.zip(max).map(|(min, max)| Kind::Int { min, max })
        }
        Some("float") => {
            let mut number = |key: &str, field: Field, required: bool| -> Option<f64> {
                let entry = if required {
                    f.need(cx, key)?
                } else {
                    f.get(key)?
                };
                spans.insert(field, entry.span().start);
                entry.number(cx).map(|n| n.value)
            };
            let min = number("min", Field::Min, true);
            let max = number("max", Field::Max, true);
            let step = number("step", Field::Step, false);
            min.zip(max).map(|(min, max)| Kind::Float {
                min,
                max,
                step: step.unwrap_or((max - min) / 100.0),
            })
        }
        Some("choice") => {
            let values = f.need(cx, "values").and_then(|e| {
                spans.insert(Field::Values, e.span().start);
                e.strings(cx)
            });
            values.map(|values| Kind::Choice {
                values: values.into_iter().map(|v| v.value).collect(),
            })
        }
        Some(other) => {
            let at = ty.as_ref().map_or(at, |t| t.span.start);
            cx.report_hint(
                at,
                format!("`{other}` is not an option type"),
                "bool, int, float or choice",
            );
            None
        }
    };
    if let Some(kind) = &kind {
        f.what = format!("a {} [[option]]", kind.type_name());
    }
    f.finish(cx);

    let id = id?;
    check_id(cx, &id, "option");
    let kind = kind?;
    for (field, message) in kind.problems() {
        cx.report(spans.get(&field).copied().unwrap_or(at), message);
    }
    let default = default?;
    let literal = default.literal(cx)?;
    let default_value = match kind.accept(literal) {
        Ok(value) => value,
        Err(message) => {
            cx.report(default.span().start, format!("default: {message}"));
            return None;
        }
    };
    Some(Decl {
        id: id.value,
        kind,
        default: default_value,
        name,
        at: cx.pos(at),
    })
}

fn read_preset(
    cx: &mut Ctx<'_>,
    table: &dyn TableLike,
    at: usize,
    options: &Options,
) -> Option<Preset> {
    let mut f = Fields::new(table, at, "[[preset]]");
    let id = f.need(cx, "id").and_then(|e| e.string(cx));
    let name = f.get("name").and_then(|e| e.string(cx)).map(|s| s.value);
    let set_entry = f.need(cx, "set");
    let mut default_on = Vec::new();
    for tier in f
        .get("default_on")
        .and_then(|e| e.strings(cx))
        .unwrap_or_default()
    {
        match caps::TIERS
            .iter()
            .map(|t| t.name)
            .find(|t| t.name() == tier.value)
        {
            Some(name) if default_on.iter().any(|(t, _)| *t == name) => {
                cx.report(tier.span.start, format!("{} is listed twice", tier.value))
            }
            Some(name) => default_on.push((name, cx.pos(tier.span.start))),
            None => {
                let names: Vec<&str> = caps::TIERS.iter().map(|t| t.name.name()).collect();
                cx.report_hint(
                    tier.span.start,
                    format!("`{}` is not a tier", tier.value),
                    format!("one of: {}", names.join(", ")),
                );
            }
        }
    }
    f.finish(cx);

    let mut set = Vec::new();
    if let Some((set_table, set_at)) = set_entry.and_then(|e| e.table(cx)) {
        for (key, item) in set_table.iter() {
            let key_at = set_table
                .key(key)
                .and_then(|k| k.span())
                .map_or(set_at, |s| s.start);
            let entry = Entry {
                key,
                at: key_at,
                item,
            };
            let Some(decl) = options.get(key) else {
                cx.report(key_at, format!("`{key}` is not an option"));
                continue;
            };
            let Some(literal) = entry.literal(cx) else {
                continue;
            };
            match decl.kind.accept(literal) {
                Ok(value) => set.push((decl.id.clone(), value)),
                Err(message) => cx.report(entry.span().start, format!("`{key}`: {message}")),
            }
        }
    }

    let id = id?;
    check_id(cx, &id, "preset");
    Some(Preset {
        id: id.value,
        name,
        set,
        default_on,
        at: cx.pos(at),
    })
}

fn read_screen(cx: &mut Ctx<'_>, table: &dyn TableLike, at: usize) -> Option<Screen> {
    let mut f = Fields::new(table, at, "[[screen]]");
    let id = f.need(cx, "id").and_then(|e| e.string(cx));
    let items: Vec<ScreenItem> = f
        .need(cx, "items")
        .and_then(|e| e.strings(cx))
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            let at = cx.pos(s.span.start);
            if s.value.is_empty() {
                ScreenItem::Gap
            } else if let Some(screen) = s.value.strip_prefix("screen:") {
                ScreenItem::Link(Named {
                    id: screen.to_owned(),
                    at,
                })
            } else {
                ScreenItem::Option(Named { id: s.value, at })
            }
        })
        .collect();
    let columns = f
        .get("columns")
        .map_or(Some(2), |e| e.integer(cx, 1..=2))
        .map(|n| n as u8);
    f.finish(cx);

    let id = id?;
    check_id(cx, &id, "screen");
    Some(Screen {
        id: id.value,
        items,
        columns: columns.unwrap_or(2),
        at: cx.pos(at),
    })
}

fn read_dimensions(cx: &mut Ctx<'_>, entry: Entry<'_>, options: &Options) -> Vec<Dimension> {
    let Some((table, _)) = entry.table(cx) else {
        return Vec::new();
    };
    let mut out: Vec<Dimension> = Vec::new();
    for (name, item) in table.iter() {
        let at = table
            .key(name)
            .and_then(|k| k.span())
            .map_or(entry.at, |s| s.start);
        let id = if VANILLA_DIMENSIONS.contains(&name) {
            Some(format!("minecraft:{name}"))
        } else if name.contains(':') && is_resource(name) {
            Some(name.to_owned())
        } else {
            cx.report_hint(
                at,
                format!("`{name}` is not a dimension"),
                "overworld, the_nether, the_end, or a namespaced id such as \"mod:dimension\"",
            );
            None
        };
        let Some((dim_table, dim_at)) = (Entry {
            key: name,
            at,
            item,
        })
        .table(cx) else {
            continue;
        };
        let mut f = Fields::new(dim_table, dim_at, format!("[dimension.{name}]"));
        let parts = read_layout(cx, &mut f, options, true);
        f.finish(cx);
        let Some(id) = id else { continue };
        if out.iter().any(|d| d.id == id) {
            cx.report(at, format!("{id} has two [dimension] tables"));
            continue;
        }
        out.push(Dimension {
            id,
            targets: parts.targets,
            geometry: parts.geometry,
            shadow: parts.shadow,
            passes: parts.passes,
        });
    }
    out
}

fn read_material(cx: &mut Ctx<'_>, table: &dyn TableLike, at: usize) -> Option<Material> {
    let mut f = Fields::new(table, at, "[[material]]");
    let id = f.need(cx, "id").and_then(|e| e.string(cx));
    let blocks = f
        .get("blocks")
        .and_then(|e| e.strings(cx))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| {
            let entry = parse_block_entry(&s.value);
            if entry.is_none() {
                cx.report_hint(
                    s.span.start,
                    format!("`{}` is not a block entry", s.value),
                    "a block name, name:property=value, or #tag",
                );
            }
            entry
        })
        .collect::<Vec<BlockEntry>>();
    let mut resources = |key: &str, what: &str| -> Vec<String> {
        f.get(key)
            .and_then(|e| e.strings(cx))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| {
                if is_resource(&s.value) {
                    Some(s.value)
                } else {
                    cx.report(s.span.start, format!("`{}` is not {what} name", s.value));
                    None
                }
            })
            .collect()
    };
    let entities = resources("entities", "an entity");
    let items = resources("items", "an item");
    f.finish(cx);

    let id = id?;
    check_id(cx, &id, "material");
    Some(Material {
        id: id.value,
        blocks,
        entities,
        items,
        at: cx.pos(at),
    })
}

fn parse_block_entry(text: &str) -> Option<BlockEntry> {
    if let Some(tag) = text.strip_prefix('#') {
        return is_resource(tag).then(|| BlockEntry::Tag(tag.to_owned()));
    }
    let Some((head, value)) = text.split_once('=') else {
        return is_resource(text).then(|| BlockEntry::Block(text.to_owned()));
    };
    let (block, property) = head.rsplit_once(':')?;
    (is_resource(block) && options::is_value_name(property) && options::is_value_name(value)).then(
        || BlockEntry::State {
            block: block.to_owned(),
            property: property.to_owned(),
            value: value.to_owned(),
        },
    )
}

fn validate(cx: &mut Ctx<'_>, manifest: &Manifest) {
    check_unique(
        cx,
        manifest.passes.iter().map(|p| (p.id.as_str(), p.at)),
        "[[pass]]",
    );
    check_pass_list(cx, &manifest.passes);
    check_target_list(cx, &manifest.targets);
    check_refs(
        cx,
        &manifest.targets,
        Some(&manifest.geometry),
        Some(manifest.passes.as_slice()),
        None,
    );
    for dimension in &manifest.dimensions {
        if let Some(passes) = &dimension.passes {
            check_unique(cx, passes.iter().map(|p| (p.id.as_str(), p.at)), "[[pass]]");
            check_pass_list(cx, passes);
        }
        if let Some(targets) = &dimension.targets {
            check_target_list(cx, targets);
        }
        let layout = manifest.layout_of(Some(dimension));
        let own_targets = dimension.targets.is_some();
        check_refs(
            cx,
            layout.targets,
            (own_targets || dimension.geometry.is_some()).then_some(layout.geometry),
            (own_targets || dimension.passes.is_some()).then_some(layout.passes),
            Some(dimension.id.as_str()),
        );
    }

    if manifest.textures.len() > MAX_TEXTURES {
        cx.report_at(
            manifest.textures[MAX_TEXTURES].at,
            format!("at most {MAX_TEXTURES} [[texture]] tables"),
        );
    }
    check_unique(
        cx,
        manifest.textures.iter().map(|t| (t.id.as_str(), t.at)),
        "[[texture]]",
    );
    check_unique(
        cx,
        manifest.presets.iter().map(|p| (p.id.as_str(), p.at)),
        "[[preset]]",
    );
    let mut defaults: HashMap<TierName, &str> = HashMap::new();
    for preset in &manifest.presets {
        for &(tier, at) in &preset.default_on {
            if let Some(first) = defaults.insert(tier, preset.id.as_str()) {
                cx.report_at(
                    at,
                    format!("{} already defaults to preset `{first}`", tier.name()),
                );
            }
        }
    }
    check_unique(
        cx,
        manifest.materials.iter().map(|m| (m.id.as_str(), m.at)),
        "[[material]]",
    );
    check_screens(cx, &manifest.screens, &manifest.options);

    for layout in manifest.layouts() {
        for problem in interface::name_problems(manifest, layout) {
            if !cx.problems.contains(&problem) {
                cx.problems.push(problem);
            }
        }
    }
}

fn check_unique<'a>(cx: &mut Ctx<'_>, ids: impl Iterator<Item = (&'a str, Position)>, what: &str) {
    let mut seen = HashSet::new();
    for (id, at) in ids {
        if !seen.insert(id) {
            cx.report_at(at, format!("a second {what} `{id}`"));
        }
    }
}

fn check_pass_list(cx: &mut Ctx<'_>, passes: &[Pass]) {
    let last_final = passes.iter().rposition(|p| p.anchor == Anchor::Final);
    for (i, pass) in passes.iter().enumerate() {
        let writes_screen = Some(i) == last_final;
        match (&pass.kind, writes_screen) {
            (PassKind::Screen, true) => {
                if let Some(first) = pass.writes.first() {
                    cx.report_problem(
                        problem_at(
                            cx.file,
                            first.at,
                            format!("`{}` is the last final pass and writes the screen", pass.id),
                        )
                        .hint("leave out `writes`"),
                    );
                }
                if pass.when.is_some() {
                    cx.report_problem(
                        problem_at(
                            cx.file,
                            pass.at,
                            format!("`{}` writes the screen and cannot have `when`", pass.id),
                        )
                        .hint("with the condition false nothing would reach the screen"),
                    );
                }
            }
            (PassKind::Compute { .. }, true) => cx.report_problem(
                problem_at(
                    cx.file,
                    pass.at,
                    format!(
                        "`{}` is the last final pass, so it writes the screen",
                        pass.id
                    ),
                )
                .hint("make it a screen pass, or add a screen pass after it"),
            ),
            (_, false) if pass.writes.is_empty() => cx.report_problem(
                problem_at(cx.file, pass.at, format!("`{}` needs `writes`", pass.id))
                    .hint("only the last final pass leaves it out"),
            ),
            _ => {}
        }
    }
}

impl Ctx<'_> {
    fn report_problem(&mut self, problem: Problem) {
        self.problems.push(problem);
    }
}

fn check_target_list(cx: &mut Ctx<'_>, targets: &[Target]) {
    if targets.len() > MAX_TARGETS {
        cx.report_at(
            targets[MAX_TARGETS].at,
            format!("at most {MAX_TARGETS} [[target]] tables"),
        );
    }
    check_unique(
        cx,
        targets.iter().map(|t| (t.id.as_str(), t.at)),
        "[[target]]",
    );
}

fn check_refs(
    cx: &mut Ctx<'_>,
    targets: &[Target],
    geometry: Option<&Geometry>,
    passes: Option<&[Pass]>,
    dimension: Option<&str>,
) {
    let known: HashSet<&str> = targets.iter().map(|t| t.id.as_str()).collect();
    let mut check = |named: &Named| {
        if known.contains(named.id.as_str()) {
            return;
        }
        let place = dimension.map_or(String::new(), |d| format!(" in {d}"));
        let mut problem = problem_at(
            cx.file,
            named.at,
            format!("`{}` is not a [[target]]{place}", named.id),
        );
        if named.id == "screen" {
            problem = problem
                .hint("only the last final pass writes the screen, and it leaves out `writes`");
        }
        cx.report_problem(problem);
    };
    for (_, group) in geometry.into_iter().flat_map(|g| g.groups()) {
        group.writes.iter().for_each(&mut check);
    }
    let passes = passes.unwrap_or_default();
    let last_final = passes.iter().rposition(|p| p.anchor == Anchor::Final);
    for (i, pass) in passes.iter().enumerate() {
        if Some(i) != last_final {
            pass.writes.iter().for_each(&mut check);
        }
        if let PassKind::Compute {
            dispatch: Dispatch::Cover(target),
            ..
        } = &pass.kind
        {
            check(target);
        }
    }
}

fn check_screens(cx: &mut Ctx<'_>, screens: &[Screen], options: &Options) {
    check_unique(
        cx,
        screens.iter().map(|s| (s.id.as_str(), s.at)),
        "[[screen]]",
    );
    let index: HashMap<&str, usize> = screens
        .iter()
        .enumerate()
        .rev()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut placed = HashSet::new();
    for screen in screens {
        for item in &screen.items {
            match item {
                ScreenItem::Option(named) => {
                    if options.get(&named.id).is_some() {
                        placed.insert(named.id.as_str());
                    } else {
                        cx.report_at(named.at, format!("`{}` is not an option", named.id));
                    }
                }
                ScreenItem::Link(named) if !index.contains_key(named.id.as_str()) => {
                    cx.report_at(named.at, format!("`{}` is not a [[screen]]", named.id));
                }
                ScreenItem::Link(_) | ScreenItem::Gap => {}
            }
        }
    }

    let mut reached = vec![false; screens.len()];
    let mut queue = Vec::new();
    if !screens.is_empty() {
        reached[0] = true;
        queue.push(0);
    }
    while let Some(at) = queue.pop() {
        for item in &screens[at].items {
            if let ScreenItem::Link(named) = item
                && let Some(&next) = index.get(named.id.as_str())
                && !reached[next]
            {
                reached[next] = true;
                queue.push(next);
            }
        }
    }
    for (screen, reached) in screens.iter().zip(reached) {
        if !reached {
            cx.report_problem(
                problem_at(
                    cx.file,
                    screen.at,
                    format!(
                        "screen `{}` cannot be reached from `{}`",
                        screen.id, screens[0].id
                    ),
                )
                .hint("link it with \"screen:<id>\" from a screen that can"),
            );
        }
    }
    for decl in options.iter() {
        if !placed.contains(decl.id.as_str()) {
            cx.report_problem(
                problem_at(
                    cx.file,
                    decl.at,
                    format!("option `{}` is on no [[screen]]", decl.id),
                )
                .hint("add its id to a screen's `items`"),
            );
        }
    }
}

#[derive(Clone, Debug, Default)]
#[allow(
    dead_code,
    reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
)]
pub(crate) struct Strings {
    options: HashMap<String, OptionStrings>,
    screens: HashMap<String, String>,
    presets: HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
#[allow(
    dead_code,
    reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
)]
struct OptionStrings {
    name: Option<String>,
    help: Option<String>,
    values: HashMap<String, String>,
}

impl Strings {
    pub(crate) fn parse(file: &str, text: &str, manifest: &Manifest) -> (Strings, Vec<Problem>) {
        let mut cx = Ctx {
            file,
            text,
            problems: Vec::new(),
        };
        let mut out = Strings::default();
        let doc = match Document::parse(text) {
            Ok(doc) => doc,
            Err(e) => {
                cx.report(
                    e.span().map_or(0, |s| s.start),
                    e.message().trim().to_owned(),
                );
                return (out, cx.problems);
            }
        };
        let mut top = Fields::new(doc.as_table(), 0, "the top level of a lang file");

        for (id, key_at, table, at) in sub_tables(&mut cx, top.get("option")) {
            let Some(decl) = manifest.options.get(id) else {
                cx.report(key_at, format!("`{id}` is not an option"));
                continue;
            };
            let mut f = Fields::new(table, at, format!("[option.{id}]"));
            let mut strings = OptionStrings {
                name: f
                    .get("name")
                    .and_then(|e| e.string(&mut cx))
                    .map(|s| s.value),
                help: f
                    .get("help")
                    .and_then(|e| e.string(&mut cx))
                    .map(|s| s.value),
                values: HashMap::new(),
            };
            if let Kind::Choice { values } = &decl.kind {
                let named = f.get("value").and_then(|e| e.table(&mut cx));
                for (value, item) in named.map(|(t, _)| t.iter()).into_iter().flatten() {
                    let value_at = named
                        .and_then(|(t, _)| t.key(value)?.span())
                        .map_or(at, |s| s.start);
                    let entry = Entry {
                        key: value,
                        at: value_at,
                        item,
                    };
                    if !values.iter().any(|v| v == value) {
                        cx.report(value_at, format!("`{value}` is not a value of `{id}`"));
                    } else if let Some(name) = entry.string(&mut cx) {
                        strings.values.insert(value.to_owned(), name.value);
                    }
                }
            }
            f.finish(&mut cx);
            out.options.insert(id.to_owned(), strings);
        }
        for (id, key_at, table, at) in sub_tables(&mut cx, top.get("screen")) {
            if !manifest.screens.iter().any(|s| s.id == id) {
                cx.report(key_at, format!("`{id}` is not a [[screen]]"));
                continue;
            }
            let mut f = Fields::new(table, at, format!("[screen.{id}]"));
            if let Some(name) = f.get("name").and_then(|e| e.string(&mut cx)) {
                out.screens.insert(id.to_owned(), name.value);
            }
            f.finish(&mut cx);
        }
        for (id, key_at, table, at) in sub_tables(&mut cx, top.get("preset")) {
            if !manifest.presets.iter().any(|p| p.id == id) {
                cx.report(key_at, format!("`{id}` is not a [[preset]]"));
                continue;
            }
            let mut f = Fields::new(table, at, format!("[preset.{id}]"));
            if let Some(name) = f.get("name").and_then(|e| e.string(&mut cx)) {
                out.presets.insert(id.to_owned(), name.value);
            }
            f.finish(&mut cx);
        }
        top.finish(&mut cx);
        (out, cx.problems)
    }
}

fn sub_tables<'a>(
    cx: &mut Ctx<'_>,
    entry: Option<Entry<'a>>,
) -> Vec<(&'a str, usize, &'a dyn TableLike, usize)> {
    let Some((table, table_at)) = entry.and_then(|e| e.table(cx)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, item) in table.iter() {
        let at = table
            .key(key)
            .and_then(|k| k.span())
            .map_or(table_at, |s| s.start);
        if let Some((sub, sub_at)) = (Entry { key, at, item }).table(cx) {
            out.push((key, at, sub, sub_at));
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
#[allow(
    dead_code,
    reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
)]
pub(crate) struct Lang {
    locales: HashMap<String, Strings>,
}

impl Lang {
    pub(crate) fn new(locales: HashMap<String, Strings>) -> Lang {
        Lang { locales }
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
    )]
    fn find<'a>(
        &'a self,
        locale: &str,
        get: impl Fn(&'a Strings) -> Option<&'a str>,
    ) -> Option<&'a str> {
        [locale, FALLBACK_LOCALE]
            .into_iter()
            .filter_map(|l| self.locales.get(l))
            .find_map(get)
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
    )]
    pub(crate) fn option_name<'a>(&'a self, locale: &str, decl: &'a Decl) -> &'a str {
        self.find(locale, |s| s.options.get(&decl.id)?.name.as_deref())
            .or(decl.name.as_deref())
            .unwrap_or(&decl.id)
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
    )]
    pub(crate) fn option_help<'a>(&'a self, locale: &str, id: &str) -> Option<&'a str> {
        self.find(locale, |s| s.options.get(id)?.help.as_deref())
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
    )]
    pub(crate) fn value_name<'a>(&'a self, locale: &str, option: &str, value: &'a str) -> &'a str {
        self.find(locale, |s| {
            s.options.get(option)?.values.get(value).map(String::as_str)
        })
        .unwrap_or(value)
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
    )]
    pub(crate) fn screen_name<'a>(&'a self, locale: &str, id: &'a str) -> &'a str {
        self.find(locale, |s| s.screens.get(id).map(String::as_str))
            .unwrap_or(id)
    }

    #[allow(
        dead_code,
        reason = "phase 4 of docs/SHADER_FORMAT.md, the options screen, reads the labels"
    )]
    pub(crate) fn preset_name<'a>(&'a self, locale: &str, preset: &'a Preset) -> &'a str {
        self.find(locale, |s| s.presets.get(&preset.id).map(String::as_str))
            .or(preset.name.as_deref())
            .unwrap_or(&preset.id)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const EXAMPLE: &str = r#"format = 1
api = "1.0"
id = "example_deferred"
name = "Example Deferred"
version = "0.3.0"

[requires]
compute = false
outputs = 2
formats = ["rg16float"]

[[target]]
id = "albedo"
format = "rgba8unorm"

[[target]]
id = "normal"
format = "rg16float"

[[target]]
id = "lit"
format = ["rgba16float", "rgba8unorm"]
history = true

[geometry.opaque]
file = "geometry.wgsl"
writes = ["albedo", "normal"]
stages = ["terrain", "entity"]

[geometry.translucent]
file = "translucent.wgsl"
writes = ["lit"]

[shadow]
resolution = 2048
distance = 128.0
when = "shadows"

[[pass]]
id = "lighting"
at = "deferred"
file = "lighting.wgsl"
writes = ["lit"]

[[pass]]
id = "tonemap"
at = "final"
file = "tonemap.wgsl"

[[option]]
id = "shadows"
type = "bool"
default = true

[[option]]
id = "exposure"
type = "float"
default = 1.0
min = 0.25
max = 4.0
step = 0.05

[[screen]]
id = "main"
items = ["shadows", "exposure"]
"#;

    pub(crate) fn parse_ok(text: &str) -> Manifest {
        let parsed = Manifest::parse(text);
        assert!(parsed.problems.is_empty(), "{:#?}", parsed.problems);
        parsed.manifest.unwrap()
    }

    fn problems(text: &str) -> Vec<Problem> {
        Manifest::parse(text).problems
    }

    fn with(extra: &str) -> String {
        format!("{EXAMPLE}{extra}")
    }

    fn only(text: &str) -> String {
        let found = problems(text);
        assert_eq!(found.len(), 1, "{found:#?}");
        let p = &found[0];
        let at = p.at.expect("a position");
        format!("{}:{}: {}", at.line, at.column, p.message)
    }

    #[test]
    fn the_spec_example_reads_cleanly() {
        let m = parse_ok(EXAMPLE);
        assert_eq!(m.id, "example_deferred");
        assert_eq!(m.api, Api { major: 1, minor: 0 });
        let layout = m.layout("overworld");
        assert_eq!(layout.targets.len(), 3);
        assert_eq!(layout.targets[0].formats, [Format::Rgba8Unorm]);
        assert_eq!(
            layout.targets[2].formats,
            [Format::Rgba16Float, Format::Rgba8Unorm]
        );
        assert!(layout.targets[2].history);
        let opaque = layout.geometry.opaque.as_ref().unwrap();
        assert_eq!(opaque.stages, [Stage::Terrain, Stage::Entity]);
        assert!(
            layout
                .geometry
                .groups()
                .map(|(group, _)| group)
                .eq([api::Group::Opaque, api::Group::Translucent])
        );
        assert_eq!(layout.passes[1].anchor, Anchor::Final);
        assert!(layout.passes[1].writes.is_empty());
        assert_eq!(
            layout.shadow.unwrap().when.as_ref().unwrap().source(),
            "shadows"
        );
        assert_eq!(
            m.manifest_options().into_iter().collect::<Vec<_>>(),
            ["shadows"]
        );
        let exposure = m.options.get("exposure").unwrap();
        assert_eq!(
            exposure.kind,
            Kind::Float {
                min: 0.25,
                max: 4.0,
                step: 0.05
            }
        );
    }

    #[test]
    fn the_spec_example_declares_what_it_needs() {
        let m = parse_ok(EXAMPLE);
        let needs = m.needs();
        assert_eq!(needs.outputs, 2);
        assert_eq!(needs.declared_formats(), [Format::Rg16Float]);
        let found: Vec<String> = m
            .requires_problems()
            .into_iter()
            .map(|p| p.message)
            .collect();
        assert!(found.is_empty(), "{found:#?}");

        let over = EXAMPLE.replace(
            "outputs = 2\nformats = [\"rg16float\"]",
            "outputs = 3\nformats = [\"rgba16float\"]",
        );
        let found: Vec<String> = parse_ok(&over)
            .requires_problems()
            .into_iter()
            .map(|p| p.message)
            .collect();
        assert_eq!(found.len(), 3, "{found:#?}");
        assert!(found[0].contains("outputs = 3"), "{found:#?}");
        assert!(found[1].contains("rg16float"), "{found:#?}");
        assert!(found[2].contains("rgba16float"), "{found:#?}");
    }

    #[test]
    fn target_formats_are_checked_where_they_are() {
        assert_eq!(
            only(&EXAMPLE.replace("format = \"rgba8unorm\"\n", "")),
            "12:1: [[target]] needs `format`"
        );
        assert!(
            problems(&EXAMPLE.replace("format = \"rgba8unorm\"\n", ""))[0]
                .hint
                .as_deref()
                .unwrap()
                .contains("rgba8unorm")
        );
        assert_eq!(
            only(&EXAMPLE.replace("format = \"rgba8unorm\"", "format = 3")),
            "14:10: `format` must be a format name or a list of them"
        );
        let lit = |list: &str| EXAMPLE.replace("[\"rgba16float\", \"rgba8unorm\"]", list);
        assert_eq!(
            only(&lit("[]")),
            "22:1: `format` lists 0 formats; a list has 1 to 4"
        );
        assert_eq!(
            only(&lit(
                "[\"rgba16float\", \"rgba8unorm\", \"rgb10a2unorm\", \"r16float\", \"rg16float\"]"
            )),
            "22:1: `format` lists 5 formats; a list has 1 to 4"
        );
        assert_eq!(
            only(&lit("[\"rgba16float\", \"rgba16float\"]")),
            "22:26: rgba16float is listed twice"
        );
        assert_eq!(
            only(&lit("[\"rgba16float\", \"depth\"]")),
            "22:26: a [[target]] is a colour target"
        );
        assert_eq!(
            only(&lit("[\"rgba16float\", \"rgba8\"]")),
            "22:26: `rgba8` is not a target format"
        );
        assert_eq!(
            only(&lit("[\"rgba16float\", \"rgba32uint\"]")),
            "22:1: the formats in `format` mix float and unsigned integer"
        );
        let m = parse_ok(&lit("[\"rgba8unorm\"]"));
        assert_eq!(
            m.layout("overworld").targets[2].formats,
            [Format::Rgba8Unorm]
        );
    }

    #[test]
    fn requires_names_the_last_entry_of_a_list() {
        let text = EXAMPLE.replace(
            "[\"rgba16float\", \"rgba8unorm\"]",
            "[\"rgba32float\", \"rgba16float\"]",
        );
        let found: Vec<String> = parse_ok(&text)
            .requires_problems()
            .into_iter()
            .map(|p| p.message)
            .collect();
        assert_eq!(
            found,
            ["a target needs rgba16float but [requires] `formats` does not list it"]
        );
        let text = text.replace(
            "formats = [\"rg16float\"]",
            "formats = [\"rg16float\", \"rgba16float\"]",
        );
        assert!(parse_ok(&text).requires_problems().is_empty());
        let text = text.replace(
            "formats = [\"rg16float\", \"rgba16float\"]",
            "formats = [\"rg16float\", \"rgba16float\", \"rgba32float\"]",
        );
        let found: Vec<String> = parse_ok(&text)
            .requires_problems()
            .into_iter()
            .map(|p| p.message)
            .collect();
        assert_eq!(found.len(), 1, "{found:#?}");
        assert!(
            found[0].starts_with("[requires] lists rgba32float but no target needs it"),
            "{found:#?}"
        );
    }

    #[test]
    fn unknown_keys_are_reported_where_they_are() {
        let text = format!("colour = 1\n{EXAMPLE}");
        assert_eq!(
            only(&text),
            "1:1: `colour` is not a key of the top level of pack.toml"
        );
        assert_eq!(
            only(&with("colour = 1\n")),
            "66:1: `colour` is not a key of [[screen]]"
        );
        let text = EXAMPLE.replace("history = true", "history = true\nhistroy = true");
        assert_eq!(only(&text), "24:1: `histroy` is not a key of [[target]]");
    }

    #[test]
    fn keys_belong_to_their_table() {
        let text = EXAMPLE.replace(
            "at = \"deferred\"",
            "at = \"deferred\"\nworkgroup = [8, 8, 1]",
        );
        assert_eq!(
            only(&text),
            "42:1: `workgroup` is not a key of a screen [[pass]]"
        );
        let text = EXAMPLE.replace("type = \"bool\"", "type = \"bool\"\nmin = 0");
        assert!(only(&text).ends_with("`min` is not a key of a bool [[option]]"));
        let text = EXAMPLE.replace("[shadow]\n", "[shadow]\nhistory = true\n");
        assert!(only(&text).ends_with("`history` is not a key of [shadow]"));
    }

    #[test]
    fn bad_ids_and_ranges() {
        let text = EXAMPLE.replace("id = \"albedo\"", "id = \"Albedo\"");
        let found = problems(&text);
        assert!(
            found[0].message.contains("not a valid target id"),
            "{found:#?}"
        );
        let text = EXAMPLE.replace("resolution = 2048", "resolution = 100");
        assert!(only(&text).contains("outside 256..8192"));
        let text = EXAMPLE.replace("id = \"example_deferred\"", "id = \"Example\"");
        assert!(only(&text).contains("not a valid pack id"));
        let text = EXAMPLE.replace("default = 1.0", "default = 9.0");
        assert!(
            problems(&text)
                .iter()
                .any(|p| p.message == "default: 9 is outside 0.25..4")
        );
        let text = EXAMPLE.replace("id = \"normal\"", "id = \"scene_depth\"");
        assert!(
            problems(&text)
                .iter()
                .any(|p| p.message.contains("a name the engine declares"))
        );
        let text = EXAMPLE.replace("id = \"normal\"", "id = \"torch_normal\"");
        assert!(
            problems(&text)
                .iter()
                .any(|p| p.message.contains("starts with `torch_`"))
        );
    }

    #[test]
    fn newer_formats_and_apis_are_refused() {
        assert!(only(&EXAMPLE.replace("format = 1", "format = 2")).contains("formats up to 1"));
        assert!(only(&EXAMPLE.replace("api = \"1.0\"", "api = \"1.3\"")).contains("API 1.3"));
        assert!(
            only(&EXAMPLE.replace("api = \"1.0\"", "api = \"one\"")).contains("not an API version")
        );
    }

    #[test]
    fn writes_must_name_targets() {
        let text = EXAMPLE.replace(
            "writes = [\"lit\"]\n\n[shadow]",
            "writes = [\"lti\"]\n\n[shadow]",
        );
        assert_eq!(only(&text), "32:11: `lti` is not a [[target]]");
        let text = EXAMPLE.replace(
            "file = \"tonemap.wgsl\"",
            "file = \"tonemap.wgsl\"\nwrites = [\"screen\"]",
        );
        assert!(only(&text).contains("is the last final pass and writes the screen"));
        let text = EXAMPLE.replace(
            "writes = [\"lit\"]\n\n[[pass]]",
            "writes = [\"screen\"]\n\n[[pass]]",
        );
        assert!(only(&text).contains("`screen` is not a [[target]]"));
    }

    #[test]
    fn the_screen_graph_is_checked() {
        let text = with("\n[[screen]]\nid = \"lost\"\nitems = []\n");
        assert!(only(&text).contains("screen `lost` cannot be reached"));
        let text = EXAMPLE.replace(
            "items = [\"shadows\", \"exposure\"]",
            "items = [\"shadows\", \"\"]",
        );
        assert!(only(&text).contains("option `exposure` is on no [[screen]]"));
        let text = with("\n[[screen]]\nid = \"more\"\nitems = [\"exposure\"]\n").replace(
            "items = [\"shadows\", \"exposure\"]",
            "items = [\"shadows\", \"screen:more\"]",
        );
        parse_ok(&text);
        let text = EXAMPLE.replace(
            "items = [\"shadows\", \"exposure\"]",
            "items = [\"shadows\", \"exposure\", \"screen:nope\"]",
        );
        assert!(only(&text).contains("`nope` is not a [[screen]]"));
    }

    #[test]
    fn condition_errors_point_inside_the_string() {
        let text = EXAMPLE.replace("when = \"shadows\"", "when = \"shadows && exposure\"");
        assert_eq!(
            only(&text),
            "37:20: `exposure` is a float option; a bare id must be a bool option"
        );
    }

    #[test]
    fn options_are_typed_by_their_type() {
        let choice = "\n[[option]]\nid = \"quality\"\ntype = \"choice\"\nvalues = [\"low\", \"high\"]\ndefault = \"mid\"\n";
        assert!(
            problems(&with(choice))
                .iter()
                .any(|p| p.message.contains("`mid` is not one of low, high"))
        );
        let int = "\n[[option]]\nid = \"samples\"\ntype = \"int\"\nmin = 4\nmax = 2\ndefault = 3\n";
        assert!(
            problems(&with(int))
                .iter()
                .any(|p| p.message.contains("max (2) must be greater than min (4)"))
        );
        let clash = "\n[[option]]\nid = \"a_b\"\ntype = \"choice\"\nvalues = [\"c\", \"d\"]\ndefault = \"c\"\n\
                     \n[[option]]\nid = \"a\"\ntype = \"choice\"\nvalues = [\"b_c\", \"e\"]\ndefault = \"e\"\n";
        assert!(
            problems(&with(clash))
                .iter()
                .any(|p| p.message.contains("`A_B_C`"))
        );
    }

    #[test]
    fn presets_are_checked_against_options() {
        let good =
            "\n[[preset]]\nid = \"low\"\nname = \"Low\"\nset = { shadows = false, exposure = 1 }\n";
        let m = parse_ok(&with(good));
        assert_eq!(m.presets[0].set.len(), 2);
        let bad = "\n[[preset]]\nid = \"low\"\nset = { shadow = false, exposure = 8.0 }\n";
        assert_eq!(problems(&with(bad)).len(), 2);
    }

    #[test]
    fn dimensions_replace_parts() {
        let nether = "\n[dimension.the_nether.shadow]\nresolution = 512\n\n[[dimension.the_nether.pass]]\nid = \"tonemap\"\nat = \"final\"\nfile = \"tonemap.wgsl\"\n";
        let m = parse_ok(&with(nether));
        assert_eq!(
            m.layout("minecraft:the_nether").shadow.unwrap().resolution,
            Setting::Fixed(512.0)
        );
        assert_eq!(m.layout("the_nether").passes.len(), 1);
        assert_eq!(m.layout("the_nether").targets.len(), 3);
        assert_eq!(m.layout("the_end").passes.len(), 2);

        let m = parse_ok(&with("\n[dimension.the_nether]\nshadow = false\n"));
        assert!(m.layout("the_nether").shadow.is_none());
        assert!(m.layout("overworld").shadow.is_some());
        let top = EXAMPLE
            .replace(
                "[shadow]\nresolution = 2048\ndistance = 128.0\nwhen = \"shadows\"\n",
                "",
            )
            .replace(
                "version = \"0.3.0\"\n",
                "version = \"0.3.0\"\nshadow = false\n",
            );
        assert!(only(&top).ends_with("`shadow` is a [shadow] table here"));

        let bad = "\n[dimension.nether.shadow]\nresolution = 512\n";
        assert!(only(&with(bad)).contains("`nether` is not a dimension"));
        let targets = "\n[[dimension.the_end.target]]\nid = \"lit\"\n";
        let found = problems(&with(targets));
        assert!(
            found
                .iter()
                .any(|p| p.message == "`albedo` is not a [[target]] in minecraft:the_end"),
            "{found:#?}"
        );
        let key = "\n[dimension.the_end]\noption = []\n";
        assert!(only(&with(key)).contains("`option` is not a key of [dimension.the_end]"));
    }

    #[test]
    fn shadow_numbers_can_come_from_options() {
        let size = "\n[[option]]\nid = \"size\"\ntype = \"int\"\ndefault = 1024\nmin = 512\nmax = 2048\n\
                    \n[[option]]\nid = \"reach\"\ntype = \"float\"\ndefault = 96.0\nmin = 32.0\nmax = 128.0\n";
        let text = with(size)
            .replace(
                "resolution = 2048\ndistance = 128.0",
                "resolution = \"size\"\ndistance = \"reach\"",
            )
            .replace(
                "items = [\"shadows\", \"exposure\"]",
                "items = [\"shadows\", \"exposure\", \"size\", \"reach\"]",
            );
        let m = parse_ok(&text);
        let shadow = m.layout("overworld").shadow.unwrap();
        let mut values = Values::defaults(&m.options);
        assert_eq!(
            (shadow.resolution(&values), shadow.distance(&values)),
            (1024, 96.0)
        );
        values.set(&m.options, "size", Value::Int(512)).unwrap();
        assert_eq!(shadow.resolution(&values), 512);
        assert_eq!(
            m.manifest_options().into_iter().collect::<Vec<_>>(),
            ["reach", "shadows", "size"]
        );

        let wide = text.replace("max = 2048", "max = 9000");
        assert!(only(&wide).ends_with(
            "option `size` runs 512..9000, and `resolution` must stay inside 256..8192"
        ));
        let float = text.replace("resolution = \"size\"", "resolution = \"reach\"");
        assert!(
            only(&float).ends_with(
                "`resolution` is set by `reach`, a float option; it takes an int option"
            )
        );
        let missing = text.replace("distance = \"reach\"", "distance = \"far\"");
        assert!(only(&missing).ends_with("`distance` names no option `far`"));
    }

    #[test]
    fn presets_can_be_a_tiers_default() {
        let presets = "\n[[preset]]\nid = \"low\"\nset = { shadows = false }\ndefault_on = [\"webgl2\"]\n\
                       \n[[preset]]\nid = \"high\"\nset = { shadows = true }\n";
        let m = parse_ok(&with(presets));
        assert_eq!(
            m.default_values(TierName::WebGl2).get("shadows"),
            Some(Value::Bool(false))
        );
        assert_eq!(
            m.default_values(TierName::Desktop).get("shadows"),
            Some(Value::Bool(true))
        );
        let unknown = with(&presets.replace("[\"webgl2\"]", "[\"webgl3\"]"));
        assert!(only(&unknown).ends_with("`webgl3` is not a tier"));
        let twice = with(&format!("{presets}default_on = [\"webgl2\"]\n"));
        assert!(only(&twice).ends_with("webgl2 already defaults to preset `low`"));
    }

    #[test]
    fn the_opaque_group_lists_its_sky_parts() {
        let m = parse_ok(EXAMPLE);
        assert_eq!(
            m.layout("overworld")
                .geometry
                .opaque
                .as_ref()
                .unwrap()
                .sky_parts,
            SkyPart::ALL
        );
        let listed = EXAMPLE.replace(
            "stages = [\"terrain\", \"entity\"]",
            "stages = [\"terrain\", \"entity\"]\nsky_parts = [\"sun\", \"sunrise\"]",
        );
        let m = parse_ok(&listed);
        assert_eq!(
            m.layout("overworld")
                .geometry
                .opaque
                .as_ref()
                .unwrap()
                .sky_parts,
            [SkyPart::Sunrise, SkyPart::Sun]
        );
        let bad = EXAMPLE.replace(
            "stages = [\"terrain\", \"entity\"]",
            "stages = [\"terrain\", \"entity\"]\nsky_parts = [\"sky\"]",
        );
        assert!(only(&bad).ends_with("`sky` is not a sky part"));
        let translucent = EXAMPLE.replace(
            "writes = [\"lit\"]\n\n[shadow]",
            "writes = [\"lit\"]\nsky_parts = []\n\n[shadow]",
        );
        assert!(only(&translucent).ends_with("`sky_parts` is not a key of [geometry.translucent]"));
    }

    #[test]
    fn compute_passes() {
        let compute = "\n[[pass]]\nid = \"voxelize\"\nat = \"begin\"\nkind = \"compute\"\nfile = \"voxel.wgsl\"\nwrites = [\"albedo\"]\nworkgroup = [8, 8, 1]\ndispatch = \"target:albedo\"\n";
        let m = Manifest::parse(&with(compute)).manifest.unwrap();
        let pass = m.passes.iter().find(|p| p.id == "voxelize").unwrap();
        assert!(
            matches!(&pass.kind, PassKind::Compute { workgroup: [8, 8, 1], dispatch: Dispatch::Cover(t) } if t.id == "albedo")
        );
        assert!(m.needs().compute && m.needs().storage_textures);
        let big = compute.replace("[8, 8, 1]", "[16, 16, 2]");
        assert!(
            problems(&with(&big))
                .iter()
                .any(|p| p.message.contains("at most 256 invocations"))
        );
    }

    #[test]
    fn materials_parse_block_entries() {
        let text = with(
            "\n[[material]]\nid = \"foliage\"\nblocks = [\"#leaves\", \"short_grass\", \"tall_grass:half=upper\", \"minecraft:tall_grass:half=lower\"]\nentities = []\nitems = []\n",
        );
        let m = parse_ok(&text);
        assert_eq!(
            m.materials[0].blocks,
            [
                BlockEntry::Tag("leaves".into()),
                BlockEntry::Block("short_grass".into()),
                BlockEntry::State {
                    block: "tall_grass".into(),
                    property: "half".into(),
                    value: "upper".into()
                },
                BlockEntry::State {
                    block: "minecraft:tall_grass".into(),
                    property: "half".into(),
                    value: "lower".into()
                },
            ]
        );
        let bad = with("\n[[material]]\nid = \"x\"\nblocks = [\"Stone\", \"a:=b\"]\n");
        assert_eq!(problems(&bad).len(), 2);
    }

    #[test]
    fn a_toml_syntax_error_has_a_position() {
        let parsed = Manifest::parse("format = 1\nid = \n");
        assert!(parsed.manifest.is_none());
        assert_eq!(parsed.problems.len(), 1);
        assert_eq!(parsed.problems[0].at.map(|p| p.line), Some(2));
    }

    #[test]
    fn lang_files_fall_back_in_order() {
        let m = parse_ok(&with(
            "\n[[preset]]\nid = \"low\"\nname = \"Low inline\"\nset = { shadows = false }\n",
        ));
        let en = "[option.shadows]\nname = \"Shadows\"\nhelp = \"Sun and moon shadows.\"\n[screen.main]\nname = \"Example\"\n";
        let de = "[option.shadows]\nname = \"Schatten\"\n";
        let (en, p) = Strings::parse("lang/en_us.toml", en, &m);
        assert!(p.is_empty(), "{p:#?}");
        let (de, p) = Strings::parse("lang/de_de.toml", de, &m);
        assert!(p.is_empty(), "{p:#?}");
        let lang = Lang::new(HashMap::from([
            ("en_us".to_owned(), en),
            ("de_de".to_owned(), de),
        ]));
        let shadows = m.options.get("shadows").unwrap();
        let exposure = m.options.get("exposure").unwrap();
        assert_eq!(lang.option_name("de_de", shadows), "Schatten");
        assert_eq!(lang.option_name("fr_fr", shadows), "Shadows");
        assert_eq!(
            lang.option_help("de_de", "shadows"),
            Some("Sun and moon shadows.")
        );
        assert_eq!(lang.option_name("de_de", exposure), "exposure");
        assert_eq!(lang.screen_name("de_de", "main"), "Example");
        assert_eq!(lang.preset_name("de_de", &m.presets[0]), "Low inline");
        assert_eq!(lang.value_name("de_de", "quality", "low"), "low");
    }

    #[test]
    fn lang_files_are_checked_against_the_manifest() {
        let m = parse_ok(EXAMPLE);
        let text = "[option.shadow]\nname = \"x\"\n[option.shadows]\nlabel = \"x\"\n[screen.main]\nname = \"m\"\n[other]\n";
        let (_, found) = Strings::parse("lang/en_us.toml", text, &m);
        let messages: Vec<&str> = found.iter().map(|p| p.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "`shadow` is not an option",
                "`label` is not a key of [option.shadows]",
                "`other` is not a key of the top level of a lang file"
            ]
        );
    }
}
