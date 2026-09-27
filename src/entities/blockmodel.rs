use std::sync::LazyLock;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::Mesh;

use crate::blocks::BakedQuad;
use crate::blocks::state::Rot;
use crate::entities::BlockRef;

static MODELS: LazyLock<crate::items::model::Assets> =
    LazyLock::new(|| crate::items::model::Assets::new(&crate::assets_root()));

pub fn mesh(block: BlockRef) -> Option<Mesh> {
    let mut out = QuadBuf::default();
    match block {
        BlockRef::State(id) => {
            let baked = state_model(id)?;
            for part in &baked.parts {
                let model = part.choices.first()?;
                for quad in &model.quads {
                    out.push(quad);
                }
            }
        }
        BlockRef::Model(model) => {
            let atlas_rows = crate::ATLAS_ROWS.get().copied()?;
            let baked =
                crate::blocks::bake::bake_model(&MODELS, model.path(), Rot::default(), atlas_rows);
            for quad in &baked.quads {
                out.push(quad);
            }
        }
    }
    out.finish()
}

fn state_model(id: u32) -> Option<std::sync::Arc<crate::blocks::BakedBlock>> {
    use azalea::block::{BlockState, BlockTrait};

    let state = BlockState::try_from(id).ok()?;
    if state.is_air() {
        return None;
    }
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    let baked = crate::blocks::registry().baked(state.id(), block.id(), &block.property_map());
    Some(baked)
}

#[derive(Default)]
struct QuadBuf {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl QuadBuf {
    fn push(&mut self, quad: &BakedQuad) {
        let base = self.positions.len() as u32;
        let normal = quad.normal;
        let shade = quad.shade(crate::renderer::dimension::current().cardinal());
        for i in 0..4 {
            self.positions.push(quad.pos[i]);
            self.uvs.push(quad.uv[i]);
            self.normals.push(normal);
            self.colors.push([1.0, 1.0, 1.0, shade]);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn finish(self) -> Option<Mesh> {
        if self.positions.is_empty() {
            return None;
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        Some(mesh)
    }
}

#[cfg(test)]
fn quad_count(model: &crate::blocks::bake::BakedModel) -> usize {
    model.quads.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_item_frame_models_resolve() {
        for path in [
            "block/item_frame",
            "block/item_frame_map",
            "block/glow_item_frame",
        ] {
            let resolved = MODELS.model(path);
            assert!(
                !resolved.elements.is_empty(),
                "{path} resolved to no elements"
            );
            let baked = crate::blocks::bake::bake_resolved(&resolved, Rot::default(), 64);
            assert!(
                quad_count(&baked) > 0 || baked.missing_textures > 0,
                "{path} baked to nothing at all"
            );
        }
    }
}
