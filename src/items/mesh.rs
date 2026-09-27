use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use bevy::math::Mat4;
use image::RgbaImage;

use crate::blockentities::render::skull;
use crate::entities::geom;

use super::icons::{
    LAYERS, Part, Synthetic, baked_quads, banner_quads, chest_quads, collect_parts, cube_quads,
    model_quads, rgb_of, rigid_quads, shield_quads, tint_at,
};
use super::model::{
    Assets, DIR_SHADE, DOWN, EAST, NORTH, Resolved, SOUTH, Tex, UP, WEST, corner_uv, face_positions,
};
use super::raster::{self, Quad};

pub use super::model::Transform;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
    pub normal: [f32; 3],
}

pub struct ItemMesh {
    pub vertices: Vec<ItemVertex>,
    pub indices: Vec<u32>,
    pub image: RgbaImage,
    pub display: HashMap<String, Transform>,
    #[allow(dead_code, reason = "asserted on by this module's tests")]
    pub generated: bool,
}

pub struct ItemMeshes {
    assets: Assets,
}

impl ItemMeshes {
    pub fn new(assets: &Path) -> ItemMeshes {
        ItemMeshes {
            assets: Assets::new(assets),
        }
    }

    pub fn context_sensitive(&self, id: &str) -> bool {
        let id = match crate::items::potions::parse_tint_key(id) {
            Some((item, _)) => item,
            None => crate::blockentities::banner::key_item(id),
        };
        self.assets
            .json(&format!("items/{}.json", id))
            .and_then(|doc| doc.get("model").cloned())
            .is_some_and(|root| super::icons::depends_on_display_context(&root, 0))
    }

    pub fn depends_on_using_item(&self, id: &str) -> bool {
        let id = match crate::items::potions::parse_tint_key(id) {
            Some((item, _)) => item,
            None => crate::blockentities::banner::key_item(id),
        };
        self.assets
            .json(&format!("items/{}.json", id))
            .and_then(|doc| doc.get("model").cloned())
            .is_some_and(|root| super::icons::depends_on_using_item(&root, 0))
    }

    pub fn bake_pattern_icon(&self, key: &str, edge: u32) -> Option<image::RgbaImage> {
        let (item, layers) = crate::blockentities::banner::parse_model_key(key)?;
        let root = self
            .assets
            .json(&format!("items/{item}.json"))?
            .get("model")
            .cloned()?;

        let mut parts = Vec::new();
        collect_parts(
            &self.assets,
            &root,
            "gui",
            false,
            Mat4::IDENTITY,
            &mut parts,
            0,
        );
        let mut quads = Vec::new();
        let model = parts.iter().find_map(|part| match &part.synthetic {
            Some(Synthetic::Banner(color, local)) => {
                banner_quads(&self.assets, *color, &layers, *local, &mut quads);
                Some(&part.model)
            }
            Some(Synthetic::Shield(local)) => {
                shield_quads(&self.assets, &layers, *local, &mut quads);
                Some(&part.model)
            }
            _ => None,
        })?;
        if quads.is_empty() {
            return None;
        }
        let transform = self.assets.model(model).gui.unwrap_or_else(|| {
            self.assets
                .model("block/block")
                .gui
                .unwrap_or(Transform::NONE)
        });
        Some(raster::render(&quads, &transform, edge))
    }

