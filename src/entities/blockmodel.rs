use bevy::prelude::Mesh;

use crate::blocks::BakedQuad;
use crate::blocks::state::Rot;
use crate::entities::BlockRef;
use crate::entities::quads::QuadMesh;

pub fn mesh(block: BlockRef) -> Option<Mesh> {
    mesh_in(&crate::blocks::registry(), block)
}

pub fn mesh_in(blocks: &crate::blocks::Blocks, block: BlockRef) -> Option<Mesh> {
    let mut out = QuadMesh::new();
    match block {
        BlockRef::State(id) => {
            let baked = state_model(blocks, id)?;
            for part in &baked.parts {
                let model = part.choices.first()?;
                for quad in &model.quads {
                    push(&mut out, quad);
                }
            }
        }
        BlockRef::Model(model) => {
            let baked = crate::blocks::bake::bake_model(
                blocks.assets(),
                model.path(),
                Rot::default(),
                blocks.sprites(),
            );
            for quad in &baked.quads {
                push(&mut out, quad);
            }
        }
    }
    (!out.is_empty()).then(|| out.finish())
}

pub(crate) fn state_model(
    blocks: &crate::blocks::Blocks,
    id: u32,
) -> Option<std::sync::Arc<crate::blocks::BakedBlock>> {
    use azalea::block::{BlockState, BlockTrait};

    let state = BlockState::try_from(id).ok()?;
    if state.is_air() {
        return None;
    }
    let block: Box<dyn BlockTrait> = Box::<dyn BlockTrait>::from(state);
    Some(blocks.baked(state.id(), block.id(), &block.property_map()))
}

fn push(out: &mut QuadMesh, quad: &BakedQuad) {
    push_at(out, quad, [0.0; 3]);
}

pub(crate) fn push_at(out: &mut QuadMesh, quad: &BakedQuad, offset: [f32; 3]) {
    let shade = quad.shade(crate::renderer::dimension::current().cardinal());
    let pos = quad
        .pos
        .map(|c| [c[0] + offset[0], c[1] + offset[1], c[2] + offset[2]]);
    out.colored(pos, quad.uv, quad.normal, [1.0, 1.0, 1.0, shade]);
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
            let resolved = crate::blocks::registry().assets().model(path);
            assert!(
                !resolved.elements.is_empty(),
                "{path} resolved to no elements"
            );
            let baked = crate::blocks::bake::bake_resolved(
                &resolved,
                Rot::default(),
                &crate::renderer::Sprites::default(),
            );
            assert!(
                quad_count(&baked) > 0 || baked.missing_textures > 0,
                "{path} baked to nothing at all"
            );
        }
    }
}
