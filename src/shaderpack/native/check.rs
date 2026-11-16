use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use super::api::{self, Binding, BindingKind, ModuleKind, PackInterface};
use super::caps::{self, Needs, TierName};
use super::compose::{self, Composed};
use super::graph::{self, Reads};
use super::interface::{self, ModuleSpec};
use super::manifest::{self, Layout, Manifest};
use super::options::Values;
use super::package::{self, Package};
use super::problem::Problem;

#[derive(Clone, Debug)]
pub(crate) struct Report {
    pub(crate) pack: String,
    pub(crate) problems: Vec<Problem>,
    pub(crate) skipped: Vec<String>,
    pub(crate) tiers: Vec<TierName>,
    pub(crate) modules: Vec<ModuleReport>,
}

#[derive(Clone, Debug)]
pub(crate) struct ModuleReport {
    pub(crate) file: String,
    pub(crate) role: String,
    pub(crate) layouts: Vec<String>,
    pub(crate) text: String,
    pub(crate) entry_points: Vec<String>,
    pub(crate) bindings: Vec<Binding>,
    pub(crate) valid: bool,
}

pub(crate) const TOP_LEVEL: &str = "top level";

impl Report {
    fn add(&mut self, problems: impl IntoIterator<Item = Problem>) {
        for problem in problems {
            if !self.problems.contains(&problem) {
                self.problems.push(problem);
            }
        }
    }

