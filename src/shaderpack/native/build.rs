use std::collections::{HashMap, HashSet};

use super::api::{
    self, Binding, Draws, Group, ModuleKind, OutputSize, PackInterface, Resource, ScreenOutput,
    Stage, TerrainTier,
};
use super::caps::Format;
use super::check;
use super::compose;
use super::graph::{self, Reads};
use super::interface::{self, MaterialBlocks};
use super::manifest::{self, Anchor, Layout, Manifest, PassKind, Size};
use super::options::Values;
use super::package::Package;
use super::problem::Problem;

#[derive(Clone, Debug)]
pub(crate) struct Built {
    pub(crate) id: String,
    pub(crate) targets: Vec<BuiltTarget>,
    pub(crate) passes: Vec<BuiltPass>,
    #[allow(dead_code, reason = "the renderer's geometry draw reads it")]
    pub(crate) geometry: Vec<BuiltGroup>,
    #[allow(dead_code, reason = "the renderer's geometry draw reads it")]
    pub(crate) terrain: TerrainTier,
    #[allow(dead_code, reason = "the renderer's geometry draw reads it")]
    pub(crate) materials: MaterialBlocks,
    #[allow(dead_code, reason = "the renderer's geometry draw reads it")]
    pub(crate) sky_parts: Vec<api::SkyPart>,
    #[allow(dead_code, reason = "the renderer's shadow pass reads it")]
    pub(crate) shadow: Option<BuiltShadow>,
}

#[derive(Clone, Debug)]
#[allow(dead_code, reason = "the renderer's shadow pass reads it")]
pub(crate) struct BuiltShadow {
    pub(crate) resolution: u32,
    pub(crate) distance: f64,
    pub(crate) color: Option<Format>,
    pub(crate) file: String,
    pub(crate) wgsl: String,
    pub(crate) bindings: Vec<Binding>,
    pub(crate) terrain: BuiltCaster,
    pub(crate) entities: BuiltCaster,
    pub(crate) overrides: Vec<(String, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code, reason = "the renderer's shadow pass reads it")]
pub(crate) struct BuiltCaster {
    pub(crate) draws: Draws,
    pub(crate) vertex: String,
    pub(crate) fragment: Option<String>,
    pub(crate) cutout: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code, reason = "the renderer's geometry draw reads it")]
pub(crate) struct BuiltGroup {
    pub(crate) group: Group,
    pub(crate) file: String,
    pub(crate) wgsl: String,
    pub(crate) bindings: Vec<Binding>,
    pub(crate) writes: Vec<usize>,
    pub(crate) stages: Vec<BuiltStage>,
    pub(crate) overrides: Vec<(String, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code, reason = "the renderer's geometry draw reads it")]