    pub fn build(&self, id: &str, ctx: &str, using: bool) -> Option<ItemMesh> {
        let assets = &self.assets;
        let (id, banner_layers) = match crate::blockentities::banner::parse_model_key(id) {
            Some((item, layers)) => (item, layers),
            None => (id, Vec::new()),
        };
        let (id, tint_override) = match crate::items::potions::parse_tint_key(id) {
            Some((item, color)) => (item, Some(rgb_of(color as i32))),
            None => (id, None),
        };
        let root = assets
            .json(&format!("items/{}.json", id))?
            .get("model")
            .cloned()?;
        let mut parts: Vec<Part> = Vec::new();
        collect_parts(assets, &root, ctx, using, Mat4::IDENTITY, &mut parts, 0);
        if let Some(tint) = tint_override {
            for part in &mut parts {
                if let Some(first) = part.tints.first_mut() {
                    *first = tint;
                }
            }
        }

        let mut quads: Vec<Quad> = Vec::new();
        let mut display: Option<HashMap<String, Transform>> = None;
        let mut generated = false;
        let mut synthesised = false;

        for part in &parts {
            let model = assets.model(&part.model);
            let before = quads.len();
            match &part.synthetic {
                Some(Synthetic::Chest(texture)) => {
                    if let Some(tex) = assets.texture(texture) {
                        chest_quads(&tex, &mut quads);
                    }
                }
                Some(Synthetic::Skull(kind, local)) => {
                    if let Some(tex) = assets.texture(skull::texture_for_type(kind)) {
                        let baked = geom::bake(&skull::layer_for_type(kind));
                        baked_quads(&baked, *local, &tex, &mut quads);
                    }
                }
                Some(Synthetic::Bed(texture, is_head, local)) => {
                    if let Some(tex) = assets.texture(texture) {
                        let layer = if *is_head {
                            crate::blockentities::models::bed_head()
                        } else {
                            crate::blockentities::models::bed_foot()
                        };
                        let baked = geom::bake(&layer);
                        baked_quads(&baked, *local, &tex, &mut quads);
                    }
                }
                Some(Synthetic::Banner(color, local)) => {
                    banner_quads(assets, *color, &banner_layers, *local, &mut quads);
                }
                Some(Synthetic::Shield(local)) => {
                    shield_quads(assets, &banner_layers, *local, &mut quads);
                }
                Some(Synthetic::Rigid(layer, texture, local)) => {
                    rigid_quads(assets, *layer, texture, *local, &mut quads);
                }
                Some(Synthetic::Cube(explicit)) => {
                    let path = explicit
                        .clone()
                        .or_else(|| model.texture_path("#particle"))
                        .unwrap_or_default();
                    if let Some(tex) = assets.texture(&path) {
                        cube_quads(&tex, &mut quads);
                    }
                }
                None if model.generated => {
                    generated |= extruded_sprite(assets, &model, &part.tints, &mut quads);
                }
                None => model_quads(assets, &model, &part.tints, &mut quads),
            }
            if quads.len() > before {
                synthesised |= part.synthetic.is_some();
                display.get_or_insert_with(|| model.display.clone());
            }
        }

        if quads.is_empty() {
            return None;
        }

        let mut display = display.unwrap_or_default();
        if display.is_empty() && synthesised {
            display = assets.model("block/block").display.clone();
        }

        let packed = Packed::of(&quads);
        let (vertices, indices) = triangulate(&quads, &packed);
        if vertices.is_empty() {
            return None;
        }
        Some(ItemMesh {
            vertices,
            indices,
            image: packed.image,
            display,
            generated,
        })
    }
}

const MIN_Z: f32 = 7.5;
const MAX_Z: f32 = 8.5;

const UV_SHRINK: f32 = 0.1;

fn extruded_sprite(
    assets: &Assets,
    model: &Resolved,
    tints: &[[f32; 3]],
    out: &mut Vec<Quad>,
) -> bool {
    let mut drew = false;
    for (index, layer) in LAYERS.iter().enumerate() {
        if !model.textures.contains_key(*layer) {
            break;
        }
        let Some(path) = model.texture_path(&format!("#{}", layer)) else {
            break;
        };
        let Some(tex) = assets.texture(&path) else {
            break;
        };
        let tint = tint_at(tints, index as i32);
        sprite_faces(&tex, tint, out);
        drew = true;
    }
    drew
}

