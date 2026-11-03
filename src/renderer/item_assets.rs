use std::borrow::Cow;
use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::items::dispatch::Needles;
use crate::items::mesh::{ItemMeshes, ModelTree, Transform as ItemTransform};
use crate::items::model::{DisplayContext, DisplayTransforms};
use crate::renderer::anim::HeldItem;

pub use crate::items::mesh::{ChargeType, ItemUse, ItemWorld, Owner};

impl ItemUse {
    pub fn of(item: &HeldItem, using: bool, use_ticks: f32) -> ItemUse {
        let charge = match (item.charged, item.rocket) {
            (false, _) => ChargeType::None,
            (true, false) => ChargeType::Arrow,
            (true, true) => ChargeType::Rocket,
        };
        ItemUse {
            using,
            use_ticks: if using { use_ticks } else { 0.0 },
            charge,
            trim: item.trim,
            has_lodestone: item.has_lodestone,
            lodestone: item.lodestone,
            owner: None,
        }
    }
}

#[derive(Clone)]
pub struct ItemGpu {
    pub mesh: Handle<Mesh>,
    pub texture: Handle<Image>,
    pub display: DisplayTransforms,
    pub bounds: (Vec3, Vec3),
}

impl ItemGpu {
    pub fn transform(&self, slot: &str) -> ItemTransform {
        DisplayContext::parse(slot).map_or(ItemTransform::NONE, |ctx| self.for_context(ctx))
    }

    pub fn for_context(&self, ctx: DisplayContext) -> ItemTransform {
        self.display.for_context(ctx)
    }
}

#[derive(Resource)]
pub struct ItemAssets {
    builder: ItemMeshes,
    built: HashMap<String, Built>,
    trees: HashMap<String, Option<ModelTree>>,
    swap_scale: HashMap<&'static str, f32>,
    world: ItemWorld,
    needles: Needles,
    local: Option<Owner>,
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
    use_: ItemUse,
    world: ItemWorld,
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
            trees: HashMap::new(),
            swap_scale: HashMap::new(),
            world: ItemWorld::default(),
            needles: Needles::default(),
            local: None,
        }
    }

    pub fn begin_frame(&mut self, view: &crate::renderer::frame_view::FrameView, sun_angle: f32) {
        let local = &view.local;
        let yaw = local.body_yaw + local.head_yaw;
        self.world = self.needles.step(
            view.game_time,
            yaw,
            sun_angle,
            view.compass_spawn,
            view.compass_death,
            crate::renderer::dimension::level_name(),
        );
        self.local = view.in_world.then_some(Owner {
            pos: view.player_lerp,
            yaw,
            local: true,
            seed: 0,
        });
    }

    pub fn world(&self) -> ItemWorld {
        self.world
    }

    pub fn local_owner(&self) -> Option<Owner> {
        self.local
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
                let parts = parts(builder, &args.id, &args.ctx, &args.use_, &args.world);
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
        self.trees.clear();
        self.swap_scale.clear();
        self.built.retain(|key, _| !key.contains(['@', '/']));
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

    fn tree(&mut self, id: &str) -> Option<&ModelTree> {
        if !self.trees.contains_key(id) {
            let tree = self.builder.tree(id);
            self.trees.insert(id.to_string(), tree);
        }
        self.trees.get(id).and_then(Option::as_ref)
    }

    pub fn branch(&mut self, id: &str, ctx: &str, use_: ItemUse) -> u32 {
        if id.is_empty() {
            return 0;
        }
        let world = self.world;
        self.tree(id)
            .map_or(0, |tree| u32::from(tree.eval(ctx, &use_, &world)) + 1)
    }

    pub fn get(
        &mut self,
        id: &str,
        ctx: &str,
        use_: ItemUse,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
    ) -> Option<&ItemGpu> {
        if id.is_empty() {
            return None;
        }
        let world = self.world;
        self.tree(id);
        let tree = self.trees.get(id).and_then(Option::as_ref);
        let key = match tree {
            None => Cow::Borrowed(id),
            Some(tree) => {
                let leaf = tree.eval(ctx, &use_, &world);
                match tree.leaf(leaf).icon.as_deref() {
                    Some(path) => Cow::Borrowed(path),
                    None => Cow::Owned(format!("{id}@{leaf}")),
                }
            }
        };
        if !self.built.contains_key(key.as_ref()) {
            let gpu = parts(&self.builder, id, ctx, &use_, &world).map(|parts| ItemGpu {
                mesh: meshes.add(parts.mesh),
                texture: images.add(parts.texture),
                display: parts.display,
                bounds: parts.bounds,
            });
            let args = ItemArgs {
                key: key.to_string(),
                id: id.to_owned(),
                ctx: ctx.to_owned(),
                use_,
                world,
            };
            self.built.insert(key.to_string(), Built { args, gpu });
        }
        self.built
            .get(key.as_ref())
            .and_then(|built| built.gpu.as_ref())
    }
}

pub fn step_item_world(
    view: Res<crate::renderer::frame_view::FrameView>,
    environment: Res<crate::renderer::environment::Environment>,
    mut assets: ResMut<ItemAssets>,
) {
    assets.begin_frame(&view, environment.sky.sun_angle);
}

fn parts(
    builder: &ItemMeshes,
    id: &str,
    ctx: &str,
    use_: &ItemUse,
    world: &ItemWorld,
) -> Option<ItemParts> {
    let built = builder.build(id, ctx, use_, world)?;
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