pub(crate) struct BuiltStage {
    pub(crate) stage: Stage,
    pub(crate) draws: Draws,
    pub(crate) vertex: String,
    pub(crate) fragment: String,
    pub(crate) cutout: Option<String>,
    pub(crate) defined: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BuiltTarget {
    pub(crate) id: String,
    pub(crate) format: Format,
    pub(crate) size: OutputSize,
    pub(crate) history: bool,
    pub(crate) flip: bool,
    pub(crate) clear: Option<[f64; 4]>,
}

impl BuiltTarget {
    pub(crate) fn double(&self) -> bool {
        self.flip || self.history
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BuiltPass {
    pub(crate) id: String,
    pub(crate) anchor: Anchor,
    pub(crate) file: String,
    pub(crate) wgsl: String,
    pub(crate) bindings: Vec<Binding>,
    pub(crate) output: ScreenOutput,
    pub(crate) overrides: Vec<(String, f64)>,
}

impl BuiltPass {
    pub(crate) fn binds(&self, resource: Resource) -> bool {
        self.bindings.iter().any(|b| b.resource == resource)
    }
}

fn unsupported(manifest: &Manifest) -> Vec<&'static str> {
    let mut found = Vec::new();
    let layouts: Vec<Layout<'_>> = manifest.layouts().collect();
    let mut note = |has: bool, what: &'static str| {
        if has && !found.contains(&what) {
            found.push(what);
        }
    };
    for layout in &layouts {
        note(
            layout
                .passes
                .iter()
                .any(|p| matches!(p.kind, PassKind::Compute { .. })),
            "compute passes",
        );
        note(layout.targets.iter().any(|t| t.mips), "mip chains");
    }
    note(!manifest.textures.is_empty(), "[[texture]]s");
    found
}

pub(crate) fn build(
    package: &Package,
    values: &Values,
    dimension: Option<&str>,
    choose: &dyn Fn(&[Format]) -> Option<usize>,
    terrain: TerrainTier,
) -> Result<Built, Vec<Problem>> {
    let manifest = &package.manifest;
    let missing = unsupported(manifest);
    if !missing.is_empty() {
        return Err(vec![
            Problem::package(format!(
                "this build draws geometry, the shadow pass and screen passes only; this pack has {}",
                missing.join(", ")
            ))
            .hint("compute, mips and custom textures arrive in later phases of docs/SHADER_FORMAT.md"),
        ]);
    }
    let layout = match dimension {
        Some(dimension) => manifest.layout(dimension),
        None => manifest
            .layouts()
            .next()
            .expect("the top-level layout is always first"),
    };
    let chosen = choose_formats(layout, choose)?;
    let pack = interface::pack_interface(manifest, layout, &chosen);
    let (specs, problems) = interface::modules(layout, &pack, terrain);
    if !problems.is_empty() {
        return Err(problems);
    }

    let tier = check::naga_tier(&manifest.needs());
    let read = |path: &str| package.wgsl(path);
    let mut problems = Vec::new();
    let mut reads = Reads::new();
    let mut modules: HashMap<graph::Module, Analysed> = HashMap::new();
    for spec in specs {
        let composed = match compose::compose_files(&pack, &spec.roots, &spec.kind, &read) {
            Ok(composed) => composed,
            Err(found) => {
                problems.extend(found);
                continue;
            }
        };
        let analysis = check::analyse(&pack, &spec.kind, &composed, tier);
        let before = problems.len();
        problems.extend(analysis.problems);
        let (Some(used), Some(options)) = (analysis.used, analysis.options) else {
            if problems.len() == before {
                problems.push(Problem::file(
                    &spec.file,
                    format!("{} did not validate", spec.role),
                ));
            }
            continue;
        };
        reads.insert(
            spec.node.clone(),
            used.iter().map(|b| b.name.clone()).collect(),
        );
        modules.insert(
            spec.node,
            Analysed {
                file: spec.file,
                kind: spec.kind,
                wgsl: composed.text().to_owned(),
                used,
                options,
            },
        );
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let (plan, problems) = graph::plan(layout, values, &reads);
    if !problems.is_empty() {
        return Err(problems);
    }

    let targets: Vec<BuiltTarget> = plan
        .targets
        .iter()
        .map(|planned| {
            let target = &layout.targets[planned.target];
            BuiltTarget {
                id: target.id.clone(),
                format: target.formats[chosen[planned.target]],
                size: match target.size {
                    Size::Scale(scale) => OutputSize::Scale(1.0 / scale.divisor() as f32),
                    Size::Fixed { width, height } => OutputSize::Fixed([width, height]),
                },
                history: planned.history,
                flip: planned.flip,
                clear: target.clear,
            }
        })
        .collect();
    let index: HashMap<&str, usize> = targets
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    let overrides = manifest.options.overrides(values);
    let module_overrides = |module: &Analysed| -> Vec<(String, f64)> {
        module
            .options
            .iter()
            .filter_map(|&option| overrides.get(option))
            .map(|(id, value)| ((*id).to_owned(), *value))
            .collect()
    };
    let mut retargeted = Retargeted {
        pack: &pack,
        index: &index,
        problems: Vec::new(),
    };

    let mut geometry = Vec::new();
    for planned in [&plan.opaque, &plan.translucent].into_iter().flatten() {
        let what = format!("[geometry.{}]", planned.kind.name());
        let Some(module) = modules.get(&graph::Module::Geometry(planned.kind)) else {
            retargeted
                .problems
                .push(Problem::package(format!("{what} has no module")));
            continue;
        };
        let ModuleKind::Geometry {
            group,
            writes,
            stages,
            ..
        } = &module.kind
        else {
            retargeted.problems.push(Problem::file(
                &module.file,
                format!("{what} is not a geometry module"),
            ));
            continue;
        };
        let stages = group
            .stages()
            .map(|stage| BuiltStage {
                stage,
                draws: stage.draw().draws,
                vertex: api::stage_vertex_entry(stage),
                fragment: api::fragment_entry(stage),
                cutout: stage.draw().cutout.then(|| api::cutout_entry(stage)),
                defined: stages.contains(&stage),
            })
            .collect();
        geometry.push(BuiltGroup {
            group: *group,
            file: module.file.clone(),
            wgsl: module.wgsl.clone(),
            bindings: retargeted.bindings(module, &what),
            writes: retargeted.writes(module, &what, writes),
            stages,
            overrides: module_overrides(module),
        });
    }

    let shadow = match (&plan.shadow, layout.shadow) {
        (Some(_), Some(table)) => built_shadow(
            table,
            values,
            modules.get(&graph::Module::Shadow),
            &mut retargeted,
            &module_overrides,
        ),
        _ => None,
    };

    let mut passes = Vec::with_capacity(plan.passes.len());
    for planned in &plan.passes {
        let pass = &layout.passes[planned.pass];
        let Some(module) = modules.get(&graph::Module::Pass(pass.id.clone())) else {
            retargeted.problems.push(Problem::file(
                &pass.file.path,
                format!("pass `{}` has no module", pass.id),
            ));
            continue;
        };
        let ModuleKind::Screen { output, .. } = &module.kind else {
            retargeted.problems.push(Problem::file(
                &module.file,
                format!("pass `{}` is not a screen pass", pass.id),
            ));
            continue;
        };
        let what = format!("pass `{}`", pass.id);
        let bindings = retargeted.bindings(module, &what);
        let output = match output {
            ScreenOutput::Screen => ScreenOutput::Screen,
            ScreenOutput::Targets(writes) => {
                ScreenOutput::Targets(retargeted.writes(module, &what, writes))
            }
        };
        passes.push(BuiltPass {
            id: pass.id.clone(),
            anchor: pass.anchor,
            file: module.file.clone(),
            wgsl: module.wgsl.clone(),
            bindings,
            output,
            overrides: module_overrides(module),
        });
    }
    let mut problems = retargeted.problems;
    if !passes.iter().any(|p| p.output == ScreenOutput::Screen) {
        problems.push(
            Problem::package("no pass writes the screen").hint("the last `final` pass writes it"),
        );
    }
    problems.extend(history_conflicts(&targets, &passes));
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Built {
        id: manifest.id.clone(),
        targets,
        passes,
        geometry,
        terrain,
        materials: interface::material_blocks(manifest),
        sky_parts: layout
            .geometry
            .opaque
            .as_ref()
            .map_or_else(Vec::new, |group| group.sky_parts.clone()),
        shadow,
    })
}

fn built_shadow(
    table: &manifest::Shadow,
    values: &Values,
    module: Option<&Analysed>,
    retargeted: &mut Retargeted<'_>,
    overrides: &dyn Fn(&Analysed) -> Vec<(String, f64)>,
) -> Option<BuiltShadow> {
    let what = "[shadow]";
    let Some(module) = module else {
        retargeted
            .problems
            .push(Problem::package(format!("{what} has no module")));
        return None;
    };
    let ModuleKind::Shadow { color, .. } = module.kind else {
        retargeted.problems.push(Problem::file(
            &module.file,
            format!("{what} is not a shadow module"),
        ));
        return None;
    };
    let lit = |entry: &str| color.then(|| entry.to_owned());
    Some(BuiltShadow {
        resolution: table.resolution(values),
        distance: table.distance(values),
        color: color.then_some(api::SHADOW_COLOR_FORMAT),
        file: module.file.clone(),
        wgsl: module.wgsl.clone(),
        bindings: retargeted.bindings(module, what),
        terrain: BuiltCaster {
            draws: api::SHADOW_TERRAIN_DRAWS,
            vertex: api::SHADOW_TERRAIN_VERTEX.to_owned(),
            fragment: lit(api::SHADOW_TERRAIN_FRAGMENT),
            cutout: api::SHADOW_TERRAIN_CUTOUT.to_owned(),
        },
        entities: BuiltCaster {
            draws: api::SHADOW_ENTITY_DRAWS,
            vertex: api::SHADOW_ENTITY_VERTEX.to_owned(),
            fragment: lit(api::SHADOW_ENTITY_FRAGMENT),
            cutout: api::SHADOW_ENTITY_CUTOUT.to_owned(),
        },
        overrides: overrides(module),
    })
}

struct Retargeted<'a> {
    pack: &'a PackInterface,
    index: &'a HashMap<&'a str, usize>,
    problems: Vec<Problem>,
}

