use crate::platform::time::Instant;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use bevy::math::{Mat4, Quat, Vec3};
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::Mesh;
use image::RgbaImage;

use crate::blockentities::banner::BannerLayer;
use crate::blockentities::models;
use crate::blockentities::render::skull;
use crate::entities::geom;
use crate::items::potions;

use super::json::Json;
use super::model::{
    Assets, DIR_SHADE, Resolved, Tex, Transform, corner_uv, default_face_uv, face_positions,
    strip_namespace,
};
use super::raster::{self, Quad};

pub const FLAT_ICON: u32 = 16;
pub const MODEL_ICON: u32 = 64;
const MODEL_ICON_TIERS: [u32; 4] = [FLAT_ICON, FLAT_ICON * 2, FLAT_ICON * 3, MODEL_ICON];
const PACK_WIDTH: u32 = 2048;

pub fn model_tier(target_px: f32) -> u32 {
    MODEL_ICON_TIERS
        .into_iter()
        .find(|size| *size as f32 + 0.5 >= target_px)
        .unwrap_or(MODEL_ICON)
}

pub struct ItemIcons {
    pub image: RgbaImage,
    pub map: HashMap<String, Vec<(u32, [u32; 4])>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Dispatch {
    Model,
    Select,
    Condition,
    RangeDispatch,
    Composite,
    Special,
    Bundle,
    Unknown,
}

impl Dispatch {
    fn of(node: &Json) -> Dispatch {
        match node.get("type").and_then(Json::as_str).map(strip_namespace) {
            Some("model") => Dispatch::Model,
            Some("select") => Dispatch::Select,
            Some("condition") => Dispatch::Condition,
            Some("range_dispatch") => Dispatch::RangeDispatch,
            Some("composite") => Dispatch::Composite,
            Some("special") => Dispatch::Special,
            Some("bundle/selected_item") => Dispatch::Bundle,
            _ => Dispatch::Unknown,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Dispatch::Model => "model",
            Dispatch::Select => "select",
            Dispatch::Condition => "condition",
            Dispatch::RangeDispatch => "range_dispatch",
            Dispatch::Composite => "composite",
            Dispatch::Special => "special",
            Dispatch::Bundle => "bundle/selected_item",
            Dispatch::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum Synthetic {
    Chest(String),
    Bed(String, bool, Mat4),
    Skull(String, Mat4),
    Banner(u8, Mat4),
    Shield(Mat4),
    Rigid(fn() -> geom::LayerDef, String, Mat4),
    Cube(Option<String>),
}

#[derive(Clone, Debug)]
pub(super) struct Part {
    pub(super) model: String,
    pub(super) tints: Vec<[f32; 3]>,
    pub(super) synthetic: Option<Synthetic>,
}

fn parse_transformation(special: &Json) -> Mat4 {
    let Some(t) = special.get("transformation") else {
        return Mat4::IDENTITY;
    };
    let vec3 = |key: &str, default: [f32; 3]| {
        Vec3::from(t.get(key).and_then(Json::as_vec3).unwrap_or(default))
    };
    let quat = |key: &str| {
        let [x, y, z, w] = t
            .get(key)
            .and_then(Json::as_vec4)
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        Quat::from_xyzw(x, y, z, w)
    };
    Mat4::from_translation(vec3("translation", [0.0; 3]))
        * Mat4::from_quat(quat("left_rotation"))
        * Mat4::from_scale(vec3("scale", [1.0; 3]))
        * Mat4::from_quat(quat("right_rotation"))
}

pub(super) fn depends_on_display_context(node: &Json, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    let branches = |keys: &[&str]| {
        keys.iter()
            .filter_map(|k| node.get(k))
            .any(|child| depends_on_display_context(child, depth + 1))
    };
    match Dispatch::of(node) {
        Dispatch::Select => {
            is_display_context(node)
                || branches(&["fallback"])
                || node
                    .get("cases")
                    .map(Json::arr)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|case| case.get("model"))
                    .any(|child| depends_on_display_context(child, depth + 1))
        }
        Dispatch::Condition => branches(&["on_false", "on_true"]),
        Dispatch::RangeDispatch => {
            branches(&["fallback"])
                || node
                    .get("entries")
                    .map(Json::arr)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|entry| entry.get("model"))
                    .any(|child| depends_on_display_context(child, depth + 1))
        }
        Dispatch::Composite => node
            .get("models")
            .map(Json::arr)
            .unwrap_or_default()
            .iter()
            .any(|child| depends_on_display_context(child, depth + 1)),
        _ => false,
    }
}

fn is_display_context(node: &Json) -> bool {
    node.get("property")
        .and_then(Json::as_str)
        .map(strip_namespace)
        == Some("display_context")
}

fn is_using_item(node: &Json) -> bool {
    node.get("property")
        .and_then(Json::as_str)
        .map(strip_namespace)
        == Some("using_item")
}

pub(super) fn depends_on_using_item(node: &Json, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    let branches = |keys: &[&str]| {
        keys.iter()
            .filter_map(|k| node.get(k))
            .any(|child| depends_on_using_item(child, depth + 1))
    };
    match Dispatch::of(node) {
        Dispatch::Select => {
            branches(&["fallback"])
                || node
                    .get("cases")
                    .map(Json::arr)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|case| case.get("model"))
                    .any(|child| depends_on_using_item(child, depth + 1))
        }
        Dispatch::Condition => is_using_item(node) || branches(&["on_false", "on_true"]),
        Dispatch::RangeDispatch => {
            branches(&["fallback"])
                || node
                    .get("entries")
                    .map(Json::arr)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|entry| entry.get("model"))
                    .any(|child| depends_on_using_item(child, depth + 1))
        }
        Dispatch::Composite => node
            .get("models")
            .map(Json::arr)
            .unwrap_or_default()
            .iter()
            .any(|child| depends_on_using_item(child, depth + 1)),
        _ => false,
    }
}

