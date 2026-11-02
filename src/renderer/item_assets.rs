use std::borrow::Cow;
use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::items::mesh::{ItemMeshes, Transform as ItemTransform};
use crate::items::model::{DisplayContext, DisplayTransforms};

#[derive(Clone)]
pub struct ItemGpu {
    pub mesh: Handle<Mesh>,
    pub texture: Handle<Image>,
    pub display: DisplayTransforms,
    pub bounds: (Vec3, Vec3),
}

impl ItemGpu {
    pub fn transform(&self, slot: &str) -> ItemTransform {
        self.display.get_named(slot).unwrap_or(ItemTransform::NONE)
    }

    pub fn for_context(&self, ctx: DisplayContext) -> ItemTransform {
        self.display.for_context(ctx)
    }
}

#[derive(Resource)]
pub struct ItemAssets {
    builder: ItemMeshes,
    built: HashMap<String, Built>,
    contextual: HashMap<String, bool>,
    using_sensitive: HashMap<String, bool>,
    swap_scale: HashMap<&'static str, f32>,
}

struct Built {
    args: ItemArgs,
    gpu: Option<ItemGpu>,
}

#[derive(Clone)]
pub struct ItemArgs {
    key: String,
    id: String,
    ctx: String,
    using: bool,
}

pub struct ItemParts {
    mesh: Mesh,
    texture: Image,
    display: DisplayTransforms,
    bounds: (Vec3, Vec3),
}

impl ItemAssets {
    pub fn new() -> ItemAssets {
        ItemAssets {
            builder: ItemMeshes::new(crate::blocks::registry().assets().clone()),
            built: HashMap::new(),
            contextual: HashMap::new(),
            using_sensitive: HashMap::new(),
            swap_scale: HashMap::new(),
        }
    }

    pub fn swap_animation_scale(&mut self, id: &'static str) -> f32 {
        if id.is_empty() {
            return 1.0;
        }
        let builder = &self.builder;
        *self
            .swap_scale
            .entry(id)
            .or_insert_with(|| builder.swap_animation_scale(id))
    }

    pub fn reload_args(&mut self) -> Vec<ItemArgs> {
        self.built.retain(|_, built| built.gpu.is_some());
        self.built
            .values()
            .map(|built| built.args.clone())
            .collect()
    }

    pub fn rebuild(
        builder: &ItemMeshes,
        args: Vec<ItemArgs>,
    ) -> Vec<(ItemArgs, Option<ItemParts>)> {
        args.into_iter()
            .map(|args| {
                let parts = parts(builder, &args.id, &args.ctx, args.using);
                (args, parts)
            })
            .collect()
    }

    pub fn adopt(
        &mut self,
        builder: ItemMeshes,
        rebuilt: Vec<(ItemArgs, Option<ItemParts>)>,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
    ) {
        self.builder = builder;
        self.contextual.clear();
        self.using_sensitive.clear();
        self.swap_scale.clear();
        for (args, parts) in rebuilt {
            let Some(gpu) = self
                .built
                .get_mut(&args.key)
                .and_then(|built| built.gpu.as_mut())
            else {
                continue;
            };
            match parts {
                Some(parts) => {
                    let _ = meshes.insert(&gpu.mesh, parts.mesh);
                    let _ = images.insert(&gpu.texture, parts.texture);
                    gpu.display = parts.display;
                    gpu.bounds = parts.bounds;
                }
                None => {
                    self.built.remove(&args.key);
                }
            }
        }
    }

    pub fn bake_pattern_icon(&self, key: &str, edge: u32) -> Option<image::RgbaImage> {
        self.builder.bake_pattern_icon(key, edge)
    }

    pub fn get(
        &mut self,
        id: &str,
        ctx: &str,
        using: bool,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
    ) -> Option<&ItemGpu> {
        if id.is_empty() {
            return None;
        }
        let contextual = match self.contextual.get(id).copied() {
            Some(flag) => flag,
            None => {
                let flag = self.builder.context_sensitive(id);
                self.contextual.insert(id.to_string(), flag);
                flag
            }
        };
        let using_sensitive = match self.using_sensitive.get(id).copied() {
            Some(flag) => flag,
            None => {
                let flag = self.builder.depends_on_using_item(id);
                self.using_sensitive.insert(id.to_string(), flag);
                flag
            }
        };
        let key = match (contextual, using_sensitive && using) {
            (false, false) => Cow::Borrowed(id),
            (true, false) => Cow::Owned(format!("{id}@{ctx}")),
            (false, true) => Cow::Owned(format!("{id}@using")),
            (true, true) => Cow::Owned(format!("{id}@{ctx}@using")),
        };
        if !self.built.contains_key(key.as_ref()) {
            let gpu = parts(&self.builder, id, ctx, using).map(|parts| ItemGpu {
                mesh: meshes.add(parts.mesh),
                texture: images.add(parts.texture),
                display: parts.display,
                bounds: parts.bounds,
            });
            let args = ItemArgs {
                key: key.to_string(),
                id: id.to_owned(),
                ctx: ctx.to_owned(),
                using,
            };
            self.built.insert(key.to_string(), Built { args, gpu });
        }
        self.built
            .get(key.as_ref())
            .and_then(|built| built.gpu.as_ref())
    }
}

fn parts(builder: &ItemMeshes, id: &str, ctx: &str, using: bool) -> Option<ItemParts> {
    let built = builder.build(id, ctx, using)?;
    if built.vertices.is_empty() {
        return None;
    }

    let texture = super::rgba_image(
        built.image.width(),
        built.image.height(),
        built.image.into_raw(),
        RenderAssetUsages::RENDER_WORLD,
    );

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        built.vertices.iter().map(|v| v.pos).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        built.vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        built.vertices.iter().map(|v| v.normal).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        built
            .vertices
            .iter()
            .map(|v| {
                [
                    crate::util::mth::srgb_to_linear(v.color[0]),
                    crate::util::mth::srgb_to_linear(v.color[1]),
                    crate::util::mth::srgb_to_linear(v.color[2]),
                    1.0,
                ]
            })
            .collect::<Vec<_>>(),
    );
    mesh.insert_indices(Indices::U32(built.indices));

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for v in &built.vertices {
        min = min.min(Vec3::from(v.pos));
        max = max.max(Vec3::from(v.pos));
    }

    Some(ItemParts {
        mesh,
        texture,
        display: built.display,
        bounds: (min, max),
    })
}
