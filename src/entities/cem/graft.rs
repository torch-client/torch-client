use crate::entities::geom::{CubeDef, Faces, Grow, LayerDef, PartDef};

use super::cube::{CemCube, Layout};
use super::jem::{BoxDef, BoxUv, Model, Part};
use super::names::{Entry, Role};

pub fn apply(
    mut layer: LayerDef,
    model: &Model,
    entry: &Entry,
    slots: usize,
    spec: &str,
) -> LayerDef {
    let sheet = [layer.tex_w as f32, layer.tex_h as f32];
    let root = layer.mesh.root();
    let textures = texture_slots(model);

    if slots > 1 {
        restamp(root, slots);
    }

    let targets: Vec<Option<Vec<usize>>> = model
        .parts
        .iter()
        .map(|part| {
            let written = part.target.as_deref()?;
            let name = entry
                .parts
                .iter()
                .find(|(of, _)| *of == written)
                .map_or(written, |(_, ours)| *ours);
            let found = find(root, name);
            if found.is_none() {
                crate::log_warn!("cem", "{spec}: no part {written} to attach to");
            }
            found
        })
        .collect();

    let mut hidden: Vec<Vec<usize>> = Vec::new();
    for (part, target) in model.parts.iter().zip(&targets) {
        let Some(path) = target else { continue };
        if part.attach {
            continue;
        }
        let node = at(root, path);
        node.cubes_mut().clear();
        for (i, (name, _)) in node.children_mut().iter().enumerate() {
            if !known(entry, name) {
                let mut child = path.clone();
                child.push(i);
                hidden.push(child);
            }
        }
    }

    let mut top_ids: Vec<String> = Vec::new();
    let ctx = Ctx {
        sheet,
        slots,
        textures: &textures,
        role: entry.role,
    };
    for (part, target) in model.parts.iter().zip(&targets) {
        if part.target.is_none() {
            continue;
        }
        let id = unique(&part.id, |name| top_ids.iter().any(|t| t == name));
        top_ids.push(id.clone());
        let Some(path) = target else { continue };
        if let Some(def) = build(part, 0, &ctx) {
            insert(at(root, path), &id, def);
        }
    }

    for path in &hidden {
        clear_subtree(at(root, path));
    }
    layer
}

struct Ctx<'a> {
    sheet: [f32; 2],
    slots: usize,
    textures: &'a [&'a str],
    role: Role,
}

fn texture_slots(model: &Model) -> Vec<&str> {
    fn walk<'a>(part: &'a Part, out: &mut Vec<&'a str>) {
        if let Some(texture) = part.texture.as_deref()
            && !out.contains(&texture)
        {
            out.push(texture);
        }
        for child in &part.children {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    for part in &model.parts {
        walk(part, &mut out);
    }
    out
}

fn build(part: &Part, inherited: usize, ctx: &Ctx) -> Option<PartDef> {
    let slot = match part.texture.as_deref() {
        Some(_) if ctx.role == Role::Layer => return None,
        Some(texture) => ctx
            .textures
            .iter()
            .position(|t| *t == texture)
            .map_or(inherited, |i| i + 1),
        None => inherited,
    };
    let tex_scale = [
        part.tex_size[0] / ctx.sheet[0],
        part.tex_size[1] / ctx.sheet[1],
    ];
    let cubes = part
        .boxes
        .iter()
        .map(|b| cube(b, tex_scale, slot, ctx.slots))
        .collect();
    let mut node = PartDef::from_cubes(cubes, part.pose);
    for child in &part.children {
        if let Some(def) = build(child, slot, ctx) {
            insert(&mut node, &child.id, def);
        }
    }
    Some(node)
}

fn cube(b: &BoxDef, tex_scale: [f32; 2], slot: usize, slots: usize) -> CubeDef {
    let [x, y, z, w, h, d] = b.coords;
    let (tex, layout) = match b.uv {
        BoxUv::Offset {
            offset,
            mirror_u: false,
            mirror_v: false,
        } => (offset, Layout::Vanilla),
        BoxUv::Offset {
            offset,
            mirror_u,
            mirror_v,
        } => (
            offset,
            Layout::MirroredBox {
                u: mirror_u,
                v: mirror_v,
            },
        ),
        BoxUv::PerFace(rects) => ([0.0, 0.0], Layout::PerFace(rects)),
    };
    CubeDef {
        tex,
        origin: [x, y, z],
        size: [w, h, d],
        grow: Grow(b.grow[0], b.grow[1], b.grow[2]),
        mirror: false,
        tex_scale,
        faces: Faces::ALL,
        cem: Some(CemCube {
            layout,
            slot: [slot as f32, slots as f32],
        }),
    }
}

fn restamp(node: &mut PartDef, slots: usize) {
    for cube in node.cubes_mut() {
        cube.cem = Some(CemCube::slotted(0, slots));
    }
    for (_, child) in node.children_mut() {
        restamp(child, slots);
    }
}

fn insert(parent: &mut PartDef, name: &str, def: PartDef) {
    let children = parent.children_mut();
    let name = unique(name, |n| children.iter().any(|(c, _)| c == n));
    children.push((name, def));
}

fn unique(desired: &str, taken: impl Fn(&str) -> bool) -> String {
    let mut name = desired.to_owned();
    while taken(&name) {
        name.push('#');
    }
    name
}

fn known(entry: &Entry, name: &str) -> bool {
    entry.parts.is_empty() || entry.parts.iter().any(|(_, ours)| *ours == name)
}

fn find(root: &mut PartDef, name: &str) -> Option<Vec<usize>> {
    fn walk(node: &mut PartDef, name: &str, path: &mut Vec<usize>, found: &mut Option<Vec<usize>>) {
        for (i, (child_name, child)) in node.children_mut().iter_mut().enumerate() {
            path.push(i);
            walk(child, name, path, found);
            if child_name == name {
                *found = Some(path.clone());
            }
            path.pop();
        }
    }
    let mut found = None;
    walk(root, name, &mut Vec::new(), &mut found);
    if found.is_none() && name == "root" {
        found = Some(Vec::new());
    }
    found
}

fn at<'a>(root: &'a mut PartDef, path: &[usize]) -> &'a mut PartDef {
    let mut node = root;
    for &i in path {
        node = &mut node.children_mut()[i].1;
    }
    node
}

