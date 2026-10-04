use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::{
    LoadOp, Operations, RenderPassColorAttachment, RenderPassDepthStencilAttachment, StoreOp,
    Texture, TextureView,
};

use super::super::compile::PACK_QUADS_SET;
use super::super::plan::Attachment;
use crate::renderer::terrain_pool::draw::StreamSources;

pub(super) fn clear_op([r, g, b, a]: [f32; 4]) -> LoadOp<wgpu::Color> {
    LoadOp::Clear(wgpu::Color {
        r: f64::from(r),
        g: f64::from(g),
        b: f64::from(b),
        a: f64::from(a),
    })
}

pub(super) fn color_attachment(
    view: &TextureView,
    load: LoadOp<wgpu::Color>,
) -> Option<RenderPassColorAttachment<'_>> {
    Some(RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load,
            store: StoreOp::Store,
        },
    })
}

pub(super) fn load_attachments(
    attachments: &[Attachment],
) -> Vec<Option<RenderPassColorAttachment<'_>>> {
    attachments
        .iter()
        .map(|a| color_attachment(&a.view, LoadOp::Load))
        .collect()
}

pub(super) fn depth_attachment(
    view: &TextureView,
    load: LoadOp<f32>,
) -> RenderPassDepthStencilAttachment<'_> {
    RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(Operations {
            load,
            store: StoreOp::Store,
        }),
        stencil_ops: None,
    }
}

pub(super) fn copy_whole(encoder: &mut wgpu::CommandEncoder, from: &Texture, to: &Texture) {
    encoder.copy_texture_to_texture(from.as_image_copy(), to.as_image_copy(), from.size());
}

pub(super) fn draw_pools<'w>(
    encoder: &mut TrackedRenderPass<'w>,
    sources: &StreamSources<'w>,
    mut draw: impl FnMut(&mut TrackedRenderPass<'w>, u32),
) {
    for (pool, quads) in sources
        .pools
        .pools
        .iter()
        .enumerate()
        .filter_map(|(at, p)| Some((at, p.pack_quads.as_ref()?)))
    {
        encoder.set_bind_group(PACK_QUADS_SET as usize, &quads.group, &[]);
        draw(encoder, pool as u32);
    }
}