fn case_matches(case: &Json, ctx: &str) -> bool {
    match case.get("when") {
        Some(Json::Arr(values)) => values.iter().any(|v| v.as_str() == Some(ctx)),
        Some(value) => value.as_str() == Some(ctx),
        None => false,
    }
}

pub(super) fn collect_parts(
    assets: &Assets,
    node: &Json,
    ctx: &str,
    using: bool,
    parent: Mat4,
    out: &mut Vec<Part>,
    depth: u32,
) {
    if depth > 8 || out.len() > 8 {
        return;
    }
    let local = parent * parse_transformation(node);
    match Dispatch::of(node) {
        Dispatch::Model => {
            let Some(model) = node.get("model").and_then(Json::as_str) else {
                return;
            };
            out.push(Part {
                model: strip_namespace(model).to_string(),
                tints: parse_tints(assets, node),
                synthetic: None,
            });
        }
        Dispatch::Special => {
            let Some(base) = node.get("base").and_then(Json::as_str) else {
                return;
            };
            let inner = node.get("model");
            let kind = inner
                .and_then(|m| m.get("type"))
                .and_then(Json::as_str)
                .map(strip_namespace)
                .unwrap_or("");
            let synthetic = match kind {
                "chest" => {
                    let texture = inner
                        .and_then(|m| m.get("texture"))
                        .and_then(Json::as_str)
                        .map(strip_namespace)
                        .unwrap_or("normal");
                    Synthetic::Chest(format!("entity/chest/{}", texture))
                }
                "bed" => {
                    let part = inner
                        .and_then(|m| m.get("part"))
                        .and_then(Json::as_str)
                        .unwrap_or("head");
                    let texture = inner
                        .and_then(|m| m.get("texture"))
                        .and_then(Json::as_str)
                        .map(strip_namespace)
                        .unwrap_or("red");
                    Synthetic::Bed(format!("entity/bed/{}", texture), part == "head", local)
                }
                "banner" => {
                    let color = inner
                        .and_then(|m| m.get("color"))
                        .and_then(Json::as_str)
                        .unwrap_or("white");
                    Synthetic::Banner(crate::blockentities::banner::color_by_name(color), local)
                }
                "head" => {
                    let kind = inner
                        .and_then(|m| m.get("kind"))
                        .and_then(Json::as_str)
                        .map(strip_namespace)
                        .unwrap_or("skeleton");
                    Synthetic::Skull(kind.to_string(), local)
                }
                "player_head" => Synthetic::Skull("player".to_string(), local),
                "shield" => Synthetic::Shield(local),
                "trident" => Synthetic::Rigid(
                    crate::entities::models::objects::projectile::trident_layer,
                    "entity/trident/trident".to_string(),
                    local,
                ),
                "conduit" => Synthetic::Rigid(
                    models::conduit_shell,
                    "entity/conduit/base".to_string(),
                    local,
                ),
                "shulker_box" => {
                    let texture = inner
                        .and_then(|m| m.get("texture"))
                        .and_then(Json::as_str)
                        .map(strip_namespace)
                        .unwrap_or("shulker");
                    Synthetic::Rigid(
                        crate::entities::models::monsters::shulker::box_layer,
                        format!("entity/shulker/{}", texture),
                        local,
                    )
                }
                _ => Synthetic::Cube(None),
            };
            out.push(Part {
                model: strip_namespace(base).to_string(),
                tints: parse_tints(assets, node),
                synthetic: Some(synthetic),
            });
        }
        Dispatch::Composite => {
            for child in node.get("models").map(Json::arr).unwrap_or_default() {
                collect_parts(assets, child, ctx, using, local, out, depth + 1);
            }
        }
        Dispatch::Select => {
            let matched = is_display_context(node)
                .then(|| {
                    node.get("cases")
                        .map(Json::arr)
                        .unwrap_or_default()
                        .iter()
                        .find(|case| case_matches(case, ctx))
                        .and_then(|case| case.get("model"))
                })
                .flatten();
            let branch = matched.or_else(|| {
                node.get("fallback").or_else(|| {
                    node.get("cases")
                        .and_then(|c| c.idx(0))
                        .and_then(|c| c.get("model"))
                })
            });
            if let Some(branch) = branch {
                collect_parts(assets, branch, ctx, using, local, out, depth + 1);
            }
        }
        Dispatch::Condition => {
            let branch = if is_using_item(node) {
                node.get(if using { "on_true" } else { "on_false" })
            } else {
                node.get("on_false").or_else(|| node.get("on_true"))
            };
            if let Some(branch) = branch {
                collect_parts(assets, branch, ctx, using, local, out, depth + 1);
            }
        }
        Dispatch::RangeDispatch => {
            let branch = node.get("fallback").or_else(|| {
                node.get("entries")
                    .and_then(|e| e.idx(0))
                    .and_then(|e| e.get("model"))
            });
            if let Some(branch) = branch {
                collect_parts(assets, branch, ctx, using, local, out, depth + 1);
            }
        }
        Dispatch::Bundle | Dispatch::Unknown => {}
    }
}