fn clear_subtree(node: &mut PartDef) {
    node.cubes_mut().clear();
    for (_, child) in node.children_mut() {
        clear_subtree(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::cem::{Dir, jem};
    use crate::entities::geom::{CubeList, MeshDef, PartPose, bake};

    const ENTRY: Entry = Entry {
        spec: "test",
        files: &["test"],
        role: Role::Main,
        parts: &[("head", "head"), ("body", "body")],
    };

    fn vanilla() -> LayerDef {
        let mut mesh = MeshDef::new();
        let root = mesh.root();
        let head = root.child(
            "head",
            CubeList::new().add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0),
            PartPose::offset(0.0, 6.0, 0.0),
        );
        head.child(
            "hat",
            CubeList::new().add_box(-4.0, -8.0, -4.0, 8.0, 8.0, 8.0),
            PartPose::ZERO,
        );
        root.child(
            "body",
            CubeList::new().add_box(-4.0, 0.0, -2.0, 8.0, 12.0, 4.0),
            PartPose::ZERO,
        );
        LayerDef::create(mesh, 64, 32)
    }

    fn model(json: &str) -> Model {
        jem::parse(
            json.as_bytes(),
            &Dir {
                folder: "optifine/cem/".into(),
                file: "t.jem".into(),
            },
        )
        .unwrap()
    }

    #[test]
    fn replace_keeps_vanilla_names() {
        let m = model(
            r#"{"models":[{"part":"head","id":"head","boxes":[{"textureOffset":[0,0],"coordinates":[0,0,0,1,1,1]}]}]}"#,
        );
        let baked = bake(&apply(vanilla(), &m, &ENTRY, 1, "test"));
        let head = baked.id("head");
        assert!(baked.parts[head].mesh.is_none());
        assert!(baked.parts[baked.id("hat")].mesh.is_none());
        let custom = baked.id("EMF_head");
        assert_eq!(baked.parts[custom].parent, Some(head));
        assert!(baked.parts[custom].mesh.is_some());
        assert!(baked.parts[baked.id("body")].mesh.is_some());
    }

    #[test]
    fn duplicate_ids_are_made_unique() {
        let m = model(
            r#"{"models":[{"part":"body","id":"x","attach":true},{"part":"body","id":"x","attach":true}]}"#,
        );
        let baked = bake(&apply(vanilla(), &m, &ENTRY, 1, "test"));
        assert!(baked.find("EMF_x").is_some());
        assert!(baked.find("EMF_x#").is_some());
        assert!(baked.parts[baked.id("body")].mesh.is_some());

        let m = model(
            r#"{"models":[{"part":"head","id":"x","attach":true},{"part":"body","id":"x","attach":true}]}"#,
        );
        let baked = bake(&apply(vanilla(), &m, &ENTRY, 1, "test"));
        assert_eq!(
            baked.parts[baked.id("EMF_x")].parent,
            Some(baked.id("head"))
        );
        assert_eq!(
            baked.parts[baked.id("EMF_x#")].parent,
            Some(baked.id("body"))
        );
    }
}
