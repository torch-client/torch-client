use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use super::api::{self, Binding, BindingKind, ModuleKind, PackInterface, Resource, TerrainTier};
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
    pub(crate) needs: Needs,
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
    check_and_open(path).0
}

pub(crate) fn check_and_open(path: &Path) -> (Report, Option<Package>) {
    let mut report = Report {
        pack: path.display().to_string(),
        problems: Vec::new(),
        skipped: Vec::new(),
        tiers: Vec::new(),
        needs: Needs::default(),
        modules: Vec::new(),
    };
    let opened = package::open(path);
    report.add(opened.problems);
    match &opened.package {
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
        Some(package) => check_package(package, &mut report),
    }
    report
        .problems
        .sort_by(|a, b| (a.file.as_str(), a.at).cmp(&(b.file.as_str(), b.at)));
    (report, opened.package)
}

pub(super) fn naga_tier(needs: &Needs) -> TierName {
    caps::tiers_for(needs)
        .first()
        .copied()
        .unwrap_or(TierName::Desktop)
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
    let tier = naga_tier(&needs);
    let read = |path: &str| package.wgsl(path);

    let mut checked: Vec<Checked> = Vec::new();
    let mut composed_all = true;
    let mut layouts: Vec<(Layout<'_>, Option<Reads>)> = Vec::new();
    for layout in manifest.layouts() {
        let layout_name = layout.dimension.unwrap_or(TOP_LEVEL).to_owned();
        let pack = interface::pack_interface(manifest, layout, &[]);
        let (specs, problems) = interface::modules(layout, &pack, TerrainTier::Indirect);
        report.add(problems);
        let mut reads = Some(Reads::new());
        for spec in specs {
            let composed = match compose::compose_files(&pack, &spec.roots, &spec.kind, &read) {
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

        let (specs, _) = interface::modules(layout, &pack, TerrainTier::Direct);
        for spec in specs
            .into_iter()
            .filter(|spec| spec.kind.terrain().is_some())
        {
            match compose::compose_files(&pack, &spec.roots, &spec.kind, &read) {
                Ok(composed) => {
                    check_module(&pack, &spec, &composed, tier, report);
                }
                Err(problems) => report.add(problems),
            }
        }

        for chosen in last_entries(layout) {
            let pack = interface::pack_interface(manifest, layout, &chosen);
            let (specs, _) = interface::modules(layout, &pack, TerrainTier::Indirect);
            for spec in specs {
                match compose::compose_files(&pack, &spec.roots, &spec.kind, &read) {
                    Ok(composed) => {
                        check_module(&pack, &spec, &composed, tier, report);
                    }
                    Err(problems) => report.add(problems),
                }
            }
        }
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
    report.needs = needs;
    report.modules = checked.into_iter().map(|c| c.report).collect();
}

fn last_entries(layout: Layout<'_>) -> Vec<Vec<usize>> {
    layout
        .targets
        .iter()
        .enumerate()
        .filter(|(_, target)| target.formats.len() > 1)
        .map(|(i, target)| {
            let mut chosen = vec![0; layout.targets.len()];
            chosen[i] = target.formats.len() - 1;
            chosen
        })
        .collect()
}

fn unused_options(manifest: &Manifest, checked: &[Checked]) -> Vec<Problem> {
    let mut used: BTreeSet<&str> = manifest.manifest_options();
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

pub(crate) fn describe(needs: &Needs) -> String {
    let formats: Vec<String> = needs
        .format_lists
        .iter()
        .filter(|list| !list.iter().any(|f| f.info().baseline))
        .map(|list| {
            list.iter()
                .map(|f| f.name())
                .collect::<Vec<_>>()
                .join(" or ")
        })
        .collect();
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
pub(super) struct Analysis {
    pub(super) problems: Vec<Problem>,
    pub(super) skipped: Option<&'static str>,
    pub(super) options: Option<Vec<usize>>,
    pub(super) used: Option<Vec<Binding>>,
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

pub(super) fn analyse(
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
    if matches!(kind, ModuleKind::Shadow { .. }) {
        for binding in used
            .iter()
            .filter(|b| matches!(b.resource, Resource::ShadowDepth | Resource::ShadowColor))
        {
            out.problems.push(
                Problem::file(composed.file(), format!("the shadow pass reads `{}`, which it draws into", binding.name))
                    .hint("call shadow_light and shadow_cascade from a geometry stage or a pass, not from `shadow` or `displace`"),
            );
        }
    }
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
                "[shadow]",
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
        assert!(report.modules.iter().all(|m| m.valid), "{report}");
        assert_eq!(
            report.modules[0].entry_points,
            [
                "torch_sky_vertex",
                "torch_terrain_vertex",
                "torch_entity_vertex",
                "torch_hand_vertex",
                "torch_sky_fragment",
                "torch_terrain_fragment",
                "torch_terrain_cutout_fragment",
                "torch_entity_fragment",
                "torch_entity_cutout_fragment",
                "torch_block_entity_fragment",
                "torch_block_entity_cutout_fragment",
                "torch_hand_fragment",
                "torch_hand_cutout_fragment",
            ]
        );
        assert_eq!(
            report.modules[1].entry_points,
            [
                "torch_terrain_vertex",
                "torch_entity_vertex",
                "torch_cloud_vertex",
                "torch_weather_vertex",
                "torch_water_fragment",
                "torch_particle_fragment",
                "torch_particle_cutout_fragment",
                "torch_clouds_fragment",
                "torch_weather_fragment",
            ]
        );
        assert_eq!(
            report.modules[2].entry_points,
            [
                api::SHADOW_TERRAIN_VERTEX,
                api::SHADOW_ENTITY_VERTEX,
                api::SHADOW_TERRAIN_CUTOUT,
                api::SHADOW_ENTITY_CUTOUT,
            ]
        );
        assert_eq!(report.modules[2].file, manifest::FILE);
        let geometry = [
            "frame",
            "linear_clamp",
            "nearest_clamp",
            "block_atlas",
            "lightmap",
            "entity_texture",
        ];
        assert_eq!(names(&report.modules[0]), geometry);
        assert_eq!(names(&report.modules[1]), geometry);
        assert_eq!(
            names(&report.modules[2]),
            ["frame", "nearest_clamp", "block_atlas", "entity_texture"]
        );
        assert_eq!(
            names(&report.modules[3]),
            [
                "frame",
                "shadow_compare",
                "scene_depth",
                "shadow_depth",
                "albedo",
                "normal",
                "lit_prev"
            ]
        );
        assert_eq!(names(&report.modules[4]), ["frame", "lit"]);
    }

    #[test]
    fn the_deferred_geometry_validates_in_both_tiers() {
        let opened = package::open(&example("deferred"));
        let package = opened.package.expect("the example opens");
        let manifest = &package.manifest;
        let layout = manifest.layouts().next().expect("the top level");
        let pack = interface::pack_interface(manifest, layout, &[]);
        let read = |path: &str| package.wgsl(path);
        for terrain in TerrainTier::ALL {
            let (specs, problems) = interface::modules(layout, &pack, terrain);
            assert!(problems.is_empty(), "{problems:#?}");
            let geometry: Vec<ModuleSpec> = specs
                .into_iter()
                .filter(|s| s.kind.terrain() == Some(terrain))
                .collect();
            let roles: Vec<&str> = geometry.iter().map(|s| s.role.as_str()).collect();
            assert_eq!(
                roles,
                ["[geometry.opaque]", "[geometry.translucent]", "[shadow]"]
            );
            for spec in geometry {
                let composed = compose::compose_files(&pack, &spec.roots, &spec.kind, &read)
                    .unwrap_or_else(|p| panic!("{p:#?}"));
                assert!(!composed.entry_points().is_empty(), "{}", spec.role);
                let tier = if terrain == TerrainTier::Direct {
                    TierName::WebGl2
                } else {
                    TierName::Desktop
                };
                let analysis = analyse(&pack, &spec.kind, &composed, tier);
                assert!(
                    analysis.problems.is_empty(),
                    "{} {terrain:?}: {:#?}",
                    spec.role,
                    analysis.problems
                );
                assert!(
                    analysis.used.is_some(),
                    "{} {terrain:?} did not validate",
                    spec.role
                );
                let direct = !composed.text().contains("var<storage");
                assert_eq!(direct, terrain == TerrainTier::Direct);
            }
        }
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
    fn a_format_list_is_checked_at_its_last_entry() {
        let dir = std::env::temp_dir().join(format!("native_format_branch_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = "format = 1\napi = \"1.0\"\nid = \"branch\"\nname = \"Branch\"\nversion = \"1\"\n\n\
                        [[target]]\nid = \"lit\"\nformat = [\"rgba16float\", \"rgba8unorm\"]\n\n\
                        [[pass]]\nid = \"fill\"\nat = \"composite\"\nfile = \"fill.wgsl\"\nwrites = [\"lit\"]\n\n\
                        [[pass]]\nid = \"show\"\nat = \"final\"\nfile = \"show.wgsl\"\n";
        let fill = "const SCALE: u32 = 1u / (1u - FORMAT_LIT);\n\
                    fn fill(in: ScreenInput) -> vec4<f32> {\n    return vec4<f32>(f32(SCALE));\n}\n";
        let show = "fn show(in: ScreenInput) -> vec4<f32> {\n    return textureSampleLevel(lit, linear_clamp, in.uv, 0.0);\n}\n";
        for (name, text) in [
            ("pack.toml", manifest),
            ("fill.wgsl", fill),
            ("show.wgsl", show),
        ] {
            std::fs::write(dir.join(name), text).unwrap();
        }
        let report = check(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(report.problems.len(), 1, "{report}");
        assert_eq!(report.problems[0].file, "fill.wgsl", "{report}");
        assert_eq!(
            roles(&report),
            ["composite pass `fill`", "final pass `show`"]
        );
        assert!(
            report
                .modules
                .iter()
                .all(|m| m.text.contains("const FORMAT_LIT: u32 = 0u;")),
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
    }

    #[test]
    fn a_missing_pack_is_one_problem_and_the_rest_is_skipped() {
        let report = check(&example("no_such_pack"));
        assert_eq!(report.problems.len(), 1, "{report}");
        assert_eq!(report.skipped.len(), 1, "{report}");
        assert!(report.modules.is_empty() && report.tiers.is_empty());
    }
}