fn sprite_faces(tex: &Arc<Tex>, tint: [f32; 3], out: &mut Vec<Quad>) {
    let from = [0.0, 0.0, MIN_Z];
    let to = [16.0, 16.0, MAX_Z];
    push_quad(out, from, to, SOUTH, [0.0, 0.0, 16.0, 16.0], tex, tint);
    push_quad(out, from, to, NORTH, [16.0, 0.0, 0.0, 16.0], tex, tint);

    let xs = 16.0 / tex.w as f32;
    let ys = 16.0 / tex.h as f32;
    for y in 0..tex.h {
        for x in 0..tex.w {
            if transparent(tex, x as i32, y as i32) {
                continue;
            }
            let (xi, yi) = (x as i32, y as i32);
            let (u0, u1) = (
                (x as f32 + UV_SHRINK) * xs,
                (x as f32 + 1.0 - UV_SHRINK) * xs,
            );
            let (vh0, vh1) = (
                (y as f32 + UV_SHRINK) * ys,
                (y as f32 + 1.0 - UV_SHRINK) * ys,
            );
            let (x0, x1) = (x as f32 * xs, (x as f32 + 1.0) * xs);
            let (y_top, y_bottom) = (16.0 - y as f32 * ys, 16.0 - (y as f32 + 1.0) * ys);
            if transparent(tex, xi, yi - 1) {
                push_quad(
                    out,
                    [x0, y_top, MIN_Z],
                    [x1, y_top, MAX_Z],
                    UP,
                    [u0, vh0, u1, vh1],
                    tex,
                    tint,
                );
            }
            if transparent(tex, xi, yi + 1) {
                push_quad(
                    out,
                    [x0, y_bottom, MIN_Z],
                    [x1, y_bottom, MAX_Z],
                    DOWN,
                    [u0, vh0, u1, vh1],
                    tex,
                    tint,
                );
            }
            let (vv0, vv1) = (vh1, vh0);
            if transparent(tex, xi - 1, yi) {
                push_quad(
                    out,
                    [x0, y_top, MIN_Z],
                    [x0, y_bottom, MAX_Z],
                    EAST,
                    [u0, vv0, u1, vv1],
                    tex,
                    tint,
                );
            }
            if transparent(tex, xi + 1, yi) {
                push_quad(
                    out,
                    [x1, y_top, MIN_Z],
                    [x1, y_bottom, MAX_Z],
                    WEST,
                    [u0, vv0, u1, vv1],
                    tex,
                    tint,
                );
            }
        }
    }
}

fn transparent(tex: &Tex, x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= tex.w as i32 || y >= tex.h as i32 {
        return true;
    }
    tex.get(x as u32, y as u32)[3] == 0
}

fn push_quad(
    out: &mut Vec<Quad>,
    from: [f32; 3],
    to: [f32; 3],
    dir: usize,
    uv: [f32; 4],
    tex: &Arc<Tex>,
    tint: [f32; 3],
) {
    let scaled_from = [from[0] / 16.0, from[1] / 16.0, from[2] / 16.0];
    let scaled_to = [to[0] / 16.0, to[1] / 16.0, to[2] / 16.0];
    let mut corners = [[0.0f32; 2]; 4];
    for (i, c) in corners.iter_mut().enumerate() {
        let [u, v] = corner_uv(uv, 0, i);
        *c = [u / 16.0, v / 16.0];
    }
    let shade = DIR_SHADE[dir];
    out.push(Quad {
        pos: face_positions(scaled_from, scaled_to, dir),
        uv: corners,
        tex: tex.clone(),
        color: [tint[0] * shade, tint[1] * shade, tint[2] * shade],
    });
}

struct Packed {
    image: RgbaImage,
    slots: HashMap<usize, [u32; 4]>,
}

impl Packed {
    fn of(quads: &[Quad]) -> Packed {
        let mut order: Vec<(usize, &Arc<Tex>)> = Vec::new();
        for quad in quads {
            let key = Arc::as_ptr(&quad.tex) as usize;
            if !order.iter().any(|(k, _)| *k == key) {
                order.push((key, &quad.tex));
            }
        }
        let width: u32 = order.iter().map(|(_, t)| t.w).sum();
        let height: u32 = order.iter().map(|(_, t)| t.h).max().unwrap_or(1);
        let mut image = RgbaImage::new(width.max(1), height.max(1));
        let mut slots = HashMap::with_capacity(order.len());
        let mut x = 0u32;
        for (key, tex) in order {
            for ty in 0..tex.h {
                for tx in 0..tex.w {
                    image.put_pixel(x + tx, ty, image::Rgba(tex.get(tx, ty)));
                }
            }
            slots.insert(key, [x, 0, tex.w, tex.h]);
            x += tex.w;
        }
        Packed { image, slots }
    }