impl Retargeted<'_> {
    fn lost(&mut self, module: &Analysed, what: &str, name: &str) {
        self.problems.push(Problem::file(
            &module.file,
            format!("{what} uses `{name}`, which does not exist"),
        ));
    }

    fn bindings(&mut self, module: &Analysed, what: &str) -> Vec<Binding> {
        let mut bindings = Vec::with_capacity(module.used.len());
        for binding in &module.used {
            match retarget(binding, self.pack, self.index) {
                Some(binding) => bindings.push(binding),
                None => self.lost(module, what, &binding.name),
            }
        }
        bindings
    }

    fn writes(&mut self, module: &Analysed, what: &str, writes: &[usize]) -> Vec<usize> {
        let mut mapped = Vec::with_capacity(writes.len());
        for &write in writes {
            match self
                .pack
                .targets
                .get(write)
                .and_then(|t| self.index.get(t.id.as_str()))
            {
                Some(&t) => mapped.push(t),
                None => {
                    let name = self
                        .pack
                        .targets
                        .get(write)
                        .map_or("?", |t| t.id.as_str())
                        .to_owned();
                    self.lost(module, what, &name);
                }
            }
        }
        mapped
    }
}

fn choose_formats(
    layout: Layout<'_>,
    choose: &dyn Fn(&[Format]) -> Option<usize>,
) -> Result<Vec<usize>, Vec<Problem>> {
    let mut chosen = Vec::with_capacity(layout.targets.len());
    let mut problems = Vec::new();
    for target in layout.targets {
        match choose(target.formats.as_slice()).filter(|&index| index < target.formats.len()) {
            Some(index) => chosen.push(index),
            None => {
                let tried: Vec<&str> = target.formats.iter().map(|f| f.name()).collect();
                let mut problem = Problem::file(
                    manifest::FILE,
                    format!(
                        "this device renders to none of target `{}`'s formats ({})",
                        target.id,
                        tried.join(", ")
                    ),
                );
                problem.at = Some(target.at);
                problems.push(
                    problem.hint("end the list with rgba8unorm, which every device renders to"),
                );
            }
        }
    }
    if problems.is_empty() {
        Ok(chosen)
    } else {
        Err(problems)
    }
}

