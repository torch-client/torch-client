use super::api::{
    self, ModuleKind, OutputSize, Owner, PackInterface, PreludeOption, PreludeTarget, ScreenOutput,
};
use super::graph;
use super::manifest::{self, Anchor, Layout, Manifest, Named, Pass, PassKind, Size};
use super::problem::Problem;

pub(crate) fn pack_interface(manifest: &Manifest, layout: Layout<'_>) -> PackInterface {
    PackInterface {
        api: manifest.api,
        targets: layout
            .targets
            .iter()
            .filter_map(|target| {
                PreludeTarget::new(target.id.clone(), target.format, target.history)
            })
            .collect(),
        textures: manifest
            .textures
            .iter()
            .map(|texture| texture.id.clone())
            .collect(),
        options: manifest
            .options
            .iter()
            .map(|decl| PreludeOption {
                id: decl.id.clone(),
                kind: decl.kind.clone(),
            })
            .collect(),
        materials: manifest
            .materials
            .iter()
            .map(|material| material.id.clone())
            .collect(),
        shadow_color: layout.shadow.is_some_and(|shadow| shadow.color),
        post_only: layout.geometry.opaque.is_none() && layout.geometry.translucent.is_none(),
    }
}

pub(crate) fn name_problems(manifest: &Manifest, layout: Layout<'_>) -> Vec<Problem> {
    let pack = pack_interface(manifest, layout);
    api::check_interface(&pack)
        .into_iter()
        .map(|problem| {
            let at = problem.owner.and_then(|owner| match owner {
                Owner::Target(index) => layout
                    .targets
                    .iter()
                    .find(|t| t.id == pack.targets[index].id)
                    .map(|t| t.at),
                Owner::Texture(index) => manifest.textures.get(index).map(|t| t.at),
                Owner::Option(index) => manifest.options.iter().nth(index).map(|decl| decl.at),
                Owner::Material(index) => manifest.materials.get(index).map(|m| m.at),
            });
            Problem {
                file: manifest::FILE.to_owned(),
                at,
                message: problem.message,
                hint: None,
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct ModuleSpec {
    pub(crate) file: String,
    pub(crate) kind: ModuleKind,
    pub(crate) node: graph::Module,
    pub(crate) role: String,
}

pub(crate) fn modules(layout: Layout<'_>, pack: &PackInterface) -> (Vec<ModuleSpec>, Vec<Problem>) {
    let mut specs = Vec::new();
    let mut problems = Vec::new();
    for (group, table) in layout.geometry.groups() {
        specs.push(ModuleSpec {
            file: table.file.path.clone(),
            kind: ModuleKind::Geometry {
                group,
                writes: target_indices(pack, &table.writes),
                stages: table.stages.clone(),
                vertex: table.vertex,
            },
            node: graph::Module::Geometry(group),
            role: format!("[geometry.{}]", group.name()),
        });
    }
    if let Some(shadow) = layout.shadow
        && shadow.color
        && let Some(file) = &shadow.file
    {
        specs.push(ModuleSpec {
            file: file.path.clone(),
            kind: ModuleKind::Shadow,
            node: graph::Module::Shadow,
            role: "[shadow]".to_owned(),
        });
    }
    let screen_writer = layout
        .passes
        .iter()
        .rposition(|pass| pass.anchor == Anchor::Final);
    for (index, pass) in layout.passes.iter().enumerate() {
        let function = pass.id.clone();
        let kind = match &pass.kind {
            PassKind::Compute { workgroup, .. } => ModuleKind::Compute {
                function,
                writes: target_indices(pack, &pass.writes),
                workgroup: *workgroup,
            },
            PassKind::Screen if Some(index) == screen_writer => ModuleKind::Screen {
                function,
                output: ScreenOutput::Screen,
                size: OutputSize::Scale(1.0),
            },
            PassKind::Screen => ModuleKind::Screen {
                function,
                output: ScreenOutput::Targets(target_indices(pack, &pass.writes)),
                size: output_size(layout, pass, &mut problems),
            },
        };
        specs.push(ModuleSpec {
            file: pass.file.path.clone(),
            kind,
            node: graph::Module::Pass(pass.id.clone()),
            role: format!("{} pass `{}`", pass.anchor.name(), pass.id),
        });
    }
    (specs, problems)
}

fn target_indices(pack: &PackInterface, writes: &[Named]) -> Vec<usize> {
    writes
        .iter()
        .filter_map(|write| pack.targets.iter().position(|t| t.id == write.id))
        .collect()
}

fn output_size(layout: Layout<'_>, pass: &Pass, problems: &mut Vec<Problem>) -> OutputSize {
    let sizes: Vec<Size> = pass
        .writes
        .iter()
        .filter_map(|write| layout.targets.iter().find(|t| t.id == write.id))
        .map(|target| target.size)
        .collect();
    if sizes.windows(2).any(|pair| pair[0] != pair[1]) {
        let mut problem = Problem::file(
            manifest::FILE,
            format!("pass `{}` writes targets of different sizes", pass.id),
        )
        .hint("one pass draws at one size: give its targets the same `scale` or `size`");
        problem.at = Some(pass.at);
        problems.push(problem);
    }
    match sizes.first().copied() {
        Some(Size::Scale(scale)) => OutputSize::Scale(1.0 / scale.divisor() as f32),
        Some(Size::Fixed { width, height }) => OutputSize::Fixed([width, height]),
        None => OutputSize::Scale(1.0),
    }
}