pub(super) fn rgb_of(packed: i32) -> [f32; 3] {
    let v = packed as u32;
    [
        ((v >> 16) & 0xff) as f32 / 255.0,
        ((v >> 8) & 0xff) as f32 / 255.0,
        (v & 0xff) as f32 / 255.0,
    ]
}

fn parse_tints(assets: &Assets, node: &Json) -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    for tint in node.get("tints").map(Json::arr).unwrap_or_default() {
        let kind = tint
            .get("type")
            .and_then(Json::as_str)
            .map(strip_namespace)
            .unwrap_or("");
        let color = match kind {
            "constant" => rgb_of(tint.get("value").and_then(Json::as_i32).unwrap_or(-1)),
            "grass" | "foliage" => {
                let temperature = tint
                    .get("temperature")
                    .and_then(Json::as_f32)
                    .unwrap_or(0.5);
                let downfall = tint.get("downfall").and_then(Json::as_f32).unwrap_or(1.0);
                let x = ((1.0 - temperature) * 255.0) as u32;
                let y = ((1.0 - downfall * temperature) * 255.0) as u32;
                assets.colormap(kind, x.min(255), y.min(255))
            }
            _ => rgb_of(tint.get("default").and_then(Json::as_i32).unwrap_or(-1)),
        };
        out.push(color);
    }
    out
}

pub(super) fn tint_at(tints: &[[f32; 3]], index: i32) -> [f32; 3] {
    if index < 0 {
        return [1.0; 3];
    }
    tints.get(index as usize).copied().unwrap_or([1.0; 3])
}

pub(super) fn model_quads(
    assets: &Assets,
    model: &Resolved,
    tints: &[[f32; 3]],
    out: &mut Vec<Quad>,
) {
    for elem in &model.elements {
        let from = [
            elem.from[0] / 16.0,
            elem.from[1] / 16.0,
            elem.from[2] / 16.0,
        ];
        let to = [elem.to[0] / 16.0, elem.to[1] / 16.0, elem.to[2] / 16.0];
        for face in &elem.faces {
            let Some(path) = model.texture_path(&face.texture) else {
                continue;
            };
            let Some(tex) = assets.texture(&path) else {
                continue;
            };
            let uv = face
                .uv
                .unwrap_or_else(|| default_face_uv(elem.from, elem.to, face.dir));
            let mut pos = face_positions(from, to, face.dir);
            if let Some(rot) = elem.rotation {
                let origin = [
                    rot.origin[0] / 16.0,
                    rot.origin[1] / 16.0,
                    rot.origin[2] / 16.0,
                ];
                for p in pos.iter_mut() {
                    *p = raster::rotate_axis(*p, origin, rot.axis, rot.angle, rot.rescale);
                }
            }
            let mut corners = [[0.0f32; 2]; 4];
            for (i, c) in corners.iter_mut().enumerate() {
                let [u, v] = corner_uv(uv, face.quadrant, i);
                *c = [u / 16.0, v / 16.0];
            }
            let shade = if elem.shade { DIR_SHADE[face.dir] } else { 1.0 };
            let tint = tint_at(tints, face.tint);
            out.push(Quad {
                pos,
                uv: corners,
                tex,
                color: [tint[0] * shade, tint[1] * shade, tint[2] * shade],
            });
        }
    }
}

pub(super) fn cube_quads(tex: &Arc<Tex>, out: &mut Vec<Quad>) {
    for dir in 0..6 {
        let uv = default_face_uv([0.0; 3], [16.0; 3], dir);
        let mut corners = [[0.0f32; 2]; 4];
        for (i, c) in corners.iter_mut().enumerate() {
            let [u, v] = corner_uv(uv, 0, i);
            *c = [u / 16.0, v / 16.0];
        }
        out.push(Quad {
            pos: face_positions([0.0; 3], [1.0; 3], dir),
            uv: corners,
            tex: tex.clone(),
            color: [DIR_SHADE[dir]; 3],
        });
    }
}

fn entity_box(
    tex: &Arc<Tex>,
    from: [f32; 3],
    to: [f32; 3],
    tex_offs: [f32; 2],
    tex_size: f32,
    out: &mut Vec<Quad>,
) {
    let (w, h, d) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    let (u, v) = (tex_offs[0], tex_offs[1]);
    let (u0, u1, u2, u22, u3, u4) = (
        u,
        u + d,
        u + d + w,
        u + d + w + w,
        u + d + w + d,
        u + d + w + d + w,
    );
    let (v0, v1, v2) = (v, v + d, v + d + h);
    let rects: [[f32; 4]; 6] = [
        [u1, v0, u2, v1],
        [u2, v1, u22, v0],
        [u2, v2, u1, v1],
        [u4, v2, u3, v1],
        [u1, v2, u0, v1],
        [u3, v2, u2, v1],
    ];
    let scaled_from = [from[0] / 16.0, from[1] / 16.0, from[2] / 16.0];
    let scaled_to = [to[0] / 16.0, to[1] / 16.0, to[2] / 16.0];
    for dir in 0..6 {
        let mut corners = [[0.0f32; 2]; 4];
        for (i, c) in corners.iter_mut().enumerate() {
            let [cu, cv] = corner_uv(rects[dir], 0, i);
            *c = [cu / tex_size, cv / tex_size];
        }
        out.push(Quad {
            pos: face_positions(scaled_from, scaled_to, dir),
            uv: corners,
            tex: tex.clone(),
            color: [DIR_SHADE[dir]; 3],
        });
    }
}