    fn skip(&mut self, why: String) {
        if !self.skipped.contains(&why) {
            self.skipped.push(why);
        }
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        let problems: Vec<serde_json::Value> = self
            .problems
            .iter()
            .map(|p| {
                serde_json::json!({
                    "file": p.file,
                    "line": p.at.map(|at| at.line),
                    "column": p.at.map(|at| at.column),
                    "message": p.message,
                    "hint": p.hint,
                })
            })
            .collect();
        let modules: Vec<serde_json::Value> = self
            .modules
            .iter()
            .map(|m| {
                let bindings: Vec<serde_json::Value> = m
                    .bindings
                    .iter()
                    .map(|b| serde_json::json!({ "name": b.name, "binding": b.binding }))
                    .collect();
                serde_json::json!({
                    "file": m.file,
                    "role": m.role,
                    "layouts": m.layouts,
                    "valid": m.valid,
                    "entry_points": m.entry_points,
                    "bindings": bindings,
                    "text": m.text,
                })
            })
            .collect();
        let tiers: Vec<&str> = self.tiers.iter().map(|tier| tier.name()).collect();
        serde_json::json!({
            "pack": self.pack,
            "ok": self.problems.is_empty(),
            "problems": problems,
            "skipped": self.skipped,
            "tiers": tiers,
            "modules": modules,
        })
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.problems.len() {
            0 => writeln!(f, "{}: no problems", self.pack)?,
            1 => writeln!(f, "{}: 1 problem", self.pack)?,
            n => writeln!(f, "{}: {n} problems", self.pack)?,
        }
        let mut file = None;
        for problem in &self.problems {
            if file != Some(problem.file.as_str()) {
                file = Some(problem.file.as_str());
                let heading = if problem.file.is_empty() {
                    "the pack"
                } else {
                    problem.file.as_str()
                };
                writeln!(f, "\n{heading}")?;
            }
            writeln!(f, "  {problem}")?;
        }
        if !self.skipped.is_empty() {
            writeln!(f, "\nnot checked")?;
            for why in &self.skipped {
                writeln!(f, "  {why}")?;
            }
        }
        if !self.tiers.is_empty() {
            let tiers: Vec<&str> = self.tiers.iter().map(|tier| tier.name()).collect();
            writeln!(f, "\nruns on: {}", tiers.join(", "))?;
        }
        if !self.modules.is_empty() {
            writeln!(f, "\nmodules")?;
            for module in &self.modules {
                writeln!(
                    f,
                    "  {} ({}), in {}",
                    module.file,
                    module.role,
                    module.layouts.join(", ")
                )?;
                if !module.entry_points.is_empty() {
                    writeln!(f, "    entry points: {}", module.entry_points.join(", "))?;
                }
                if !module.bindings.is_empty() {
                    let bindings: Vec<String> = module
                        .bindings
                        .iter()
                        .map(|b| format!("{} @{}", b.name, b.binding))
                        .collect();
                    writeln!(f, "    binds: {}", bindings.join(", "))?;
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn run_cli(path: &str, json: bool) -> i32 {
    let report = check(Path::new(path));
    if json {
        println!("{:#}", report.json());
    } else {
        print!("{report}");
    }
    if report.problems.is_empty() { 0 } else { 1 }
}

pub(crate) fn check(path: &Path) -> Report {
    let mut report = Report {
        pack: path.display().to_string(),
        problems: Vec::new(),
        skipped: Vec::new(),
        tiers: Vec::new(),
        modules: Vec::new(),
    };
    let opened = package::open(path);
    report.add(opened.problems);
    match opened.package {
        None => report.skip(
            "the module and pack-wide checks: the pack could not be read as far as its manifest"
                .to_owned(),
        ),
        Some(_) if report.problems.iter().any(|p| p.file == manifest::FILE) => {
            report.skip(format!(
                "the module and pack-wide checks: {} has problems",
                manifest::FILE
            ));
        }
        Some(package) => check_package(&package, &mut report),
    }
    report
        .problems
        .sort_by(|a, b| (a.file.as_str(), a.at).cmp(&(b.file.as_str(), b.at)));
    report
}

struct Checked {
    kind: ModuleKind,
    report: ModuleReport,
    options: Option<Vec<usize>>,
    reads: Option<HashSet<String>>,
    sampled: u32,
    files: Vec<String>,
}

fn check_package(package: &Package, report: &mut Report) {
    let manifest = &package.manifest;
    let mut needs = manifest.needs();
    let tier = caps::tiers_for(&needs)
        .first()
        .copied()
        .unwrap_or(TierName::Desktop);
    let read = |path: &str| package.wgsl(path);

    let mut checked: Vec<Checked> = Vec::new();
    let mut composed_all = true;
    let mut layouts: Vec<(Layout<'_>, Option<Reads>)> = Vec::new();
    for layout in manifest.layouts() {
        let layout_name = layout.dimension.unwrap_or(TOP_LEVEL).to_owned();
        let pack = interface::pack_interface(manifest, layout);
        let (specs, problems) = interface::modules(layout, &pack);
        report.add(problems);
        let mut reads = Some(Reads::new());
        for spec in specs {
            let composed = match compose::compose(&pack, &spec.file, &spec.kind, &read) {
                Ok(composed) => composed,
                Err(problems) => {
                    report.add(problems);
                    report.skip(format!(
                        "{} ({}): every later check, since it could not be composed",
                        spec.file, spec.role
                    ));
                    composed_all = false;
                    reads = None;
                    continue;
                }
            };
            let found = checked
                .iter()
                .position(|c| c.kind == spec.kind && c.report.text == composed.text());
            let index = match found {
                Some(index) => index,
                None => {
                    checked.push(check_module(&pack, &spec, &composed, tier, report));
                    checked.len() - 1
                }
            };
            let module = &mut checked[index];
            if !module.report.layouts.contains(&layout_name) {
                module.report.layouts.push(layout_name.clone());
            }
            if let Some(names) = &module.reads {
                if let Some(map) = reads.as_mut() {
                    map.insert(spec.node, names.clone());
                }
            } else {
                reads = None;
            }
        }
        layouts.push((layout, reads));
    }

    if composed_all {
        let used: HashSet<&str> = checked
            .iter()
            .flat_map(|c| c.files.iter().map(String::as_str))
            .collect();
        let unused: Vec<Problem> = package
            .wgsl_files()
            .filter(|file| !used.contains(file))
            .map(|file| {
                Problem::file(file, "nothing imports or names this file")
                    .hint("import it from a module's file, name it in pack.toml, or remove it")
            })
            .collect();
        report.add(unused);
    } else {
        report.skip(
            "unused files: a module could not be composed, so what it imports is not known"
                .to_owned(),
        );
    }

    if composed_all && checked.iter().all(|c| c.options.is_some()) {
        report.add(unused_options(manifest, &checked));
    } else {
        report.skip(
            "unused options: a module did not parse, so which overrides it declares is not known"
                .to_owned(),
        );
    }

    let value_sets = value_sets(manifest);
    for (layout, reads) in &layouts {
        let Some(reads) = reads else {
            let name = layout.dimension.unwrap_or(TOP_LEVEL);
            report.skip(format!("the pass graph of the {name} layout: one of its modules did not validate, so what it reads is not known"));
            continue;
        };
        for values in &value_sets {
            let (_, problems) = graph::plan(*layout, values, reads);
            report.add(problems);
        }
    }

    report.add(manifest.requires_problems());
    needs.sampled_textures = checked.iter().map(|c| c.sampled).max().unwrap_or(0);
    report.tiers = caps::tiers_for(&needs);
    if !caps::TIERS
        .iter()
        .any(|tier| tier.loads_packs && report.tiers.contains(&tier.name))
    {
        report.add([
            Problem::package("no tier that loads packs can run this pack").hint(describe(&needs)),
        ]);
    }
    report.modules = checked.into_iter().map(|c| c.report).collect();
}

fn unused_options(manifest: &Manifest, checked: &[Checked]) -> Vec<Problem> {
    let mut used: BTreeSet<&str> = manifest.condition_options();
    for decl in checked
        .iter()
        .flat_map(|c| c.options.iter().flatten())
        .filter_map(|&i| manifest.options.iter().nth(i))
    {
        used.insert(decl.id.as_str());
    }
    manifest
        .options
        .iter()
        .filter(|decl| !used.contains(decl.id.as_str()))
        .map(|decl| Problem {
            file: manifest::FILE.to_owned(),
            at: Some(decl.at),
            message: format!(
                "option `{}` is read by no `override` and no `when`",
                decl.id
            ),
            hint: Some(format!(
                "declare `override {}: {};` where it is used, or remove it",
                decl.id,
                decl.kind.wgsl_type()
            )),
        })
        .collect()
}

fn value_sets(manifest: &Manifest) -> Vec<Values> {
    let defaults = Values::defaults(&manifest.options);
    let mut sets = vec![defaults.clone()];
    for preset in &manifest.presets {
        let mut values = defaults.clone();
        values.apply(preset);
        if !sets.contains(&values) {
            sets.push(values);
        }
    }
    sets
}

fn describe(needs: &Needs) -> String {
    let formats: Vec<&str> = needs.formats.iter().map(|f| f.name()).collect();
    format!(
        "it needs {} outputs from one pass, {} sampled textures in one module, compute {}, storage textures {}, formats [{}]",
        needs.outputs,
        needs.sampled_textures,
        needs.compute,
        needs.storage_textures,
        formats.join(", ")
    )
}

fn capabilities(tier: TierName) -> naga::valid::Capabilities {
    match tier {
        TierName::Desktop | TierName::WebGpu => naga::valid::Capabilities::default(),
        TierName::WebGl2 | TierName::Android => naga::valid::Capabilities::empty(),
    }
}

fn guarded<R>(work: impl FnOnce() -> R) -> Result<R, String> {
    panic::catch_unwind(AssertUnwindSafe(work)).map_err(|payload| {
        payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "no message".to_owned())
    })
}

fn panicked(composed: &Composed, doing: &str, message: &str) -> Problem {
    Problem::file(
        composed.file(),
        format!("naga panicked {doing} this module: {message}"),
    )
    .hint("a naga bug: the module may still be wrong, but this check cannot say how")
}

#[derive(Default)]
struct Analysis {
    problems: Vec<Problem>,
    skipped: Option<&'static str>,
    options: Option<Vec<usize>>,
    used: Option<Vec<Binding>>,
}

fn check_module(
    pack: &PackInterface,
    spec: &ModuleSpec,
    composed: &Composed,
    tier: TierName,
    report: &mut Report,
) -> Checked {
    let analysis = analyse(pack, &spec.kind, composed, tier);
    if let Some(why) = analysis.skipped {
        report.skip(format!("{} ({}): {why}", spec.file, spec.role));
    }
    let validated = analysis.used.is_some();
    let valid = validated && analysis.problems.is_empty();
    report.add(analysis.problems);
    let bindings = analysis.used.unwrap_or_default();
    let sampled = bindings
        .iter()
        .filter(|b| matches!(b.kind, BindingKind::Texture(_)))
        .count() as u32;
    let reads: Option<HashSet<String>> =
        validated.then(|| bindings.iter().map(|b| b.name.clone()).collect());
    Checked {
        kind: spec.kind.clone(),
        options: analysis.options,
        reads,
        sampled,
        files: composed.files().map(str::to_owned).collect(),
        report: ModuleReport {
            file: spec.file.clone(),
            role: spec.role.clone(),
            layouts: Vec::new(),
            text: composed.text().to_owned(),
            entry_points: composed.entry_points().to_vec(),
            bindings,
            valid,
        },
    }
}

fn analyse(
    pack: &PackInterface,
    kind: &ModuleKind,
    composed: &Composed,
    tier: TierName,
) -> Analysis {
    let mut out = Analysis::default();
    let module = match guarded(|| naga::front::wgsl::parse_str(composed.text())) {
        Ok(Ok(module)) => module,
        Ok(Err(error)) => {
            pack_only(pack, kind, composed, &error, &mut out);
            return out;
        }
        Err(panic) => {
            out.problems.push(panicked(composed, "parsing", &panic));
            out.skipped = Some(
                "the API, signature, override and validation checks: naga panicked parsing it",
            );
            return out;
        }
    };
    out.problems.extend(api::check_rules(&module, composed));
    let signatures = api::check_signatures(&module, composed, pack, kind);
    let callable = signatures.is_empty();
    out.problems.extend(signatures);
    let overrides = api::check_overrides(&module, composed, pack);
    out.problems.extend(overrides.problems);
    out.options = Some(overrides.options);
    if !callable {
        out.skipped = Some(
            "validation: its functions do not have the signatures the engine's entry points call",
        );
        return out;
    }

    let allowed = capabilities(tier);
    let validated = guarded(|| {
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), allowed).validate(&module)
    });
    let info = match validated {
        Ok(Ok(info)) => info,
        Ok(Err(error)) => {
            out.problems.push(composed.validation_problem(&error));
            return out;
        }
        Err(panic) => {
            out.problems.push(panicked(composed, "validating", &panic));
            return out;
        }
    };
    let entries: Vec<&str> = composed.entry_points().iter().map(String::as_str).collect();
    let used = if entries.is_empty() {
        api::used_by_functions(&module, &info, composed, &kind.called())
    } else {
        api::used_globals(&module, &info, composed, &entries)
    };
    out.used = Some(used.into_iter().cloned().collect());
    out
}

fn pack_only(
    pack: &PackInterface,
    kind: &ModuleKind,
    composed: &Composed,
    full: &naga::front::wgsl::ParseError,
    out: &mut Analysis,
) {
    let module = match guarded(|| naga::front::wgsl::parse_str(composed.without_entries())) {
        Ok(Ok(module)) => module,
        Ok(Err(error)) => {
            out.problems.push(composed.parse_problem(&error));
            out.skipped =
                Some("the API, signature, override and validation checks: it does not parse");
            return;
        }
        Err(panic) => {
            out.problems.push(panicked(composed, "parsing", &panic));
            out.skipped = Some(
                "the API, signature, override and validation checks: naga panicked parsing it",
            );
            return;
        }
    };
    out.problems.extend(api::check_rules(&module, composed));
    let signatures = api::check_signatures(&module, composed, pack, kind);
    if signatures.is_empty() {
        out.problems.push(composed.parse_problem(full));
    } else {
        out.problems.extend(signatures);
    }
    let overrides = api::check_overrides(&module, composed, pack);
    out.problems.extend(overrides.problems);
    out.options = Some(overrides.options);
    out.skipped = Some("validation: the engine's entry points do not parse with it");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("shaderpacks/native")
            .join(name)
    }

    fn roles(report: &Report) -> Vec<&str> {
        report.modules.iter().map(|m| m.role.as_str()).collect()
    }

    fn names(module: &ModuleReport) -> Vec<&str> {
        module.bindings.iter().map(|b| b.name.as_str()).collect()
    }

    #[test]
    fn the_post_only_example_is_clean() {
        let report = check(&example("post_only"));
        assert!(
            report.problems.is_empty() && report.skipped.is_empty(),
            "{report}"
        );
        assert_eq!(
            report.tiers,
            [
                TierName::Desktop,
                TierName::WebGpu,
                TierName::WebGl2,
                TierName::Android
            ]
        );
        assert_eq!(
            roles(&report),
            ["composite pass `extract`", "final pass `tonemap`"]
        );
        assert!(
            report
                .modules
                .iter()
                .all(|m| m.valid && m.layouts == [TOP_LEVEL]),
            "{report}"
        );
        assert_eq!(
            names(&report.modules[0]),
            ["frame", "linear_clamp", "scene_color"]
        );
        assert_eq!(
            names(&report.modules[1]),
            ["frame", "linear_clamp", "scene_color", "bright"]
        );
        assert_eq!(
            report.modules[1].entry_points,
            [api::SCREEN_VERTEX, api::SCREEN_FRAGMENT]
        );
        assert!(report.modules[1].text.contains("fn luminance("));
        assert_eq!(report.json()["ok"], serde_json::Value::Bool(true));
    }

    #[test]
    fn the_deferred_example_is_clean_in_every_layout() {
        let report = check(&example("deferred"));
        assert!(
            report.problems.is_empty() && report.skipped.is_empty(),
            "{report}"
        );
        assert_eq!(
            report.tiers,
            [TierName::Desktop, TierName::WebGpu, TierName::Android]
        );
        assert_eq!(
            roles(&report),
            [
                "[geometry.opaque]",
                "[geometry.translucent]",
                "deferred pass `lighting`",
                "final pass `tonemap`"
            ]
        );
        assert!(
            report
                .modules
                .iter()
                .all(|m| m.layouts == [TOP_LEVEL, "minecraft:the_nether"]),
            "{report}"
        );
        assert!(report.modules[0].entry_points.is_empty());
        assert_eq!(
            names(&report.modules[2]),
            ["frame", "albedo", "normal", "lit_prev"]
        );
        assert_eq!(names(&report.modules[3]), ["frame", "lit"]);
    }

    #[test]
    fn the_broken_example_reports_each_problem_where_it_is() {
        let report = check(&example("broken"));
        let places: Vec<String> = report
            .problems
            .iter()
            .map(|p| match p.at {
                Some(at) => format!("{}:{}:{}", p.file, at.line, at.column),
                None => p.file.clone(),
            })
            .collect();
        assert_eq!(
            places,
            [
                "lib/wave.wgsl:3:30",
                "pack.toml:7:1",
                "present.wgsl:1:1",
                "present.wgsl:5:1",
                "stray.wgsl"
            ],
            "{report}"
        );
        let messages: Vec<&str> = report.problems.iter().map(|p| p.message.as_str()).collect();
        assert!(messages[0].contains("not_declared"), "{report}");
        assert_eq!(
            messages[1],
            "[requires] says `outputs = 2` but no pass writes more than 1"
        );
        assert_eq!(
            messages[2],
            "the option `strength` is a float, so its override is `f32`; this one is `i32`"
        );
        assert_eq!(
            messages[3],
            "`fn present` writes one target, `screen`, and must return `vec4<f32>`; it returns `vec3<f32>`"
        );
        assert_eq!(messages[4], "nothing imports or names this file");
        assert_eq!(report.skipped.len(), 4, "{report}");
        assert!(
            report
                .to_string()
                .starts_with(&format!("{}: 5 problems\n", report.pack)),
            "{report}"
        );
        assert_eq!(report.json()["problems"][0]["line"], serde_json::json!(3));
    }

    #[test]
    fn a_missing_pack_is_one_problem_and_the_rest_is_skipped() {
        let report = check(&example("no_such_pack"));
        assert_eq!(report.problems.len(), 1, "{report}");
        assert_eq!(report.skipped.len(), 1, "{report}");
        assert!(report.modules.is_empty() && report.tiers.is_empty());
    }
}
