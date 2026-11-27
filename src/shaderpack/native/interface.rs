use super::api::{
    self, ModuleKind, OutputSize, Owner, PackInterface, PreludeOption, PreludeTarget, ScreenOutput,
    TerrainTier,
};
use super::graph;
use super::manifest::{self, Anchor, BlockEntry, Layout, Manifest, Named, Pass, PassKind, Size};
use super::problem::Problem;
use crate::shaderpack::blockids;

pub(crate) fn pack_interface(
    manifest: &Manifest,
    layout: Layout<'_>,
    chosen: &[usize],
) -> PackInterface {
    PackInterface {
        api: manifest.api,
        targets: layout
            .targets
            .iter()
            .enumerate()
            .filter_map(|(i, target)| {
                let chosen = chosen.get(i).copied().unwrap_or(0);
                PreludeTarget::new(
                    target.id.clone(),
                    target.formats.clone(),
                    chosen,
                    target.history,
                )
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
    let pack = pack_interface(manifest, layout, &[]);
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
    pub(crate) roots: Vec<String>,
    pub(crate) kind: ModuleKind,
    pub(crate) node: graph::Module,
    pub(crate) role: String,
}

pub(crate) fn modules(
    layout: Layout<'_>,
    pack: &PackInterface,
    terrain: TerrainTier,
) -> (Vec<ModuleSpec>, Vec<Problem>) {
    let mut specs = Vec::new();
    let mut problems = Vec::new();
    for (group, table) in layout.geometry.groups() {
        specs.push(ModuleSpec {
            file: table.file.path.clone(),
            roots: vec![table.file.path.clone()],
            kind: ModuleKind::Geometry {
                group,
                writes: target_indices(pack, &table.writes),
                stages: table.stages.clone(),
                vertex: table.vertex,
                terrain,
            },
            node: graph::Module::Geometry(group),
            role: format!("[geometry.{}]", group.name()),
        });
    }
    if let Some(shadow) = layout.shadow {
        let opaque = layout.geometry.opaque.as_ref().filter(|table| table.vertex);
        let file = shadow.file.as_ref().filter(|_| shadow.color);
        let roots: Vec<String> = opaque
            .map(|table| &table.file)
            .into_iter()
            .chain(file)
            .map(|f| f.path.clone())
            .collect();
        specs.push(ModuleSpec {
            file: roots
                .last()
                .cloned()
                .unwrap_or_else(|| manifest::FILE.to_owned()),
            kind: ModuleKind::Shadow {
                color: file.is_some(),
                displace: opaque.is_some(),
                terrain,
            },
            roots,
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
            roots: vec![pass.file.path.clone()],
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

#[derive(Clone, Debug, Default, PartialEq)]
#[allow(
    dead_code,
    reason = "the renderer's geometry draw builds the mesher's block ids from it"
)]
pub(crate) struct MaterialBlocks {
    pub(crate) entries: Vec<blockids::Entry>,
    pub(crate) tags: Vec<(u16, String)>,
}

pub(crate) fn material_blocks(manifest: &Manifest) -> MaterialBlocks {
    let mut out = MaterialBlocks::default();
    for (index, material) in manifest.materials.iter().enumerate() {
        let Some(id) = u16::try_from(index + 1).ok().filter(|&id| id != u16::MAX) else {
            break;
        };
        for entry in &material.blocks {
            match entry {
                BlockEntry::Block(name) => {
                    if let Some(block) = vanilla_path(name) {
                        out.entries.push(blockids::Entry {
                            id,
                            block,
                            properties: Vec::new(),
                        });
                    }
                }
                BlockEntry::State {
                    block,
                    property,
                    value,
                } => {
                    if let Some(block) = vanilla_path(block) {
                        let properties = vec![(property.clone(), vec![value.clone()])];
                        out.entries.push(blockids::Entry {
                            id,
                            block,
                            properties,
                        });
                    }
                }
                BlockEntry::Tag(tag) => out.tags.push((id, tag.clone())),
            }
        }
    }
    out
}

fn vanilla_path(name: &str) -> Option<String> {
    match name.split_once(':') {
        None => Some(name.to_owned()),
        Some(("minecraft", path)) => Some(path.to_owned()),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::manifest::tests::{EXAMPLE, parse_ok};
    use super::*;

    #[test]
    fn materials_number_their_blocks_from_one() {
        let text = format!(
            "{EXAMPLE}\n[[material]]\nid = \"glow\"\nblocks = [\"minecraft:glowstone\", \"mod:lamp\"]\n\n\
             [[material]]\nid = \"tops\"\nblocks = [\"#leaves\", \"tall_grass:half=upper\"]\n"
        );
        let manifest = parse_ok(&text);
        let found = material_blocks(&manifest);
        let first = manifest.materials.len() - 1;
        let number = |index: usize| (index + 1) as u16;
        let glow = found
            .entries
            .iter()
            .find(|e| e.block == "glowstone")
            .expect("glowstone");
        assert_eq!((glow.id, glow.properties.len()), (number(first - 1), 0));
        assert!(
            !found.entries.iter().any(|e| e.block.contains("lamp")),
            "{found:?}"
        );
        let grass = found
            .entries
            .iter()
            .find(|e| e.block == "tall_grass" && e.id == number(first))
            .expect("tall_grass");
        assert_eq!(
            grass.properties,
            [("half".to_owned(), vec!["upper".to_owned()])]
        );
        assert!(
            found.tags.contains(&(number(first), "leaves".to_owned())),
            "{found:?}"
        );
        assert!(
            found
                .entries
                .windows(2)
                .all(|pair| pair[0].id <= pair[1].id)
        );
    }
}