    fn remap(&self, tex: &Arc<Tex>, uv: [f32; 2]) -> [f32; 2] {
        let Some([x, y, w, h]) = self.slots.get(&(Arc::as_ptr(tex) as usize)).copied() else {
            return uv;
        };
        [
            (x as f32 + uv[0] * w as f32) / self.image.width() as f32,
            (y as f32 + uv[1] * h as f32) / self.image.height() as f32,
        ]
    }
}

fn quad_normal(pos: &[[f32; 3]; 4]) -> Option<[f32; 3]> {
    let mut n = [0.0f32; 3];
    for i in 0..4 {
        let a = pos[i];
        let b = pos[(i + 1) & 3];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if !len.is_finite() || len < 1e-9 {
        return None;
    }
    Some([n[0] / len, n[1] / len, n[2] / len])
}

fn triangulate(quads: &[Quad], packed: &Packed) -> (Vec<ItemVertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(quads.len() * 4);
    let mut indices = Vec::with_capacity(quads.len() * 6);
    for quad in quads {
        let Some(normal) = quad_normal(&quad.pos) else {
            continue;
        };
        let base = vertices.len() as u32;
        for i in 0..4 {
            vertices.push(ItemVertex {
                pos: quad.pos[i],
                uv: packed.remap(&quad.tex, quad.uv[i]),
                color: quad.color,
                normal,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

#[allow(dead_code, reason = "used by this module's tests")]
pub fn apply_transform(p: [f32; 3], t: &Transform, left_hand: bool) -> [f32; 3] {
    let sign = if left_hand { -1.0 } else { 1.0 };
    let rotation = [t.rotation[0], t.rotation[1] * sign, t.rotation[2] * sign];
    let translation = [t.translation[0] * sign, t.translation[1], t.translation[2]];
    let centred = [
        (p[0] - 0.5) * t.scale[0],
        (p[1] - 0.5) * t.scale[1],
        (p[2] - 0.5) * t.scale[2],
    ];
    let rotated = raster::rotate_xyz(centred, rotation);
    [
        rotated[0] + translation[0],
        rotated[1] + translation[1],
        rotated[2] + translation[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASSETS: &str = "reference/minecraft-26.1.1/assets/minecraft";

    fn meshes() -> Option<ItemMeshes> {
        let root = Path::new(ASSETS);
        if !root.exists() {
            eprintln!("assets missing, skipping");
            return None;
        }
        Some(ItemMeshes::new(root))
    }

    #[test]
    fn sprite_item_is_extruded() {
        let Some(meshes) = meshes() else { return };
        let mesh = meshes
            .build("diamond_sword", "thirdperson_righthand", false)
            .expect("diamond_sword");
        assert!(mesh.generated);
        assert_eq!(mesh.indices.len(), mesh.vertices.len() / 4 * 6);
        assert!(
            mesh.vertices.len() > 4 * 20,
            "only {} vertices",
            mesh.vertices.len()
        );

        let mut depths: Vec<f32> = mesh.vertices.iter().map(|v| v.pos[2]).collect();
        depths.sort_by(f32::total_cmp);
        assert!((depths[0] - MIN_Z / 16.0).abs() < 1e-6);
        assert!((depths[depths.len() - 1] - MAX_Z / 16.0).abs() < 1e-6);

        for v in &mesh.vertices {
            assert!(v.uv[0] >= 0.0 && v.uv[0] <= 1.0, "u {:?}", v.uv);
            assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0, "v {:?}", v.uv);
            for axis in 0..3 {
                assert!(
                    v.pos[axis] >= -1e-6 && v.pos[axis] <= 1.0 + 1e-6,
                    "pos {:?}",
                    v.pos
                );
            }
            let len =
                (v.normal[0] * v.normal[0] + v.normal[1] * v.normal[1] + v.normal[2] * v.normal[2])
                    .sqrt();
            assert!((len - 1.0).abs() < 1e-4, "normal {:?}", v.normal);
        }
        assert_eq!((mesh.image.width(), mesh.image.height()), (16, 16));
    }

    #[test]
    fn block_item_is_a_cube() {
        let Some(meshes) = meshes() else { return };
        let mesh = meshes
            .build("stone", "thirdperson_righthand", false)
            .expect("stone");
        assert!(!mesh.generated);
        assert_eq!(mesh.vertices.len(), 6 * 4);
        assert_eq!(mesh.indices.len(), 6 * 6);
        let mut normals: Vec<[i32; 3]> = mesh
            .vertices
            .chunks(4)
            .map(|face| {
                let n = face[0].normal;
                [
                    n[0].round() as i32,
                    n[1].round() as i32,
                    n[2].round() as i32,
                ]
            })
            .collect();
        normals.sort_unstable();
        assert_eq!(
            normals,
            [
                [-1, 0, 0],
                [0, -1, 0],
                [0, 0, -1],
                [0, 0, 1],
                [0, 1, 0],
                [1, 0, 0]
            ]
        );
        assert!(mesh.display.contains_key("thirdperson_righthand"));
    }

    #[test]
    fn chest_gets_synthetic_geometry() {
        let Some(meshes) = meshes() else { return };
        let mesh = meshes
            .build("chest", "thirdperson_righthand", false)
            .expect("chest");
        assert_eq!(mesh.vertices.len(), 3 * 6 * 4);
        assert!(mesh.display.contains_key("thirdperson_righthand"));
    }

    #[test]
    fn skull_gets_synthetic_geometry() {
        let Some(meshes) = meshes() else { return };
        let mesh = meshes
            .build("skeleton_skull", "thirdperson_righthand", false)
            .expect("skeleton_skull");
        assert_eq!(mesh.vertices.len(), 6 * 4);
    }

    #[test]
    fn transform_matches_hand_computed_point() {
        let Some(meshes) = meshes() else { return };
        let mesh = meshes
            .build("diamond_sword", "thirdperson_righthand", false)
            .expect("diamond_sword");
        let t = mesh.display.get("thirdperson_righthand").expect("slot");
        assert_eq!(t.rotation, [0.0, -90.0, 55.0]);

        let right = apply_transform([1.0, 0.5, 0.5], t, false);
        for (got, want) in right.iter().zip([0.0, 0.59813962, 0.27501999].iter()) {
            assert!((got - want).abs() < 1e-5, "{:?} != {:?}", right, want);
        }
        let left = apply_transform([1.0, 0.5, 0.5], t, true);
        for (got, want) in left.iter().zip([0.0, -0.09813962, -0.21251999].iter()) {
            assert!((got - want).abs() < 1e-5, "{:?} != {:?}", left, want);
        }
    }

    #[test]
    fn no_transform_only_recentres() {
        let p = [0.25, 1.0, 0.0];
        assert_eq!(
            apply_transform(p, &Transform::NONE, false),
            [-0.25, 0.5, -0.5]
        );
        assert_eq!(
            apply_transform(p, &Transform::NONE, true),
            [-0.25, 0.5, -0.5]
        );
    }

    #[test]
    fn unknown_id_is_none() {
        let Some(meshes) = meshes() else { return };
        assert!(meshes.build("not_a_real_item", "gui", false).is_none());
        assert!(meshes.build("", "gui", false).is_none());
        assert!(meshes.build("../../etc/passwd", "gui", false).is_none());
    }

    #[test]
    fn display_context_picks_the_selected_case() {
        let Some(meshes) = meshes() else { return };
        assert!(meshes.context_sensitive("trident"));
        assert!(!meshes.context_sensitive("stone"));

        let framed = meshes
            .build("trident", "fixed", false)
            .expect("framed trident");
        assert!(framed.generated, "the fixed case is the flat sprite model");
        let held = meshes
            .build("trident", "thirdperson_righthand", false)
            .expect("held trident");
        assert!(!held.generated);
        assert_ne!(framed.vertices.len(), held.vertices.len());
    }
}