pub(super) fn baked_quads(
    model: &geom::BakedModel,
    root: Mat4,
    tex: &Arc<Tex>,
    out: &mut Vec<Quad>,
) {
    let rest = model.rest_pose();
    let mut world = vec![Mat4::IDENTITY; model.parts.len()];
    for (id, part) in model.parts.iter().enumerate() {
        let local = rest[id].transform().to_matrix();
        world[id] = match part.parent {
            Some(parent) => world[parent] * local,
            None => root * local,
        };
    }
    for (id, part) in model.parts.iter().enumerate() {
        let Some(mesh) = &part.mesh else { continue };
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
            .unwrap_or(&[]);
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .and_then(|a| a.as_float3())
            .unwrap_or(&[]);
        let uvs: &[[f32; 2]] = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(v)) => v,
            _ => &[],
        };
        let mat = world[id];
        for (i, (pos, uv)) in positions
            .chunks_exact(4)
            .zip(uvs.chunks_exact(4))
            .enumerate()
        {
            let normal = normals.get(i * 4).copied().unwrap_or([0.0, 1.0, 0.0]);
            let world_normal = mat.transform_vector3(Vec3::from(normal));
            out.push(Quad {
                pos: std::array::from_fn(|j| mat.transform_point3(Vec3::from(pos[j])).to_array()),
                uv: std::array::from_fn(|j| uv[j]),
                tex: tex.clone(),
                color: [DIR_SHADE[nearest_direction(world_normal)]; 3],
            });
        }
    }
}

fn nearest_direction(n: Vec3) -> usize {
    const AXES: [[f32; 3]; 6] = [
        [0.0, -1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
        [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ];
    AXES.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            n.dot(Vec3::from_array(**a))
                .total_cmp(&n.dot(Vec3::from_array(**b)))
        })
        .map_or(1, |(i, _)| i)
}

pub(super) fn chest_quads(tex: &Arc<Tex>, out: &mut Vec<Quad>) {
    entity_box(
        tex,
        [1.0, 0.0, 1.0],
        [15.0, 10.0, 15.0],
        [0.0, 19.0],
        64.0,
        out,
    );
    entity_box(
        tex,
        [1.0, 9.0, 1.0],
        [15.0, 14.0, 15.0],
        [0.0, 0.0],
        64.0,
        out,
    );
    entity_box(
        tex,
        [7.0, 7.0, 15.0],
        [9.0, 11.0, 16.0],
        [0.0, 0.0],
        64.0,
        out,
    );
}

pub(super) fn banner_quads(
    assets: &Assets,
    color: u8,
    layers: &[BannerLayer],
    local: Mat4,
    out: &mut Vec<Quad>,
) {
    let masks = std::iter::once(("base", color)).chain(mask_pairs(layers));
    let Some(tex) = pattern_tex(assets, "entity/banner", "banner_base", masks) else {
        return;
    };
    for layer in [
        models::banner_body_standing(),
        models::banner_flag_standing(),
    ] {
        baked_quads(&geom::bake(&layer), local, &tex, out);
    }
}

pub(super) fn shield_quads(
    assets: &Assets,
    layers: &[BannerLayer],
    local: Mat4,
    out: &mut Vec<Quad>,
) {
    let base = match layers.is_empty() {
        true => "shield_base_nopattern",
        false => "shield_base",
    };
    let Some(tex) = pattern_tex(assets, "entity/shield", base, mask_pairs(layers)) else {
        return;
    };
    baked_quads(
        &geom::bake(&crate::entities::models::objects::misc::shield_layer()),
        local,
        &tex,
        out,
    );
}

pub(super) fn rigid_quads(
    assets: &Assets,
    layer: fn() -> geom::LayerDef,
    texture: &str,
    local: Mat4,
    out: &mut Vec<Quad>,
) {
    if let Some(tex) = assets.texture(texture) {
        baked_quads(&geom::bake(&layer()), local, &tex, out);
    }
}

fn mask_pairs(layers: &[BannerLayer]) -> impl Iterator<Item = (&str, u8)> {
    layers.iter().map(|layer| (&*layer.asset, layer.color))
}