struct Analysed {
    file: String,
    kind: ModuleKind,
    wgsl: String,
    used: Vec<Binding>,
    options: Vec<usize>,
}

fn retarget(
    binding: &Binding,
    pack: &PackInterface,
    index: &HashMap<&str, usize>,
) -> Option<Binding> {
    let mut out = binding.clone();
    let lookup = |t: usize| {
        pack.targets
            .get(t)
            .and_then(|target| index.get(target.id.as_str()))
            .copied()
    };
    out.resource = match binding.resource {
        Resource::Target(t) => Resource::Target(lookup(t)?),
        Resource::TargetPrevious(t) => Resource::TargetPrevious(lookup(t)?),
        Resource::TargetOut(t) => Resource::TargetOut(lookup(t)?),
        other => other,
    };
    Some(out)
}

fn history_conflicts(targets: &[BuiltTarget], passes: &[BuiltPass]) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut written: HashSet<usize> = targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.clear.is_some())
        .map(|(i, _)| i)
        .collect();
    for pass in passes {
        let ScreenOutput::Targets(writes) = &pass.output else {
            continue;
        };
        for &t in writes {
            if targets[t].history && written.contains(&t) && pass.binds(Resource::Target(t)) {
                problems.push(Problem::file(
                    &pass.file,
                    format!(
                        "pass `{}` reads and writes history target `{}` after it was already written or cleared this frame",
                        pass.id, targets[t].id
                    ),
                ));
            }
        }
        written.extend(writes.iter().copied());
    }
    problems
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::options::Value;
    use super::super::package;
    use super::*;

    fn open(name: &str) -> Package {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("shaderpacks/native")
            .join(name);
        let opened = package::open(&path);
        assert!(opened.problems.is_empty(), "{:?}", opened.problems);
        opened.package.expect("the example opens")
    }

    fn names(pass: &BuiltPass) -> Vec<&str> {
        pass.bindings.iter().map(|b| b.name.as_str()).collect()
    }

    fn ids(built: &Built) -> Vec<&str> {
        built.passes.iter().map(|p| p.id.as_str()).collect()
    }

    fn first(_: &[Format]) -> Option<usize> {
        Some(0)
    }

    fn last(list: &[Format]) -> Option<usize> {
        list.len().checked_sub(1)
    }

    #[test]
    fn post_only_builds_in_frame_order() {
        let package = open("post_only");
        let values = Values::defaults(&package.manifest.options);
        let built = build(&package, &values, None, &first, TerrainTier::Indirect)
            .unwrap_or_else(|p| panic!("{p:#?}"));
        assert_eq!(built.id, "post_only");
        assert_eq!(ids(&built), ["extract", "tonemap"]);
        assert_eq!(
            built.targets,
            [BuiltTarget {
                id: "bright".to_owned(),
                format: Format::Rgba16Float,
                size: OutputSize::Scale(0.5),
                history: false,
                flip: false,
                clear: Some([0.0; 4]),
            }]
        );
        let (extract, tonemap) = (&built.passes[0], &built.passes[1]);
        assert_eq!(extract.anchor, Anchor::Composite);
        assert_eq!(extract.output, ScreenOutput::Targets(vec![0]));
        assert_eq!(tonemap.output, ScreenOutput::Screen);
        assert_eq!(names(extract), ["frame", "linear_clamp", "scene_color"]);
        assert_eq!(
            names(tonemap),
            ["frame", "linear_clamp", "scene_color", "bright"]
        );
        assert!(tonemap.binds(Resource::Target(0)) && tonemap.binds(Resource::SceneColor));
        assert!(!tonemap.binds(Resource::SceneDepth));
        assert!(extract.overrides.is_empty());
        assert_eq!(tonemap.overrides, [("exposure".to_owned(), 1.0)]);
        assert!(
            tonemap.wgsl.contains("fn luminance(") && tonemap.wgsl.contains("override exposure")
        );
        assert!(
            built
                .passes
                .iter()
                .all(|p| p.wgsl.contains("const FORMAT_BRIGHT: u32 = 0u;"))
        );
    }

    #[test]
    fn post_only_builds_at_the_last_format() {
        let package = open("post_only");
        let values = Values::defaults(&package.manifest.options);
        let built = build(&package, &values, None, &last, TerrainTier::Direct)
            .unwrap_or_else(|p| panic!("{p:#?}"));
        assert_eq!(built.targets[0].format, Format::Rgba8Unorm);
        assert_eq!(ids(&built), ["extract", "tonemap"]);
        assert!(
            built
                .passes
                .iter()
                .all(|p| p.wgsl.contains("const FORMAT_BRIGHT: u32 = 1u;"))
        );
    }

    #[test]
    fn a_target_the_device_cannot_render_is_refused() {
        let package = open("post_only");
        let values = Values::defaults(&package.manifest.options);
        let problems = build(
            &package,
            &values,
            None,
            &|_: &[Format]| None,
            TerrainTier::Indirect,
        )
        .expect_err("no format renders");
        assert_eq!(problems.len(), 1, "{problems:#?}");
        assert_eq!(
            problems[0].message,
            "this device renders to none of target `bright`'s formats (rgba16float, rgba8unorm)"
        );
        assert_eq!(problems[0].file, "pack.toml");
    }

    #[test]
    fn a_false_when_leaves_its_pass_out() {
        let package = open("post_only");
        let options = &package.manifest.options;
        let mut values = Values::defaults(options);
        values.set(options, "bloom", Value::Bool(false)).unwrap();
        values.set(options, "exposure", Value::Float(2.0)).unwrap();
        let built = build(&package, &values, None, &first, TerrainTier::Indirect)
            .unwrap_or_else(|p| panic!("{p:#?}"));
        assert_eq!(ids(&built), ["tonemap"]);
        assert_eq!(built.targets.len(), 1);
        assert_eq!(built.passes[0].overrides, [("exposure".to_owned(), 2.0)]);
    }

    #[test]
    fn the_deferred_example_builds_its_shadow_pass() {
        let package = open("deferred");
        let options = &package.manifest.options;
        let mut values = Values::defaults(options);
        for terrain in TerrainTier::ALL {
            let built = build(&package, &values, None, &first, terrain)
                .unwrap_or_else(|p| panic!("{p:#?}"));
            let shadow = built.shadow.as_ref().expect("[shadow] runs by default");
            assert_eq!(
                (shadow.resolution, shadow.distance, shadow.color),
                (2048, 128.0, None)
            );
            assert_eq!(shadow.file, "pack.toml");
            assert_eq!(
                shadow.terrain,
                BuiltCaster {
                    draws: api::SHADOW_TERRAIN_DRAWS,
                    vertex: "torch_shadow_terrain_vertex".to_owned(),
                    fragment: None,
                    cutout: "torch_shadow_terrain_cutout_fragment".to_owned(),
                }
            );
            assert_eq!(shadow.entities.fragment, None);
            assert_eq!(shadow.entities.vertex, "torch_shadow_entity_vertex");
            let bound: Vec<&str> = shadow.bindings.iter().map(|b| b.name.as_str()).collect();
            assert_eq!(
                bound,
                ["frame", "nearest_clamp", "block_atlas", "entity_texture"]
            );
            assert_eq!(
                shadow.wgsl.contains("var<storage"),
                terrain == TerrainTier::Indirect
            );
            let lighting = built
                .passes
                .iter()
                .find(|p| p.id == "lighting")
                .expect("lighting runs");
            assert!(
                lighting.binds(Resource::ShadowDepth) && lighting.binds(Resource::ShadowCompare)
            );
            let depth = lighting
                .bindings
                .iter()
                .find(|b| b.resource == Resource::ShadowDepth)
                .expect("bound");
            assert_eq!(depth.dimension, api::ViewDimension::D2Array);
        }
        let nether = build(
            &package,
            &values,
            Some("the_nether"),
            &first,
            TerrainTier::Direct,
        )
        .unwrap_or_else(|p| panic!("{p:#?}"));
        assert_eq!(nether.shadow.map(|s| s.resolution), Some(512));

        values.set(options, "shadows", Value::Bool(false)).unwrap();
        let built = build(&package, &values, None, &first, TerrainTier::Indirect)
            .unwrap_or_else(|p| panic!("{p:#?}"));
        assert!(built.shadow.is_none());
        let lighting = built
            .passes
            .iter()
            .find(|p| p.id == "lighting")
            .expect("lighting runs");
        assert!(lighting.binds(Resource::ShadowDepth));
    }

    fn geometry_pack() -> Package {
        geometry_pack_with("plain", "", &[])
    }

    fn geometry_pack_with(name: &str, extra: &str, files: &[(&str, &str)]) -> Package {
        let dir = std::env::temp_dir().join(format!(
            "native_geometry_build_{name}_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = "format = 1\napi = \"1.0\"\nid = \"gbuffer\"\nname = \"G-buffer\"\nversion = \"1\"\n\n\
                        [[target]]\nid = \"albedo\"\nformat = \"rgba8unorm\"\n\n\
                        [[target]]\nid = \"lit\"\nformat = \"rgba16float\"\n\n\
                        [geometry.opaque]\nfile = \"opaque.wgsl\"\nwrites = [\"albedo\"]\nstages = [\"terrain\", \"hand\"]\nvertex = true\n\n\
                        [geometry.translucent]\nfile = \"translucent.wgsl\"\nwrites = [\"lit\"]\n\n\
                        [[pass]]\nid = \"show\"\nat = \"final\"\nfile = \"show.wgsl\"\n\n\
                        [[material]]\nid = \"leaves\"\nblocks = [\"oak_leaves\", \"#leaves\"]\n";
        let opaque = "fn encode(s: Surface, at: FragmentInfo) -> vec4<f32> {\n    return vec4<f32>(s.color.rgb, f32(s.material));\n}\n\
                      fn terrain(in: TerrainInput) -> Surface {\n    var s = in.base;\n    s.color = vec4<f32>(s.color.rgb * in.color.a, s.color.a);\n    return s;\n}\n\
                      fn hand(in: EntityInput) -> Surface {\n    return in.base;\n}\n\
                      fn displace(in: VertexInput) -> vec3<f32> {\n    \
                          let sway = sin(frame.time.seconds + in.position.x) * 0.05;\n    \
                          return select(vec3<f32>(0.0), vec3<f32>(sway, 0.0, 0.0), in.material == MATERIAL_LEAVES);\n}\n";
        let translucent =
            "fn encode(s: Surface, at: FragmentInfo) -> vec4<f32> {\n    return s.color;\n}\n";
        let show = "fn show(in: ScreenInput) -> vec4<f32> {\n    \
                        return textureSampleLevel(albedo, linear_clamp, in.uv, 0.0) + textureSampleLevel(lit, linear_clamp, in.uv, 0.0);\n}\n";
        let manifest = format!("{manifest}{extra}");
        let own = [
            ("pack.toml", manifest.as_str()),
            ("opaque.wgsl", opaque),
            ("translucent.wgsl", translucent),
            ("show.wgsl", show),
        ];
        for (file, text) in own.iter().chain(files) {
            std::fs::write(dir.join(file), text).unwrap();
        }
        let opened = package::open(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(opened.problems.is_empty(), "{:?}", opened.problems);
        opened.package.expect("the pack opens")
    }

    #[test]
    fn a_geometry_pack_builds_for_each_terrain_tier() {
        let package = geometry_pack();
        let values = Values::defaults(&package.manifest.options);
        for terrain in TerrainTier::ALL {
            let built = build(&package, &values, None, &first, terrain)
                .unwrap_or_else(|p| panic!("{p:#?}"));
            assert_eq!(built.terrain, terrain);
            assert!(built.shadow.is_none());
            assert_eq!(ids(&built), ["show"]);
            let groups: Vec<Group> = built.geometry.iter().map(|g| g.group).collect();
            assert_eq!(groups, [Group::Opaque, Group::Translucent]);
            let (opaque, translucent) = (&built.geometry[0], &built.geometry[1]);
            assert_eq!(
                (opaque.writes.as_slice(), translucent.writes.as_slice()),
                (&[0][..], &[1][..])
            );
            for group in &built.geometry {
                let bound: Vec<&str> = group.bindings.iter().map(|b| b.name.as_str()).collect();
                let expected = [
                    "frame",
                    "linear_clamp",
                    "nearest_clamp",
                    "block_atlas",
                    "lightmap",
                    "entity_texture",
                ];
                assert_eq!(bound, expected, "{:?}", group.group);
                assert_eq!(
                    group.wgsl.contains("var<storage"),
                    terrain == TerrainTier::Indirect
                );
            }
            assert!(opaque.wgsl.contains("return displace(torch_in);"));
            assert!(!translucent.wgsl.contains("return displace(torch_in);"));

            let stage = |group: &BuiltGroup, stage: Stage| {
                group
                    .stages
                    .iter()
                    .find(|s| s.stage == stage)
                    .cloned()
                    .expect("listed")
            };
            let terrain_stage = stage(opaque, Stage::Terrain);
            assert_eq!(
                terrain_stage,
                BuiltStage {
                    stage: Stage::Terrain,
                    draws: Stage::Terrain.draw().draws,
                    vertex: "torch_terrain_vertex".to_owned(),
                    fragment: "torch_terrain_fragment".to_owned(),
                    cutout: Some("torch_terrain_cutout_fragment".to_owned()),
                    defined: true,
                }
            );
            let hand = stage(opaque, Stage::Hand);
            assert_eq!(
                (hand.vertex.as_str(), hand.defined),
                ("torch_hand_vertex", true)
            );
            assert!(!stage(opaque, Stage::Sky).defined && !stage(opaque, Stage::Entity).defined);
            let water = stage(translucent, Stage::Water);
            assert_eq!(
                (
                    water.vertex.as_str(),
                    water.cutout.as_deref(),
                    water.defined
                ),
                ("torch_terrain_vertex", None, false)
            );
            assert_eq!(opaque.stages.len(), 5);
            assert_eq!(translucent.stages.len(), 4);
        }
        let built = build(&package, &values, None, &first, TerrainTier::Direct)
            .unwrap_or_else(|p| panic!("{p:#?}"));
        let leaves = &built.materials;
        assert_eq!(leaves.entries.len(), 1);
        assert_eq!(
            (leaves.entries[0].id, leaves.entries[0].block.as_str()),
            (1, "oak_leaves")
        );
        assert_eq!(leaves.tags, [(1, "leaves".to_owned())]);
    }

    #[test]
    fn a_colour_shadow_pass_builds_with_displace() {
        let extra = "\n[shadow]\ncolor = true\nfile = \"shadow.wgsl\"\ndistance = 64.0\n";
        let shadow = "fn shadow(in: TerrainInput) -> vec4<f32> {\n    return vec4<f32>(in.base.color.rgb * in.block_lit, 1.0);\n}\n";
        let package = geometry_pack_with("colour_shadow", extra, &[("shadow.wgsl", shadow)]);
        let values = Values::defaults(&package.manifest.options);
        for terrain in TerrainTier::ALL {
            let built = build(&package, &values, None, &first, terrain)
                .unwrap_or_else(|p| panic!("{p:#?}"));
            let shadow = built.shadow.expect("[shadow] has no `when`");
            assert_eq!((shadow.resolution, shadow.distance), (2048, 64.0));
            assert_eq!(shadow.color, Some(api::SHADOW_COLOR_FORMAT));
            assert_eq!(shadow.file, "shadow.wgsl");
            assert_eq!(
                shadow.terrain.fragment.as_deref(),
                Some(api::SHADOW_TERRAIN_FRAGMENT)
            );
            assert_eq!(
                shadow.entities.fragment.as_deref(),
                Some(api::SHADOW_ENTITY_FRAGMENT)
            );
            assert!(
                shadow.wgsl.contains("return displace(torch_in);")
                    && shadow.wgsl.contains("fn shadow(")
            );
            let bound: Vec<&str> = shadow.bindings.iter().map(|b| b.name.as_str()).collect();
            assert_eq!(
                bound,
                [
                    "frame",
                    "linear_clamp",
                    "nearest_clamp",
                    "block_atlas",
                    "lightmap",
                    "entity_texture"
                ]
            );
        }
    }

    #[test]
    fn post_only_does_not_depend_on_the_terrain_tier() {
        let package = open("post_only");
        let values = Values::defaults(&package.manifest.options);
        let direct = build(&package, &values, None, &first, TerrainTier::Direct)
            .unwrap_or_else(|p| panic!("{p:#?}"));
        let indirect = build(&package, &values, None, &first, TerrainTier::Indirect)
            .unwrap_or_else(|p| panic!("{p:#?}"));
        assert!(direct.geometry.is_empty() && indirect.geometry.is_empty());
        let texts = |built: &Built| {
            built
                .passes
                .iter()
                .map(|p| p.wgsl.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(&direct), texts(&indirect));
    }
}