fn pattern_tex<'a>(
    assets: &Assets,
    dir: &str,
    base_png: &str,
    masks: impl Iterator<Item = (&'a str, u8)>,
) -> Option<Arc<Tex>> {
    let base = assets.texture(&format!("{}/{}", dir, base_png))?;
    let mut px = base.px.clone();
    let mut blend = |mask: &Tex, color: u8| {
        if mask.w != base.w || mask.h != base.h {
            return;
        }
        let dye = crate::blockentities::banner::tint(color);
        for (i, dst) in px.iter_mut().enumerate() {
            let src = mask.get(i as u32 % base.w, i as u32 / base.w);
            if src[3] == 0 {
                continue;
            }
            let a = src[3] as f32 / 255.0;
            for c in 0..3 {
                let src_lin = crate::util::mth::srgb_to_linear(src[c] as f32 / 255.0);
                let dst_lin = crate::util::mth::srgb_to_linear(dst[c] as f32 / 255.0);
                let tinted_lin = src_lin * dye[c];
                let out_lin = tinted_lin * a + dst_lin * (1.0 - a);
                dst[c] = (crate::util::mth::linear_to_srgb(out_lin) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            dst[3] = dst[3].max(src[3]);
        }
    };

    for (asset, color) in masks.take(crate::blockentities::banner::MAX_PATTERNS + 1) {
        let Some(mask) = assets.texture(&format!("{}/{}", dir, asset)) else {
            continue;
        };
        blend(&mask, color);
    }

    Some(Arc::new(Tex {
        w: base.w,
        h: base.h,
        px,
    }))
}

pub(super) const LAYERS: [&str; 5] = ["layer0", "layer1", "layer2", "layer3", "layer4"];

fn flat_icon(assets: &Assets, model: &Resolved, tints: &[[f32; 3]], dst: &mut RgbaImage) -> bool {
    let mut drew = false;
    for (index, layer) in LAYERS.iter().enumerate() {
        if !model.textures.contains_key(*layer) {
            continue;
        }
        let Some(path) = model.texture_path(&format!("#{}", layer)) else {
            continue;
        };
        let Some(tex) = assets.texture(&path) else {
            continue;
        };
        let tint = tint_at(tints, index as i32);
        blit_scaled(&tex, tint, dst);
        drew = true;
    }
    drew
}

fn blit_scaled(tex: &Tex, tint: [f32; 3], dst: &mut RgbaImage) {
    let (dw, dh) = (dst.width(), dst.height());
    for y in 0..dh {
        for x in 0..dw {
            let src = tex.get(x * tex.w / dw, y * tex.h / dh);
            let a = src[3] as f32 / 255.0;
            if a <= 0.0 {
                continue;
            }
            let old = dst.get_pixel(x, y).0;
            let old_a = old[3] as f32 / 255.0;
            let out_a = a + old_a * (1.0 - a);
            let mut px = [0u8; 4];
            for c in 0..3 {
                let s = src[c] as f32 / 255.0 * tint[c];
                let o = old[c] as f32 / 255.0;
                px[c] = (((s * a + o * old_a * (1.0 - a)) / out_a) * 255.0).clamp(0.0, 255.0) as u8;
            }
            px[3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
            dst.put_pixel(x, y, image::Rgba(px));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    Rendered,
    ItemTexture,
    BlockTexture,
    Checker,
}

fn checker() -> RgbaImage {
    let mut img = RgbaImage::new(FLAT_ICON, FLAT_ICON);
    for y in 0..FLAT_ICON {
        for x in 0..FLAT_ICON {
            let dark = ((x / 4) + (y / 4)) % 2 == 0;
            let px = if dark {
                [0, 0, 0, 255]
            } else {
                [248, 0, 248, 255]
            };
            img.put_pixel(x, y, image::Rgba(px));
        }
    }
    img
}

fn bake_potion_variants(
    assets: &Assets,
) -> Vec<(String, (Vec<(u32, RgbaImage)>, Dispatch, Source))> {
    let mut out = Vec::new();
    let entry = |image| (vec![(FLAT_ICON, image)], Dispatch::Model, Source::Rendered);
    for item in potions::POTION_ITEMS {
        let model = assets.model(&format!("item/{item}"));
        for color in potions::baked_colors() {
            let mut image = RgbaImage::new(FLAT_ICON, FLAT_ICON);
            if !flat_icon(assets, &model, &[rgb_of(color as i32)], &mut image) {
                continue;
            }
            out.push((potions::tint_key(item, color), entry(image)));
        }
        for layer in 0..2 {
            let Some(path) = model.texture_path(&format!("#{}", LAYERS[layer])) else {
                continue;
            };
            let Some(tex) = assets.texture(&path) else {
                continue;
            };
            let mut image = RgbaImage::new(FLAT_ICON, FLAT_ICON);
            blit_scaled(&tex, [1.0; 3], &mut image);
            out.push((potions::layer_key(item, layer), entry(image)));
        }
    }
    out
}

fn bake_one(
    assets: &Assets,
    id: &str,
    block_gui: Transform,
) -> (Vec<(u32, RgbaImage)>, Dispatch, Source) {
    let root = assets
        .json(&format!("items/{}.json", id))
        .and_then(|doc| doc.get("model").cloned().map(Arc::new));
    let dispatch = root
        .as_ref()
        .map(|n| Dispatch::of(n))
        .unwrap_or(Dispatch::Unknown);

    let mut parts = Vec::new();
    if let Some(node) = &root {
        collect_parts(assets, node, "gui", false, Mat4::IDENTITY, &mut parts, 0);
    }

    let mut quads: Vec<Quad> = Vec::new();
    let mut transform: Option<Transform> = None;
    let mut flat: Vec<(Arc<Resolved>, Vec<[f32; 3]>)> = Vec::new();

    for part in parts.iter() {
        let model = assets.model(&part.model);
        match &part.synthetic {
            Some(Synthetic::Chest(texture)) => {
                if let Some(tex) = assets.texture(texture) {
                    chest_quads(&tex, &mut quads);
                    transform.get_or_insert(model.gui.unwrap_or(block_gui));
                }
            }
            Some(Synthetic::Bed(texture, is_head, local)) => {
                if let Some(tex) = assets.texture(texture) {
                    let layer = if *is_head {
                        models::bed_head()
                    } else {
                        models::bed_foot()
                    };
                    let baked = geom::bake(&layer);
                    baked_quads(&baked, *local, &tex, &mut quads);
                    transform.get_or_insert(model.gui.unwrap_or(block_gui));
                }
            }
            Some(Synthetic::Skull(kind, local)) => {
                if let Some(tex) = assets.texture(skull::texture_for_type(kind)) {
                    let baked = geom::bake(&skull::layer_for_type(kind));
                    baked_quads(&baked, *local, &tex, &mut quads);
                    transform.get_or_insert(model.gui.unwrap_or(block_gui));
                }
            }
            Some(Synthetic::Banner(color, local)) => {
                let before = quads.len();
                banner_quads(assets, *color, &[], *local, &mut quads);
                if quads.len() > before {
                    transform.get_or_insert(model.gui.unwrap_or(block_gui));
                }
            }
            Some(Synthetic::Shield(local)) => {
                let before = quads.len();
                shield_quads(assets, &[], *local, &mut quads);
                if quads.len() > before {
                    transform.get_or_insert(model.gui.unwrap_or(block_gui));
                }
            }
            Some(Synthetic::Rigid(layer, texture, local)) => {
                let before = quads.len();
                rigid_quads(assets, *layer, texture, *local, &mut quads);
                if quads.len() > before {
                    transform.get_or_insert(model.gui.unwrap_or(block_gui));
                }
            }
            Some(Synthetic::Cube(explicit)) => {
                let path = explicit
                    .clone()
                    .or_else(|| model.texture_path("#particle"))
                    .unwrap_or_default();
                if let Some(tex) = assets.texture(&path) {
                    cube_quads(&tex, &mut quads);
                    transform.get_or_insert(model.gui.unwrap_or(block_gui));
                }
            }
            None if model.generated => flat.push((model, part.tints.clone())),
            None => {
                model_quads(assets, &model, &part.tints, &mut quads);
                transform.get_or_insert(model.gui.unwrap_or(Transform::NONE));
            }
        }
    }

    let size = if quads.is_empty() {
        FLAT_ICON
    } else {
        MODEL_ICON
    };
    let mut image = RgbaImage::new(size, size);
    let mut flat_drew = false;
    for (model, tints) in &flat {
        flat_drew |= flat_icon(assets, model, tints, &mut image);
    }

    if !quads.is_empty() {
        let t = transform.unwrap_or(Transform::NONE);
        if !flat_drew {
            let images = MODEL_ICON_TIERS
                .iter()
                .map(|&s| (s, raster::render(&quads, &t, s)))
                .collect();
            return (images, dispatch, Source::Rendered);
        }
        let rendered = raster::render(&quads, &t, size);
        for y in 0..size {
            for x in 0..size {
                let px = rendered.get_pixel(x, y).0;
                if px[3] > 0 {
                    image.put_pixel(x, y, image::Rgba(px));
                }
            }
        }
        return (vec![(size, image)], dispatch, Source::Rendered);
    }
    if flat_drew {
        return (vec![(size, image)], dispatch, Source::Rendered);
    }

    for (folder, source) in [
        ("item", Source::ItemTexture),
        ("block", Source::BlockTexture),
    ] {
        if let Some(tex) = assets.texture(&format!("{}/{}", folder, id)) {
            let mut img = RgbaImage::new(FLAT_ICON, FLAT_ICON);
            blit_scaled(&tex, [1.0; 3], &mut img);
            return (vec![(FLAT_ICON, img)], dispatch, source);
        }
    }
    (vec![(FLAT_ICON, checker())], dispatch, Source::Checker)
}

pub fn bake_item_icons(assets: &Path) -> ItemIcons {
    let start = Instant::now();
    let cache = Assets::new(assets);

    let mut ids: Vec<String> = crate::platform::assets::read_dir(assets.join("items"))
        .iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();
    ids.sort();

    let block_gui = cache.model("block/block").gui.unwrap_or(Transform::NONE);

    let threads = if cfg!(target_arch = "wasm32") {
        1
    } else {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(1, 12)
    };
    let chunk = ids.len().div_ceil(threads.max(1)).max(1);
    let mut results: Vec<(Vec<(u32, RgbaImage)>, Dispatch, Source)> = Vec::with_capacity(ids.len());

    #[cfg(target_arch = "wasm32")]
    {
        let cache = &cache;
        results.extend(ids.iter().map(|id| bake_one(cache, id, block_gui)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    std::thread::scope(|scope| {
        let cache = &cache;
        let handles: Vec<_> = ids
            .chunks(chunk)
            .map(|slice| {
                scope.spawn(move || {
                    slice
                        .iter()
                        .map(|id| bake_one(cache, id, block_gui))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            match handle.join() {
                Ok(part) => results.extend(part),
                Err(_) => results.extend((0..chunk).map(|_| {
                    (
                        vec![(FLAT_ICON, checker())],
                        Dispatch::Unknown,
                        Source::Checker,
                    )
                })),
            }
        }
    });
    results.truncate(ids.len());
    while results.len() < ids.len() {
        results.push((
            vec![(FLAT_ICON, checker())],
            Dispatch::Unknown,
            Source::Checker,
        ));
    }

    let variants = bake_potion_variants(&cache);
    let variant_count = variants.len();
    ids.reserve(variants.len());
    results.reserve(variants.len());
    for (key, baked) in variants {
        ids.push(key);
        results.push(baked);
    }

    let mut order: Vec<(usize, usize)> = results
        .iter()
        .enumerate()
        .flat_map(|(i, (images, ..))| (0..images.len()).map(move |t| (i, t)))
        .collect();
    order.sort_by_key(|&(i, t)| std::cmp::Reverse(results[i].0[t].0));
    let mut rects: Vec<Vec<[u32; 4]>> = results
        .iter()
        .map(|(images, ..)| vec![[0u32; 4]; images.len()])
        .collect();
    let (mut sx, mut sy, mut shelf_h) = (0u32, 0u32, 0u32);
    for &(i, t) in &order {
        let s = results[i].0[t].0;
        if sx + s > PACK_WIDTH {
            sx = 0;
            sy += shelf_h;
            shelf_h = 0;
        }
        rects[i][t] = [sx, sy, s, s];
        sx += s;
        shelf_h = shelf_h.max(s);
    }

    let mut image = RgbaImage::new(PACK_WIDTH, (sy + shelf_h).max(1));
    let mut map = HashMap::with_capacity(ids.len());
    let mut dispatch_counts: HashMap<&'static str, u32> = HashMap::new();
    let mut source_counts: HashMap<&'static str, u32> = HashMap::new();
    let mut fell_back: Vec<&str> = Vec::new();

    for (index, (id, (images, dispatch, source))) in ids.iter().zip(results.iter()).enumerate() {
        let mut tiers = Vec::with_capacity(images.len());
        for (t, (size, icon)) in images.iter().enumerate() {
            let [x, y, w, h] = rects[index][t];
            for py in 0..h {
                for px in 0..w {
                    image.put_pixel(x + px, y + py, *icon.get_pixel(px, py));
                }
            }
            tiers.push((*size, [x, y, w, h]));
        }
        map.insert(id.clone(), tiers);
        if *source != Source::Rendered {
            fell_back.push(id);
        }
        *dispatch_counts.entry(dispatch.name()).or_default() += 1;
        *source_counts
            .entry(match source {
                Source::Rendered => "rendered",
                Source::ItemTexture => "item texture fallback",
                Source::BlockTexture => "block texture fallback",
                Source::Checker => "checker",
            })
            .or_default() += 1;
    }

    let mut dispatch_report: Vec<_> = dispatch_counts.into_iter().collect();
    dispatch_report.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let mut source_report: Vec<_> = source_counts.into_iter().collect();
    source_report.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    eprintln!(
        "item icons: {} items (+{variant_count} potion variants), {}x{} atlas, {:.2?} on {} threads",
        ids.len() - variant_count,
        image.width(),
        image.height(),
        start.elapsed(),
        threads
    );
    eprintln!("item icons: dispatch {:?}", dispatch_report);
    eprintln!("item icons: source {:?}", source_report);
    if !fell_back.is_empty() {
        eprintln!("item icons: no model geometry for {:?}", fell_back);
    }

    ItemIcons { image, map }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASSETS: &str = "reference/minecraft-26.1.1/assets/minecraft";

    #[test]
    fn parse_transformation_matches_vanillas_compose_order() {
        let node = Json::parse(
            r#"{
                "transformation": {
                    "left_rotation": [1.0, 0.0, 0.0, -0.0],
                    "right_rotation": [0.0, 0.0, 0.0, 1.0],
                    "scale": [1.0, 1.0, 1.0],
                    "translation": [0.5, 0.0, 0.5]
                }
            }"#,
        )
        .unwrap();
        let m = parse_transformation(&node);
        let p = m.transform_point3(Vec3::new(0.0, 1.0, 0.0));
        assert!(p.abs_diff_eq(Vec3::new(0.5, -1.0, 0.5), 1e-6), "{p:?}");
    }

    #[test]
    fn a_missing_transformation_is_the_identity() {
        let node = Json::parse(r#"{}"#).unwrap();
        assert_eq!(parse_transformation(&node), Mat4::IDENTITY);
    }

    #[test]
    fn a_special_inherits_its_ancestors_transformation() {
        let assets = Path::new(ASSETS);
        if !assets.exists() {
            eprintln!("assets missing, skipping");
            return;
        }
        let cache = Assets::new(assets);
        let root = cache
            .json("items/shield.json")
            .and_then(|doc| doc.get("model").cloned())
            .expect("items/shield.json");
        let mut parts = Vec::new();
        collect_parts(&cache, &root, "gui", false, Mat4::IDENTITY, &mut parts, 0);
        let Some(Synthetic::Shield(local)) = parts.first().and_then(|p| p.synthetic.as_ref())
        else {
            panic!("shield resolved to {:?}", parts.first());
        };
        assert_eq!(
            *local,
            Mat4::from_scale(Vec3::new(1.0, -1.0, -1.0)),
            "the condition's transformation was dropped"
        );
    }

    #[test]
    fn a_banner_item_resolves_to_a_dyed_banner() {
        let assets = Path::new(ASSETS);
        if !assets.exists() {
            eprintln!("assets missing, skipping");
            return;
        }
        let cache = Assets::new(assets);
        for (index, name) in crate::blockentities::banner::NAMES.iter().enumerate() {
            let root = cache
                .json(&format!("items/{name}_banner.json"))
                .and_then(|doc| doc.get("model").cloned())
                .unwrap_or_else(|| panic!("no items/{name}_banner.json"));
            let mut parts = Vec::new();
            collect_parts(&cache, &root, "gui", false, Mat4::IDENTITY, &mut parts, 0);
            let synthetic = parts.first().and_then(|p| p.synthetic.as_ref());
            assert!(
                matches!(synthetic, Some(Synthetic::Banner(dye, _)) if *dye == index as u8),
                "{name}_banner resolved to {synthetic:?}",
            );
        }
    }

    #[test]
    fn the_banner_sheet_dyes_only_the_cloth() {
        let assets = Path::new(ASSETS);
        if !assets.exists() {
            eprintln!("assets missing, skipping");
            return;
        }
        let cache = Assets::new(assets);
        let plain = cache.texture("entity/banner/banner_base").unwrap();
        let black = pattern_tex(
            &cache,
            "entity/banner",
            "banner_base",
            std::iter::once(("base", 15u8)),
        )
        .unwrap();
        assert_ne!(black.get(20, 20), plain.get(20, 20), "cloth not dyed");
        assert_eq!(black.get(46, 10), plain.get(46, 10), "pole was dyed");
        assert_eq!(black.get(10, 44), plain.get(10, 44), "bar was dyed");
        assert!(black.get(20, 20)[0] < plain.get(20, 20)[0]);
    }

    #[test]
    fn bake_and_dump() {
        let assets = Path::new(ASSETS);
        if !assets.exists() {
            eprintln!("assets missing, skipping");
            return;
        }
        let icons = bake_item_icons(assets);
        assert!(icons.map.len() > 1400, "only {} icons", icons.map.len());

        let out = crate::gui::preview::output_dir();
        std::fs::create_dir_all(&out).unwrap();
        icons.image.save(out.join("item_icons.png")).unwrap();

        let samples = [
            "stone",
            "grass_block",
            "oak_stairs",
            "oak_slab",
            "torch",
            "chest",
            "crafting_table",
            "diamond_sword",
            "apple",
            "oak_door",
            "glass",
            "oak_leaves",
            "redstone_torch",
            "cake",
            "ladder",
            "water_bucket",
            "potion",
            "leather_helmet",
            "zombie_spawn_egg",
            "end_rod",
            "player_head",
            "skeleton_skull",
            "piglin_head",
            "dragon_head",
            "shield",
            "conduit",
            "shulker_box",
            "red_shulker_box",
            "decorated_pot",
            "copper_golem_statue",
        ];
        let mut sheet = RgbaImage::new(MODEL_ICON * samples.len() as u32, MODEL_ICON);
        for (i, id) in samples.iter().enumerate() {
            let tiers = icons
                .map
                .get(*id)
                .unwrap_or_else(|| panic!("missing {}", id));
            let (_, rect) = tiers.last().unwrap();
            let f = MODEL_ICON / rect[2];
            let mut icon = RgbaImage::new(rect[2], rect[3]);
            for y in 0..rect[3] {
                for x in 0..rect[2] {
                    let px = *icons.image.get_pixel(rect[0] + x, rect[1] + y);
                    icon.put_pixel(x, y, px);
                    for sy in 0..f {
                        for sx in 0..f {
                            sheet.put_pixel(i as u32 * MODEL_ICON + x * f + sx, y * f + sy, px);
                        }
                    }
                }
            }
            icon.save(out.join(format!("icon_{}.png", id))).unwrap();
        }
        sheet.save(out.join("icon_samples.png")).unwrap();

        let mut sparse: Vec<&str> = Vec::new();
        let mut blank_white: Vec<&str> = Vec::new();
        for (id, tiers) in &icons.map {
            let (_, rect) = tiers.last().unwrap();
            let mut covered = 0u32;
            let mut colored = 0u32;
            for y in 0..rect[3] {
                for x in 0..rect[2] {
                    let px = icons.image.get_pixel(rect[0] + x, rect[1] + y).0;
                    if px[3] > 16 {
                        covered += 1;
                        if px[0].min(px[1]).min(px[2]) < 245 {
                            colored += 1;
                        }
                    }
                }
            }
            assert!(covered > 0, "{} baked to an empty icon", id);
            if covered * 20 < rect[2] * rect[3] {
                sparse.push(id);
            }
            if colored * 50 < covered {
                blank_white.push(id);
            }
        }
        sparse.sort_unstable();
        blank_white.sort_unstable();
        eprintln!("under 5% coverage: {:?}", sparse);
        eprintln!("all but white: {:?}", blank_white);
        assert!(sparse.is_empty(), "sparse icons: {:?}", sparse);
    }
}
